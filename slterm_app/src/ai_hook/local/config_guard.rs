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
use std::time::Duration;

use slterm_settings::{AgentHook, RawSettings};

use super::settings;

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

fn heal_all() {
    let Ok(_lock) = settings::lock() else { return };
    // 锁内重读授权,不能把菜单/卸载刚移除的 hook 按旧快照装回。
    let raw = match RawSettings::try_load() {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!("ai_hook: cannot read hook preferences; installation skipped: {error}");
            return;
        }
    };
    // 全局开关短路经由 AgentHook::enabled 逐家生效(独立键 > 全局键 > 默认
    // 两家——M1 键域钉死的语义);显式开了某家的用户不因全局关闭被回拆。
    settings::heal_enabled(&raw);
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
                    log::warn!("ai_hook: failed to install {agent} runtime skill: {error}")
                }
            }
        }
    }
}

fn config_guard(tx: mpsc::SyncSender<()>, rx: mpsc::Receiver<()>, stop: Arc<AtomicBool>) {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::RecvTimeoutError;

    heal_all();
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
            heal_all();
        }
    }
}
