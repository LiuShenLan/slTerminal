//! 软换行索引的后台调度与回投；只由 prepaint 的实际宽度启动。

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Task};
use ropey::Rope;
use slterm_document::TextStateId;

use super::super::wrap::SoftWrapIndex;
use super::DocumentTextView;

pub(super) type SharedWrapIndex = Rc<RefCell<Option<SoftWrapIndex>>>;
pub(super) type SharedWrapBuild = Rc<RefCell<Option<(TextStateId, usize, Task<()>)>>>;

/// 只有 prepaint 的实际宽度能启动或确认构建；完成回投再唤醒下一帧。
pub(super) fn schedule_wrap_index(
    text: Rope,
    text_state: TextStateId,
    wrap_columns: usize,
    wrap_index: SharedWrapIndex,
    wrap_build: SharedWrapBuild,
    target: gpui::WeakEntity<DocumentTextView>,
    cx: &App,
) {
    if wrap_index
        .borrow()
        .as_ref()
        .is_some_and(|index| index.matches(text_state, wrap_columns))
    {
        *wrap_build.borrow_mut() = None;
        return;
    }
    *wrap_index.borrow_mut() = None;
    if wrap_build
        .borrow()
        .as_ref()
        .is_some_and(|(state, columns, _)| *state == text_state && *columns == wrap_columns)
    {
        return;
    }
    let building = cx
        .background_executor()
        .spawn(async move { SoftWrapIndex::new(&text, text_state, wrap_columns) });
    let completed_build = wrap_build.clone();
    let task = cx.spawn(async move |cx| {
        let index = building.await;
        let _ = target.update(cx, |view, cx| {
            let current_width = view
                .metrics()
                .map(|metrics| metrics.visible_whole_columns().max(1));
            if !view.wrap || view.text_state != text_state || current_width != Some(wrap_columns) {
                return;
            }
            *view.wrap_index.borrow_mut() = Some(index);
            *completed_build.borrow_mut() = None;
            if let Some(line) = view.wrap_reveal_line.take() {
                view.reveal_logical_line(line);
            } else if std::mem::take(&mut view.wrap_reveal_cursor) {
                view.ensure_cursor_visible();
            }
            cx.notify();
        });
    });
    *wrap_build.borrow_mut() = Some((text_state, wrap_columns, task));
}
