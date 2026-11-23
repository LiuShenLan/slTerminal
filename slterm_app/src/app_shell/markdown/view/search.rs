//! 渲染可见文本查找；输入、匹配与当前项均为瞬态。

use gpui::{AppContext as _, Context, Entity, Focusable as _, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use slterm_document::MarkdownSearchMatch;

use super::MarkdownView;

pub(super) struct MarkdownSearch {
    pub(super) input: Entity<InputState>,
    pub(super) matches: Vec<MarkdownSearchMatch>,
    pub(super) active: Option<usize>,
    _subscription: Subscription,
}

impl MarkdownView {
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.effective_mode() != super::super::progressive::MarkdownRenderDecision::Progressive {
            cx.propagate();
            return;
        }
        if let Some(search) = &self.search {
            search.input.read(cx).focus_handle(cx).focus(window, cx);
            return;
        }
        let placeholder = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(crate::i18n::Message::EditorSearchPlaceholder)
            .to_owned();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe_in(&input, window, |view, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                view.refresh_search(cx);
            }
        });
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.search = Some(MarkdownSearch {
            input,
            matches: Vec::new(),
            active: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(super) fn refresh_search(&mut self, cx: &mut Context<'_, Self>) {
        let target = self.search.as_mut().and_then(|search| {
            let query = search.input.read(cx).value();
            search.matches = if self.document_revision == Some(self.revision) {
                self.document
                    .as_deref()
                    .map(|document| document.find_literal(&query))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            search.active = (!search.matches.is_empty()).then_some(0);
            search.matches.first().map(|matched| matched.block())
        });
        if let Some(target) = target {
            self.scroll_to_block(target);
        }
        cx.notify();
    }

    pub(super) fn move_search(&mut self, forward: bool, cx: &mut Context<'_, Self>) {
        let target = self.search.as_mut().and_then(|search| {
            let count = search.matches.len();
            if count == 0 {
                search.active = None;
                return None;
            }
            let current = search.active.unwrap_or(if forward { count - 1 } else { 0 });
            let next = if forward {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            };
            search.active = Some(next);
            search.matches.get(next).map(|matched| matched.block())
        });
        if let Some(target) = target {
            self.scroll_to_block(target);
        }
        cx.notify();
    }

    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.search.take().is_none() {
            cx.propagate();
            return;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
}
