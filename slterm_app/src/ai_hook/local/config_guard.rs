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

/// 自愈失败的用户可见类别；不包含命令或配置正文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookFailureKind {
    /// 某家 agent 的 hook 注入/修复失败（agent = CLI slug）。
    Install { agent: String },
    /// 自愈授权快照不可读，无法判断该装什么。
    Settings,
}

/// 同类后台失败的提示冷却，避免 watcher 连发形成通知风暴。
const COOLDOWN: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct FailureGate {
    last_fired: Option<Instant>,
}

impl FailureGate {
    fn report(&mut self, failures: &[HookFailureKind], now: Instant) -> Option<HookFailureKind> {
        let first = failures.first()?;
        if self
            .last_fired
            .is_some_and(|last| now.saturating_duration_since(last) < COOLDOWN)
        {
            return None;
        }
        self.last_fired = Some(now);
        Some(first.clone())
    }
}

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

/// 消费点归设置域启动链(挂 app 生命周期,`ai_hooks` 开关落定后
/// 按授权自愈);只接事件通道,本模块只落本体。
///
/// 失败边呈现回调只拿到 [`HookFailureKind`](不含合成命令),由壳启动链转成
/// 进程级通知;不需要提示的调用方传 `|_| {}`。
pub fn spawn_config_guard_with<F>(on_alert: F) -> Option<ConfigGuard>
where
    F: Fn(HookFailureKind) + Send + 'static,
{
    let (wake, receive) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let events = wake.clone();
    match std::thread::Builder::new()
        .name("slterm-ai-setup".into())
        .spawn(move || config_guard(events, receive, stopping, on_alert))
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

/// 自愈一轮。`failure_gate` 跨轮存活，让连续 watcher 事件共享提示冷却。
fn heal_all(failure_gate: &mut FailureGate, on_alert: &(impl Fn(HookFailureKind) + ?Sized)) {
    let Ok(_lock) = settings::lock() else { return };
    // 锁竞争 = 另一个安装操作在进行，不是自愈失败，不提示。
    // 锁内重读授权，不能把菜单/卸载刚移除的 hook 按旧快照装回。
    let raw = match RawSettings::try_load() {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!("ai_hook: cannot read hook preferences; installation skipped: {error}");
            if let Some(kind) =
                report_heal_failures(failure_gate, &[settings_failure()], Instant::now())
            {
                on_alert(kind);
            }
            return;
        }
    };
    let mut failures = Vec::new();
    // 全局开关短路经由 AgentHook::enabled 逐家生效(独立键 > 全局键 > 默认
    // ——键域钉死的语义);显式开了某家的用户不因全局关闭被回拆。
    for agent in settings::heal_enabled(&raw) {
        failures.push(install_failure(agent));
    }
    if let Some(kind) = report_heal_failures(failure_gate, &failures, Instant::now()) {
        on_alert(kind);
    }
}

fn install_failure(agent: AgentHook) -> HookFailureKind {
    HookFailureKind::Install {
        agent: agent
            .settings_key()
            .trim_start_matches("slterm_ai_hooks_")
            .to_owned(),
    }
}

fn settings_failure() -> HookFailureKind {
    HookFailureKind::Settings
}

/// 同一轮只提示首条失败；空失败不消耗冷却。
fn report_heal_failures(
    gate: &mut FailureGate,
    failures: &[HookFailureKind],
    now: Instant,
) -> Option<HookFailureKind> {
    gate.report(failures, now)
}

fn config_guard(
    tx: mpsc::SyncSender<()>,
    rx: mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
    on_alert: impl Fn(HookFailureKind),
) {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::RecvTimeoutError;

    // 进程级提示冷却：跨轮存活，watcher 连发才不重复提示。
    let mut failure_gate = FailureGate::default();
    heal_all(&mut failure_gate, &on_alert);
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
            heal_all(&mut failure_gate, &on_alert);
        }
    }
}

#[cfg(test)]
mod config_guard_tests {
    use super::*;

    #[test]
    fn empty_failures_do_not_consume_cooldown() {
        let mut gate = FailureGate::default();
        let now = Instant::now();
        assert_eq!(report_heal_failures(&mut gate, &[], now), None);
        assert_eq!(
            report_heal_failures(&mut gate, &[install_failure(AgentHook::Claude)], now),
            Some(HookFailureKind::Install {
                agent: "claude".to_owned()
            })
        );
    }

    #[test]
    fn failure_alerts_are_cooled_down_without_assistant_config() {
        let mut gate = FailureGate::default();
        let now = Instant::now();
        let failures = [install_failure(AgentHook::Claude), settings_failure()];
        assert_eq!(
            report_heal_failures(&mut gate, &failures, now),
            Some(HookFailureKind::Install {
                agent: "claude".to_owned()
            }),
            "同一轮只提示首条失败"
        );
        assert_eq!(
            report_heal_failures(&mut gate, &failures, now + Duration::from_secs(4)),
            None
        );
        assert_eq!(
            report_heal_failures(&mut gate, &failures, now + COOLDOWN),
            Some(HookFailureKind::Install {
                agent: "claude".to_owned()
            })
        );
    }

    /// 失败提示只带 agent 与类别，不携带合成命令。
    #[test]
    fn failure_kinds_carry_no_synthetic_command_text() {
        let install = install_failure(AgentHook::Codex);
        assert_eq!(
            install,
            HookFailureKind::Install {
                agent: "codex".to_owned()
            }
        );
        assert_eq!(settings_failure(), HookFailureKind::Settings);
        for kind in [install, settings_failure()] {
            let display = format!("{kind:?}");
            assert!(
                !display.contains("slterm-hook"),
                "呈现类别不得含合成命令: {display}"
            );
        }
    }
}
