//! 安装/修复调度:provider 文件编辑留在各家安装器里,这里只做调度。
//!
//! 形态:notify watcher 监听各家配置文件目录 + 唤醒 channel 合并信号 +
//! 300s 轮询兜底(watcher 建不起来的目录也覆盖)+ Drop 停止。

use std::collections::HashSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use slterm_settings::{AgentHook, RawSettings};

use super::settings;
use crate::ai_assistant::{AssistantConfig, HookFailure, TriggerGate, evaluate_hook_failure};

pub struct ConfigGuard {
    stop: Arc<AtomicBool>,
    wake: mpsc::SyncSender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// 消费点归 06 篇设置域启动链(挂 app 生命周期,`ai_hooks` 开关落定后
/// 按授权自愈);M4.5 只接事件通道,本步只落本体。
pub fn spawn_config_guard() -> Option<ConfigGuard> {
    let (wake, receive) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let events = wake.clone();
    match std::thread::Builder::new()
        .name("slterm-ai-setup".into())
        .spawn(move || config_guard(events, receive, stopping))
    {
        Ok(worker) => Some(ConfigGuard {
            stop,
            wake,
            worker: Some(worker),
        }),
        Err(error) => {
            log::warn!("ai_hook: failed to spawn settings guard: {error}");
            None
        }
    }
}

/// 自愈一轮。`failure_gate` 是进程级失败边闸门(D08-2),跨轮存活以让
/// 冷却生效;自愈主链语义不因失败边改变。
fn heal_all(failure_gate: &mut TriggerGate) {
    let Ok(_lock) = settings::lock() else { return };
    // 锁竞争 = 另一个安装操作在进行,不是自愈失败,不投失败边。
    // 锁内重读授权,不能把菜单/卸载刚移除的 hook 按旧快照装回。
    let raw = match RawSettings::try_load() {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!("ai_hook: cannot read hook preferences; installation skipped: {error}");
            report_heal_failures(failure_gate, &[settings_failure()], Instant::now());
            return;
        }
    };
    let mut failures = Vec::new();
    // 全局开关短路经由 AgentHook::enabled 逐家生效(独立键 > 全局键 > 默认
    // 两家——M1 键域钉死的语义);显式开了某家的用户不因全局关闭被回拆。
    for agent in settings::heal_enabled(&raw) {
        failures.push(install_failure(agent));
    }
    // runtime skill 自愈(04 篇 M7.4):与 hook 自愈同一授权快照;全局显式
    // 关闭(`slterm_ai_hooks=false`)不进 skill 自愈。只为已有配置目录的
    // agent 投放,单家失败不阻断另一家。
    if raw.bool_on(AgentHook::GLOBAL_SETTINGS_KEY) != Some(false) {
        for (agent, path, result) in super::runtime_skills::ensure_runtime_skills() {
            match result {
                Ok(super::runtime_skills::ManagedSkillInstall::Installed) => {
                    log::info!(
                        "ai_hook: installed {agent} runtime skill at {}",
                        path.display()
                    )
                }
                Ok(super::runtime_skills::ManagedSkillInstall::Current) => {}
                Ok(super::runtime_skills::ManagedSkillInstall::Conflict) => log::warn!(
                    "ai_hook: preserving edited or unmanaged {agent} runtime skill near {}",
                    path.display()
                ),
                Err(error) => {
                    log::warn!("ai_hook: failed to install {agent} runtime skill: {error}");
                    failures.push(skill_failure(agent));
                }
            }
        }
    }
    report_heal_failures(failure_gate, &failures, Instant::now());
}

/// hook 安装/修复失败的失败边投影。命令描述只参与规则表判定
/// (08 篇 `should_suggest`),不对应用户输入的历史命令。
fn install_failure(agent: AgentHook) -> HookFailure {
    HookFailure {
        command: format!("slterm-hook install {}", agent.settings_key()),
        program: None,
    }
}

/// runtime skill 投放失败的失败边投影。
fn skill_failure(agent: &str) -> HookFailure {
    HookFailure {
        command: format!("slterm-hook skill {agent}"),
        program: None,
    }
}

/// 授权快照不可读时的失败边投影(本轮自愈没有执行)。
fn settings_failure() -> HookFailure {
    HookFailure {
        command: "slterm-hook settings".into(),
        program: None,
    }
}

/// 失败边消费点(08 篇 M9.3 / D08-2):一条自愈失败进入 AI assistant
/// 触发闸门。配置每轮失败读一次;同一轮的多条失败共享同一 `now`,冷却
/// 保证只投第一条。默认关时 [`evaluate_hook_failure`] 不碰闸门。
fn report_heal_failures(
    gate: &mut TriggerGate,
    failures: &[HookFailure],
    now: Instant,
) -> Option<u64> {
    if failures.is_empty() {
        return None;
    }
    report_heal_failures_at(gate, &AssistantConfig::load(), failures, now)
}

/// 显式配置注入形态(测试用),避免读生产数据目录。
fn report_heal_failures_at(
    gate: &mut TriggerGate,
    config: &AssistantConfig,
    failures: &[HookFailure],
    now: Instant,
) -> Option<u64> {
    let mut fired = None;
    for failure in failures {
        if let Some(seq) = evaluate_hook_failure(gate, config, failure, now) {
            log::info!(
                "ai_hook: failure edge entered AI trigger chain: {} (seq {seq})",
                failure.command
            );
            fired.get_or_insert(seq);
        }
    }
    fired
}

fn config_guard(tx: mpsc::SyncSender<()>, rx: mpsc::Receiver<()>, stop: Arc<AtomicBool>) {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::RecvTimeoutError;

    // 进程级失败边闸门(D08-2):跨轮存活,冷却才有效。
    let mut failure_gate = TriggerGate::default();
    heal_all(&mut failure_gate);
    // 配置事件只代表需要重读;合并成一个信号,避免繁忙目录堆积整批事件。
    let mut watcher =
        match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            let relevant = result.as_ref().map_or(true, |event| {
                event.paths.is_empty()
                    || event.paths.iter().any(|path| {
                        path.file_name().is_some_and(|name| {
                            name == "settings.json"
                                || name == "config.toml"
                                || name == "hooks.json"
                                || name == "slterm.js"
                                || name == "slterm.ts"
                                || name == "slterm.json"
                        })
                    })
            });
            if relevant {
                let _ = tx.try_send(());
            }
        }) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                log::warn!("ai_hook: settings watcher unavailable ({error}); polling instead");
                None
            }
        };
    let mut watched = HashSet::new();
    while !stop.load(Ordering::Acquire) {
        let dirs = std::iter::once(slterm_settings::settings_dir()).chain(
            AgentHook::ALL.into_iter().filter_map(|agent| {
                settings::configuration(agent)?
                    .0
                    .parent()
                    .map(std::path::Path::to_path_buf)
            }),
        );
        for dir in dirs.filter(|dir| dir.is_dir()) {
            if let Some(watcher) = watcher.as_mut()
                && !watched.contains(&dir)
            {
                match watcher.watch(&dir, RecursiveMode::NonRecursive) {
                    Ok(()) => {
                        watched.insert(dir);
                    }
                    Err(error) => log::debug!("ai_hook: cannot watch {}: {error}", dir.display()),
                }
            }
        }
        match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(()) => {
                while !stop.load(Ordering::Acquire)
                    && rx.recv_timeout(Duration::from_millis(400)).is_ok()
                {}
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // 定时一轮兼顾启动后新安装的 agent,以及未能建立 watcher 的目录。
        if !stop.load(Ordering::Acquire) {
            heal_all(&mut failure_gate);
        }
    }
}

#[cfg(test)]
mod config_guard_tests {
    use super::*;
    use crate::ai_assistant::{COOLDOWN, HOOK_FAILURE_EXIT_CODE};

    fn enabled_config() -> AssistantConfig {
        AssistantConfig {
            enabled: true,
            ..AssistantConfig::default()
        }
    }

    #[test]
    fn failure_edge_is_silent_by_default() {
        let mut gate = TriggerGate::default();
        let config = AssistantConfig::default();
        assert!(!config.enabled, "D08-2:失败边默认关");
        let now = Instant::now();
        assert!(
            report_heal_failures_at(
                &mut gate,
                &config,
                &[install_failure(AgentHook::Claude)],
                now,
            )
            .is_none()
        );
        // 默认关不碰闸门:同一时刻开启后立即可触发(冷却未被占位)。
        assert!(
            gate.evaluate(true, 1, "cargoo build", None, &[], now)
                .is_some()
        );
    }

    #[test]
    fn enabled_failure_edge_enters_the_gate_with_cooldown() {
        let mut gate = TriggerGate::default();
        let config = enabled_config();
        let now = Instant::now();
        let failures = [install_failure(AgentHook::Claude), skill_failure("codex")];
        let first = report_heal_failures_at(&mut gate, &config, &failures, now).unwrap();
        // 同轮第二条失败共享同一时刻,被冷却吸收。
        assert!(
            report_heal_failures_at(&mut gate, &config, &failures, now + Duration::from_secs(4))
                .is_none()
        );
        let next = report_heal_failures_at(&mut gate, &config, &failures, now + COOLDOWN).unwrap();
        assert!(first < next, "seq 必须全局递增: {first} {next}");
    }

    #[test]
    fn ignored_exit_code_silences_failure_edge() {
        let mut gate = TriggerGate::default();
        let config = AssistantConfig {
            enabled: true,
            ignored_exit_codes: vec![HOOK_FAILURE_EXIT_CODE],
            ..AssistantConfig::default()
        };
        let now = Instant::now();
        assert!(
            report_heal_failures_at(
                &mut gate,
                &config,
                &[install_failure(AgentHook::Claude)],
                now,
            )
            .is_none()
        );
        // 规则门不过不写冷却:同一时刻其它失败仍可触发。
        assert!(
            gate.evaluate(true, 1, "cargoo build", None, &[], now)
                .is_some()
        );
    }

    #[test]
    fn failure_edge_commands_avoid_rule_table_traps() {
        // claude/codex 在 INTERACTIVE 表里;失败边 program 必须为 None,
        // 否则安装失败会被当成交互式程序的常态退出而静音。
        let install = install_failure(AgentHook::Claude);
        assert_eq!(
            install.command,
            "slterm-hook install slterm_ai_hooks_claude"
        );
        assert!(install.program.is_none());
        assert_eq!(skill_failure("codex").command, "slterm-hook skill codex");
        assert_eq!(skill_failure("codex").program, None);
        assert_eq!(settings_failure().command, "slterm-hook settings");
        assert_eq!(settings_failure().program, None);

        let mut gate = TriggerGate::default();
        let now = Instant::now();
        assert!(report_heal_failures_at(&mut gate, &enabled_config(), &[install], now).is_some());
    }
}
