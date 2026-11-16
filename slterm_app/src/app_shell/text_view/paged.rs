use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::ops::Range;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};

use gpui::{
    App, ContentMask, Context, Entity, FocusHandle, Focusable, Font, InteractiveElement as _,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Render,
    ScrollWheelEvent, ShapedLine, Styled as _, Task, TextAlign, TextRun, Window, canvas, div, fill,
    point, px, size,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{
    Copy, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine, MoveToStartOfLine, MoveUp, SelectAll,
    SelectToEndOfLine, SelectToStartOfLine,
};
use slterm_document::{
    DocumentId, DocumentPageCache, DocumentSession, DocumentTextRequest, PageKey, PagedSource,
    TextStateId,
};

use super::layout::{FontMetricsKey, LayoutCache, LayoutMetrics, TextLayoutKey, ThemeFingerprint};
use super::metrics::DisplayMap;
use super::source::{PagedTextSource, TextSelection, VisibleLine, VisibleRow};
use super::{SelectDown, SelectLeft, SelectRight, SelectUp};
use crate::app_shell::paged_document::{PageReader, PagedReaderChunks};

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

impl PagedTextView {
    pub(crate) fn new(
        session: &DocumentSession,
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        request: DocumentTextRequest,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let source = session
            .paged()
            .expect("PagedTextView 只接受分页正文")
            .clone();
        assert_eq!(request.document_id(), session.id());
        assert_eq!(
            request.source_generation(),
            Some(source.source_generation())
        );
        Self {
            document_id: session.id(),
            source,
            text_state: session.current_state(),
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
            scroll_line: 0,
            scroll_column: 0,
            wheel_remainder: 0.0,
            layout: Rc::default(),
            layout_cache: Rc::default(),
            theme_revision: 0,
            theme_fingerprint: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_lines: Rc::default(),
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn painted_lines_for_test(&self) -> Vec<(usize, Option<String>)> {
        self.painted_lines.borrow().clone()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn is_closed_for_test(&self) -> bool {
        self.closed
    }

    /// 最后布局引用消失时立即关闭读取面，迟到回执不得复活共享缓存。
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

    fn ensure_cursor_visible(&mut self) {
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
            return;
        };
        let visible_columns = metrics.visible_whole_columns();
        if column < self.scroll_column {
            self.scroll_column = column;
        } else if column > self.scroll_column + visible_columns {
            self.scroll_column = column - visible_columns;
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
                self.scroll_column..self.scroll_column + metrics.visible_columns(),
            )
            .into_iter()
            .next()?;
        let line = row.as_text()?;
        let viewport_column = (x / metrics.cell_width).round() as usize;
        let target_column = self.scroll_column + viewport_column;
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
        if lines > 0 {
            self.scroll_line = self.scroll_line.saturating_sub(lines as usize);
        } else if lines < 0 {
            self.scroll_line = self
                .scroll_line
                .saturating_add(lines.unsigned_abs() as usize)
                .min(self.source.line_index().line_count().saturating_sub(1));
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
                        scroll_column..scroll_column + columns,
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
        div()
            .key_context("DocumentText")
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
            .child(content)
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests {
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use gpui::{AppContext as _, Entity, Render, TestAppContext, Window, px};
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentPageCache, DocumentRegistry, PageIndex,
        PagedLineIndex, PagedSource, SourceByteRange, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};
    use uuid::Uuid;

    use super::{PagedMotion, PagedTextView};
    use crate::app_shell::paged_document::PageReader;

    fn source_for(pages: &[&str]) -> PagedSource {
        let mut offset = 0_u64;
        let mut ranges = Vec::with_capacity(pages.len());
        let mut stats = Vec::with_capacity(pages.len());
        for page in pages {
            let end = offset + page.len() as u64;
            ranges.push(SourceByteRange::new(offset, end).unwrap());
            stats.push((
                page.bytes().filter(|byte| *byte == b'\n').count(),
                page.len(),
                page.ends_with('\n'),
            ));
            offset = end;
        }
        PagedSource::new(
            PageIndex::new(offset, ranges).unwrap(),
            [21; 32],
            PagedLineIndex::from_page_stats(&stats),
        )
    }

    fn make_view(
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        source: PagedSource,
        cx: &mut gpui::App,
    ) -> Entity<PagedTextView> {
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/paged.txt").unwrap(), None),
                source,
                TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                Arc::<[u8]>::default(),
                DocumentMetadata {
                    read_only: true,
                    ..DocumentMetadata::default()
                },
            )
            .unwrap();
        let request = registry.text_request(id, 0).unwrap();
        cx.new(|cx| PagedTextView::new(registry.session(id).unwrap(), cache, reader, request, cx))
    }

    struct Host {
        view: Entity<PagedTextView>,
    }

    impl Host {
        fn new(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            Self::with_source(cache, reader, source_for(&["hello\n", "world\n"]), cx)
        }

        fn with_source(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            source: PagedSource,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            let view = make_view(cache, reader, source, cx);
            Self { view }
        }
    }

    impl Render for Host {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui::Context<'_, Self>,
        ) -> impl gpui::IntoElement {
            self.view.clone()
        }
    }

    #[gpui::test]
    fn paged_view_loads_missing_pages_into_shared_cache(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "hello\n".to_owned(),
                1 => "world\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });

        let painted = window
            .update(cx, |host, _, cx| host.view.read(cx).painted_lines.clone())
            .unwrap();
        assert_eq!(painted.borrow().len(), 3);
        visual.run_until_parked();
        assert_eq!(cache.borrow().len(), 2);

        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        assert_eq!(
            painted.borrow().as_slice(),
            &[
                (0, Some("hello".to_owned())),
                (1, Some("world".to_owned())),
                (2, Some(String::new())),
            ]
        );
        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        let tasks = window
            .update(cx, |host, _, cx| host.view.read(cx).tasks.len())
            .unwrap();
        assert_eq!(tasks, 0);
    }

    #[gpui::test]
    fn paged_view_does_not_retry_failed_pages_on_every_frame(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "读取失败"))
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        for _ in 0..2 {
            visual.refresh().unwrap();
            visual.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
        }

        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        assert!(cache.borrow().is_empty());
        let failed = window
            .update(cx, |host, _, cx| host.view.read(cx).failed_pages.len())
            .unwrap();
        assert_eq!(failed, 2);
    }

    #[gpui::test]
    fn paged_view_scrolls_to_request_a_later_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let first = (0..100).map(|line| format!("{line}\n")).collect::<String>();
        let pages = Arc::new(vec![first, "last\n".to_owned()]);
        let page_refs = pages.iter().map(String::as_str).collect::<Vec<_>>();
        let source = source_for(&page_refs);
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader_pages = pages.clone();
        let reader: PageReader = Arc::new(move |page| {
            reader_pages
                .get(page)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "页号越界"))
        });
        let window = cx.add_window(|_, cx| Host::with_source(cache.clone(), reader, source, cx));
        window
            .update(cx, |_, window, _| {
                window.resize(gpui::size(px(160.0), px(40.0)))
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 0))
                .is_some()
        );
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_none()
        );

        let metrics = window
            .update(cx, |host, _, cx| *host.view.read(cx).layout.borrow())
            .unwrap()
            .unwrap();
        visual.simulate_event(gpui::ScrollWheelEvent {
            position: metrics.bounds.center(),
            delta: gpui::ScrollDelta::Lines(gpui::point(0.0, -100.0)),
            ..Default::default()
        });
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_some()
        );
    }

    #[gpui::test]
    fn paged_view_deduplicates_inflight_page_requests(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok("only\n".to_owned())
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        window
            .update(cx, |host, _, cx| {
                PagedTextView::request_pages(&host.view, vec![0], cx);
                PagedTextView::request_pages(&host.view, vec![0], cx);
            })
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        assert_eq!(requests.lock().unwrap().as_slice(), &[0]);
    }

    #[gpui::test]
    fn destroying_paged_view_cancels_requests_and_ignores_late_results(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("late\n".to_owned()));
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        let weak = window
            .update(cx, |host, _, cx| {
                let weak = host.view.downgrade();
                PagedTextView::request_pages(&host.view, vec![0], cx);
                weak
            })
            .unwrap();
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();

        assert!(weak.upgrade().is_none(), "在途任务不得强持已销毁视图");
        assert!(cache.borrow().is_empty(), "迟到回执不得进入共享缓存");
    }

    #[gpui::test]
    fn closing_paged_view_cancels_requests_and_ignores_late_results(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("late\n".to_owned()));
        let view = cx.update(|cx| make_view(cache.clone(), reader, source_for(&["only\n"]), cx));
        cx.update(|cx| PagedTextView::request_pages(&view, vec![0], cx));
        view.update(cx, |view, _| view.close());
        cx.run_until_parked();

        assert!(cache.borrow().is_empty());
        cx.update(|cx| {
            assert!(view.read(cx).tasks.is_empty());
            assert!(view.read(cx).pending_pages.is_empty());
        });
    }

    #[gpui::test]
    fn paged_copy_reads_selected_uncached_pages_without_filling_view_cache(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "ab".to_owned(),
                1 => "cd".to_owned(),
                2 => "ef\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["ab", "cd", "ef\n"]), cx));
        view.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 1, head: 5 };
            view.copy(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("bcde".to_owned())
            );
        });
        assert!(cache.borrow().is_empty());
        assert_eq!(requests.lock().unwrap().as_slice(), &[0, 1, 2]);

        view.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 2, head: 2 };
            view.copy(cx);
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_owned())
            );
        });
        view.update(cx, |view, cx| {
            view.select_all(cx);
            view.copy(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("abcdef\n".to_owned())
            );
        });
    }

    #[gpui::test]
    fn paged_copy_rejects_stale_results_and_read_failures(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("ab".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["ab"]), cx));
        view.update(cx, |view, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
            view.copy_generation = 7;
            view.apply_copy_result(6, Ok("old".to_owned()), cx);
            view.apply_copy_result(7, Err(slterm_document::SearchError::ReadFailed), cx);
        });
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_owned())
            );
        });
        view.update(cx, |view, cx| {
            view.apply_copy_result(7, Ok("new".to_owned()), cx);
        });
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("new".to_owned())
            );
        });
    }

    #[gpui::test]
    fn paged_mouse_drag_selects_across_cached_pages(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "ab\n".to_owned(),
                1 => "cd\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache, reader, source_for(&["ab\n", "cd\n"]), cx)
        });
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let metrics = window
            .update(cx, |host, _, cx| *host.view.read(cx).layout.borrow())
            .unwrap()
            .unwrap();
        visual.simulate_mouse_down(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width,
                metrics.bounds.origin.y,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        visual.simulate_mouse_move(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width * 2.0,
                metrics.bounds.origin.y + metrics.line_height,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        visual.simulate_mouse_up(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width * 2.0,
                metrics.bounds.origin.y + metrics.line_height,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        window
            .update(cx, |host, _, cx| {
                assert_eq!(host.view.read(cx).selection.range(), 1..5);
            })
            .unwrap();
    }

    #[gpui::test]
    fn paged_keyboard_navigation_preserves_unicode_columns_and_expands_selection(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
            crate::app_shell::text_view::init(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "中A\nx\n中😀".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache, reader, source_for(&["中A\nx\n中😀"]), cx)
        });
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
        visual.run_until_parked();

        visual.simulate_keystrokes("right");
        visual.simulate_keystrokes("down");
        visual.simulate_keystrokes("down");
        window
            .update(cx, |host, _, cx| {
                let view = host.view.read(cx);
                assert_eq!(view.selection.range(), 10..10);
            })
            .unwrap();

        visual.simulate_keystrokes("shift-home");
        window
            .update(cx, |host, _, cx| {
                let view = host.view.read(cx);
                assert_eq!(view.selection.range(), 7..10);
                assert!(view.selection.anchor > view.selection.head);
            })
            .unwrap();
        visual.simulate_keystrokes("end");
        visual.simulate_keystrokes("left");
        window
            .update(cx, |host, _, cx| {
                assert_eq!(host.view.read(cx).selection.range(), 10..10);
            })
            .unwrap();
    }

    #[gpui::test]
    fn paged_keyboard_navigation_resumes_after_loading_the_target_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
            crate::app_shell::text_view::init(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a\n".to_owned());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "a\n".to_owned(),
                1 => "b\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["a\n", "b\n"]), cx));
        view.update(cx, |view, cx| {
            let mut metrics = super::super::layout::LayoutMetrics {
                bounds: gpui::Bounds::default(),
                cell_width: px(8.0),
                line_height: px(20.0),
            };
            metrics.bounds.size = gpui::size(px(80.0), px(20.0));
            *view.layout.borrow_mut() = Some(metrics);
            view.start_motion(
                PagedMotion::Vertical {
                    down: true,
                    select: true,
                },
                cx,
            );
        });
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_none()
        );

        cx.run_until_parked();

        assert!(requests.lock().unwrap().contains(&1));
        view.update(cx, |view, _| {
            assert_eq!(view.selection.range(), 0..2);
            assert_eq!(view.scroll_line, 1);
        });
    }

    #[gpui::test]
    fn horizontal_boundary_movement_resets_paged_vertical_column(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache.borrow_mut().insert(
            slterm_document::PageKey::new(id, 0, 0),
            "abcdef\nx".to_owned(),
        );
        let reader: PageReader = Arc::new(|_| Ok("abcdef\nx".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["abcdef\nx"]), cx));
        view.update(cx, |view, cx| {
            for motion in [
                PagedMotion::LineBoundary {
                    end: true,
                    select: false,
                },
                PagedMotion::Vertical {
                    down: true,
                    select: false,
                },
                PagedMotion::Horizontal {
                    left: false,
                    select: false,
                },
                PagedMotion::Vertical {
                    down: false,
                    select: false,
                },
            ] {
                view.start_motion(motion, cx);
            }
            assert_eq!(view.selection.range(), 1..1);
            assert_eq!(view.preferred_column, Some(1));
        });
    }

    #[gpui::test]
    fn paged_horizontal_navigation_resumes_after_loading_the_next_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a".to_owned());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "a".to_owned(),
                1 => "b".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["a", "b"]), cx));
        view.update(cx, |view, cx| {
            view.start_motion(
                PagedMotion::Horizontal {
                    left: false,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();

        assert!(requests.lock().unwrap().contains(&1));
        view.update(cx, |view, _| {
            assert_eq!(view.selection.range(), 0..1);
        });

        let reverse_cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        reverse_cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 1), "b".to_owned());
        let reverse_reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "a".to_owned(),
                1 => "b".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let reverse =
            cx.update(|cx| make_view(reverse_cache, reverse_reader, source_for(&["a", "b"]), cx));
        reverse.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 1, head: 1 };
            view.anchor_line = 0;
            view.head_line = 0;
            view.anchor_column = Some(1);
            view.head_column = Some(1);
            view.start_motion(
                PagedMotion::Horizontal {
                    left: true,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();
        reverse.update(cx, |view, _| {
            assert_eq!(view.selection.anchor, 1);
            assert_eq!(view.selection.head, 0);
            assert_eq!(view.selection.range(), 0..1);
        });
    }

    #[gpui::test]
    fn failed_paged_navigation_does_not_resume_after_later_cache_fill(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a\n".to_owned());
        let reader: PageReader = Arc::new(|_| Err(io::Error::other("注入失败")));
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["a\n", "b\n"]), cx));
        view.update(cx, |view, cx| {
            view.start_motion(
                PagedMotion::Vertical {
                    down: true,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert!(view.failed_pages.contains(&1));
            assert!(view.pending_motion.is_none());
            assert_eq!(view.selection.range(), 0..0);
        });

        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 1), "b\n".to_owned());
        view.update(cx, |view, cx| {
            view.apply_page_result(0, 0, [21; 32], Ok("a\n".to_owned()), cx);
            assert_eq!(view.selection.range(), 0..0);
        });
    }

    #[gpui::test]
    fn collapsing_select_all_to_the_end_reveals_the_real_column(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "abcdef".to_owned());
        let reader: PageReader = Arc::new(|_| Ok("abcdef".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["abcdef"]), cx));
        view.update(cx, |view, cx| {
            let mut metrics = super::super::layout::LayoutMetrics {
                bounds: gpui::Bounds::default(),
                cell_width: px(8.0),
                line_height: px(20.0),
            };
            metrics.bounds.size = gpui::size(px(24.0), px(20.0));
            *view.layout.borrow_mut() = Some(metrics);
            view.select_all(cx);
            view.start_motion(
                PagedMotion::Horizontal {
                    left: false,
                    select: false,
                },
                cx,
            );

            assert_eq!(view.selection.range(), 6..6);
            assert_eq!(view.head_column, Some(6));
            assert_eq!(view.scroll_column, 3);
        });
    }

    #[gpui::test]
    fn paged_view_rejects_a_mismatched_source_generation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("unused\n".to_owned()));
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        window
            .update(cx, |host, _, cx| {
                host.view.update(cx, |view, cx| {
                    view.pending_pages.insert(0);
                    view.apply_page_result(0, 1, [21; 32], Ok("stale\n".to_owned()), cx);
                    assert!(view.pending_pages.contains(&0));
                });
            })
            .unwrap();
        assert!(cache.borrow().is_empty());
    }
}
