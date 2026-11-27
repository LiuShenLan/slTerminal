//! codex native hooks:有界能力探针、归属 JSON 合并与可逆移除。
//!
//! 能力探针真跑 codex(3s 限时、输出 64KB 上限),不认广告信口;探针失败
//! 退回 legacy notify(只有 notify 的合同一直可用)。marker 文件
//! `hooks.slterm-managed.json` 记录我们安装过的精确组。

use std::io::{self, Read, Seek};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::hook_journal::{
    HOOKS_INSTALL_KIND, HOOKS_UNINSTALL_KIND, HookFilePlan, HookJournal, HookJournalPort,
};
use super::{codex_config_dir, helper_path};
use crate::ai_hook::{CodexHookMode, installation};

const MARKER: &str = "hooks.slterm-managed.json";
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

fn read(path: &Path) -> io::Result<Option<String>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut value = String::new();
    file.take(MAX_CONFIG_BYTES + 1).read_to_string(&mut value)?;
    if value.len() as u64 > MAX_CONFIG_BYTES {
        return Err(io::Error::other("hook configuration exceeds size limit"));
    }
    Ok(Some(value))
}

fn write_changed(path: &Path, value: &str) -> io::Result<bool> {
    if read(path)?.as_deref() == Some(value) {
        return Ok(false);
    }
    crate::atomic_file::write(path, value.as_bytes())?;
    Ok(true)
}

/// 真跑 codex 取版本/feature 文本:3s 限时 + 64KB 输出上限,任一超限即
/// 杀进程按「不支持」处理。探针是无控制台的一次性进程,绝不附着宿主
/// 控制台(继承会共享 Ctrl 事件路由)。
fn probe(args: &[&str]) -> Option<String> {
    let mut output = tempfile::tempfile().ok()?;
    // Windows 的 codex 常是 .cmd shim,必须经 cmd.exe 解析 PATHEXT。
    let mut command = Command::new("cmd.exe");
    command.args(["/d", "/c", "codex"]);
    let mut child = crate::platform::process::hidden_command(&mut command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(output.try_clone().ok()?)
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None)
                if Instant::now() < deadline
                    && output.metadata().is_ok_and(|meta| meta.len() <= 64 * 1024) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    output.rewind().ok()?;
    let mut text = String::new();
    output.take(64 * 1024).read_to_string(&mut text).ok()?;
    Some(text)
}

fn supported_mode() -> Option<CodexHookMode> {
    static CACHE: Mutex<Option<(Instant, Option<CodexHookMode>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if let Some((at, mode)) = *cache
        && at.elapsed() < Duration::from_secs(300)
    {
        return mode;
    }
    let mode = probe(&["--version"])
        .zip(probe(&["features", "list"]))
        .and_then(|(version, features)| installation::codex_mode(&version, &features));
    *cache = Some((Instant::now(), mode));
    mode
}

/// 待确认的既有 hooks.json 命令原文；无命中或不可读时为空。
pub(super) fn pending_confirmation() -> Vec<String> {
    let Some(directory) = codex_config_dir().filter(|path| path.is_dir()) else {
        return Vec::new();
    };
    pending_confirmation_at(&directory)
}

/// 显式目录形态(设置页联调/测试注入)；只读不写。
fn pending_confirmation_at(directory: &Path) -> Vec<String> {
    let Ok(Some(raw)) = read(&directory.join("hooks.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(hooks) = value.get("hooks") else {
        return Vec::new();
    };
    match installation::install_gate(hooks) {
        installation::InstallConfirmation::RequireConfirmation(commands) => commands,
        installation::InstallConfirmation::Proceed => Vec::new(),
    }
}

pub(super) fn ensure_codex_hooks(confirmed: bool, journal: Option<&HookJournalPort>) -> bool {
    let Some(directory) = codex_config_dir().filter(|path| path.is_dir()) else {
        return false;
    };
    let Some(helper) = helper_path() else {
        return false;
    };
    let Some(mode) = supported_mode() else {
        return false;
    };
    let helper = helper.to_string_lossy().into_owned().replace('\\', "/");
    match install(&directory, &helper, mode, confirmed, journal) {
        Ok(changed) => {
            if changed {
                log::info!(
                    "ai_hook: Codex native hooks installed; Codex may require review in /hooks"
                );
            }
            changed
        }
        Err(error) => {
            log::warn!("ai_hook: preserving Codex hook configuration: {error}");
            false
        }
    }
}

fn groups(helper: &str, mode: CodexHookMode) -> Value {
    // Codex 在 Windows 经 PowerShell -Command 跑 hook:裸引号路径只是字符串
    // 表达式,须 `&` 调用;单引号保 $、` 等字符字面量,PowerShell 以双倍
    // '' 转义单引号。
    let command = format!(
        "& '{}' codex {}",
        helper.replace('\'', "''"),
        mode.argument()
    );
    installation::codex_groups(&command, Some(command.as_str()), mode)
}

pub(super) fn installed_at(directory: &Path) -> io::Result<bool> {
    let Some(marker) = read(&directory.join(MARKER))? else {
        return Ok(false);
    };
    let marker: Value = serde_json::from_str(&marker)?;
    let previous = marker
        .get("groups")
        .ok_or_else(|| io::Error::other("invalid hook ownership marker"))?;
    let raw = read(&directory.join("hooks.json"))?;
    // 先复用卸载的归属校验,编辑过的自有条目必须在菜单中暴露冲突。
    installation::merge_groups(raw.as_deref(), Some(&previous.to_string()), &json!({}))
        .map_err(io::Error::other)?;
    let current: Value = raw
        .map(|raw| serde_json::from_str(&raw))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    Ok(previous.as_object().is_some_and(|events| {
        events.iter().any(|(event, groups)| {
            groups.as_array().is_some_and(|groups| {
                groups.iter().any(|group| {
                    current["hooks"][event]
                        .as_array()
                        .is_some_and(|entries| entries.contains(group))
                })
            })
        })
    }))
}

pub(super) fn current_at(directory: &Path, helper: &str) -> io::Result<bool> {
    current_for_mode(directory, helper, supported_mode())
}

fn current_for_mode(
    directory: &Path,
    helper: &str,
    mode: Option<CodexHookMode>,
) -> io::Result<bool> {
    let Some(mode) = mode else { return Ok(true) };
    let Some(config) = read(&directory.join("config.toml"))? else {
        return Ok(false);
    };
    let Some((_, introduced)) =
        installation::enable_codex_feature(&config).map_err(io::Error::other)?
    else {
        // provider 的显式 opt-out 保持有效,此时 legacy notify 仍可独立工作。
        return Ok(true);
    };
    if introduced {
        return Ok(false);
    }
    let marker: Value = read(&directory.join(MARKER))?
        .map(|raw| serde_json::from_str(&raw))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    let desired = groups(helper, mode);
    let previous = marker.get("groups").map(Value::to_string);
    let raw = read(&directory.join("hooks.json"))?;
    let (updated, _) = installation::merge_groups(raw.as_deref(), previous.as_deref(), &desired)
        .map_err(io::Error::other)?;
    let current: Option<Value> = raw.map(|raw| serde_json::from_str(&raw)).transpose()?;
    Ok(marker.get("groups") == Some(&desired) && current == Some(serde_json::from_str(&updated)?))
}

/// 单个文件的预期变更；内容不变时不进入 Journal。
fn plan_bytes(
    key: &'static str,
    path: &Path,
    after: Option<&[u8]>,
) -> io::Result<Option<HookFilePlan>> {
    let before = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if before.as_deref() == after {
        return Ok(None);
    }
    Ok(Some(HookFilePlan {
        key,
        path: path.to_path_buf(),
        before,
        after: after.map(|bytes| bytes.to_vec()),
    }))
}

fn write_plan(plan: &HookFilePlan) -> io::Result<()> {
    match &plan.after {
        Some(bytes) => crate::atomic_file::write(&plan.path, bytes),
        None => match std::fs::remove_file(&plan.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

/// 有 Journal 时全量事务化；无 Journal 时保持既有直接原子写。
fn apply_plans(
    journal: Option<&HookJournalPort>,
    kind: &'static str,
    plans: &[HookFilePlan],
) -> io::Result<bool> {
    if plans.is_empty() {
        return Ok(false);
    }
    if let Some(port) = journal {
        let run = HookJournal::begin(port, kind, plans).map_err(io::Error::other)?;
        for plan in plans {
            if let Err(error) = write_plan(plan) {
                let rollback = run.rollback();
                log::warn!("Codex hook write failed; rollback result: {rollback:?}");
                return Err(error);
            }
            if let Err(error) = run.record(plan.key) {
                let rollback = run.rollback();
                log::warn!("Codex hook outcome failed; rollback result: {rollback:?}");
                return Err(io::Error::other(error));
            }
        }
        run.commit().map_err(io::Error::other)?;
        return Ok(true);
    }
    for plan in plans {
        write_plan(plan)?;
    }
    Ok(true)
}

fn install(
    directory: &Path,
    helper: &str,
    mode: CodexHookMode,
    confirmed: bool,
    journal: Option<&HookJournalPort>,
) -> io::Result<bool> {
    let config_path = directory.join("config.toml");
    let Some(_config_lock) = crate::atomic_file::try_lock(&config_path)? else {
        return Ok(false);
    };
    let Some(raw) = read(&config_path)? else {
        return Ok(false);
    };
    let Some((config, introduced)) =
        installation::enable_codex_feature(&raw).map_err(io::Error::other)?
    else {
        return Ok(false);
    };
    let hooks_path = directory.join("hooks.json");
    let Some(_lock) = crate::atomic_file::try_lock(&hooks_path)? else {
        return Ok(false);
    };
    let marker_path = directory.join(MARKER);
    let previous: Value = read(&marker_path)?
        .map(|raw| serde_json::from_str(&raw))
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_else(|| json!({}));
    let groups = groups(helper, mode);
    let previous_groups = previous.get("groups").map(Value::to_string);
    let current_hooks = read(&hooks_path)?;
    // 安装前审查与合并共用同一把文件锁：未确认时命中词表零写入。
    if !confirmed
        && let Some(raw) = current_hooks.as_deref()
        && let Ok(value) = serde_json::from_str::<Value>(raw)
        && let Some(hooks) = value.get("hooks")
        && let installation::InstallConfirmation::RequireConfirmation(_) =
            installation::install_gate(hooks)
    {
        crate::audit::record(crate::audit::AuditEvent::new(
            "hook_install_review",
            "ai_hook::local::codex_hooks::install",
            crate::audit::AuditResult::Suspended,
        ));
        return Ok(false);
    }
    let (hooks, _) = installation::merge_groups(
        current_hooks.as_deref(),
        previous_groups.as_deref(),
        &groups,
    )
    .map_err(io::Error::other)?;
    let marker = json!({
        "groups":groups,
        "enabled_feature": previous.get("enabled_feature").and_then(Value::as_bool).unwrap_or(false) || introduced,
    });
    let marker = serde_json::to_string_pretty(&marker)? + "\n";
    // marker 先写：即使 hooks/config 稍后失败，卸载仍能定位自有组。
    let plans = [
        plan_bytes("marker", &marker_path, Some(marker.as_bytes()))?,
        plan_bytes("hooks", &hooks_path, Some(hooks.as_bytes()))?,
        plan_bytes("config", &config_path, Some(config.as_bytes()))?,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    apply_plans(journal, HOOKS_INSTALL_KIND, &plans)
}

pub(super) fn remove_codex_hooks(journal: Option<&HookJournalPort>) -> io::Result<bool> {
    let Some(directory) = codex_config_dir() else {
        return Ok(false);
    };
    remove(&directory, journal)
}

fn remove(directory: &Path, journal: Option<&HookJournalPort>) -> io::Result<bool> {
    if !directory.join(MARKER).is_file() {
        return Ok(false);
    }
    let Some(_config_lock) = crate::atomic_file::try_lock(&directory.join("config.toml"))? else {
        return Err(io::Error::other("Codex configuration is busy"));
    };
    let marker_path = directory.join(MARKER);
    let Some(marker) = read(&marker_path)? else {
        return Ok(false);
    };
    let marker: Value = serde_json::from_str(&marker)?;
    let hooks_path = directory.join("hooks.json");
    let Some(_lock) = crate::atomic_file::try_lock(&hooks_path)? else {
        return Err(io::Error::other("Codex hook configuration is busy"));
    };
    let previous = marker
        .get("groups")
        .ok_or_else(|| io::Error::other("invalid hook ownership marker"))?
        .to_string();
    let (hooks, _) =
        installation::merge_groups(read(&hooks_path)?.as_deref(), Some(&previous), &json!({}))
            .map_err(io::Error::other)?;
    let config_path = directory.join("config.toml");
    let mut config_after: Option<String> = None;
    let remaining: Value = serde_json::from_str(&hooks)?;
    if marker.get("enabled_feature").and_then(Value::as_bool) == Some(true)
        && remaining["hooks"]
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        && let Some(raw) = read(&config_path)?
    {
        let mut config = raw
            .parse::<toml_edit::DocumentMut>()
            .map_err(io::Error::other)?;
        // 内联 TOML hooks 也持有这个 feature,即使 hooks.json 已空。
        if config.get("hooks").is_none()
            && let Some(features) = config
                .get_mut("features")
                .and_then(toml_edit::Item::as_table_like_mut)
            && features.get("hooks").and_then(toml_edit::Item::as_bool) == Some(true)
        {
            features.remove("hooks");
            config_after = Some(config.to_string());
        }
    }
    let plans = [
        plan_bytes("hooks", &hooks_path, Some(hooks.as_bytes()))?,
        config_after
            .as_deref()
            .map(|config| plan_bytes("config", &config_path, Some(config.as_bytes())))
            .transpose()?
            .flatten(),
        // marker 最后删除：它存在就代表卸载尚未完成。
        plan_bytes("marker", &marker_path, None)?,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    apply_plans(journal, HOOKS_UNINSTALL_KIND, &plans)?;
    Ok(true)
}

#[cfg(test)]
mod codex_hooks_tests {
    use super::*;

    #[test]
    fn upgrades_cmd_style_owned_hooks_without_changing_user_entries() {
        let root = tempfile::tempdir().unwrap();
        let helper = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";
        let old_command = format!("\"{helper}\" codex --hooks=full");
        let previous =
            installation::codex_groups(&old_command, Some(&old_command), CodexHookMode::Full);
        let foreign = json!({"hooks":[{"type":"command","command":"user-hook"}]});
        let mut hooks = json!({"hooks": previous});
        hooks["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(foreign.clone());
        std::fs::write(root.path().join("hooks.json"), hooks.to_string()).unwrap();
        std::fs::write(
            root.path().join(MARKER),
            json!({"groups": previous, "enabled_feature": true}).to_string(),
        )
        .unwrap();
        let config = "notify = ['user-notifier']\n[features]\nhooks = true\n";
        std::fs::write(root.path().join("config.toml"), config).unwrap();

        assert!(!current_for_mode(root.path(), helper, Some(CodexHookMode::Full)).unwrap());
        assert!(install(root.path(), helper, CodexHookMode::Full, true, None).unwrap());
        assert!(current_for_mode(root.path(), helper, Some(CodexHookMode::Full)).unwrap());
        assert!(!install(root.path(), helper, CodexHookMode::Full, true, None).unwrap());
        let hooks: Value =
            serde_json::from_str(&read(&root.path().join("hooks.json")).unwrap().unwrap()).unwrap();
        let mut expected = json!({"hooks": groups(helper, CodexHookMode::Full)});
        expected["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .insert(0, foreign);
        assert_eq!(hooks, expected);
        assert_eq!(
            read(&root.path().join("config.toml")).unwrap().unwrap(),
            config
        );
    }

    /// 生成的 PowerShell 命令真跑一轮:字面量路径(含 '/$/空格/反引号)+
    /// stdin 载荷直通,这是「helper 命令行形态与 provider 调用方式互钉」的
    /// 实机锚点。
    #[test]
    fn powershell_executes_generated_hooks_with_literal_paths_and_stdin() {
        use std::io::Write as _;

        let root = tempfile::tempdir().unwrap();
        let helper = root.path().join("slTerm's $data `hook.ps1");
        std::fs::write(
            &helper,
            "Write-Output ($args -join '|')\r\nWrite-Output ([Console]::In.ReadLine())\r\n",
        )
        .unwrap();
        let powershell = Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        for mode in [CodexHookMode::Turns, CodexHookMode::Full] {
            let groups = groups(&helper.to_string_lossy().replace('\\', "/"), mode);
            let command = groups["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
            let mut probe = Command::new(&powershell);
            probe
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    command,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = crate::platform::process::hidden_command(&mut probe)
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"{\"session_id\":\"fixture\"}\n")
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                stdout.contains(&format!("codex|{}", mode.argument())),
                "{stdout}"
            );
            assert!(stdout.contains(r#"{"session_id":"fixture"}"#), "{stdout}");
        }
    }

    #[test]
    fn install_upgrade_remove_preserves_notify_and_user_hooks() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.toml");
        std::fs::write(&config, "# user\nnotify = ['notifier', 'argument']\n").unwrap();
        std::fs::write(
            root.path().join("hooks.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"user-hook"}]}]}}"#,
        )
        .unwrap();
        let helper = "C:/Program Files/slTerminal/slterm-hook.exe";
        assert!(install(root.path(), helper, CodexHookMode::Turns, true, None).unwrap());
        assert!(!install(root.path(), helper, CodexHookMode::Turns, true, None).unwrap());
        assert!(
            install(
                root.path(),
                "D:/new/slterm-hook.exe",
                CodexHookMode::Full,
                true,
                None
            )
            .unwrap()
        );
        assert!(remove(root.path(), None).unwrap());
        let hooks: Value =
            serde_json::from_str(&read(&root.path().join("hooks.json")).unwrap().unwrap()).unwrap();
        assert_eq!(hooks["hooks"]["Stop"].as_array().unwrap().len(), 1);
        let config = read(&config).unwrap().unwrap();
        assert!(config.contains("# user") && config.contains("notify = ['notifier', 'argument']"));
        assert!(
            config.contains("hooks = true"),
            "another hook still needs the feature"
        );
    }

    #[test]
    fn repeated_toggle_cycles_keep_one_owned_group_and_preserve_the_original_notifier() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.toml");
        std::fs::write(&config, "# keep\nnotify = ['user-notifier', 'argument']\n").unwrap();
        let foreign = json!({"hooks":[{"type":"command","command":"user-hook"}]});
        std::fs::write(
            root.path().join("hooks.json"),
            json!({"hooks":{"Stop":[foreign]}}).to_string(),
        )
        .unwrap();
        let helper = "C:/Program Files/slTerminal/slterm-hook.exe";
        for _ in 0..20 {
            assert!(install(root.path(), helper, CodexHookMode::Full, true, None).unwrap());
            assert!(installed_at(root.path()).unwrap());
            assert!(current_for_mode(root.path(), helper, Some(CodexHookMode::Full)).unwrap());
            assert!(!install(root.path(), helper, CodexHookMode::Full, true, None).unwrap());
            let hooks: Value =
                serde_json::from_str(&read(&root.path().join("hooks.json")).unwrap().unwrap())
                    .unwrap();
            assert_eq!(hooks["hooks"]["Stop"].as_array().unwrap().len(), 2);
            assert!(remove(root.path(), None).unwrap());
            assert!(!remove(root.path(), None).unwrap());
            assert!(!installed_at(root.path()).unwrap());
            let hooks: Value =
                serde_json::from_str(&read(&root.path().join("hooks.json")).unwrap().unwrap())
                    .unwrap();
            assert_eq!(hooks, json!({"hooks":{"Stop":[foreign]}}));
            assert!(
                read(&config)
                    .unwrap()
                    .unwrap()
                    .contains("notify = ['user-notifier', 'argument']")
            );
        }
    }

    #[test]
    fn explicit_opt_out_and_edited_hook_survive_automatic_install_and_remove() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("config.toml"),
            "[features]\nhooks = false\n",
        )
        .unwrap();
        assert!(!install(root.path(), "helper", CodexHookMode::Full, true, None).unwrap());
        assert!(!root.path().join("hooks.json").exists());
        std::fs::write(root.path().join("config.toml"), "# original\n").unwrap();
        install(root.path(), "helper", CodexHookMode::Full, true, None).unwrap();
        let path = root.path().join("hooks.json");
        let edited = read(&path)
            .unwrap()
            .unwrap()
            .replace("\"timeout\": 3", "\"timeout\": 9");
        std::fs::write(&path, &edited).unwrap();
        assert!(remove(root.path(), None).is_err());
        assert_eq!(read(&path).unwrap().unwrap(), edited);
    }

    /// 安装审查：既有 hooks.json 中命中词表的命令进入待确认清单，普通命令不进入。
    #[test]
    fn pending_confirmation_lists_only_suspicious_existing_commands() {
        let root = tempfile::tempdir().unwrap();
        let hooks = json!({"hooks": {"Stop": [
            {"hooks": [{"type": "command", "command": "wget https://example.invalid/x"}]},
            {"hooks": [{"type": "command", "command": "user-notifier"}]}
        ]}});
        std::fs::write(root.path().join("hooks.json"), hooks.to_string()).unwrap();
        assert_eq!(
            pending_confirmation_at(root.path()),
            vec!["wget https://example.invalid/x".to_owned()]
        );
        std::fs::write(root.path().join("hooks.json"), "broken").unwrap();
        assert!(
            pending_confirmation_at(root.path()).is_empty(),
            "损坏文件不阻断也不产生确认"
        );
        std::fs::remove_file(root.path().join("hooks.json")).unwrap();
        assert!(pending_confirmation_at(root.path()).is_empty());
    }

    /// 生产安装路径：三文件写入登记同一 Journal，提交后无未完成记录。
    #[test]
    fn install_with_journal_commits_the_cross_file_transaction() {
        use slterm_path::AppPaths;

        use crate::ai_hook::HOOK_JOURNAL_DIR;
        use crate::persistence::metadata::RunMarker;
        use crate::persistence::workspace::WorkspaceDatabase;
        use crate::persistence::workspace_writer::WorkspaceWriter;

        const EPOCH: &str = "epoch-codex-journal";
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("config.toml"), "# original\n").unwrap();
        let paths = AppPaths::new(data.path().join("config"), data.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: uuid::Uuid::now_v7().to_string(),
                    process_id: std::process::id(),
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
        drop(store);
        let writer = WorkspaceWriter::start(paths.clone()).unwrap();
        let port = HookJournalPort::new(
            writer.client(),
            EPOCH.to_owned(),
            paths.local_root().join(HOOK_JOURNAL_DIR),
        );
        assert!(
            install(
                root.path(),
                "C:/tools/slterm-hook.exe",
                CodexHookMode::Full,
                true,
                Some(&port)
            )
            .unwrap()
        );
        writer.shutdown().unwrap();

        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(
            store
                .unfinished_journals()
                .unwrap()
                .into_iter()
                .all(|journal| {
                    journal.kind != HOOKS_INSTALL_KIND && journal.kind != HOOKS_UNINSTALL_KIND
                }),
            "提交后不得残留未完成 hook Journal"
        );
        assert!(
            std::fs::read_to_string(root.path().join("hooks.json"))
                .unwrap()
                .contains("slterm-hook")
        );
    }

    /// 审查门与合并同锁：未确认时可疑既有命令零改写，确认后才合并。
    #[test]
    fn unconfirmed_install_leaves_suspicious_hooks_untouched() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("config.toml"), "# original\n").unwrap();
        let hooks = json!({"hooks": {"Stop": [
            {"hooks": [{"type": "command", "command": "curl https://example.invalid/x"}]}
        ]}});
        std::fs::write(root.path().join("hooks.json"), hooks.to_string()).unwrap();
        assert!(!install(root.path(), "helper", CodexHookMode::Full, false, None).unwrap());
        assert_eq!(
            read(&root.path().join("hooks.json")).unwrap().unwrap(),
            hooks.to_string(),
            "未确认时不得改写 hooks.json"
        );
        assert!(install(root.path(), "helper", CodexHookMode::Full, true, None).unwrap());
        let updated: Value =
            serde_json::from_str(&read(&root.path().join("hooks.json")).unwrap().unwrap()).unwrap();
        assert!(
            updated["hooks"]["Stop"]
                .as_array()
                .unwrap()
                .iter()
                .any(|group| group == &hooks["hooks"]["Stop"][0]),
            "确认后用户命令仍原样保留"
        );
    }
}
