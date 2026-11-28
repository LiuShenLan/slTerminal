//! 只读渲染文本的选区映射、可见高亮与复制。

use std::ops::Range;

use gpui::{App, Hsla, Pixels, Point, TextLayout, TextRun, Window};

use super::MarkdownView;

#[derive(Clone, Copy)]
pub(super) struct MarkdownSelection {
    anchor: usize,
    head: usize,
}

impl MarkdownSelection {
    fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

#[derive(Clone)]
pub(super) struct LayoutSegment {
    pub(super) local: Range<usize>,
    pub(super) visible: Range<usize>,
}

pub(super) struct TextLayoutRecord {
    layout: TextLayout,
    segments: Vec<LayoutSegment>,
}

impl MarkdownView {
    pub(super) fn register_text_layout(
        &mut self,
        id: String,
        layout: TextLayout,
        segments: Vec<LayoutSegment>,
    ) {
        self.text_layouts
            .borrow_mut()
            .insert(id, TextLayoutRecord { layout, segments });
    }

    fn hit_text(&self, id: &str, position: Point<Pixels>) -> Option<usize> {
        let layouts = self.text_layouts.borrow();
        let record = layouts.get(id)?;
        let local = match record.layout.index_for_position(position) {
            Ok(index) | Err(index) => index,
        };
        record.segments.iter().find_map(|segment| {
            (segment.local.start <= local && local <= segment.local.end)
                .then(|| segment.visible.start + (local - segment.local.start))
        })
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn text_layout_ids_for_test(&self) -> Vec<String> {
        let mut ids = self
            .text_layouts
            .borrow()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn selection_hit_for_test(
        &self,
        id: &str,
        position: Point<Pixels>,
    ) -> Option<usize> {
        self.hit_text(id, position)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn selection_position_for_test(
        &self,
        id: &str,
        index: usize,
    ) -> Option<Point<Pixels>> {
        self.text_layouts
            .borrow()
            .get(id)?
            .layout
            .position_for_index(index)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn selection_local_hit_for_test(
        &self,
        id: &str,
        position: Point<Pixels>,
    ) -> Result<usize, usize> {
        self.text_layouts
            .borrow()
            .get(id)
            .expect("测试布局必须登记")
            .layout
            .index_for_position(position)
    }

    pub(super) fn begin_text_selection(
        &mut self,
        id: &str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(offset) = self.hit_text(id, position) else {
            return;
        };
        self.selection = Some(MarkdownSelection {
            anchor: offset,
            head: offset,
        });
        self.selecting = true;
        self.focus.focus(window, cx);
    }

    pub(super) fn extend_text_selection(&mut self, id: &str, position: Point<Pixels>) {
        if !self.selecting {
            return;
        }
        let Some(offset) = self.hit_text(id, position) else {
            return;
        };
        if let Some(selection) = &mut self.selection {
            selection.head = offset;
        }
    }

    pub(super) fn end_text_selection(&mut self) {
        self.selecting = false;
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn selection_range_for_test(&self) -> Option<Range<usize>> {
        self.selection.map(MarkdownSelection::range)
    }

    pub(super) fn select_all_rendered(&mut self) {
        let Some(document) = &self.document else {
            return;
        };
        self.selection = Some(MarkdownSelection {
            anchor: 0,
            head: document.visible_text().len(),
        });
    }

    pub(super) fn copy_rendered_selection(&self, cx: &mut App) {
        let Some(selection) = self.selection else {
            return;
        };
        let range = selection.range();
        if range.is_empty() {
            return;
        }
        let Some(document) = &self.document else {
            return;
        };
        let Some(text) = document.visible_text().get(range) else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_owned()));
    }
}

/// 将全局可见文本选区投影回每个行内 run；run 分裂保留原样式。
pub(super) fn highlight_selected_runs(
    runs: Vec<TextRun>,
    segments: &[LayoutSegment],
    selection: Option<MarkdownSelection>,
    color: Hsla,
) -> Vec<TextRun> {
    let Some(selection) = selection.map(MarkdownSelection::range) else {
        return runs;
    };
    let mut output = Vec::with_capacity(runs.len());
    for (run, segment) in runs.into_iter().zip(segments) {
        let start = selection.start.max(segment.visible.start);
        let end = selection.end.min(segment.visible.end);
        if start >= end {
            output.push(run);
            continue;
        }
        let selected = segment.local.start + (start - segment.visible.start)
            ..segment.local.start + (end - segment.visible.start);
        let mut push = |range: Range<usize>, highlighted: bool| {
            if range.is_empty() {
                return;
            }
            let mut part = run.clone();
            part.len = range.len();
            if highlighted {
                part.background_color = Some(color);
            }
            output.push(part);
        };
        push(segment.local.start..selected.start, false);
        push(selected.clone(), true);
        push(selected.end..segment.local.end, false);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(len: usize) -> TextRun {
        TextRun {
            len,
            font: gpui::Font::default(),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn selection_splits_runs_without_changing_visible_byte_ranges() {
        let color = Hsla {
            h: 0.5,
            s: 0.5,
            l: 0.5,
            a: 1.0,
        };
        let runs = highlight_selected_runs(
            vec![run(3), run(2)],
            &[
                LayoutSegment {
                    local: 0..3,
                    visible: 10..13,
                },
                LayoutSegment {
                    local: 3..5,
                    visible: 13..15,
                },
            ],
            Some(MarkdownSelection {
                anchor: 11,
                head: 15,
            }),
            color,
        );
        assert_eq!(
            runs.iter().map(|run| run.len).collect::<Vec<_>>(),
            [1, 2, 2]
        );
        assert_eq!(runs[0].background_color, None);
        assert_eq!(runs[1].background_color, Some(color));
        assert_eq!(runs[2].background_color, Some(color));
    }
}
