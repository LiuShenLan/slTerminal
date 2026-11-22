//! 终端普通文本搜索：瞬态查询、worker 只读匹配与本地滚动。

use gpui::{AppContext as _, Context, KeyDownEvent};
use slterm_terminal::event_loop::Msg;
use slterm_terminal::grid::Scroll;
use slterm_terminal::term::search::Match;

use super::TerminalView;

#[derive(Default)]
pub(crate) struct TerminalSearchState {
    pub query: String,
    pub matches: Vec<Match>,
    pub selected: Option<usize>,
    generation: u64,
}

impl TerminalSearchState {
    pub(super) fn invalidate(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("终端搜索代次不得回绕");
        self.matches.clear();
        self.selected = None;
    }
}

impl TerminalView {
    pub(super) fn open_terminal_search(&mut self, cx: &mut Context<'_, Self>) {
        if self.terminal_search.is_none() {
            self.terminal_search = Some(TerminalSearchState::default());
        }
        self.suggestion_task = None;
        self.suggest.clear_completion_hints();
        cx.notify();
    }

    fn close_terminal_search(&mut self, cx: &mut Context<'_, Self>) {
        self.terminal_search = None;
        self.terminal_search_task = None;
        cx.notify();
    }

    /// 返回 true 表示搜索上下文接管；普通文本不 stop，让 IME 正常提交。
    pub(super) fn handle_terminal_search_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if modifiers.control || modifiers.alt {
            return false;
        }
        match keystroke.key.as_str() {
            "escape" => self.close_terminal_search(cx),
            "backspace" => {
                if let Some(search) = &mut self.terminal_search
                    && search.query.pop().is_some()
                {
                    self.refresh_terminal_search(cx);
                }
            }
            "enter" | "tab" | "down" | "up" => {
                let backwards = modifiers.shift || keystroke.key == "up";
                self.move_terminal_search(if backwards { -1 } else { 1 }, cx);
            }
            _ => return true,
        }
        cx.stop_propagation();
        true
    }

    pub(super) fn terminal_search_input(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        let text: String = text.chars().filter(|ch| !ch.is_control()).collect();
        if text.is_empty() {
            return;
        }
        if let Some(search) = &mut self.terminal_search {
            search.query.push_str(&text);
            self.refresh_terminal_search(cx);
        }
        cx.notify();
    }

    /// 新查询或新帧都先作废旧位置；只有同代次 worker 回包可回填。
    pub(super) fn refresh_terminal_search(&mut self, cx: &mut Context<'_, Self>) {
        self.terminal_search_task = None;
        let Some(search) = &mut self.terminal_search else {
            return;
        };
        search.invalidate();
        let generation = search.generation;
        let query = search.query.clone();
        if query.is_empty() {
            cx.notify();
            return;
        }
        let (reply, response) = std::sync::mpsc::sync_channel(1);
        if self
            .send_worker_command(Msg::SearchLiteral { query, reply })
            .is_err()
        {
            cx.notify();
            return;
        }
        let calculation = cx.background_spawn(async move { response.recv() });
        self.terminal_search_task = Some(cx.spawn(async move |this, cx| {
            let result = calculation.await;
            let _ = this.update(cx, |view, cx| {
                let Some(search) = &mut view.terminal_search else {
                    return;
                };
                if search.generation != generation {
                    return;
                }
                view.terminal_search_task = None;
                let Ok(matches) = result else {
                    return;
                };
                search.matches = matches;
                search.selected = (!search.matches.is_empty()).then_some(0);
                view.scroll_to_terminal_match(cx);
                cx.notify();
            });
        }));
    }

    fn move_terminal_search(&mut self, delta: isize, cx: &mut Context<'_, Self>) {
        let Some(search) = &mut self.terminal_search else {
            return;
        };
        let len = search.matches.len();
        if len == 0 {
            return;
        }
        let current = search.selected.unwrap_or(0);
        search.selected = Some((current as isize + delta).rem_euclid(len as isize) as usize);
        self.scroll_to_terminal_match(cx);
        cx.notify();
    }

    fn scroll_to_terminal_match(&mut self, cx: &mut Context<'_, Self>) {
        let Some(frame) = &self.frame else {
            return;
        };
        let Some(matched) = self
            .terminal_search
            .as_ref()
            .and_then(|search| search.selected.and_then(|index| search.matches.get(index)))
        else {
            return;
        };
        let top = -(frame.render().display_offset as i32);
        let bottom = top + i32::from(frame.render().rows) - 1;
        let line = matched.start().line.0;
        let delta = if line < top {
            top - line
        } else if line > bottom {
            bottom - line
        } else {
            0
        };
        if delta != 0 {
            let _ = self.send_worker_command(Msg::Scroll(Scroll::Delta(delta)));
        }
        cx.notify();
    }
}
