use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::ops::Range;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};

use gpui::{
    App, ContentMask, Context, Entity, FocusHandle, Focusable, Font, InteractiveElement as _,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Render,
    ScrollWheelEvent, ShapedLine, StatefulInteractiveElement as _, Styled as _, Task, TextAlign,
    TextRun, Window, canvas, div, fill, point, px, size,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{
    Copy, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine, MoveToStartOfLine, MoveUp, SelectAll,
    SelectToEndOfLine, SelectToStartOfLine,
};
use slterm_document::{
    DocumentId, DocumentPageCache, DocumentSession, DocumentTextRequest, LanguageId, PageKey,
    PagedSource, TextStateId,
};
use slterm_workspace::ViewKey;

use super::layout::{FontMetricsKey, LayoutCache, LayoutMetrics, TextLayoutKey, ThemeFingerprint};
use super::metrics::DisplayMap;
use super::source::{PagedTextSource, TextSelection, VisibleLine, VisibleRow};
use super::view_state::{EditorViewState, EditorViewStateLease, EditorViewStateStore};
use super::{SelectDown, SelectLeft, SelectRight, SelectUp};
use crate::app_shell::config::Settings;
use crate::app_shell::paged_document::{PageReader, PagedReaderChunks};
use crate::i18n::Message;

#[derive(Clone, Copy)]
struct PagedCursor {
    byte: usize,
    line: usize,
    column: Option<usize>,
}

#[derive(Clone, Copy)]
enum PagedMotion {
    Horizontal { left: bool, select: bool },
    Vertical { down: bool, select: bool },
    LineBoundary { end: bool, select: bool },
}

struct PagedMotionOutcome {
    cursor: PagedCursor,
    preferred_column: Option<usize>,
}

#[cfg(all(test, feature = "gpui-test-support"))]
type PaintedLines = Rc<RefCell<Vec<(usize, Option<String>)>>>;

/// 只读分页文本视图；正文只来自真实 PagedSource 与共享页缓存。
pub(crate) struct PagedTextView {
    document_id: DocumentId,
    view_lease: Option<EditorViewStateLease>,
    view_states: Option<EditorViewStateStore>,
    view_state_dirty: bool,
    language_override: Option<LanguageId>,
    wrap: bool,
    source: PagedSource,
    text_state: TextStateId,
    request: DocumentTextRequest,
    selection: TextSelection,
    anchor_line: usize,
    head_line: usize,
    anchor_column: Option<usize>,
    head_column: Option<usize>,
    preferred_column: Option<usize>,
    pending_motion: Option<PagedMotion>,
    mouse_anchor: Option<usize>,
    focus: FocusHandle,
    copy_generation: u64,
    copy_cancel: Option<SyncSender<()>>,
    copy_task: Option<Task<()>>,
    cache: Rc<RefCell<DocumentPageCache>>,
    reader: PageReader,
    pending_pages: BTreeSet<usize>,
    failed_pages: BTreeSet<usize>,
    tasks: BTreeMap<usize, Task<()>>,
    closed: bool,
    scroll_line: usize,
    scroll_column: usize,
    wheel_remainder: f32,
    layout: Rc<RefCell<Option<LayoutMetrics>>>,
    layout_cache: Rc<RefCell<LayoutCache>>,
    theme_revision: u64,
    theme_fingerprint: Option<ThemeFingerprint>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    painted_lines: PaintedLines,
}

struct PagedPaintFrame {
    cell_width: Pixels,
    line_height: Pixels,
    selection: Range<usize>,
    rows: Vec<(usize, Option<(ShapedLine, VisibleLine)>)>,
}

struct PagedTextViewConfig {
    source: PagedSource,
    document_id: DocumentId,
    text_state: TextStateId,
    cache: Rc<RefCell<DocumentPageCache>>,
    reader: PageReader,
    request: DocumentTextRequest,
    view_state: Option<(ViewKey, EditorViewStateStore)>,
}

impl PagedTextView {
    pub(crate) fn new(
        session: &DocumentSession,
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        request: DocumentTextRequest,
        view_key: ViewKey,
        view_states: EditorViewStateStore,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_parts(
            PagedTextViewConfig {
                source: session
                    .paged()
                    .expect("PagedTextView 只接受分页正文")
                    .clone(),
                document_id: session.id(),
                text_state: session.current_state(),
                cache,
                reader,
                request,
                view_state: Some((view_key, view_states)),
            },
            cx,
        )
    }

    /// 固定只读分页快照不注册 Document，也不恢复 Editor 阅读位置。
    pub(crate) fn fixed(
        source: PagedSource,
        document_id: DocumentId,
        text_state: TextStateId,
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        request: DocumentTextRequest,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_parts(
            PagedTextViewConfig {
                source,
                document_id,
                text_state,
                cache,
                reader,
                request,
                view_state: None,
            },
            cx,
        )
    }

    fn from_parts(config: PagedTextViewConfig, cx: &mut Context<'_, Self>) -> Self {
        let PagedTextViewConfig {
            source,
            document_id,
            text_state,
            cache,
            reader,
            request,
            view_state,
        } = config;
        assert_eq!(request.document_id(), document_id);
        assert_eq!(
            request.source_generation(),
            Some(source.source_generation())
        );
        let restored = view_state
            .as_ref()
            .map(|(key, states)| states.state(key))
            .unwrap_or_default();
        let (view_lease, view_states) = view_state.map_or((None, None), |(key, states)| {
            let lease = states.lease(&key);
            (Some(lease), Some(states))
        });
        let scroll_line = restored
            .scroll_line()
            .min(source.line_index().line_count().saturating_sub(1));
        Self {
            document_id,
            view_lease,
            view_states,
            view_state_dirty: false,
            language_override: restored.language_override(),
            wrap: restored.wrap(),
            source,
            text_state,
            request,
            selection: TextSelection { anchor: 0, head: 0 },
            anchor_line: 0,
            head_line: 0,
            anchor_column: Some(0),
            head_column: Some(0),
            preferred_column: None,
            pending_motion: None,
            mouse_anchor: None,
            focus: cx.focus_handle().tab_stop(true),
            copy_generation: 0,
            copy_cancel: None,
            copy_task: None,
            cache,
            reader,
            pending_pages: BTreeSet::new(),
            failed_pages: BTreeSet::new(),
            tasks: BTreeMap::new(),
            closed: false,
            scroll_line,
            scroll_column: restored.scroll_column(),
            wheel_remainder: 0.0,
            layout: Rc::default(),
            layout_cache: Rc::default(),
            theme_revision: 0,
            theme_fingerprint: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_lines: Rc::default(),
        }
    }

    pub(crate) fn renew_view_lease(&mut self) {
        if let (Some(states), Some(lease)) = (&self.view_states, &mut self.view_lease) {
            states.renew(lease);
        }
    }

    pub(crate) fn share_focus(&mut self, focus: FocusHandle) {
        self.focus = focus;
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_lines_for_test(&self) -> Vec<(usize, Option<String>)> {
        self.painted_lines.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn scroll_state_for_test(&self) -> (usize, usize, Range<usize>, bool) {
        (
            self.scroll_line,
            self.scroll_column,
            self.selection.range(),
            self.pending_motion.is_some(),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn is_closed_for_test(&self) -> bool {
        self.closed
    }

    pub(crate) fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.tasks.clear();
        self.pending_pages.clear();
        self.pending_motion = None;
        self.invalidate_copy();
    }

    pub(crate) fn relocate_reader(&mut self, reader: PageReader, cx: &mut Context<'_, Self>) {
        self.reader = reader;
        self.tasks.clear();
        self.pending_pages.clear();
        self.failed_pages.clear();
        self.try_pending_motion(cx);
        cx.notify();
    }

    pub(crate) fn apply_snapshot(
        &mut self,
        session: &DocumentSession,
        request: DocumentTextRequest,
        reader: PageReader,
        cx: &mut Context<'_, Self>,
    ) {
        let source = session
            .paged()
            .expect("PagedTextView 只接受分页正文")
            .clone();
        assert_eq!(request.document_id(), session.id());
        assert_eq!(
            request.source_generation(),
            Some(source.source_generation())
        );
        self.source = source;
        self.text_state = session.current_state();
        self.request = request;
        self.reader = reader;
        self.tasks.clear();
        self.pending_pages.clear();
        self.failed_pages.clear();
        self.pending_motion = None;
        self.mouse_anchor = None;
        self.selection = TextSelection { anchor: 0, head: 0 };
        self.anchor_line = 0;
        self.head_line = 0;
        self.anchor_column = Some(0);
        self.head_column = Some(0);
        self.preferred_column = None;
        self.scroll_line = self
            .scroll_line
            .min(self.source.line_index().line_count().saturating_sub(1));
        self.layout = Rc::default();
        self.layout_cache = Rc::default();
        self.invalidate_copy();
        cx.notify();
    }

    /// 旧代次回执不得结算或污染当前源；失败页在当前视图内不自动重试。
    fn apply_page_result(
        &mut self,
        page: usize,
        generation: u64,
        fingerprint: [u8; 32],
        result: io::Result<String>,
        cx: &mut Context<'_, Self>,
    ) {
        if self.closed
            || generation != self.source.source_generation()
            || fingerprint != *self.source.fingerprint()
        {
            return;
        }
        self.pending_pages.remove(&page);
        match result {
            Ok(text) => {
                let key = PageKey::new(self.document_id, generation, page);
                if !self.cache.borrow_mut().insert(key, text) {
                    self.failed_pages.insert(page);
                }
            }
            Err(_) => {
                self.failed_pages.insert(page);
            }
        }
        self.try_pending_motion(cx);
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<'_, Self>) {
        self.pending_motion = None;
        self.selection = TextSelection {
            anchor: 0,
            head: self.source.line_index().total_utf8_bytes(),
        };
        self.anchor_line = 0;
        self.head_line = self.source.line_index().line_count().saturating_sub(1);
        self.anchor_column = Some(0);
        self.head_column = None;
        self.preferred_column = None;
        cx.notify();
    }

    fn invalidate_copy(&mut self) -> u64 {
        self.copy_generation = self
            .copy_generation
            .checked_add(1)
            .expect("复制代次不得耗尽");
        if let Some(cancel) = self.copy_cancel.take() {
            let _ = cancel.try_send(());
        }
        self.copy_task = None;
        self.copy_generation
    }

    fn apply_copy_result(
        &mut self,
        generation: u64,
        result: Result<String, slterm_document::SearchError>,
        cx: &mut Context<'_, Self>,
    ) {
        if generation != self.copy_generation {
            return;
        }
        self.copy_cancel = None;
        if let Ok(text) = result {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    fn copy(&mut self, cx: &mut Context<'_, Self>) {
        if self.closed {
            return;
        }
        let generation = self.invalidate_copy();
        let range = self.selection.range();
        if range.is_empty() {
            return;
        }
        let request = self.request.clone();
        let reader = self.reader.clone();
        let (cancel, cancellation) = sync_channel(1);
        self.copy_cancel = Some(cancel);
        let background = cx.background_executor().spawn(async move {
            let chunks =
                PagedReaderChunks::new(request.paged().expect("复制请求必须为 Paged"), reader);
            request.copy_range(range, chunks, &cancellation)
        });
        let target = cx.entity().downgrade();
        self.copy_task = Some(cx.spawn(async move |_, cx| {
            let result = background.await;
            let Some(target) = target.upgrade() else {
                return;
            };
            target.update(cx, |view, cx| {
                view.apply_copy_result(generation, result, cx);
            });
        }));
    }

    fn selection_cursor(&self, anchor: bool) -> PagedCursor {
        if anchor {
            PagedCursor {
                byte: self.selection.anchor,
                line: self.anchor_line,
                column: self.anchor_column,
            }
        } else {
            PagedCursor {
                byte: self.selection.head,
                line: self.head_line,
                column: self.head_column,
            }
        }
    }

    fn compute_motion(
        &self,
        source: &mut PagedTextSource<'_>,
        motion: PagedMotion,
    ) -> Option<PagedMotionOutcome> {
        match motion {
            PagedMotion::Horizontal { left, select } => {
                let range = self.selection.range();
                if !select && !range.is_empty() {
                    let anchor = if left {
                        self.selection.anchor <= self.selection.head
                    } else {
                        self.selection.anchor >= self.selection.head
                    };
                    let mut cursor = self.selection_cursor(anchor);
                    if cursor.column.is_none() {
                        cursor.column = Some(source.column_at(cursor.line, cursor.byte)?);
                    }
                    return Some(PagedMotionOutcome {
                        cursor,
                        preferred_column: None,
                    });
                }
                let byte = if left {
                    source.previous_grapheme_boundary(self.selection.head)
                } else {
                    source.next_grapheme_boundary(self.selection.head)
                };
                let Some(byte) = byte else {
                    let at_boundary = if left {
                        self.selection.head == 0
                    } else {
                        self.selection.head == self.source.line_index().total_utf8_bytes()
                    };
                    if !at_boundary {
                        return None;
                    }
                    return Some(PagedMotionOutcome {
                        cursor: self.selection_cursor(false),
                        preferred_column: None,
                    });
                };
                let current = source.line_byte_range(self.head_line)?;
                let line = if byte < current.start {
                    self.head_line.checked_sub(1)?
                } else if byte > current.end {
                    (self.head_line + 1).min(self.source.line_index().line_count() - 1)
                } else {
                    self.head_line
                };
                let column = source.column_at(line, byte)?;
                Some(PagedMotionOutcome {
                    cursor: PagedCursor {
                        byte,
                        line,
                        column: Some(column),
                    },
                    preferred_column: None,
                })
            }
            PagedMotion::Vertical { down, .. } => {
                let target = if down {
                    (self.head_line + 1).min(self.source.line_index().line_count() - 1)
                } else {
                    self.head_line.saturating_sub(1)
                };
                if target == self.head_line {
                    return None;
                }
                let current_column = match self.head_column {
                    Some(column) => Some(column),
                    None => source.column_at(self.head_line, self.selection.head),
                };
                let preferred = match (self.preferred_column, current_column) {
                    (Some(column), _) => column,
                    (None, Some(column)) => column,
                    (None, None) => return None,
                };
                let (byte, column) = source.byte_at_column(target, preferred)?;
                Some(PagedMotionOutcome {
                    cursor: PagedCursor {
                        byte,
                        line: target,
                        column: Some(column),
                    },
                    preferred_column: Some(preferred),
                })
            }
            PagedMotion::LineBoundary { end, .. } => {
                let range = source.line_byte_range(self.head_line)?;
                let byte = if end { range.end } else { range.start };
                let column = if end {
                    source.column_at(self.head_line, byte)?
                } else {
                    0
                };
                Some(PagedMotionOutcome {
                    cursor: PagedCursor {
                        byte,
                        line: self.head_line,
                        column: Some(column),
                    },
                    preferred_column: None,
                })
            }
        }
    }

    fn start_motion(&mut self, motion: PagedMotion, cx: &mut Context<'_, Self>) {
        if self.closed {
            return;
        }
        self.pending_motion = Some(motion);
        self.try_pending_motion(cx);
    }

    fn try_pending_motion(&mut self, cx: &mut Context<'_, Self>) {
        let Some(motion) = self.pending_motion else {
            return;
        };
        let source = self.source.clone();
        let cache = self.cache.clone();
        let (outcome, missing) = {
            let mut cache = cache.borrow_mut();
            let mut text =
                PagedTextSource::new(self.document_id, &source, &mut cache, DisplayMap::new(4));
            let outcome = self.compute_motion(&mut text, motion);
            (
                outcome,
                text.missing_pages().iter().copied().collect::<Vec<_>>(),
            )
        };
        if let Some(outcome) = outcome {
            self.pending_motion = None;
            self.move_to(
                outcome.cursor,
                match motion {
                    PagedMotion::Horizontal { select, .. }
                    | PagedMotion::Vertical { select, .. }
                    | PagedMotion::LineBoundary { select, .. } => select,
                },
                outcome.preferred_column,
                cx,
            );
        } else if missing.is_empty() {
            self.pending_motion = None;
        } else {
            let requests = self.claim_pages(&missing);
            if !missing.iter().any(|page| self.pending_pages.contains(page)) {
                self.pending_motion = None;
                return;
            }
            let tasks = Self::spawn_page_requests(&cx.entity(), requests, cx);
            self.tasks.extend(tasks);
        }
    }

    fn move_to(
        &mut self,
        cursor: PagedCursor,
        select: bool,
        preferred_column: Option<usize>,
        cx: &mut Context<'_, Self>,
    ) {
        if select {
            self.selection.head = cursor.byte;
            self.head_line = cursor.line;
            self.head_column = cursor.column;
        } else {
            self.selection = TextSelection {
                anchor: cursor.byte,
                head: cursor.byte,
            };
            self.anchor_line = cursor.line;
            self.head_line = cursor.line;
            self.anchor_column = cursor.column;
            self.head_column = cursor.column;
        }
        self.preferred_column = preferred_column;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn persist_view_state(&mut self) {
        let (Some(states), Some(lease)) = (&self.view_states, &self.view_lease) else {
            return;
        };
        self.view_state_dirty = true;
        states.checkpoint(
            lease,
            EditorViewState::new(self.scroll_line, self.scroll_column)
                .with_wrap(self.wrap)
                .with_language_override(self.language_override),
        );
    }

    fn ensure_cursor_visible(&mut self) {
        let previous = (self.scroll_line, self.scroll_column);
        let Some(metrics) = *self.layout.borrow() else {
            return;
        };
        let visible_rows = (metrics.bounds.size.height / metrics.line_height)
            .floor()
            .max(1.0) as usize;
        if self.head_line < self.scroll_line {
            self.scroll_line = self.head_line;
        } else if self.head_line >= self.scroll_line + visible_rows {
            self.scroll_line = self.head_line + 1 - visible_rows;
        }
        let Some(column) = self.head_column else {
            if previous != (self.scroll_line, self.scroll_column) {
                self.persist_view_state();
            }
            return;
        };
        let visible_columns = metrics.visible_whole_columns();
        if column < self.scroll_column {
            self.scroll_column = column;
        } else if column > self.scroll_column.saturating_add(visible_columns) {
            self.scroll_column = column - visible_columns;
        }
        if previous != (self.scroll_line, self.scroll_column) {
            self.persist_view_state();
        }
    }

    fn byte_for_point(&self, point: gpui::Point<Pixels>) -> Option<PagedCursor> {
        let metrics = (*self.layout.borrow())?;
        let x = (point.x - metrics.bounds.origin.x).max(px(0.0));
        let y = (point.y - metrics.bounds.origin.y).max(px(0.0));
        let row = self.scroll_line + (y / metrics.line_height).floor() as usize;
        let map = DisplayMap::new(4);
        let mut cache = self.cache.borrow_mut();
        let mut source = PagedTextSource::new(self.document_id, &self.source, &mut cache, map);
        let row = source
            .visible_line_windows(
                row..row + 1,
                self.scroll_column..self.scroll_column.saturating_add(metrics.visible_columns()),
            )
            .into_iter()
            .next()?;
        let line = row.as_text()?;
        let viewport_column = (x / metrics.cell_width).round() as usize;
        let target_column = self.scroll_column.saturating_add(viewport_column);
        let local = map.byte_at_column_from(line.text(), target_column, line.display_start())?;
        let column = map.column_at_from(line.text(), local, line.display_start())?;
        Some(PagedCursor {
            byte: line.byte_range().start + local,
            line: line.index(),
            column: Some(column),
        })
    }

    fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        let Some(position) = self.byte_for_point(event.position) else {
            return;
        };
        let anchor = if event.modifiers.shift {
            self.selection.anchor
        } else {
            position.byte
        };
        if !event.modifiers.shift {
            self.anchor_line = position.line;
            self.anchor_column = position.column;
        }
        self.pending_motion = None;
        self.mouse_anchor = Some(anchor);
        self.selection = TextSelection {
            anchor,
            head: position.byte,
        };
        self.head_line = position.line;
        self.head_column = position.column;
        self.preferred_column = None;
        window.focus(&self.focus, cx);
        self.ensure_cursor_visible();
        cx.notify();
        cx.stop_propagation();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
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
            head: position.byte,
        };
        self.head_line = position.line;
        self.head_column = position.column;
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
        let Some(metrics) = *self.layout.borrow() else {
            return;
        };
        let pixels = event.delta.pixel_delta(metrics.line_height).y.as_f32();
        if !pixels.is_finite() {
            return;
        }
        self.wheel_remainder += pixels;
        let lines = (self.wheel_remainder / metrics.line_height.as_f32().max(1.0)).trunc() as i32;
        self.wheel_remainder -= lines as f32 * metrics.line_height.as_f32();
        let previous = self.scroll_line;
        if lines > 0 {
            self.scroll_line = self.scroll_line.saturating_sub(lines as usize);
        } else if lines < 0 {
            self.scroll_line = self
                .scroll_line
                .saturating_add(lines.unsigned_abs() as usize)
                .min(self.source.line_index().line_count().saturating_sub(1));
        }
        if self.scroll_line != previous {
            self.persist_view_state();
        }
        if lines != 0 {
            cx.notify();
        }
    }

    fn claim_pages(&mut self, pages: &[usize]) -> Vec<(usize, u64, [u8; 32], PageReader)> {
        if self.closed {
            return Vec::new();
        }
        let generation = self.source.source_generation();
        let fingerprint = *self.source.fingerprint();
        pages
            .iter()
            .copied()
            .filter(|page| {
                if self.source.index().page(*page).is_none()
                    || self.pending_pages.contains(page)
                    || self.failed_pages.contains(page)
                {
                    return false;
                }
                let key = PageKey::new(self.document_id, generation, *page);
                if self.cache.borrow_mut().get(&key).is_some() {
                    return false;
                }
                self.pending_pages.insert(*page);
                true
            })
            .map(|page| (page, generation, fingerprint, self.reader.clone()))
            .collect()
    }

    /// 用户显式重试：清空失败抑制并重新请求失败页；不做逐帧自动重试。
    pub(crate) fn retry_failed_pages(&mut self, cx: &mut Context<'_, Self>) {
        if self.closed || self.failed_pages.is_empty() {
            return;
        }
        let pages = std::mem::take(&mut self.failed_pages)
            .into_iter()
            .collect::<Vec<_>>();
        let requests = self.claim_pages(&pages);
        let tasks = Self::spawn_page_requests(&cx.entity(), requests, cx);
        self.tasks.extend(tasks);
        cx.notify();
    }

    /// 缺页读取由 foreground Task 持有；Drop 会级联取消等待中的后台读取。
    fn request_pages(view: &Entity<Self>, pages: Vec<usize>, cx: &mut App) {
        let requests = view.update(cx, |view, _| {
            view.tasks.retain(|_, task| !task.is_ready());
            view.claim_pages(&pages)
        });
        let tasks = Self::spawn_page_requests(view, requests, cx);
        view.update(cx, |view, _| view.tasks.extend(tasks));
    }

    fn spawn_page_requests(
        view: &Entity<Self>,
        requests: Vec<(usize, u64, [u8; 32], PageReader)>,
        cx: &mut App,
    ) -> Vec<(usize, Task<()>)> {
        let mut tasks = Vec::with_capacity(requests.len());
        for (page, generation, fingerprint, reader) in requests {
            let background = cx.background_executor().spawn(async move { reader(page) });
            let target = view.downgrade();
            let task = cx.spawn(async move |cx| {
                let result = background.await;
                let Some(target) = target.upgrade() else {
                    return;
                };
                target.update(cx, |view, cx| {
                    view.apply_page_result(page, generation, fingerprint, result, cx);
                });
            });
            tasks.push((page, task));
        }
        tasks
    }

    fn shape_line(
        line: VisibleLine,
        cache: &mut LayoutCache,
        window: &mut Window,
        font_size: Pixels,
        run: &TextRun,
    ) -> (ShapedLine, VisibleLine) {
        let shaped = cache.get(line.index()).unwrap_or_else(|| {
            let shaped = window.text_system().shape_line(
                line.text().to_owned().into(),
                font_size,
                std::slice::from_ref(run),
                None,
            );
            cache.insert(line.index(), shaped.clone());
            shaped
        });
        (shaped, line)
    }
}

impl Drop for PagedTextView {
    fn drop(&mut self) {
        self.close();
        if self.view_state_dirty {
            self.persist_view_state();
        }
    }
}

impl Focusable for PagedTextView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PagedTextView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let accent = theme.accent;
        let background = theme.background;
        let font_size = theme.mono_font_size;
        let banner_background = theme.muted;
        let banner_foreground = theme.warning_foreground;
        let language = cx
            .try_global::<Settings>()
            .map_or(crate::i18n::UiLanguage::EnUs, |settings| settings.language);
        let page_failed = language.text(Message::PagedPageFailed);
        let retry_label = language.text(Message::PagedRetry);
        let fingerprint = ThemeFingerprint {
            foreground,
            accent: theme.accent,
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
        let run = TextRun {
            len: 0,
            font: font.clone(),
            color: foreground,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let view = cx.entity();
        let document_id = self.document_id;
        let source = self.source.clone();
        let text_state = self.text_state;
        let cache = self.cache.clone();
        let layout_cache = self.layout_cache.clone();
        let layout = self.layout.clone();
        let theme_revision = self.theme_revision;
        let scroll_line = self.scroll_line;
        let scroll_column = self.scroll_column;
        let selection = self.selection.range();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let painted_lines = self.painted_lines.clone();
        let content = canvas(
            move |bounds, window, cx| {
                let line_height = px(20.0);
                let sample = window.text_system().shape_line(
                    "M".into(),
                    font_size,
                    &[TextRun {
                        len: 1,
                        ..run.clone()
                    }],
                    None,
                );
                let cell_width = sample.width;
                *layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width,
                    line_height,
                });
                layout_cache.borrow_mut().activate(TextLayoutKey {
                    document_id,
                    text_state,
                    language_revision: 0,
                    font: FontMetricsKey {
                        family: font.family.clone(),
                        font_size_bits: font_size.as_f32().to_bits(),
                        cell_width_bits: cell_width.as_f32().to_bits(),
                        line_height_bits: line_height.as_f32().to_bits(),
                    },
                    wrap_width_bits: bounds.size.width.as_f32().to_bits(),
                    soft_wrap: false,
                    first_visible_column: scroll_column,
                    dpi_bits: window.scale_factor().to_bits(),
                    theme_revision,
                });
                let height: f32 = bounds.size.height.into();
                let count = (height / line_height.as_f32()).ceil().max(1.0) as usize;
                let columns = (bounds.size.width / cell_width).ceil().max(1.0) as usize;
                let (rows, missing) = {
                    let mut cache = cache.borrow_mut();
                    let mut text =
                        PagedTextSource::new(document_id, &source, &mut cache, DisplayMap::new(4));
                    let rows = text.visible_line_windows(
                        scroll_line..scroll_line + count,
                        scroll_column..scroll_column.saturating_add(columns),
                    );
                    let missing = text.missing_pages().iter().copied().collect::<Vec<_>>();
                    (rows, missing)
                };
                Self::request_pages(&view, missing, cx);
                #[cfg(all(test, feature = "gpui-test-support"))]
                {
                    *painted_lines.borrow_mut() = rows
                        .iter()
                        .map(|row| {
                            (
                                row.index(),
                                row.as_text().map(|line| line.text().to_owned()),
                            )
                        })
                        .collect();
                }
                let mut cache = layout_cache.borrow_mut();
                let rows = rows
                    .into_iter()
                    .map(|row| {
                        let index = row.index();
                        let line = match row {
                            VisibleRow::Text(line) => {
                                let run = TextRun {
                                    len: line.text().len(),
                                    ..run.clone()
                                };
                                Some(Self::shape_line(line, &mut cache, window, font_size, &run))
                            }
                            VisibleRow::Loading { .. } => None,
                        };
                        (index, line)
                    })
                    .collect();
                PagedPaintFrame {
                    cell_width,
                    line_height,
                    selection,
                    rows,
                }
            },
            move |bounds, frame, window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    let map = DisplayMap::new(4);
                    for (row, (_, line)) in frame.rows.iter().enumerate() {
                        if let Some((_, line)) = line {
                            let range = line.byte_range();
                            let start = frame.selection.start.max(range.start);
                            let end = frame.selection.end.min(range.end);
                            if start < end
                                && let (Some(x0), Some(x1)) = (
                                    map.column_at_from(
                                        line.text(),
                                        start - range.start,
                                        line.display_start(),
                                    ),
                                    map.column_at_from(
                                        line.text(),
                                        end - range.start,
                                        line.display_start(),
                                    ),
                                )
                            {
                                fill(
                                    gpui::Bounds {
                                        origin: point(
                                            bounds.origin.x
                                                + frame.cell_width
                                                    * (x0 as f32 - scroll_column as f32),
                                            bounds.origin.y + frame.line_height * row as f32,
                                        ),
                                        size: size(
                                            frame.cell_width * (x1 - x0) as f32,
                                            frame.line_height,
                                        ),
                                    },
                                    accent,
                                );
                            }
                        }
                    }
                    for (row, (_, line)) in frame.rows.iter().enumerate() {
                        if let Some((shaped, line)) = line {
                            let origin = point(
                                bounds.origin.x
                                    + frame.cell_width
                                        * (line.display_start() as f32 - scroll_column as f32),
                                bounds.origin.y + frame.line_height * row as f32,
                            );
                            let _ = shaped.paint(
                                origin,
                                frame.line_height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                });
            },
        )
        .size_full()
        .overflow_hidden()
        .bg(background);
        let mut root = div().relative().size_full().child(content);
        if !self.failed_pages.is_empty() {
            let view = cx.entity();
            root = root.child(
                div()
                    .id("paged-page-retry")
                    .debug_selector(|| "paged-page-retry".to_string())
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .px_3()
                    .py_1()
                    .bg(banner_background)
                    .text_color(banner_foreground)
                    .text_sm()
                    .cursor_pointer()
                    .child(format!("{page_failed} {retry_label}"))
                    .on_click(move |_, _, cx| {
                        view.update(cx, |view, cx| view.retry_failed_pages(cx));
                    }),
            );
        }
        root.key_context("DocumentText")
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|view, _: &MoveLeft, _, cx| {
                view.start_motion(
                    PagedMotion::Horizontal {
                        left: true,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &MoveRight, _, cx| {
                view.start_motion(
                    PagedMotion::Horizontal {
                        left: false,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &MoveUp, _, cx| {
                view.start_motion(
                    PagedMotion::Vertical {
                        down: false,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &MoveDown, _, cx| {
                view.start_motion(
                    PagedMotion::Vertical {
                        down: true,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectLeft, _, cx| {
                view.start_motion(
                    PagedMotion::Horizontal {
                        left: true,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectRight, _, cx| {
                view.start_motion(
                    PagedMotion::Horizontal {
                        left: false,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectUp, _, cx| {
                view.start_motion(
                    PagedMotion::Vertical {
                        down: false,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectDown, _, cx| {
                view.start_motion(
                    PagedMotion::Vertical {
                        down: true,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &MoveToStartOfLine, _, cx| {
                view.start_motion(
                    PagedMotion::LineBoundary {
                        end: false,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &MoveToEndOfLine, _, cx| {
                view.start_motion(
                    PagedMotion::LineBoundary {
                        end: true,
                        select: false,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectToStartOfLine, _, cx| {
                view.start_motion(
                    PagedMotion::LineBoundary {
                        end: false,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectToEndOfLine, _, cx| {
                view.start_motion(
                    PagedMotion::LineBoundary {
                        end: true,
                        select: true,
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| view.select_all(cx)))
            .on_action(cx.listener(|view, _: &Copy, _, cx| view.copy(cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, window, cx| {
                    view.mouse_down(event, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
                view.mouse_move(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, event: &MouseUpEvent, _, _| {
                    view.mouse_up(event);
                }),
            )
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _, cx| {
                view.scroll_wheel(event, cx);
            }))
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    include!("paged_tests.rs");
}
