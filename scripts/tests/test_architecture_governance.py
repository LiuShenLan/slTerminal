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

    def test_ci_runs_core_tests_in_release_mode(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        dependencies = tomllib.loads(
            (ROOT / "architecture/dependencies.toml").read_text(encoding="utf-8")
        )
        excluded = []
        for crate in manifest["workspace"]["members"]:
            if dependencies["crates"][crate]["layer"] == "core":
                continue
            package = tomllib.loads(
                (ROOT / crate / "Cargo.toml").read_text(encoding="utf-8")
            )
            excluded.append(package["package"]["name"])
        command = "cargo test --release --workspace"
        command += "".join(f" --exclude {package}" for package in excluded)
        command += " -- --test-threads=1"
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn(
            f"run: {command}",
            workflow,
            "CI 必须按架构层执行 core release 串行测试",
        )

    def test_license_compliance_artifacts_exist(self):
        license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", license_text)
        self.assertTrue((ROOT / "THIRD-PARTY-NOTICES").is_file())
        licenses = [path for path in (ROOT / "licenses").iterdir() if path.is_file()]
        self.assertTrue(licenses, "licenses/ 目录必须含第三方许可证文本")


if __name__ == "__main__":
    unittest.main()
