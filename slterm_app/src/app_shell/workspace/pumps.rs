//! 工作区后台泵与窗口收尾:自动保存 / AI hook / 屏幕看门狗 /
//! 壳事件队列 / 系统关窗。
//!
//! 从 `mod.rs` 拆出只为文件行数预算;各泵的节奏与因果注释随实现迁移,
//! 前台线程只做状态更新,阻塞读取都在管道/后台线程。

use std::sync::mpsc;
use std::time::Duration;

use gpui::{App, Context, Window};

use super::super::SltermShellEvent;
use super::{SltermWorkspace, session_persistence::SaveReason, session_recovery};
use crate::ai_hook::AiHookEvent;
use crate::background_tasks;

impl SltermWorkspace {
    /// 1 Hz 自动保存泵:节奏与 agent 看门狗同源,前台线程
    /// 拍快照、状态机判无变化/写盘。失败只 warn,终端不受影响。
    pub(super) fn start_autosave_pump(&mut self, cx: &mut Context<'_, Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(1)).await;
                let Ok(()) = this.update(cx, |workspace, cx| workspace.autosave_tick(cx)) else {
                    return;
                };
            }
        })
        .detach();
    }

    /// 一拍自动保存:快照 → 状态机 → 失败 warn。首次成功快照的
    /// `boot_attempts = 0` 由 `snapshot_session` 的规范形态保证。
    pub(super) fn autosave_tick(&mut self, cx: &mut Context<'_, Self>) {
        let window_state = Self::current_window_state(cx);
        let session = self.snapshot_session(window_state, cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::Checkpoint)
        {
            log::warn!("session checkpoint: {error}");
        }
    }

    /// 单窗:取第一个窗口的当前尺寸/最大化态(无窗口 = 不写 window 段)。
    fn current_window_state(cx: &mut App) -> Option<crate::session::WindowState> {
        let handle = cx.windows().first().copied()?;
        handle
            .update(cx, |_, window, _| {
                session_recovery::capture_window_state(window)
            })
            .ok()
    }

    /// 关窗终局笔:1 Hz 快照恒 false,这一笔才 `clean_exit = true`。
    pub(super) fn save_on_window_close(&mut self, window: &Window, cx: &App) {
        let session =
            self.snapshot_session(Some(session_recovery::capture_window_state(window)), cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::WindowClose)
        {
            log::warn!("could not persist final terminal session: {error}");
        }
    }

    /// 最后 tab 关闭:空快照干净落盘(空会话不恢复)。
    pub(super) fn save_on_tabs_closed(&mut self, window: &Window, cx: &App) {
        let session =
            self.snapshot_session(Some(session_recovery::capture_window_state(window)), cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::TabsClosed)
        {
            log::warn!("could not persist empty workspace: {error}");
        }
    }

    /// 系统关窗(Alt+F4/OS 关闭请求)先过脏文件守卫,再落终局快照。
    ///
    /// 脏守卫返回 false = 本轮关窗被拦截(确认框在途);用户选「保存/不保存」
    /// 后由异步回调自行 `remove_window`,不再经本钩子。
    pub(super) fn install_window_close_save(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let handle = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let mut proceed = true;
            let _ = handle.update(cx, |workspace, cx| {
                proceed = workspace.guard_file_window_close(window, cx);
                if proceed {
                    workspace.save_on_window_close(window, cx);
                }
            });
            proceed
        });
    }

    /// AI hook 事件泵(照上游 agents.rs 形态):管道服务线程写进程内
    /// mpsc,后台定时器每 75ms 唤醒一次,前台线程排空并与 UI 状态合并。
    /// 单批上限 64;满批说明下游有积压,退 1ms 让出渲染。
    ///
    /// 前台只做状态更新:解析与阻塞读都在管道线程,本任务只消费已解析事件。
    pub(super) fn start_ai_hook_pump(
        &mut self,
        receiver: Option<mpsc::Receiver<AiHookEvent>>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(receiver) = receiver else {
            return;
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let mut backlog = false;
            loop {
                executor
                    .timer(Duration::from_millis(if backlog { 1 } else { 75 }))
                    .await;
                let mut events = Vec::new();
                let mut disconnected = false;
                while events.len() < 64 {
                    match receiver.try_recv() {
                        Ok(event) => events.push(event),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
                backlog = events.len() == 64;
                if events.is_empty() {
                    if disconnected {
                        return;
                    }
                    continue;
                }
                let Ok(()) = this.update(cx, |workspace, cx| {
                    // 严格 pane 路由 + 仲裁在宿主内完成;UI 消费(页签徽标/
                    // attention)归工作区/窗口域,本模块不产生视图态。
                    let focused = workspace.focused_pane();
                    let (_, accepted) = workspace.agent_hub.dispatch_lifecycle(events, focused);
                    // SessionEnd/TurnDone 先清理任务栏进度，再投递通知。
                    workspace.process_agent_lifecycle_events(&accepted, cx);
                    // 通知漏斗:TurnDone/NeedsAttention 两状态边
                    // → 通知漏斗(前台门控/节流/时长全在漏斗内)。
                    for event in &accepted {
                        if let Some((notification, pane_id)) =
                            crate::notify::from_accepted_hook(event, focused)
                        {
                            crate::notify::deliver_gpui(&notification, pane_id, cx);
                        }
                    }
                }) else {
                    return;
                };
                if disconnected {
                    return;
                }
            }
        })
        .detach();
    }

    /// 1 Hz 屏幕看门狗(照上游形态,收编进后台任务注册表):
    /// 逐 pane 取终端网格末尾样本喂仲裁器,给无 hook 的客户端补位、纠正
    /// 丢边的 hook 状态。
    ///
    /// 节奏刻意与输出解耦:每个 `Wakeup` 上跑规则会在输出洪流下放大成本,
    /// 而屏幕证据只需要拍帧级频率;`allows_screen()` 的 pane 由宿主内判定。
    ///
    /// 订阅随 workspace 生命周期存活;退订归零后下一拍循环自行退出。按需
    /// 下载任务同样由 workspace 订阅(点击路径只经注册表闸门触发)。
    pub(super) fn start_agent_screen_watchdog(&mut self, cx: &mut Context<'_, Self>) {
        for task_id in [
            background_tasks::AGENT_SCREEN_WATCHDOG,
            background_tasks::AUTO_UPDATE_DOWNLOAD,
        ] {
            self.background_tasks.push(
                background_tasks::subscribe(task_id)
                    .expect("background task must be registered in TASKS"),
            );
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                // 每轮重读节拍:applyConfig 直写原子量,下一轮即生效。
                let interval =
                    background_tasks::interval_sec(background_tasks::AGENT_SCREEN_WATCHDOG).max(1);
                executor.timer(Duration::from_secs(interval)).await;
                if !background_tasks::has_subscribers(background_tasks::AGENT_SCREEN_WATCHDOG) {
                    background_tasks::stop_loop(background_tasks::AGENT_SCREEN_WATCHDOG);
                    return;
                }
                let Ok(()) = this.update(cx, |workspace, cx| {
                    background_tasks::run_once(background_tasks::AGENT_SCREEN_WATCHDOG, || {
                        workspace.screen_watchdog_tick(cx);
                        Ok(())
                    });
                }) else {
                    return;
                };
            }
        })
        .detach();
    }

    /// 一拍看门狗:屏幕证据 + 托盘快照。失败策略/重入闸门由注册表统一兜住。
    pub(super) fn screen_watchdog_tick(&mut self, cx: &mut Context<'_, Self>) {
        for (pane_id, screen) in self.screen_samples(cx) {
            self.agent_hub.observe_screen(pane_id, &screen);
        }
        // 托盘快照搭同一拍:秒级延迟对托盘无感,
        // 内容相等时 `tray::update` 内部直接返回(天然去抖)。
        self.publish_tray_snapshot(cx);
    }

    /// 托盘 1 Hz 快照推送:语言与托盘开关取全局设置;
    /// 关闭态只在翻转时删图标(线程留驻),开启态每拍都投递挂载请求,
    /// 让 `NIM_ADD` 失败(explorer 未就绪)借该节拍自愈。
    pub(super) fn publish_tray_snapshot(&self, cx: &App) {
        let (language, enabled) = cx
            .try_global::<crate::app_shell::config::Settings>()
            .map(|settings| (settings.language, settings.tray))
            .unwrap_or((crate::i18n::UiLanguage::EnUs, true));
        crate::tray::set_enabled(enabled);
        crate::tray::update(self.tray_agents(cx), language);
    }

    /// 托盘与侧栏读取同一 pane 事实源(AgentHub):只收已识别为 AI CLI 的
    /// pane;身份查不到时不进托盘菜单。
    /// 单窗化后窗口目标塌缩为唯一主窗,pane 即完整路由信息。
    fn tray_agents(&self, cx: &App) -> Vec<crate::tray::TrayAgent> {
        let mut agents = Vec::new();
        for entry in &self.tabs {
            let super::WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
                continue;
            };
            for pane in panes {
                let Some(host) = self.agent_hub.host(pane.id) else {
                    continue;
                };
                let Some(program) = host.program() else {
                    continue;
                };
                if crate::ai_agents::AgentKind::parse(program).is_none() {
                    continue;
                }
                // 项目目录比 pane id 更适合作为托盘中的人工识别信息。
                let cwd = pane.view.read(cx).cwd.trim();
                let place = std::path::Path::new(cwd)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let label = if place.is_empty() {
                    program.to_owned()
                } else {
                    format!("{program} · {place}")
                };
                agents.push(crate::tray::TrayAgent {
                    pane: pane.id,
                    label,
                    needs_attention: host.status() == crate::ai_agents::AgentStatus::Blocked,
                });
            }
        }
        agents
    }

    /// 每个终端 pane 的屏幕观察窗口(最近至多 24 行,与屏幕规则注意力区
    /// 窗口同源)。无会话或零几何的 pane 跳过。
    fn screen_samples(&self, cx: &App) -> Vec<(slterm_split::PaneId, String)> {
        let mut samples = Vec::new();
        for entry in &self.tabs {
            let super::WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
                continue;
            };
            for pane in panes {
                if let Some(screen) = pane.view.read(cx).agent_screen_sample() {
                    samples.push((pane.id, screen));
                }
            }
        }
        samples
    }

    /// 壳事件泵：120ms 节奏排空通道，在前台线程派发。
    pub(super) fn start_shell_event_pump(
        &mut self,
        rx: mpsc::Receiver<SltermShellEvent>,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(120)).await;
                let mut events = Vec::new();
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
                if events.is_empty() {
                    continue;
                }
                // 窗口可能已关闭：此时丢弃已排队的壳事件。
                cx.update(|cx| {
                    let _ = window_handle.update(cx, |_root, window, cx| {
                        let _ = this.update(cx, |workspace, cx| {
                            for event in events {
                                workspace.handle_shell_event(event, window, cx);
                            }
                        });
                    });
                });
            }
        })
        .detach();
    }
}
