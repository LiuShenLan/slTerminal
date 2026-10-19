from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class GovernanceTests(unittest.TestCase):
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
