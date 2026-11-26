#!/usr/bin/env python3
"""校验追溯矩阵的目标测试例名：已自动化行必须在源码中有唯一测试函数。"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "docs" / "devPlan" / "traceability.md"

EXCLUDED_PREFIXES = (
    ".claude/",
    "assets/",
    "dist/",
    "slterm_terminal/vendor/",
    "target/",
)

STATUSES = {"待实现", "已自动化", "人工豁免", "延期"}
TEST_NAME_RE = re.compile(r"`([a-z][a-z0-9_]{5,})`")
TEST_ATTR_RE = re.compile(r"#\[(?:[\w:]+::)?(?:test|gpui::test)\b")
FUNCTION_RE = re.compile(
    r"(?P<attrs>(?:#\[[^\]]*\]\s*)+)(?:pub\s+)?(?:async\s+)?fn\s+"
    r"(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\("
)
SEPARATOR_CELL_RE = re.compile(r"^:?-+:?$")


def rust_files() -> list[Path]:
    files = []
    for path in ROOT.rglob("*.rs"):
        relative = path.relative_to(ROOT).as_posix()
        if any(relative.startswith(prefix) for prefix in EXCLUDED_PREFIXES):
            continue
        if ".git" in path.parts:
            continue
        files.append(path)
    return files


def collect_test_functions() -> dict[str, list[str]]:
    """返回测试例名到源码位置的映射；同名测试会保留全部位置。"""
    names: dict[str, list[str]] = defaultdict(list)
    for path in rust_files():
        text = path.read_text(encoding="utf-8", errors="ignore")
        for match in FUNCTION_RE.finditer(text):
            if TEST_ATTR_RE.search(match.group("attrs")):
                names[match.group("name")].append(
                    path.relative_to(ROOT).as_posix()
                )
    return names


def matrix_rows() -> list[tuple[int, list[str], str]]:
    """返回 (行号, 目标测试例名单元格锚点, 状态)；只解析带表头的矩阵行。"""
    rows = []
    header: list[str] | None = None
    for line_no, line in enumerate(MATRIX.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if not stripped.startswith("|"):
            header = None
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if "目标测试例名" in cells:
            header = cells
            continue
        if header is None or len(cells) != len(header):
            continue
        if all(SEPARATOR_CELL_RE.match(cell) for cell in cells):
            continue
        status = cells[-1]
        if status not in STATUSES:
            continue
        column = header.index("目标测试例名")
        rows.append((line_no, TEST_NAME_RE.findall(cells[column]), status))
    return rows


def collect_findings() -> list[str]:
    findings = []
    tests = collect_test_functions()
    for name, locations in sorted(tests.items()):
        if len(locations) > 1:
            findings.append(
                f"测试例名 `{name}` 在多个位置定义: {', '.join(sorted(locations))}"
            )

    owners: dict[str, list[int]] = defaultdict(list)
    for line_no, anchors, status in matrix_rows():
        for anchor in anchors:
            owners[anchor].append(line_no)
        if status != "已自动化":
            continue
        if not anchors:
            findings.append(f"追溯矩阵行 {line_no}: 已自动化行没有目标测试例名")
        for anchor in anchors:
            if anchor not in tests:
                findings.append(f"追溯矩阵行 {line_no}: `{anchor}` 在源码中没有测试函数")

    for anchor, lines in sorted(owners.items()):
        if len(lines) > 1:
            joined = "、".join(str(line) for line in lines)
            findings.append(f"测试例名 `{anchor}` 同时用于追溯矩阵行 {joined}")
    return findings


def main() -> int:
    try:
        findings = collect_findings()
    except OSError as error:
        print(f"Traceability check could not complete: {error}", file=sys.stderr)
        return 2
    for finding in findings:
        print(f"ERROR: {finding}")
    print(f"Traceability: {len(findings)} finding(s)")
    return int(bool(findings))


if __name__ == "__main__":
    raise SystemExit(main())
