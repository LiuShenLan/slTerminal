from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts import check_brand_regress


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check_brand_regress.py"
FIXTURES = ROOT / "scripts" / "tests" / "fixtures" / "brand_regress"


def run_git(repository: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repository, check=True, stdout=subprocess.DEVNULL)


def run_checker(repository: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), *args], cwd=repository,
        text=True, encoding="utf-8", capture_output=True,
        env={**os.environ, "PYTHONUTF8": "1"},
    )


def init_repository(repository: Path) -> None:
    run_git(repository, "init", "-q")
    run_git(repository, "config", "user.name", "CI test")
    run_git(repository, "config", "user.email", "ci@example.invalid")


def head_revision(repository: Path) -> str:
    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repository, text=True
    ).strip()


class BrandRegressTests(unittest.TestCase):
    def test_names_do_not_match_inside_unrelated_words(self) -> None:
        self.assertFalse(check_brand_regress.prohibited("slpebrel; nebular; pebrel2; nebula_"))
        self.assertTrue(check_brand_regress.prohibited("fork 自 Pebrel 重构"))
        self.assertTrue(check_brand_regress.prohibited("branch nebula-v1.16.1-owned-base"))

    def test_ordinary_upstream_comparison_is_rejected(self) -> None:
        text = (FIXTURES / "ordinary-comparison.md").read_text(encoding="utf-8")
        self.assertTrue(check_brand_regress.prohibited_source_line("comparison.md", text))

    def test_inline_legal_attribution_is_allowed_but_comparison_is_not(self) -> None:
        text = (FIXTURES / "legal-notice.txt").read_text(encoding="utf-8")
        self.assertFalse(check_brand_regress.prohibited_source_line("source.rs", text))
        self.assertTrue(
            check_brand_regress.prohibited_source_line("README.md", "Compare with pebrel.")
        )
        self.assertTrue(
            check_brand_regress.prohibited_source_line(
                "source.rs", "Copyright (c) pebrel contributors; compare with nebula."
            )
        )

    def test_real_cargo_git_dependency_is_allowed_but_readme_reference_is_not(self) -> None:
        dependency = 'gpui = { git = "https://github.com/example/pebrel", rev = "deadbeef" }'
        self.assertFalse(check_brand_regress.prohibited_source_line("Cargo.toml", dependency))
        self.assertTrue(check_brand_regress.prohibited_source_line("README.md", dependency))

    def test_staged_added_line_hit_and_safe_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            init_repository(repository)
            path = repository / "source.rs"
            path.write_text("safe\n", encoding="utf-8")
            run_git(repository, "add", "source.rs")
            self.assertEqual(run_checker(repository, "staged").returncode, 0)
            path.write_text("safe\n// backport from pebrel\n", encoding="utf-8")
            run_git(repository, "add", "source.rs")
            result = run_checker(repository, "staged")
            self.assertEqual(result.returncode, 1)
            self.assertIn("pebrel", result.stderr)
            self.assertIn("source.rs", result.stderr)

    def test_staged_exempt_paths_and_temp_doc_prefixes_are_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            init_repository(repository)
            (repository / "Cargo.toml").write_text(
                "# anchor nebula-v1.16.1-owned-base\n", encoding="utf-8"
            )
            note = repository / "docs" / "pebrel-refactor" / "note.md"
            note.parent.mkdir(parents=True)
            note.write_text("pebrel baseline review\n", encoding="utf-8")
            design = repository / "docs" / "pebrel-design" / "draft.md"
            design.parent.mkdir(parents=True)
            design.write_text("nebula design\n", encoding="utf-8")
            ordinary = repository / "docs" / "guide.md"
            ordinary.write_text("pebrel mention\n", encoding="utf-8")
            run_git(repository, "add", "-A")
            result = run_checker(repository, "staged")
            self.assertEqual(result.returncode, 1)
            self.assertIn("docs/guide.md", result.stderr)
            self.assertNotIn("Cargo.toml", result.stderr)
            self.assertNotIn("pebrel-refactor", result.stderr)
            self.assertNotIn("pebrel-design", result.stderr)

    def test_message_mode_hit_and_safe_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            run_git(repository, "init", "-q")
            message = repository / "message.txt"
            message.write_text("slTerminal: terminal baseline\n", encoding="utf-8")
            self.assertEqual(run_checker(repository, "message", str(message)).returncode, 0)
            message.write_text("merge pebrel upstream\n", encoding="utf-8")
            result = run_checker(repository, "message", str(message))
            self.assertEqual(result.returncode, 1)
            self.assertIn("commit message", result.stderr)

    def test_invalid_utf8_text_and_message_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            run_git(repository, "init", "-q")
            path = repository / "source.rs"
            path.write_bytes(b"safe \xff\n")
            run_git(repository, "add", "source.rs")
            staged = run_checker(repository, "staged")
            message = run_checker(repository, "message", str(path))
            for result in (staged, message):
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("utf-8", result.stderr.lower())

    def test_range_merge_combined_diff_scans_resolution_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            init_repository(repository)
            (repository / "shared.md").write_text("base\n", encoding="utf-8")
            run_git(repository, "add", "shared.md")
            run_git(repository, "commit", "-qm", "baseline")
            run_git(repository, "branch", "main")

            run_git(repository, "switch", "-c", "feature")
            (repository / "shared.md").write_text("feature\n", encoding="utf-8")
            run_git(repository, "add", "shared.md")
            run_git(repository, "commit", "-qm", "feature change")
            feature_parent = head_revision(repository)
            run_git(repository, "switch", "main")
            (repository / "shared.md").write_text("main Pebrel\n", encoding="utf-8")
            run_git(repository, "add", "shared.md")
            run_git(repository, "commit", "-qm", "main change")
            main_head = head_revision(repository)
            run_git(repository, "switch", "feature")
            merge = subprocess.run(
                ["git", "merge", "--no-ff", "main", "-m", "merge main"],
                cwd=repository, text=True, capture_output=True,
            )
            self.assertNotEqual(merge.returncode, 0, merge.stdout + merge.stderr)
            (repository / "shared.md").write_text("resolved safe\n", encoding="utf-8")
            run_git(repository, "add", "shared.md")
            run_git(repository, "commit", "-qm", "resolve safe merge")
            safe_head = head_revision(repository)
            safe_result = run_checker(
                repository, "range", "--base", main_head, "--head", safe_head
            )
            self.assertEqual(safe_result.returncode, 0, safe_result.stderr)

            run_git(repository, "switch", "-c", "feature-bad", feature_parent)
            merge_bad = subprocess.run(
                ["git", "merge", "--no-ff", "main", "-m", "merge main with bad resolution"],
                cwd=repository, text=True, capture_output=True,
            )
            self.assertNotEqual(merge_bad.returncode, 0, merge_bad.stdout + merge_bad.stderr)
            (repository / "shared.md").write_text("resolved Pebrel\n", encoding="utf-8")
            run_git(repository, "add", "shared.md")
            run_git(repository, "commit", "-qm", "resolve bad merge")
            bad_head = head_revision(repository)
            bad_result = run_checker(
                repository, "range", "--base", main_head, "--head", bad_head
            )
            self.assertNotEqual(bad_result.returncode, 0)
            self.assertIn("shared.md", bad_result.stderr)


if __name__ == "__main__":
    unittest.main()
