//! Rope Editor 查找／替换栏：后台匹配、导航、替换与瞬态状态。
//!
//! 视图只持不可变正文快照；匹配与替换计划经 core 的纯请求在后台执行，
//! 回执按文档、revision 与代次对账，迟到结果不得覆盖新正文。

use std::ops::Range;
use std::sync::mpsc::{SyncSender, sync_channel};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, Focusable as _, Hsla,
    InteractiveElement as _, IntoElement as _, ParentElement as _, Pixels, ShapedLine, Styled as _,
    Subscription, Task, Window, div, point, px, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex,
};
use slterm_document::{
    DocumentReplaceRequest, DocumentReplacement, DocumentTextRequest, EditApplyOutcome,
    EditTransaction, EditTransactionError, ReplacePlanError, SearchError, SearchOptions,
    TextStateId,
};
use uuid::Uuid;

use super::super::TEXT_VIEW_SEARCH_CONTEXT;
use super::super::metrics::DisplayMap;
use super::super::source::VisibleLine;
use super::{DocumentEditRequest, DocumentTextView, LayoutMetrics, TextSelection};
use crate::app_shell::config::Settings;
use crate::i18n::Message;

/// 一帧的正文绘制快照；查找命中、行号、折叠与括号高亮都只是瞬态投影。
pub(super) struct PaintFrame {
    pub(super) lines: Vec<(ShapedLine, VisibleLine)>,
    pub(super) selection: Range<usize>,
    pub(super) cursor: usize,
    pub(super) search_matches: Vec<Range<usize>>,
    pub(super) active_match: Option<Range<usize>>,
    pub(super) layout: LayoutMetrics,
    pub(super) wrap: bool,
    pub(super) cursor_after_wrap: bool,
    pub(super) cursor_before_wrap: bool,
    /// 视口行 → 1 起算逻辑行号；软换行只在逻辑行首段显示。
    pub(super) line_numbers: Vec<(usize, usize)>,
    /// 视口行 → 是否已折叠；仅折叠区域起始行生成标记。
    pub(super) fold_markers: Vec<(usize, bool)>,
    /// 光标当前视口行；不可见时为 None。
    pub(super) current_row: Option<usize>,
    /// 命中的一对括号字节位置；只绘制仍可见的字符。
    pub(super) bracket_bytes: Vec<usize>,
}

/// 命中高亮只生成绘制矩形，颜色取主题 warning 语义 token。
pub(super) fn search_highlight_rects(
    frame: &PaintFrame,
    scroll_column: usize,
    map: &DisplayMap,
    cx: &App,
) -> Vec<(Bounds<Pixels>, Hsla)> {
    if frame.search_matches.is_empty() {
        return Vec::new();
    }
    let bounds = frame.layout.bounds;
    let warning = cx.theme().warning;
    let mut rects = Vec::new();
    for (row, (_, line)) in frame.lines.iter().enumerate() {
        let range = line.byte_range();
        let horizontal_start = if frame.wrap {
            line.display_start()
        } else {
            scroll_column
        };
        let first_match = frame
            .search_matches
            .partition_point(|matched| matched.end <= range.start);
        for matched in frame.search_matches[first_match..]
            .iter()
            .take_while(|matched| matched.start < range.end)
        {
            let start = matched.start.max(range.start);
            let end = matched.end.min(range.end);
            if start >= end {
                continue;
            }
            let (Some(x0), Some(x1)) = (
                map.column_at_from(line.text(), start - range.start, line.display_start()),
                map.column_at_from(line.text(), end - range.start, line.display_start()),
            ) else {
                continue;
            };
            let color = if frame.active_match.as_ref() == Some(matched) {
                warning.opacity(0.55)
            } else {
                warning.opacity(0.28)
            };
            rects.push((
                Bounds {
                    origin: point(
                        bounds.origin.x
                            + frame.layout.cell_width * (x0 as f32 - horizontal_start as f32),
                        bounds.origin.y + frame.layout.line_height * row as f32,
                    ),
                    size: size(
                        frame.layout.cell_width * (x1 - x0) as f32,
                        frame.layout.line_height,
                    ),
                },
                color,
            ));
        }
    }
    rects
}

/// 行内错误是封闭原因，不携带底层文本。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorSearchError {
    InvalidPattern,
    SearchFailed,
    ReplaceFailed,
    ReplaceStale,
}

impl EditorSearchError {
    fn message(self) -> Message {
        match self {
            Self::InvalidPattern => Message::EditorFindInvalidRegex,
            Self::SearchFailed => Message::EditorFindFailed,
            Self::ReplaceFailed | Self::ReplaceStale => Message::EditorReplaceFailed,
        }
    }
}

enum SearchReply {
    Matches(Vec<Range<usize>>),
    InvalidPattern,
    Failed,
}

/// 查找栏全部瞬态：查询、选项、匹配、当前项与确认态都不进入 ViewState。
pub(super) struct EditorSearchState {
    find_input: Entity<InputState>,
    replace_input: Entity<InputState>,
    options: SearchOptions,
    matches: Vec<Range<usize>>,
    active: Option<usize>,
    error: Option<EditorSearchError>,
    /// 后台扫描在途；计数显示为省略号并提供显式取消。
    pending: bool,
    confirm_replace_all: bool,
    generation: u64,
    task: Option<Task<()>>,
    cancel: Option<SyncSender<()>>,
    replace_task: Option<Task<()>>,
    replace_cancel: Option<SyncSender<()>>,
    _find_subscription: Subscription,
    _replace_subscription: Subscription,
}

impl EditorSearchState {
    fn active_range(&self) -> Option<Range<usize>> {
        self.active
            .and_then(|active| self.matches.get(active))
            .cloned()
    }
}

impl DocumentTextView {
    /// Ctrl+F：打开或还焦查找栏；再次打开沿用同一瞬态查询。
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(search) = &self.search {
            search
                .find_input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            return;
        }
        let language = cx.global::<Settings>().language;
        let find_placeholder = language.text(Message::EditorFindPlaceholder).to_owned();
        let replace_placeholder = language.text(Message::EditorReplacePlaceholder).to_owned();
        let find_input = cx.new(|cx| InputState::new(window, cx).placeholder(find_placeholder));
        let replace_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(replace_placeholder));
        let find_subscription =
            cx.subscribe_in(&find_input, window, |view, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    view.refresh_search(true, cx);
                }
            });
        let replace_subscription = cx.subscribe_in(
            &replace_input,
            window,
            |view, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(search) = &mut view.search
                {
                    search.confirm_replace_all = false;
                    cx.notify();
                }
            },
        );
        find_input.read(cx).focus_handle(cx).focus(window, cx);
        self.search = Some(EditorSearchState {
            find_input,
            replace_input,
            options: SearchOptions::default(),
            matches: Vec::new(),
            active: None,
            error: None,
            pending: false,
            confirm_replace_all: false,
            generation: 0,
            task: None,
            cancel: None,
            replace_task: None,
            replace_cancel: None,
            _find_subscription: find_subscription,
            _replace_subscription: replace_subscription,
        });
        self.refresh_search(true, cx);
        cx.notify();
    }

    /// 关闭即清除全部瞬态、取消在途扫描并还焦正文。
    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.search.take().is_none() {
            return;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// 查询、选项或 revision 影响匹配时重扫；reveal 只由用户查询／选项触发。
    fn refresh_search(&mut self, reveal: bool, cx: &mut Context<'_, Self>) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        let query = search.find_input.read(cx).value().to_string();
        let options = search.options;
        search.generation = search.generation.wrapping_add(1);
        let generation = search.generation;
        search.matches.clear();
        search.active = None;
        search.error = None;
        search.pending = false;
        search.confirm_replace_all = false;
        search.task = None;
        search.cancel = None;
        search.replace_task = None;
        search.replace_cancel = None;
        if query.is_empty() {
            cx.notify();
            return;
        }

        let revision = self.revision;
        let request = DocumentTextRequest::for_editable_snapshot(
            self.document_id,
            revision,
            self.text.clone(),
            generation,
        );
        let (cancel, cancellation) = sync_channel(1);
        let executor = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    let reply = match request.chunks() {
                        Ok(chunks) => {
                            request.search_with_options(&query, options, chunks, &cancellation)
                        }
                        Err(error) => Err(error),
                    };
                    Some(match reply {
                        Ok(found) => SearchReply::Matches(
                            found
                                .matches()
                                .iter()
                                .map(|matched| matched.range())
                                .collect(),
                        ),
                        Err(SearchError::InvalidPattern) => SearchReply::InvalidPattern,
                        Err(SearchError::Cancelled) => return None,
                        Err(_) => SearchReply::Failed,
                    })
                })
                .await;
            let Some(reply) = result else {
                return;
            };
            let _ = this.update(cx, |view, cx| {
                view.accept_search_reply(generation, revision, reply, reveal, cx);
            });
        });
        if let Some(search) = self.search.as_mut() {
            search.task = Some(task);
            search.cancel = Some(cancel);
            search.pending = true;
        }
        cx.notify();
    }

    /// 正文快照变化后旧结果立即失效；查找栏开着时按新 revision 重扫。
    pub(super) fn refresh_search_after_text_change(&mut self, cx: &mut Context<'_, Self>) {
        if self.search.is_some() {
            self.refresh_search(false, cx);
        }
    }

    /// 只接受仍匹配当前代次与 revision 的结果。
    fn accept_search_reply(
        &mut self,
        generation: u64,
        revision: u64,
        reply: SearchReply,
        reveal: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if revision != self.revision {
            return;
        }
        let active = {
            let Some(search) = self.search.as_mut() else {
                return;
            };
            if search.generation != generation {
                return;
            }
            search.task = None;
            search.cancel = None;
            search.pending = false;
            match reply {
                SearchReply::Matches(matches) => {
                    search.matches = matches
                        .into_iter()
                        .filter(|matched| matched.end <= self.text.len())
                        .collect();
                    search.active = (!search.matches.is_empty()).then_some(0);
                    search.error = None;
                }
                SearchReply::InvalidPattern => {
                    search.matches.clear();
                    search.active = None;
                    search.error = Some(EditorSearchError::InvalidPattern);
                }
                SearchReply::Failed => {
                    search.matches.clear();
                    search.active = None;
                    search.error = Some(EditorSearchError::SearchFailed);
                }
            }
            if reveal { search.active_range() } else { None }
        };
        if let Some(range) = active {
            self.select_search_match(range, cx);
        }
        cx.notify();
    }

    /// Enter／Shift+Enter 与 F3／Shift+F3 共用单一回绕导航。
    pub(super) fn move_search(&mut self, forward: bool, cx: &mut Context<'_, Self>) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        let count = search.matches.len();
        if count == 0 {
            search.active = None;
            return;
        }
        let current = search.active.unwrap_or(if forward { count - 1 } else { 0 });
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        search.active = Some(next);
        search.confirm_replace_all = false;
        let range = search.matches[next].clone();
        self.select_search_match(range, cx);
        cx.notify();
    }

    /// 当前匹配只经现有选区与滚动机制进入视口，不写任何持久状态。
    fn select_search_match(&mut self, range: Range<usize>, cx: &mut Context<'_, Self>) {
        if range.start > range.end || range.end > self.text.len() {
            return;
        }
        self.selection = TextSelection {
            anchor: range.start,
            head: range.end,
        };
        self.caret_affinity = super::super::wrap::CaretAffinity::AfterWrap;
        self.preferred_column = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    /// 取消在途扫描但保留查找栏与查询；迟到回执因代次推进被丢弃。
    pub(super) fn cancel_search(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(search) = &mut self.search {
            search.generation = search.generation.wrapping_add(1);
            search.task = None;
            search.cancel = None;
            search.pending = false;
            search.matches.clear();
            search.active = None;
            search.error = None;
            search.confirm_replace_all = false;
        }
        cx.notify();
    }

    pub(super) fn toggle_search_case(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(search) = &mut self.search {
            search.options.case_sensitive = !search.options.case_sensitive;
        }
        self.refresh_search(true, cx);
    }

    pub(super) fn toggle_search_whole_word(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(search) = &mut self.search {
            search.options.whole_word = !search.options.whole_word;
        }
        self.refresh_search(true, cx);
    }

    pub(super) fn toggle_search_regex(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(search) = &mut self.search {
            search.options.regex = !search.options.regex;
        }
        self.refresh_search(true, cx);
    }

    /// 替换当前匹配；只读、无当前项或非法正则时零副作用。
    pub(super) fn replace_search_current(&mut self, cx: &mut Context<'_, Self>) {
        if self.read_only {
            return;
        }
        let Some(search) = self.search.as_mut() else {
            return;
        };
        if search.error == Some(EditorSearchError::InvalidPattern) {
            return;
        }
        let Some(range) = search.active_range() else {
            return;
        };
        let query = search.find_input.read(cx).value().to_string();
        if query.is_empty() {
            return;
        }
        let replacement = search.replace_input.read(cx).value().to_string();
        let options = search.options;
        search.confirm_replace_all = false;
        search.replace_task = None;
        search.replace_cancel = None;
        let revision = self.revision;
        let request = DocumentReplaceRequest::for_editable_match(
            self.document_id,
            revision,
            self.text.clone(),
            query,
            replacement,
            options,
            range,
        );
        let (cancel, cancellation) = sync_channel(1);
        let executor = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { request.plan_cancellable(&cancellation) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.accept_replace_reply(revision, result, cx);
            });
        });
        if let Some(search) = self.search.as_mut() {
            search.replace_task = Some(task);
            search.replace_cancel = Some(cancel);
        }
        cx.notify();
    }

    /// 全部替换需要显式二次确认；确认后整次替换是单个撤销组。
    pub(super) fn replace_search_all(&mut self, cx: &mut Context<'_, Self>) {
        if self.read_only {
            return;
        }
        let Some(search) = self.search.as_mut() else {
            return;
        };
        if search.error == Some(EditorSearchError::InvalidPattern) || search.matches.is_empty() {
            return;
        }
        if !search.confirm_replace_all {
            search.confirm_replace_all = true;
            cx.notify();
            return;
        }
        search.confirm_replace_all = false;
        let query = search.find_input.read(cx).value().to_string();
        if query.is_empty() {
            return;
        }
        let replacement = search.replace_input.read(cx).value().to_string();
        let options = search.options;
        search.replace_task = None;
        search.replace_cancel = None;
        let revision = self.revision;
        let request = DocumentReplaceRequest::for_editable_snapshot(
            self.document_id,
            revision,
            self.text.clone(),
            query,
            replacement,
            options,
        );
        let (cancel, cancellation) = sync_channel(1);
        let executor = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { request.plan_cancellable(&cancellation) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.accept_replace_reply(revision, result, cx);
            });
        });
        if let Some(search) = self.search.as_mut() {
            search.replace_task = Some(task);
            search.replace_cancel = Some(cancel);
        }
        cx.notify();
    }

    fn accept_replace_reply(
        &mut self,
        revision: u64,
        result: Result<DocumentReplacement, ReplacePlanError>,
        cx: &mut Context<'_, Self>,
    ) {
        if revision != self.revision || self.read_only {
            if let Some(search) = &mut self.search {
                search.replace_task = None;
                search.replace_cancel = None;
                search.error = Some(EditorSearchError::ReplaceStale);
            }
            cx.notify();
            return;
        }
        let Some(replacement) = (match result {
            Ok(replacement) => {
                if let Some(search) = &mut self.search {
                    search.replace_task = None;
                    search.replace_cancel = None;
                }
                Some(replacement)
            }
            Err(error) => {
                if let Some(search) = &mut self.search {
                    search.replace_task = None;
                    search.replace_cancel = None;
                    search.error = Some(match error {
                        ReplacePlanError::InvalidRegex => EditorSearchError::InvalidPattern,
                        ReplacePlanError::StaleMatch => EditorSearchError::ReplaceStale,
                        ReplacePlanError::Cancelled => return,
                        ReplacePlanError::ReadOnly | ReplacePlanError::DocumentNotFound(_) => {
                            EditorSearchError::ReplaceFailed
                        }
                    });
                }
                None
            }
        }) else {
            cx.notify();
            return;
        };
        let transaction = replacement.into_transaction();
        let caret = transaction
            .edits
            .first()
            .map(|edit| edit.utf8_range().start + edit.replacement().len());
        if let Err(error) = self.commit_replacement(transaction, caret, cx) {
            if let Some(search) = &mut self.search {
                search.error = Some(match error {
                    EditTransactionError::RevisionMismatch
                    | EditTransactionError::OriginalTextMismatch => EditorSearchError::ReplaceStale,
                    _ => EditorSearchError::ReplaceFailed,
                });
            }
            cx.notify();
        }
    }

    /// 替换计划在后台产生；提交仍经唯一编辑事务入口并采纳权威快照。
    fn commit_replacement(
        &mut self,
        transaction: EditTransaction,
        caret: Option<usize>,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), EditTransactionError> {
        let before = self.text.clone();
        let syntax_transaction = transaction.clone();
        let state = TextStateId::from_uuid(Uuid::now_v7()).expect("系统 UUIDv7 必须有效");
        let origin = cx.entity().entity_id();
        let response = (self.edit_sink)(DocumentEditRequest::Edit(transaction, state), origin, cx)?;
        if response.outcome != EditApplyOutcome::Unchanged {
            if let Some(syntax) = &self.syntax {
                syntax.borrow_mut().edit(
                    &before,
                    response.snapshot.text.clone(),
                    response.snapshot.revision,
                    &syntax_transaction,
                );
            }
            self.adopt_snapshot(response.snapshot, cx);
            self.selection = caret.map_or(self.selection, |caret| TextSelection {
                anchor: caret,
                head: caret,
            });
            self.marked_range = None;
            self.reconcile_transient_ranges();
            self.preferred_column = None;
            self.ensure_cursor_visible();
            self.schedule_syntax_poll(cx);
        }
        cx.notify();
        Ok(())
    }

    /// 绘制阶段只读取当前匹配投影，不重新扫描正文。
    pub(super) fn search_paint_ranges(&self) -> (Vec<Range<usize>>, Option<Range<usize>>) {
        match &self.search {
            Some(search) => (
                search.matches.clone(),
                search
                    .active
                    .and_then(|active| search.matches.get(active))
                    .cloned(),
            ),
            None => (Vec::new(), None),
        }
    }

    /// 查找栏 UI：稳定 debug selector 供页面级测试定位。
    pub(super) fn search_bar(&self, cx: &mut Context<'_, Self>) -> Option<AnyElement> {
        let search = self.search.as_ref()?;
        let language = cx.global::<Settings>().language;
        let count = search.matches.len();
        let position = search.active.map_or(0, |active| active + 1);
        let counter = div()
            .id("editor-find-count")
            .debug_selector(|| "editor-find-count".to_owned())
            .min_w(px(48.0))
            .text_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(if search.pending {
                "…".to_owned()
            } else {
                format!("{position}/{count}")
            });

        let find_row = h_flex()
            .gap_1()
            .items_center()
            .child(
                div()
                    .id("editor-find-input")
                    .debug_selector(|| "editor-find-input".to_owned())
                    .w(px(200.0))
                    .child(Input::new(&search.find_input)),
            )
            .child(counter)
            .child(self.search_option_button(
                "editor-find-case",
                "Aa",
                Message::EditorFindCase,
                search.options.case_sensitive,
                cx,
                |view, cx| view.toggle_search_case(cx),
            ))
            .child(self.search_option_button(
                "editor-find-word",
                "ab",
                Message::EditorFindWord,
                search.options.whole_word,
                cx,
                |view, cx| view.toggle_search_whole_word(cx),
            ))
            .child(self.search_option_button(
                "editor-find-regex",
                ".*",
                Message::EditorFindRegex,
                search.options.regex,
                cx,
                |view, cx| view.toggle_search_regex(cx),
            ))
            .child(self.search_nav_button(
                "editor-find-previous",
                "↑",
                Message::EditorFindPrevious,
                cx,
                |view, _window, cx| view.move_search(false, cx),
            ))
            .child(self.search_nav_button(
                "editor-find-next",
                "↓",
                Message::EditorFindNext,
                cx,
                |view, _window, cx| view.move_search(true, cx),
            ))
            .children(search.pending.then(|| {
                self.search_nav_button(
                    "editor-find-cancel",
                    "■",
                    Message::EditorFindCancel,
                    cx,
                    |view, _window, cx| view.cancel_search(cx),
                )
            }))
            .child(self.search_nav_button(
                "editor-find-close",
                "×",
                Message::EditorFindClose,
                cx,
                |view, window, cx| view.close_search(window, cx),
            ));

        let mut bar = v_flex()
            .id("editor-find-bar")
            .debug_selector(|| "editor-find-bar".to_owned())
            .key_context(TEXT_VIEW_SEARCH_CONTEXT)
            .flex_shrink_0()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(find_row);
        if let Some(error) = search.error {
            bar = bar.child(
                div()
                    .id("editor-find-error")
                    .debug_selector(|| "editor-find-error".to_owned())
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(language.text(error.message())),
            );
        }
        if !self.read_only {
            let replace_one = search.active_range().is_some() && search.error.is_none();
            let confirm_label = if search.confirm_replace_all {
                Message::EditorReplaceConfirm
            } else {
                Message::EditorReplaceAll
            };
            bar = bar.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .id("editor-replace-input")
                            .debug_selector(|| "editor-replace-input".to_owned())
                            .w(px(160.0))
                            .child(Input::new(&search.replace_input)),
                    )
                    .child(
                        Button::new("editor-replace-one")
                            .debug_selector(|| "editor-replace-one".to_owned())
                            .label(language.text(Message::EditorReplaceOne))
                            .ghost()
                            .small()
                            .disabled(!replace_one)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.replace_search_current(cx);
                            })),
                    )
                    .child(
                        Button::new("editor-replace-all")
                            .debug_selector(|| "editor-replace-all".to_owned())
                            .label(language.text(confirm_label))
                            .ghost()
                            .small()
                            .selected(search.confirm_replace_all)
                            .disabled(search.matches.is_empty())
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.replace_search_all(cx);
                            })),
                    ),
            );
        }
        Some(bar.into_any_element())
    }

    fn search_option_button(
        &self,
        id: &'static str,
        label: &'static str,
        tooltip: Message,
        selected: bool,
        cx: &mut Context<'_, Self>,
        on_click: impl Fn(&mut Self, &mut Context<'_, Self>) + 'static,
    ) -> Button {
        let tooltip = cx.global::<Settings>().language.text(tooltip);
        Button::new(id)
            .debug_selector(move || id.to_owned())
            .label(label)
            .tooltip(tooltip)
            .selected(selected)
            .ghost()
            .small()
            .on_click(cx.listener(move |view, _, _, cx| on_click(view, cx)))
    }

    fn search_nav_button(
        &self,
        id: &'static str,
        label: &'static str,
        tooltip: Message,
        cx: &mut Context<'_, Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<'_, Self>) + 'static,
    ) -> Button {
        let tooltip = cx.global::<Settings>().language.text(tooltip);
        Button::new(id)
            .debug_selector(move || id.to_owned())
            .label(label)
            .tooltip(tooltip)
            .ghost()
            .small()
            .on_click(cx.listener(move |view, _, window, cx| on_click(view, window, cx)))
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl DocumentTextView {
    pub(super) fn search_matches_for_test(&self) -> Option<Vec<Range<usize>>> {
        self.search.as_ref().map(|search| search.matches.clone())
    }

    pub(super) fn search_active_for_test(&self) -> Option<usize> {
        self.search.as_ref().and_then(|search| search.active)
    }

    pub(super) fn search_error_for_test(&self) -> Option<bool> {
        self.search.as_ref().map(|search| search.error.is_some())
    }

    pub(super) fn search_confirm_for_test(&self) -> bool {
        self.search
            .as_ref()
            .is_some_and(|search| search.confirm_replace_all)
    }

    pub(super) fn search_generation_for_test(&self) -> Option<u64> {
        self.search.as_ref().map(|search| search.generation)
    }

    pub(super) fn search_pending_for_test(&self) -> bool {
        self.search.as_ref().is_some_and(|search| search.pending)
    }

    /// 后台扫描太快时用于稳定验证在途 UI 与取消语义。
    pub(super) fn mark_search_pending_for_test(&mut self) {
        if let Some(search) = &mut self.search {
            search.pending = true;
            search.matches = std::iter::once(0..1).collect();
            search.active = Some(0);
        }
    }

    /// 注入旧代次回执；返回是否被采纳，用于证明迟到结果不得覆盖新 revision。
    pub(super) fn apply_stale_search_reply_for_test(
        &mut self,
        generation: u64,
        revision: u64,
        matches: Vec<Range<usize>>,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(search) = &self.search else {
            return false;
        };
        if search.generation != generation || revision != self.revision {
            return false;
        }
        self.accept_search_reply(
            generation,
            revision,
            SearchReply::Matches(matches),
            false,
            cx,
        );
        true
    }
}
