use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, Font, Pixels, Point, Render, ShapedLine, SharedString, Styled as _, TextAlign,
    TextRun, UTF16Selection, Window, canvas, fill, point, px, size,
};
use gpui_component::ActiveTheme as _;
use ropey::{LineType, Rope};
use slterm_document::{
    DocumentId, DocumentSession, EditOrigin, EditTransaction, EditTransactionError, MergePolicy,
    TextEdit, TextStateId,
};
use uuid::Uuid;

use super::metrics::{
    DisplayMap, rope_ceil_grapheme_boundary, rope_floor_grapheme_boundary,
    rope_is_grapheme_boundary,
};
use super::source::RopeTextSource;

/// 一次同步提交成功后回投的只读正文快照。
pub(crate) struct DocumentTextSnapshot {
    text: Rope,
    revision: u64,
    read_only: bool,
}

impl DocumentTextSnapshot {
    fn from_session(session: &DocumentSession) -> Self {
        Self {
            text: session.text().expect("Paged 不进入 Rope 编辑器").clone(),
            revision: session.text_revision(),
            read_only: session.is_read_only(),
        }
    }
}

/// 视图只提交事务；组合根同步调用唯一 Registry，并立即返回权威快照。
type DocumentEditSink = Box<
    dyn FnMut(
        EditTransaction,
        TextStateId,
        &mut App,
    ) -> Result<DocumentTextSnapshot, EditTransactionError>,
>;

#[derive(Clone, Copy, Debug)]
struct TextSelection {
    anchor: usize,
    head: usize,
}

impl TextSelection {
    fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundaryBias {
    Left,
    Right,
}

/// 将旧正文字节位置按一组有序、不重叠编辑映射到新正文。
fn transform_offset(offset: usize, edits: &[TextEdit], bias: BoundaryBias) -> usize {
    let mut result = offset as i128;
    let mut delta = 0_i128;
    for edit in edits {
        let range = &edit.utf8_range();
        if offset < range.start {
            break;
        }
        let start = range.start as i128 + delta;
        let old_len = (range.end - range.start) as i128;
        let new_len = edit.replacement().len() as i128;
        if range.is_empty() {
            if offset == range.start {
                if bias == BoundaryBias::Right {
                    result = start + new_len;
                }
            } else {
                result = offset as i128 + delta + new_len;
            }
            delta += new_len;
            continue;
        }
        if offset > range.end {
            delta += new_len - old_len;
            result = offset as i128 + delta;
            continue;
        }
        result = if offset == range.start {
            if bias == BoundaryBias::Right {
                start + new_len
            } else {
                start
            }
        } else if offset == range.end || bias == BoundaryBias::Right {
            start + new_len
        } else {
            start
        };
        if offset == range.end {
            delta += new_len - old_len;
            continue;
        }
        break;
    }
    result as usize
}

fn transform_selection(selection: TextSelection, edits: &[TextEdit]) -> TextSelection {
    if selection.anchor == selection.head {
        let cursor = transform_offset(selection.head, edits, BoundaryBias::Left);
        return TextSelection {
            anchor: cursor,
            head: cursor,
        };
    }
    let range = selection.range();
    let start = transform_offset(range.start, edits, BoundaryBias::Right);
    let end = transform_offset(range.end, edits, BoundaryBias::Left);
    if start > end {
        return TextSelection {
            anchor: end,
            head: end,
        };
    }
    if selection.anchor > selection.head {
        TextSelection {
            anchor: end,
            head: start,
        }
    } else {
        TextSelection {
            anchor: start,
            head: end,
        }
    }
}

/// 将基于原始输入的 UTF-16 偏移映射到 CRLF 归一化后的文本。
fn normalized_input_utf16_offset(original: &Rope, offset: usize) -> Option<usize> {
    if offset > original.len_utf16() {
        return None;
    }
    let byte = original.utf16_to_byte_idx(offset);
    if original.byte_to_utf16_idx(byte) != offset || !rope_is_grapheme_boundary(original, byte) {
        return None;
    }
    let mut crlf_count = 0;
    let mut previous_cr = false;
    for chunk in original.slice(..byte).chunks() {
        for byte in chunk.bytes() {
            if previous_cr && byte == b'\n' {
                crlf_count += 1;
            }
            previous_cr = byte == b'\r';
        }
    }
    offset.checked_sub(crlf_count)
}

#[derive(Clone, Copy)]
struct LayoutMetrics {
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
}

impl LayoutMetrics {
    fn visible_columns(&self) -> usize {
        (self.bounds.size.width / self.cell_width).ceil().max(1.0) as usize
    }
}

struct PaintFrame {
    bounds: Bounds<Pixels>,
    lines: Vec<(ShapedLine, super::source::VisibleLine)>,
    selection: Range<usize>,
    cursor: usize,
    layout: LayoutMetrics,
}

/// Document 的专用文本视图；Rope 是不可变展示快照，修改只发 core 事务。
pub(crate) struct DocumentTextView {
    document_id: DocumentId,
    text: Rope,
    revision: u64,
    read_only: bool,
    edit_sink: DocumentEditSink,
    selection: TextSelection,
    marked_range: Option<Range<usize>>,
    scroll_line: usize,
    focus: FocusHandle,
    layout: Rc<RefCell<Option<LayoutMetrics>>>,
    #[cfg(test)]
    painted_content_mask: Rc<RefCell<Option<gpui::ContentMask<Pixels>>>>,
}

impl DocumentTextView {
    pub(crate) fn new(
        document_id: DocumentId,
        session: &DocumentSession,
        edit_sink: DocumentEditSink,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self {
            document_id,
            text: session.text().expect("Paged 不进入 Rope 编辑器").clone(),
            revision: session.text_revision(),
            read_only: session.is_read_only(),
            edit_sink,
            selection: TextSelection { anchor: 0, head: 0 },
            marked_range: None,
            scroll_line: 0,
            focus: cx.focus_handle().tab_stop(true),
            layout: Rc::default(),
            #[cfg(test)]
            painted_content_mask: Rc::default(),
        }
    }

    /// 无编辑增量的外部快照无法保留逻辑位置，必须取消组合并复位选区。
    pub(crate) fn apply_snapshot(&mut self, session: &DocumentSession, cx: &mut Context<'_, Self>) {
        self.adopt_snapshot(DocumentTextSnapshot::from_session(session));
        self.selection = TextSelection { anchor: 0, head: 0 };
        self.marked_range = None;
        cx.notify();
    }

    /// 共享 Document 的普通编辑按真实增量重定位视图范围。
    pub(crate) fn apply_edit_snapshot(
        &mut self,
        session: &DocumentSession,
        transaction: &EditTransaction,
        cx: &mut Context<'_, Self>,
    ) {
        if transaction.document_id != self.document_id
            || transaction.expected_text_revision != self.revision
        {
            self.apply_snapshot(session, cx);
            return;
        }
        // core 会跳过原样替换；视图也只能按实际改变正文的编辑重定位。
        let edits = transaction
            .edits
            .iter()
            .filter(|edit| self.text.slice(edit.utf8_range().clone()) != edit.replacement())
            .cloned()
            .collect::<Vec<_>>();
        let selection = transform_selection(self.selection, &edits);
        let marked_range = self.marked_range.clone().and_then(|marked| {
            let invalidated = edits.iter().any(|edit| {
                let range = &edit.utf8_range();
                if range.is_empty() {
                    marked.start < range.start && range.start < marked.end
                } else {
                    range.start < marked.end && marked.start < range.end
                }
            });
            if invalidated {
                return None;
            }
            let start = transform_offset(marked.start, &edits, BoundaryBias::Right);
            let end = transform_offset(marked.end, &edits, BoundaryBias::Left);
            (start <= end).then_some(start..end)
        });
        self.adopt_snapshot(DocumentTextSnapshot::from_session(session));
        self.selection = selection;
        self.marked_range = marked_range;
        self.reconcile_transient_ranges();
        cx.notify();
    }

    fn adopt_snapshot(&mut self, snapshot: DocumentTextSnapshot) {
        self.text = snapshot.text;
        self.revision = snapshot.revision;
        self.read_only = snapshot.read_only;
        self.scroll_line = self
            .scroll_line
            .min(self.text.len_lines(LineType::LF).saturating_sub(1));
    }

    /// 编辑可能把相邻字符合并为一个字素；最终正文决定可用的瞬态边界。
    fn reconcile_transient_ranges(&mut self) {
        let selection = self.selection;
        self.selection = if selection.anchor == selection.head {
            let cursor = rope_ceil_grapheme_boundary(&self.text, selection.head).unwrap_or(0);
            TextSelection {
                anchor: cursor,
                head: cursor,
            }
        } else {
            let range = selection.range();
            let start = rope_floor_grapheme_boundary(&self.text, range.start).unwrap_or(0);
            let end = rope_ceil_grapheme_boundary(&self.text, range.end).unwrap_or(self.text.len());
            if selection.anchor > selection.head {
                TextSelection {
                    anchor: end,
                    head: start,
                }
            } else {
                TextSelection {
                    anchor: start,
                    head: end,
                }
            }
        };
        self.marked_range = self.marked_range.clone().and_then(|marked| {
            let start = rope_floor_grapheme_boundary(&self.text, marked.start)?;
            let end = rope_ceil_grapheme_boundary(&self.text, marked.end)?;
            (start <= end).then_some(start..end)
        });
    }

    fn utf16_to_byte(&self, offset: usize) -> Option<usize> {
        if offset > self.text.len_utf16() {
            return None;
        }
        let byte = self.text.utf16_to_byte_idx(offset);
        (self.text.byte_to_utf16_idx(byte) == offset).then_some(byte)
    }

    fn bytes_from_utf16(&self, range: Range<usize>) -> Option<Range<usize>> {
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

    fn range_to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text.byte_to_utf16_idx(range.start)..self.text.byte_to_utf16_idx(range.end)
    }

    fn replace(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        mark: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if self.read_only {
            return;
        }
        let selected = match selected {
            Some(selected) => {
                let original = Rope::from_str(text);
                let Some(start) = normalized_input_utf16_offset(&original, selected.start) else {
                    return;
                };
                let Some(end) = normalized_input_utf16_offset(&original, selected.end) else {
                    return;
                };
                Some(start..end)
            }
            None => None,
        };
        let text = if text.contains("\r\n") {
            Cow::Owned(text.replace("\r\n", "\n"))
        } else {
            Cow::Borrowed(text)
        };
        let range = match range_utf16 {
            Some(range) => {
                let Some(range) = self.bytes_from_utf16(range) else {
                    return;
                };
                range
            }
            None => self
                .marked_range
                .clone()
                .unwrap_or_else(|| self.selection.range()),
        };
        if range.start > range.end
            || range.end > self.text.len()
            || !rope_is_grapheme_boundary(&self.text, range.start)
            || !rope_is_grapheme_boundary(&self.text, range.end)
        {
            return;
        }
        let selected_bytes = match selected {
            Some(selected) => {
                let replacement = Rope::from_str(&text);
                if selected.start > selected.end || selected.end > replacement.len_utf16() {
                    return;
                }
                let start = replacement.utf16_to_byte_idx(selected.start);
                let end = replacement.utf16_to_byte_idx(selected.end);
                if replacement.byte_to_utf16_idx(start) != selected.start
                    || replacement.byte_to_utf16_idx(end) != selected.end
                    || !rope_is_grapheme_boundary(&replacement, start)
                    || !rope_is_grapheme_boundary(&replacement, end)
                {
                    return;
                }
                Some(range.start + start..range.start + end)
            }
            None => None,
        };
        let new_selection =
            selected_bytes.unwrap_or(range.start + text.len()..range.start + text.len());
        let new_marked = if mark && !text.is_empty() {
            Some(range.start..range.start + text.len())
        } else {
            None
        };
        let new_state = TextStateId::from_uuid(Uuid::now_v7()).expect("系统 UUIDv7 必须有效");
        let transaction = EditTransaction {
            document_id: self.document_id,
            expected_text_revision: self.revision,
            edits: vec![TextEdit::new(range, text.as_ref())],
            origin: EditOrigin::UserInput,
            merge_policy: MergePolicy::Adjacent,
        };
        let Ok(snapshot) = (self.edit_sink)(transaction, new_state, cx) else {
            return;
        };
        self.adopt_snapshot(snapshot);
        self.selection = TextSelection {
            anchor: new_selection.start,
            head: new_selection.end,
        };
        self.marked_range = new_marked;
        self.reconcile_transient_ranges();
        cx.notify();
    }

    fn metrics(&self) -> Option<LayoutMetrics> {
        *self.layout.borrow()
    }
}

impl Focusable for DocumentTextView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
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
        let visible_row = row.checked_sub(self.scroll_line)?;
        let line_start = self.text.line_to_byte_idx(row, LineType::LF);
        let metrics = self.metrics().unwrap_or(LayoutMetrics {
            bounds: element_bounds,
            cell_width: px(8.0),
            line_height: px(20.0),
        });
        let line = RopeTextSource::new(&self.text, DisplayMap::new(4))
            .visible_line_windows(row..row + 1, metrics.visible_columns())
            .into_iter()
            .next()?;
        let local = byte - line_start;
        let x = match DisplayMap::new(4).column_at(line.text(), local) {
            Some(column) => metrics.bounds.origin.x + metrics.cell_width * column as f32,
            None if local > line.text().len() => {
                metrics.bounds.origin.x + metrics.bounds.size.width
            }
            None => return None,
        };
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
        let metrics = self.metrics()?;
        let x = (point.x - metrics.bounds.origin.x).max(px(0.0));
        let y = (point.y - metrics.bounds.origin.y).max(px(0.0));
        let row = self.scroll_line + (y / metrics.line_height).floor() as usize;
        let map = DisplayMap::new(4);
        let line = RopeTextSource::new(&self.text, map)
            .visible_line_windows(row..row + 1, metrics.visible_columns())
            .into_iter()
            .next()?;
        let column = (x / metrics.cell_width).round() as usize;
        let local = map.byte_at_column(line.text(), column)?;
        Some(self.text.byte_to_utf16_idx(line.byte_range().start + local))
    }
}

impl DocumentTextView {
    fn text_canvas(&mut self, cx: &mut Context<'_, Self>) -> gpui::Canvas<PaintFrame> {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let accent = theme.accent;
        let background = theme.background;
        let font = Font {
            family: theme.mono_font_family.clone(),
            ..Font::default()
        };
        let text = self.text.clone();
        let selection = self.selection.range();
        let cursor = self.selection.head;
        let scroll_line = self.scroll_line;
        let layout = self.layout.clone();
        let focus = self.focus.clone();
        let view = cx.entity();
        #[cfg(test)]
        let painted_content_mask = self.painted_content_mask.clone();
        canvas(
            move |bounds, window, _| {
                let line_height = px(20.0);
                let map = DisplayMap::new(4);
                let make_run = |len| TextRun {
                    len,
                    font: font.clone(),
                    color: foreground,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let sample = window.text_system().shape_line(
                    SharedString::new_static("M"),
                    px(14.0),
                    &[make_run(1)],
                    None,
                );
                let metrics = LayoutMetrics {
                    bounds,
                    cell_width: sample.width,
                    line_height,
                };
                *layout.borrow_mut() = Some(metrics);
                let height: f32 = bounds.size.height.into();
                let count = (height / line_height.as_f32()).ceil().max(1.0) as usize;
                let lines = RopeTextSource::new(&text, map)
                    .visible_line_windows(
                        scroll_line..scroll_line + count,
                        metrics.visible_columns(),
                    )
                    .into_iter()
                    .map(|line| {
                        let shaped = window.text_system().shape_line(
                            SharedString::from(line.text().to_owned()),
                            px(14.0),
                            &[make_run(line.text().len())],
                            None,
                        );
                        (shaped, line)
                    })
                    .collect();
                PaintFrame {
                    bounds,
                    lines,
                    selection,
                    cursor,
                    layout: metrics,
                }
            },
            move |bounds, frame, window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    #[cfg(test)]
                    {
                        *painted_content_mask.borrow_mut() = Some(window.content_mask());
                    }
                    let map = DisplayMap::new(4);
                    for (row, (_, line)) in frame.lines.iter().enumerate() {
                        let range = line.byte_range();
                        let start = frame.selection.start.max(range.start);
                        let end = frame.selection.end.min(range.end);
                        if start < end {
                            let x0 = map.column_at(line.text(), start - range.start);
                            let x1 = map.column_at(line.text(), end - range.start);
                            if let (Some(x0), Some(x1)) = (x0, x1) {
                                fill(
                                    Bounds {
                                        origin: point(
                                            bounds.origin.x + frame.layout.cell_width * x0 as f32,
                                            bounds.origin.y + frame.layout.line_height * row as f32,
                                        ),
                                        size: size(
                                            frame.layout.cell_width * (x1 - x0) as f32,
                                            frame.layout.line_height,
                                        ),
                                    },
                                    accent,
                                );
                            }
                        }
                        if range.start <= frame.cursor
                            && frame.cursor <= range.end
                            && let Some(column) =
                                map.column_at(line.text(), frame.cursor - range.start)
                        {
                            fill(
                                Bounds {
                                    origin: point(
                                        bounds.origin.x + frame.layout.cell_width * column as f32,
                                        bounds.origin.y + frame.layout.line_height * row as f32,
                                    ),
                                    size: size(px(1.0), frame.layout.line_height),
                                },
                                accent,
                            );
                        }
                    }
                    for (row, (shaped, _)) in frame.lines.iter().enumerate() {
                        let origin = point(
                            bounds.origin.x,
                            bounds.origin.y + frame.layout.line_height * row as f32,
                        );
                        let _ = shaped.paint(
                            origin,
                            frame.layout.line_height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                    window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
                });
            },
        )
        .size_full()
        .overflow_hidden()
        .bg(background)
    }
}

impl Render for DocumentTextView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        self.text_canvas(cx)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Instant;

    use gpui::{
        AppContext as _, Bounds, Context, Entity, EntityInputHandler as _, Render, TestAppContext,
        Window, px,
    };
    use ropey::Rope;
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy,
        TextEdit, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};
    use uuid::Uuid;

    use super::{
        DocumentTextSnapshot, DocumentTextView, LayoutMetrics, TextSelection, transform_selection,
    };

    struct Host {
        registry: Rc<RefCell<DocumentRegistry>>,
        id: DocumentId,
        view: Entity<DocumentTextView>,
    }

    impl Host {
        fn new(text: &str, read_only: bool, cx: &mut Context<'_, Self>) -> Self {
            let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
            let state = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
            let mut registry = DocumentRegistry::new();
            registry
                .open(
                    id,
                    ResolvedPath::new(CanonicalPath::new("D:/work/editor.txt").unwrap(), None),
                    Rope::from_str(text),
                    state,
                    std::sync::Arc::from(text.as_bytes()),
                    DocumentMetadata {
                        read_only,
                        ..DocumentMetadata::default()
                    },
                )
                .unwrap();
            let registry = Rc::new(RefCell::new(registry));
            let sink_registry = registry.clone();
            let view = cx.new(|cx| {
                let session = registry.borrow();
                DocumentTextView::new(
                    id,
                    session.session(id).unwrap(),
                    Box::new(move |transaction, state, _| {
                        let mut registry = sink_registry.borrow_mut();
                        registry.apply(transaction, state, Instant::now())?;
                        Ok(DocumentTextSnapshot::from_session(
                            registry.session(id).unwrap(),
                        ))
                    }),
                    cx,
                )
            });
            Self { registry, id, view }
        }

        fn text(&self) -> String {
            self.registry
                .borrow()
                .session(self.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string()
        }
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl gpui::IntoElement {
            self.view.clone()
        }
    }

    #[test]
    fn external_replacement_collapses_a_fully_covered_selection() {
        let selection = TextSelection { anchor: 2, head: 4 };
        let edits = vec![TextEdit::new(1..5, "X")];
        let mapped = transform_selection(selection, &edits);
        assert_eq!(mapped.anchor, 1);
        assert_eq!(mapped.head, 1);
    }

    #[test]
    fn adjacent_edits_rebase_boundaries_through_same_offset_insertions() {
        let selection = TextSelection { anchor: 1, head: 2 };
        let edits = vec![TextEdit::new(0..1, "X"), TextEdit::new(1..1, "Q")];
        let mapped = transform_selection(selection, &edits);
        assert_eq!(mapped.range(), 2..3);
    }

    #[gpui::test]
    fn ime_commits_utf16_edits_through_the_shared_document(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("A😀B", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "中", None, window, cx)
                });
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |host, window, cx| {
                assert_eq!(host.text(), "中A😀B");
                host.view.update(cx, |view, cx| {
                    assert_eq!(view.marked_text_range(window, cx), Some(0..1));
                    view.replace_text_in_range(None, "🙂", window, cx);
                    assert_eq!(view.marked_text_range(window, cx), None);
                });
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |host, window, cx| {
                assert_eq!(host.text(), "🙂A😀B");
                host.view.update(cx, |view, cx| {
                    let mut adjusted = None;
                    assert_eq!(
                        view.text_for_range(0..2, &mut adjusted, window, cx),
                        Some("🙂".to_owned())
                    );
                    assert_eq!(adjusted, Some(0..2));
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn painted_view_dispatches_text_input_to_document(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
        window
            .update(cx, |host, window, cx| {
                let focus = host.view.read(cx).focus.clone();
                window.focus(&focus, cx);
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.simulate_input("中");

        window
            .update(cx, |host, _, _| assert_eq!(host.text(), "中ab"))
            .unwrap();
    }

    #[gpui::test]
    fn invalid_explicit_input_range_does_not_fall_back_to_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("abc", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_text_in_range(Some(99..100), "x", window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |host, _, _| assert_eq!(host.text(), "abc"))
            .unwrap();
    }

    #[gpui::test]
    fn ime_geometry_uses_scrolled_visible_origin(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("A中\nB", false, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    let metrics = view.metrics().unwrap();
                    let hit = view
                        .character_index_for_point(
                            gpui::point(
                                metrics.bounds.origin.x + metrics.cell_width * 2.0,
                                metrics.bounds.origin.y,
                            ),
                            window,
                            cx,
                        )
                        .unwrap();
                    assert_eq!(hit, 2);
                    view.scroll_line = 1;
                    let bounds = view
                        .bounds_for_range(3..4, metrics.bounds, window, cx)
                        .unwrap();
                    assert_eq!(bounds.origin.y, metrics.bounds.origin.y);
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn sequential_input_in_one_update_is_not_lost(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_text_in_range(None, "a", window, cx);
                    view.replace_text_in_range(None, "b", window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |host, _, _| assert_eq!(host.text(), "ab"))
            .unwrap();
    }

    #[gpui::test]
    fn marked_range_is_queryable_immediately(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "中", None, window, cx);
                    assert_eq!(view.marked_text_range(window, cx), Some(0..1));
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn crlf_input_is_normalized_before_commit(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_text_in_range(None, "a\r\nb", window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |host, _, _| assert_eq!(host.text(), "a\nb"))
            .unwrap();
    }

    #[gpui::test]
    fn system_selection_rejects_grapheme_internal_offsets(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("e\u{301}", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.set_selected_text_range(1..1, window, cx);
                    let selection = view.selected_text_range(false, window, cx).unwrap();
                    assert_eq!(selection.range, 0..0);
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn input_after_a_grapheme_merge_continues_after_the_cluster(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("\u{301}", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_text_in_range(None, "e", window, cx);
                    view.replace_text_in_range(None, "x", window, cx);
                });
                assert_eq!(host.text(), "e\u{301}x");
            })
            .unwrap();
    }

    #[gpui::test]
    fn ime_mark_expands_when_input_merges_with_adjacent_grapheme(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("e", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.set_selected_text_range(1..1, window, cx);
                    view.replace_and_mark_text_in_range(None, "\u{301}", None, window, cx);
                    assert_eq!(view.marked_text_range(window, cx), Some(0..2));
                    view.replace_text_in_range(None, "x", window, cx);
                });
                assert_eq!(host.text(), "x");
            })
            .unwrap();
    }

    #[gpui::test]
    fn explicit_utf16_ranges_reject_surrogate_internal_offsets(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("😀", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.set_selected_text_range(1..1, window, cx);
                    assert_eq!(
                        view.selected_text_range(false, window, cx).unwrap().range,
                        0..0
                    );
                    view.replace_text_in_range(Some(1..2), "x", window, cx);
                });
                assert_eq!(host.text(), "😀");
            })
            .unwrap();
    }

    #[gpui::test]
    fn text_canvas_clips_horizontal_and_vertical_overflow(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("long", false, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        window
            .update(cx, |host, _, cx| {
                let view = host.view.read(cx);
                let bounds = view.metrics().unwrap().bounds;
                assert_eq!(view.painted_content_mask.borrow().unwrap().bounds, bounds);
            })
            .unwrap();
    }

    #[gpui::test]
    fn external_edit_rebases_selection_and_marked_range(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "x", None, window, cx);
                });
                let revision = host
                    .registry
                    .borrow()
                    .session(host.id)
                    .unwrap()
                    .text_revision();
                let transaction = EditTransaction {
                    document_id: host.id,
                    expected_text_revision: revision,
                    edits: vec![TextEdit::new(0..0, "Q")],
                    origin: EditOrigin::UserInput,
                    merge_policy: MergePolicy::Disallowed,
                };
                host.registry
                    .borrow_mut()
                    .apply(
                        transaction.clone(),
                        TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                        Instant::now(),
                    )
                    .unwrap();
                {
                    let registry = host.registry.borrow();
                    let session = registry.session(host.id).unwrap();
                    host.view.update(cx, |view, cx| {
                        view.apply_edit_snapshot(session, &transaction, cx);
                        assert_eq!(view.marked_text_range(window, cx), Some(1..2));
                        assert_eq!(
                            view.selected_text_range(false, window, cx).unwrap().range,
                            2..2
                        );
                    });
                }
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "y", None, window, cx);
                });
            })
            .unwrap();
        window
            .update(cx, |host, _, _| assert_eq!(host.text(), "Qy"))
            .unwrap();
    }

    #[gpui::test]
    fn rejected_edit_preserves_selection_and_marked_range(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                let transaction = EditTransaction {
                    document_id: host.id,
                    expected_text_revision: 0,
                    edits: vec![TextEdit::new(0..0, "external")],
                    origin: EditOrigin::UserInput,
                    merge_policy: MergePolicy::Disallowed,
                };
                host.registry
                    .borrow_mut()
                    .apply(
                        transaction,
                        TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                        Instant::now(),
                    )
                    .unwrap();
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "stale", None, window, cx);
                    assert_eq!(view.marked_text_range(window, cx), None);
                    assert_eq!(
                        view.selected_text_range(false, window, cx).unwrap().range,
                        0..0
                    );
                });
                assert_eq!(host.text(), "external");
            })
            .unwrap();
    }

    #[gpui::test]
    fn unchanged_edits_do_not_rebase_or_cancel_view_ranges(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.set_selected_text_range(0..1, window, cx);
                });
                let transaction = EditTransaction {
                    document_id: host.id,
                    expected_text_revision: 0,
                    edits: vec![TextEdit::new(0..1, "a"), TextEdit::new(2..2, "X")],
                    origin: EditOrigin::UserInput,
                    merge_policy: MergePolicy::Disallowed,
                };
                host.registry
                    .borrow_mut()
                    .apply(
                        transaction.clone(),
                        TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                        Instant::now(),
                    )
                    .unwrap();
                let registry = host.registry.borrow();
                let session = registry.session(host.id).unwrap();
                host.view.update(cx, |view, cx| {
                    view.apply_edit_snapshot(session, &transaction, cx);
                    assert_eq!(
                        view.selected_text_range(false, window, cx).unwrap().range,
                        0..1
                    );
                });
                assert_eq!(host.text(), "abX");
            })
            .unwrap();
    }

    #[gpui::test]
    fn crlf_composition_maps_selected_range_to_normalized_text(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    view.replace_and_mark_text_in_range(None, "a\r\nb", Some(4..4), window, cx);
                    assert_eq!(
                        view.selected_text_range(false, window, cx).unwrap().range,
                        3..3
                    );
                });
                assert_eq!(host.text(), "a\nb");
            })
            .unwrap();
    }

    #[gpui::test]
    fn snapshot_clamps_scroll_and_restores_ime_geometry(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("a\nb\nc", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, _cx| view.scroll_line = 2);
                let transaction = EditTransaction {
                    document_id: host.id,
                    expected_text_revision: 0,
                    edits: vec![TextEdit::new(0..5, "x")],
                    origin: EditOrigin::UserInput,
                    merge_policy: MergePolicy::Disallowed,
                };
                host.registry
                    .borrow_mut()
                    .apply(
                        transaction.clone(),
                        TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                        Instant::now(),
                    )
                    .unwrap();
                let registry = host.registry.borrow();
                let session = registry.session(host.id).unwrap();
                host.view.update(cx, |view, cx| {
                    view.apply_edit_snapshot(session, &transaction, cx);
                    assert_eq!(view.scroll_line, 0);
                    assert!(
                        view.bounds_for_range(0..1, Bounds::default(), window, cx)
                            .is_some()
                    );
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn long_line_tail_uses_the_visible_right_edge_for_ime(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("abcdefgh", false, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    let bounds = Bounds {
                        origin: gpui::point(px(10.0), px(10.0)),
                        size: gpui::size(px(32.0), px(20.0)),
                    };
                    *view.layout.borrow_mut() = Some(LayoutMetrics {
                        bounds,
                        cell_width: px(8.0),
                        line_height: px(20.0),
                    });
                    let caret = view.bounds_for_range(8..8, bounds, window, cx).unwrap();
                    assert_eq!(caret.origin.x, px(42.0));
                });
            })
            .unwrap();
    }

    #[gpui::test]
    fn read_only_view_rejects_ime_without_touching_document(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let window = cx.add_window(|_, cx| Host::new("locked", true, cx));
        window
            .update(cx, |host, window, cx| {
                host.view.update(cx, |view, cx| {
                    assert!(!view.accepts_text_input(window, cx));
                    view.replace_and_mark_text_in_range(None, "x", None, window, cx);
                });
                assert_eq!(host.text(), "locked");
            })
            .unwrap();
    }
}
