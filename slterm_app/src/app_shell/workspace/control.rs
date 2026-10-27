//! 工作区前台控制：壳事件、更新交互与 tab/pane 关闭。

use std::time::Duration;

use gpui::{App, Context, PromptLevel, Window};
use gpui_component::WindowExt as _;
use slterm_split::PaneId;

use super::super::SltermShellEvent;
use super::session_recovery;
use super::{SltermWorkspace, WorkspaceTab};
use crate::ai_agents::AgentStatus;
use crate::ai_hook::{AiHookEvent, AiHookKind, HookFailureKind};
use crate::i18n::{Message, UiLanguage};
use crate::session::Session;
use crate::update_check::{RELEASES_PAGE, UpdateAsset, UpdateCheckResult};
use crate::update_download::DownloadStatus;

/// 忙碌判据：正在工作或等待用户输入的 pane 不允许静默关闭。
fn busy_state(state: AgentStatus) -> bool {
    matches!(state, AgentStatus::Working | AgentStatus::Blocked)
}

impl SltermWorkspace {
    pub(super) fn pane_is_busy(&self, pane_id: PaneId) -> bool {
        self.agent_hub
            .host(pane_id)
            .is_some_and(|host| busy_state(host.status()))
    }

    /// 壳事件泵入口(前台线程,持有 Window)。
    pub(crate) fn handle_shell_event(
        &mut self,
        event: SltermShellEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            // toast 点击与托盘回窗共用 facade 的 pane
            // 级语义(window + 选中所在 tab + 窗内聚焦 pane)。
            SltermShellEvent::NotificationFocus(pane) => {
                crate::windowing::focus::focus(self, pane, window, cx);
                cx.notify();
            }
            // 权限确认动作按钮经窗口 facade 回窗。消费 =
            // 回窗聚焦 + 固定字段审计。按键注入语义取决于 confirmation
            // 标签生产方——当前 hook 是「转发后退出」的非阻塞桥,无法回写
            // 权限决定,故不猜测按键。
            SltermShellEvent::NotificationChoice { pane_id } => {
                crate::windowing::focus::focus(self, Some(pane_id), window, cx);
                crate::audit::record(crate::audit::AuditEvent::new(
                    "notification_choice",
                    "app_shell::workspace::control::handle_shell_event",
                    crate::audit::AuditResult::Acknowledged,
                ));
                cx.notify();
            }
            // 托盘「退出」:与关窗同一条退出链——先落终局
            // 快照,再移除唯一主窗;LastWindowClosed 随即结束进程。
            SltermShellEvent::TrayQuit => self.quit_from_tray(window, cx),
            // 更新提示:弹窗 + 安装授权两步;若用户开启
            // 后台下载,同时预取安装包(不授予安装权限)。
            SltermShellEvent::UpdateAvailable(result) => {
                self.update_available(result, window, cx);
            }
            // 自愈失败边 → 进程级通知。文本只由类别/agent 生成,合成
            // 命令(`slterm-hook ...`)不进入呈现层。
            SltermShellEvent::AiHookAlert(kind) => {
                let language = cx.global::<crate::app_shell::config::Settings>().language;
                let text = hook_alert_text(&kind, language);
                crate::notify::deliver_startup_summary(
                    &crate::notify::Notification::ProcessAlert { text },
                    cx,
                );
                cx.notify();
            }
            // 消费(右键「在此处打开」)归窗口/平台域。
            SltermShellEvent::OpenDirectories(_) => {}
            // 启动不可用与运行写失败共用同一脱敏进程提示。
            SltermShellEvent::ViewStateWriteFailed => {
                let language = cx.global::<crate::app_shell::config::Settings>().language;
                crate::notify::deliver_startup_summary(
                    &view_state_write_failure_notification(language),
                    cx,
                );
                cx.notify();
            }
        }
    }

    /// 更新可用弹窗与可选后台预取。
    ///
    /// 检查任务只负责发现;本方法在 UI 线程决定按钮集并弹第一层提示。
    /// 「下载并安装」后才进入安装授权(第二层);授权经 handoff 两阶段
    /// 文件机提交,helper 准备/等待全程在后台。
    fn update_available(
        &mut self,
        result: UpdateCheckResult,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(plan) = update_prompt_plan(&result) else {
            log::debug!("update-check: ignoring a result without an available update");
            return;
        };
        // 单实例闸门:弹窗/下载/安装授权在途时重复事件直接丢弃。
        if self.update_prompt_open {
            log::debug!(
                "update-check: prompt already open; dropped v{}",
                result.latest
            );
            return;
        }
        // 安装失败「未见过」的重提示不受 prompted 抑制(deliver_cached 已
        // 判 Unseen);其余同版本已提示/已跳过/延迟中不得重复弹窗。
        let failure_unseen = result.asset.as_ref().is_some_and(|asset| {
            matches!(
                crate::update_download::status(asset),
                DownloadStatus::InstallFailed(_)
            )
        });
        if !failure_unseen && !crate::update_check::should_prompt(&result.latest) {
            log::debug!("update-check: prompt suppressed for v{}", result.latest);
            return;
        }
        // 打开即记 prompted(best-effort;失败只告警,不阻断提示)。
        if let Err(error) = crate::update_check::mark_prompted(&result.latest) {
            log::warn!(
                "update-check: could not record prompt for v{}: {error}",
                result.latest
            );
        }
        self.update_prompt_open = true;
        self.auto_prefetch_update(&result, cx);

        let language = update_language(cx);
        let title = language.format(
            Message::UpdateTitle,
            &[("app", crate::brand::NAME), ("version", &result.latest)],
        );
        let args = [
            ("app", crate::brand::NAME),
            ("version", &result.latest),
            ("current", &result.current),
        ];
        let body = if result.manual_reinstall {
            // 早于首个 Inno 安装基的历史安装不能被自动覆盖。
            language.format(Message::UpdateBodyManualReinstall, &args)
        } else {
            match plan {
                UpdatePromptPlan::Installable => {
                    language.format(Message::UpdateBodyAvailable, &args)
                }
                UpdatePromptPlan::ManualDownload => {
                    language.format(Message::UpdateBodyManualDownload, &args)
                }
            }
        };
        let buttons: [&str; 3] = match plan {
            UpdatePromptPlan::Installable => [
                language.text(Message::UpdateDownloadAndInstall),
                language.text(Message::UpdateRemindLater),
                language.text(Message::UpdateSkipVersion),
            ],
            UpdatePromptPlan::ManualDownload => [
                language.text(Message::UpdateOpenDownloadPage),
                language.text(Message::UpdateRemindLater),
                language.text(Message::UpdateSkipVersion),
            ],
        };
        log::info!(
            "update-check: v{} available (current v{})",
            result.latest,
            result.current
        );
        // 窗口可能处于静默/最小化态:系统 toast 与窗内弹窗共用同一文案,
        // 保证后台启动也能收到更新信号。
        crate::platform::notifications::show(&title, &body);

        let latest = result.latest.clone();
        let asset = result.asset.clone();
        let answer = window.prompt(PromptLevel::Info, &title, Some(&body), &buttons, cx);
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let choice = answer.await.ok();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.finish_update_prompt(choice, plan, asset, latest, window, cx);
                });
            });
        })
        .detach();
    }

    /// 第一层提示的答案分派:「稍后提醒」「跳过此版本」只写提示状态;
    /// 自动安装分支进入后台下载,其余走下载页。
    fn finish_update_prompt(
        &mut self,
        choice: Option<usize>,
        plan: UpdatePromptPlan,
        asset: Option<UpdateAsset>,
        latest: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match choice {
            Some(0) => match (plan, asset) {
                (UpdatePromptPlan::Installable, Some(asset)) => {
                    self.start_update_download(asset, window, cx);
                }
                _ => {
                    self.update_prompt_open = false;
                    cx.open_url(RELEASES_PAGE);
                    cx.notify();
                }
            },
            Some(1) => {
                // 稍后提醒 = now + 3 天;失败只告警。
                if let Err(error) = crate::update_check::remind_later(&latest) {
                    log::warn!(
                        "update-check: could not record remind-later for v{latest}: {error}"
                    );
                }
                self.update_prompt_open = false;
                cx.notify();
            }
            Some(2) => {
                // 跳过该版本,落盘后不再提示同版本。
                if let Err(error) = crate::update_check::skip_version(&latest) {
                    log::warn!("update-check: could not record skip for v{latest}: {error}");
                }
                self.update_prompt_open = false;
                cx.notify();
            }
            _ => {
                // 对话框被系统取消/关闭:只清单实例标志。
                self.update_prompt_open = false;
                cx.notify();
            }
        }
    }

    /// 「下载并安装」:后台执行 begin/run,完成后回前台读取终态。
    fn start_update_download(
        &mut self,
        asset: UpdateAsset,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let task = Self::update_download_task(&asset, cx);
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            // 后台任务结束(终态已写入会话)后,前台再读一次状态。
            let outcome = task.await.map(|_| crate::update_download::status(&asset));
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.finish_update_download(asset, outcome, window, cx);
                });
            });
        })
        .detach();
    }

    /// 后台下载单点(自动预取与用户点击共用)。
    ///
    /// `begin`/`run` 全程在 background executor;同资产已有下载/就绪时
    /// `begin` 返回 None,任务等待既有会话终态,不重复起下载任务。
    fn update_download_task(
        asset: &UpdateAsset,
        cx: &App,
    ) -> gpui::Task<Result<DownloadStatus, String>> {
        let executor = cx.background_executor().clone();
        let poller = executor.clone();
        let asset = asset.clone();
        // 注册表闸门覆盖整轮;重入(已有预取)时仍等待
        // 既有会话,不叠加下载。
        let guard = crate::background_tasks::begin(crate::background_tasks::AUTO_UPDATE_DOWNLOAD);
        executor.spawn(async move {
            let _guard = guard;
            let job = crate::update_download::begin(&asset)?;
            if let Some(job) = job {
                crate::update_download::run(job);
            }
            loop {
                match crate::update_download::status(&asset) {
                    DownloadStatus::Downloading { .. } => {
                        poller.timer(UPDATE_DOWNLOAD_POLL).await;
                    }
                    status => return Ok(status),
                }
            }
        })
    }

    /// 下载终态消费:Ready 打开安装授权;失败/中断只给窗口错误通知。
    fn finish_update_download(
        &mut self,
        asset: UpdateAsset,
        outcome: Result<DownloadStatus, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = update_language(cx);
        let error = match outcome {
            Ok(DownloadStatus::Ready { .. }) => {
                self.open_install_authorization_prompt(asset, window, cx);
                return;
            }
            Ok(DownloadStatus::Failed(error)) | Ok(DownloadStatus::InstallFailed(error)) => error,
            Ok(_) => language.text(Message::UpdateDownloadIncomplete).to_owned(),
            Err(error) => error,
        };
        self.update_prompt_open = false;
        let detail = language.format(Message::UpdateDownloadFailedDetail, &[("error", &error)]);
        window.push_notification(
            gpui_component::notification::Notification::error(
                language.text(Message::UpdateDownloadFailedTitle),
            )
            .message(detail),
            cx,
        );
        cx.notify();
    }

    /// 安装授权(第二层确认):用户选择后才写 commit.json。
    fn open_install_authorization_prompt(
        &mut self,
        asset: UpdateAsset,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = update_language(cx);
        let title = language.format(
            Message::UpdateInstallTitle,
            &[("app", crate::brand::NAME), ("version", &asset.version)],
        );
        let body = language.text(Message::UpdateInstallBody).to_owned();
        let buttons = [
            language.text(Message::UpdateInstallNow),
            language.text(Message::UpdateInstallNextLaunch),
            language.text(Message::UpdateInstallCancel),
        ];
        let answer = window.prompt(PromptLevel::Info, &title, Some(&body), &buttons, cx);
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let choice = answer.await.ok();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.finish_install_prompt(choice, asset, window, cx);
                });
            });
        })
        .detach();
    }

    /// 安装授权答案分派。
    fn finish_install_prompt(
        &mut self,
        choice: Option<usize>,
        asset: UpdateAsset,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = update_language(cx);
        match choice {
            Some(0) => self.start_install_now(asset, window, cx),
            Some(1) => {
                // 预约是小体积原子写,允许同步;helper 不在此时启动。
                let outcome = crate::update_download::handoff::schedule(&asset);
                self.update_prompt_open = false;
                match outcome {
                    Ok(()) => {
                        let detail = language.format(
                            Message::UpdateScheduledDetail,
                            &[("app", crate::brand::NAME), ("version", &asset.version)],
                        );
                        window.push_notification(
                            gpui_component::notification::Notification::success(
                                language.text(Message::UpdateScheduledTitle),
                            )
                            .message(detail),
                            cx,
                        );
                    }
                    Err(error) => {
                        let detail = language
                            .format(Message::UpdateScheduleFailedDetail, &[("error", &error)]);
                        window.push_notification(
                            gpui_component::notification::Notification::error(
                                language.text(Message::UpdateScheduleFailedTitle),
                            )
                            .message(detail),
                            cx,
                        );
                    }
                }
                cx.notify();
            }
            _ => {
                self.update_prompt_open = false;
                cx.notify();
            }
        }
    }

    /// 「立即安装并退出」:前台先拍工作区快照;prepare(等待 helper 就绪)
    /// 与 commit 全在后台;成功才回前台终局落盘并关窗。
    fn start_install_now(
        &mut self,
        asset: UpdateAsset,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 快照必须在窗口仍可用时取;helper 的准备可能耗时数十秒。
        let session =
            self.snapshot_session(Some(session_recovery::capture_window_state(window)), cx);
        let sessions = vec![session];
        #[cfg(all(test, feature = "gpui-test-support"))]
        let runner = self.update_install_runner.clone();
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let runner: Option<UpdateInstallRunner> = None;
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let work_asset = asset.clone();
            let outcome = executor
                .spawn(async move { install_update(&work_asset, &sessions, runner.as_ref()) })
                .await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.finish_update_install(asset, outcome, window, cx);
                });
            });
        })
        .detach();
    }

    /// 安装结果回到前台:成功 = 终局快照 + 退出;失败 = 错误通知且不关窗。
    fn finish_update_install(
        &mut self,
        asset: UpdateAsset,
        outcome: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = update_language(cx);
        match outcome {
            Ok(()) => {
                log::info!(
                    "update-install: v{} authorized; exiting for the helper",
                    asset.version
                );
                self.update_prompt_open = false;
                // 与确认关窗同语义:防 remove_window 重入脏文件确认。
                self.window_close_confirmed = true;
                self.save_on_window_close(window, cx);
                #[cfg(all(test, feature = "gpui-test-support"))]
                {
                    // 虚拟窗口平台不支持真实 remove_window;记录退出请求
                    // (与托盘退出同一处置,断言落在请求端)。
                    self.update_install_exit_requested = true;
                    cx.notify();
                }
                #[cfg(not(all(test, feature = "gpui-test-support")))]
                {
                    window.remove_window();
                }
            }
            Err(error) => {
                self.update_prompt_open = false;
                let detail =
                    language.format(Message::UpdateInstallFailedDetail, &[("error", &error)]);
                window.push_notification(
                    gpui_component::notification::Notification::error(
                        language.text(Message::UpdateInstallFailedTitle),
                    )
                    .message(detail),
                    cx,
                );
                cx.notify();
            }
        }
    }

    /// 自动预取(设置开启且非手动重装):与用户点击共用下载单点。
    fn auto_prefetch_update(&mut self, result: &UpdateCheckResult, cx: &mut Context<'_, Self>) {
        if result.manual_reinstall || !update_auto_download(cx) {
            return;
        }
        let Some(asset) = result.asset.as_ref() else {
            return;
        };
        // 无摘要的安装包不可验证,预取也拒绝执行;下载器还会二次校验。
        if asset.digest.is_none() {
            return;
        }
        let notice = asset.clone();
        let task = Self::update_download_task(asset, cx);
        cx.spawn(async move |_this, _cx| match task.await {
            Ok(DownloadStatus::Ready { .. }) => {
                log::info!("update-download: v{} ready", notice.version);
            }
            Ok(DownloadStatus::Failed(error)) | Ok(DownloadStatus::InstallFailed(error)) => {
                log::warn!("update-download: v{} failed: {error}", notice.version);
            }
            Ok(_) => {}
            Err(error) => {
                log::debug!("update-download: v{} refused: {error}", notice.version);
            }
        })
        .detach();
    }

    /// 托盘退出:与标题栏关闭钮同语义(自绘入口绕过 OS 关闭请求,终局
    /// 快照在这里补)。数据丢失风险高于普通关窗,必须先过整窗脏
    /// 守卫;确认框在途/保存在途 = 本轮拦截,异步回调放行后自行关窗。
    /// 测试构建只记录已放行的请求,虚拟窗口平台不支持真实移除。
    fn quit_from_tray(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.guard_file_window_close(window, cx) {
            return;
        }
        #[cfg(all(test, feature = "gpui-test-support"))]
        {
            self.tray_quit_requested = true;
            cx.notify();
        }
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        {
            self.save_on_window_close(window, cx);
            window.remove_window();
        }
    }

    /// pane 级回窗的 workspace 落点(窗口 facade 委托):找到所在
    /// tab,切换为活跃并聚焦该 pane。
    pub(crate) fn focus_notification_pane(
        &mut self,
        pane_id: PaneId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(tab_ix) = self.tabs.iter().position(|entry| match &entry.tab {
            WorkspaceTab::Terminal { tree, .. } => tree.contains(pane_id),
            _ => false,
        }) else {
            // pane 已关闭:只保留窗口前置,不猜目标。
            return;
        };
        if tab_ix != self.active {
            self.activate_tab(tab_ix, window, cx);
        }
        self.focus_pane(tab_ix, pane_id, window, cx);
    }

    /// 关闭任意 tab(活跃 tab 快捷键与文件关闭守卫共用)。
    ///
    /// 脏文件 tab 先走确认路径(documents.rs);确认/保存在途期间关闭被
    /// 拦截,避免草稿被静默丢弃。
    pub(super) fn close_tab_at(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.guard_file_tab_close(index, window, cx) {
            return;
        }
        self.finish_close_tab(index, window, cx);
    }

    /// 确认后的实际摘除:逐 pane 显式 shutdown,不留孤儿 PTY;最后一个
    /// tab = 关窗。脏确认流程只在本方法放行后到达。
    pub(super) fn finish_close_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(entry) = self.tabs.get(index) else {
            return;
        };
        if let WorkspaceTab::Terminal { panes, .. } = &entry.tab {
            let views: Vec<_> = panes.iter().map(|pane| pane.view.clone()).collect();
            let ids: Vec<_> = panes.iter().map(|pane| pane.id).collect();
            for view in &views {
                view.read(cx).shutdown();
            }
            for id in ids {
                self.forget_pane(id, cx);
            }
        }
        self.remove_tab_entry(index);
        if self.tabs.is_empty() {
            // 空会话干净落盘:下次启动不恢复,也不把这次关标签误判成崩溃。
            self.save_on_tabs_closed(window, cx);
            window.remove_window();
            return;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// hook 批次后的任务栏清理(前台线程)。
    pub(crate) fn process_agent_lifecycle_events(
        &mut self,
        events: &[AiHookEvent],
        cx: &mut Context<'_, Self>,
    ) {
        for event in events {
            let Some(pane) = event.pane else {
                continue;
            };
            if matches!(event.kind, AiHookKind::SessionEnd | AiHookKind::TurnDone) {
                self.clear_taskbar_progress(PaneId(pane), cx);
            }
        }
    }
}

/// 下载终态轮询节拍:仅在前台之外的 background executor 上等待。
const UPDATE_DOWNLOAD_POLL: Duration = Duration::from_millis(200);

/// 更新弹窗按钮集的纯决策(表驱动测试与 UI 共用)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdatePromptPlan {
    /// 非手动重装 + 可更新 + 资产带 digest:自动下载并安装。
    Installable,
    /// 手动重装或无可验证资产:只提供下载页。
    ManualDownload,
}

/// 按检查结果决定弹窗形态;None = 该结果不应弹窗。
fn update_prompt_plan(result: &UpdateCheckResult) -> Option<UpdatePromptPlan> {
    if !result.update_available {
        return None;
    }
    let installable = !result.manual_reinstall
        && result
            .asset
            .as_ref()
            .is_some_and(|asset| asset.digest.is_some());
    Some(if installable {
        UpdatePromptPlan::Installable
    } else {
        UpdatePromptPlan::ManualDownload
    })
}

/// 壳内 UI 语言快照(System 档的 locale 协商已在设置装配时完成)。
fn update_language(cx: &App) -> UiLanguage {
    cx.try_global::<crate::app_shell::config::Settings>()
        .map(|settings| settings.language)
        .unwrap_or(crate::i18n::UiLanguage::EnUs)
}

/// 「后台自动下载更新」设置位(壳内快照;默认关)。
fn update_auto_download(cx: &App) -> bool {
    cx.try_global::<crate::app_shell::config::Settings>()
        .is_some_and(|settings| settings.auto_download_updates)
}

/// 安装执行注入点:生产传 None(走 handoff 两阶段文件机);UI 用例注入
/// 替身,避免真实 helper/安装目录。分支在非测试构建被 cfg 编译剔除。
pub(crate) type UpdateInstallRunner =
    std::sync::Arc<dyn Fn(&UpdateAsset, &[Session]) -> Result<(), String> + Send + Sync>;

/// 后台安装执行体:`prepare` 会 spawn helper 并等待 ready,严禁前台调用。
fn install_update(
    asset: &UpdateAsset,
    sessions: &[Session],
    runner: Option<&UpdateInstallRunner>,
) -> Result<(), String> {
    #[cfg(all(test, feature = "gpui-test-support"))]
    if let Some(runner) = runner {
        return runner(asset, sessions);
    }
    #[cfg(not(all(test, feature = "gpui-test-support")))]
    let _ = runner;
    let mut prepared = crate::update_download::handoff::prepare(asset)?;
    prepared.commit(sessions)
}

/// 失败提示只消费类别/agent，永不拼接技术化合成命令。
fn hook_alert_text(kind: &HookFailureKind, language: UiLanguage) -> String {
    match kind {
        HookFailureKind::Install { agent } => language.format(
            Message::NotifyHookInstallFailed,
            &[("agent", &agent_display_name(agent))],
        ),
        HookFailureKind::Settings => language
            .text(Message::NotifyHookSettingsUnreadable)
            .to_owned(),
    }
}

/// CLI slug → 展示名;未知 slug 原样返回(不猜身份)。
fn agent_display_name(slug: &str) -> String {
    crate::ai_agents::AgentKind::parse(slug)
        .map(|kind| kind.display_name().to_owned())
        .unwrap_or_else(|| slug.to_owned())
}

/// ViewState 写失败只给泛化提示，不暴露键、路径、SQL 或状态正文。
fn view_state_write_failure_notification(language: UiLanguage) -> crate::notify::Notification {
    crate::notify::Notification::ProcessAlert {
        text: language
            .text(Message::NotifyViewStateWriteFailed)
            .to_owned(),
    }
}

#[cfg(test)]
mod shell_alert_tests {
    use super::view_state_write_failure_notification;
    use crate::i18n::{Message, UiLanguage};

    /// 启动不可用与运行写失败共用不含内部细节的本地化进程提示。
    #[test]
    fn view_state_write_failure_alert_is_localized_and_generic() {
        for language in [UiLanguage::EnUs, UiLanguage::ZhCn] {
            let notification = view_state_write_failure_notification(language);
            let (title, body) = notification.raw_toast_text();
            assert_eq!(title, crate::brand::NAME);
            assert_eq!(body, language.text(Message::NotifyViewStateWriteFailed));
        }
    }
}

#[cfg(test)]
mod update_prompt_tests {
    use super::{UpdatePromptPlan, update_prompt_plan};
    use crate::update_check::{UpdateAsset, UpdateCheckResult};

    fn installer_asset(digest: Option<&str>) -> UpdateAsset {
        let version = "0.4.0";
        let name = format!("slTerminal-v{version}-windows-x64-setup.exe");
        UpdateAsset {
            version: version.to_owned(),
            name: name.clone(),
            url: format!(
                "https://github.com/LiuShenLan/slTerminal/releases/download/v{version}/{name}"
            ),
            size: Some(42),
            digest: digest.map(str::to_owned),
        }
    }

    fn check_result(
        update_available: bool,
        manual_reinstall: bool,
        asset: Option<UpdateAsset>,
    ) -> UpdateCheckResult {
        UpdateCheckResult {
            current: "0.3.0".to_owned(),
            latest: "0.4.0".to_owned(),
            update_available,
            manual_reinstall,
            asset,
        }
    }

    /// 按钮集决策表:手动重装 / 无资产 / 无摘要 → 下载页;齐备 → 可安装。
    #[test]
    fn update_prompt_plan_covers_manual_reinstall_missing_asset_and_digest() {
        let digest = "a".repeat(64);
        for (label, result, expected) in [
            (
                "manual reinstall",
                check_result(true, true, Some(installer_asset(Some(&digest)))),
                UpdatePromptPlan::ManualDownload,
            ),
            (
                "missing asset",
                check_result(true, false, None),
                UpdatePromptPlan::ManualDownload,
            ),
            (
                "missing digest",
                check_result(true, false, Some(installer_asset(None))),
                UpdatePromptPlan::ManualDownload,
            ),
            (
                "installable",
                check_result(true, false, Some(installer_asset(Some(&digest)))),
                UpdatePromptPlan::Installable,
            ),
        ] {
            assert_eq!(update_prompt_plan(&result), Some(expected), "{label}");
        }
        assert_eq!(
            update_prompt_plan(&check_result(
                false,
                false,
                Some(installer_asset(Some(&digest)))
            )),
            None,
            "非「有更新」结果不得弹窗"
        );
    }
}

#[cfg(test)]
mod hook_alert_tests {
    use super::*;

    /// 呈现文本带 agent 展示名,且不含任何合成命令/内部键。
    #[test]
    fn hook_alert_text_is_human_readable_and_command_free() {
        for &language in UiLanguage::ALL {
            let install = hook_alert_text(
                &HookFailureKind::Install {
                    agent: "claude".to_owned(),
                },
                language,
            );
            assert!(install.contains("Claude Code"), "{install}");
            let settings = hook_alert_text(&HookFailureKind::Settings, language);
            assert!(!settings.is_empty());
            for text in [install, settings] {
                assert!(
                    !text.contains("slterm-hook") && !text.contains("slterm_ai_hooks"),
                    "呈现层不得出现合成命令或内部键: {text}"
                );
            }
        }
    }
}

#[cfg(test)]
mod busy_state_tests {
    use super::*;

    #[test]
    fn busy_state_consumes_agent_status_directly() {
        for (status, expected) in [
            (crate::ai_agents::AgentStatus::Working, true),
            (crate::ai_agents::AgentStatus::Blocked, true),
            (crate::ai_agents::AgentStatus::Idle, false),
            (crate::ai_agents::AgentStatus::Done, false),
            (crate::ai_agents::AgentStatus::Unknown, false),
        ] {
            assert_eq!(busy_state(status), expected, "{status:?}");
        }
    }
}
