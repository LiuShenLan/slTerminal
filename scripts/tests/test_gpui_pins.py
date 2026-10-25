from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
ZED_URL = "https://github.com/Kuddev/zed"
COMPONENT_URL = "https://github.com/Kuddev/gpui-component"
ZED_REV = "d7b616062879de95ecd60dffa05ca6b1d835647a"
COMPONENT_REV = "558ae28d01f871c6b1536f5070704239a9f3a490"
EXPECTED_DEPENDENCIES = {
    "gpui": (ZED_URL, ZED_REV, "=0.2.2"),
    "gpui_platform": (ZED_URL, ZED_REV, "=0.1.0"),
    "gpui-component": (COMPONENT_URL, COMPONENT_REV, "=0.5.2"),
    "gpui-component-assets": (COMPONENT_URL, COMPONENT_REV, "=0.5.1"),
}
AUDIT_ANCHORS = (
    ZED_REV,
    COMPONENT_REV,
    "1b913031d01f5b3df8fcab3e8c2ac3568e7f5524",
    "8a1a62727bcb2ead7352e32e7e0d2e919a943535",
    "7c662384cd77c1d1e1566206920e3a420ea6767b",
    "2b0d6d1948e374dc7a1263a27661df1409ae24c1",
    "fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4",
    "9a2f663cad2f375e9433a8abdb512009f00d62cc",
    "1cacf8b08a87c06f27ecc99748302eb812b5e8c0",
    "c20dbef8ed0d4c0fbebbf2e8f02d39e4fa0ac0a7",
    "fa3763c681dba5b3ea028395148488e1281d478d",
    "f46ecbb06de5907af9700f3f6e2b5b0845459188",
)


class GpuiPinTests(unittest.TestCase):
    def test_workspace_dependencies_use_audited_revisions(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        dependencies = manifest["workspace"]["dependencies"]
        for name, (url, rev, version) in EXPECTED_DEPENDENCIES.items():
            with self.subTest(dependency=name):
                dependency = dependencies[name]
                self.assertEqual(dependency["git"], url)
                self.assertEqual(dependency["rev"], rev)
                self.assertEqual(dependency["version"], version)

    def test_lockfile_has_one_audited_source_for_each_gpui_package(self):
        lockfile = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
        expected_sources = {
            name: f"git+{url}?rev={rev}#{rev}"
            for name, (url, rev, _) in EXPECTED_DEPENDENCIES.items()
        }
        found = {name: [] for name in EXPECTED_DEPENDENCIES}
        for package in lockfile["package"]:
            name = package["name"]
            source = package.get("source", "")
            for url, rev in ((ZED_URL, ZED_REV), (COMPONENT_URL, COMPONENT_REV)):
                if source.startswith(f"git+{url}?"):
                    self.assertEqual(
                        source,
                        f"git+{url}?rev={rev}#{rev}",
                        f"GPUI 锁文件不得混入未审计 rev: {name}",
                    )
            if name in expected_sources:
                found[name].append(source)

        for name, sources in found.items():
            with self.subTest(package=name):
                self.assertEqual(sources, [expected_sources[name]])

    def test_provenance_records_final_revisions_and_patch_audit(self):
        provenance = (ROOT / ".claude/docs/architecture/provenance.md").read_text(
            encoding="utf-8"
        )
        for anchor in AUDIT_ANCHORS:
            with self.subTest(anchor=anchor):
                self.assertIn(anchor, provenance)


if __name__ == "__main__":
    unittest.main()
