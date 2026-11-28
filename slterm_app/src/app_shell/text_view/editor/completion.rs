//! 文档内标识符补全：前台扫描当前 Rope，不依赖外部语言服务。
//!
//! 超过正常编辑上限（10,000,000 字节）的文档关闭补全；候选与选中项
//! 全部瞬态，不进入 ViewState。接受候选取当前词前缀并经唯一编辑事务提交。

use std::collections::BTreeSet;
use std::ops::Range;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    Styled as _, div, px,
};
use gpui_component::{ActiveTheme as _, v_flex};
use slterm_document::NORMAL_EDIT_LIMIT_BYTES;

use super::DocumentTextView;

const MAX_CANDIDATES: usize = 100;
const VISIBLE_CANDIDATES: usize = 8;

pub(super) struct CompletionState {
    candidates: Vec<String>,
    selected: usize,
    /// 被替换的当前词前缀；接受后插入候选。
    prefix: Range<usize>,
}

fn is_identifier_char(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn identifier_start(character: char) -> bool {
    !character.is_ascii_digit()
}

/// 光标前的当前词前缀；非标识符字符或空前缀返回 None。
/// 单步向前找一个 UTF-8 字符起点；只检查边界前的至多四个字节。
fn previous_char_start(text: &ropey::Rope, byte: usize) -> Option<usize> {
    for start in (byte.saturating_sub(4)..byte).rev() {
        if !text.is_char_boundary(start) {
            continue;
        }
        let Ok(character) = text.get_char(start) else {
            continue;
        };
        if start + character.len_utf8() == byte {
            return Some(start);
        }
    }
    None
}

fn word_prefix(text: &ropey::Rope, cursor: usize) -> Option<Range<usize>> {
    let mut start = cursor;
    while let Some(previous) = previous_char_start(text, start) {
        let character = text.get_char(previous).ok()?;
        if !is_identifier_char(character) {
            break;
        }
        start = previous;
    }
    if start == cursor {
        return None;
    }
    let first = text.get_char(start).ok()?;
    identifier_start(first).then_some(start..cursor)
}

/// 全文标识符去重后按字典序；排除与前缀相同且不足两字符的词。
fn collect_candidates(text: &ropey::Rope, prefix: &str, current: Range<usize>) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    let mut word_start: Option<usize> = None;
    for (byte, character) in text.char_indices() {
        if is_identifier_char(character) {
            if word_start.is_none() {
                word_start = identifier_start(character).then_some(byte);
            }
        } else if let Some(start) = word_start.take() {
            push_candidate(text, start..byte, prefix, &current, &mut candidates);
        }
    }
    if let Some(start) = word_start {
        push_candidate(text, start..text.len(), prefix, &current, &mut candidates);
    }
    candidates.into_iter().take(MAX_CANDIDATES).collect()
}

fn push_candidate(
    text: &ropey::Rope,
    range: Range<usize>,
    prefix: &str,
    current: &Range<usize>,
    candidates: &mut BTreeSet<String>,
) {
    if range == *current || range.end <= range.start {
        return;
    }
    let word = text.slice(range).to_string();
    if word.len() < 2 || word == prefix || !word.starts_with(prefix) {
        return;
    }
    candidates.insert(word);
}

impl DocumentTextView {
    /// Ctrl+Space：文档内标识符前缀补全；只读与超限文档不产生候选。
    pub(super) fn trigger_completion(&mut self, cx: &mut Context<'_, Self>) {
        if self.read_only
            || self.marked_range.is_some()
            || self.text.len() > NORMAL_EDIT_LIMIT_BYTES as usize
            || self.selection.range().start != self.selection.range().end
        {
            self.completion = None;
            cx.notify();
            return;
        }
        let Some(prefix) = word_prefix(&self.text, self.selection.head) else {
            self.completion = None;
            cx.notify();
            return;
        };
        let text = self.text.slice(prefix.clone()).to_string();
        let candidates = collect_candidates(&self.text, &text, prefix.clone());
        self.completion = (!candidates.is_empty()).then_some(CompletionState {
            candidates,
            selected: 0,
            prefix,
        });
        cx.notify();
    }

    /// 候选导航与光标导航共用 Up/Down；返回 true 表示事件已被候选消费。
    pub(super) fn move_completion(&mut self, forward: bool) -> bool {
        let Some(completion) = &mut self.completion else {
            return false;
        };
        let count = completion.candidates.len();
        if count == 0 {
            return false;
        }
        completion.selected = if forward {
            (completion.selected + 1) % count
        } else {
            (completion.selected + count - 1) % count
        };
        true
    }

    /// Tab／Enter 接受当前候选；替换当前词前缀并经编辑事务提交。
    pub(super) fn accept_completion(&mut self, cx: &mut Context<'_, Self>) -> bool {
        let Some(completion) = &self.completion else {
            return false;
        };
        let Some(candidate) = completion.candidates.get(completion.selected).cloned() else {
            return false;
        };
        let prefix = completion.prefix.clone();
        self.completion = None;
        self.edit_by_keyboard(prefix, &candidate, cx);
        true
    }

    pub(super) fn close_completion(&mut self, cx: &mut Context<'_, Self>) {
        if self.completion.take().is_some() {
            cx.notify();
        }
    }

    /// 补全弹层只展示候选，不接管焦点；键盘导航由动作处理。
    pub(super) fn completion_popup(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let completion = self.completion.as_ref()?;
        let (top, left) = self.completion_anchor().unwrap_or((px(8.0), px(8.0)));
        let first = completion
            .selected
            .saturating_sub(VISIBLE_CANDIDATES / 2)
            .min(
                completion
                    .candidates
                    .len()
                    .saturating_sub(VISIBLE_CANDIDATES),
            );
        let last = (first + VISIBLE_CANDIDATES).min(completion.candidates.len());
        let mut list = v_flex()
            .id("editor-completion-popup")
            .debug_selector(|| "editor-completion-popup".to_owned())
            .absolute()
            .top(top)
            .left(left)
            .min_w(px(180.0))
            .max_w(px(360.0))
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .shadow_md()
            .py_1();
        for index in first..last {
            let selected = index == completion.selected;
            let candidate = completion.candidates[index].clone();
            let id = format!("editor-completion-item-{index}");
            let selector = id.clone();
            list = list.child(
                div()
                    .id(id)
                    .debug_selector(move || selector.clone())
                    .px_2()
                    .py_0p5()
                    .text_sm()
                    .when(selected, |element| {
                        element
                            .bg(cx.theme().accent)
                            .text_color(cx.theme().accent_foreground)
                    })
                    .child(candidate.clone())
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            if let Some(completion) = &mut view.completion
                                && let Some(position) = completion
                                    .candidates
                                    .iter()
                                    .position(|item| item == &candidate)
                            {
                                completion.selected = position;
                                view.accept_completion(cx);
                            }
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        Some(list.into_any_element())
    }

    /// 弹层锚在光标下一显示行；布局尚未就绪时退回左上角。
    fn completion_anchor(&self) -> Option<(gpui::Pixels, gpui::Pixels)> {
        let metrics = self.metrics()?;
        let (row, column) = self.cursor_viewport_row_column()?;
        let top = metrics.bounds.origin.y + metrics.line_height * (row.saturating_add(1) as f32);
        let left = metrics.bounds.origin.x + metrics.cell_width * column as f32;
        Some((
            top,
            left.min(metrics.bounds.origin.x + metrics.bounds.size.width),
        ))
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl DocumentTextView {
    pub(super) fn completion_candidates_for_test(&self) -> Option<Vec<String>> {
        self.completion
            .as_ref()
            .map(|completion| completion.candidates.clone())
    }

    pub(super) fn completion_selected_for_test(&self) -> Option<usize> {
        self.completion
            .as_ref()
            .map(|completion| completion.selected)
    }
}
