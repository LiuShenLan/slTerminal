//! AppRoot 的壳事件落点。

use std::time::Duration;

use gpui::{App, Context, Window};
use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError};
use slterm_workspace::ProjectCommandError;

use super::AppRoot;
use super::command_registry::{CommandId, CommandParameters, CommandSource, CommandTarget};
use super::commands::AppRootError;
use super::focus_manager::FocusError;
use super::task_service::CancellationToken;
use crate::app_shell::{SltermShellEvent, handover_error};

/// 窗口或实体先行销毁时，也必须为未结算 handover 返回 Closing。
struct HandoverCompletionGuard {
    completion: Option<std::sync::mpsc::SyncSender<Result<HandoverResponse, JsonRpcError>>>,
}

impl HandoverCompletionGuard {
    fn new(
        completion: std::sync::mpsc::SyncSender<Result<HandoverResponse, JsonRpcError>>,
    ) -> Self {
        Self {
            completion: Some(completion),
        }
    }

    fn send(mut self, response: Result<HandoverResponse, JsonRpcError>) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.try_send(response);
        }
    }
}

impl Drop for HandoverCompletionGuard {
    fn drop(&mut self) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.try_send(Err(closing_handover_error()));
        }
    }
}

impl AppRoot {
    /// 后台只负责定时唤醒；通道排空与状态变更都在窗口前台完成。
    pub(crate) fn start_shell_event_pump(
        &mut self,
        receiver: std::sync::mpsc::Receiver<SltermShellEvent>,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<'_, Self>,
    ) {
        let executor = cx.background_executor().clone();
        let cancellation = CancellationToken::new();
        let task = {
            let cancellation = cancellation.clone();
            cx.spawn(async move |this, cx| {
                loop {
                    executor.timer(Duration::from_millis(120)).await;
                    if cancellation.is_cancelled() {
                        break;
                    }
                    let mut events = Vec::new();
                    while let Ok(event) = receiver.try_recv() {
                        events.push(event);
                    }
                    if events.is_empty() {
                        continue;
                    }
                    cx.update(|cx| {
                        let _ = window_handle.update(cx, |_root, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                let mut events = events.into_iter();
                                while let Some(event) = events.next() {
                                    root.handle_shell_event(event, window, cx);
                                    if root.close_completed {
                                        // 关闭后不再消费领域事件，但同批 handover 必须得到
                                        // 结构化 Closing，不能只让调用方观察发送端断连。
                                        for event in events {
                                            if let SltermShellEvent::Handover {
                                                completion, ..
                                            } = event
                                            {
                                                let _ = completion
                                                    .try_send(Err(closing_handover_error()));
                                            }
                                        }
                                        break;
                                    }
                                }
                            });
                        });
                    });
                }
            })
        };
        if let Err(error) =
            self.tasks
                .register(super::task_service::TaskId::ShellEvents, cancellation, task)
        {
            log::warn!("壳事件泵重复注册被拒绝: {error:?}");
        }
    }

    pub(crate) fn handle_shell_event(
        &mut self,
        event: SltermShellEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            SltermShellEvent::NotificationFocus(pane) => {
                crate::windowing::focus::focus_window(window);
                if let Some(pane) = pane
                    && let Err(error) = self.focus_notification_pane(pane, window, cx)
                {
                    log::warn!("通知目标终端不存在: {error}");
                }
                cx.notify();
            }
            SltermShellEvent::NotificationChoice { pane_id } => {
                crate::windowing::focus::focus_window(window);
                if let Err(error) = self.focus_notification_pane(pane_id, window, cx) {
                    log::warn!("通知动作目标终端不存在: {error}");
                }
                crate::audit::record(crate::audit::AuditEvent::new(
                    "notification_choice",
                    "app_shell::app_root::events::handle_shell_event",
                    crate::audit::AuditResult::Acknowledged,
                ));
                cx.notify();
            }
            SltermShellEvent::TrayQuit => {
                let _ = self.request_close(window, cx);
            }
            SltermShellEvent::WorkspaceWriteFailed => {
                notify_workspace_write_failed(cx);
                cx.notify();
            }
            SltermShellEvent::WorkspaceRecovered(notice) => {
                notify_workspace_recovered(notice, cx);
                cx.notify();
            }
            SltermShellEvent::Handover { params, completion } => {
                let response = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_handover_error())
                } else {
                    match params {
                        HandoverParams::Focus => {
                            crate::windowing::focus::focus_window(window);
                            cx.notify();
                            Ok(HandoverResponse::accepted())
                        }
                        HandoverParams::OpenProject { project_root } => {
                            let path = match CanonicalPath::new(project_root) {
                                Ok(path) => path,
                                Err(_) => {
                                    let _ =
                                        completion.try_send(Err(invalid_open_project_request()));
                                    return;
                                }
                            };
                            let asynchronous_completion =
                                HandoverCompletionGuard::new(completion.clone());
                            self.open_project_with_completion(
                                path,
                                window,
                                cx,
                                move |result, _, window, cx| {
                                    let response = match result {
                                        Ok(_) => {
                                            if let Some(window) = window {
                                                crate::windowing::focus::focus_window(window);
                                                cx.notify();
                                                Ok(HandoverResponse::accepted())
                                            } else {
                                                Err(closing_handover_error())
                                            }
                                        }
                                        Err(error) => Err(open_project_handover_error(error)),
                                    };
                                    asynchronous_completion.send(response);
                                },
                            );
                            return;
                        }
                        HandoverParams::NewTerminal { project_root, cwd } => {
                            let created = parse_new_terminal_paths(project_root, cwd).and_then(
                                |(project_root, cwd)| {
                                    let shell = self.shell.clone();
                                    self.execute_command(
                                        CommandId::NewTerminal,
                                        CommandSource::Application,
                                        CommandTarget::Focused,
                                        Some(CommandParameters::NewTerminal {
                                            shell,
                                            project_root,
                                            cwd,
                                        }),
                                        window,
                                        cx,
                                    )
                                    .map(|_| ())
                                    .map_err(new_terminal_handover_error)
                                },
                            );
                            match created {
                                Ok(()) => {
                                    crate::windowing::focus::focus_window(window);
                                    cx.notify();
                                    Ok(HandoverResponse::accepted())
                                }
                                Err(error) => Err(error),
                            }
                        }
                        HandoverParams::OpenFile { .. } => Err(handover_error(
                            DomainErrorCode::InvalidRequest,
                            -32000,
                            "handover request is not implemented",
                        )),
                    }
                };
                // 回执接收端可能已超时退出；非阻塞发送失败只结束本次答复。
                let _ = completion.try_send(response);
            }
            SltermShellEvent::OpenDirectories(paths) => {
                for path in paths {
                    match CanonicalPath::new(path) {
                        Ok(path) => {
                            self.open_project_with_completion(
                                path,
                                window,
                                cx,
                                |result, _, _, _| {
                                    if let Err(error) = result {
                                        log::warn!("打开外部目录失败: {error}");
                                    }
                                },
                            );
                        }
                        Err(error) => log::warn!("外部目录路径无效: {error}"),
                    }
                }
            }
        }
    }
}

fn parse_new_terminal_paths(
    project_root: Option<String>,
    cwd: Option<String>,
) -> Result<(Option<CanonicalPath>, Option<CanonicalPath>), JsonRpcError> {
    let project_root = project_root
        .map(CanonicalPath::new)
        .transpose()
        .map_err(|_| invalid_new_terminal_request())?;
    let cwd = cwd
        .map(CanonicalPath::new)
        .transpose()
        .map_err(|_| invalid_new_terminal_request())?;
    Ok((project_root, cwd))
}

fn invalid_new_terminal_request() -> JsonRpcError {
    handover_error(
        DomainErrorCode::InvalidRequest,
        -32000,
        "invalid new terminal path",
    )
}

fn new_terminal_handover_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_handover_error(),
        AppRootError::Focus(FocusError::ModalActive)
        | AppRootError::NewTerminalTargetUnauthorized
        | AppRootError::Project(ProjectCommandError::TerminalCwdOutsideProject) => handover_error(
            DomainErrorCode::Unauthorized,
            -32002,
            "new terminal is not authorized",
        ),
        _ => handover_error(
            DomainErrorCode::NotReady,
            -32001,
            "new terminal is not ready",
        ),
    }
}

fn invalid_open_project_request() -> JsonRpcError {
    handover_error(
        DomainErrorCode::InvalidRequest,
        -32000,
        "invalid project root",
    )
}

fn open_project_handover_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_handover_error(),
        AppRootError::Focus(FocusError::ModalActive) => handover_error(
            DomainErrorCode::Unauthorized,
            -32002,
            "project activation is not authorized",
        ),
        _ => handover_error(
            DomainErrorCode::NotReady,
            -32001,
            "open project is not ready",
        ),
    }
}

fn closing_handover_error() -> JsonRpcError {
    handover_error(DomainErrorCode::Closing, -32004, "closing")
}

pub(crate) fn notify_workspace_write_failed(cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    crate::notify::deliver_startup_summary(
        &crate::notify::Notification::ProcessAlert {
            text: language
                .text(crate::i18n::Message::NotifyWorkspaceWriteFailed)
                .to_owned(),
        },
        cx,
    );
}

pub(crate) fn notify_workspace_recovered(
    notice: crate::persistence::startup::WorkspaceRecoveryNotice,
    cx: &mut App,
) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let message = match notice {
        crate::persistence::startup::WorkspaceRecoveryNotice::RestoredFromBackup => {
            crate::i18n::Message::NotifyWorkspaceRestoredFromBackup
        }
        crate::persistence::startup::WorkspaceRecoveryNotice::Isolated => {
            crate::i18n::Message::NotifyWorkspaceIsolated
        }
    };
    crate::notify::deliver_startup_summary(
        &crate::notify::Notification::ProcessAlert {
            text: language.text(message).to_owned(),
        },
        cx,
    );
}

#[cfg(test)]
mod handover_completion_guard_tests {
    use super::*;

    #[test]
    fn dropping_unsettled_guard_returns_closing_without_a_window() {
        let (completion, response) = std::sync::mpsc::sync_channel(1);
        drop(HandoverCompletionGuard::new(completion));
        let error = response.try_recv().unwrap().unwrap_err();
        assert_eq!(error.code, -32004);
        assert_eq!(
            error.data.map(|data| data.code),
            Some(DomainErrorCode::Closing)
        );
    }

    #[test]
    fn completed_guard_does_not_send_a_second_response() {
        let (completion, response) = std::sync::mpsc::sync_channel(1);
        HandoverCompletionGuard::new(completion).send(Ok(HandoverResponse::accepted()));
        assert!(response.try_recv().unwrap().unwrap().is_accepted());
        assert!(response.try_recv().is_err());
    }
}
