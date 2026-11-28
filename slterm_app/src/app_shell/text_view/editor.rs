mod brackets;
mod buttons;
mod completion;
mod construct;
mod fold;
mod goto;
mod gutter;
mod input;
mod render;
mod scroll;
mod search;
mod status;
mod transform;
mod wrap_schedule;

use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Bounds, ContentMask, Context, ElementInputHandler, EntityId, EventEmitter, FocusHandle,
    Font, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollWheelEvent, SharedString, Styled as _, Task, TextAlign, TextRun, Window, canvas, fill,
    point, px, size,
};
use gpui_component::ActiveTheme as _;
use ropey::{LineType, Rope};
use slterm_document::{
    DocumentId, DocumentSession, EditApplyOutcome, EditOrigin, EditTransaction,
    EditTransactionError, LanguageId, MergePolicy, TextEdit, TextStateId,
};
use uuid::Uuid;

use self::gutter::line_text_runs;
use self::search::PaintFrame;
use self::wrap_schedule::{SharedWrapBuild, SharedWrapIndex, schedule_wrap_index};
use super::layout::{FontMetricsKey, LayoutCache, LayoutMetrics, TextLayoutKey, ThemeFingerprint};
use super::metrics::{
    DisplayMap, rope_ceil_grapheme_boundary, rope_floor_grapheme_boundary,
    rope_is_grapheme_boundary, rope_next_grapheme_boundary, rope_previous_grapheme_boundary,
};
use super::source::{RopeTextSource, TextSelection};
use super::syntax::DocumentSyntax;
use super::view_state::{EditorViewState, EditorViewStateLease, EditorViewStateStore};
use super::wrap::DisplayPosition;
use super::{
    CloseEditorCompletion, CloseEditorGoto, CloseEditorSearch, CompleteEditor, FindEditor,
    FindNextEditor, FindPreviousEditor, GoToLineEditor, IndentEditor, OutdentEditor, SaveAsFile,
    SaveFile, SelectDown, SelectLeft, SelectRight, SelectUp, ToggleFoldEditor, editor_font_size_px,
    zoom_editor_font_size,
};

/// 一次同步提交成功后回投的只读正文快照。
#[derive(Clone)]
pub(crate) struct DocumentTextSnapshot {
    pub(crate) text: Rope,
    pub(crate) revision: u64,
    pub(crate) text_state: TextStateId,
    pub(crate) read_only: bool,
}

impl DocumentTextSnapshot {
    pub(crate) fn from_session(session: &DocumentSession) -> Self {
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
    pub(crate) snapshot: DocumentTextSnapshot,
    pub(crate) outcome: EditApplyOutcome,
}

/// 视图只提交事务；组合根同步调用唯一 Registry，并立即返回权威快照。
pub(crate) type DocumentEditSink = Box<
    dyn FnMut(
        DocumentEditRequest,
        EntityId,
        &mut App,
    ) -> Result<DocumentEditResponse, EditTransactionError>,
>;

use transform::{BoundaryBias, transform_offset, transform_selection};

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

#[cfg(all(test, feature = "gpui-test-support"))]
type PaintedSyntax = Rc<RefCell<Vec<(usize, Vec<TextRun>)>>>;

pub(crate) enum DocumentTextEvent {
    Save,
    SaveAs,
    OpenLanguageSelector { position: Point<Pixels> },
    OpenInSystem,
    OpenGitDiff { line: usize },
    ToggleLineEnding,
    ToggleManualReadOnly,
}

impl EventEmitter<DocumentTextEvent> for DocumentTextView {}

/// Document 的专用文本视图；Rope 是不可变展示快照，修改只发 core 事务。
pub(crate) struct DocumentTextView {
    document_id: DocumentId,
    view_lease: Option<EditorViewStateLease>,
    view_states: Option<EditorViewStateStore>,
    view_state_dirty: bool,
    text: Rope,
    revision: u64,
    text_state: TextStateId,
    read_only: bool,
    manual_read_only: bool,
    inherently_read_only: bool,
    encoding: slterm_document::TextEncoding,
    edit_sink: DocumentEditSink,
    automatic_language: LanguageId,
    allow_language_override: bool,
    language_override: Option<LanguageId>,
    git_marks: Vec<GitMark>,
    language_revision: u64,
    theme_revision: u64,
    theme_fingerprint: Option<ThemeFingerprint>,
    layout_cache: Rc<RefCell<LayoutCache>>,
    syntax: Option<Rc<RefCell<DocumentSyntax>>>,
    applied_syntax: Option<(u64, u64)>,
    syntax_poll_task: Option<Task<()>>,
    selection: TextSelection,
    preferred_column: Option<usize>,
    marked_range: Option<Range<usize>>,
    scroll_line: usize,
    scroll_column: usize,
    wrap: bool,
    wrap_index: SharedWrapIndex,
    wrap_build: SharedWrapBuild,
    caret_affinity: super::wrap::CaretAffinity,
    wrap_reveal_cursor: bool,
    /// 编辑域固定预留行标槽位；不随瞬态 `git_marks` 显隐变化。
    git_gutter: bool,
    /// 行号与折叠标记槽位；独立于 Git 行标，生产默认开启。
    line_numbers: bool,
    /// 折叠区域与折叠集合全部瞬态；正文变化立即展开。
    fold_state: fold::FoldState,
    /// 跳转行与补全栏均为瞬态，不进入 EditorViewState。
    goto: Option<goto::GoToLineState>,
    completion: Option<completion::CompletionState>,
    /// 索引未就绪时待揭示的逻辑行；索引完成后换算为显示行。
    wrap_reveal_line: Option<usize>,
    wheel_remainder: f32,
    mouse_anchor: Option<usize>,
    /// 查找／替换栏与匹配结果全部瞬态；不进入 EditorViewState。
    search: Option<search::EditorSearchState>,
    focus: FocusHandle,
    layout: Rc<RefCell<Option<LayoutMetrics>>>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_syntax: PaintedSyntax,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_lines: Rc<RefCell<Vec<String>>>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_cursors: Rc<RefCell<Vec<(usize, usize)>>>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_line_numbers: Rc<RefCell<Vec<(usize, usize)>>>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_current_line: Rc<RefCell<Option<usize>>>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_brackets: Rc<RefCell<Vec<usize>>>,
    #[cfg(test)]
    painted_content_mask: Rc<RefCell<Option<gpui::ContentMask<Pixels>>>>,
}

/// Git 行标语义：新增、修改、删除（删除行落在其后的工作区行）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitMarkKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GitMark {
    pub(crate) line: usize,
    pub(crate) kind: GitMarkKind,
}

impl DocumentTextView {
    fn schedule_syntax_poll(&mut self, cx: &mut Context<'_, Self>) {
        let Some(syntax) = self.syntax.clone() else {
            return;
        };
        if self
            .syntax_poll_task
            .as_ref()
            .is_some_and(|task| !task.is_ready())
        {
            return;
        }
        let target = cx.entity().downgrade();
        self.syntax_poll_task = Some(cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                let Some(target) = target.upgrade() else {
                    return;
                };
                let resolved = target.update(cx, |view, cx| {
                    syntax.borrow_mut().poll();
                    let marker = syntax.borrow().current_marker();
                    let changed = view.applied_syntax != marker;
                    view.applied_syntax = marker;
                    if changed {
                        view.language_revision = view
                            .language_revision
                            .checked_add(1)
                            .expect("语言 revision 不得耗尽");
                        view.refresh_fold_regions();
                        cx.notify();
                    }
                    syntax.borrow().is_resolved()
                });
                if resolved {
                    return;
                }
            }
        }));
    }

    fn open_language_selector(&mut self, position: Point<Pixels>, cx: &mut Context<'_, Self>) {
        if !self.allow_language_override {
            return;
        }
        cx.emit(DocumentTextEvent::OpenLanguageSelector { position });
    }

    pub(crate) fn text_snapshot(&self) -> DocumentTextSnapshot {
        DocumentTextSnapshot {
            text: self.text.clone(),
            revision: self.revision,
            text_state: self.text_state,
            read_only: self.read_only,
        }
    }

    /// Git 行标只是最新 core 差分的展示投影，不进入正文或阅读状态。
    pub(crate) fn set_git_marks(&mut self, marks: Vec<GitMark>, cx: &mut Context<'_, Self>) {
        if self.git_marks == marks {
            return;
        }
        self.git_marks = marks;
        cx.notify();
    }

    /// Diff 导航只改变当前定位，不写持久阅读状态。
    pub(crate) fn reveal_line(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        self.reveal_logical_line(line);
        cx.notify();
    }

    pub(crate) fn language_override(&self) -> Option<LanguageId> {
        self.language_override
    }

    pub(crate) fn renew_view_lease(&mut self) {
        if let (Some(states), Some(lease)) = (&self.view_states, &mut self.view_lease) {
            states.renew(lease);
        }
    }

    pub(crate) fn share_focus(&mut self, focus: FocusHandle) {
        self.focus = focus;
    }

    pub(crate) fn share_lease(&mut self, lease: EditorViewStateLease) {
        if let Some(current) = &mut self.view_lease {
            *current = lease;
        }
    }

    pub(crate) fn view_state_access(&self) -> Option<(EditorViewStateStore, EditorViewStateLease)> {
        Some((self.view_states.clone()?, self.view_lease.clone()?))
    }

    pub(crate) fn effective_language(&self, language_override: Option<LanguageId>) -> LanguageId {
        language_override.unwrap_or(self.automatic_language)
    }

    /// 组合根选定同 Document、同语言的共享语法后，才提交单视图覆盖状态。
    pub(crate) fn apply_language_override(
        &mut self,
        language_override: Option<LanguageId>,
        syntax: Option<Rc<RefCell<DocumentSyntax>>>,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.allow_language_override || self.language_override == language_override {
            return;
        }
        self.language_override = language_override;
        self.syntax = syntax;
        self.applied_syntax = None;
        self.syntax_poll_task = None;
        // 折叠区域必须来自当前语言；切换后旧区域立即失效。
        self.fold_state.clear_for_change();
        self.language_revision = self
            .language_revision
            .checked_add(1)
            .expect("语言 revision 不得耗尽");
        self.persist_view_state();
        self.schedule_syntax_poll(cx);
        cx.notify();
    }

    /// 外部改名重算自动语言；手动覆盖保持不变，布局只随有效语言失效。
    pub(crate) fn relocate_language(
        &mut self,
        automatic_language: LanguageId,
        syntax: Option<Rc<RefCell<DocumentSyntax>>>,
        cx: &mut Context<'_, Self>,
    ) {
        if self.automatic_language == automatic_language {
            return;
        }
        self.automatic_language = automatic_language;
        if self.language_override.is_none() {
            self.syntax = syntax;
            self.applied_syntax = None;
            self.syntax_poll_task = None;
            self.fold_state.clear_for_change();
            self.language_revision = self
                .language_revision
                .checked_add(1)
                .expect("语言 revision 不得耗尽");
            self.schedule_syntax_poll(cx);
        }
        cx.notify();
    }

    fn toggle_wrap(&mut self, cx: &mut Context<'_, Self>) {
        self.set_wrap(!self.wrap, cx);
    }

    #[allow(dead_code)] // 消费点归 Diff 显示参数对共享工作区侧的应用。
    pub(crate) fn set_wrap(&mut self, wrap: bool, cx: &mut Context<'_, Self>) {
        if self.wrap == wrap {
            return;
        }
        self.wrap = wrap;
        self.invalidate_wrap_index();
        self.preferred_column = None;
        self.mouse_anchor = None;
        self.caret_affinity = super::wrap::CaretAffinity::AfterWrap;
        self.scroll_column = 0;
        self.wrap_reveal_line = None;
        if self.wrap {
            // 首次检查点只能使用确定有效的显示行 0，不能混入旧逻辑行。
            self.scroll_line = 0;
            self.wrap_reveal_cursor = true;
        } else {
            self.wrap_reveal_cursor = false;
            self.scroll_line = self
                .text
                .byte_to_line_idx(self.selection.head, LineType::LF);
            self.ensure_cursor_visible();
        }
        self.persist_view_state();
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn language_state_for_test(&self) -> (Option<LanguageId>, LanguageId) {
        (
            self.language_override,
            self.language_override.unwrap_or(self.automatic_language),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn syntax_is_current_for_test(&self) -> bool {
        self.syntax.as_ref().is_some_and(|syntax| {
            let syntax = syntax.borrow();
            syntax.revision_for_test() == self.revision && syntax.is_resolved()
        })
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn wrap_enabled_for_test(&self) -> bool {
        self.wrap
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_lines_for_test(&self) -> Vec<String> {
        self.painted_lines.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_cursors_for_test(&self) -> Vec<(usize, usize)> {
        self.painted_cursors.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_line_numbers_for_test(&self) -> Vec<(usize, usize)> {
        self.painted_line_numbers.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_current_line_for_test(&self) -> Option<usize> {
        *self.painted_current_line.borrow()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_brackets_for_test(&self) -> Vec<usize> {
        self.painted_brackets.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn bracket_pair_for_test(&self) -> Option<(usize, usize)> {
        brackets::matching_bracket(&self.text, self.selection.head)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_line_numbers_for_test(&mut self, enabled: bool) {
        self.line_numbers = enabled;
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn refresh_folds_for_test(&mut self) {
        self.refresh_fold_regions();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn toggle_fold_for_test(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        self.toggle_fold_at_line(line, cx);
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn fold_state_for_test(&self) -> (Vec<(usize, usize)>, Vec<usize>) {
        (
            self.fold_state
                .regions()
                .iter()
                .map(|region| (region.start_line, region.end_line))
                .collect(),
            self.fold_state.folded_starts().iter().copied().collect(),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn hidden_row_count_for_test(&self) -> usize {
        self.hidden_rows().iter().map(Range::len).sum()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_scroll_for_test(&mut self, line: usize, column: usize) {
        self.scroll_line = line;
        self.scroll_column = column;
        self.persist_view_state();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn scroll_state_for_test(&self) -> (usize, usize, DocumentId, Range<usize>) {
        (
            self.scroll_line,
            self.scroll_column,
            self.document_id,
            self.selection.range(),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn content_mask_width_for_test(&self) -> Option<f32> {
        self.painted_content_mask
            .borrow()
            .as_ref()
            .map(|mask| f32::from(mask.bounds.size.width))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn git_marks_for_test(&self) -> Vec<(usize, GitMarkKind)> {
        self.git_marks
            .iter()
            .map(|mark| (mark.line, mark.kind))
            .collect()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn input_text_for_test(&self) -> String {
        self.text.to_string()
    }

    /// 只同步只读与编码元数据，不触碰正文、选区或组合态。
    pub(crate) fn apply_document_metadata(
        &mut self,
        session: &DocumentSession,
        cx: &mut Context<'_, Self>,
    ) {
        self.read_only = session.is_read_only();
        self.manual_read_only = session.is_manually_read_only();
        self.inherently_read_only = session.is_inherently_read_only();
        self.encoding = session.encoding().clone();
        cx.notify();
    }

    /// 无编辑增量的外部快照无法保留逻辑位置，必须取消组合并复位选区。
    pub(crate) fn apply_snapshot(&mut self, session: &DocumentSession, cx: &mut Context<'_, Self>) {
        let snapshot = DocumentTextSnapshot::from_session(session);
        if let Some(syntax) = &self.syntax {
            syntax
                .borrow_mut()
                .rebuild(snapshot.text.clone(), snapshot.revision);
        }
        self.adopt_snapshot(snapshot, cx);
        self.selection = TextSelection { anchor: 0, head: 0 };
        self.marked_range = None;
        self.preferred_column = None;
        // 撤销/重做与外部快照只复位瞬态范围：滚动行/列保留当前阅读位置，
        // 越界行已由 adopt_snapshot 按新正文夹紧；不得把视口拉回光标（顶部）。
        self.schedule_syntax_poll(cx);
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
        let snapshot = DocumentTextSnapshot::from_session(session);
        if let Some(syntax) = &self.syntax {
            syntax.borrow_mut().edit(
                &self.text,
                snapshot.text.clone(),
                snapshot.revision,
                transaction,
            );
        }
        self.adopt_snapshot(snapshot, cx);
        self.selection = selection;
        self.marked_range = marked_range;
        self.reconcile_transient_ranges();
        self.preferred_column = None;
        self.ensure_cursor_visible();
        self.schedule_syntax_poll(cx);
        cx.notify();
    }

    fn adopt_snapshot(&mut self, snapshot: DocumentTextSnapshot, cx: &mut Context<'_, Self>) {
        self.text = snapshot.text;
        self.revision = snapshot.revision;
        self.text_state = snapshot.text_state;
        self.read_only = snapshot.read_only;
        // 旧差分行号在新 TextStateId 下不再有效，必须等新后台结果重新投影。
        self.git_marks.clear();
        // 行号与 fold query 区间随正文失效：折叠立即展开，等新语法树再建区域。
        self.fold_state.clear_for_change();
        self.completion = None;
        self.mouse_anchor = None;
        self.caret_affinity = super::wrap::CaretAffinity::AfterWrap;
        self.wrap_reveal_line = None;
        self.invalidate_wrap_index();
        let previous_scroll_line = self.scroll_line;
        if !self.wrap {
            self.scroll_line = self
                .scroll_line
                .min(self.text.len_lines(LineType::LF).saturating_sub(1));
        }
        if self.scroll_line != previous_scroll_line {
            self.persist_view_state();
        }
        // 旧匹配绑定旧 revision；新正文必须重扫，迟到回执由代次丢弃。
        self.refresh_search_after_text_change(cx);
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
        let merge_policy = if range.start == 0 && range.end == self.text.len() {
            MergePolicy::Disallowed
        } else {
            MergePolicy::Adjacent
        };
        let transaction = EditTransaction {
            document_id: self.document_id,
            expected_text_revision: self.revision,
            edits: vec![TextEdit::new(range, text)],
            origin: EditOrigin::UserInput,
            merge_policy,
        };
        let syntax_transaction = transaction.clone();
        let before = self.text.clone();
        let origin = cx.entity().entity_id();
        let Ok(response) = (self.edit_sink)(
            DocumentEditRequest::Edit(transaction, new_state),
            origin,
            cx,
        ) else {
            return;
        };
        let changed = response.outcome != EditApplyOutcome::Unchanged;
        // 原样普通替换保留既有选区；原样 IME 提交仍须落实组合态与组合内选区。
        if !changed && self.marked_range.is_none() && new_marked.is_none() {
            return;
        }
        if changed {
            if let Some(syntax) = &self.syntax {
                syntax.borrow_mut().edit(
                    &before,
                    response.snapshot.text.clone(),
                    response.snapshot.revision,
                    &syntax_transaction,
                );
            }
            self.adopt_snapshot(response.snapshot, cx);
        }
        self.selection = TextSelection {
            anchor: new_selection.start,
            head: new_selection.end,
        };
        self.marked_range = new_marked;
        self.reconcile_transient_ranges();
        self.preferred_column = None;
        self.ensure_cursor_visible();
        if changed {
            self.schedule_syntax_poll(cx);
        }
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
        // 补全弹层打开时 Enter 接受候选，不插入换行。
        if self.accept_completion(cx) {
            return;
        }
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
        self.caret_affinity = super::wrap::CaretAffinity::AfterWrap;
        self.completion = None;
        self.preferred_column = None;
        cx.notify();
    }

    fn copy(&self, cx: &mut Context<'_, Self>) {
        let range = self.selection.range();
        if range.is_empty() {
            return;
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            self.text.slice(range).to_string(),
        ));
    }

    fn paste(&mut self, cx: &mut Context<'_, Self>) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.replace(None, &text, None, false, cx);
    }

    fn travel_history(&mut self, redo: bool, cx: &mut Context<'_, Self>) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        let origin = cx.entity().entity_id();
        let Ok(response) = (self.edit_sink)(DocumentEditRequest::Travel { redo }, origin, cx)
        else {
            return;
        };
        if response.outcome == EditApplyOutcome::Unchanged {
            return;
        }
        let snapshot = response.snapshot;
        if let Some(syntax) = &self.syntax {
            syntax
                .borrow_mut()
                .rebuild(snapshot.text.clone(), snapshot.revision);
        }
        self.adopt_snapshot(snapshot, cx);
        self.selection = TextSelection { anchor: 0, head: 0 };
        self.marked_range = None;
        self.preferred_column = None;
        // 撤销/重做与外部快照只复位瞬态范围：滚动行/列保留当前阅读位置，
        // 越界行已由 adopt_snapshot 按新正文夹紧；不得把视口拉回光标（顶部）。
        self.schedule_syntax_poll(cx);
        cx.notify();
    }

    fn move_to(&mut self, position: usize, select: bool, cx: &mut Context<'_, Self>) {
        self.move_to_affined(position, super::wrap::CaretAffinity::AfterWrap, select, cx);
    }

    fn move_to_affined(
        &mut self,
        position: usize,
        affinity: super::wrap::CaretAffinity,
        select: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if self.marked_range.is_some() {
            return;
        }
        self.caret_affinity = affinity;
        if select {
            self.selection.head = position;
        } else {
            self.selection = TextSelection {
                anchor: position,
                head: position,
            };
        }
        // 光标进入折叠区域时自动展开，禁止把光标画到不可见行。
        self.unfold_line(self.logical_line_of_byte(position));
        self.completion = None;
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
        if self.wrap {
            let Some(target) = self.wrap_index.borrow().as_ref().and_then(|index| {
                index.line_boundary_at(self.selection.head, end, self.caret_affinity)
            }) else {
                return;
            };
            self.move_to_affined(target.byte, target.affinity, select, cx);
            return;
        }
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
        // 补全弹层打开时上下键只导航候选。
        if self.completion.is_some() {
            if self.move_completion(down) {
                cx.notify();
            }
            return;
        }
        let hidden = self.hidden_rows();
        if self.wrap {
            let Some(current) = self.wrap_position(self.selection.head) else {
                return;
            };
            let Some(total_raw) = self.raw_row_count() else {
                return;
            };
            let target_raw = if down {
                fold::next_visible_raw(current.row, &hidden)
            } else {
                fold::previous_visible_raw(current.row, &hidden)
            };
            let Some(target_raw) = target_raw.filter(|row| *row < total_raw) else {
                return;
            };
            if fold::visible_row(target_raw, &hidden).is_none() {
                return;
            }
            let preferred = self.preferred_column.unwrap_or(current.column);
            let target = self.wrap_index.borrow().as_ref().and_then(|index| {
                index.byte_at_with_affinity(DisplayPosition {
                    row: target_raw,
                    column: preferred,
                })
            });
            let Some(target) = target else {
                return;
            };
            self.move_to_affined(target.byte, target.affinity, select, cx);
            self.preferred_column = Some(preferred);
            return;
        }
        let map = DisplayMap::new(4);
        let source = RopeTextSource::new(&self.text, map);
        let row = self.logical_line_of_byte(self.selection.head);
        let total_raw = self.text.len_lines(LineType::LF);
        let target_row = if down {
            fold::next_visible_raw(row, &hidden)
        } else {
            fold::previous_visible_raw(row, &hidden)
        };
        let Some(target_row) = target_row.filter(|row| *row < total_raw) else {
            return;
        };
        if fold::visible_row(target_row, &hidden).is_none() {
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

    fn invalidate_wrap_index(&mut self) {
        *self.wrap_index.borrow_mut() = None;
        *self.wrap_build.borrow_mut() = None;
    }

    /// 可见显示行数（折叠隐藏的行不计入），滚动坐标与光标导航共用。
    fn display_row_count(&self) -> Option<usize> {
        let raw = self.raw_row_count()?;
        Some(fold::visible_row_count(raw, &self.hidden_rows()))
    }

    fn clamped_scroll_line(&self) -> usize {
        let Some(rows) = self.display_row_count() else {
            return self.scroll_line;
        };
        self.scroll_line.min(rows.saturating_sub(1))
    }

    fn wrap_position(&self, byte: usize) -> Option<DisplayPosition> {
        self.wrap_index
            .borrow()
            .as_ref()?
            .position_with_affinity(byte, self.caret_affinity)
    }

    fn persist_view_state(&mut self) {
        let (Some(states), Some(lease)) = (&self.view_states, &self.view_lease) else {
            return;
        };
        self.view_state_dirty = true;
        // 持久位置始终是逻辑行；软换行下由当前索引换算，索引未就绪时只保存确定的行 0。
        let raw = fold::raw_row(self.scroll_line, &self.hidden_rows()).unwrap_or(0);
        let scroll_line = if self.wrap {
            self.wrap_index
                .borrow()
                .as_ref()
                .and_then(|index| index.logical_line_at_display_row(raw))
                .unwrap_or(0)
        } else {
            raw.min(self.text.len_lines(LineType::LF).saturating_sub(1))
        };
        states.checkpoint(
            lease,
            EditorViewState::new(scroll_line, self.scroll_column)
                .with_wrap(self.wrap)
                .with_language_override(self.language_override)
                .with_scroll_unit(super::view_state::ScrollUnit::LogicalLine),
        );
    }

    fn byte_for_point(&self, point: Point<Pixels>) -> Option<usize> {
        let metrics = self.metrics()?;
        let x = point.x - metrics.bounds.origin.x;
        if x < px(0.0) {
            // 行号槽不属于正文命中区；折叠点击由 mouse_down 单独处理。
            return None;
        }
        let y = (point.y - metrics.bounds.origin.y).max(px(0.0));
        let viewport_column = (x / metrics.cell_width).round() as usize;
        let visible_row = self
            .clamped_scroll_line()
            .saturating_add((y / metrics.line_height).floor() as usize);
        let raw = self.raw_row_of_visible(visible_row)?;
        if self.wrap {
            return self.wrap_index.borrow().as_ref()?.byte_at(DisplayPosition {
                row: raw,
                column: viewport_column,
            });
        }
        let map = DisplayMap::new(4);
        let line = RopeTextSource::new(&self.text, map)
            .visible_line_windows(
                raw..raw + 1,
                self.scroll_column..self.scroll_column.saturating_add(metrics.visible_columns()),
            )
            .into_iter()
            .next()?;
        let target_column = self.scroll_column.saturating_add(viewport_column);
        let local = map.byte_at_column_from(line.text(), target_column, line.display_start())?;
        Some(line.byte_range().start + local)
    }

    /// 行号槽点击：命中有折叠区域的行则切换折叠，否则保持选区不变。
    fn gutter_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<'_, Self>) -> bool {
        let Some(metrics) = self.metrics() else {
            return false;
        };
        if event.position.x >= metrics.bounds.origin.x {
            return false;
        }
        let visible_row = self.clamped_scroll_line().saturating_add(
            ((event.position.y - metrics.bounds.origin.y).max(px(0.0)) / metrics.line_height)
                .floor() as usize,
        );
        let Some(raw) = self.raw_row_of_visible(visible_row) else {
            return true;
        };
        let line = if self.wrap {
            let logical = {
                let index = self.wrap_index.borrow();
                index
                    .as_ref()
                    .and_then(|index| index.logical_line_at_display_row(raw))
            };
            let Some(logical) = logical else {
                return true;
            };
            logical
        } else {
            raw
        };
        self.toggle_fold_at_line(line, cx);
        true
    }

    fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.button != MouseButton::Left || self.marked_range.is_some() {
            return;
        }
        if self.gutter_mouse_down(event, cx) {
            cx.stop_propagation();
            return;
        }
        let Some(position) = self.byte_for_point(event.position) else {
            return;
        };
        let anchor = if event.modifiers.shift {
            self.selection.anchor
        } else {
            position
        };
        self.mouse_anchor = Some(anchor);
        self.selection = TextSelection {
            anchor,
            head: position,
        };
        self.caret_affinity = super::wrap::CaretAffinity::AfterWrap;
        self.completion = None;
        self.preferred_column = None;
        window.focus(&self.focus, cx);
        self.ensure_cursor_visible();
        cx.notify();
        cx.stop_propagation();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        if self.marked_range.is_some() {
            self.mouse_anchor = None;
            return;
        }
        let Some(anchor) = self.mouse_anchor else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.mouse_anchor = None;
            return;
        }
        let Some(position) = self.byte_for_point(event.position) else {
            return;
        };
        self.selection = TextSelection {
            anchor,
            head: position,
        };
        self.caret_affinity = super::wrap::CaretAffinity::AfterWrap;
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn mouse_up(&mut self, event: &MouseUpEvent) {
        if event.button == MouseButton::Left {
            self.mouse_anchor = None;
        }
    }

    fn scroll_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<'_, Self>) {
        let Some(metrics) = self.metrics() else {
            return;
        };
        let pixels = event.delta.pixel_delta(metrics.line_height).y.as_f32();
        if !pixels.is_finite() {
            return;
        }
        // Ctrl+滚轮只缩放全局编辑器字号，不滚动正文、不重建 Document。
        if event.modifiers.control {
            if pixels != 0.0 {
                self.zoom_font(if pixels > 0.0 { 1 } else { -1 }, cx);
            }
            return;
        }
        self.wheel_remainder += pixels;
        let lines = (self.wheel_remainder / metrics.line_height.as_f32().max(1.0)).trunc() as i32;
        self.wheel_remainder -= lines as f32 * metrics.line_height.as_f32();
        let Some(row_count) = self.display_row_count() else {
            return;
        };
        let previous = self.scroll_line;
        if lines > 0 {
            self.scroll_line = self.scroll_line.saturating_sub(lines as usize);
        } else if lines < 0 {
            self.scroll_line = self
                .scroll_line
                .saturating_add(lines.unsigned_abs() as usize)
                .min(row_count.saturating_sub(1));
        }
        if self.scroll_line != previous {
            self.persist_view_state();
        }
        cx.notify();
    }

    fn metrics(&self) -> Option<LayoutMetrics> {
        *self.layout.borrow()
    }

    fn logical_line_of_byte(&self, byte: usize) -> usize {
        self.text
            .byte_to_line_idx(byte.min(self.text.len()), LineType::LF)
    }

    /// 折叠后的原始行总数：非换行即逻辑行，换行为折行显示行。
    fn raw_row_count(&self) -> Option<usize> {
        if self.wrap {
            self.wrap_index
                .borrow()
                .as_ref()
                .map(|index| index.display_row_count())
        } else {
            Some(self.text.len_lines(LineType::LF))
        }
    }

    fn hidden_rows(&self) -> Vec<Range<usize>> {
        let Some(total) = self.raw_row_count() else {
            return Vec::new();
        };
        self.fold_state.hidden_ranges(
            self.wrap,
            self.wrap_index.borrow().as_ref(),
            &self.text,
            total,
        )
    }

    fn raw_row_of_visible(&self, visible: usize) -> Option<usize> {
        let raw = fold::raw_row(visible, &self.hidden_rows())?;
        (raw < self.raw_row_count()?).then_some(raw)
    }

    /// 折叠起始行来自当前语言 fold query；新树到达才刷新。
    fn refresh_fold_regions(&mut self) {
        let regions = self
            .syntax
            .as_ref()
            .map(|syntax| {
                let syntax = syntax.borrow();
                fold::regions_from_ranges(
                    &self.text,
                    &syntax.fold_ranges(&self.text, self.revision),
                )
            })
            .unwrap_or_default();
        self.fold_state.set_regions(regions);
    }

    /// 折叠或展开光标所在区域；无区域时零副作用。
    fn toggle_fold_at_line(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        if self.fold_state.toggle(line).is_some() {
            self.completion = None;
            cx.notify();
        }
    }

    fn unfold_line(&mut self, line: usize) -> bool {
        self.fold_state.unfold_line(line)
    }

    /// Ctrl+滚轮缩放共享字号；布局缓存随 finger 字号失效。
    fn zoom_font(&mut self, steps: i32, cx: &mut Context<'_, Self>) {
        let before = editor_font_size_px();
        let after = zoom_editor_font_size(steps);
        if before != after {
            // 度量变化由下一帧的字体指纹自然失效布局缓存。
            self.theme_fingerprint = None;
            cx.notify();
        }
    }

    /// 当前光标在可见行坐标系中的行列；弹层定位与测试共用。
    fn cursor_viewport_row_column(&self) -> Option<(usize, usize)> {
        let raw = if self.wrap {
            self.wrap_position(self.selection.head)?.row
        } else {
            self.logical_line_of_byte(self.selection.head)
        };
        let visible = fold::visible_row(raw, &self.hidden_rows())?;
        let row = visible.checked_sub(self.clamped_scroll_line())?;
        let column = if self.wrap {
            self.wrap_position(self.selection.head)?.column
        } else {
            let line = self.logical_line_of_byte(self.selection.head);
            let line_start = self.text.line_to_byte_idx(line, LineType::LF);
            DisplayMap::new(4).column_at_rope(
                self.text
                    .slice(self.text.line_to_byte_idx(line, LineType::LF)..self.text.len()),
                self.selection.head - line_start,
            )?
        };
        Some((row, column))
    }

    /// 缩进/反缩进选中行或插入光标位置；整次操作是一个撤销组。
    fn indent(&mut self, outdent: bool, cx: &mut Context<'_, Self>) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        let selection = self.selection.range();
        let first_line = self.logical_line_of_byte(selection.start);
        // 选区终点落在下一行行首时不得把该行算入缩进范围。
        let last_line = if selection.end > selection.start {
            self.logical_line_of_byte(selection.end.saturating_sub(1))
        } else {
            first_line
        };
        let line_count = self.text.len_lines(LineType::LF);
        let last_line = last_line.min(line_count.saturating_sub(1));
        let mut edits = Vec::new();
        for line in first_line..=last_line {
            let start = self.text.line_to_byte_idx(line, LineType::LF);
            if outdent {
                let line_end = self
                    .text
                    .line_to_byte_idx((line + 1).min(line_count), LineType::LF);
                let mut remove = 0;
                while remove < 4
                    && start + remove < line_end
                    && self.text.byte(start + remove) == b' '
                {
                    remove += 1;
                }
                if remove == 0 && start < line_end && self.text.byte(start) == b'\t' {
                    remove = 1;
                }
                if remove > 0 {
                    edits.push(TextEdit::new(start..start + remove, ""));
                }
            } else {
                edits.push(TextEdit::new(start..start, "    "));
            }
        }
        if edits.is_empty() {
            return;
        }
        let before = self.text.clone();
        let new_state = TextStateId::from_uuid(Uuid::now_v7()).expect("系统 UUIDv7 必须有效");
        let transaction = EditTransaction {
            document_id: self.document_id,
            expected_text_revision: self.revision,
            edits,
            origin: EditOrigin::UserInput,
            merge_policy: MergePolicy::Disallowed,
        };
        let syntax_transaction = transaction.clone();
        let origin = cx.entity().entity_id();
        let Ok(response) = (self.edit_sink)(
            DocumentEditRequest::Edit(transaction, new_state),
            origin,
            cx,
        ) else {
            return;
        };
        if response.outcome == EditApplyOutcome::Unchanged {
            return;
        }
        if let Some(syntax) = &self.syntax {
            syntax.borrow_mut().edit(
                &before,
                response.snapshot.text.clone(),
                response.snapshot.revision,
                &syntax_transaction,
            );
        }
        self.adopt_snapshot(response.snapshot, cx);
        self.selection = TextSelection {
            anchor: selection.start,
            head: selection.end,
        };
        self.reconcile_transient_ranges();
        self.preferred_column = None;
        self.ensure_cursor_visible();
        self.schedule_syntax_poll(cx);
        cx.notify();
    }
}

impl DocumentTextView {
    fn text_canvas(&mut self, cx: &mut Context<'_, Self>) -> gpui::Canvas<PaintFrame> {
        if !self.wrap || self.wrap_index.borrow().is_some() {
            // 布局修正只更新运行态；后续显式滚动才会写回新位置。
            self.scroll_line = self.clamped_scroll_line();
        }
        let theme = cx.theme();
        let foreground = theme.foreground;
        let accent = theme.accent;
        let muted = theme.muted_foreground;
        let background = theme.background;
        // 全局编辑器字号独立于终端字号；指纹变化驱动布局缓存失效。
        let font_size = px(editor_font_size_px());
        let fingerprint = ThemeFingerprint {
            foreground,
            accent,
            muted_foreground: theme.muted_foreground,
            info_foreground: theme.info_foreground,
            success_foreground: theme.success_foreground,
            warning_foreground: theme.warning_foreground,
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
        let syntax = self.syntax.clone();
        let revision = self.revision;
        let syntax_colors = cx.theme().colors;
        let selection = self.selection.range();
        let cursor = self.selection.head;
        let cursor_after_wrap = self.wrap
            && self.caret_affinity == super::wrap::CaretAffinity::AfterWrap
            && self
                .wrap_position(cursor)
                .is_some_and(|position| position.row > 0 && position.column == 0);
        let cursor_before_wrap =
            self.wrap && self.caret_affinity == super::wrap::CaretAffinity::BeforeWrap;
        let scroll_line = self.scroll_line;
        let scroll_column = self.scroll_column;
        let wrap = self.wrap;
        let wrap_index = self.wrap_index.clone();
        let wrap_build = self.wrap_build.clone();
        let layout = self.layout.clone();
        let layout_cache = self.layout_cache.clone();
        let document_id = self.document_id;
        let text_state = self.text_state;
        let language_revision = self.language_revision;
        let theme_revision = self.theme_revision;
        let (search_matches, active_match) = self.search_paint_ranges();
        let focus = self.focus.clone();
        let view = cx.entity();
        let wrap_view = view.downgrade();
        #[cfg(test)]
        let painted_content_mask = self.painted_content_mask.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_syntax = self.painted_syntax.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_lines = self.painted_lines.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_cursors = self.painted_cursors.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_line_numbers = self.painted_line_numbers.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_current_line = self.painted_current_line.clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_brackets = self.painted_brackets.clone();
        // 折叠、行号、当前行与括号全部是瞬态绘制输入。
        let show_line_numbers = self.line_numbers;
        let line_digits = self
            .text
            .len_lines(LineType::LF)
            .max(1)
            .ilog10()
            .saturating_add(1) as usize;
        let hidden = self.hidden_rows();
        let cursor_logical = self.logical_line_of_byte(cursor);
        let cursor_raw = if self.wrap {
            self.wrap_position(cursor).map(|position| position.row)
        } else {
            Some(cursor_logical)
        };
        let cursor_visible_row = cursor_raw.and_then(|raw| fold::visible_row(raw, &hidden));
        let fold_visible_rows = if show_line_numbers {
            let index = self.wrap_index.borrow();
            self.fold_state
                .regions()
                .iter()
                .filter_map(|region| {
                    let raw = if self.wrap {
                        let byte = self.text.line_to_byte_idx(region.start_line, LineType::LF);
                        index
                            .as_ref()?
                            .position_with_affinity(byte, super::wrap::CaretAffinity::AfterWrap)?
                            .row
                    } else {
                        region.start_line
                    };
                    let visible = fold::visible_row(raw, &hidden)?;
                    Some((visible, self.fold_state.is_folded(region.start_line)))
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let bracket_bytes = brackets::matching_bracket(&self.text, cursor)
            .map(|(open, close)| vec![open, close])
            .unwrap_or_default();
        let paint_font = font.clone();
        canvas(
            move |bounds, window, cx| {
                let line_height = px((font_size.as_f32() * 1.45).round().max(12.0));
                let map = DisplayMap::new(4);
                let make_run = |len, color| TextRun {
                    len,
                    font: font.clone(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let sample = window.text_system().shape_line(
                    SharedString::new_static("M"),
                    font_size,
                    &[make_run(1, foreground)],
                    None,
                );
                let cell_width = sample.width;
                // 行号槽扣除后才是正文宽度；测量与 wrap 索引共用该宽度。
                let gutter_width = if show_line_numbers {
                    cell_width * (line_digits as f32 + 2.5)
                } else {
                    px(0.0)
                };
                let text_bounds = Bounds {
                    origin: point(bounds.origin.x + gutter_width, bounds.origin.y),
                    size: size(
                        px((bounds.size.width.as_f32() - gutter_width.as_f32()).max(1.0)),
                        bounds.size.height,
                    ),
                };
                let metrics = LayoutMetrics {
                    bounds: text_bounds,
                    cell_width,
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
                    wrap_width_bits: text_bounds.size.width.as_f32().to_bits(),
                    soft_wrap: wrap,
                    first_visible_column: scroll_column,
                    dpi_bits: window.scale_factor().to_bits(),
                    theme_revision,
                });
                *layout.borrow_mut() = Some(metrics);
                let height: f32 = bounds.size.height.into();
                let count = (height / line_height.as_f32()).ceil().max(1.0) as usize;
                let mut cache = layout_cache.borrow_mut();
                #[cfg(all(test, feature = "gpui-test-support"))]
                {
                    painted_lines.borrow_mut().clear();
                    painted_line_numbers.borrow_mut().clear();
                    painted_brackets.borrow_mut().clear();
                    *painted_current_line.borrow_mut() = None;
                }
                if wrap {
                    schedule_wrap_index(
                        text.clone(),
                        text_state,
                        metrics.visible_whole_columns().max(1),
                        wrap_index.clone(),
                        wrap_build.clone(),
                        wrap_view.clone(),
                        cx,
                    );
                }
                let current_row = cursor_visible_row
                    .and_then(|visible| visible.checked_sub(scroll_line))
                    .filter(|row| *row < count);
                let fold_markers = fold_visible_rows
                    .iter()
                    .filter_map(|(visible, folded)| {
                        visible
                            .checked_sub(scroll_line)
                            .filter(|row| *row < count)
                            .map(|row| (row, *folded))
                    })
                    .collect::<Vec<_>>();
                let index = wrap_index.borrow();
                if wrap && index.is_none() {
                    return PaintFrame {
                        lines: Vec::new(),
                        selection,
                        cursor,
                        search_matches: search_matches.clone(),
                        active_match: active_match.clone(),
                        layout: metrics,
                        wrap,
                        cursor_after_wrap,
                        cursor_before_wrap,
                        line_numbers: Vec::new(),
                        fold_markers,
                        current_row,
                        bracket_bytes: bracket_bytes.clone(),
                    };
                }
                let total_raw = if wrap {
                    index.as_ref().map_or(0, |index| index.display_row_count())
                } else {
                    text.len_lines(LineType::LF)
                };
                let source = RopeTextSource::new(&text, map);
                let mut visible_lines = Vec::new();
                let mut line_numbers = Vec::new();
                for offset in 0..count {
                    let visible_row = scroll_line.saturating_add(offset);
                    let Some(raw) = fold::raw_row(visible_row, &hidden) else {
                        break;
                    };
                    if raw >= total_raw {
                        break;
                    }
                    let line = if wrap {
                        index
                            .as_ref()
                            .and_then(|index| index.visible_lines(raw..raw + 1).into_iter().next())
                    } else {
                        source
                            .visible_line_windows(
                                raw..raw + 1,
                                scroll_column
                                    ..scroll_column.saturating_add(metrics.visible_columns()),
                            )
                            .into_iter()
                            .next()
                    };
                    let Some(line) = line else {
                        continue;
                    };
                    if show_line_numbers && line.display_start() == 0 {
                        line_numbers.push((offset, line.index() + 1));
                    }
                    #[cfg(all(test, feature = "gpui-test-support"))]
                    painted_lines.borrow_mut().push(line.text().to_owned());
                    let shaped = cache.get(line.byte_range().start).unwrap_or_else(|| {
                        let spans = syntax
                            .as_ref()
                            .map(|syntax| {
                                syntax
                                    .borrow()
                                    .highlight_line(&text, line.byte_range(), revision)
                            })
                            .unwrap_or_default();
                        let runs = line_text_runs(&line, spans, &font, foreground, &syntax_colors);
                        #[cfg(all(test, feature = "gpui-test-support"))]
                        painted_syntax
                            .borrow_mut()
                            .push((line.index(), runs.clone()));
                        let shaped = window.text_system().shape_line(
                            SharedString::from(line.text().to_owned()),
                            font_size,
                            &runs,
                            None,
                        );
                        cache.insert(line.byte_range().start, shaped.clone());
                        shaped
                    });
                    visible_lines.push((shaped, line));
                }
                #[cfg(all(test, feature = "gpui-test-support"))]
                {
                    painted_line_numbers
                        .borrow_mut()
                        .extend(line_numbers.iter().copied());
                    if current_row.is_some() {
                        *painted_current_line.borrow_mut() = Some(cursor_logical);
                    }
                    painted_brackets
                        .borrow_mut()
                        .extend(bracket_bytes.iter().copied());
                }
                PaintFrame {
                    lines: visible_lines,
                    selection,
                    cursor,
                    search_matches,
                    active_match,
                    layout: metrics,
                    wrap,
                    cursor_after_wrap,
                    cursor_before_wrap,
                    line_numbers,
                    fold_markers,
                    current_row,
                    bracket_bytes,
                }
            },
            move |bounds, frame, window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    #[cfg(test)]
                    {
                        *painted_content_mask.borrow_mut() = Some(window.content_mask());
                    }
                    let map = DisplayMap::new(4);
                    let text_origin = frame.layout.bounds.origin;
                    let cell_width = frame.layout.cell_width;
                    let line_height = frame.layout.line_height;
                    #[cfg(all(test, feature = "gpui-test-support"))]
                    painted_cursors.borrow_mut().clear();
                    // 当前行背景横跨行号槽与正文，使用主题 accent 低权重。
                    if let Some(row) = frame.current_row {
                        fill(
                            Bounds {
                                origin: point(
                                    bounds.origin.x,
                                    text_origin.y + line_height * row as f32,
                                ),
                                size: size(bounds.size.width, line_height),
                            },
                            accent.opacity(0.08),
                        );
                    }
                    for (highlight, color) in
                        search::search_highlight_rects(&frame, scroll_column, &map, cx)
                    {
                        fill(highlight, color);
                    }
                    for byte in &frame.bracket_bytes {
                        for (row, (_, line)) in frame.lines.iter().enumerate() {
                            let range = line.byte_range();
                            if *byte < range.start || *byte > range.end {
                                continue;
                            }
                            let horizontal_start = if frame.wrap {
                                line.display_start()
                            } else {
                                scroll_column
                            };
                            if let Some(column) = map.column_at_from(
                                line.text(),
                                *byte - range.start,
                                line.display_start(),
                            ) {
                                fill(
                                    Bounds {
                                        origin: point(
                                            text_origin.x
                                                + cell_width
                                                    * (column as f32 - horizontal_start as f32),
                                            text_origin.y + line_height * row as f32,
                                        ),
                                        size: size(cell_width, line_height),
                                    },
                                    accent.opacity(0.3),
                                );
                            }
                            break;
                        }
                    }
                    for (row, (_, line)) in frame.lines.iter().enumerate() {
                        let range = line.byte_range();
                        let horizontal_start = if frame.wrap {
                            line.display_start()
                        } else {
                            scroll_column
                        };
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
                                            text_origin.x
                                                + cell_width
                                                    * (x0 as f32 - horizontal_start as f32),
                                            text_origin.y + line_height * row as f32,
                                        ),
                                        size: size(cell_width * (x1 - x0) as f32, line_height),
                                    },
                                    accent,
                                );
                            }
                        }
                        let next_owns_cursor = frame.cursor_after_wrap && range.end == frame.cursor;
                        if range.start <= frame.cursor
                            && !(frame.cursor_before_wrap && range.start == frame.cursor)
                            && (frame.cursor < range.end
                                || (frame.cursor == range.end && !next_owns_cursor))
                            && let Some(column) = map.column_at_from(
                                line.text(),
                                frame.cursor - range.start,
                                line.display_start(),
                            )
                        {
                            #[cfg(all(test, feature = "gpui-test-support"))]
                            painted_cursors.borrow_mut().push((row, frame.cursor));
                            fill(
                                Bounds {
                                    origin: point(
                                        text_origin.x
                                            + cell_width
                                                * (column as f32 - horizontal_start as f32),
                                        text_origin.y + line_height * row as f32,
                                    ),
                                    size: size(px(1.0), line_height),
                                },
                                accent,
                            );
                        }
                    }
                    for (row, (shaped, line)) in frame.lines.iter().enumerate() {
                        let horizontal_start = if frame.wrap {
                            line.display_start()
                        } else {
                            scroll_column
                        };
                        let origin = point(
                            text_origin.x
                                + cell_width
                                    * (line.display_start() as f32 - horizontal_start as f32),
                            text_origin.y + line_height * row as f32,
                        );
                        let _ =
                            shaped.paint(origin, line_height, TextAlign::Left, None, window, cx);
                    }
                    // 行号右对齐到正文左缘；当前行使用 accent。
                    for (row, number) in &frame.line_numbers {
                        let label = number.to_string();
                        let color = if Some(*row) == frame.current_row {
                            accent
                        } else {
                            muted
                        };
                        let run = TextRun {
                            len: label.len(),
                            font: paint_font.clone(),
                            color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(
                            SharedString::from(label),
                            font_size,
                            &[run],
                            None,
                        );
                        let x = text_origin.x - cell_width * 0.75 - shaped.width();
                        let _ = shaped.paint(
                            point(x, text_origin.y + line_height * *row as f32),
                            line_height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                    // 折叠标记：+ 折叠、- 展开，仅在区域起始行出现。
                    for (row, folded) in &frame.fold_markers {
                        let label = if *folded { "+" } else { "-" };
                        let run = TextRun {
                            len: 1,
                            font: paint_font.clone(),
                            color: muted,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(
                            SharedString::new_static(label),
                            font_size,
                            &[run],
                            None,
                        );
                        let _ = shaped.paint(
                            point(
                                bounds.origin.x + cell_width * 0.25,
                                text_origin.y + line_height * *row as f32,
                            ),
                            line_height,
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

#[cfg(all(test, feature = "gpui-test-support"))]
#[path = "editor_tests.rs"]
mod tests;
