import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from scripts import check_doc_policy as policy


class RuleTests(unittest.TestCase):
    def rules(self, text: str, is_code: bool = False) -> set[str]:
        return {rule for rule, _, _ in policy.scan_text(Path("doc.md"), 1, text, is_code)}

    def test_phase_rule_and_layer_ids_are_rejected(self):
        self.assertIn("phase", self.rules("阶段 M8.2 的落点"))
        self.assertIn("rule", self.rules("SEC-12 审查门"))
        self.assertIn("layer", self.rules("L1 纯函数用例"))

    def test_batch_ids_require_process_context_in_code(self):
        self.assertIn("batch", self.rules("批次 B75 收尾", is_code=True))
        self.assertNotIn("batch", self.rules("B75 without context", is_code=True))

    def test_batch_ids_fire_in_chinese_documents(self):
        self.assertIn("batch", self.rules("版本 v1.2.3 与 B12 维生素", is_code=False))
        self.assertNotIn("batch", self.rules("A4 paper", is_code=False))

    def test_line_references_are_rejected_but_urls_are_not(self):
        self.assertIn("line-ref", self.rules("见 src/lib.rs:42"))
        self.assertNotIn("line-ref", self.rules("见 https://example.com/src/lib.rs:42"))

    def test_dead_document_paths_are_rejected(self):
        self.assertIn("dead-doc", self.rules("见 .claude/adr.md"))
        self.assertIn("dead-doc", self.rules("见 architecture/notes/x.md"))

    def test_allowlisted_technical_tokens_do_not_fire(self):
        self.assertFalse(self.rules("C0 控制字符 与 R100 git 状态"))
        self.assertFalse(self.rules("L2 范数"))
        self.assertFalse(self.rules("UTF-8 / SHA-256 / 0xc0000139"))
        self.assertFalse(self.rules("{D9D92995-C831-4610-87BC-4B82B43B3E71}"))

    def test_doc_references_only_fire_in_code_comments(self):
        self.assertNotIn("doc-ref-in-code", self.rules("见 CLAUDE.md", is_code=False))
        self.assertIn("doc-ref-in-code", self.rules("见 CLAUDE.md", is_code=True))
        self.assertNotIn(
            "doc-ref-in-code",
            self.rules("见 architecture/dependencies.toml", is_code=True),
        )


class CommentExtractionTests(unittest.TestCase):
    def test_rust_whole_line_and_inline_comments(self):
        path = Path("src/lib.rs")
        self.assertEqual(policy.comment_texts(path, "// M8.2 落点"), ["// M8.2 落点"])
        self.assertEqual(
            policy.comment_texts(path, "let value = 1; // M8.2 落点"),
            ["// M8.2 落点"],
        )
        self.assertEqual(policy.comment_texts(path, "let url = \"https://x\";"), [])

    def test_python_powershell_toml_and_iss_comments(self):
        self.assertEqual(policy.comment_texts(Path("a.py"), "# M8.2"), ["# M8.2"])
        self.assertEqual(policy.comment_texts(Path("a.ps1"), "# M8.2"), ["# M8.2"])
        self.assertEqual(policy.comment_texts(Path("a.toml"), "# M8.2"), ["# M8.2"])
        self.assertEqual(policy.comment_texts(Path("a.iss"), "; M8.2"), ["; M8.2"])
        self.assertEqual(policy.comment_texts(Path("a.py"), "x = 1  # M8.2"), [])

    def test_markdown_is_scanned_in_full(self):
        self.assertEqual(policy.comment_texts(Path("a.md"), "正文 M8.2"), ["正文 M8.2"])


class StagedDiffTests(unittest.TestCase):
    @patch("scripts.check_doc_policy.subprocess.run")
    def test_staged_added_lines_track_file_and_line_numbers(self, run):
        run.return_value.stdout = (
            "+++ b/doc.md\n"
            "@@ -1,0 +2,2 @@\n"
            "+阶段 M8.2\n"
            "+安全\n"
        )
        self.assertEqual(
            policy.staged_added_lines(),
            [("doc.md", 2, "阶段 M8.2"), ("doc.md", 3, "安全")],
        )


class ScopeTests(unittest.TestCase):
    def test_excluded_paths_are_out_of_scope(self):
        self.assertFalse(policy.in_scope(ROOT / "assets/corpus/x.md"))
        self.assertFalse(policy.in_scope(ROOT / "slterm_terminal/vendor/conpty/README.md"))
        self.assertFalse(policy.in_scope(ROOT / "LICENSE"))
        self.assertFalse(policy.in_scope(ROOT / "scripts/check_doc_policy.py"))

    def test_project_docs_and_source_are_in_scope(self):
        self.assertTrue(policy.in_scope(ROOT / "CONTEXT.md"))
        self.assertTrue(policy.in_scope(ROOT / "slterm_app/src/lib.rs"))
        self.assertTrue(policy.in_scope(ROOT / "scripts/release.ps1"))


if __name__ == "__main__":
    unittest.main()
