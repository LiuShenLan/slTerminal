//! 设置页诊断导出：显式完整路径类别、后台白名单收集与原子写。
//!
//! 默认导出只含环境、健康、脱敏日志尾与端点发现元数据；完整路径必须
//! 由用户显式勾选，且不进入通知或日志。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

pub(super) struct DiagnosticsDialog {
    include_full_paths: bool,
    busy: bool,
    error: Option<Message>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    pub(super) fn open_diagnostics_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.ensure_not_closing().is_err() {
            return Ok(());
        }
        if self.diagnostics_dialog.is_some() {
            return Ok(());
        }
        let focus_handle = cx.focus_handle();
        let registration = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self.focus_manager.open_modal(
            self.focus_manager.workspace_token(),
            self.focus_manager.workspace_token(),
            registration.token(),
            window,
            cx,
        ) {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.diagnostics_dialog = Some(DiagnosticsDialog {
            include_full_paths: false,
            busy: false,
            error: None,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_diagnostics_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(dialog) = self.diagnostics_dialog.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(dialog.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn toggle_diagnostics_full_paths(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(dialog) = self.diagnostics_dialog.as_mut() {
            dialog.include_full_paths = !dialog.include_full_paths;
            dialog.error = None;
            cx.notify();
        }
    }

    /// 完整路径只在显式选择时收集；全部来自运行态只读事实。
    fn diagnostics_full_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        let app_paths = crate::platform::dirs::app_paths();
        for root in [app_paths.config_root(), app_paths.local_root()] {
            if let Some(text) = root.to_str() {
                paths.push(text.to_owned());
            }
        }
        for project in self.snapshot.registry().projects() {
            if let Some(text) = project.root().as_path().to_str() {
                paths.push(text.to_owned());
            }
        }
        for (_, location) in self.snapshot.documents() {
            if let Some(text) = location.canonical_path().as_path().to_str() {
                paths.push(text.to_owned());
            }
        }
        paths
    }

    /// 导出入口：后台构建白名单报告、选择目标并原子写。
    pub(super) fn start_export_diagnostics(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.diagnostics_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let include_full_paths = dialog.include_full_paths;
        let runtime_available = self.global_close.is_some();
        let full_paths = if include_full_paths {
            self.diagnostics_full_paths()
        } else {
            Vec::new()
        };
        let windows = self.windows.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::DiagnosticsExport;
        if let Some(dialog) = self.diagnostics_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let written = executor.spawn(async move {
                    let mut report = crate::diagnostics::DiagnosticReport::new(
                        crate::diagnostics::DiagnosticSnapshot::current(
                            runtime_available,
                            runtime_available,
                        ),
                    );
                    report.collect_app_log(&crate::logging::current_log_path(), 200);
                    if include_full_paths {
                        report = report.with_full_paths(full_paths);
                    }
                    let Some(target) = windows
                        .pick_save_target(owner, std::path::Path::new("diagnostics.json"))
                        .map_err(|error| format!("选择诊断目标失败: {error}"))?
                    else {
                        return Ok::<bool, String>(false);
                    };
                    crate::atomic_file::write(&target, report.to_json().as_bytes())
                        .map_err(|error| format!("写入诊断文件失败: {error}"))?;
                    Ok(true)
                });
                cx.spawn(async move |this, cx| {
                    let result = written.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_export_diagnostics(result, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("诊断导出任务未登记");
        }
    }

    /// 复制入口：后台生成同一 DTO 文本，前台写入剪贴板。
    pub(super) fn start_copy_diagnostics(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(dialog) = self.diagnostics_dialog.as_ref() else {
            return;
        };
        if dialog.busy {
            return;
        }
        if self.ensure_not_closing().is_err() {
            return;
        }
        let include_full_paths = dialog.include_full_paths;
        let runtime_available = self.global_close.is_some();
        let full_paths = if include_full_paths {
            self.diagnostics_full_paths()
        } else {
            Vec::new()
        };
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task = TaskId::DiagnosticsCopy;
        if let Some(dialog) = self.diagnostics_dialog.as_mut() {
            dialog.busy = true;
            dialog.error = None;
        }
        cx.notify();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let generated = executor.spawn(async move {
                    let mut report = crate::diagnostics::DiagnosticReport::new(
                        crate::diagnostics::DiagnosticSnapshot::current(
                            runtime_available,
                            runtime_available,
                        ),
                    );
                    report.collect_app_log(&crate::logging::current_log_path(), 200);
                    if include_full_paths {
                        report = report.with_full_paths(full_paths);
                    }
                    report.to_json()
                });
                cx.spawn(async move |this, cx| {
                    let generated = generated.await;
                    let _ = window_handle.update(cx, |_, _, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            if let Some(dialog) = root.diagnostics_dialog.as_mut() {
                                dialog.busy = false;
                            }
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(generated));
                            super::events::notify_diagnostics_copied(cx);
                            cx.notify();
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("诊断复制任务未登记");
        }
    }

    fn finish_export_diagnostics(
        &mut self,
        result: Result<bool, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(dialog) = self.diagnostics_dialog.as_mut() else {
            return;
        };
        dialog.busy = false;
        match result {
            Ok(true) => {
                let close = self.close_diagnostics_dialog(window, cx);
                super::events::notify_diagnostics_exported(cx);
                if let Err(error) = close {
                    log::warn!("诊断对话框关闭失败: {error:?}");
                }
            }
            Ok(false) => cx.notify(),
            Err(error) => {
                log::warn!("诊断导出失败: {error}");
                dialog.error = Some(Message::DiagnosticsFailed);
                cx.notify();
            }
        }
    }

    pub(super) fn render_diagnostics_dialog(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let dialog = self.diagnostics_dialog.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .child(language.text(Message::DiagnosticsTitle)),
            )
            .child(
                div()
                    .id("diagnostics-full-paths")
                    .debug_selector(|| "diagnostics-full-paths".to_owned())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, _, cx| {
                            root.toggle_diagnostics_full_paths(cx);
                        }),
                    )
                    .child(
                        div()
                            .w(px(14.0))
                            .text_xs()
                            .text_color(if dialog.include_full_paths {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(if dialog.include_full_paths {
                                "✓"
                            } else {
                                "–"
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(language.text(Message::DiagnosticsFullPaths)),
                    ),
            );
        if let Some(error) = dialog.error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(error)),
            );
        }
        let mut actions = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("diagnostics-cancel")
                    .debug_selector(|| "diagnostics-cancel".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .child(language.text(Message::EditorCancel))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            if let Err(error) = root.close_diagnostics_dialog(window, cx) {
                                log::warn!("诊断对话框取消失败: {error:?}");
                            }
                        }),
                    ),
            );
        if dialog.busy {
            actions = actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::BackupExporting)),
            );
        } else {
            actions = actions.child(
                div()
                    .id("diagnostics-copy")
                    .debug_selector(|| "diagnostics-copy".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .child(language.text(Message::DiagnosticsCopy))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            root.start_copy_diagnostics(window, cx);
                        }),
                    ),
            );
            actions = actions.child(
                div()
                    .id("diagnostics-export")
                    .debug_selector(|| "diagnostics-export".to_owned())
                    .cursor_pointer()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().accent)
                    .child(language.text(Message::DiagnosticsExport))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|root, _, window, cx| {
                            root.start_export_diagnostics(window, cx);
                        }),
                    ),
            );
        }
        body = body.child(actions);
        let dialog_element = div()
            .debug_selector(|| "diagnostics-dialog".to_owned())
            .track_focus(&dialog.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.close_diagnostics_dialog(window, cx)
                {
                    log::warn!("诊断对话框关闭失败: {error:?}");
                }
            }))
            .w(px(420.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(body);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(110.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog_element),
                )
                .into_any_element(),
        )
    }
}
