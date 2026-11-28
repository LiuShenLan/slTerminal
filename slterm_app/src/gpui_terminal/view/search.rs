//! 终端普通文本搜索：瞬态查询、worker 只读匹配与本地滚动。

use gpui::{AppContext as _, Context, KeyDownEvent};
use slterm_terminal::event_loop::Msg;
use slterm_terminal::grid::Scroll;
use slterm_terminal::term::search::{Match, SEARCH_LITERAL_MATCH_LIMIT};

use super::TerminalView;

#[derive(Default)]
pub(crate) struct TerminalSearchState {
    pub query: String,
    pub matches: Vec<Match>,
    pub selected: Option<usize>,
    /// 结果达到上限时导航只在截断集合内循环；界面必须显示截断事实。
    pub truncated: bool,
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
        self.truncated = false;
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
        self.refresh_terminal_search_inner(false, cx);
    }

    /// 输出帧驱动的刷新：保留当前导航位置、不滚动，避免
    /// 「刷新 → 滚动 → 新帧 → 再刷新」的回环风暴；在途请求只让位于
    /// 下一次新帧，不取消重排。
    pub(super) fn refresh_terminal_search_from_frame(&mut self, cx: &mut Context<'_, Self>) {
        if self.terminal_search_task.is_some() {
            return;
        }
        self.refresh_terminal_search_inner(true, cx);
    }

    fn refresh_terminal_search_inner(
        &mut self,
        preserve_selection: bool,
        cx: &mut Context<'_, Self>,
    ) {
        self.terminal_search_task = None;
        let Some(search) = &mut self.terminal_search else {
            return;
        };
        let previous_selected = if preserve_selection {
            search.selected
        } else {
            None
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
                search.truncated = matches.len() >= SEARCH_LITERAL_MATCH_LIMIT;
                search.matches = matches;
                search.selected = if search.matches.is_empty() {
                    None
                } else if preserve_selection {
                    Some(previous_selected.unwrap_or(0).min(search.matches.len() - 1))
                } else {
                    Some(0)
                };
                if !preserve_selection {
                    view.scroll_to_terminal_match(cx);
                }
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
