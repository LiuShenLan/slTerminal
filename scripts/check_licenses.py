#!/usr/bin/env python3
"""生成并核验 Windows 生产依赖的第三方许可文本。"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
NOTICE_PATH = ROOT / "licenses" / "THIRD-PARTY-CRATE-NOTICES"
TARGET = "x86_64-pc-windows-msvc"
ROOT_PACKAGES = {"slterm", "slterm-hook"}
NATIVE_DOCUMENTS = (
    ("dav1d-sys", ROOT / "licenses" / "LICENSE-DAV1D"),
    ("libsqlite3-sys", ROOT / "licenses" / "LICENSE-SQLITE"),
)
TEXT_PREFIXES = ("license", "licence", "copying", "notice", "copyright")


def is_text_name(name: str) -> bool:
    """只匹配许可文件名及其后缀，排除 licenserc 等工具配置。"""
    name = name.lower()
    return any(
        name == prefix
        or name.startswith((f"{prefix}-", f"{prefix}.", f"{prefix}_"))
        for prefix in TEXT_PREFIXES
    )


class LicenseAuditError(Exception):
    """许可文本缺失或生成文件过期。"""


def production_packages(metadata: dict) -> list[dict]:
    """沿正常与构建依赖边遍历，排除 dev 依赖和 workspace 自有包。"""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [
        package["id"]
        for package in packages.values()
        if package["name"] in ROOT_PACKAGES
    ]
    if not pending:
        raise LicenseAuditError("生产依赖图缺少应用或 hook 根包")
    seen = set()
    while pending:
        package_id = pending.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        for dependency in nodes[package_id]["deps"]:
            if any(
                kind["kind"] in (None, "build")
                for kind in dependency["dep_kinds"]
            ):
                pending.append(dependency["pkg"])
    workspace = set(metadata["workspace_members"])
    external = [
        packages[package_id]
        for package_id in seen
        if package_id not in workspace
    ]
    return sorted(
        external,
        key=lambda package: (
            package["name"],
            package["version"],
            package["source"] or "path",
        ),
    )


def text_files(root: Path) -> list[Path]:
    if not root.is_dir():
        return []
    return sorted(
        path
        for path in root.iterdir()
        if path.is_file() and is_text_name(path.name)
    )


def select_checkout_files(package: dict, files: list[Path]) -> list[Path]:
    """Git workspace 根可能携带其他 crate 的许可，只取当前包适用文本。"""
    expression = (package.get("license") or "").lower()
    notices = [path for path in files if path.name.lower().startswith("notice")]
    licenses = [path for path in files if path not in notices]
    if "apache-2.0" in expression and "mit" not in expression:
        apache = [path for path in licenses if "apache" in path.name.lower()]
        if apache:
            licenses = apache
    return sorted(licenses + notices)


def license_files(package: dict) -> list[Path]:
    root = Path(package["manifest_path"]).parent
    direct = text_files(root)
    if direct:
        return direct
    source = package.get("source")
    if source is None:
        raise LicenseAuditError(f"{package['name']} 的仓内补丁缺少许可文本")
    if not source.startswith("git+"):
        # 部分 registry 包遗漏 workspace 根许可；只接受仓内精确版本快照。
        snapshot = (
            ROOT
            / "licenses"
            / "crates"
            / f"{package['name']}-{package['version']}"
        )
        files = text_files(snapshot)
        if files:
            return files
        raise LicenseAuditError(f"{package['name']} 缺少可定位许可文本")
    for parent in root.parents:
        files = select_checkout_files(package, text_files(parent))
        if files:
            return files
        if (parent / ".git").exists():
            break
    raise LicenseAuditError(f"{package['name']} 的 Git 来源缺少许可文本")


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8-sig").replace("\r\n", "\n")
    except (OSError, UnicodeError) as error:
        raise LicenseAuditError(f"无法读取许可文本 {path}") from error


def append_document(lines: list[str], root: Path, path: Path) -> None:
    try:
        relative = Path(os.path.relpath(path, root)).as_posix()
    except ValueError:
        # Windows 跨盘路径无相对形式；快照标签只保留确定性的文件名。
        relative = path.name
    lines.extend(
        [
            f"File: {relative}",
            "---------------------------------------------------------------------",
            read_text(path).rstrip("\n"),
            "",
        ]
    )


def render_notices(metadata: dict, extra_documents=()) -> str:
    packages = production_packages(metadata)
    lines = [
        "Third-party Rust crate and native source notices for slTerminal",
        "===================================================================",
        "",
        "Generated from the Windows x64 production dependency graph.",
        "Run scripts/check_licenses.py --write after dependency changes.",
        "",
    ]
    for package in packages:
        root = Path(package["manifest_path"]).parent
        lines.extend(
            [
                "=====================================================================",
                f"Package: {package['name']} {package['version']}",
                f"Source: {package['source'] or 'path'}",
                f"License: {package.get('license') or 'unspecified'}",
                "",
            ]
        )
        for path in license_files(package):
            append_document(lines, root, path)
    active = {package["name"] for package in packages}
    for name, raw_path in sorted(extra_documents):
        if name not in active:
            continue
        path = Path(raw_path)
        if not path.is_file():
            raise LicenseAuditError(f"{name} 缺少原生源许可文本 {path.name}")
        lines.extend(
            [
                "=====================================================================",
                f"Native source for: {name}",
                "",
            ]
        )
        append_document(lines, path.parent, path)
    return "\n".join(lines).rstrip("\n") + "\n"


def write_notice(metadata: dict, destination: Path, extra_documents=()) -> None:
    """同目录临时文件 + 原子替换，避免留下半份许可清单。"""
    destination = Path(destination)
    rendered = render_notices(metadata, extra_documents)
    descriptor, temporary = tempfile.mkstemp(
        dir=destination.parent,
        prefix=f".{destination.name}.",
        text=True,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(rendered)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
    except Exception:
        Path(temporary).unlink(missing_ok=True)
        raise


def check_notice(metadata: dict, destination: Path, extra_documents=()) -> None:
    destination = Path(destination)
    expected = render_notices(metadata, extra_documents)
    if not destination.is_file() or read_text(destination) != expected:
        raise LicenseAuditError(f"stale license notice: {destination}")


def load_metadata() -> dict:
    import json

    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            TARGET,
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return json.loads(result.stdout)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="原子更新随包许可清单")
    arguments = parser.parse_args()
    try:
        metadata = load_metadata()
        if arguments.write:
            write_notice(metadata, NOTICE_PATH, NATIVE_DOCUMENTS)
        else:
            check_notice(metadata, NOTICE_PATH, NATIVE_DOCUMENTS)
    except (LicenseAuditError, OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f"License audit failed: {error}", file=sys.stderr)
        return 1
    print(f"License audit: {len(production_packages(metadata))} production crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
