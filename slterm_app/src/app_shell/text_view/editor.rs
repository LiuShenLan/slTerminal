use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, Font, Hsla, InteractiveElement as _, ParentElement as _, Pixels, Point, Render,
    ShapedLine, SharedString, Styled as _, TextAlign, TextRun, UTF16Selection, Window, canvas, div,
    fill, point, px, size,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{
    Backspace, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine, MoveToStartOfLine,
    MoveUp, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine, Undo,
};
use ropey::{LineType, Rope};
use slterm_document::{
    DocumentId, DocumentSession, EditApplyOutcome, EditOrigin, EditTransaction,
    EditTransactionError, MergePolicy, TextEdit, TextStateId,
};
use uuid::Uuid;

use super::layout::{FontMetricsKey, LayoutCache, TextLayoutKey};
use super::metrics::{
    DisplayMap, rope_ceil_grapheme_boundary, rope_floor_grapheme_boundary,
    rope_is_grapheme_boundary, rope_next_grapheme_boundary, rope_previous_grapheme_boundary,
};
use super::source::RopeTextSource;
use super::{SelectDown, SelectLeft, SelectRight, SelectUp};

/// 一次同步提交成功后回投的只读正文快照。
pub(crate) struct DocumentTextSnapshot {
    text: Rope,
    revision: u64,
    text_state: TextStateId,
    read_only: bool,
}

impl DocumentTextSnapshot {
    fn from_session(session: &DocumentSession) -> Self {
        Self {
            text: session.text().expect("Paged 不进入 Rope 编辑器").clone(),
            revision: session.text_revision(),
            text_state: session.current_state(),
            read_only: session.is_read_only(),
        }
    }
}

pub(crate) enum DocumentEditRequest {
    Edit(EditTransaction, TextStateId),
    Travel { redo: bool },
}

pub(crate) struct DocumentEditResponse {
    snapshot: DocumentTextSnapshot,
    outcome: EditApplyOutcome,
}

/// 视图只提交事务；组合根同步调用唯一 Registry，并立即返回权威快照。
type DocumentEditSink = Box<
    dyn FnMut(DocumentEditRequest, &mut App) -> Result<DocumentEditResponse, EditTransactionError>,
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

    fn visible_whole_columns(&self) -> usize {
        (self.bounds.size.width / self.cell_width).floor() as usize
    }
}

struct PaintFrame {
    bounds: Bounds<Pixels>,
    lines: Vec<(ShapedLine, super::source::VisibleLine)>,
    selection: Range<usize>,
    cursor: usize,
    layout: LayoutMetrics,
}

#[derive(Clone, PartialEq)]
struct ThemeFingerprint {
    foreground: Hsla,
    accent: Hsla,
    mono_font_family: SharedString,
    mono_font_size: Pixels,
}

/// Document 的专用文本视图；Rope 是不可变展示快照，修改只发 core 事务。
pub(crate) struct DocumentTextView {
    document_id: DocumentId,
    text: Rope,
    revision: u64,
    text_state: TextStateId,
    read_only: bool,
    edit_sink: DocumentEditSink,
    language_revision: u64,
    theme_revision: u64,
    theme_fingerprint: Option<ThemeFingerprint>,
    layout_cache: Rc<RefCell<LayoutCache>>,
    selection: TextSelection,
    preferred_column: Option<usize>,
    marked_range: Option<Range<usize>>,
    scroll_line: usize,
    scroll_column: usize,
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
            text_state: session.current_state(),
            read_only: session.is_read_only(),
            edit_sink,
            language_revision: 0,
            theme_revision: 0,
            theme_fingerprint: None,
            layout_cache: Rc::default(),
            selection: TextSelection { anchor: 0, head: 0 },
            preferred_column: None,
            marked_range: None,
            scroll_line: 0,
            scroll_column: 0,
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
        self.preferred_column = None;
        self.ensure_cursor_visible();
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
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn adopt_snapshot(&mut self, snapshot: DocumentTextSnapshot) {
        self.text = snapshot.text;
        self.revision = snapshot.revision;
        self.text_state = snapshot.text_state;
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
        self.replace_normalized(range, &text, selected, mark, cx);
    }

    fn replace_normalized(
        &mut self,
        range: Range<usize>,
        text: &str,
        selected: Option<Range<usize>>,
        mark: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if range.start > range.end
            || range.end > self.text.len()
            || !rope_is_grapheme_boundary(&self.text, range.start)
            || !rope_is_grapheme_boundary(&self.text, range.end)
        {
            return;
        }
        let selected_bytes = match selected {
            Some(selected) => {
                let replacement = Rope::from_str(text);
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
            edits: vec![TextEdit::new(range, text)],
            origin: EditOrigin::UserInput,
            merge_policy: MergePolicy::Adjacent,
        };
        let Ok(response) = (self.edit_sink)(DocumentEditRequest::Edit(transaction, new_state), cx)
        else {
            return;
        };
        self.adopt_snapshot(response.snapshot);
        self.selection = TextSelection {
            anchor: new_selection.start,
            head: new_selection.end,
        };
        self.marked_range = new_marked;
        self.reconcile_transient_ranges();
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn edit_by_keyboard(&mut self, range: Range<usize>, text: &str, cx: &mut Context<'_, Self>) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        self.replace_normalized(range, text, None, false, cx);
    }

    fn backspace(&mut self, cx: &mut Context<'_, Self>) {
        let range = self.selection.range();
        let range = if range.start == range.end {
            let Some(start) = rope_previous_grapheme_boundary(&self.text, range.start) else {
                return;
            };
            start..range.end
        } else {
            range
        };
        self.edit_by_keyboard(range, "", cx);
    }

    fn delete(&mut self, cx: &mut Context<'_, Self>) {
        let range = self.selection.range();
        let range = if range.start == range.end {
            let Some(end) = rope_next_grapheme_boundary(&self.text, range.start) else {
                return;
            };
            range.start..end
        } else {
            range
        };
        self.edit_by_keyboard(range, "", cx);
    }

    fn insert_newline(&mut self, cx: &mut Context<'_, Self>) {
        self.edit_by_keyboard(self.selection.range(), "\n", cx);
    }

    fn select_all(&mut self, cx: &mut Context<'_, Self>) {
        if self.marked_range.is_some() {
            return;
        }
        self.selection = TextSelection {
            anchor: 0,
            head: self.text.len(),
        };
        self.preferred_column = None;
        cx.notify();
    }

    fn travel_history(&mut self, redo: bool, cx: &mut Context<'_, Self>) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        let Ok(response) = (self.edit_sink)(DocumentEditRequest::Travel { redo }, cx) else {
            return;
        };
        if response.outcome == EditApplyOutcome::Unchanged {
            return;
        }
        self.adopt_snapshot(response.snapshot);
        self.selection = TextSelection { anchor: 0, head: 0 };
        self.marked_range = None;
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn move_to(&mut self, position: usize, select: bool, cx: &mut Context<'_, Self>) {
        if self.marked_range.is_some() {
            return;
        }
        if select {
            self.selection.head = position;
        } else {
            self.selection = TextSelection {
                anchor: position,
                head: position,
            };
        }
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn move_horizontal(&mut self, left: bool, select: bool, cx: &mut Context<'_, Self>) {
        let range = self.selection.range();
        let position = if !select && range.start < range.end {
            if left { range.start } else { range.end }
        } else {
            let boundary = if left {
                rope_previous_grapheme_boundary(&self.text, self.selection.head)
            } else {
                rope_next_grapheme_boundary(&self.text, self.selection.head)
            };
            boundary.unwrap_or(self.selection.head)
        };
        self.move_to(position, select, cx);
    }

    fn move_line_boundary(&mut self, end: bool, select: bool, cx: &mut Context<'_, Self>) {
        let row = self
            .text
            .byte_to_line_idx(self.selection.head, LineType::LF);
        let Some(range) = RopeTextSource::new(&self.text, DisplayMap::new(4)).line_byte_range(row)
        else {
            return;
        };
        self.move_to(if end { range.end } else { range.start }, select, cx);
    }

    fn move_vertical(&mut self, down: bool, select: bool, cx: &mut Context<'_, Self>) {
        if self.marked_range.is_some() {
            return;
        }
        let map = DisplayMap::new(4);
        let source = RopeTextSource::new(&self.text, map);
        let row = self
            .text
            .byte_to_line_idx(self.selection.head, LineType::LF);
        let target_row = if down {
            row.saturating_add(1)
                .min(self.text.len_lines(LineType::LF) - 1)
        } else {
            row.saturating_sub(1)
        };
        if target_row == row {
            return;
        }
        let Some(current_range) = source.line_byte_range(row) else {
            return;
        };
        let current_column = map.column_at_rope(
            self.text.slice(current_range),
            self.selection.head - self.text.line_to_byte_idx(row, LineType::LF),
        );
        let preferred = match (self.preferred_column, current_column) {
            (Some(column), _) => column,
            (None, Some(column)) => column,
            (None, None) => return,
        };
        let Some(line) = source
            .visible_line_windows(target_row..target_row + 1, 0..preferred + 1)
            .into_iter()
            .next()
        else {
            return;
        };
        let Some(local) = map.byte_at_column(line.text(), preferred) else {
            return;
        };
        let target = line.byte_range().start + local;
        self.move_to(target, select, cx);
        self.preferred_column = Some(preferred);
    }

    fn ensure_cursor_visible(&mut self) {
        let Some(metrics) = self.metrics() else {
            return;
        };
        let row = self
            .text
            .byte_to_line_idx(self.selection.head, LineType::LF);
        let visible_rows = (metrics.bounds.size.height / metrics.line_height)
            .floor()
            .max(1.0) as usize;
        if row < self.scroll_line {
            self.scroll_line = row;
        } else if row >= self.scroll_line + visible_rows {
            self.scroll_line = row + 1 - visible_rows;
        }
        let map = DisplayMap::new(4);
        let Some(range) = RopeTextSource::new(&self.text, map).line_byte_range(row) else {
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
        } else if column > self.scroll_column + visible_columns {
            self.scroll_column = column - visible_columns;
        }
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
        let visible_row = row.checked_sub(self.scroll_line)?;
        let metrics = self.metrics().unwrap_or(LayoutMetrics {
            bounds: element_bounds,
            cell_width: px(8.0),
            line_height: px(20.0),
        });
        let line = RopeTextSource::new(&self.text, DisplayMap::new(4))
            .visible_line_windows(
                row..row + 1,
                self.scroll_column..self.scroll_column + metrics.visible_columns(),
            )
            .into_iter()
            .next()?;
        let visible = line.byte_range();
        let x = if byte < visible.start {
            metrics.bounds.origin.x
        } else if byte > visible.end {
            metrics.bounds.origin.x + metrics.bounds.size.width
        } else {
            let column = DisplayMap::new(4).column_at_from(
                line.text(),
                byte - visible.start,
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
        let metrics = self.metrics()?;
        let x = (point.x - metrics.bounds.origin.x).max(px(0.0));
        let y = (point.y - metrics.bounds.origin.y).max(px(0.0));
        let row = self.scroll_line + (y / metrics.line_height).floor() as usize;
        let map = DisplayMap::new(4);
        let line = RopeTextSource::new(&self.text, map)
            .visible_line_windows(
                row..row + 1,
                self.scroll_column..self.scroll_column + metrics.visible_columns(),
            )
            .into_iter()
            .next()?;
        let viewport_column = (x / metrics.cell_width).round() as usize;
        let target_column = self.scroll_column + viewport_column;
        let local = map.byte_at_column_from(line.text(), target_column, line.display_start())?;
        Some(self.text.byte_to_utf16_idx(line.byte_range().start + local))
    }
}

impl DocumentTextView {
    fn text_canvas(&mut self, cx: &mut Context<'_, Self>) -> gpui::Canvas<PaintFrame> {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let accent = theme.accent;
        let background = theme.background;
        let font_size = theme.mono_font_size;
        let fingerprint = ThemeFingerprint {
            foreground,
            accent,
            mono_font_family: theme.mono_font_family.clone(),
            mono_font_size: font_size,
        };
        if self.theme_fingerprint.as_ref() != Some(&fingerprint) {
            self.theme_revision = self
                .theme_revision
                .checked_add(1)
                .expect("主题 revision 不得耗尽");
            self.theme_fingerprint = Some(fingerprint);
        }
        let font = Font {
            family: theme.mono_font_family.clone(),
            ..Font::default()
        };
        let text = self.text.clone();
        let selection = self.selection.range();
        let cursor = self.selection.head;
        let scroll_line = self.scroll_line;
        let scroll_column = self.scroll_column;
        let layout = self.layout.clone();
        let layout_cache = self.layout_cache.clone();
        let document_id = self.document_id;
        let text_state = self.text_state;
        let language_revision = self.language_revision;
        let theme_revision = self.theme_revision;
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
                    font_size,
                    &[make_run(1)],
                    None,
                );
                let metrics = LayoutMetrics {
                    bounds,
                    cell_width: sample.width,
                    line_height,
                };
                layout_cache.borrow_mut().activate(TextLayoutKey {
                    document_id,
                    text_state,
                    language_revision,
                    font: FontMetricsKey {
                        family: font.family.clone(),
                        font_size_bits: font_size.as_f32().to_bits(),
                        cell_width_bits: metrics.cell_width.as_f32().to_bits(),
                        line_height_bits: line_height.as_f32().to_bits(),
                    },
                    wrap_width_bits: bounds.size.width.as_f32().to_bits(),
                    first_visible_column: scroll_column,
                    dpi_bits: window.scale_factor().to_bits(),
                    theme_revision,
                });
                *layout.borrow_mut() = Some(metrics);
                let height: f32 = bounds.size.height.into();
                let count = (height / line_height.as_f32()).ceil().max(1.0) as usize;
                let mut cache = layout_cache.borrow_mut();
                let lines = RopeTextSource::new(&text, map)
                    .visible_line_windows(
                        scroll_line..scroll_line + count,
                        scroll_column..scroll_column + metrics.visible_columns(),
                    )
                    .into_iter()
                    .map(|line| {
                        let shaped = cache.get(line.index()).unwrap_or_else(|| {
                            let shaped = window.text_system().shape_line(
                                SharedString::from(line.text().to_owned()),
                                font_size,
                                &[make_run(line.text().len())],
                                None,
                            );
                            cache.insert(line.index(), shaped.clone());
                            shaped
                        });
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
                            let x0 = map.column_at_from(
                                line.text(),
                                start - range.start,
                                line.display_start(),
                            );
                            let x1 = map.column_at_from(
                                line.text(),
                                end - range.start,
                                line.display_start(),
                            );
                            if let (Some(x0), Some(x1)) = (x0, x1) {
                                fill(
                                    Bounds {
                                        origin: point(
                                            bounds.origin.x
                                                + frame.layout.cell_width
                                                    * (x0 as f32 - scroll_column as f32),
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
                            && let Some(column) = map.column_at_from(
                                line.text(),
                                frame.cursor - range.start,
                                line.display_start(),
                            )
                        {
                            fill(
                                Bounds {
                                    origin: point(
                                        bounds.origin.x
                                            + frame.layout.cell_width
                                                * (column as f32 - scroll_column as f32),
                                        bounds.origin.y + frame.layout.line_height * row as f32,
                                    ),
                                    size: size(px(1.0), frame.layout.line_height),
                                },
                                accent,
                            );
                        }
                    }
                    for (row, (shaped, line)) in frame.lines.iter().enumerate() {
                        let origin = point(
                            bounds.origin.x
                                + frame.layout.cell_width
                                    * (line.display_start() as f32 - scroll_column as f32),
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
        let canvas = self.text_canvas(cx);
        let focus = self.focus.clone();
        div()
            .key_context("DocumentText")
            .track_focus(&focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .on_action(cx.listener(|view, _: &MoveLeft, _, cx| {
                view.move_horizontal(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveRight, _, cx| {
                view.move_horizontal(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveUp, _, cx| {
                view.move_vertical(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveDown, _, cx| {
                view.move_vertical(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectLeft, _, cx| {
                view.move_horizontal(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectRight, _, cx| {
                view.move_horizontal(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectUp, _, cx| {
                view.move_vertical(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectDown, _, cx| {
                view.move_vertical(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveToStartOfLine, _, cx| {
                view.move_line_boundary(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveToEndOfLine, _, cx| {
                view.move_line_boundary(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectToStartOfLine, _, cx| {
                view.move_line_boundary(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectToEndOfLine, _, cx| {
                view.move_line_boundary(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &Backspace, _, cx| view.backspace(cx)))
            .on_action(cx.listener(|view, _: &Delete, _, cx| view.delete(cx)))
            .on_action(cx.listener(|view, _: &Enter, _, cx| view.insert_newline(cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| view.select_all(cx)))
            .on_action(cx.listener(|view, _: &Undo, _, cx| view.travel_history(false, cx)))
            .on_action(cx.listener(|view, _: &Redo, _, cx| view.travel_history(true, cx)))
            .child(canvas)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
#[path = "editor_tests.rs"]
mod tests;
