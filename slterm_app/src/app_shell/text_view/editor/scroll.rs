//! 编辑器滚动定位：光标可见性、逻辑行揭示与软换行换算。

use ropey::LineType;

use super::super::metrics::DisplayMap;
use super::super::source::RopeTextSource;
use super::super::wrap::CaretAffinity;
use super::DocumentTextView;
use super::fold;

impl DocumentTextView {
    /// 逻辑行 → 当前模式的滚动行：软换行下必须换算为显示行；
    /// 索引未就绪时登记待揭示行，由索引完成回投统一消费。
    pub(super) fn reveal_logical_line(&mut self, line: usize) {
        let line = line.min(self.text.len_lines(LineType::LF).saturating_sub(1));
        // 目标行落在折叠区内时先展开，禁止把视口滚到不可见行。
        self.unfold_line(line);
        if self.wrap {
            let row = self.wrap_index.borrow().as_ref().and_then(|index| {
                let byte = self.text.line_to_byte_idx(line, LineType::LF);
                index
                    .position_with_affinity(byte, CaretAffinity::AfterWrap)
                    .map(|position| position.row)
            });
            let Some(row) = row else {
                self.wrap_reveal_line = Some(line);
                return;
            };
            self.scroll_line = fold::visible_row(row, &self.hidden_rows()).unwrap_or(0);
        } else {
            self.scroll_line = fold::visible_row(line, &self.hidden_rows()).unwrap_or(0);
        }
        self.scroll_column = 0;
        self.persist_view_state();
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        if self.wrap && self.wrap_index.borrow().is_none() {
            self.wrap_reveal_cursor = true;
            return;
        }
        self.unfold_line(self.logical_line_of_byte(self.selection.head));
        let previous = (self.scroll_line, self.scroll_column);
        let Some(metrics) = self.metrics() else {
            return;
        };
        let hidden = self.hidden_rows();
        let raw = if self.wrap {
            let Some(position) = self.wrap_position(self.selection.head) else {
                return;
            };
            position.row
        } else {
            self.text
                .byte_to_line_idx(self.selection.head, LineType::LF)
        };
        let Some(row) = fold::visible_row(raw, &hidden) else {
            return;
        };
        let visible_rows = (metrics.bounds.size.height / metrics.line_height)
            .floor()
            .max(1.0) as usize;
        if row < self.scroll_line {
            self.scroll_line = row;
        } else if row >= self.scroll_line.saturating_add(visible_rows) {
            self.scroll_line = row.saturating_add(1).saturating_sub(visible_rows);
        }
        if self.wrap {
            self.scroll_column = 0;
        } else {
            let map = DisplayMap::new(4);
            let Some(range) = RopeTextSource::new(&self.text, map).line_byte_range(raw) else {
                return;
            };
            let line_start = range.start;
            let Some(column) =
                map.column_at_rope(self.text.slice(range), self.selection.head - line_start)
            else {
                return;
            };
            let visible_columns = metrics.visible_whole_columns();
            if column < self.scroll_column {
                self.scroll_column = column;
            } else if column > self.scroll_column.saturating_add(visible_columns) {
                self.scroll_column = column - visible_columns;
            }
        }
        if previous != (self.scroll_line, self.scroll_column) {
            self.persist_view_state();
        }
    }
}
