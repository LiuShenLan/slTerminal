import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from scripts import check_traceability as trace


MATRIX_HEADER = "| 验收场景 | 目标测试例名 | 状态 |\n|---|---|---|\n"


class TraceabilityTests(unittest.TestCase):
    def test_default_matrix_lives_under_maintained_test_docs(self):
        self.assertEqual(
            trace.MATRIX,
            trace.ROOT / ".claude" / "docs" / "testing" / "traceability.md",
        )

    def check(self, matrix: str, sources: dict[str, str]) -> list[str]:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            matrix_path = root / "traceability.md"
            matrix_path.write_text(matrix, encoding="utf-8")
            source_root = root / "src"
            source_root.mkdir()
            for name, content in sources.items():
                (source_root / name).write_text(content, encoding="utf-8")
            with patch.multiple(
                trace,
                ROOT=root,
                MATRIX=matrix_path,
            ):
                return trace.collect_findings()

    def test_existing_unique_test_anchor_passes(self):
        findings = self.check(
            MATRIX_HEADER + "| 场景 | `gate_anchor_test` | 已自动化 |\n",
            {"a.rs": "#[test]\nfn gate_anchor_test() {}\n"},
        )
        self.assertEqual(findings, [])

    def test_missing_anchor_is_reported(self):
        findings = self.check(
            MATRIX_HEADER + "| 场景 | `missing_anchor_test` | 已自动化 |\n",
            {"a.rs": "#[test]\nfn other_test() {}\n"},
        )
        self.assertTrue(any("missing_anchor_test" in item for item in findings))

    def test_automated_row_needs_an_anchor(self):
        findings = self.check(MATRIX_HEADER + "| 场景 | — | 已自动化 |\n", {})
        self.assertTrue(any("没有目标测试例名" in item for item in findings))

    def test_shared_anchor_is_reported(self):
        findings = self.check(
            MATRIX_HEADER
            + "| 场景一 | `shared_anchor_test` | 已自动化 |\n"
            + "| 场景二 | `shared_anchor_test` | 已自动化 |\n",
            {"a.rs": "#[test]\nfn shared_anchor_test() {}\n"},
        )
        self.assertTrue(any("同时用于追溯矩阵行" in item for item in findings))

    def test_planned_row_may_name_a_future_test(self):
        findings = self.check(
            MATRIX_HEADER + "| 场景 | `planned_anchor_test` | 待实现 |\n",
            {},
        )
        self.assertEqual(findings, [])

    def test_duplicate_test_names_are_reported(self):
        findings = self.check(
            MATRIX_HEADER + "| 场景 | `duplicated_name_test` | 已自动化 |\n",
            {
                "a.rs": "#[test]\nfn duplicated_name_test() {}\n",
                "b.rs": "#[gpui::test]\nfn duplicated_name_test(cx: &mut Ctx) {}\n",
            },
        )
        self.assertTrue(any("在多个位置定义" in item for item in findings))

    def test_non_test_function_with_matching_name_does_not_count(self):
        findings = self.check(
            MATRIX_HEADER + "| 场景 | `helper_named_test` | 已自动化 |\n",
            {"a.rs": "fn helper_named_test() {}\n"},
        )
        self.assertTrue(any("没有测试函数" in item for item in findings))


if __name__ == "__main__":
    unittest.main()
