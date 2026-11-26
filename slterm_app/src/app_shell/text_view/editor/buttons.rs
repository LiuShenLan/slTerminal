//! Document 视图底栏按钮：语言、自动换行、行尾与手动只读。

use gpui::{Context, InteractiveElement as _};
use gpui_component::Sizable as _;
use gpui_component::button::{Button, ButtonVariants as _};

use slterm_document::LanguageId;

use super::{DocumentTextEvent, DocumentTextView};

impl DocumentTextView {
    pub(super) fn language_selector(&self, cx: &mut Context<'_, Self>) -> Option<Button> {
        if !self.allow_language_override {
            return None;
        }
        let id = format!("document-language-selector-{}", cx.entity().entity_id());
        let selector = id.clone();
        let effective = self.language_override.unwrap_or(self.automatic_language);
        Some(
            Button::new(id)
                .debug_selector(move || selector.clone())
                .label(effective.as_str())
                .ghost()
                .small()
                .on_click(cx.listener(|view, event: &gpui::ClickEvent, _, cx| {
                    view.open_language_selector(event.position(), cx);
                })),
        )
    }

    pub(super) fn wrap_button(&self, cx: &mut Context<'_, Self>) -> impl gpui::IntoElement + use<> {
        let id = format!("document-wrap-toggle-{}", cx.entity().entity_id());
        let selector = id.clone();
        let label = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(crate::i18n::Message::EditorWrap)
            .to_owned();
        Button::new(id)
            .debug_selector(move || selector.clone())
            .label(label)
            .ghost()
            .small()
            .on_click(cx.listener(|view, _, _, cx| view.toggle_wrap(cx)))
    }

    pub(super) fn line_ending_button(&self, cx: &mut Context<'_, Self>) -> Option<Button> {
        if !self.allow_language_override || self.inherently_read_only {
            return None;
        }
        let id = format!("document-line-ending-{}", cx.entity().entity_id());
        let selector = id.clone();
        let label = match self.encoding.line_ending {
            slterm_document::LineEnding::Lf => "LF",
            slterm_document::LineEnding::Crlf => "CRLF",
        };
        Some(
            Button::new(id)
                .debug_selector(move || selector.clone())
                .label(label)
                .ghost()
                .small()
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(DocumentTextEvent::ToggleLineEnding);
                })),
        )
    }

    pub(super) fn read_only_button(&self, cx: &mut Context<'_, Self>) -> Option<Button> {
        if !self.allow_language_override || self.inherently_read_only {
            return None;
        }
        let id = format!("document-read-only-{}", cx.entity().entity_id());
        let selector = id.clone();
        let message = if self.manual_read_only {
            crate::i18n::Message::EditorEditable
        } else {
            crate::i18n::Message::EditorReadOnly
        };
        let label = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(message)
            .to_owned();
        Some(
            Button::new(id)
                .debug_selector(move || selector.clone())
                .label(label)
                .ghost()
                .small()
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(DocumentTextEvent::ToggleManualReadOnly);
                })),
        )
    }

    pub(super) fn system_open_button(&self, cx: &mut Context<'_, Self>) -> Option<Button> {
        if self.automatic_language != LanguageId::Html {
            return None;
        }
        let id = format!("document-system-open-{}", cx.entity().entity_id());
        let selector = id.clone();
        let label = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(crate::i18n::Message::EditorOpenSystem)
            .to_owned();
        Some(
            Button::new(id)
                .debug_selector(move || selector.clone())
                .label(label)
                .ghost()
                .small()
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(DocumentTextEvent::OpenInSystem);
                })),
        )
    }
}
