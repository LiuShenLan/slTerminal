//! 根外、失权或尚待专用视图 Document 的真实恢复占位；不持有伪正文。

use gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div,
};
use gpui_component::ActiveTheme as _;

use crate::app_shell::config::Settings;
use crate::i18n::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentRecoveryKind {
    Authorization,
    Paged,
    Replaced,
}

pub(crate) struct DocumentRecoveryView {
    pub(crate) focus_handle: FocusHandle,
    kind: DocumentRecoveryKind,
}

impl DocumentRecoveryView {
    pub(crate) fn new(kind: DocumentRecoveryKind, cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            kind,
        }
    }
}

impl Render for DocumentRecoveryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<Settings>().language;
        let (selector, message) = match self.kind {
            DocumentRecoveryKind::Authorization => (
                "document-recovery-authorization",
                Message::EditorReauthorizationRequired,
            ),
            DocumentRecoveryKind::Paged => (
                "document-recovery-paged",
                Message::EditorPagedRecoveryRequired,
            ),
            DocumentRecoveryKind::Replaced => (
                "document-recovery-replaced",
                Message::EditorReplacedRecovery,
            ),
        };
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&self.focus_handle)
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .debug_selector(|| selector.to_owned())
            .child(language.text(message))
    }
}
