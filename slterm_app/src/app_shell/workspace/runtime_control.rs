//! Runtime 写命令在 UI owner 上的执行器。
//!
//! 全序:transport 线程 `RuntimeDispatch` → 壳事件队列 → 前台泵 →
//! **本模块**把命令映射到 Workspace/TerminalView 既有入口 → `respond`。
//! 回包形状按 M7.4 缝合点:`{action: {window_id, pane_id}(或 action.agent),
//! snapshot: RuntimeSnapshot}`;prompt/paste 成功回包携带**提交后**快照与
//! `state_change_seq`(CLI `--wait` 基线链依赖)。
//!
//! 生命周期:AgentStart/AgentFork 成功后以 worktree provenance 注册
//! managed agent;SessionEnd/pane 关闭关闭注册项;TurnDone 完成委派并把
//! 回传提示写回 origin pane。

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Context, PromptLevel, Window};
use gpui_component::WindowExt as _;
use serde_json::{Value, json};
use slterm_split::{PaneId, SplitDirection, TabId};

use crate::git;

use super::super::SltermShellEvent;
use super::runtime_projection;
use super::session_recovery;
use super::{SltermWorkspace, WorkspaceTab};
use crate::ai_assistant::HookFailureKind;
use crate::ai_hook::{AiHookEvent, AiHookKind};
use crate::gpui_terminal::view::TerminalView;
use crate::i18n::{Message, UiLanguage};
use crate::runtime_api::{
    ApiError, RuntimeCommand, RuntimeDispatch, RuntimeSplitDirection, RuntimeTaskState,
    SINGLE_WINDOW_ID, capture_process_tree,
};
use crate::runtime_exec;
use crate::session::Session;
use crate::update_check::{RELEASES_PAGE, UpdateAsset, UpdateCheckResult};
use crate::update_download::DownloadStatus;

/// 忙碌判据:等待用户/正在运行/需要 attention 的 pane 不允许静默关闭。
fn busy_state(state: RuntimeTaskState) -> bool {
    matches!(
        state,
        RuntimeTaskState::Running | RuntimeTaskState::WaitingInput | RuntimeTaskState::Attention
    )
}

fn target_not_found(what: &str) -> ApiError {
    ApiError::new("target_not_found", format!("{what} does not exist"))
}

fn confirmation_required(busy: &[u64]) -> ApiError {
    ApiError::new(
        "confirmation_required",
        "the target contains panes that are running or waiting for input",
    )
    .details(json!({ "busy_pane_ids": busy }))
}

impl SltermWorkspace {
    /// 壳事件泵入口(前台线程,持有 Window)。
    pub(crate) fn handle_shell_event(
        &mut self,
        event: SltermShellEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            // 单窗语义:legacy ATTACH = 聚焦现有唯一窗(最小化则还原),
            // 不建第二窗;窗口成分经窗口 facade 单点实现。
            SltermShellEvent::MuxAttach => {
                crate::windowing::focus::focus(self, None, window, cx);
                cx.notify();
            }
            SltermShellEvent::RuntimeControl(dispatch) => {
                self.execute_runtime_dispatch(dispatch, window, cx);
            }
            // toast 点击与托盘回窗共用 facade 的 pane
            // 级语义(window + 选中所在 tab + 窗内聚焦 pane)。
            SltermShellEvent::NotificationFocus(pane) => {
                crate::windowing::focus::focus(self, pane, window, cx);
                cx.notify();
            }
            // 权限确认动作按钮是四路回窗之一(窗口 facade)。B43:消费 =
            // 回窗聚焦 + 审计留痕(选择、请求 id、pane)。按键注入语义取决于
            // confirmation 标签生产方——当前 hook 是「转发后退出」的非阻塞
            // 桥,无法回写权限决定,故不猜测按键。
            SltermShellEvent::NotificationChoice {
                pane_id,
                request_id,
                choice,
            } => {
                crate::windowing::focus::focus(self, Some(pane_id), window, cx);
                tracing::info!(
                    target: crate::audit::TARGET,
                    operation = "notification_choice",
                    pane = pane_id.0,
                    request_id = %request_id,
                    choice = %choice,
                    result = "acknowledged",
                    "通知动作按钮回执已消费"
                );
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
            // B38:自愈失败边 → 进程级通知。文本只由类别/agent 生成,合成
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
        }
    }

    /// 更新可用弹窗与可选后台预取(R7 C27/B52)。
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
        // B52:打开即记 prompted(best-effort;失败只告警,不阻断提示)。
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
            // D12-4:早于首个 Inno 安装基的历史安装不能被自动覆盖。
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
                // B52:稍后提醒 = now + 3 天;失败只告警。
                if let Err(error) = crate::update_check::remind_later(&latest) {
                    log::warn!(
                        "update-check: could not record remind-later for v{latest}: {error}"
                    );
                }
                self.update_prompt_open = false;
                cx.notify();
            }
            Some(2) => {
                // B52:跳过该版本,落盘后不再提示同版本。
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
    /// 快照在这里补)。B47:数据丢失风险高于普通关窗,必须先过整窗脏
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

    /// 执行一条写命令并回执;Exec 是异步执行器,自行 respond。
    fn execute_runtime_dispatch(
        &mut self,
        dispatch: Arc<RuntimeDispatch>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let command = dispatch.command.clone();
        if let RuntimeCommand::Exec {
            window_id, pane_id, ..
        } = &command
        {
            let started = self.start_pane_exec(dispatch.clone(), *window_id, *pane_id, cx);
            if let Err(error) = started {
                dispatch.respond(Err(error));
            }
            return;
        }
        let result = self.execute_runtime_command(&command, window, cx);
        dispatch.respond(result);
    }

    /// `pane.exec`:独立非 TTY 子进程由 runtime_exec 线程执行并回投。
    fn start_pane_exec(
        &self,
        dispatch: Arc<RuntimeDispatch>,
        window_id: Option<u64>,
        pane_id: u64,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), ApiError> {
        ensure_window(window_id)?;
        let (_, _, view) = self.pane_entry(pane_id)?;
        let (cwd, context) = {
            let view = view.read(cx);
            (
                view.cwd.clone(),
                view.runtime_exec_context().ok_or_else(|| {
                    ApiError::new(
                        "exec_unavailable",
                        "the pane has no frozen PTY environment to reuse",
                    )
                })?,
            )
        };
        runtime_exec::spawn(dispatch, context, cwd);
        Ok(())
    }

    fn execute_runtime_command(
        &mut self,
        command: &RuntimeCommand,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        match command {
            RuntimeCommand::Snapshot => {
                let snapshot = self.publish_runtime_snapshot(cx);
                serde_json::to_value(snapshot)
                    .map_err(|error| ApiError::new("invalid_runtime_response", error.to_string()))
            }
            RuntimeCommand::CloseWindow { window_id } => {
                ensure_window(*window_id)?;
                let busy = self.busy_panes(self.tabs.iter().flat_map(tab_pane_ids));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                let result = self.action_result(json!({ "window_id": SINGLE_WINDOW_ID }), cx)?;
                window.remove_window();
                Ok(result)
            }
            RuntimeCommand::Focus { window_id, pane_id } => {
                ensure_window(*window_id)?;
                if let Some(pane_id) = pane_id {
                    // 目标校验保留协议语义:不存在的 pane 仍是 target_not_found。
                    self.pane_entry(*pane_id)?;
                }
                crate::windowing::focus::focus(self, pane_id.map(PaneId), window, cx);
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::NewTab {
                window_id,
                cwd,
                shell_id,
            } => {
                ensure_window(*window_id)?;
                let shell = resolve_shell(shell_id.as_deref())?;
                self.add_terminal_tab(cwd.clone(), shell, false, window, cx);
                let tab_id = self.tabs.last().map(|entry| entry.id.0);
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": self.focused_pane().map(|pane| pane.0),
                        "tab_id": tab_id
                    }),
                    cx,
                )
            }
            RuntimeCommand::CloseTab {
                window_id,
                tab_index,
                tab_id,
                force,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                let busy = self.busy_panes(self.tab_pane_ids(index));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                // B12:脏文件默认返回明确错误码,不再「先回成功、异步弹确认」;
                // 调用方须显式 force=true 才丢弃草稿。
                if !*force && self.tab_has_dirty_document(index, cx) {
                    return Err(ApiError::new(
                        "dirty_document",
                        "tab has unsaved changes; pass force=true to discard them",
                    ));
                }
                let closed = self.tabs[index].id.0;
                // 关最后一个 tab = 关窗;先取快照再摘除,避免在已移除窗口
                // 的上下文里继续投影。
                let result = self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "tab_id": closed, "tab_index": index }),
                    cx,
                )?;
                if *force {
                    // 已显式确认丢弃草稿:跳过交互式脏守卫。
                    self.finish_close_tab(index, window, cx);
                } else {
                    self.close_tab_at(index, window, cx);
                }
                Ok(result)
            }
            RuntimeCommand::RenameTab {
                window_id,
                tab_index,
                tab_id,
                name,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                if let Some(entry) = self.tabs.get_mut(index) {
                    entry.meta.custom_name = (!name.is_empty()).then(|| name.clone());
                }
                self.sync_window_title(window, cx);
                cx.notify();
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "tab_id": self.tabs[index].id.0 }),
                    cx,
                )
            }
            RuntimeCommand::MoveTab {
                window_id,
                tab_index,
                tab_id,
                to_index,
            } => {
                ensure_window(*window_id)?;
                let index = self.tab_index_for(*tab_id, *tab_index)?;
                if *to_index >= self.tabs.len() {
                    return Err(ApiError::invalid_params(format!(
                        "to_index {} is outside the tab list",
                        to_index
                    )));
                }
                self.move_tab_index(index, *to_index);
                self.sync_window_title(window, cx);
                cx.notify();
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "tab_id": self.tabs[*to_index].id.0,
                        "tab_index": to_index
                    }),
                    cx,
                )
            }
            RuntimeCommand::Split {
                window_id,
                pane_id,
                direction,
            } => {
                ensure_window(*window_id)?;
                if let Some(pane_id) = pane_id {
                    let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                    if tab_ix != self.active {
                        self.activate_tab(tab_ix, window, cx);
                    }
                    self.focus_pane(tab_ix, PaneId(*pane_id), window, cx);
                }
                let direction = match direction {
                    RuntimeSplitDirection::LeftRight => SplitDirection::LeftRight,
                    RuntimeSplitDirection::TopBottom => SplitDirection::TopBottom,
                };
                let new_pane = self.split_focused(direction, window, cx).ok_or_else(|| {
                    ApiError::new("invalid_state", "the focused pane cannot be split")
                })?;
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": new_pane.0 }),
                    cx,
                )
            }
            RuntimeCommand::ClosePane { window_id, pane_id } => {
                ensure_window(*window_id)?;
                self.pane_entry(*pane_id)?;
                let busy = self.busy_panes(std::iter::once(PaneId(*pane_id)));
                if !busy.is_empty() {
                    return Err(confirmation_required(&busy));
                }
                // 关最后一个 pane = 关窗:同样先取快照再摘除。
                let result = self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )?;
                self.close_pane(PaneId(*pane_id), window, cx);
                Ok(result)
            }
            RuntimeCommand::ZoomPane {
                window_id,
                pane_id,
                zoomed,
            } => {
                ensure_window(*window_id)?;
                let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                if tab_ix != self.active {
                    self.activate_tab(tab_ix, window, cx);
                }
                self.focus_pane(tab_ix, PaneId(*pane_id), window, cx);
                let mut effective = *zoomed;
                let changed = match self.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab) {
                    Some(WorkspaceTab::Terminal {
                        panes,
                        zoomed: current,
                        ..
                    }) => {
                        let next = *zoomed && panes.len() > 1;
                        effective = next;
                        let changed = *current != next;
                        *current = next;
                        changed
                    }
                    _ => false,
                };
                if changed {
                    self.mark_structural_resize(tab_ix, cx);
                    cx.notify();
                }
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "zoomed": effective
                    }),
                    cx,
                )
            }
            RuntimeCommand::ResizePane {
                window_id,
                pane_id,
                ratio,
            } => {
                ensure_window(*window_id)?;
                let (tab_ix, _, _) = self.pane_entry(*pane_id)?;
                let resized = match self.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab) {
                    Some(WorkspaceTab::Terminal { tree, .. }) => {
                        tree.set_leaf_parent_ratio(PaneId(*pane_id), *ratio)
                    }
                    _ => false,
                };
                if !resized {
                    return Err(ApiError::new(
                        "invalid_state",
                        "the pane has no direct parent split to resize",
                    ));
                }
                self.mark_structural_resize(tab_ix, cx);
                cx.notify();
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id, "ratio": ratio }),
                    cx,
                )
            }
            RuntimeCommand::Prompt {
                window_id,
                pane_id,
                text,
                submit,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| view.runtime_prompt(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(*pane_id), text);
                }
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::Paste {
                window_id,
                pane_id,
                text,
                submit,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| view.runtime_paste(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(*pane_id), text);
                }
                self.action_result(
                    json!({ "window_id": SINGLE_WINDOW_ID, "pane_id": pane_id }),
                    cx,
                )
            }
            RuntimeCommand::ReadPane {
                window_id,
                pane_id,
                lines,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                let task_state = self.pane_task_state(PaneId(*pane_id));
                let read =
                    view.read(cx)
                        .runtime_read(SINGLE_WINDOW_ID, *pane_id, *lines, task_state)?;
                self.action_result(json!(read), cx)
            }
            RuntimeCommand::Procs { window_id, pane_id } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                let root_pid = view.read(cx).runtime_shell_pid().ok_or_else(|| {
                    ApiError::new(
                        "process_query_failed",
                        "the pane has no live shell process to inspect",
                    )
                })?;
                let processes = capture_process_tree(SINGLE_WINDOW_ID, *pane_id, root_pid)?;
                self.action_result(json!(processes), cx)
            }
            RuntimeCommand::SendKey {
                window_id,
                pane_id,
                key,
                modifiers,
                repeat,
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                view.update(cx, |view, cx| {
                    view.runtime_send_key(*key, *modifiers, *repeat, cx)
                })?;
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "key": key.as_str(),
                        "repeat": repeat
                    }),
                    cx,
                )
            }
            RuntimeCommand::Run {
                window_id,
                pane_id,
                command,
                timeout_ms,
                ..
            } => {
                ensure_window(*window_id)?;
                let (_, _, view) = self.pane_entry(*pane_id)?;
                // run 身份与视图的 active_run 同源:OSC 133;C 推进到 Started,
                // 133;D 产出 last_run;等待链据此区分启动超时/完成超时。
                let run = crate::runtime_api::begin_runtime_run();
                let run_id = run.run_id;
                view.update(cx, |view, cx| {
                    view.runtime_prompt(command, true, cx)?;
                    view.attach_run(run);
                    Ok::<(), crate::runtime_api::ApiError>(())
                })?;
                self.agent_hub.submitted(PaneId(*pane_id), command);
                self.action_result(
                    json!({
                        "window_id": SINGLE_WINDOW_ID,
                        "pane_id": pane_id,
                        "run_id": run_id,
                        "timeout_ms": timeout_ms
                    }),
                    cx,
                )
            }
            RuntimeCommand::Exec { .. } => unreachable!("Exec is handled asynchronously"),
            RuntimeCommand::Git { request, .. } => self.execute_git_command(request, cx),
            RuntimeCommand::AgentStart {
                window_id,
                pane_id,
                name,
                kind,
                cwd,
                session_id,
                command,
                worktree,
            } => self.start_agent(
                *window_id,
                *pane_id,
                name,
                *kind,
                cwd.clone(),
                session_id.clone(),
                command,
                worktree.clone(),
                window,
                cx,
            ),
            // prepare_dispatch_command 已把 agent.fork 转成 AgentStart;直接
            // 到达 UI 说明调用方绕过了事务原语,拒绝而不是无事务启动。
            RuntimeCommand::AgentFork { .. } => Err(ApiError::new(
                "invalid_runtime_command",
                "agent.fork must be prepared by the dispatch transaction",
            )),
            RuntimeCommand::AgentPrompt {
                agent,
                generation,
                text,
                submit,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                view.update(cx, |view, cx| view.runtime_prompt(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(target.pane_id), text);
                }
                self.action_result(json!({ "agent": target }), cx)
            }
            RuntimeCommand::AgentPaste {
                agent,
                generation,
                text,
                submit,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                view.update(cx, |view, cx| view.runtime_paste(text, *submit, cx))?;
                if *submit {
                    self.agent_hub.submitted(PaneId(target.pane_id), text);
                }
                self.action_result(json!({ "agent": target }), cx)
            }
            RuntimeCommand::AgentRead {
                agent,
                generation,
                lines,
            } => {
                let target = self.runtime_hub.active_agent(agent, *generation)?;
                let (_, _, view) = self.pane_entry(target.pane_id)?;
                let task_state = self.pane_task_state(PaneId(target.pane_id));
                let read = view.read(cx).runtime_read(
                    SINGLE_WINDOW_ID,
                    target.pane_id,
                    *lines,
                    task_state,
                )?;
                self.action_result(json!({ "agent": target, "read": read }), cx)
            }
        }
    }

    /// `git.*` 四方法执行器(子进程统一)。
    ///
    /// 全序:显式 pane + `expected_cwd` → 重查 pane cwd 与仓库根 → 写操作
    /// 重跑 status 比对 revision(防陈旧三板斧)→ 执行 → 回包带提交后
    /// snapshot 与最新 status 摘要。路径是纯 argv 元素,不解释为 shell。
    fn execute_git_command(
        &mut self,
        request: &crate::runtime_api::git::Request,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        ensure_window(Some(request.window_id))?;
        let (_, _, view) = self.pane_entry(request.pane_id)?;
        let pane_cwd = view.read(cx).cwd.clone();
        if pane_cwd.is_empty() {
            return Err(ApiError::new(
                "git_target_changed",
                "the pane has no working directory",
            ));
        }
        let root = git::repository_root(Path::new(&pane_cwd)).map_err(git_api_error)?;
        let mut request = request.clone();
        request.expected_cwd = normalized_path(&request.expected_cwd);
        request.verify_repository(
            &normalized_path(&pane_cwd),
            &normalized_path(&root.to_string_lossy()),
        )?;
        let status = git::status(&root).map_err(git_api_error)?;
        if request.is_write() {
            request.verify_revision(&status.digest)?;
        }
        let relative = if request.operation == "status" {
            None
        } else {
            Some(git_relative(&root, &request.path)?)
        };
        let action = match request.operation.as_str() {
            "status" => json!({
                "window_id": SINGLE_WINDOW_ID,
                "pane_id": request.pane_id,
                "operation": "status",
                "revision": status.digest.clone(),
                "branch": status.branch.clone(),
                "head": status.head.clone(),
                "ahead": status.ahead,
                "behind": status.behind,
                "entries": status
                    .entries
                    .iter()
                    .map(|entry| {
                        json!({
                            "path": entry.path,
                            "original": entry.original,
                            "index": entry.index.to_string(),
                            "worktree": entry.worktree.to_string(),
                            "conflict": entry.conflict
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            "diff" => {
                let relative = relative.as_deref().expect("diff requires a path");
                let diff = git::diff_text(&root, relative).map_err(git_api_error)?;
                let hunks = git::file_hunks(&root, relative).map_err(git_api_error)?;
                json!({
                    "window_id": SINGLE_WINDOW_ID,
                    "pane_id": request.pane_id,
                    "operation": "diff",
                    "path": relative,
                    "revision": status.digest.clone(),
                    "diff": diff,
                    "hunks": hunks
                        .iter()
                        .map(|hunk| {
                            json!({
                                "old_start": hunk.old_start,
                                "old_lines": hunk.old_lines,
                                "new_start": hunk.new_start,
                                "new_lines": hunk.new_lines
                            })
                        })
                        .collect::<Vec<_>>(),
                })
            }
            "rollback" | "unstage" => {
                let relative = relative.as_deref().expect("write requires a path");
                if request.operation == "rollback" {
                    git::rollback(&root, relative).map_err(git_api_error)?;
                } else {
                    git::unstage(&root, relative).map_err(git_api_error)?;
                }
                // 写后摘要:客户端下一次写操作直接可用,无需额外 status 往返。
                let after = git::status(&root).map_err(git_api_error)?;
                json!({
                    "window_id": SINGLE_WINDOW_ID,
                    "pane_id": request.pane_id,
                    "operation": request.operation,
                    "path": relative,
                    "revision": after.digest,
                })
            }
            other => {
                return Err(ApiError::new(
                    "method_not_found",
                    format!("unknown Git operation {other:?}"),
                ));
            }
        };
        self.action_result(action, cx)
    }

    /// AgentStart/AgentFork(已事务化)的 UI 落地:新 tab 或既有 pane 里
    /// 提交冷启动命令,随后以 worktree provenance 注册 managed agent。
    #[allow(clippy::too_many_arguments)]
    fn start_agent(
        &mut self,
        window_id: Option<u64>,
        pane_id: Option<u64>,
        name: &str,
        kind: crate::ai_agents::AgentKind,
        cwd: Option<std::path::PathBuf>,
        session_id: Option<String>,
        command: &str,
        worktree: Option<crate::git_worktree::WorktreeProvenance>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        ensure_window(window_id)?;
        let created_tab = pane_id.is_none();
        let target = match pane_id {
            Some(pane_id) => {
                self.pane_entry(pane_id)?;
                pane_id
            }
            None => {
                self.add_terminal_tab(cwd, None, false, window, cx);
                self.focused_pane()
                    .map(|pane| pane.0)
                    .ok_or_else(|| ApiError::new("action_failed", "no pane to launch into"))?
            }
        };
        let (_, _, view) = self.pane_entry(target)?;
        let launched = view.update(cx, |view, cx| view.runtime_prompt(command, true, cx));
        if let Err(error) = launched {
            self.rollback_created_tab(created_tab, window, cx);
            return Err(error);
        }
        let agent = self.runtime_hub.register_agent(
            name.to_owned(),
            kind,
            SINGLE_WINDOW_ID,
            target,
            session_id,
            worktree,
        );
        match agent {
            Ok(agent) => self.action_result(json!({ "agent": agent }), cx),
            Err(error) => {
                self.rollback_created_tab(created_tab, window, cx);
                Err(error)
            }
        }
    }

    fn rollback_created_tab(
        &mut self,
        created_tab: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if created_tab {
            self.close_tab_at(self.active, window, cx);
        }
    }

    /// 提交后快照随回执返回:transport 侧 CLI 以 `state_change_seq` 作
    /// `--wait` 基线(缺基线不降级)。
    fn action_result(&self, action: Value, cx: &App) -> Result<Value, ApiError> {
        let snapshot = self.publish_runtime_snapshot(cx);
        let snapshot = serde_json::to_value(snapshot)
            .map_err(|error| ApiError::new("invalid_runtime_response", error.to_string()))?;
        Ok(json!({ "action": action, "snapshot": snapshot }))
    }

    fn pane_task_state(&self, pane_id: PaneId) -> RuntimeTaskState {
        self.agent_hub
            .host(pane_id)
            .map_or(RuntimeTaskState::Idle, |host| {
                runtime_projection::runtime_task_state(host.status())
            })
    }

    fn pane_entry(
        &self,
        pane_id: u64,
    ) -> Result<(usize, usize, gpui::Entity<TerminalView>), ApiError> {
        for (tab_ix, entry) in self.tabs.iter().enumerate() {
            let WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
                continue;
            };
            if let Some(position) = panes.iter().position(|pane| pane.id.0 == pane_id) {
                return Ok((tab_ix, position, panes[position].view.clone()));
            }
        }
        Err(target_not_found(&format!("pane {pane_id}")))
    }

    fn tab_index_for(&self, tab_id: Option<TabId>, index: usize) -> Result<usize, ApiError> {
        if let Some(tab_id) = tab_id {
            return self
                .tabs
                .iter()
                .position(|entry| entry.id == tab_id)
                .ok_or_else(|| target_not_found(&format!("tab {}", tab_id.0)));
        }
        if index < self.tabs.len() {
            Ok(index)
        } else {
            Err(target_not_found(&format!("tab index {index}")))
        }
    }

    pub(super) fn busy_panes(&self, panes: impl Iterator<Item = PaneId>) -> Vec<u64> {
        panes
            .filter(|pane| busy_state(self.pane_task_state(*pane)))
            .map(|pane| pane.0)
            .collect()
    }

    fn tab_pane_ids(&self, index: usize) -> impl Iterator<Item = PaneId> + '_ {
        self.tab_panes(index).map(|pane| pane.id)
    }

    /// tab 重排的活跃下标调整(稳定 TabId 不受位置变化影响)。
    fn move_tab_index(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let entry = self.tabs.remove(from);
        self.tabs.insert(to, entry);
        self.active = if self.active == from {
            to
        } else {
            let mut index = self.active;
            if index > from {
                index -= 1;
            }
            if index >= to {
                index += 1;
            }
            index
        };
    }

    /// 关闭任意 tab(活跃 tab 快捷键与 runtime `tab.close` 共用)。
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

    /// hook 批次后的 managed agent 生命周期与委派回传(前台线程)。
    pub(crate) fn process_agent_lifecycle_events(
        &mut self,
        events: &[AiHookEvent],
        cx: &mut Context<'_, Self>,
    ) {
        for event in events {
            match event.kind {
                AiHookKind::SessionEnd => {
                    if let Some(pane) = event.pane {
                        // agent 退出即清任务栏进度,不留残条。
                        self.clear_taskbar_progress(PaneId(pane), cx);
                        self.runtime_hub.close_agents_for_pane(
                            SINGLE_WINDOW_ID,
                            pane,
                            "agent_exited",
                        );
                    }
                }
                AiHookKind::TurnDone => {
                    if let Some(pane) = event.pane {
                        // 回合结束,进度条属于刚收尾的这一回合。
                        self.clear_taskbar_progress(PaneId(pane), cx);
                    }
                    self.runtime_hub.complete_delegations_for_turn(event);
                    let Some(pane) = event.pane else { continue };
                    let Some(identity) = self
                        .runtime_hub
                        .active_agent_in_pane(SINGLE_WINDOW_ID, pane)
                        .map(|agent| (agent.agent_id, agent.generation))
                    else {
                        continue;
                    };
                    let Some(callback) = self
                        .runtime_hub
                        .take_delegation_callback(&identity.0, identity.1)
                    else {
                        continue;
                    };
                    let origin = callback.origin.pane_id;
                    let Ok((_, _, view)) = self.pane_entry(origin) else {
                        log::warn!(
                            "delegation callback dropped task_id={} reason=origin_pane_missing",
                            callback.task_id
                        );
                        continue;
                    };
                    if view
                        .update(cx, |view, cx| {
                            view.runtime_prompt(&callback.prompt, true, cx)
                        })
                        .is_ok()
                    {
                        self.agent_hub.submitted(PaneId(origin), &callback.prompt);
                    }
                }
                _ => {}
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

/// 路径比较形态:规范化 + `\\?\` 去前缀 + 正斜杠 + 小写(Windows 大小写
/// 不敏感;`verify_repository` 是逐字比较,归一必须在调用前完成)。
fn normalized_path(text: &str) -> String {
    let path = Path::new(text);
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let display = canonical.to_string_lossy().into_owned();
    let display = display.strip_prefix(r"\\?\").unwrap_or(&display);
    display.replace('\\', "/").to_ascii_lowercase()
}

/// 请求路径 → 仓库根相对路径;拒绝 `..` 逃逸(路径是纯 argv,但不是沙箱)。
fn git_relative(root: &Path, raw: &str) -> Result<String, ApiError> {
    let path = Path::new(raw);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApiError::invalid_params(
            "Git path must not escape the repository",
        ));
    }
    let absolute: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    git::relative_path(root, &absolute).map_err(git_api_error)
}

/// git 执行体错误 → 协议错误码(UI/CLI 按 code 分流,消息保持可读)。
fn git_api_error(error: git::GitError) -> ApiError {
    let code = match &error {
        git::GitError::Unavailable(_) => "git_unavailable",
        git::GitError::NotRepository(_) => "git_not_repository",
        git::GitError::NotInHead(_) => "git_not_in_head",
        git::GitError::AccessDenied(_) => "git_access_denied",
        git::GitError::Corrupt(_) => "git_corrupt",
        git::GitError::Command(_) => "git_command_failed",
    };
    ApiError::new(code, error.to_string())
}

/// B38:失败边的用户可见文本。只消费类别/agent,永不拼接
/// `HookFailure.command` 那类技术化合成命令。
fn hook_alert_text(kind: &HookFailureKind, language: UiLanguage) -> String {
    match kind {
        HookFailureKind::Install { agent } => language.format(
            Message::NotifyHookInstallFailed,
            &[("agent", &agent_display_name(agent))],
        ),
        HookFailureKind::Skill { agent } => language.format(
            Message::NotifyHookSkillFailed,
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

fn ensure_window(window_id: Option<u64>) -> Result<(), ApiError> {
    match window_id {
        Some(id) if id != SINGLE_WINDOW_ID => Err(target_not_found(&format!("window {id}"))),
        _ => Ok(()),
    }
}

/// `tab.new` 的可选 shell id → 启动身份;None = 对端默认 shell 探测链。
fn resolve_shell(
    shell_id: Option<&str>,
) -> Result<Option<(slterm_terminal::tty::Shell, String)>, ApiError> {
    let Some(shell_id) = shell_id else {
        return Ok(None);
    };
    let shell = slterm_terminal::tty::resolve_shell(Some(shell_id)).map_err(|error| {
        ApiError::invalid_params(format!("unknown shell {shell_id:?}: {error}"))
    })?;
    let name = super::shell_display_name(shell.program());
    Ok(Some((shell, name)))
}

fn tab_pane_ids(entry: &super::TabEntry) -> Vec<PaneId> {
    match &entry.tab {
        WorkspaceTab::Terminal { panes, .. } => panes.iter().map(|pane| pane.id).collect(),
        WorkspaceTab::Settings { .. }
        | WorkspaceTab::Document { .. }
        | WorkspaceTab::Code { .. }
        | WorkspaceTab::Image { .. } => Vec::new(),
    }
}

/// 虚拟窗口用例驱动端:直接执行 runtime 写命令。
#[cfg(all(test, feature = "gpui-test-support"))]
impl SltermWorkspace {
    pub(crate) fn execute_runtime_command_for_test(
        &mut self,
        command: &RuntimeCommand,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Value, ApiError> {
        self.execute_runtime_command(command, window, cx)
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
            let skill = hook_alert_text(
                &HookFailureKind::Skill {
                    agent: "codex".to_owned(),
                },
                language,
            );
            assert!(skill.contains("Codex"), "{skill}");
            let settings = hook_alert_text(&HookFailureKind::Settings, language);
            assert!(!settings.is_empty());
            for text in [install, skill, settings] {
                assert!(
                    !text.contains("slterm-hook") && !text.contains("slterm_ai_hooks"),
                    "呈现层不得出现合成命令或内部键: {text}"
                );
            }
        }
    }
}
