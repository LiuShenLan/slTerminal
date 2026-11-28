//! 跳转行栏：从 1 开始的行号，越界夹取并在栏内说明实际范围。
//!
//! 输入、提示与栏开关全部瞬态；跳转只改选区与滚动，不写 Document。

use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Styled as _, Subscription, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::h_flex;
use gpui_component::input::{Input, InputEvent, InputState};
use ropey::LineType;

use super::super::TEXT_VIEW_GOTO_CONTEXT;
use super::super::source::TextSelection;
use super::DocumentTextView;
use crate::app_shell::config::Settings;
use crate::i18n::Message;

pub(super) struct GoToLineState {
    input: Entity<InputState>,
    /// 已渲染的封闭提示；成功且未夹取时为空。
    message: Option<String>,
    _subscription: Subscription,
}

impl DocumentTextView {
    /// Ctrl+Alt+G：打开或还焦跳转行栏。
    pub(super) fn open_goto(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(goto) = &self.goto {
            goto.input.read(cx).focus_handle(cx).focus(window, cx);
            return;
        }
        let language = cx.global::<Settings>().language;
        let placeholder = language.text(Message::EditorGoToLinePlaceholder).to_owned();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |view, _, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.commit_goto(cx);
                }
            },
        );
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.goto = Some(GoToLineState {
            input,
            message: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// 解析并跳转；非法输入只显示提示，越界夹取后说明 1..=max。
    pub(super) fn commit_goto(&mut self, cx: &mut Context<'_, Self>) {
        let Some(goto) = &self.goto else {
            return;
        };
        let language = cx.global::<Settings>().language;
        let value = goto.input.read(cx).value().trim().to_owned();
        if value.is_empty() {
            self.goto.as_mut().expect("栏必须存在").message = None;
            cx.notify();
            return;
        }
        let Ok(requested) = value.parse::<u128>() else {
            self.goto.as_mut().expect("栏必须存在").message =
                Some(language.text(Message::EditorGoToLineInvalid).to_owned());
            cx.notify();
            return;
        };
        let max = self.text.len_lines(LineType::LF).max(1);
        let line = usize::try_from(requested)
            .unwrap_or(usize::MAX)
            .clamp(1, max);
        let clamped = line as u128 != requested;
        self.unfold_line(line.saturating_sub(1));
        self.reveal_logical_line(line.saturating_sub(1));
        let byte = self.text.line_to_byte_idx(line - 1, LineType::LF);
        self.selection = TextSelection {
            anchor: byte,
            head: byte,
        };
        self.preferred_column = None;
        self.caret_affinity = super::super::wrap::CaretAffinity::AfterWrap;
        self.completion = None;
        self.ensure_cursor_visible();
        let message = clamped.then(|| {
            language.format(
                Message::EditorGoToLineClamped,
                &[("line", &line.to_string()), ("max", &max.to_string())],
            )
        });
        self.goto.as_mut().expect("栏必须存在").message = message;
        cx.notify();
    }

    pub(super) fn close_goto(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.goto.take().is_none() {
            return;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// 跳转栏 UI：输入框 + 封闭提示，稳定 selector 供页面级测试定位。
    pub(super) fn goto_bar(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let goto = self.goto.as_ref()?;
        let mut bar = h_flex()
            .id("editor-goto-bar")
            .debug_selector(|| "editor-goto-bar".to_owned())
            .key_context(TEXT_VIEW_GOTO_CONTEXT)
            .gap_2()
            .items_center()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id("editor-goto-input")
                    .debug_selector(|| "editor-goto-input".to_owned())
                    .w(px(160.0))
                    .child(Input::new(&goto.input)),
            );
        if let Some(message) = &goto.message {
            bar = bar.child(
                div()
                    .id("editor-goto-message")
                    .debug_selector(|| "editor-goto-message".to_owned())
                    .text_xs()
                    .text_color(cx.theme().warning_foreground)
                    .child(message.clone()),
            );
        }
        Some(bar.into_any_element())
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl DocumentTextView {
    pub(super) fn goto_message_for_test(&self) -> Option<String> {
        self.goto.as_ref().and_then(|goto| goto.message.clone())
    }

    pub(super) fn goto_input_for_test(&self) -> Option<Entity<InputState>> {
        self.goto.as_ref().map(|goto| goto.input.clone())
    }
}
