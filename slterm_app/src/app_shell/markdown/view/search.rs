//! 渲染可见文本查找；输入、匹配与当前项均为瞬态。

use gpui::{AppContext as _, Context, Entity, Focusable as _, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use slterm_document::MarkdownSearchMatch;

use super::MarkdownView;

pub(super) struct MarkdownSearch {
    pub(super) input: Entity<InputState>,
    pub(super) matches: Vec<MarkdownSearchMatch>,
    pub(super) active: Option<usize>,
    generation: u64,
    task: Option<gpui::Task<()>>,
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
            generation: 0,
            task: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(super) fn refresh_search(&mut self, cx: &mut Context<'_, Self>) {
        let document = (self.document_revision == Some(self.revision))
            .then(|| self.document.clone())
            .flatten();
        let Some(search) = self.search.as_mut() else {
            return;
        };
        let query = search.input.read(cx).value();
        search.matches.clear();
        search.active = None;
        search.generation = search.generation.wrapping_add(1);
        let generation = search.generation;
        let Some(document) = document.filter(|_| !query.is_empty()) else {
            search.task = None;
            cx.notify();
            return;
        };

        // 匹配扫描可能覆盖整篇可见文本，必须在后台执行；回执按查询与代次对账。
        let executor = cx.background_executor().clone();
        let background_query = query.clone();
        let task = cx.spawn(async move |this, cx| {
            let matches = executor
                .spawn(async move { document.find_literal(&background_query) })
                .await;
            let _ = this.update(cx, |view, cx| {
                let current = view.search.as_ref().is_some_and(|search| {
                    search.generation == generation && search.input.read(cx).value() == query
                });
                if !current {
                    return;
                }
                let target = {
                    let Some(search) = view.search.as_mut() else {
                        return;
                    };
                    search.matches = matches;
                    search.active = (!search.matches.is_empty()).then_some(0);
                    search.matches.first().map(|matched| matched.block())
                };
                if let Some(target) = target {
                    view.scroll_to_block(target, cx);
                }
                cx.notify();
            });
        });
        search.task = Some(task);
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
            self.scroll_to_block(target, cx);
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
