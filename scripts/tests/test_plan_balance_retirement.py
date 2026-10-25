from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
APP_SOURCE = ROOT / "slterm_app/src"
PLAN_BALANCE_SOURCE = APP_SOURCE / "plan_balance"


def plan_balance_sources():
    files = sorted(PLAN_BALANCE_SOURCE.rglob("*.rs"))
    if len(files) < 3:
        raise AssertionError("套餐解析源码扫描不能为空或缺少模块")
    for path in files:
        yield path.relative_to(APP_SOURCE).as_posix(), path.read_text(encoding="utf-8")


class PlanBalanceRetirementTests(unittest.TestCase):
    def test_cli_credentials_and_dynamic_url_discovery_are_removed(self):
        retired = (
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            ".claude",
            "ClaudeUserSettingsSource",
            "SettingsPathGuard",
            "resolve_settings_path",
            "PlanSource",
            "SOURCES",
            "normalize_base_url",
            "find_query_by_url",
            "base_urls",
            "home_dir",
            "std::env::var",
            "var_os",
        )
        for relative, text in plan_balance_sources():
            for marker in retired:
                self.assertNotIn(
                    marker,
                    text,
                    f"套餐不得读取 CLI 认证材料或按 CLI URL 发现来源: {relative} / {marker}",
                )

    def test_legacy_query_and_polling_execution_is_removed(self):
        retired = (
            "PlanQuery",
            "QUERIES",
            "http_agent",
            "query_err",
            "poll_once_with",
            "poll_once_production",
            "poll_once_executor",
            "start_background_poll",
            "spawn_background_poll",
            "refresh_plan_balance",
            "merge_slot",
            "SNAPSHOT",
            "UPDATE_SUBSCRIBERS",
            "PlanBalanceUpdates",
        )
        for relative, text in plan_balance_sources():
            for marker in retired:
                self.assertNotIn(
                    marker,
                    text,
                    f"旧套餐查询或轮询执行面不得回流: {relative} / {marker}",
                )

        wiring = {
            "app_shell/mod.rs": ("start_background_poll",),
            "background_tasks.rs": ("PLAN_BALANCE", "poll_once_executor"),
        }
        for relative, markers in wiring.items():
            text = (APP_SOURCE / relative).read_text(encoding="utf-8")
            for marker in markers:
                self.assertNotIn(
                    marker,
                    text,
                    f"旧套餐后台任务不得保留注册或启动接线: {relative} / {marker}",
                )


if __name__ == "__main__":
    unittest.main()
