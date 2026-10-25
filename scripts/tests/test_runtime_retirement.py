from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


class RuntimeRetirementTests(unittest.TestCase):
    def test_legacy_runtime_skill_is_not_packaged(self):
        for relative in (
            "scripts/build-windows-product.ps1",
            "scripts/installer.iss",
        ):
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertNotIn(
                "slterm-runtime",
                text,
                f"旧 Runtime skill 不得进入发布清单: {relative}",
            )
        self.assertFalse((ROOT / "assets/skills/slterm-runtime").exists())

    def test_hook_self_healing_has_no_runtime_skill_chain(self):
        module = ROOT / "slterm_app/src/ai_hook/local/runtime_skills.rs"
        self.assertFalse(module.exists(), "旧 Runtime skill 投放模块必须删除")
        local = (ROOT / "slterm_app/src/ai_hook/local.rs").read_text(encoding="utf-8")
        guard = (ROOT / "slterm_app/src/ai_hook/local/config_guard.rs").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("runtime_skills", local)
        self.assertNotIn("runtime_skills", guard)
        self.assertNotIn("Skill", guard, "自愈失败类别不得保留 Skill 占位")

    def test_legacy_runtime_assets_are_not_packaged(self):
        for relative in (
            "scripts/build-windows-product.ps1",
            "scripts/installer.iss",
        ):
            text = (ROOT / relative).read_text(encoding="utf-8")
            for marker in ("runtime-api-v1", "runtime-control-api"):
                self.assertNotIn(
                    marker,
                    text,
                    f"旧 Runtime 合同不得进入发布清单: {relative} / {marker}",
                )
        for relative in (
            "assets/runtime-api-v1.schema.json",
            "assets/runtime-control-api.md",
        ):
            self.assertFalse((ROOT / relative).exists(), f"旧资产必须删除: {relative}")

    def test_legacy_runtime_implementation_is_removed(self):
        for relative in (
            "slterm_app/src/runtime_api.rs",
            "slterm_app/src/runtime_api",
            "slterm_app/src/runtime_exec.rs",
            "slterm_app/src/runtime_projection.rs",
            "slterm_app/src/app_shell/workspace/runtime_control.rs",
            "slterm_app/src/app_shell/workspace/runtime_projection.rs",
            "slterm_app/src/gpui_terminal/view/runtime_control.rs",
            "slterm_app/src/git_worktree.rs",
            "slterm_app/src/terminal_registry.rs",
            "slterm_app/tests/runtime_conformance.rs",
            "slterm_runtime_protocol/src/methods",
        ):
            self.assertFalse((ROOT / relative).exists(), f"旧实现必须删除: {relative}")
        manifest = (ROOT / "slterm_app/Cargo.toml").read_text(encoding="utf-8")
        self.assertNotIn("legacy-attach", manifest)
        self.assertNotIn("runtime_conformance", manifest)

    def test_completion_keeps_only_its_frozen_pty_context(self):
        metadata = (ROOT / "slterm_app/src/completion/metadata.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("struct PaneExecContext", metadata)
        self.assertNotIn("runtime_exec", metadata)
        for removed_member in ("fallback_cwd", "for_git", "process_instance"):
            self.assertNotIn(removed_member, metadata)

    def test_child_environment_cannot_inherit_legacy_runtime_endpoint(self):
        agent_env = (ROOT / "slterm_app/src/agent_env.rs").read_text(encoding="utf-8")
        normalized = "".join(agent_env.split())
        self.assertIn("SLTERM_RUNTIME_ENDPOINT", agent_env)
        self.assertIn("options.env.remove(LEGACY_RUNTIME_ENDPOINT_ENV);", normalized)
        self.assertIn(
            "options.inherited_env_exclusions.push(excluded.to_os_string());",
            normalized,
        )
        self.assertNotIn("apply_child_endpoint", agent_env)

    def test_legacy_pane_resize_helper_is_not_retained_for_tests(self):
        split = (ROOT / "slterm_split/src/lib.rs").read_text(encoding="utf-8")
        self.assertNotIn(
            "set_leaf_parent_ratio",
            split,
            "旧 pane.resize 辅助面不得仅由测试保留",
        )


if __name__ == "__main__":
    unittest.main()
