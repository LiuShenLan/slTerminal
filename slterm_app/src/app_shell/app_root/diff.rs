//! Diff 面板：左侧固定 HEAD，右侧直接编辑共享 Document。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use std::ops::Range;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, px,
};
use gpui_component::{ActiveTheme as _, button::Button};
use slterm_document::{
    DiffPair, DiffPairKind, DiffSnapshot, DocumentId, DocumentPageCache, compute_diff_cancellable,
};

use super::git_show::GitShowView;
use super::panel_host::{EditorContent, PanelContent};
use crate::app_shell::config::Settings;
use crate::app_shell::text_view::{
    DiffCollapsedSide, DiffViewMode, DiffViewState, DocumentTextView, EditorViewStateLease,
    EditorViewStateStore, PagedTextView,
};
use crate::git::{GitError, GitShowSnapshot};
use crate::i18n::Message;

/// Diff 打开失败或 HEAD 读取失败时由复合视图请求同固定版本重试。
pub(crate) enum DiffViewEvent {
    RetryHead,
}

impl EventEmitter<DiffViewEvent> for DiffView {}

/// 工作区侧正文：可编辑 Rope 或超限分页只读。
pub(super) enum DiffRightContent {
    Editor(EditorContent),
    Paged {
        view: Entity<PagedTextView>,
        view_states: EditorViewStateStore,
        view_key: slterm_workspace::ViewKey,
    },
}

/// Diff 行内模式的单行投影；不插入真实正文行。
struct InlineRow {
    head: Option<(usize, String)>,
    worktree: Option<(usize, String)>,
    kind: DiffPairKind,
    /// Some = 折叠占位行，值为被隐藏的等值行数。
    collapsed_count: Option<usize>,
}

const INLINE_UNCHANGED_CONTEXT: usize = 2;
const COLLAPSE_UNCHANGED_CONTEXT: usize = 2;
const COLLAPSE_UNCHANGED_MIN_RUN: usize = 6;
const COLUMN_WIDTH_STEP: u16 = 40;

pub(crate) struct DiffOpenRequest {
    pub(crate) project_id: slterm_workspace::ProjectId,
    pub(crate) target_group: Option<slterm_workspace::PanelGroupId>,
    pub(crate) document_id: DocumentId,
    pub(crate) binding: slterm_document::RepositoryBinding,
    pub(crate) path: slterm_document::RepoPath,
    pub(crate) focus: bool,
    pub(crate) target_line: Option<usize>,
}

pub(super) type DiffHeadResult =
    Result<(GitShowSnapshot, Option<slterm_document::DiffBinding>), GitError>;

/// 单视图差分调度：连续编辑合并，任何时刻最多一个在途计算。
#[derive(Default)]
struct DiffRefreshState {
    in_flight: bool,
    dirty: bool,
    debounce_pending: bool,
}

impl DiffRefreshState {
    /// 共享编辑到达；返回是否应取消在途计算以释放预算。
    fn on_edit(&mut self) -> bool {
        self.dirty = true;
        self.debounce_pending = true;
        self.in_flight
    }

    /// 合并窗口到点；若有在途计算则只保留脏位，等其结束。
    fn on_debounce(&mut self) -> bool {
        self.debounce_pending = false;
        self.take_start()
    }

    /// 在途计算结束；合并窗口仍待计时则不提前启动。
    fn on_finished(&mut self) -> bool {
        self.in_flight = false;
        if self.debounce_pending {
            return false;
        }
        self.take_start()
    }

    /// 立即路径（HEAD 刷新、可见转换）：取消合并窗口，等待在途计算让位。
    fn force(&mut self) -> bool {
        self.dirty = true;
        self.debounce_pending = false;
        self.take_start()
    }

    fn take_start(&mut self) -> bool {
        if self.in_flight || !self.dirty {
            return false;
        }
        self.dirty = false;
        self.in_flight = true;
        true
    }

    fn reset(&mut self) {
        self.in_flight = false;
        self.dirty = false;
        self.debounce_pending = false;
    }
}

/// 工作区行标：与导航位置同源，但保留增／改／删语义。
pub(super) fn diff_change_marks(
    diff: &DiffSnapshot,
) -> Vec<(usize, crate::app_shell::text_view::GitMarkKind)> {
    use crate::app_shell::text_view::GitMarkKind;
    let mut marks = Vec::new();
    let mut in_change = false;
    let mut last_worktree = 0;
    for pair in diff.pairs().unwrap_or(&[]) {
        if let Some(line) = &pair.worktree {
            last_worktree = line.index;
        }
        if pair.kind == DiffPairKind::Equal {
            in_change = false;
        } else if !in_change {
            let kind = match pair.kind {
                DiffPairKind::Insert => GitMarkKind::Added,
                DiffPairKind::Delete => GitMarkKind::Deleted,
                DiffPairKind::Replace => GitMarkKind::Modified,
                DiffPairKind::Equal => unreachable!("相等配对已在上方处理"),
            };
            marks.push((last_worktree, kind));
            in_change = true;
        }
    }
    marks
}

pub(super) fn diff_change_locations(diff: &DiffSnapshot) -> Vec<(usize, usize)> {
    let mut locations = Vec::new();
    let mut in_change = false;
    let mut last_head = 0;
    let mut last_worktree = 0;
    for pair in diff.pairs().unwrap_or(&[]) {
        if let Some(line) = &pair.head {
            last_head = line.index;
        }
        if let Some(line) = &pair.worktree {
            last_worktree = line.index;
        }
        if pair.kind == DiffPairKind::Equal {
            in_change = false;
        } else if !in_change {
            locations.push((last_head, last_worktree));
            in_change = true;
        }
    }
    locations
}

pub(crate) struct DiffView {
    root: gpui::WeakEntity<super::AppRoot>,
    document_id: DocumentId,
    focus: FocusHandle,
    left: Entity<GitShowView>,
    right: Option<Entity<DocumentTextView>>,
    right_paged: Option<Entity<PagedTextView>>,
    _right_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    head: Option<GitShowSnapshot>,
    head_loading: bool,
    head_error: bool,
    view_states: EditorViewStateStore,
    view_lease: EditorViewStateLease,
    diff: Option<DiffSnapshot>,
    diff_error: bool,
    paged_diff_unavailable: bool,
    diff_generation: u64,
    diff_task: Option<Task<()>>,
    diff_debounce: Option<Task<()>>,
    diff_cancel: Option<Arc<AtomicBool>>,
    refresh: DiffRefreshState,
    active_change: Option<usize>,
    pending_line: Option<usize>,
}

impl DiffView {
    pub(super) fn new(
        root: gpui::WeakEntity<super::AppRoot>,
        document_id: DocumentId,
        right: DiffRightContent,
        cache: Rc<RefCell<DocumentPageCache>>,
        pending_line: Option<usize>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        let (right, right_paged, right_subscription, view_states, view_lease) = match right {
            DiffRightContent::Editor(content) => {
                let (right, subscription) = content.into_parts();
                let (view_states, view_lease) = right
                    .read(cx)
                    .view_state_access()
                    .expect("Diff 工作区侧必须持有专属 ViewState 租约");
                right.update(cx, |view, _| view.share_focus(focus.clone()));
                (
                    Some(right),
                    None,
                    Some(subscription),
                    view_states,
                    view_lease,
                )
            }
            DiffRightContent::Paged {
                view,
                view_states,
                view_key,
            } => {
                view.update(cx, |view, _| view.share_focus(focus.clone()));
                let lease = view_states.lease(&view_key);
                (None, Some(view), None, view_states, lease)
            }
        };
        let left = cx.new(|cx| GitShowView::loading(cache, cx));
        left.update(cx, |view, cx| view.share_focus(focus.clone(), cx));
        let paged_diff_unavailable = right_paged.is_some();
        let mut subscriptions = Vec::new();
        if let Some(right) = &right {
            subscriptions.push(cx.subscribe(right, |view, _, event, cx| {
                if let crate::app_shell::text_view::DocumentTextEvent::Scrolled(line) = event {
                    view.sync_from_worktree(*line, cx);
                }
            }));
        }
        subscriptions.push(cx.subscribe(&left, |view, _, event, cx| {
            if let super::git_show::GitShowEvent::Scrolled(line) = event {
                view.sync_from_head(*line, cx);
            }
        }));
        Self {
            root,
            document_id,
            focus,
            left,
            right,
            right_paged,
            _right_subscription: right_subscription,
            _subscriptions: subscriptions,
            head: None,
            head_loading: false,
            head_error: false,
            view_states,
            view_lease,
            diff: None,
            diff_error: false,
            paged_diff_unavailable,
            diff_generation: 0,
            diff_task: None,
            diff_debounce: None,
            diff_cancel: None,
            refresh: DiffRefreshState::default(),
            active_change: None,
            pending_line,
        }
    }

    fn sync_from_worktree(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        if !self.view_state().sync_scroll() || self.view_state().mode() != DiffViewMode::SideBySide
        {
            return;
        }
        self.left
            .update(cx, |view, cx| view.set_scroll_line_from_peer(line, cx));
    }

    fn sync_from_head(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        if !self.view_state().sync_scroll() || self.view_state().mode() != DiffViewMode::SideBySide
        {
            return;
        }
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| view.set_scroll_line_from_peer(line, cx));
        }
    }

    /// 只同步工作区来源视图的只读与编码元数据。
    pub(crate) fn apply_document_metadata(
        &mut self,
        session: &slterm_document::DocumentSession,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| view.apply_document_metadata(session, cx));
        }
        cx.notify();
    }

    fn apply_head(&mut self, result: DiffHeadResult, cx: &mut Context<'_, Self>) {
        self.head_loading = false;
        let generation = self.left.read(cx).current_generation();
        match result {
            Ok((snapshot, refreshed)) => {
                if let Some(binding) = refreshed {
                    let state = self.view_state().with_binding(binding);
                    self.view_states.checkpoint_diff(&self.view_lease, state);
                }
                self.head = Some(snapshot.clone());
                self.head_error = false;
                self.left.update(cx, |view, cx| {
                    view.apply_result(generation, Ok(snapshot), cx);
                    view.share_focus(self.focus.clone(), cx);
                });
            }
            Err(error) => {
                self.head_error = true;
                // 刷新失败只替换展示错误；保留最后一次已验证的固定快照供恢复与差分使用。
                self.left
                    .update(cx, |view, cx| view.apply_result(generation, Err(error), cx));
            }
        }
        if self.head.is_some() {
            self.schedule_diff_now(cx);
        } else {
            self.apply_pending_line(cx);
        }
        cx.notify();
    }

    pub(crate) fn apply_shared_edit(
        &mut self,
        snapshot: &slterm_document::DocumentSession,
        transaction: &slterm_document::EditTransaction,
        origin: gpui::EntityId,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(right) = &self.right
            && right.entity_id() != origin
        {
            right.update(cx, |view, cx| {
                view.apply_edit_snapshot(snapshot, transaction, cx)
            });
        }
        self.schedule_shared_refresh(cx);
        cx.notify();
    }

    pub(crate) fn apply_shared_snapshot(
        &mut self,
        session: &slterm_document::DocumentSession,
        origin: gpui::EntityId,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(right) = &self.right
            && right.entity_id() != origin
        {
            right.update(cx, |view, cx| view.apply_snapshot(session, cx));
        }
        self.schedule_shared_refresh(cx);
        cx.notify();
    }

    /// 隐藏时取消在途差分与合并窗口；可见后由 AppRoot 用最新快照重新调度。
    pub(crate) fn cancel_pending(&mut self) {
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.cancel_in_flight();
        self.diff_task = None;
        self.diff_debounce = None;
        self.refresh.reset();
    }

    /// 可见转换：先同步右侧正文，再立即按当前设置重新调度差分。
    pub(crate) fn refresh_shared(
        &mut self,
        session: &slterm_document::DocumentSession,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| view.apply_snapshot(session, cx));
        }
        self.schedule_diff_now(cx);
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn diff_task_pending_for_test(&self) -> bool {
        self.diff_task.is_some()
    }

    /// 共享编辑：按设置间隔合并连续输入，只保留最新一次计算。
    fn schedule_shared_refresh(&mut self, cx: &mut Context<'_, Self>) {
        if self.refresh.on_edit() {
            self.cancel_in_flight();
        }
        self.diff_debounce = None;
        let interval = cx.global::<Settings>().diff_refresh_interval_ms;
        let target = cx.entity().downgrade();
        self.diff_debounce = Some(cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(interval))
                .await;
            let _ = target.update(cx, |view, cx| {
                view.diff_debounce = None;
                if view.refresh.on_debounce() {
                    view.start_compute(cx);
                }
            });
        }));
    }

    /// HEAD 刷新、保存与可见转换的立即路径；在途旧计算先被取消。
    fn schedule_diff_now(&mut self, cx: &mut Context<'_, Self>) {
        self.diff_debounce = None;
        if self.refresh.in_flight {
            self.cancel_in_flight();
        }
        if self.refresh.force() {
            self.start_compute(cx);
        }
    }

    fn cancel_in_flight(&mut self) {
        if let Some(cancel) = &self.diff_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    fn start_compute(&mut self, cx: &mut Context<'_, Self>) {
        let Some(head) = self.head.clone() else {
            let _ = self.refresh.on_finished();
            return;
        };
        let Some(right) = &self.right else {
            // 工作区侧为分页只读：仍显示正文与 HEAD，但差异不可计算。
            self.paged_diff_unavailable = true;
            self.diff = None;
            self.apply_collapsed_unchanged(cx);
            cx.notify();
            return;
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        // 前台只复制 Rope 句柄；全文转换与差分都在后台执行。
        let snapshot = right.read(cx).text_snapshot();
        let head_bytes = head
            .bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .map_or_else(|| head.bytes.clone(), Arc::from);
        let whitespace = self.view_state().whitespace();
        let commit = head.commit.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.diff_cancel = Some(cancel.clone());
        let compute = cx.background_executor().spawn(async move {
            let input = slterm_document::DiffInput::git(
                head_bytes,
                Arc::from(snapshot.text.to_string().into_bytes()),
                whitespace,
                snapshot.text_state,
                commit,
                generation,
            );
            compute_diff_cancellable(&input, &cancel)
        });
        let target = cx.entity().downgrade();
        self.diff_task = Some(cx.spawn(async move |_, cx| {
            let result = compute.await;
            let _ = target.update(cx, |view, cx| {
                // 隐藏/立即换代后旧任务的完成不得结算新任务或清掉单飞行位。
                if view.diff_generation != generation {
                    return;
                }
                view.diff_task = None;
                view.diff_cancel = None;
                view.apply_diff(generation, result, cx);
                if view.refresh.on_finished() {
                    view.start_compute(cx);
                }
            });
        }));
    }

    fn apply_diff(
        &mut self,
        generation: u64,
        result: Result<DiffSnapshot, slterm_document::DiffError>,
        cx: &mut Context<'_, Self>,
    ) {
        if generation != self.diff_generation {
            return;
        }
        if matches!(result, Err(slterm_document::DiffError::Cancelled)) {
            // 新一代输入已排队；保留上一次已验证结果，避免闪烁为空。
            return;
        }
        match result {
            Ok(diff) => {
                self.diff = Some(diff);
                self.diff_error = false;
                self.paged_diff_unavailable = false;
            }
            Err(_) => {
                // 保留最后一次可用差异与正文，只显示可重试的过期说明。
                self.diff_error = true;
            }
        }
        self.active_change = None;
        self.apply_collapsed_unchanged(cx);
        self.apply_pending_line(cx);
        cx.notify();
    }

    fn request_line(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        self.pending_line = Some(line);
        if !self.head_loading && self.diff.is_some() {
            self.apply_pending_line(cx);
        }
        cx.notify();
    }

    fn apply_pending_line(&mut self, cx: &mut Context<'_, Self>) {
        let Some(line) = self.pending_line.take() else {
            return;
        };
        let mut change = None;
        let mut change_index = 0;
        let mut in_change = false;
        let mut seen_change = false;
        for pair in self
            .diff
            .as_ref()
            .and_then(|diff| diff.pairs())
            .unwrap_or(&[])
        {
            if pair.kind == DiffPairKind::Equal {
                in_change = false;
                continue;
            }
            if !in_change {
                if seen_change {
                    change_index += 1;
                }
                seen_change = true;
                in_change = true;
            }
            if pair
                .worktree
                .as_ref()
                .is_some_and(|worktree| worktree.index == line)
            {
                change = Some(change_index);
                break;
            }
        }
        if let Some(index) = change {
            self.reveal_change(index, cx);
        } else {
            self.reveal_worktree_line(line, cx);
        }
    }

    fn change_locations(&self) -> Vec<(usize, usize)> {
        self.diff
            .as_ref()
            .map_or_else(Vec::new, diff_change_locations)
    }

    fn reveal_worktree_line(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| view.reveal_line(line, cx));
        }
    }

    fn reveal_change(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        let locations = self.change_locations();
        let Some((head, worktree)) = locations.get(index).copied() else {
            return;
        };
        self.active_change = Some(index);
        self.left.update(cx, |view, cx| view.reveal_line(head, cx));
        self.reveal_worktree_line(worktree, cx);
        cx.notify();
    }

    pub(crate) fn next_change(&mut self, cx: &mut Context<'_, Self>) {
        let count = self.change_locations().len();
        if count == 0 {
            return;
        }
        let index = self.active_change.map_or(0, |active| (active + 1) % count);
        self.reveal_change(index, cx);
    }

    pub(crate) fn previous_change(&mut self, cx: &mut Context<'_, Self>) {
        let count = self.change_locations().len();
        if count == 0 {
            return;
        }
        let index = self.active_change.map_or(count - 1, |active| {
            active.checked_sub(1).unwrap_or(count - 1)
        });
        self.reveal_change(index, cx);
    }

    fn view_state(&self) -> DiffViewState {
        self.view_states
            .diff_state(self.view_lease.key())
            .expect("Diff 固定绑定与展示参数必须随面板存活")
    }

    pub(crate) fn right_view(&self) -> &Entity<DocumentTextView> {
        self.right
            .as_ref()
            .expect("right_view 只用于可编辑工作区侧测试")
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn change_state_for_test(&self) -> (usize, Option<usize>) {
        (self.change_locations().len(), self.active_change)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_view_state_for_test(
        &mut self,
        state: DiffViewState,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.view_states
            .checkpoint_diff(&self.view_lease, state.clone());
        let source = state.source();
        self.active_change = None;
        self.pending_line = None;
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| {
                view.set_wrap(source.wrap(), cx);
                gpui::EntityInputHandler::set_selected_text_range(view, 0..1, window, cx);
                view.set_scroll_for_test(3, 2);
            });
        }
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn view_state_for_test(&self) -> DiffViewState {
        self.view_state()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn transients_for_test(&self, cx: &App) -> (usize, std::ops::Range<usize>) {
        let Some(right) = &self.right else {
            return (0, 0..0);
        };
        let state = right.read(cx).scroll_state_for_test();
        (state.0, state.3)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn shared_identity_for_test(&self) -> DocumentId {
        self.document_id
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn shared_text_for_test(&self, cx: &App) -> String {
        self.right
            .as_ref()
            .expect("shared_text_for_test 只用于可编辑工作区侧测试")
            .read(cx)
            .input_text_for_test()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn head_bytes_option_for_test(&self) -> Option<std::sync::Arc<[u8]>> {
        self.head.as_ref().map(|head| head.bytes.clone())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn head_bytes_for_test(&self) -> std::sync::Arc<[u8]> {
        self.head.as_ref().unwrap().bytes.clone()
    }
}

impl DiffView {
    /// 展示参数的真实编辑入口；持久字段与渲染必须在同一调用内闭合。
    fn update_state(
        &mut self,
        update: impl FnOnce(DiffViewState) -> DiffViewState,
        cx: &mut Context<'_, Self>,
    ) {
        let before = self.view_state();
        let after = update(before.clone());
        if before == after {
            return;
        }
        self.view_states
            .checkpoint_diff(&self.view_lease, after.clone());
        if before.whitespace() != after.whitespace() {
            self.schedule_diff_now(cx);
        }
        if before.mode() != after.mode()
            || before.collapse_unchanged() != after.collapse_unchanged()
        {
            self.apply_collapsed_unchanged(cx);
        }
        cx.notify();
    }

    fn toggle_mode(&mut self, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let next = match state.mode() {
                    DiffViewMode::SideBySide => DiffViewMode::Inline,
                    DiffViewMode::Inline => DiffViewMode::SideBySide,
                };
                state.with_mode(next)
            },
            cx,
        );
    }

    fn toggle_swap(&mut self, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let swapped = state.swapped();
                state.with_swapped(!swapped)
            },
            cx,
        );
    }

    fn toggle_collapsed(&mut self, side: DiffCollapsedSide, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                if state.collapsed() == Some(side) {
                    state.with_collapsed(None)
                } else {
                    state.with_collapsed(Some(side))
                }
            },
            cx,
        );
    }

    fn adjust_width(&mut self, wider: bool, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let width = if wider {
                    state.column_width().saturating_add(COLUMN_WIDTH_STEP)
                } else {
                    state.column_width().saturating_sub(COLUMN_WIDTH_STEP)
                }
                .clamp(160, 2_000);
                state.with_column_width(width)
            },
            cx,
        );
    }

    fn toggle_sync_scroll(&mut self, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let enabled = state.sync_scroll();
                state.with_sync_scroll(!enabled)
            },
            cx,
        );
    }

    fn toggle_collapse_unchanged(&mut self, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let collapsed = state.collapse_unchanged();
                state.with_collapse_unchanged(!collapsed)
            },
            cx,
        );
    }

    fn cycle_whitespace(&mut self, cx: &mut Context<'_, Self>) {
        self.update_state(
            |state| {
                let next = match state.whitespace() {
                    slterm_document::DiffWhitespaceMode::Exact => {
                        slterm_document::DiffWhitespaceMode::Trailing
                    }
                    slterm_document::DiffWhitespaceMode::Trailing => {
                        slterm_document::DiffWhitespaceMode::All
                    }
                    slterm_document::DiffWhitespaceMode::All => {
                        slterm_document::DiffWhitespaceMode::Exact
                    }
                };
                state.with_whitespace(next)
            },
            cx,
        );
    }

    /// 把未变更折叠区间应用到两侧真实文本视图；行内模式由行投影自行折叠。
    fn apply_collapsed_unchanged(&mut self, cx: &mut Context<'_, Self>) {
        let state = self.view_state();
        let enabled = state.collapse_unchanged() && state.mode() == DiffViewMode::SideBySide;
        let (head_ranges, worktree_ranges) = if enabled {
            self.collapsed_ranges()
        } else {
            (Vec::new(), Vec::new())
        };
        self.left.update(cx, |view, cx| {
            view.set_diff_collapsed_ranges(head_ranges, cx)
        });
        if let Some(right) = &self.right {
            right.update(cx, |view, cx| {
                view.set_diff_collapsed_ranges(worktree_ranges, cx)
            });
        }
    }

    fn collapsed_ranges(&self) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
        let Some(pairs) = self.diff.as_ref().and_then(|diff| diff.pairs()) else {
            return (Vec::new(), Vec::new());
        };
        (
            unchanged_collapse_ranges(pairs, true),
            unchanged_collapse_ranges(pairs, false),
        )
    }

    fn inline_rows(&self) -> Vec<InlineRow> {
        let Some(pairs) = self.diff.as_ref().and_then(|diff| diff.pairs()) else {
            return Vec::new();
        };
        let collapse = self.view_state().collapse_unchanged();
        let mut rows = Vec::new();
        let mut index = 0;
        while index < pairs.len() {
            if pairs[index].kind != DiffPairKind::Equal {
                rows.push(inline_row(&pairs[index]));
                index += 1;
                continue;
            }
            let start = index;
            while index < pairs.len() && pairs[index].kind == DiffPairKind::Equal {
                index += 1;
            }
            let run = &pairs[start..index];
            if collapse && run.len() > INLINE_UNCHANGED_CONTEXT * 2 {
                for pair in &run[..INLINE_UNCHANGED_CONTEXT] {
                    rows.push(inline_row(pair));
                }
                rows.push(InlineRow {
                    head: None,
                    worktree: None,
                    kind: DiffPairKind::Equal,
                    collapsed_count: Some(run.len() - INLINE_UNCHANGED_CONTEXT * 2),
                });
                for pair in &run[run.len() - INLINE_UNCHANGED_CONTEXT..] {
                    rows.push(inline_row(pair));
                }
            } else {
                rows.extend(run.iter().map(inline_row));
            }
        }
        rows
    }

    fn retry_diff(&mut self, cx: &mut Context<'_, Self>) {
        let side_unavailable = self
            .diff
            .as_ref()
            .is_some_and(|diff| diff.pairs().is_none());
        if self.head_error || self.paged_diff_unavailable {
            cx.emit(DiffViewEvent::RetryHead);
        } else if self.diff_error || side_unavailable {
            self.schedule_diff_now(cx);
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn collapse_ranges_for_test(&self) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
        self.collapsed_ranges()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn inline_row_count_for_test(&self) -> usize {
        self.inline_rows().len()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn scroll_right_for_test(&mut self, line: usize, cx: &mut Context<'_, Self>) {
        let Some(right) = &self.right else {
            return;
        };
        right.update(cx, |view, cx| view.scroll_to_line_for_test(line, cx));
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn worktree_scroll_line_for_test(&self, cx: &App) -> Option<usize> {
        self.right
            .as_ref()
            .map(|right| right.read(cx).scroll_state_for_test().0)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn head_scroll_line_for_test(&self, cx: &App) -> usize {
        self.left.read(cx).transients_for_test(cx).0
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn head_loaded_for_test(&self) -> bool {
        self.head.is_some()
    }
}

fn inline_row(pair: &DiffPair) -> InlineRow {
    InlineRow {
        head: pair
            .head
            .as_ref()
            .map(|line| (line.index, line.text.clone())),
        worktree: pair
            .worktree
            .as_ref()
            .map(|line| (line.index, line.text.clone())),
        kind: pair.kind,
        collapsed_count: None,
    }
}

fn unchanged_collapse_ranges(pairs: &[DiffPair], head: bool) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < pairs.len() {
        if pairs[index].kind != DiffPairKind::Equal {
            index += 1;
            continue;
        }
        let start = index;
        while index < pairs.len() && pairs[index].kind == DiffPairKind::Equal {
            index += 1;
        }
        let run = &pairs[start..index];
        if run.len() <= COLLAPSE_UNCHANGED_MIN_RUN {
            continue;
        }
        let lines = run
            .iter()
            .filter_map(|pair| {
                if head {
                    pair.head.as_ref().map(|line| line.index)
                } else {
                    pair.worktree.as_ref().map(|line| line.index)
                }
            })
            .collect::<Vec<_>>();
        if lines.len() <= COLLAPSE_UNCHANGED_CONTEXT * 2 {
            continue;
        }
        let start_line = lines[COLLAPSE_UNCHANGED_CONTEXT];
        let end_line = lines[lines.len() - COLLAPSE_UNCHANGED_CONTEXT];
        if start_line < end_line {
            ranges.push(start_line..end_line);
        }
    }
    ranges
}

impl Focusable for DiffView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<Settings>().language;
        let state = self.view_state();
        let mode = state.mode();
        let pairs_available = self.diff.as_ref().and_then(|diff| diff.pairs()).is_some();
        let diff_unavailable =
            self.paged_diff_unavailable || (self.diff.is_some() && !pairs_available);

        let mut controls = div()
            .id("diff-toolbar")
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .flex_wrap()
            .text_xs()
            .text_color(cx.theme().muted_foreground);
        controls = controls.child(control(
            "diff-mode-toggle",
            if mode == DiffViewMode::Inline {
                language.text(Message::EditorDiffModeInline)
            } else {
                language.text(Message::EditorDiffModeSideBySide)
            },
            mode == DiffViewMode::Inline,
            cx,
            cx.listener(|view, _, _, cx| view.toggle_mode(cx)),
        ));
        if mode == DiffViewMode::SideBySide {
            controls = controls.child(control(
                "diff-swap",
                language.text(Message::EditorDiffSwap),
                state.swapped(),
                cx,
                cx.listener(|view, _, _, cx| view.toggle_swap(cx)),
            ));
            controls = controls.child(control(
                "diff-collapse-head",
                language.text(Message::EditorDiffCollapseHead),
                state.collapsed() == Some(DiffCollapsedSide::Head),
                cx,
                cx.listener(|view, _, _, cx| view.toggle_collapsed(DiffCollapsedSide::Head, cx)),
            ));
            controls = controls.child(control(
                "diff-collapse-worktree",
                language.text(Message::EditorDiffCollapseWorktree),
                state.collapsed() == Some(DiffCollapsedSide::Worktree),
                cx,
                cx.listener(|view, _, _, cx| {
                    view.toggle_collapsed(DiffCollapsedSide::Worktree, cx)
                }),
            ));
            controls = controls.child(control(
                "diff-width-narrower",
                language.text(Message::EditorDiffWidthNarrower),
                false,
                cx,
                cx.listener(|view, _, _, cx| view.adjust_width(false, cx)),
            ));
            controls = controls.child(control(
                "diff-width-wider",
                language.text(Message::EditorDiffWidthWider),
                false,
                cx,
                cx.listener(|view, _, _, cx| view.adjust_width(true, cx)),
            ));
            controls = controls.child(control(
                "diff-sync-scroll",
                language.text(Message::EditorDiffSyncScroll),
                state.sync_scroll(),
                cx,
                cx.listener(|view, _, _, cx| view.toggle_sync_scroll(cx)),
            ));
        }
        controls = controls.child(control(
            "diff-collapse-unchanged",
            language.text(Message::EditorDiffCollapseUnchanged),
            state.collapse_unchanged(),
            cx,
            cx.listener(|view, _, _, cx| view.toggle_collapse_unchanged(cx)),
        ));
        let whitespace_label = match state.whitespace() {
            slterm_document::DiffWhitespaceMode::Exact => Message::EditorDiffWhitespaceExact,
            slterm_document::DiffWhitespaceMode::Trailing => Message::EditorDiffWhitespaceTrailing,
            slterm_document::DiffWhitespaceMode::All => Message::EditorDiffWhitespaceAll,
        };
        controls = controls.child(control(
            "diff-whitespace",
            language.text(whitespace_label),
            false,
            cx,
            cx.listener(|view, _, _, cx| view.cycle_whitespace(cx)),
        ));
        let previous_id = format!("diff-previous-change-{}", cx.entity().entity_id());
        let next_id = format!("diff-next-change-{}", cx.entity().entity_id());
        let previous_selector = previous_id.clone();
        let next_selector = next_id.clone();
        let count = self.change_locations().len();
        controls = controls
            .child(div().flex_grow(1.0))
            .child(count.to_string())
            .child(
                Button::new(previous_id)
                    .debug_selector(move || previous_selector.clone())
                    .label(language.text(Message::EditorDiffPrevious))
                    .on_click(cx.listener(|view, _, _, cx| view.previous_change(cx))),
            )
            .child(
                Button::new(next_id)
                    .debug_selector(move || next_selector.clone())
                    .label(language.text(Message::EditorDiffNext))
                    .on_click(cx.listener(|view, _, _, cx| view.next_change(cx))),
            );

        let mut panel = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(controls);
        if self.head_error || self.diff_error || diff_unavailable {
            let message = if self.head_error {
                Message::GitshowReadFailed
            } else if self.diff_error {
                Message::EditorDiffStale
            } else {
                Message::EditorDiffUnavailable
            };
            panel = panel.child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .debug_selector(|| "diff-error-banner".to_owned())
                    .child(language.text(message))
                    .child(control(
                        "diff-retry",
                        language.text(Message::EditorDiffRetry),
                        false,
                        cx,
                        cx.listener(|view, _, _, cx| view.retry_diff(cx)),
                    )),
            );
        }

        let right_content = if let Some(editor) = &self.right {
            editor.clone().into_any_element()
        } else if let Some(paged) = &self.right_paged {
            paged.clone().into_any_element()
        } else {
            div().into_any_element()
        };
        let body = if mode == DiffViewMode::Inline && pairs_available {
            inline_body(self, cx)
        } else if mode == DiffViewMode::Inline {
            // 差异不可用时保留工作区正文与编辑能力，不退化为空面板。
            div()
                .size_full()
                .min_h_0()
                .flex()
                .child(right_content)
                .into_any_element()
        } else {
            side_by_side_body(self, language, state, right_content, cx)
        };
        panel
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

fn control(
    debug_id: &'static str,
    label: &'static str,
    active: bool,
    cx: &Context<'_, DiffView>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(debug_id)
        .debug_selector(move || debug_id.to_owned())
        .px_1()
        .py_0p5()
        .when(active, |element| element.bg(cx.theme().list_active))
        .text_color(if active {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
        .into_any_element()
}

fn pane(
    selector: &'static str,
    label: &'static str,
    content: AnyElement,
    width: Option<u16>,
    cx: &Context<'_, DiffView>,
) -> AnyElement {
    div()
        .id(selector)
        .debug_selector(move || selector.to_owned())
        .h_full()
        .min_w_0()
        .flex()
        .flex_col()
        .when_some(width, |element, width| {
            element.w(px(f32::from(width))).flex_none()
        })
        .when(width.is_none(), |element| element.flex_1())
        .child(
            div()
                .px_2()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_h_0().child(content))
        .into_any_element()
}

fn side_by_side_body(
    view: &DiffView,
    language: crate::i18n::UiLanguage,
    state: DiffViewState,
    right_content: AnyElement,
    cx: &Context<'_, DiffView>,
) -> AnyElement {
    let head_label = language.text(Message::EditorDiffHead);
    let worktree_label = language.text(Message::EditorDiffWorktree);
    let width = Some(state.column_width());
    let mut body = div().size_full().min_h_0().flex();
    match state.collapsed() {
        Some(DiffCollapsedSide::Head) => {
            body = body.child(pane(
                "diff-pane-worktree",
                worktree_label,
                right_content,
                None,
                cx,
            ));
        }
        Some(DiffCollapsedSide::Worktree) => {
            body = body.child(pane(
                "diff-pane-head",
                head_label,
                view.left.clone().into_any_element(),
                None,
                cx,
            ));
        }
        None => {
            let divider = || div().w(px(1.0)).h_full().bg(cx.theme().border);
            if state.swapped() {
                body = body
                    .child(pane(
                        "diff-pane-worktree",
                        worktree_label,
                        right_content,
                        width,
                        cx,
                    ))
                    .child(divider())
                    .child(pane(
                        "diff-pane-head",
                        head_label,
                        view.left.clone().into_any_element(),
                        None,
                        cx,
                    ));
            } else {
                body = body
                    .child(pane(
                        "diff-pane-head",
                        head_label,
                        view.left.clone().into_any_element(),
                        width,
                        cx,
                    ))
                    .child(divider())
                    .child(pane(
                        "diff-pane-worktree",
                        worktree_label,
                        right_content,
                        None,
                        cx,
                    ));
            }
        }
    }
    body.into_any_element()
}

fn inline_body(view: &DiffView, cx: &Context<'_, DiffView>) -> AnyElement {
    let language = cx.global::<Settings>().language;
    let rows = view.inline_rows();
    let mono = cx.theme().mono_font_family.clone();
    let mut list = div()
        .id("diff-inline")
        .debug_selector(|| "diff-inline".to_owned())
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .font_family(mono)
        .text_xs();
    for row in rows {
        if let Some(count) = row.collapsed_count {
            list = list.child(
                div()
                    .px_2()
                    .text_color(cx.theme().muted_foreground)
                    .debug_selector(|| "diff-inline-collapsed".to_owned())
                    .child(language.format(
                        Message::EditorDiffCollapsedLines,
                        &[("count", &count.to_string())],
                    )),
            );
            continue;
        }
        let (head_line, worktree_line, marker, color) = match row.kind {
            DiffPairKind::Equal => (
                row.head.as_ref().map(|line| line.0),
                row.worktree.as_ref().map(|line| line.0),
                " ",
                cx.theme().foreground,
            ),
            DiffPairKind::Delete => (
                row.head.as_ref().map(|line| line.0),
                None,
                "-",
                cx.theme().danger,
            ),
            DiffPairKind::Insert => (
                None,
                row.worktree.as_ref().map(|line| line.0),
                "+",
                cx.theme().success,
            ),
            DiffPairKind::Replace => (
                row.head.as_ref().map(|line| line.0),
                row.worktree.as_ref().map(|line| line.0),
                "~",
                cx.theme().warning,
            ),
        };
        list = list.child(
            div()
                .flex()
                .gap_1()
                .text_color(color)
                .child(number_slot(head_line))
                .child(number_slot(worktree_line))
                .child(marker)
                .child(row.head.as_ref().map_or_else(
                    || {
                        row.worktree
                            .as_ref()
                            .map(|line| line.1.clone())
                            .unwrap_or_default()
                    },
                    |line| line.1.clone(),
                )),
        );
    }
    list.into_any_element()
}

fn number_slot(line: Option<usize>) -> AnyElement {
    div()
        .w(px(44.0))
        .flex_none()
        .text_right()
        .child(line.map(|line| (line + 1).to_string()).unwrap_or_default())
        .into_any_element()
}

impl super::AppRoot {
    pub(crate) fn open_diff(
        &mut self,
        request: DiffOpenRequest,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<slterm_workspace::PanelId, super::commands::AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        let DiffOpenRequest {
            project_id,
            target_group,
            document_id,
            binding,
            path,
            focus,
            target_line,
        } = request;
        let view_key = slterm_workspace::ViewKey::Diff {
            project_id,
            document_id,
        };
        if let Ok(panel_id) = self.panel_for_view(project_id, &view_key, cx) {
            self.activate_view(view_key, window, cx)?;
            if let Some(line) = target_line {
                let view = self.diff_view(panel_id, cx)?;
                view.update(cx, |view, cx| view.request_line(line, cx));
            }
            return Ok(panel_id);
        }
        let view_states = self.side_view_persistence.editor_view_states();
        let view_state = view_states.diff_state(&view_key).unwrap_or_else(|| {
            DiffViewState::new(slterm_document::DiffBinding::new(binding, path))
        });
        view_state
            .binding()
            .map_err(|_| super::commands::AppRootError::MissingPanel)?;
        let already_in_layout = self
            .snapshot
            .registry()
            .project(project_id)
            .is_some_and(|project| project.layout().view_keys().contains(&view_key));
        let candidate = if already_in_layout {
            None
        } else {
            Some(self.snapshot.with_opened_view(
                project_id,
                target_group.ok_or(super::commands::AppRootError::MissingPanel)?,
                view_key.clone(),
            )?)
        };
        let target = super::documents::DocumentPanelTarget {
            project_id,
            view: view_key.clone(),
            focus,
            activate_project: false,
        };
        let built = self.build_diff_content(&target, document_id, target_line, window, cx)?;
        let panel_id = self.install_panel(target, window, cx, move |_, _| built)?;
        if let Some(candidate) = candidate {
            self.commit_project_snapshot(candidate, cx);
        }
        // 只有面板真实建立后才发布新固定绑定，失败重试不得继承未生效请求。
        view_states.restore_diff(view_key, view_state);
        let view = self.diff_view(panel_id, cx)?;
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |root, view, event: &DiffViewEvent, window, cx| match event {
                DiffViewEvent::RetryHead => {
                    root.retry_diff_head(panel_id, view.clone(), window, cx)
                }
            },
        );
        self.panel_subscriptions.insert(panel_id, subscription);
        let owner = super::task_service::ViewTaskOwner::Panel(panel_id);
        if let Some(ticket) = self.tasks.request_view_refresh(owner) {
            self.schedule_diff_head(panel_id, view, ticket, false, cx);
        }
        Ok(panel_id)
    }

    pub(super) fn diff_view(
        &self,
        panel_id: slterm_workspace::PanelId,
        cx: &App,
    ) -> Result<Entity<DiffView>, super::commands::AppRootError> {
        self.projects
            .values()
            .find_map(|project| project.read(cx).panels.get(&panel_id))
            .and_then(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .ok_or(super::commands::AppRootError::MissingPanel)
    }

    /// Diff 错误横幅重试：复用固定绑定与可见性票据，不前进 HEAD。
    fn retry_diff_head(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        view: Entity<DiffView>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return;
        }
        let owner = super::task_service::ViewTaskOwner::Panel(panel_id);
        if let Some(ticket) = self.tasks.request_view_refresh(owner) {
            self.schedule_diff_head(panel_id, view, ticket, false, cx);
        }
    }

    pub(super) fn schedule_diff_head(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        view: Entity<DiffView>,
        ticket: super::task_service::ViewRefreshTicket,
        refresh: bool,
        cx: &mut App,
    ) {
        let task_id = super::task_service::TaskId::DiffHead(panel_id);
        let fixed = view
            .read(cx)
            .view_state()
            .binding()
            .expect("Diff 固定绑定在读取前必须有效");
        let generation = view.update(cx, |view, cx| {
            view.head_loading = true;
            view.left.update(cx, |left, _| left.begin_load())
        });
        let grants = self.gitshow_grants(fixed.repository_binding());
        let git = self.git.clone();
        let executor = cx.background_executor().clone();
        let load = executor.spawn(async move {
            let binding = if refresh {
                git.refresh_diff_binding(&fixed, &grants, 0)?
            } else {
                fixed
            };
            let snapshot = git.show(
                binding.repository_binding(),
                binding.head_path(),
                &grants,
                0,
                generation,
            )?;
            Ok::<_, GitError>((snapshot, refresh.then_some(binding)))
        });
        let root = view.read(cx).root.clone();
        let task_view = view.downgrade();
        let cancellation = super::task_service::CancellationToken::new();
        let cancelled = cancellation.clone();
        let registration = self.tasks.register_with(task_id, cancellation, || {
            view.update(cx, |_, cx| {
                cx.spawn(async move |_, cx| {
                    let result = load.await;
                    if cancelled.is_cancelled() {
                        return;
                    }
                    let _ = root.update(cx, |root, cx| {
                        root.finish_diff_head(panel_id, task_view, ticket, result, cx);
                    });
                })
            })
        });
        if registration.is_err() {
            log::debug!("diff HEAD load already scheduled");
        }
    }

    pub(super) fn finish_diff_head(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        view: gpui::WeakEntity<DiffView>,
        ticket: super::task_service::ViewRefreshTicket,
        result: DiffHeadResult,
        cx: &mut Context<'_, Self>,
    ) {
        let completion = self.tasks.finish_view_refresh(ticket);
        if completion.outcome == super::task_service::ViewRefreshOutcome::Stale {
            return;
        }
        self.tasks
            .complete(super::task_service::TaskId::DiffHead(panel_id));
        let Some(view) = view.upgrade() else {
            return;
        };
        view.update(cx, |view, cx| view.apply_head(result, cx));
        if let Some(next) = completion.next {
            self.schedule_diff_head(panel_id, view, next, true, cx);
        }
    }
}

#[cfg(test)]
mod refresh_state_tests {
    use super::DiffRefreshState;

    /// 连续编辑在窗口内只产生一次启动；在途期间的新编辑只保留一个待算位。
    #[test]
    fn edit_burst_coalesces_and_keeps_single_flight() {
        let mut state = DiffRefreshState::default();
        assert!(!state.on_edit());
        assert!(!state.on_edit());
        assert!(state.on_debounce(), "窗口到点后只启动一次");

        // 在途期间的编辑立即请求取消，但不得再启动第二个任务。
        assert!(state.on_edit(), "在途编辑必须请求取消旧计算");
        assert!(!state.on_debounce(), "在途时不得并行启动");
        assert!(state.dirty);
        assert!(state.on_finished(), "结束后补算最新一次");
        assert!(!state.on_finished(), "没有新编辑时不得重算");
    }

    /// 立即路径在途时也保持单飞行：取消旧计算，结束后补算最新输入。
    #[test]
    fn immediate_refresh_waits_for_cancelled_compute() {
        let mut state = DiffRefreshState::default();
        assert!(state.force());
        assert!(!state.force());
        assert!(state.on_finished());
        assert!(!state.on_finished());
    }

    /// 隐藏重置会丢弃脏位与合并窗口，重新可见前不得自行启动。
    #[test]
    fn reset_discards_pending_work() {
        let mut state = DiffRefreshState::default();
        let _ = state.on_edit();
        state.reset();
        assert!(!state.on_finished());
        assert!(!state.on_debounce());
    }
}
