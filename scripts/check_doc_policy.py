#!/usr/bin/env python3
"""拒绝把过程编号、行号引用与失效文档路径写入文档或代码注释。"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

DOC_SUFFIXES = {".md"}
CODE_SUFFIXES = {".rs", ".py", ".ps1", ".toml", ".iss"}

EXCLUDED_PREFIXES = (
    "assets/corpus/",
    "assets/skills/",
    "slterm_terminal/vendor/",
    "target/",
    "dist/",
)
EXCLUDED_FILES = {
    "LICENSE",
    "THIRD-PARTY-NOTICES",
    "assets/runtime-control-api.md",
    "scripts/check_doc_policy.py",
    "scripts/tests/test_doc_policy.py",
}

URL_RE = re.compile(r"https?://\S+")
GUID_RE = re.compile(r"\{[0-9A-Fa-f-]{36}\}")

RULES = (
    ("phase", re.compile(r"\bM\d{1,2}(?:\.\d{1,2}[a-z]?)?\b")),
    ("rule", re.compile(r"\b(?:SEC|BE|TQ|PTY|CP|FE|IC|SVC|SPE|ADR|DOC|TE|NAV|TB|MC|ZQ|AC|IHE)[-_]\d+\b")),
    ("layer", re.compile(r"\bL[1-4]\b")),
    ("batch", re.compile(r"\b[ABCDR]\d{1,3}\b")),
    ("line-ref", re.compile(r"\b[\w./\\-]+\.(?:rs|md|py|ps1|toml|iss|json|ts):\d{1,5}\b")),
    ("dead-doc", re.compile(r"\.claude/adr\.md|\.claude/test-exemptions\.md|\.claude/rules/|architecture/notes|docs/FIX-PLAN\.md|docs/REFACTOR-ISSUES\.md")),
    ("doc-ref-in-code", re.compile(r"CLAUDE\.md|\.claude/docs/|CONTEXT\.md|release-notes/")),
)

ALLOW_TOKENS = {"C0", "C1", "R100"}
ALLOW_HINTS = {
    "C0": ("控制字符", "control"),
    "C1": ("控制字符", "control"),
    "L2": ("范数", "norm"),
}
BATCH_CONTEXT = ("阶段", "批次", "条目", "落点", "起", "归", "计划", "问题", "修复")


def git_lines(*args: str) -> list[str]:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True
    )
    return result.stdout.splitlines()


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True
    )
    return [ROOT / item for item in result.stdout.decode("utf-8").split("\0") if item]


def in_scope(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    if relative in EXCLUDED_FILES:
        return False
    if any(relative.startswith(prefix) for prefix in EXCLUDED_PREFIXES):
        return False
    return path.suffix in DOC_SUFFIXES | CODE_SUFFIXES


def split_rust_inline(line: str) -> str | None:
    if "://" in line:
        return None
    index = line.rfind("//")
    if index <= 0:
        return None
    return line[index:]


def comment_texts(path: Path, line: str) -> list[str]:
    if path.suffix in DOC_SUFFIXES:
        return [line]
    stripped = line.lstrip()
    if path.suffix == ".rs":
        if stripped.startswith(("//", "/*", "*")):
            return [line]
        inline = split_rust_inline(line)
        return [inline] if inline else []
    if path.suffix in {".py", ".ps1", ".toml"}:
        return [line] if stripped.startswith("#") else []
    if path.suffix == ".iss":
        return [line] if stripped.startswith(";") else []
    return []


def should_flag(rule: str, token: str, text: str, is_code: bool) -> bool:
    if token in ALLOW_TOKENS:
        return False
    hints = ALLOW_HINTS.get(token)
    if hints and any(hint in text for hint in hints):
        return False
    if rule == "batch":
        # 文档中的中文行直接判红；代码注释只在过程语境中判红，避免 GUID/版本误报。
        if any(context in text for context in BATCH_CONTEXT):
            return True
        return not is_code and bool(re.search(r"[一-鿿]", text))
    if rule == "doc-ref-in-code":
        # 依赖策略文件不是文档，允许出现在注释中。
        return "dependencies.toml" not in text and "file-budgets.txt" not in text
    return True


def scan_text(path: Path, line_no: int, text: str, is_code: bool) -> list[tuple[str, str, str]]:
    sanitized = GUID_RE.sub("", URL_RE.sub("", text))
    findings = []
    for rule, pattern in RULES:
        if rule == "doc-ref-in-code" and not is_code:
            continue
        for match in pattern.finditer(sanitized):
            token = match.group(0)
            if should_flag(rule, token, sanitized, is_code):
                findings.append((rule, token, sanitized.strip()))
    return findings


def scan_full() -> list[tuple[str, int, str, str]]:
    findings = []
    for path in tracked_files():
        if not in_scope(path) or not path.is_file():
            continue
        relative = path.relative_to(ROOT).as_posix()
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            continue
        is_code = path.suffix in CODE_SUFFIXES
        for line_no, line in enumerate(lines, 1):
            for text in comment_texts(path, line):
                for rule, token, excerpt in scan_text(path, line_no, text, is_code):
                    findings.append((relative, line_no, f"{rule}:{token}", excerpt))
    return findings


def staged_added_lines() -> list[tuple[str, int, str]]:
    result = subprocess.run(
        ["git", "diff", "--cached", "--unified=0", "--no-color", "--diff-filter=ACMR"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=True,
    )
    current = None
    line_no = 0
    added = []
    for raw in result.stdout.splitlines():
        if raw.startswith("+++ b/"):
            current = raw[6:]
        elif raw.startswith("@@"):
            match = re.search(r"\+(\d+)", raw)
            line_no = int(match.group(1)) if match else 0
        elif raw.startswith("+") and not raw.startswith("+++"):
            if current is not None:
                added.append((current, line_no, raw[1:]))
            line_no += 1
    return added


def scan_staged() -> list[tuple[str, int, str, str]]:
    findings = []
    for relative, line_no, line in staged_added_lines():
        path = ROOT / relative
        if not in_scope(path):
            continue
        is_code = path.suffix in CODE_SUFFIXES
        for text in comment_texts(path, line):
            for rule, token, excerpt in scan_text(path, line_no, text, is_code):
                findings.append((relative, line_no, f"{rule}:{token}", excerpt))
    return findings


def report(findings: list[tuple[str, int, str, str]]) -> None:
    for relative, line_no, rule, excerpt in findings:
        print(f"ERROR: {relative}:{line_no}:{rule}: {excerpt}")
    print(f"Doc policy: {len(findings)} finding(s)")


def main() -> int:
    parser = argparse.ArgumentParser(description="Document and comment policy gate")
    parser.add_argument("mode", nargs="?", default="full", choices=("full", "staged", "report"))
    options = parser.parse_args()
    try:
        findings = scan_staged() if options.mode == "staged" else scan_full()
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Doc policy check could not complete: {error}", file=sys.stderr)
        return 2
    report(findings)
    if options.mode == "report":
        return 0
    return int(bool(findings))


if __name__ == "__main__":
    raise SystemExit(main())
