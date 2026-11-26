//! 根外、失权或尚待专用视图 Document 的真实恢复占位；不持有伪正文。

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, button::Button};

use crate::app_shell::config::Settings;
use crate::i18n::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentRecoveryKind {
    Authorization,
    Paged,
    Replaced,
    /// 文件缺失或无法解码的干净 Document：保留可关闭占位，不静默丢面板。
    Unavailable,
}

/// 占位只发意图；重新授权由 AppRoot 持有真实恢复器与后台读取。
pub(crate) enum DocumentRecoveryEvent {
    Reauthorize,
}

pub(crate) struct DocumentRecoveryView {
    pub(crate) focus_handle: FocusHandle,
    kind: DocumentRecoveryKind,
    reauthorization_failed: bool,
}

impl EventEmitter<DocumentRecoveryEvent> for DocumentRecoveryView {}

impl DocumentRecoveryView {
    pub(crate) fn new(kind: DocumentRecoveryKind, cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            kind,
            reauthorization_failed: false,
        }
    }

    pub(crate) fn kind(&self) -> DocumentRecoveryKind {
        self.kind
    }

    pub(crate) fn set_kind(&mut self, kind: DocumentRecoveryKind, cx: &mut Context<'_, Self>) {
        self.kind = kind;
        self.reauthorization_failed = false;
        cx.notify();
    }

    pub(crate) fn show_reauthorization_failed(&mut self, cx: &mut Context<'_, Self>) {
        self.reauthorization_failed = true;
        cx.notify();
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
            DocumentRecoveryKind::Unavailable => (
                "document-recovery-unavailable",
                Message::EditorRecoveryUnavailable,
            ),
        };
        let mut view = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .track_focus(&self.focus_handle)
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .debug_selector(|| selector.to_owned())
            .child(language.text(message));
        if self.kind == DocumentRecoveryKind::Authorization {
            if self.reauthorization_failed {
                view = view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .debug_selector(|| "document-recovery-reauthorization-failed".to_owned())
                        .child(language.text(Message::EditorReauthorizationFailed)),
                );
            }
            view = view.child(
                Button::new("document-recovery-reauthorize")
                    .debug_selector(|| "document-recovery-reauthorize".to_owned())
                    .label(language.text(Message::EditorReauthorize))
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(DocumentRecoveryEvent::Reauthorize);
                    })),
            );
        }
        view
    }
}
