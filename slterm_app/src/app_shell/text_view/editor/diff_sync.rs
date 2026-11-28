// 并排 Diff 的滚动、折叠区间与显示行过滤；只改显示瞬态，不改正文。

use std::ops::Range;

use gpui::Context;

use super::super::wrap::CaretAffinity;
use super::fold;
use super::*;

impl DocumentTextView {
    /// 并排 Diff 的对侧滚动：只改视口，不回写本视图持久状态。
    pub(crate) fn set_scroll_line_from_peer(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        let clamped = line.min(self.display_row_count().unwrap_or(line).saturating_sub(1));
        if self.scroll_line != clamped {
            self.scroll_line = clamped;
            cx.notify();
        }
    }

    /// 测试与同步入口共用同一滚动行语义。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn scroll_to_line_for_test(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        let previous = self.scroll_line;
        self.scroll_line = line.min(self.display_row_count().unwrap_or(line).saturating_sub(1));
        if self.scroll_line != previous {
            cx.emit(DocumentTextEvent::Scrolled(self.scroll_line));
        }
        cx.notify();
    }

    /// 折叠后的原始行总数：非换行即逻辑行，换行为折行显示行。
    pub(super) fn raw_row_count(&self) -> Option<usize> {
        if self.wrap {
            self.wrap_index
                .borrow()
                .as_ref()
                .map(|index| index.display_row_count())
        } else {
            Some(self.text.len_lines(LineType::LF))
        }
    }

    pub(super) fn hidden_rows(&self) -> Vec<Range<usize>> {
        let Some(total) = self.raw_row_count() else {
            return Vec::new();
        };
        let mut hidden = self.fold_state.hidden_ranges(
            self.wrap,
            self.wrap_index.borrow().as_ref(),
            &self.text,
            total,
        );
        // Diff 折叠区间以原始行表示；软换行下与语法折叠共用显示行过滤。
        if !self.diff_collapsed.is_empty() {
            let mut extra = self
                .diff_collapsed
                .iter()
                .filter_map(|range| {
                    if self.wrap {
                        let index = self.wrap_index.borrow();
                        let index = index.as_ref()?;
                        let byte = (range.start < total)
                            .then(|| self.text.line_to_byte_idx(range.start, LineType::LF))?;
                        let first = index
                            .position_with_affinity(byte, CaretAffinity::AfterWrap)
                            .map(|position| position.row)?;
                        let end_byte = (range.end < total)
                            .then(|| self.text.line_to_byte_idx(range.end, LineType::LF));
                        let end = end_byte
                            .and_then(|byte| {
                                index
                                    .position_with_affinity(byte, CaretAffinity::AfterWrap)
                                    .map(|position| position.row)
                            })
                            .unwrap_or(total);
                        Some(first.min(total)..end.min(total))
                    } else {
                        Some(range.start.min(total)..range.end.min(total))
                    }
                })
                .filter(|range| range.start < range.end)
                .collect::<Vec<_>>();
            extra.sort_by_key(|range| range.start);
            hidden.extend(extra);
            hidden.sort_by_key(|range| range.start);
        }
        fold::merge_ranges(hidden)
    }

    /// Diff 复合视图设置未变更区域折叠区间；只影响显示，不改正文。
    pub(crate) fn set_diff_collapsed_ranges(
        &mut self,
        ranges: Vec<Range<usize>>,
        cx: &mut Context<'_, Self>,
    ) {
        if self.diff_collapsed == ranges {
            return;
        }
        self.diff_collapsed = ranges;
        cx.notify();
    }
}
