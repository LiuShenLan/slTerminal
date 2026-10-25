from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
APP_SOURCE = ROOT / "slterm_app/src"
TEST_MODULES = {
    "ai_sessions.rs": "\n#[cfg(test)]\nmod session_scan_tests",
    "encrypted_backup.rs": "\n#[cfg(test)]\nmod encrypted_backup_tests",
}


def production_sources():
    for path in sorted(APP_SOURCE.rglob("*.rs")):
        relative = path.relative_to(APP_SOURCE).as_posix()
        text = path.read_text(encoding="utf-8")
        marker = TEST_MODULES.get(relative)
        if marker is not None:
            text = text.split(marker, 1)[0]
        yield relative, text


class AiSessionRegistryRetirementTests(unittest.TestCase):
    def test_retired_registry_file_has_no_production_reference(self):
        for relative, text in production_sources():
            self.assertNotIn(
                "ai_sessions.json",
                text,
                f"旧 AI 会话注册表不得保留生产读写入口: {relative}",
            )

    def test_registry_implementation_and_session_start_writer_are_removed(self):
        retired = {
            "ai_sessions.rs": (
                "SessionRegistry",
                "RegistrySession",
                "REGISTRY_VERSION",
                "REGISTRY_LIMIT",
                "record_hook_session",
                "valid_registry_id",
                "load_registry",
                "registry_sessions_at",
                "merge_registry",
            ),
            "agent_host.rs": (
                "SessionStartRecorder",
                "SessionRegistryWrite",
                "spawn_session_registry_write",
                "record_session_start",
                "enable_session_registry",
            ),
            "app_shell/workspace/mod.rs": ("enable_session_registry",),
        }
        for relative, markers in retired.items():
            text = (APP_SOURCE / relative).read_text(encoding="utf-8")
            for marker in markers:
                self.assertNotIn(
                    marker,
                    text,
                    f"旧注册表实现或 SessionStart 写盘链不得回流: {relative} / {marker}",
                )


if __name__ == "__main__":
    unittest.main()
