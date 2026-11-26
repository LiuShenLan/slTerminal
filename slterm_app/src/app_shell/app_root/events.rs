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
use crate::app_shell::{SltermShellEvent, documents::DocumentOpenError, handover_error};

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
                                            match event {
                                                SltermShellEvent::Handover {
                                                    completion, ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_handover_error()));
                                                }
                                                SltermShellEvent::RuntimeSnapshot {
                                                    completion,
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeMutation {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeRead {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeWait {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeAgents { completion } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeGitStatus {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeGitDiff {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                SltermShellEvent::RuntimeGitOpen {
                                                    completion,
                                                    ..
                                                } => {
                                                    let _ = completion
                                                        .try_send(Err(closing_snapshot_error()));
                                                }
                                                _ => {}
                                            }
                                        }
                                        break;
                                    }
                                }
                                if !root.close_completed {
                                    root.poll_fs_watch(window, cx);
                                    root.resume_pending_application_close(cx);
                                    root.poll_document_autosave(cx);
                                    root.poll_settings_save(cx);
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
                        HandoverParams::OpenFile { path } => {
                            let path = match CanonicalPath::new(path) {
                                Ok(path) => path,
                                Err(_) => {
                                    let _ = completion.try_send(Err(invalid_open_file_request()));
                                    return;
                                }
                            };
                            let request = match self.prepare_document_handover(path, window, cx) {
                                Ok(request) => request,
                                Err(error) => {
                                    let _ =
                                        completion.try_send(Err(open_file_handover_error(error)));
                                    return;
                                }
                            };
                            let asynchronous_completion =
                                HandoverCompletionGuard::new(completion.clone());
                            self.queue_document_open(
                                request,
                                Some(Box::new(move |result, window, cx| {
                                    let response = match result {
                                        Ok(()) => {
                                            crate::windowing::focus::focus_window(window);
                                            cx.notify();
                                            Ok(HandoverResponse::accepted())
                                        }
                                        Err(error) => Err(open_file_handover_error(error)),
                                    };
                                    asynchronous_completion.send(response);
                                })),
                                window,
                                cx,
                            );
                            return;
                        }
                    }
                };
                // 回执接收端可能已超时退出；非阻塞发送失败只结束本次答复。
                let _ = completion.try_send(response);
            }
            SltermShellEvent::RuntimeSnapshot { completion } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.runtime_snapshot_resources()
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeMutation {
                mutation,
                completion,
            } => match self.handle_runtime_mutation(mutation, &completion, window, cx) {
                super::runtime_snapshot::RuntimeMutationDispatch::Reply(reply) => {
                    let _ = completion.try_send(reply);
                }
                super::runtime_snapshot::RuntimeMutationDispatch::Deferred => {}
            },
            SltermShellEvent::RuntimeRead {
                terminal_id,
                generation,
                agent_id,
                completion,
            } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.handle_runtime_read(terminal_id, generation, agent_id.as_deref(), cx)
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeWait {
                terminal_id,
                generation,
                agent_id,
                completion,
            } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.handle_runtime_wait(terminal_id, generation, agent_id.as_deref(), cx)
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeAgents { completion } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.runtime_agents(cx)
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeGitStatus {
                project_id,
                completion,
            } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.runtime_git_target(project_id)
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeGitDiff {
                project_id,
                path,
                completion,
            } => {
                let result = if self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed
                {
                    Err(closing_snapshot_error())
                } else {
                    self.runtime_git_diff_target(project_id, path)
                };
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeGitOpen {
                project_id,
                binding,
                path,
                kind,
                completion,
            } => {
                let result =
                    self.handle_runtime_git_open(project_id, binding, path, kind, window, cx);
                let _ = completion.try_send(result);
            }
            SltermShellEvent::RuntimeGitRefresh { project_id } => {
                // Git 写已完成；只刷新同一项目的 Commit 视图，其他视图由 watcher 刷新。
                if !(self.close_in_progress
                    || self.application_close.is_some()
                    || self.close_completed)
                {
                    self.refresh_commit_view_for_project(project_id, window, cx);
                }
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

fn invalid_open_file_request() -> JsonRpcError {
    handover_error(DomainErrorCode::InvalidRequest, -32000, "invalid file path")
}

pub(super) fn open_file_handover_error(error: AppRootError) -> JsonRpcError {
    match error {
        AppRootError::Closing => closing_handover_error(),
        AppRootError::Focus(FocusError::ModalActive)
        | AppRootError::OpenFileTargetUnauthorized
        | AppRootError::DocumentOpen(DocumentOpenError::Unauthorized) => handover_error(
            DomainErrorCode::Unauthorized,
            -32002,
            "open file is not authorized",
        ),
        AppRootError::DocumentOpen(DocumentOpenError::Io(error))
            if error.kind() == std::io::ErrorKind::InvalidInput =>
        {
            invalid_open_file_request()
        }
        _ => handover_error(DomainErrorCode::NotReady, -32001, "open file is not ready"),
    }
}

fn closing_handover_error() -> JsonRpcError {
    handover_error(DomainErrorCode::Closing, -32004, "closing")
}

fn closing_snapshot_error() -> JsonRpcError {
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

pub(crate) fn notify_terminal_cwd_fallback(cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    crate::notify::deliver_startup_summary(
        &crate::notify::Notification::ProcessAlert {
            text: language
                .text(crate::i18n::Message::NotifyTerminalCwdFallback)
                .to_owned(),
        },
        cx,
    );
}

fn workspace_repair_text(count: usize, language: crate::i18n::UiLanguage) -> String {
    language.format(
        crate::i18n::Message::NotifyWorkspaceRepairs,
        &[("count", &count.to_string())],
    )
}

/// 启动恢复跳过或修复条目后的聚合提示；不含路径或具体条目。
pub(crate) fn notify_workspace_repairs(count: usize, cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let text = workspace_repair_text(count, language);
    crate::notify::deliver_startup_summary(&crate::notify::Notification::ProcessAlert { text }, cx);
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

fn document_save_error_text(
    error: &crate::text_document::SaveError,
    language: crate::i18n::UiLanguage,
) -> String {
    match error {
        crate::text_document::SaveError::Changed => language
            .text(crate::i18n::Message::EditorSaveChanged)
            .to_owned(),
        crate::text_document::SaveError::ReadOnly => language
            .text(crate::i18n::Message::EditorSaveReadOnly)
            .to_owned(),
        error => language.format(
            crate::i18n::Message::EditorSaveFailed,
            &[("error", &error.to_string())],
        ),
    }
}

fn application_close_decision_text(count: usize, language: crate::i18n::UiLanguage) -> String {
    language.format(
        crate::i18n::Message::EditorCloseWindowDetail,
        &[("count", &count.to_string())],
    )
}

pub(crate) fn notify_hooks_draft_close_pending(cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let text = language
        .text(crate::i18n::Message::SettingsAgentsHooksClosePending)
        .to_owned();
    crate::notify::deliver_startup_summary(&crate::notify::Notification::ProcessAlert { text }, cx);
}

pub(crate) fn notify_application_close_decision_required(count: usize, cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let text = application_close_decision_text(count, language);
    crate::notify::deliver_startup_summary(&crate::notify::Notification::ProcessAlert { text }, cx);
}

pub(crate) fn notify_document_save_error(error: &crate::text_document::SaveError, cx: &mut App) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let text = document_save_error_text(error, language);
    crate::notify::deliver_startup_summary(&crate::notify::Notification::ProcessAlert { text }, cx);
}

fn document_save_state_text(
    error: slterm_document::SaveTransactionError,
    language: crate::i18n::UiLanguage,
) -> String {
    if error == slterm_document::SaveTransactionError::ReadOnly {
        language
            .text(crate::i18n::Message::EditorSaveReadOnly)
            .to_owned()
    } else {
        language.format(
            crate::i18n::Message::EditorSaveFailed,
            &[("error", &error.to_string())],
        )
    }
}

pub(crate) fn notify_document_save_state(
    error: slterm_document::SaveTransactionError,
    cx: &mut App,
) {
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let text = document_save_state_text(error, language);
    crate::notify::deliver_startup_summary(&crate::notify::Notification::ProcessAlert { text }, cx);
}

#[cfg(test)]
mod document_save_notice_tests {
    use super::{
        application_close_decision_text, document_save_error_text, document_save_state_text,
        workspace_repair_text,
    };
    use crate::i18n::{Message, UiLanguage};
    use crate::text_document::SaveError;

    /// 启动恢复聚合提示带计数且不插入路径或具体条目。
    #[test]
    fn workspace_repair_notice_states_the_skipped_item_count() {
        let en = workspace_repair_text(3, UiLanguage::EnUs);
        assert!(en.contains('3'), "必须显示条目数: {en}");
        assert!(!en.contains("C:"), "提示不得携带路径: {en}");
        let zh = workspace_repair_text(3, UiLanguage::ZhCn);
        assert!(zh.contains('3'), "必须显示条目数: {zh}");
    }

    #[test]
    fn application_close_decision_notice_preserves_document_count() {
        assert_eq!(
            application_close_decision_text(2, UiLanguage::EnUs),
            UiLanguage::EnUs.format(Message::EditorCloseWindowDetail, &[("count", "2")])
        );
    }

    #[test]
    fn save_failures_keep_specific_user_visible_causes() {
        assert_eq!(
            document_save_error_text(&SaveError::Changed, UiLanguage::EnUs),
            UiLanguage::EnUs.text(Message::EditorSaveChanged)
        );
        assert_eq!(
            document_save_error_text(&SaveError::ReadOnly, UiLanguage::EnUs),
            UiLanguage::EnUs.text(Message::EditorSaveReadOnly)
        );
        assert_eq!(
            document_save_state_text(
                slterm_document::SaveTransactionError::ReadOnly,
                UiLanguage::EnUs,
            ),
            UiLanguage::EnUs.text(Message::EditorSaveReadOnly)
        );
        let error = SaveError::Unauthorized;
        assert_eq!(
            document_save_error_text(&error, UiLanguage::EnUs),
            UiLanguage::EnUs.format(Message::EditorSaveFailed, &[("error", &error.to_string())],)
        );
    }
}

#[cfg(test)]
mod open_file_handover_error_tests {
    use super::*;

    fn open_io_error(kind: std::io::ErrorKind, message: &'static str) -> AppRootError {
        AppRootError::DocumentOpen(DocumentOpenError::Io(std::io::Error::new(kind, message)))
    }

    #[test]
    fn open_file_errors_preserve_protocol_boundary_without_paths() {
        let private_path = "D:/private/project/file.txt";
        let cases = [
            (AppRootError::Closing, DomainErrorCode::Closing),
            (
                AppRootError::Focus(FocusError::ModalActive),
                DomainErrorCode::Unauthorized,
            ),
            (
                AppRootError::OpenFileTargetUnauthorized,
                DomainErrorCode::Unauthorized,
            ),
            (
                AppRootError::DocumentOpen(DocumentOpenError::Unauthorized),
                DomainErrorCode::Unauthorized,
            ),
            (
                open_io_error(std::io::ErrorKind::PermissionDenied, private_path),
                DomainErrorCode::NotReady,
            ),
            (
                open_io_error(std::io::ErrorKind::InvalidInput, private_path),
                DomainErrorCode::InvalidRequest,
            ),
            (
                open_io_error(std::io::ErrorKind::NotFound, private_path),
                DomainErrorCode::NotReady,
            ),
        ];
        for (error, expected) in cases {
            let error = open_file_handover_error(error);
            assert_eq!(error.data.unwrap().code, expected);
            assert!(!error.message.contains(private_path));
        }
    }
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
