#!/usr/bin/env python3
"""校验随包 ConPTY 运行时：固定 SHA256、x64 PE 架构与微软许可文本。"""

from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONPTY_DIR = ROOT / "slterm_terminal" / "vendor" / "conpty"
PE_MACHINE_X64 = 0x8664
ASSETS = (
    ("conpty.dll", "39fba2713e2495117b1591ae8c32a3b904bea7aa66069cf7815e2844c76d75d8"),
    (
        "OpenConsole.exe",
        "b7fd936c2668b87b9ecf7b3366dc6568afc1c6f981874cba3e955a1c35cf8160",
    ),
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pe_machine(path: Path) -> int:
    """读取 PE 头的 machine 字段；不是 PE 文件即失败。"""
    data = path.read_bytes()
    if len(data) < 0x40 or data[:2] != b"MZ":
        raise ValueError("缺少 MZ 头")
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if len(data) < offset + 6 or data[offset : offset + 4] != b"PE\0\0":
        raise ValueError("缺少 PE 签名")
    return struct.unpack_from("<H", data, offset + 4)[0]


def collect_findings() -> list[str]:
    findings = []
    for name, expected in ASSETS:
        path = CONPTY_DIR / name
        if not path.is_file():
            findings.append(f"{name} 不存在：{path}")
            continue
        actual = sha256(path)
        if actual != expected:
            findings.append(f"{name} SHA256 漂移：{actual} != {expected}")
        try:
            machine = pe_machine(path)
        except (OSError, ValueError) as error:
            findings.append(f"{name} 不是有效 PE：{error}")
            continue
        if machine != PE_MACHINE_X64:
            findings.append(f"{name} 架构不是 x64：0x{machine:04x}")
    license_path = CONPTY_DIR / "LICENSE"
    if not license_path.is_file():
        findings.append("缺少 ConPTY 许可文本")
    elif "Microsoft Corporation" not in license_path.read_text(encoding="utf-8"):
        findings.append("ConPTY 许可文本缺少 Microsoft 版权行")
    return findings


def main() -> int:
    findings = collect_findings()
    for finding in findings:
        print(f"ERROR: {finding}")
    print(f"Bundled runtime: {len(findings)} finding(s)")
    return int(bool(findings))


if __name__ == "__main__":
    raise SystemExit(main())
