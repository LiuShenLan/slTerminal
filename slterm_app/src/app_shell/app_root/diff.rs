//! Diff 面板：左侧固定 HEAD，右侧直接编辑共享 Document。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Task, Window,
    div, px,
};
use gpui_component::{ActiveTheme as _, button::Button};
use slterm_document::{DiffPairKind, DiffSnapshot, DocumentId, DocumentPageCache, compute_diff};

use super::git_show::GitShowView;
use super::panel_host::{EditorContent, PanelContent};
use crate::app_shell::config::Settings;
use crate::app_shell::text_view::{
    DiffViewState, DocumentTextView, EditorViewStateLease, EditorViewStateStore,
};
use crate::git::{GitError, GitShowSnapshot};
use crate::i18n::Message;

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
    right: Entity<DocumentTextView>,
    _right_subscription: gpui::Subscription,
    head: Option<GitShowSnapshot>,
    head_loading: bool,
    view_states: EditorViewStateStore,
    view_lease: EditorViewStateLease,
    diff: Option<DiffSnapshot>,
    diff_generation: u64,
    diff_task: Option<Task<()>>,
    active_change: Option<usize>,
    pending_line: Option<usize>,
}

impl DiffView {
    fn new(
        root: gpui::WeakEntity<super::AppRoot>,
        document_id: DocumentId,
        right: EditorContent,
        cache: Rc<RefCell<DocumentPageCache>>,
        pending_line: Option<usize>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let (right, subscription) = right.into_parts();
        let (view_states, view_lease) = right
            .read(cx)
            .view_state_access()
            .expect("Diff 工作区侧必须持有专属 ViewState 租约");
        let focus = cx.focus_handle().tab_stop(true);
        right.update(cx, |view, _| view.share_focus(focus.clone()));
        let left = cx.new(|cx| GitShowView::loading(cache, cx));
        left.update(cx, |view, cx| view.share_focus(focus.clone(), cx));
        Self {
            root,
            document_id,
            focus,
            left,
            right,
            _right_subscription: subscription,
            head: None,
            head_loading: false,
            view_states,
            view_lease,
            diff: None,
            diff_generation: 0,
            diff_task: None,
            active_change: None,
            pending_line,
        }
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
                self.left.update(cx, |view, cx| {
                    view.apply_result(generation, Ok(snapshot), cx);
                    view.share_focus(self.focus.clone(), cx);
                });
            }
            Err(error) => {
                // 刷新失败只替换展示错误；保留最后一次已验证的固定快照供恢复与差分使用。
                self.left
                    .update(cx, |view, cx| view.apply_result(generation, Err(error), cx));
            }
        }
        if self.head.is_some() {
            self.schedule_diff(cx);
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
        if self.right.entity_id() != origin {
            self.right.update(cx, |view, cx| {
                view.apply_edit_snapshot(snapshot, transaction, cx)
            });
        }
        // 来源视图仍在 GPUI 更新栈中；差分捕获延迟到本次广播结束后，避免重入读取。
        let target = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = target.update(cx, |view, cx| view.schedule_diff(cx));
        });
        cx.notify();
    }

    pub(crate) fn apply_shared_snapshot(
        &mut self,
        session: &slterm_document::DocumentSession,
        cx: &mut Context<'_, Self>,
    ) {
        self.right
            .update(cx, |view, cx| view.apply_snapshot(session, cx));
        self.schedule_diff(cx);
        cx.notify();
    }

    fn schedule_diff(&mut self, cx: &mut Context<'_, Self>) {
        let Some(head) = self.head.clone() else {
            return;
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        let snapshot = self.right.read(cx).text_snapshot();
        let head_bytes = head
            .bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .map_or_else(|| head.bytes.clone(), Arc::from);
        let input = slterm_document::DiffInput::git(
            head_bytes,
            Arc::from(snapshot.text.to_string().into_bytes()),
            self.view_state().whitespace(),
            snapshot.text_state,
            head.commit.clone(),
            generation,
        );
        let compute = cx
            .background_executor()
            .spawn(async move { compute_diff(&input) });
        let target = cx.entity().downgrade();
        self.diff_task = Some(cx.spawn(async move |_, cx| {
            let result = compute.await;
            let _ = target.update(cx, |view, cx| view.apply_diff(generation, result, cx));
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
        self.diff = result.ok();
        self.active_change = None;
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
        self.right.update(cx, |view, cx| view.reveal_line(line, cx));
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
        &self.right
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
        self.right.update(cx, |view, cx| {
            view.set_wrap(source.wrap(), cx);
            gpui::EntityInputHandler::set_selected_text_range(view, 0..1, window, cx);
            view.set_scroll_for_test(3, 2);
        });
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn view_state_for_test(&self) -> DiffViewState {
        self.view_state()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn transients_for_test(&self, cx: &App) -> (usize, std::ops::Range<usize>) {
        let state = self.right.read(cx).scroll_state_for_test();
        (state.0, state.3)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn shared_identity_for_test(&self) -> DocumentId {
        self.document_id
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn shared_text_for_test(&self, cx: &App) -> String {
        self.right.read(cx).input_text_for_test()
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

impl Focusable for DiffView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let language = cx.global::<Settings>().language;
        let pane = |label: &str, content: AnyElement, selector: &'static str| {
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px_2()
                        .text_color(cx.theme().muted_foreground)
                        .debug_selector(move || selector.to_owned())
                        .child(label.to_owned()),
                )
                .child(div().flex_1().min_h_0().child(content))
        };
        let previous_id = format!("diff-previous-change-{}", cx.entity().entity_id());
        let next_id = format!("diff-next-change-{}", cx.entity().entity_id());
        let previous_selector = previous_id.clone();
        let next_selector = next_id.clone();
        let count = self.change_locations().len();
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .px_2()
                    .text_color(cx.theme().muted_foreground)
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
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(pane(
                        language.text(Message::EditorDiffHead),
                        self.left.clone().into_any_element(),
                        "diff-head",
                    ))
                    .child(div().w(px(1.0)).h_full().bg(cx.theme().border))
                    .child(pane(
                        language.text(Message::EditorDiffWorktree),
                        self.right.clone().into_any_element(),
                        "diff-worktree",
                    )),
            )
    }
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
        let right = self.build_source_content(&target, document_id, window, cx)?;
        let PanelContent::Editor(right) = right.0 else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let cache = self.page_cache.clone();
        let root = cx.entity().downgrade();
        let panel_id = self.install_panel(target, window, cx, move |_, cx| {
            let view = cx.new(|cx| DiffView::new(root, document_id, right, cache, target_line, cx));
            let focus = view.read(cx).focus_handle(cx);
            (PanelContent::Diff(view), focus)
        })?;
        if let Some(candidate) = candidate {
            self.commit_project_snapshot(candidate, cx);
        }
        // 只有面板真实建立后才发布新固定绑定，失败重试不得继承未生效请求。
        view_states.restore_diff(view_key, view_state);
        let view = self.diff_view(panel_id, cx)?;
        let owner = super::task_service::ViewTaskOwner::Panel(panel_id);
        if let Some(ticket) = self.tasks.request_view_refresh(owner) {
            self.schedule_diff_head(panel_id, view, ticket, false, cx);
        }
        Ok(panel_id)
    }

    fn diff_view(
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
