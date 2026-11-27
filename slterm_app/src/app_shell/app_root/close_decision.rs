//! 关闭决定面：脏 Document 与 Hooks 草稿的保存／丢弃／取消唯一交互入口。
//!
//! 决定只描述关闭范围；真正的保存、关闭意图、PTY 停止与清理仍由各作用域
//! 既有命令链执行，本模块不另建关闭通道。

use std::collections::BTreeMap;

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::DocumentId;
use slterm_workspace::{CloseDecision, PanelGroupId, PanelId, ProjectId, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use crate::i18n::Message;

/// 用户对本次关闭的显式处置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseDecisionChoice {
    Save,
    Discard,
    Cancel,
}

/// 关闭范围：决定完成后原样回到同一作用域的既有命令链。
#[derive(Clone, Debug)]
pub(super) enum CloseDecisionScope {
    Application,
    Group {
        group_id: PanelGroupId,
        others_only: bool,
    },
    View {
        view: ViewKey,
    },
    Project {
        project_id: ProjectId,
    },
    /// 无项目设置浮层：脏 Hooks 草稿只允许丢弃或取消。
    SettingsOverlay,
    /// 根重定位：脏 Hooks 草稿只允许丢弃或取消，保存仍须在 Hooks 页完成。
    RelocateProject {
        project_id: ProjectId,
    },
}

pub(super) struct PendingCloseDecision {
    scope: CloseDecisionScope,
    dirty_documents: Vec<DocumentId>,
    /// 本次关闭涉及的、存在未提交草稿的 Hooks 面板。
    hooks_panels: Vec<PanelId>,
    hooks_dirty: bool,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

impl AppRoot {
    /// 打开决定面；已有决定面时只还焦，不叠加第二层模态。
    pub(super) fn open_close_decision(
        &mut self,
        scope: CloseDecisionScope,
        dirty_documents: Vec<DocumentId>,
        hooks_panels: Vec<PanelId>,
        hooks_dirty: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if self.close_decision.is_some() {
            if let Some(host) = &self.close_decision {
                let token = host.registration.token();
                self.focus_manager.request_focus(token, window, cx)?;
            }
            return Ok(());
        }
        // 关闭决定优先于保存冲突与编码选择：先收合下层模态，避免争抢焦点。
        self.cancel_save_conflict(window, cx)?;
        self.cancel_encoding_prompt(window, cx)?;
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
        self.close_decision = Some(PendingCloseDecision {
            scope,
            dirty_documents,
            hooks_panels,
            hooks_dirty,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    /// 关闭集合中必须取得决定的最后引用脏 Document；集合外仍有引用不算。
    pub(super) fn dirty_documents_for_views(
        &self,
        views: &[ViewKey],
    ) -> Result<Vec<DocumentId>, slterm_workspace::DocumentCloseError> {
        slterm_workspace::last_reference_dirty_documents(&self.snapshot, &self.documents, views)
    }

    /// 给定面板中仍有未提交 Hooks 草稿的身份。
    pub(super) fn dirty_hooks_panels(
        &self,
        panels: impl IntoIterator<Item = PanelId>,
    ) -> Vec<PanelId> {
        panels
            .into_iter()
            .filter(|panel| {
                self.hook_drafts
                    .owner_has_dirty(crate::ai_hook::draft_host::HooksDraftOwner::Panel(*panel))
            })
            .collect()
    }

    /// 取消：只收合决定面并恢复原焦点，不触碰任何关闭状态。
    pub(super) fn cancel_close_decision(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.close_decision.take() else {
            return Ok(());
        };
        if matches!(host.scope, CloseDecisionScope::RelocateProject { .. }) {
            self.pending_project_relocation = None;
        }
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 决定提交：先收合决定面，再按原作用域复用既有关闭命令。
    pub(super) fn resolve_close_decision(
        &mut self,
        choice: CloseDecisionChoice,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.close_decision.as_ref() else {
            return Ok(());
        };
        if choice == CloseDecisionChoice::Save && host.hooks_dirty {
            // Hooks 草稿只能由用户在 Hooks 页显式保存，关闭不得代写配置。
            super::events::notify_hooks_draft_close_pending(cx);
            return Ok(());
        }
        let Some(host) = self.close_decision.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        // 决定面必须真正收合；失败时保留关闭事实不动，让用户重试。
        close?;
        unregister?;

        if choice == CloseDecisionChoice::Cancel {
            return Ok(());
        }
        let decisions = host
            .dirty_documents
            .iter()
            .map(|id| {
                let decision = match choice {
                    CloseDecisionChoice::Save => CloseDecision::Save,
                    CloseDecisionChoice::Discard => CloseDecision::Discard,
                    CloseDecisionChoice::Cancel => unreachable!(),
                };
                (*id, decision)
            })
            .collect::<BTreeMap<_, _>>();
        if choice == CloseDecisionChoice::Discard {
            match &host.scope {
                CloseDecisionScope::Application => {
                    self.discard_all_hooks_drafts_for_close(cx);
                }
                CloseDecisionScope::Group { .. } | CloseDecisionScope::View { .. } => {
                    for panel in &host.hooks_panels {
                        self.discard_panel_hooks_for_close(*panel, cx);
                    }
                }
                CloseDecisionScope::Project { project_id } => {
                    self.discard_project_hooks_drafts_for_close(*project_id, cx);
                }
                CloseDecisionScope::SettingsOverlay => {
                    self.discard_hooks_owner_for_close(
                        crate::ai_hook::draft_host::HooksDraftOwner::Overlay,
                        cx,
                    );
                }
                CloseDecisionScope::RelocateProject { project_id } => {
                    self.discard_project_hooks_drafts_for_close(*project_id, cx);
                }
            }
        }
        match host.scope {
            CloseDecisionScope::Application => {
                match self.start_application_close_with_decisions(decisions, true, window, cx) {
                    Ok(super::application_close::ApplicationCloseStart::Rejected) => {
                        super::events::notify_workspace_write_failed(cx);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::warn!("应用关闭仍缺少文档决定: {error}");
                        super::events::notify_application_close_decision_required(
                            host.dirty_documents.len(),
                            cx,
                        );
                    }
                }
            }
            CloseDecisionScope::Group {
                group_id,
                others_only,
            } => {
                let mode = if others_only {
                    super::batch_close::GroupCloseMode::Others
                } else {
                    super::batch_close::GroupCloseMode::All
                };
                self.close_group_from_request(group_id, mode, decisions, window, cx)?;
            }
            CloseDecisionScope::View { view } => {
                self.close_view_from_request(view, decisions, window, cx)?;
            }
            CloseDecisionScope::Project { project_id } => {
                let allow_hooks_discard = choice == CloseDecisionChoice::Discard;
                self.remove_project_with_decisions(
                    project_id,
                    decisions,
                    allow_hooks_discard,
                    window,
                    cx,
                )?;
            }
            CloseDecisionScope::SettingsOverlay => {
                self.close_settings_overlay(window, cx)?;
            }
            CloseDecisionScope::RelocateProject { project_id } => {
                // 丢弃分支已清除草稿；保存按钮在草稿存在时禁用，不会走到这里。
                if let Some((pending_id, target)) = self.pending_project_relocation.take()
                    && pending_id == project_id
                {
                    self.relocate_project_from_request(project_id, target, cx)?;
                }
            }
        }
        Ok(())
    }

    /// 模态决定面：遮罩拦截背景命中，Escape 等同取消。
    pub(super) fn render_close_decision(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let host = self.close_decision.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let count = host.dirty_documents.len();
        let detail = (count > 0).then(|| {
            language.format(
                Message::EditorCloseWindowDetail,
                &[("count", &count.to_string())],
            )
        });
        let relocation = matches!(host.scope, CloseDecisionScope::RelocateProject { .. });
        let hooks_note = host.hooks_dirty.then(|| {
            if relocation {
                language.text(Message::NavRelocateHooksPending).to_owned()
            } else {
                language
                    .text(Message::SettingsAgentsHooksClosePending)
                    .to_owned()
            }
        });
        let save_enabled = !host.hooks_dirty;
        let mut buttons = div().flex().flex_row().justify_end().gap_2();
        buttons = buttons.child(decision_button(
            "close-decision-cancel",
            language.text(Message::EditorCancel),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) = root.cancel_close_decision(window, cx) {
                    log::warn!("关闭决定面取消失败: {error:?}");
                }
            }),
        ));
        let discard_label = if relocation {
            language.text(Message::NavRelocateDiscard)
        } else {
            language.text(Message::EditorDiscard)
        };
        buttons = buttons.child(decision_button(
            "close-decision-discard",
            discard_label,
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) =
                    root.resolve_close_decision(CloseDecisionChoice::Discard, window, cx)
                {
                    log::warn!("丢弃并关闭失败: {error:?}");
                }
            }),
        ));
        if save_enabled {
            buttons = buttons.child(decision_button(
                "close-decision-save",
                language.text(Message::EditorSave),
                cx,
                cx.listener(|root, _, window, cx| {
                    if let Err(error) =
                        root.resolve_close_decision(CloseDecisionChoice::Save, window, cx)
                    {
                        log::warn!("保存并关闭失败: {error:?}");
                    }
                }),
            ));
        }
        let dialog = div()
            .debug_selector(|| "close-decision-dialog".to_owned())
            .track_focus(&host.focus_handle)
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.cancel_close_decision(window, cx)
                {
                    log::warn!("关闭决定面取消失败: {error:?}");
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
            .child(div().text_sm().child(if relocation {
                language.text(Message::NavRelocatePendingTitle)
            } else {
                language.text(Message::EditorCloseWindowTitle)
            }))
            .children(detail.map(|detail| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail)
            }))
            .children(
                hooks_note.map(|note| div().text_xs().text_color(cx.theme().danger).child(note)),
            )
            .child(buttons);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .flex()
                .items_center()
                .justify_center()
                .child(dialog)
                .into_any_element(),
        )
    }
}

fn decision_button(
    debug_id: &'static str,
    label: &'static str,
    cx: &Context<'_, AppRoot>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(debug_id)
        .debug_selector(move || debug_id.to_owned())
        .px_3()
        .py_1()
        .border_1()
        .border_color(cx.theme().border)
        .text_sm()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
        .into_any_element()
}
