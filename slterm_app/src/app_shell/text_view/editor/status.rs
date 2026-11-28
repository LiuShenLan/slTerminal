//! Editor 状态条：语言、编码／BOM、行尾与只读原因。
//!
//! 状态条只读当前视图元数据；编码与只读原因不得另建运行态副本。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    Styled as _, div,
};
use gpui_component::{ActiveTheme as _, h_flex};
use slterm_document::{DocumentEncoding, LineEnding};

use super::DocumentTextView;
use crate::app_shell::config::Settings;
use crate::i18n::Message;

impl DocumentTextView {
    fn encoding_label(&self) -> &'static str {
        match (self.encoding.encoding, self.encoding.bom) {
            (DocumentEncoding::Utf8, false) => "UTF-8",
            (DocumentEncoding::Utf8, true) => "UTF-8 BOM",
            (DocumentEncoding::Utf16Le, true) => "UTF-16 LE BOM",
            (DocumentEncoding::Utf16Le, false) => "UTF-16 LE",
            (DocumentEncoding::Utf16Be, true) => "UTF-16 BE BOM",
            (DocumentEncoding::Utf16Be, false) => "UTF-16 BE",
            (DocumentEncoding::Gbk, _) => "GBK",
            (DocumentEncoding::Gb18030, _) => "GB18030",
        }
    }

    fn line_ending_label(&self) -> &'static str {
        match self.encoding.line_ending {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
        }
    }

    /// 状态条左侧信息；可编辑时没有只读原因条目。
    pub(super) fn status_bar(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let language = cx.global::<Settings>().language;
        let effective = self.language_override.unwrap_or(self.automatic_language);
        let entity = cx.entity().entity_id();
        let mut row = h_flex()
            .id("editor-status-bar")
            .debug_selector(|| "editor-status-bar".to_owned())
            .gap_3()
            .items_center()
            .min_w_0()
            .text_xs()
            .text_color(cx.theme().muted_foreground);
        for (name, value) in [
            ("language", effective.as_str().to_owned()),
            ("encoding", self.encoding_label().to_owned()),
            ("line-ending", self.line_ending_label().to_owned()),
        ] {
            let message = match name {
                "language" => Message::EditorStatusLanguage,
                "encoding" => Message::EditorStatusEncoding,
                _ => Message::EditorStatusLineEnding,
            };
            let text = language.format(message, &[("value", &value)]);
            let selector = format!("editor-status-{name}-{entity}");
            row = row.child(
                div()
                    .id(format!("editor-status-{name}"))
                    .debug_selector(move || selector.clone())
                    .child(text),
            );
        }
        if self.read_only {
            let message = if self.manual_read_only {
                Message::EditorStatusReadOnlyManual
            } else {
                Message::EditorStatusReadOnlyInherent
            };
            let selector = format!("editor-status-read-only-{entity}");
            row = row.child(
                div()
                    .id("editor-status-read-only")
                    .debug_selector(move || selector.clone())
                    .text_color(cx.theme().warning_foreground)
                    .child(language.text(message)),
            );
        }
        div().flex_1().min_w_0().child(row).into_any_element()
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl DocumentTextView {
    /// 与渲染使用同一字段与 typed 文案，供视图级断言值集合。
    pub(super) fn status_texts_for_test(&self, cx: &gpui::App) -> Vec<String> {
        let language = cx.global::<Settings>().language;
        let effective = self.language_override.unwrap_or(self.automatic_language);
        let mut texts = vec![
            language.format(
                Message::EditorStatusLanguage,
                &[("value", effective.as_str())],
            ),
            language.format(
                Message::EditorStatusEncoding,
                &[("value", self.encoding_label())],
            ),
            language.format(
                Message::EditorStatusLineEnding,
                &[("value", self.line_ending_label())],
            ),
        ];
        if self.read_only {
            let message = if self.manual_read_only {
                Message::EditorStatusReadOnlyManual
            } else {
                Message::EditorStatusReadOnlyInherent
            };
            texts.push(language.text(message).to_owned());
        }
        texts
    }
}
