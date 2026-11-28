//! Editor 的系统输入合同：UTF-16 映射、IME 组合与光标几何。

use std::ops::Range;

use gpui::{
    App, Bounds, Context, EntityInputHandler, FocusHandle, Focusable, Pixels, Point,
    UTF16Selection, Window, point, px, size,
};
use ropey::LineType;

use super::super::metrics::{DisplayMap, rope_is_grapheme_boundary};
use super::super::source::{RopeTextSource, TextSelection};
use super::DocumentTextView;
use super::fold;

impl DocumentTextView {
    pub(super) fn utf16_to_byte(&self, offset: usize) -> Option<usize> {
        if offset > self.text.len_utf16() {
            return None;
        }
        let byte = self.text.utf16_to_byte_idx(offset);
        (self.text.byte_to_utf16_idx(byte) == offset).then_some(byte)
    }

    pub(super) fn bytes_from_utf16(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start > range.end {
            return None;
        }
        let start = self.utf16_to_byte(range.start)?;
        let end = self.utf16_to_byte(range.end)?;
        if start > end
            || !rope_is_grapheme_boundary(&self.text, start)
            || !rope_is_grapheme_boundary(&self.text, end)
        {
            return None;
        }
        Some(start..end)
    }

    pub(super) fn range_to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text.byte_to_utf16_idx(range.start)..self.text.byte_to_utf16_idx(range.end)
    }
}

impl Focusable for DocumentTextView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Drop for DocumentTextView {
    fn drop(&mut self) {
        // 任务 future 也持任务槽；先清槽打破自持环并取消在途构建。
        *self.wrap_build.borrow_mut() = None;
        if self.view_state_dirty {
            self.persist_view_state();
        }
    }
}

impl EntityInputHandler for DocumentTextView {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<'_, Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(self.selection.range()),
            reversed: self.selection.anchor > self.selection.head,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<'_, Self>) -> Option<Range<usize>> {
        self.marked_range
            .clone()
            .map(|range| self.range_to_utf16(range))
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<'_, Self>,
    ) -> Option<String> {
        let range = self.bytes_from_utf16(range_utf16)?;
        *adjusted_range = Some(self.range_to_utf16(range.clone()));
        Some(self.text.slice(range).to_string())
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.replace(range, text, None, false, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.replace(range, new_text, new_selected_range, true, cx);
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.marked_range = None;
        cx.notify();
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(range) = self.bytes_from_utf16(range) else {
            return;
        };
        self.selection = TextSelection {
            anchor: range.start,
            head: range.end,
        };
        self.caret_affinity = super::super::wrap::CaretAffinity::AfterWrap;
        self.completion = None;
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> Option<usize> {
        Some(self.text.len_utf16())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<'_, Self>) -> bool {
        !self.read_only
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<'_, Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.bytes_from_utf16(range_utf16)?;
        let byte = range.start;
        let row = self.text.byte_to_line_idx(byte, LineType::LF);
        let metrics = self.metrics().unwrap_or(super::LayoutMetrics {
            bounds: element_bounds,
            cell_width: px(8.0),
            line_height: px(20.0),
        });
        let hidden = self.hidden_rows();
        if self.wrap {
            let position = self.wrap_position(byte)?;
            let visible = fold::visible_row(position.row, &hidden)?;
            let visible_row = visible.checked_sub(self.clamped_scroll_line())?;
            let x = (metrics.bounds.origin.x + metrics.cell_width * position.column as f32)
                .max(metrics.bounds.origin.x)
                .min(metrics.bounds.origin.x + metrics.bounds.size.width);
            return Some(Bounds {
                origin: point(
                    x,
                    metrics.bounds.origin.y + metrics.line_height * visible_row as f32,
                ),
                size: size(px(1.0), metrics.line_height),
            });
        }
        let visible = fold::visible_row(row, &hidden)?;
        let visible_row = visible.checked_sub(self.scroll_line)?;
        let line = RopeTextSource::new(&self.text, DisplayMap::new(4))
            .visible_line_windows(
                row..row + 1,
                self.scroll_column..self.scroll_column.saturating_add(metrics.visible_columns()),
            )
            .into_iter()
            .next()?;
        let range = line.byte_range();
        let x = if byte < range.start {
            metrics.bounds.origin.x
        } else if byte > range.end {
            metrics.bounds.origin.x + metrics.bounds.size.width
        } else {
            let column = DisplayMap::new(4).column_at_from(
                line.text(),
                byte - range.start,
                line.display_start(),
            )?;
            metrics.bounds.origin.x
                + metrics.cell_width * (column as f32 - self.scroll_column as f32)
        };
        let x = x
            .max(metrics.bounds.origin.x)
            .min(metrics.bounds.origin.x + metrics.bounds.size.width);
        Some(Bounds {
            origin: point(
                x,
                metrics.bounds.origin.y + metrics.line_height * visible_row as f32,
            ),
            size: size(px(1.0), metrics.line_height),
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<'_, Self>,
    ) -> Option<usize> {
        let byte = self.byte_for_point(point)?;
        Some(self.text.byte_to_utf16_idx(byte))
    }
}
