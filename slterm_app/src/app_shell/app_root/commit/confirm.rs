// Commit 破坏性写确认面：默认取消，脏 Document 走保存／丢弃／取消。

use gpui::{AnyElement, Context, IntoElement as _, MouseButton, ParentElement as _};
use gpui_component::ActiveTheme as _;

use super::super::AppRoot;
use super::*;
use crate::i18n::Message;

/// 破坏性写确认面：默认取消；脏 Document 存在时保存／丢弃／取消。
pub(crate) fn render_commit_confirmation(
    root: &AppRoot,
    cx: &Context<'_, AppRoot>,
) -> Option<AnyElement> {
    let host = root.commit_confirmation.as_ref()?;
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let permanent = matches!(
        &host.action,
        CommitWriteAction::Delete {
            permanent: true,
            ..
        }
    );
    let (title, body) = match &host.action {
        CommitWriteAction::Rollback { .. } => (
            language.text(Message::CommitConfirmRollbackTitle),
            language.text(Message::CommitConfirmRollbackBody),
        ),
        CommitWriteAction::Delete { .. } => (
            language.text(Message::CommitConfirmDeleteTitle),
            language.text(Message::CommitConfirmDeleteBody),
        ),
    };
    let path = host.action.path();
    let path_text = path.as_path().to_string_lossy().into_owned();
    let dirty = !host.dirty_documents.is_empty();
    let mut buttons = div().flex().flex_row().justify_end().gap_2();
    buttons = buttons.child(confirmation_button(
        "commit-confirm-cancel",
        language.text(Message::CommitConfirmCancel),
        cx,
        cx.listener(|root, _, window, cx| {
            if let Err(error) = root.cancel_commit_confirmation(window, cx) {
                log::warn!("Commit 确认面取消失败: {error:?}");
            }
        }),
    ));
    if dirty {
        buttons = buttons.child(confirmation_button(
            "commit-confirm-discard",
            language.text(Message::CommitConfirmDiscard),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) =
                    root.resolve_commit_confirmation(CommitConfirmChoice::Discard, window, cx)
                {
                    log::warn!("Commit 丢弃并继续失败: {error:?}");
                }
            }),
        ));
        buttons = buttons.child(confirmation_button(
            "commit-confirm-save",
            language.text(Message::CommitConfirmSave),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) =
                    root.resolve_commit_confirmation(CommitConfirmChoice::Save, window, cx)
                {
                    log::warn!("Commit 保存并继续失败: {error:?}");
                }
            }),
        ));
    } else {
        buttons = buttons.child(confirmation_button(
            "commit-confirm-execute",
            language.text(Message::CommitConfirmExecute),
            cx,
            cx.listener(|root, _, window, cx| {
                if let Err(error) =
                    root.resolve_commit_confirmation(CommitConfirmChoice::Execute, window, cx)
                {
                    log::warn!("Commit 确认执行失败: {error:?}");
                }
            }),
        ));
    }
    let note = (dirty || permanent).then(|| {
        let text = if dirty {
            language.text(Message::CommitConfirmDirtyBody)
        } else {
            language.text(Message::CommitConfirmDeleteBody)
        };
        div()
            .text_xs()
            .text_color(cx.theme().danger)
            .child(text)
            .into_any_element()
    });
    let dialog = div()
        .debug_selector(|| "commit-confirm-dialog".to_owned())
        .track_focus(&host.focus_handle)
        .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape"
                && let Err(error) = root.cancel_commit_confirmation(window, cx)
            {
                log::warn!("Commit 确认面取消失败: {error:?}");
            }
        }))
        .w(px(480.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .text_color(cx.theme().foreground)
        .child(div().text_sm().child(title))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(body),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(path_text),
        )
        .children(note)
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

fn confirmation_button(
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
