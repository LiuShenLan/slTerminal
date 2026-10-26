from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import check_licenses


class LicenseNoticeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def package(self, name, source="registry+https://example.invalid", license="MIT"):
        root = self.root / name
        root.mkdir(parents=True)
        return {
            "id": name,
            "name": name,
            "version": "1.0.0",
            "source": source,
            "license": license,
            "manifest_path": str(root / "Cargo.toml"),
        }

    def metadata(self, packages, dependency_kinds=None):
        nodes = [
            {
                "id": "slterm",
                "deps": [
                    {
                        "pkg": package["id"],
                        "dep_kinds": [
                            {"kind": (dependency_kinds or {}).get(package["name"])}
                        ],
                    }
                    for package in packages
                ],
            },
            {"id": "slterm-hook", "deps": []},
        ]
        nodes.extend({"id": package["id"], "deps": []} for package in packages)
        return {
            "packages": packages
            + [
                self.package("slterm", source=None),
                self.package("slterm-hook", source=None),
            ],
            "workspace_members": ["slterm", "slterm-hook"],
            "resolve": {"nodes": nodes},
        }

    def test_production_graph_includes_build_and_excludes_dev(self):
        normal = self.package("normal")
        build = self.package("build")
        development = self.package("development")
        metadata = self.metadata(
            [normal, build, development],
            {"build": "build", "development": "dev"},
        )
        self.assertEqual(
            [package["name"] for package in check_licenses.production_packages(metadata)],
            ["build", "normal"],
        )

    def test_render_includes_license_and_notice_text(self):
        package = self.package("demo")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("MIT demo", encoding="utf-8")
        (root / "NOTICE").write_text("Copyright Demo", encoding="utf-8")
        rendered = check_licenses.render_notices(self.metadata([package]))
        self.assertIn("Package: demo 1.0.0", rendered)
        self.assertIn("MIT demo", rendered)
        self.assertIn("Copyright Demo", rendered)

    def test_render_accepts_british_licence_spelling(self):
        package = self.package("british-licence")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("Primary license", encoding="utf-8")
        (root / "LICENCE").write_text("British licence", encoding="utf-8")
        rendered = check_licenses.render_notices(self.metadata([package]))
        self.assertIn("British licence", rendered)

    def test_license_file_match_excludes_tool_configuration(self):
        package = self.package("license-config")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("Actual license", encoding="utf-8")
        (root / "licenserc.toml").write_text("config-only-marker", encoding="utf-8")
        rendered = check_licenses.render_notices(self.metadata([package]))
        self.assertIn("Actual license", rendered)
        self.assertNotIn("config-only-marker", rendered)

    def test_notice_label_survives_cross_drive_snapshot(self):
        package = self.package("cross-drive")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("Cross-drive license", encoding="utf-8")
        with mock.patch.object(check_licenses.os.path, "relpath", side_effect=ValueError):
            try:
                rendered = check_licenses.render_notices(self.metadata([package]))
            except ValueError as error:
                self.fail(f"跨盘许可路径未降级为文件名: {error}")
        self.assertIn("File: LICENSE\n", rendered)
        self.assertIn("Cross-drive license", rendered)

    def test_missing_license_fails_closed(self):
        package = self.package("missing")
        with self.assertRaisesRegex(check_licenses.LicenseAuditError, "missing"):
            check_licenses.render_notices(self.metadata([package]))

    def test_registry_package_uses_exact_version_license_snapshot(self):
        package = self.package("snapshot-demo")
        snapshot = self.root / "licenses" / "crates" / "snapshot-demo-1.0.0"
        snapshot.mkdir(parents=True)
        (snapshot / "LICENSE").write_text("Snapshot MIT", encoding="utf-8")
        metadata = self.metadata([package])
        with mock.patch.object(check_licenses, "ROOT", self.root):
            try:
                rendered = check_licenses.render_notices(metadata)
            except check_licenses.LicenseAuditError as error:
                self.fail(f"精确版本许可快照未被使用: {error}")
            self.assertIn("Snapshot MIT", rendered)
            package["version"] = "2.0.0"
            with self.assertRaisesRegex(check_licenses.LicenseAuditError, "snapshot-demo"):
                check_licenses.render_notices(metadata)

    def test_git_package_uses_workspace_license_and_filters_unrelated_files(self):
        package = self.package("git-demo", source="git+https://example.invalid/repo")
        checkout = self.root / "checkout"
        nested = checkout / "crates" / "git-demo"
        nested.mkdir(parents=True)
        (checkout / ".git").mkdir()
        (checkout / "LICENSE-APACHE").write_text("Apache text", encoding="utf-8")
        (checkout / "LICENSE-GPL").write_text("GPL text", encoding="utf-8")
        (checkout / "NOTICE").write_text("Upstream notice", encoding="utf-8")
        package["manifest_path"] = str(nested / "Cargo.toml")
        package["license"] = "Apache-2.0"
        rendered = check_licenses.render_notices(self.metadata([package]))
        self.assertIn("Apache text", rendered)
        self.assertIn("Upstream notice", rendered)
        self.assertNotIn("GPL text", rendered)

    def test_vendored_path_package_is_included_and_requires_license(self):
        package = self.package("dav1d-sys", source=None)
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("Vendored binding MIT", encoding="utf-8")
        metadata = self.metadata([package])
        rendered = check_licenses.render_notices(metadata)
        self.assertIn("Package: dav1d-sys 1.0.0", rendered)
        self.assertIn("Source: path", rendered)
        self.assertIn("Vendored binding MIT", rendered)
        (root / "LICENSE").unlink()
        with self.assertRaisesRegex(check_licenses.LicenseAuditError, "dav1d-sys"):
            check_licenses.render_notices(metadata)

    def test_native_notice_is_included_and_required_when_dependency_is_present(self):
        package = self.package("dav1d-sys")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("Binding MIT", encoding="utf-8")
        native = self.root / "DAV1D-LICENSE"
        native.write_text("Native BSD", encoding="utf-8")
        metadata = self.metadata([package])
        rendered = check_licenses.render_notices(
            metadata, extra_documents=(("dav1d-sys", native),)
        )
        self.assertIn("Binding MIT", rendered)
        self.assertIn("Native BSD", rendered)
        native.unlink()
        with self.assertRaisesRegex(check_licenses.LicenseAuditError, "dav1d"):
            check_licenses.render_notices(
                metadata, extra_documents=(("dav1d-sys", native),)
            )

    def test_write_and_check_use_the_same_deterministic_content(self):
        package = self.package("demo")
        root = Path(package["manifest_path"]).parent
        (root / "LICENSE").write_text("MIT demo\n", encoding="utf-8")
        metadata = self.metadata([package])
        destination = self.root / "THIRD-PARTY-NOTICES"
        check_licenses.write_notice(metadata, destination)
        self.assertTrue(destination.is_file(), "生成器必须写出许可文件")
        self.assertEqual(destination.read_text(encoding="utf-8"), check_licenses.render_notices(metadata))
        check_licenses.check_notice(metadata, destination)
        destination.write_text("stale", encoding="utf-8")
        with self.assertRaisesRegex(check_licenses.LicenseAuditError, "stale"):
            check_licenses.check_notice(metadata, destination)


if __name__ == "__main__":
    unittest.main()
