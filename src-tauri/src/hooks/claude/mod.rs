//! claude hooks provider（MC-210/213 下沉）
//!
//! provider 内部是 claude 合法领地（D11）：claude 命名与 claude 知识全部保留
//! （HOOK_EVENTS 10 事件、~/.claude/settings.json、matcher 结构、SCRIPT_VERSION 检测、
//! reporter 模板、三层配置路径）。
//!
//! 对外暴露：
//! - `ClaudeHooksProvider`：`CliHooksProvider` trait 实现（注册表条目）
//! - `home_dir()`：统一 home 解析（测试经 `HomeDirGuard` 注入覆盖，L1 隔离纪律）

pub mod config;
pub mod inject;

use std::path::PathBuf;

use crate::error::AppError;
use crate::hooks::provider::CliHooksProvider;
use crate::hooks::{AgentHookInjectionStatus, AgentInjectionStatus};
use serde_json::Value;

// ── 路径辅助（provider impl 内部，home 解析统一走 home_dir()） ──

fn hooks_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".slterminal").join("hooks"))
}

fn hooks_events_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".slterminal").join("hooks-events"))
}

fn claude_settings_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join("settings.json"))
}

fn hook_script_path() -> Option<PathBuf> {
    hooks_dir().map(|d| d.join("slterm-hook-reporter.js"))
}

/// statusline 桥接脚本路径（B15：reinject 必须用桥接脚本而非 reporter——
/// 误传 reporter 会把 statusLine 写成 reporter 包裹，透传末端 stdout 恒空致状态行空白）
fn statusline_script_path() -> Option<PathBuf> {
    hooks_dir().map(|d| d.join(inject::STATUSLINE_SCRIPT_NAME))
}

fn home_dir_err() -> AppError {
    AppError::IoKind {
        kind: "home_dir".into(),
        message: "无法获取用户 home 目录".into(),
    }
}

/// 测试用 home 目录覆盖注入槽（仅测试编译，生产零行为变更——照 settings.rs AppDataDirGuard 先例）
#[cfg(test)]
static HOME_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// 测试用 RAII 守卫：把 home_dir() 指向指定目录，Drop 时恢复原值
/// （防测试 panic 残留覆盖污染后续用例；命令层 cliId 透传测试使用）
#[cfg(test)]
pub(crate) struct HomeDirGuard(Option<PathBuf>);

#[cfg(test)]
impl HomeDirGuard {
    pub(crate) fn set(dir: &std::path::Path) -> Self {
        let mut slot = HOME_DIR_OVERRIDE.lock().unwrap();
        let prev = slot.clone();
        *slot = Some(dir.to_path_buf());
        HomeDirGuard(prev)
    }
}

#[cfg(test)]
impl Drop for HomeDirGuard {
    fn drop(&mut self) {
        *HOME_DIR_OVERRIDE.lock().unwrap() = self.0.clone();
    }
}

/// 统一 home 解析：测试经 HomeDirGuard 注入覆盖；生产走 dirs::home_dir()
pub(crate) fn home_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(d) = HOME_DIR_OVERRIDE.lock().unwrap().clone() {
            return Some(d);
        }
    }
    dirs::home_dir()
}

/// claude hooks provider（单元结构，静态注册表条目）
#[derive(Debug)]
pub struct ClaudeHooksProvider;

impl CliHooksProvider for ClaudeHooksProvider {
    fn inject(&self) -> Result<AgentHookInjectionStatus, AppError> {
        let script_dir = hooks_dir().ok_or_else(home_dir_err)?;
        let settings_path = claude_settings_path().ok_or_else(home_dir_err)?;
        inject::inject_impl(&settings_path, &script_dir)
    }

    fn uninstall(&self) -> Result<(), AppError> {
        let settings_path = claude_settings_path();
        let script_dir = hooks_dir();
        let events_dir = hooks_events_dir();
        inject::uninstall_impl(
            settings_path.as_deref(),
            script_dir.as_deref(),
            events_dir.as_deref(),
        )
    }

    fn injection_status(&self) -> Result<AgentHookInjectionStatus, AppError> {
        let script_path = match hook_script_path() {
            Some(p) => p,
            None => {
                return Ok(AgentHookInjectionStatus {
                    status: AgentInjectionStatus::NotInjected,
                    version: None,
                });
            }
        };
        let settings_path = match claude_settings_path() {
            Some(p) => p,
            None => {
                return Ok(AgentHookInjectionStatus {
                    status: AgentInjectionStatus::NotInjected,
                    version: None,
                });
            }
        };
        Ok(inject::injection_status_impl(&script_path, &settings_path))
    }

    fn restore_statusline(&self) -> Result<(), AppError> {
        let settings_path = claude_settings_path();
        let backup_path = inject::statusline_backup_path(home_dir());
        inject::restore_statusline_impl(settings_path.as_deref(), backup_path.as_deref())
    }

    fn reinject_statusline(&self) -> Result<(), AppError> {
        let settings_path = claude_settings_path();
        let backup_path = inject::statusline_backup_path(home_dir());
        let script_path = statusline_script_path();
        inject::reinject_statusline_impl(
            settings_path.as_deref(),
            backup_path.as_deref(),
            script_path.as_deref(),
        )
    }

    fn config_read(
        &self,
        layer: &str,
        project_path: Option<&str>,
        project_root: &Option<PathBuf>,
    ) -> Result<Value, AppError> {
        config::config_read_sync(layer, project_path, project_root, home_dir)
    }

    fn config_write(
        &self,
        layer: &str,
        hooks: Value,
        project_path: Option<&str>,
        project_root: &Option<PathBuf>,
    ) -> Result<(), AppError> {
        config::config_write_sync(layer, hooks, project_path, project_root, home_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── home_dir 覆盖守卫（L1 隔离纪律：命令层测试经守卫注入 tempdir） ──

    #[test]
    fn home_dir_override_guard_injects_and_restores() {
        let dir = tempfile::tempdir().unwrap();
        let real = dirs::home_dir();
        {
            let _guard = HomeDirGuard::set(dir.path());
            assert_eq!(home_dir(), Some(dir.path().to_path_buf()));
        }
        assert_eq!(home_dir(), real, "守卫 Drop 后应恢复原 home 解析");
    }

    // ── B15 防复发：provider 层 reinject 必须用 statusline 桥接脚本（非 reporter） ──
    // 背景：B15 根因 = reinject_statusline 误传 hook_script_path()（reporter 路径）——
    // impl 层测试传参正确掩盖了 provider 层路径 bug（注入后重启 settings.json 被写成
    // reporter 包裹：透传末端 stdout 恒空 → TUI 状态行空白 + 状态检测 Outdated）。
    #[test]
    fn reinject_statusline_provider_uses_statusline_script() {
        let home = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(home.path());

        // 三件套：settings.json（statusLine = 原配置）+ 干净备份 + 两脚本落盘
        let claude_dir = home.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let hooks_dir = home.path().join(".slterminal").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        let original = serde_json::json!({
            "type": "command",
            "command": "~/.claude/statusline-deepseek.sh",
        });
        let settings = serde_json::json!({
            "hooks": {},
            "statusLine": original,
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        std::fs::write(
            home.path()
                .join(".slterminal")
                .join("statusline-backup.json"),
            serde_json::to_string_pretty(&original).unwrap(),
        )
        .unwrap();
        // 脚本内容任意——reinject 只检查 is_file()
        std::fs::write(hooks_dir.join("slterm-hook-reporter.js"), "// reporter\n").unwrap();
        std::fs::write(
            hooks_dir.join("slterm-statusline.js"),
            "// statusline bridge\n",
        )
        .unwrap();

        // 启动重注入（provider 层真实路径解析）
        ClaudeHooksProvider.reinject_statusline().unwrap();

        let after: Value = serde_json::from_str(
            &std::fs::read_to_string(claude_dir.join("settings.json")).unwrap(),
        )
        .unwrap();
        let cmd = after
            .get("statusLine")
            .and_then(|sl| sl.get("command"))
            .and_then(|c| c.as_str())
            .expect("statusLine command 应存在");
        assert!(
            cmd.contains("slterm-statusline"),
            "桥接脚本应为 statusline（B15 复发：误用 reporter）: {cmd}"
        );
        assert!(
            !cmd.contains("slterm-hook-reporter"),
            "桥接脚本不得含 reporter（透传末端 stdout 恒空）: {cmd}"
        );
        assert!(
            cmd.contains("~/.claude/statusline-deepseek.sh"),
            "原命令应作为 argv 内嵌: {cmd}"
        );
    }
}
