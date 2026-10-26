//! hook 检视与串行化设置操作,与 config_guard / 未来 CLI 共用。

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use slterm_settings::{AgentHook, RawSettings};

use super::super::installation::{desired_codex_notify, has_codex_notify};
use super::super::integrations::HookInspection;
use super::*;

/// 跨进程串行化:agent-hooks 操作锁(句柄型,进程崩溃内核即释放)。
pub(super) fn lock() -> io::Result<crate::atomic_file::LifetimeFileLock> {
    crate::atomic_file::try_lifetime_lock(&crate::platform::dirs::local_dir().join("agent-hooks"))?
        .ok_or_else(|| io::Error::other("Another hook operation is running. Please retry."))
}

/// (配置文件路径, 是否有足迹)。足迹 = 目录/文件已存在——不 scaffold。
pub(super) fn configuration(agent: AgentHook) -> Option<(PathBuf, bool)> {
    let (path, footprint) = match agent {
        AgentHook::Claude => {
            let dir = claude_config_dir()?;
            (dir.join("settings.json"), dir.is_dir())
        }
        AgentHook::Codex => {
            let path = codex_config_dir()?.join("config.toml");
            let footprint = path.is_file();
            (path, footprint)
        }
        AgentHook::OpenCode => {
            let dir = opencode_config_dir()?;
            (dir.join("plugins/slterm.js"), dir.is_dir())
        }
        AgentHook::Pi => {
            let dir = pi_agent_dir()?;
            (dir.join("extensions/slterm.ts"), dir.is_dir())
        }
        AgentHook::Kimi => {
            let dir = kimi::kimi_config_dir()?;
            (dir.join("config.toml"), dir.is_dir())
        }
        AgentHook::Cursor => {
            let path = cursor::path()?;
            let footprint = path.parent()?.is_dir();
            (path, footprint)
        }
        _ => (extended::path(agent)?, extended::directory(agent)?.is_dir()),
    };
    Some((path, footprint))
}

pub(crate) fn inspect(agent: AgentHook) -> HookInspection {
    inspect_with(agent, configuration(agent), helper_command())
}

/// 显式 claude 配置目录的检视(设置页联调/测试注入 tempdir 假 home)。
/// helper 显式给:三态里的 helper_missing 由 `None` 直接表达。
pub(crate) fn inspect_claude_at(dir: &Path, helper: Option<&str>) -> HookInspection {
    inspect_with(
        AgentHook::Claude,
        Some((dir.join("settings.json"), dir.is_dir())),
        helper.map(str::to_owned),
    )
}

fn inspect_with(
    agent: AgentHook,
    config: Option<(PathBuf, bool)>,
    helper: Option<String>,
) -> HookInspection {
    let mut result = HookInspection {
        helper_missing: helper.is_none(),
        ..Default::default()
    };
    if let Some((path, footprint)) = config {
        result.available = footprint;
        match installed_at(agent, &path, helper.as_deref().unwrap_or_default()) {
            Ok(installed) => {
                result.installed = installed;
                // 已安装的 hook 必须可见,即使保存的授权是关:上一次移除
                // 可能失败过,或是别的工具写入的。
                result.enabled = installed;
                if installed && let Some(helper) = helper.as_deref() {
                    match current_at(agent, &path, helper) {
                        Ok(current) => {
                            result.needs_repair = !current;
                            // claude 的结构比对不一致同样落
                            // 期望/实测哈希指纹,取证面与其他载体对齐。
                            if result.needs_repair && agent == AgentHook::Claude {
                                audit_claude_hash_mismatch(&path, helper);
                            }
                        }
                        Err(error) => result.error = Some(error.to_string()),
                    }
                }
            }
            Err(error) => result.error = Some(error.to_string()),
        }
        result.config_path = Some(path);
    }
    result
}

/// claude hook 文档与期望形态不一致时补哈希审计。期望形态 =
/// 把当前文档修复到完整安装后的序列化字节;实测 = 当前文件字节。只落
/// SHA-256 指纹,不落内容。
fn audit_claude_hash_mismatch(path: &Path, helper: &str) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    if install_into(&mut value, helper) != Some(true) {
        return;
    }
    let Ok(expected) = serde_json::to_string_pretty(&value) else {
        return;
    };
    crate::audit::hook_hash_mismatch(
        path,
        &super::managed_files::fingerprint(expected.as_bytes()),
        &super::managed_files::fingerprint(raw.as_bytes()),
    );
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn installed_at(agent: AgentHook, path: &Path, helper: &str) -> io::Result<bool> {
    match agent {
        AgentHook::Claude => {
            let Some(raw) = read_optional(path)? else {
                return Ok(false);
            };
            let value: Value = serde_json::from_str(&raw).map_err(io::Error::other)?;
            let root = value
                .as_object()
                .ok_or_else(|| io::Error::other("Expected a JSON object"))?;
            let Some(hooks) = root.get("hooks") else {
                return Ok(false);
            };
            let hooks = hooks
                .as_object()
                .ok_or_else(|| io::Error::other("Expected hooks to be an object"))?;
            Ok(CLAUDE_EVENTS.iter().any(|event| {
                hooks
                    .get(*event)
                    .and_then(Value::as_array)
                    .is_some_and(|groups| {
                        groups.iter().any(|group| {
                            group
                                .get("hooks")
                                .and_then(Value::as_array)
                                .is_some_and(|commands| commands.iter().any(is_our_claude_hook))
                        })
                    })
            }))
        }
        AgentHook::Codex => {
            let native = path
                .parent()
                .map(codex_hooks::installed_at)
                .transpose()?
                .unwrap_or(false);
            let Some(raw) = read_optional(path)? else {
                return Ok(native);
            };
            let doc = raw
                .parse::<toml_edit::DocumentMut>()
                .map_err(io::Error::other)?;
            Ok(native
                || doc
                    .get("notify")
                    .and_then(|v| v.as_array())
                    .is_some_and(|array| {
                        let current: Vec<_> = array
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_owned))
                            .collect();
                        has_codex_notify(&current)
                    }))
        }
        AgentHook::Kimi => {
            let Some(raw) = read_optional(path)? else {
                return Ok(false);
            };
            Ok(kimi::inspect(&raw, Path::new(helper))?.0)
        }
        AgentHook::Cursor => cursor::installed_at(path),
        AgentHook::OpenCode | AgentHook::Pi => {
            let content = if agent == AgentHook::Pi {
                PI_EXTENSION_TS
            } else {
                OPENCODE_PLUGIN_JS
            };
            managed_files::installed(path, content)
        }
        _ => extended::installed(agent, helper),
    }
}

/// 「装了」与「装全」是两个不同事实:部分安装的旧配置保持可见,但永远
/// 过不了写入校验。
fn current_at(agent: AgentHook, path: &Path, helper: &str) -> io::Result<bool> {
    let Some(raw) = read_optional(path)? else {
        return Ok(false);
    };
    match agent {
        AgentHook::Cursor => cursor::current(&raw, helper),
        AgentHook::Kimi => Ok(kimi::inspect(&raw, Path::new(helper))?.1),
        AgentHook::Claude => {
            let mut value: Value = serde_json::from_str(&raw).map_err(io::Error::other)?;
            Ok(install_into(&mut value, helper) == Some(false))
        }
        AgentHook::Codex => {
            let doc = raw
                .parse::<toml_edit::DocumentMut>()
                .map_err(io::Error::other)?;
            let Some(array) = doc.get("notify").and_then(|value| value.as_array()) else {
                return Ok(false);
            };
            let Some(current) = array
                .iter()
                .map(|value| value.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
            else {
                return Ok(false);
            };
            Ok(has_codex_notify(&current)
                && desired_codex_notify(&current, helper).is_none()
                && codex_hooks::current_at(
                    path.parent()
                        .ok_or_else(|| io::Error::other("Missing configuration directory"))?,
                    helper,
                )?)
        }
        AgentHook::OpenCode | AgentHook::Pi => {
            let content = if agent == AgentHook::Pi {
                PI_EXTENSION_TS
            } else {
                OPENCODE_PLUGIN_JS
            };
            Ok(raw == content)
        }
        _ => Ok(raw == extended::content(agent, helper)?),
    }
}

pub(crate) fn set_enabled(agent: AgentHook, enabled: bool) -> io::Result<()> {
    let _lock = lock()?;
    if enabled {
        let (_, footprint) = configuration(agent)
            .ok_or_else(|| io::Error::other("Agent configuration unavailable"))?;
        if !footprint {
            return Err(io::Error::other(
                "Start this agent once to create its configuration, then refresh.",
            ));
        }
        if helper_path().is_none() {
            return Err(io::Error::other(
                "The slterm hook helper is missing from this installation.",
            ));
        }
    }
    // 先持久化授权:别的实例拿到锁后只会看到最新授权。settings.json 全仓
    // 唯一写入口 = 应用侧写通道(校验/保存锁/原子写/.bak)。
    crate::settings_store::save_settings_blocking(&[(
        agent.settings_key(),
        slterm_settings::json::Value::Bool(enabled),
    )])?;
    apply_and_verify(agent, enabled)
}

/// 显式 claude 目录的接入/卸载(设置页联调/测试注入;授权已由调用方经
/// 设置写通道落盘,本入口不再写偏好)。写后经同一检视面校验。
pub(crate) fn apply_claude_at(dir: &Path, helper: &str, enabled: bool) -> io::Result<()> {
    apply_claude_at_with(dir, helper, enabled, false)
}

/// confirmed=true 时绕过审查门(设置页已展示原文并确认)。
pub(crate) fn apply_claude_at_with(
    dir: &Path,
    helper: &str,
    enabled: bool,
    confirmed: bool,
) -> io::Result<()> {
    if enabled {
        let outcome = if confirmed {
            ensure_claude_hooks_in(dir, helper, |_| InstallGate::Proceed)
        } else {
            ensure_claude_hooks_at(dir, helper)
        };
        match outcome {
            InstallOutcome::Installed | InstallOutcome::Current => {}
            InstallOutcome::Suspended { suspicious_command } => {
                return Err(io::Error::other(format!(
                    "statusline 原命令需二次确认: {suspicious_command}"
                )));
            }
            InstallOutcome::Conflict => {
                return Err(io::Error::other(
                    "Claude hook 配置无法写入(文件被占用或形状非法)",
                ));
            }
        }
    } else {
        remove_claude_hooks_at(dir)?;
    }
    let state = inspect_claude_at(dir, Some(helper));
    if let Some(error) = state.error {
        return Err(io::Error::other(error));
    }
    if state.installed != enabled || (enabled && state.needs_repair) {
        return Err(io::Error::other(
            "Hook configuration could not be verified after the write.",
        ));
    }
    Ok(())
}

/// 卸载器入口(`setup-ai --remove`):取跨进程锁后移除全部 agent 的 hook
/// 注入。配置不存在不算失败;Cursor 无配置路径时跳过。
pub(crate) fn remove_all() -> io::Result<Vec<(AgentHook, io::Error)>> {
    let _lock = lock()?;
    Ok(remove_all_for(&AgentHook::ALL))
}

/// 不取锁的内核:生产恒全量,测试注入 agent 子集与显式目录。
pub(super) fn remove_all_for(agents: &[AgentHook]) -> Vec<(AgentHook, io::Error)> {
    let mut failures = Vec::new();
    for &agent in agents {
        // 未安装的 CLI 没有配置路径时不算失败——卸载必须能在任意机器上跑完。
        if agent == AgentHook::Cursor && cursor::path().is_none() {
            continue;
        }
        if let Err(error) = apply(agent, false) {
            failures.push((agent, error));
        }
    }
    failures
}

/// 按已落盘的偏好执行接入/卸载(不写授权;03 安装权威的应用入口)。
pub(crate) fn apply_and_verify(agent: AgentHook, enabled: bool) -> io::Result<()> {
    apply_and_verify_with(agent, enabled, false)
}

/// confirmed=true 时审查门恒放行(设置页已展示原文并由用户确认)。
pub(crate) fn apply_and_verify_with(
    agent: AgentHook,
    enabled: bool,
    confirmed: bool,
) -> io::Result<()> {
    apply_with_confirmation(agent, enabled, confirmed)?;
    let state = inspect(agent);
    if let Some(error) = state.error {
        return Err(io::Error::other(error));
    }
    if state.installed != enabled || (enabled && state.needs_repair) {
        return Err(io::Error::other(
            "Hook configuration could not be verified. Check file permissions and configuration, then retry.",
        ));
    }
    Ok(())
}

pub(super) fn apply(agent: AgentHook, enabled: bool) -> io::Result<()> {
    apply_with_confirmation(agent, enabled, false)
}

/// 命中审查词表时给出待确认原文(设置页据此展示「确认并重试」)。
pub(crate) fn statusline_confirmation(agent: AgentHook) -> Option<String> {
    if agent != AgentHook::Claude {
        return None;
    }
    statusline_confirmation_at(&claude_config_dir()?)
}

/// 显式目录形态(设置页联调/测试注入)。
pub(crate) fn statusline_confirmation_at(dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(dir.join("settings.json")).ok()?;
    let root: Value = serde_json::from_str(&raw).ok()?;
    let command = root.get("statusLine")?.get("command")?.as_str()?;
    match review_statusline_command(command) {
        InstallGate::Suspended { suspicious_command } => Some(suspicious_command),
        InstallGate::Proceed => None,
    }
}

pub(super) fn apply_with_confirmation(
    agent: AgentHook,
    enabled: bool,
    confirmed: bool,
) -> io::Result<()> {
    match (agent, enabled) {
        (AgentHook::Claude, true) => {
            let outcome = if confirmed {
                ensure_claude_hooks(|_| InstallGate::Proceed)
            } else {
                ensure_claude_hooks(review_statusline_command)
            };
            // reconcile 路径携带真实审查门:命中 → 审计 + 跳过(零写盘),
            // 无交互可用不当错误上抛(确认路径传 confirmed=true)。
            if let InstallOutcome::Suspended { .. } = outcome {
                crate::audit::record(crate::audit::AuditEvent::new(
                    "statusline_review",
                    "ai_hook::local::settings::apply_with_confirmation",
                    crate::audit::AuditResult::Suspended,
                ));
            }
        }
        (AgentHook::Codex, true) => {
            ensure_codex_notify();
            ensure_codex_hooks();
        }
        (AgentHook::OpenCode, true) => {
            ensure_opencode_plugin();
        }
        (AgentHook::Pi, true) => {
            ensure_pi_extension();
        }
        (AgentHook::Kimi, true) => {
            kimi::ensure_kimi_hooks();
        }
        (AgentHook::Claude, false) => {
            remove_hooks()?;
        }
        (AgentHook::Codex, false) => {
            let native = remove_codex_hooks();
            let notify = remove_codex_notify();
            native?;
            notify?;
        }
        (AgentHook::OpenCode, false) => {
            remove_opencode_plugin()?;
        }
        (AgentHook::Pi, false) => {
            remove_pi_extension()?;
        }
        (AgentHook::Kimi, false) => {
            kimi::remove_kimi_hooks()?;
        }
        (AgentHook::Cursor, _) => {
            let path = cursor::path()
                .ok_or_else(|| io::Error::other("Cursor configuration unavailable"))?;
            cursor::apply_at(&path, &helper_command().unwrap_or_default(), enabled)?;
        }
        (_, _) => extended::apply(agent, &helper_command().unwrap_or_default(), enabled)?,
    }
    Ok(())
}

/// 自愈入口:逐家按授权快照补装(只补缺失,不覆盖)。返回失败清单供
/// config_guard 的失败边通道消费;自愈语义与日志不变。
pub(super) fn heal_enabled(raw: &RawSettings) -> Vec<AgentHook> {
    let mut failures = Vec::new();
    for agent in AgentHook::ALL {
        if agent.enabled(raw)
            && configuration(agent).is_some_and(|(_, footprint)| footprint)
            && let Err(error) = apply(agent, true)
        {
            log::warn!("ai_hook: {}: {error}", agent.settings_key());
            failures.push(agent);
        }
    }
    failures
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn detects_existing_hooks_without_a_saved_preference_or_current_helper() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"hooks":{"Stop":[{"hooks":[{"command":"C:/old/slterm-hook.exe claude"}]}]}}"#,
        )
        .unwrap();
        assert!(installed_at(AgentHook::Claude, &path, "").unwrap());
        std::fs::write(
            &path,
            r#"{"hooks":{"Stop":[{"hooks":[{"command":"user-notifier"}]}]}}"#,
        )
        .unwrap();
        assert!(!installed_at(AgentHook::Claude, &path, "").unwrap());
        std::fs::write(&path, "broken JSON").unwrap();
        assert!(installed_at(AgentHook::Claude, &path, "").is_err());
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "notify = ['C:/old/slterm-hook.exe', 'codex', '--chain', 'my-notifier']\n",
        )
        .unwrap();
        assert!(installed_at(AgentHook::Codex, &path, "").unwrap());
        std::fs::write(&path, "notify = ['my-notifier']\n").unwrap();
        assert!(!installed_at(AgentHook::Codex, &path, "").unwrap());
    }

    #[test]
    fn discovers_installed_plugins_and_reports_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slterm.ts");
        std::fs::write(&path, PI_EXTENSION_TS).unwrap();
        assert!(installed_at(AgentHook::Pi, &path, "").unwrap());
        std::fs::write(&path, "user extension").unwrap();
        assert!(installed_at(AgentHook::Pi, &path, "").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "user extension");
    }

    /// 防复发:claude「已装但需修复」必须落哈希事件。
    #[test]
    fn claude_repair_needed_emits_hash_mismatch_audit_event() {
        let capture = crate::audit::test_support::capture_events();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let helper = "C:/tools/slterm-hook.exe";
        std::fs::write(
            &path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"C:/tools/slterm-hook.exe","args":["claude"]}]}]}}"#,
        )
        .unwrap();
        let state = inspect_claude_at(dir.path(), Some(helper));
        assert!(state.installed, "部分安装仍是已安装");
        assert!(state.needs_repair, "缺事件即需修复");
        let text = capture.text();
        assert!(text.contains("hook_hash_mismatch"), "缺少审计事件: {text}");
        assert!(text.contains("expected="), "{text}");
        assert!(text.contains("actual="), "{text}");
        assert!(
            !text.contains("C:/tools/slterm-hook.exe"),
            "事件只落指纹,不落内容: {text}"
        );
    }

    /// `setup-ai --remove` 的内核:移除自有 hook、保留用户键与外来 hooks。
    #[test]
    fn uninstall_removes_own_hooks_and_preserves_user_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"model":"opus","hooks":{"Stop":[{"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]},{"type":"command","command":"user-notifier"}]}]}}"#,
        )
        .unwrap();
        // CLAUDE_CONFIG_DIR 是进程级;全量测试串行纪律保证用例间互斥。
        unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", dir.path()) };
        let failures = remove_all_for(&[AgentHook::Claude]);
        unsafe { std::env::remove_var("CLAUDE_CONFIG_DIR") };
        assert!(failures.is_empty(), "{failures:?}");
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["model"], json!("opus"), "无关顶层键必须保留");
        assert_eq!(
            root["hooks"]["Stop"],
            json!([{"hooks":[{"type":"command","command":"user-notifier"}]}]),
            "只移除自有条目,用户 handler 原样保留"
        );
    }

    #[test]
    fn partial_claude_install_is_visible_but_never_verified_as_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let helper = "C:/tools/slterm-hook.exe";
        let mut value = json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":helper,"args":HELPER_ARGS}]}]}});
        std::fs::write(&path, value.to_string()).unwrap();
        assert!(installed_at(AgentHook::Claude, &path, helper).unwrap());
        assert!(!current_at(AgentHook::Claude, &path, helper).unwrap());
        install_into(&mut value, helper).unwrap();
        std::fs::write(&path, value.to_string()).unwrap();
        assert!(current_at(AgentHook::Claude, &path, helper).unwrap());
        value["hooks"]["SessionStart"] = json!("invalid shape");
        std::fs::write(&path, value.to_string()).unwrap();
        assert!(!current_at(AgentHook::Claude, &path, helper).unwrap());
    }
}
