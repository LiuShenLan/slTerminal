from pathlib import Path
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
NOTES_ROOT = ROOT / "architecture" / "notes"
NOTE_REQUIRED_SECTIONS = (
    "Status", "Context", "Evidence", "Decision", "Rejected alternatives",
    "Consequences", "Validation", "Supersedes", "Revisit when",
)
MAX_NOTE_LINES = 200


def decision_note_errors(text):
    errors = []
    if not re.search(r"(?m)^# [^#\n]+$", text):
        errors.append("missing one level-one title")
    headings = {
        match.group(1).strip().casefold()
        for match in re.finditer(r"(?m)^##\s+(.+?)\s*$", text)
    }
    for section in NOTE_REQUIRED_SECTIONS:
        if section.casefold() not in headings:
            errors.append(f"missing section: {section}")
    line_count = len(text.splitlines())
    if line_count > MAX_NOTE_LINES:
        errors.append(f"{line_count} lines exceeds {MAX_NOTE_LINES}")
    return errors


class GovernanceTests(unittest.TestCase):
    def test_decision_note_contract_accepts_and_rejects_known_examples(self):
        valid = "# Decision\n\n" + "\n\n".join(
            f"## {section}\n\nRecorded." for section in NOTE_REQUIRED_SECTIONS
        )
        self.assertEqual(decision_note_errors(valid), [])

        missing_alternative = valid.replace("## Rejected alternatives", "## Options")
        self.assertIn(
            "missing section: Rejected alternatives",
            decision_note_errors(missing_alternative),
        )

        oversized = valid + "\n" + "\n".join("detail" for _ in range(MAX_NOTE_LINES))
        self.assertTrue(
            any("exceeds" in error for error in decision_note_errors(oversized)),
            "an oversized note must fail",
        )

    def test_decision_notes_are_scoped_and_complete(self):
        notes = [
            path for path in NOTES_ROOT.rglob("*.md")
            if path.name != "AGENTS.md"
        ]
        # 允许零 note 起步;一旦入库即受属主路径、命名与九段结构约束。
        self.assertFalse(
            any(path.name.casefold() == "index.md" for path in NOTES_ROOT.rglob("*.md")),
            "decision notes must not use a global index",
        )
        for path in notes:
            relative = path.relative_to(NOTES_ROOT)
            with self.subTest(note=relative.as_posix()):
                self.assertGreaterEqual(len(relative.parts), 2, "notes must follow an owner path")
                self.assertRegex(
                    path.name,
                    r"^\d{4}-\d{2}-\d{2}-[a-z0-9][a-z0-9-]*\.md$",
                )
                self.assertEqual(
                    decision_note_errors(path.read_text(encoding="utf-8")),
                    [],
                )

    def test_toolchain_pin_matches_workspace_rust_version(self):
        toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(
            toolchain["toolchain"]["channel"],
            manifest["workspace"]["package"]["rust-version"],
            "rust-toolchain channel 与 workspace rust-version 必须钉在同一版本",
        )

    def test_license_compliance_artifacts_exist(self):
        license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", license_text)
        self.assertTrue((ROOT / "THIRD-PARTY-NOTICES").is_file())
        licenses = [path for path in (ROOT / "licenses").iterdir() if path.is_file()]
        self.assertTrue(licenses, "licenses/ 目录必须含第三方许可证文本")


if __name__ == "__main__":
    unittest.main()
