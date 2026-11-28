//! Commit 视图运行态：仓库状态分组、打开映射与展示参数。
//!
//! 数据根固定为活跃项目所在仓库的 RepoRoot；行只投影状态事实，
//! 打开动作统一产生 `CommitIntent`，由 AppRoot 复用既有打开链。

use std::collections::BTreeSet;

use gpui::{
    AnyElement, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Task, WeakEntity, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::DocumentId;
use slterm_path::CanonicalPath;
use slterm_sidebar::SideViewState;
use slterm_workspace::{ProjectId, ProjectRegistryError};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusRegistration;
use super::side_view::SideView;
use super::task_service::{CancellationToken, TaskId};

mod confirm;

pub(super) use confirm::render_commit_confirmation;

/// 三项分组顺序固定；ignored 只作展示，不参与打开映射。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CommitGroup {
    Changes,
    Unversioned,
    Ignored,
}

impl CommitGroup {
    const ALL: [Self; 3] = [Self::Changes, Self::Unversioned, Self::Ignored];

    fn title(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Unversioned => "Unversioned Files",
            Self::Ignored => "Ignored",
        }
    }
}

/// 单行状态事实；路径为绝对工作区位置，展示用仓库相对路径。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CommitRow {
    pub path: CanonicalPath,
    pub repo_relative: String,
    pub name: String,
    pub group: CommitGroup,
    pub index: char,
    pub worktree: char,
    pub status_label: String,
    pub conflict: bool,
    pub ignored: bool,
    pub untracked: bool,
    pub renamed_from: Option<String>,
}

/// 打开映射只区分三种落点；unknown 缺失文件不产生意图。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommitOpenKind {
    Document,
    Diff,
    GitShow,
}

/// 已在后台捕获的 Diff 打开事实；等待工作区 Document 注册后兑现。
pub(super) struct PendingCommitDiff {
    project_id: ProjectId,
    target_group: slterm_workspace::PanelGroupId,
    binding: slterm_document::RepositoryBinding,
    repo_path: slterm_document::RepoPath,
}

/// 已通过前置校验、等待用户确认的破坏性写。
#[derive(Clone, Debug)]
pub(super) enum CommitWriteAction {
    Rollback {
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        kind: RollbackKind,
    },
    Delete {
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        permanent: bool,
    },
}

impl CommitWriteAction {
    fn path(&self) -> &CanonicalPath {
        match self {
            Self::Rollback { row, .. } | Self::Delete { row, .. } => &row.path,
        }
    }
}

/// 唯一破坏性确认面：脏 Document 的保存／丢弃决定与操作确认共用。
pub(super) struct CommitConfirmation {
    action: CommitWriteAction,
    dirty_documents: Vec<DocumentId>,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

/// 保存完成后继续同一 Git 写；任一保存失败即取消。
pub(super) struct CommitWriteAfterSave {
    action: CommitWriteAction,
    pending: BTreeSet<DocumentId>,
    window_handle: gpui::AnyWindowHandle,
    root: WeakEntity<AppRoot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CommitConfirmChoice {
    Execute,
    Save,
    Discard,
}

pub(super) enum CommitIntent {
    Open {
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        kind: CommitOpenKind,
    },
    LoadStatus {
        project_id: ProjectId,
        root: CanonicalPath,
        generation: u64,
    },
    Rollback {
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        kind: RollbackKind,
    },
    Delete {
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        permanent: bool,
    },
}

fn status_label(index: char, worktree: char, conflict: bool) -> String {
    if conflict {
        return "U".to_owned();
    }
    let mut label = String::new();
    if index != '.' && index != '?' {
        label.push(index);
    }
    if worktree != '.' {
        label.push(worktree);
    }
    if label.is_empty() {
        "?".to_owned()
    } else {
        label
    }
}

/// 状态快照投影为固定分组行；路径用仓库根拼接且不越界。
pub(super) fn build_rows(
    repo_root: &CanonicalPath,
    snapshot: &crate::git::StatusSnapshot,
) -> Vec<CommitRow> {
    let mut rows = Vec::with_capacity(snapshot.entries.len());
    for entry in &snapshot.entries {
        let relative = entry.path.trim_matches('/').to_owned();
        if relative.is_empty() {
            continue;
        }
        let Ok(path) = CanonicalPath::new(
            repo_root
                .as_path()
                .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)),
        ) else {
            continue;
        };
        if !path.as_path().starts_with(repo_root.as_path()) {
            continue;
        }
        let name = relative.rsplit('/').next().unwrap_or(&relative).to_owned();
        let group = if entry.ignored {
            CommitGroup::Ignored
        } else if entry.index == '?' {
            CommitGroup::Unversioned
        } else {
            CommitGroup::Changes
        };
        rows.push(CommitRow {
            path,
            repo_relative: relative,
            name,
            group,
            index: entry.index,
            worktree: entry.worktree,
            status_label: status_label(entry.index, entry.worktree, entry.conflict),
            conflict: entry.conflict,
            ignored: entry.ignored,
            untracked: entry.index == '?',
            renamed_from: entry.original.clone(),
        });
    }
    rows.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| left.repo_relative.cmp(&right.repo_relative))
    });
    rows
}

/// 通用回滚只覆盖精确的未暂存 modified/deleted。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RollbackKind {
    Modified,
    Deleted,
}

/// 不具备可放弃的未暂存变化或形态不可靠时明确禁用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RollbackDisabled {
    Conflict,
    StagedOnly,
    StagedRename,
    NotTracked,
    Indeterminate,
}

pub(super) fn rollback_availability(row: &CommitRow) -> Result<RollbackKind, RollbackDisabled> {
    if row.conflict {
        return Err(RollbackDisabled::Conflict);
    }
    if row.ignored || row.untracked {
        return Err(RollbackDisabled::NotTracked);
    }
    if row.index == 'R' {
        return Err(RollbackDisabled::StagedRename);
    }
    if row.worktree == 'R' {
        return Err(RollbackDisabled::Indeterminate);
    }
    if matches!(row.index, 'A' | 'D') {
        return Err(RollbackDisabled::StagedOnly);
    }
    match row.worktree {
        'M' => Ok(RollbackKind::Modified),
        'D' => Ok(RollbackKind::Deleted),
        '.' | ' ' => Err(RollbackDisabled::StagedOnly),
        _ => Err(RollbackDisabled::Indeterminate),
    }
}

/// 打开映射：ignored 不打开；untracked/added 走普通文档；
/// deleted 走固定版本；modified/renamed/conflict 走共享 Diff。
fn open_kind(row: &CommitRow) -> Option<CommitOpenKind> {
    open_kind_for_status(row.index, row.worktree, row.conflict, row.ignored)
}

/// 状态条目到打开方式的固定映射；Commit 行激活与 Runtime 打开方法共用。
pub(crate) fn open_kind_for_status(
    index: char,
    worktree: char,
    conflict: bool,
    ignored: bool,
) -> Option<CommitOpenKind> {
    if ignored {
        return None;
    }
    let label = status_label(index, worktree, conflict);
    if conflict || label.contains('M') || label.contains('R') {
        return Some(CommitOpenKind::Diff);
    }
    if label.contains('D') {
        return Some(CommitOpenKind::GitShow);
    }
    Some(CommitOpenKind::Document)
}

pub(super) struct CommitRuntime {
    project: ProjectId,
    /// 活跃项目根：写能力与打开落点只认它。
    project_root: Option<CanonicalPath>,
    /// 仓库数据根：行路径一律相对它拼接。
    repo_root: Option<CanonicalPath>,
    rows: Vec<CommitRow>,
    filter: String,
    show_ignored: bool,
    selected: Option<CanonicalPath>,
    collapsed: [bool; 3],
    scroll_handle: gpui::ScrollHandle,
    restoring_scroll: Option<f64>,
    filter_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    _filter_change: Option<gpui::Subscription>,
    loading: bool,
    stale: bool,
    /// 回收站删除失败后的待永久确认目标；仅该路径允许再次按 Delete 永久删除。
    pending_permanent: Option<CanonicalPath>,
    generation: u64,
    /// 在途状态读取期间又出现新数据根：只保留合并位，回执后重投。
    pending_status: bool,
    task: Option<Task<()>>,
}

impl CommitRuntime {
    fn new(project: ProjectId) -> Self {
        Self {
            project,
            project_root: None,
            repo_root: None,
            rows: Vec::new(),
            filter: String::new(),
            show_ignored: false,
            selected: None,
            collapsed: [false, false, false],
            scroll_handle: gpui::ScrollHandle::new(),
            restoring_scroll: None,
            filter_input: None,
            _filter_change: None,
            loading: false,
            stale: false,
            pending_permanent: None,
            generation: 0,
            pending_status: false,
            task: None,
        }
    }

    /// 在途读取期间的新请求只保留合并位；回执完成后由同一 owner 重投。
    fn defer_status(&mut self) {
        self.pending_status = true;
        self.loading = true;
    }

    fn take_deferred_status(&mut self) -> bool {
        std::mem::take(&mut self.pending_status)
    }

    fn group_index(group: CommitGroup) -> usize {
        match group {
            CommitGroup::Changes => 0,
            CommitGroup::Unversioned => 1,
            CommitGroup::Ignored => 2,
        }
    }

    fn matches_filter(&self, row: &CommitRow) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let needle = self.filter.to_lowercase();
        row.name.to_lowercase().contains(&needle)
            || row.repo_relative.to_lowercase().contains(&needle)
    }

    fn visible_rows(&self) -> Vec<&CommitRow> {
        let mut rows = Vec::new();
        for group in CommitGroup::ALL {
            if group == CommitGroup::Ignored && !self.show_ignored {
                continue;
            }
            if self.collapsed[Self::group_index(group)] {
                continue;
            }
            rows.extend(
                self.rows
                    .iter()
                    .filter(|row| row.group == group && self.matches_filter(row)),
            );
        }
        rows
    }

    fn row_for_path(&self, path: &CanonicalPath) -> Option<&CommitRow> {
        self.rows.iter().find(|row| &row.path == path)
    }
}

impl SideView {
    /// 活跃项目变化后重建数据根；过滤与当前行随数据根变化清空。
    pub(super) fn sync_commit_root(
        &mut self,
        project: ProjectId,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let restored = match &self.state {
            SideViewState::Commit(state) => Some(state.clone()),
            _ => None,
        };
        let unchanged = self.commit.as_ref().is_some_and(|runtime| {
            runtime.project == project && runtime.project_root.as_ref() == Some(&root)
        });
        if unchanged {
            return;
        }
        let previous = self.commit.take();
        let generation = previous
            .as_ref()
            .map(|old| old.generation.wrapping_add(1))
            .unwrap_or(1);
        let mut runtime = CommitRuntime::new(project);
        runtime.generation = generation;
        runtime.project_root = Some(root.clone());
        runtime.filter.clear();
        runtime.selected = None;
        runtime.rows.clear();
        runtime.loading = true;
        if let Some(restored) = restored {
            runtime.show_ignored = restored.show_ignored();
            runtime.collapsed = [
                restored.changes_collapsed(),
                restored.unversioned_collapsed(),
                restored.ignored_collapsed(),
            ];
            runtime.restoring_scroll = Some(restored.scroll());
        }
        // 旧任务随旧运行态释放；迟到回执由视图代次拒绝。
        runtime.task = previous.and_then(|mut old| old.task.take());
        self.commit = Some(runtime);
        self.ensure_commit_filter_input(window, cx);
        self.dispatch_commit_intent(
            CommitIntent::LoadStatus {
                project_id: project,
                root,
                generation,
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn ensure_commit_filter_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &self.commit else {
            return;
        };
        if runtime.filter_input.is_some() {
            return;
        }
        let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx));
        let change = cx.subscribe_in(
            &input,
            window,
            |view, input, event: &gpui_component::input::InputEvent, _, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    view.commit_set_filter(&text, cx);
                }
            },
        );
        if let Some(runtime) = &mut self.commit {
            runtime.filter_input = Some(input);
            runtime._filter_change = Some(change);
        }
    }

    pub(super) fn clear_commit_root(&mut self, cx: &mut Context<'_, Self>) {
        if self.commit.take().is_some() {
            cx.notify();
        }
    }

    /// 状态回执只替换列表；失败保留旧列表并标记可能过期。
    pub(super) fn commit_apply_status(
        &mut self,
        generation: u64,
        repo_root: CanonicalPath,
        snapshot: Result<std::sync::Arc<crate::git::StatusSnapshot>, ()>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.commit else {
            return;
        };
        if runtime.generation != generation {
            return;
        }
        runtime.loading = false;
        match snapshot {
            Ok(snapshot) => {
                runtime.repo_root = Some(repo_root.clone());
                runtime.rows = build_rows(&repo_root, &snapshot);
                runtime.stale = false;
            }
            Err(()) => {
                runtime.stale = !runtime.rows.is_empty();
            }
        }
        if let Some(scroll) = runtime.restoring_scroll.take()
            && scroll > 0.0
        {
            runtime
                .scroll_handle
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-(scroll as f32))));
        }
        cx.notify();
    }

    /// 折叠、忽略开关与滚动是唯一持久展示参数；过滤与当前行不写盘。
    pub(super) fn checkpoint_commit_state(&mut self) {
        let Some(runtime) = &self.commit else {
            return;
        };
        let collapsed = runtime.collapsed;
        let show_ignored = runtime.show_ignored;
        let scroll = f64::from(-runtime.scroll_handle.offset().y.as_f32()).max(0.0);
        let _ = self.edit_state(|state| {
            let SideViewState::Commit(state) = state else {
                return;
            };
            state.set_changes_collapsed(collapsed[0]);
            state.set_unversioned_collapsed(collapsed[1]);
            state.set_ignored_collapsed(collapsed[2]);
            state.set_show_ignored(show_ignored);
            state.set_scroll(scroll);
        });
    }

    pub(super) fn commit_set_show_ignored(&mut self, show: bool, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.commit {
            runtime.show_ignored = show;
        }
        self.checkpoint_commit_state();
        cx.notify();
    }

    pub(super) fn commit_toggle_group(&mut self, group: CommitGroup, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.commit {
            let index = CommitRuntime::group_index(group);
            runtime.collapsed[index] = !runtime.collapsed[index];
        }
        self.checkpoint_commit_state();
        cx.notify();
    }

    pub(super) fn commit_set_filter(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.commit {
            runtime.filter = text.to_owned();
        }
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn commit_visible_row_count_for_test(&self) -> usize {
        self.commit
            .as_ref()
            .map(|runtime| runtime.visible_rows().len())
            .unwrap_or_default()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn commit_show_ignored(&self) -> bool {
        self.commit
            .as_ref()
            .is_some_and(|runtime| runtime.show_ignored)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn commit_group_collapsed(&self, group: CommitGroup) -> bool {
        self.commit
            .as_ref()
            .is_some_and(|runtime| runtime.collapsed[CommitRuntime::group_index(group)])
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn commit_filter(&self) -> &str {
        self.commit
            .as_ref()
            .map(|runtime| runtime.filter.as_str())
            .unwrap_or_default()
    }

    pub(super) fn commit_generation_for_refresh(&self) -> Option<u64> {
        self.commit.as_ref().map(|runtime| runtime.generation)
    }

    /// 同一项目的刷新目标；视图未打开或数据根不一致时返回 None。
    pub(super) fn commit_defer_status(&mut self, generation: u64) {
        if let Some(runtime) = &mut self.commit
            && runtime.generation == generation
        {
            runtime.defer_status();
        }
    }

    pub(super) fn commit_take_deferred_status(&mut self) -> bool {
        self.commit
            .as_mut()
            .is_some_and(CommitRuntime::take_deferred_status)
    }

    pub(super) fn commit_refresh_target(&self) -> Option<(ProjectId, CanonicalPath, u64)> {
        let runtime = self.commit.as_ref()?;
        Some((
            runtime.project,
            runtime.project_root.clone()?,
            runtime.generation,
        ))
    }

    pub(super) fn commit_scroll_handle(&self) -> Option<&gpui::ScrollHandle> {
        self.commit.as_ref().map(|runtime| &runtime.scroll_handle)
    }

    /// 待永久确认目标；只有该路径的 Delete 会被提升为永久删除。
    pub(super) fn commit_pending_permanent_path(&self) -> Option<&CanonicalPath> {
        self.commit
            .as_ref()
            .and_then(|runtime| runtime.pending_permanent.as_ref())
    }

    /// 删除失败回投：仅回收站不可用且非永久请求时保留待确认窗口。
    pub(super) fn commit_set_pending_permanent(
        &mut self,
        path: Option<CanonicalPath>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.commit else {
            return;
        };
        if runtime.pending_permanent == path {
            return;
        }
        runtime.pending_permanent = path;
        cx.notify();
    }

    /// 键盘入口：选中行普通删除；已处于待确认的同一行提升为永久删除。
    pub(super) fn commit_delete_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(path) = self
            .commit
            .as_ref()
            .and_then(|runtime| runtime.selected.clone())
        else {
            return;
        };
        if self.commit_pending_permanent_path() == Some(&path) {
            self.commit_force_delete_row(&path, window, cx);
        } else {
            self.commit_delete_row(&path, window, cx);
        }
    }

    /// 单击选中不打开；选中行是 Ctrl+R 回滚与后续菜单动作的目标。
    pub(super) fn commit_select_at(&mut self, path: &CanonicalPath, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.commit else {
            return;
        };
        if runtime.row_for_path(path).is_none() {
            return;
        }
        runtime.selected = Some(path.clone());
        cx.notify();
    }

    /// 键盘入口：对选中行请求回滚。
    pub(super) fn commit_rollback_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(path) = self
            .commit
            .as_ref()
            .and_then(|runtime| runtime.selected.clone())
        else {
            return;
        };
        self.commit_rollback(&path, window, cx);
    }

    /// untracked／staged added 删除入口；回收站不可用时由 AppRoot 保留待确认。
    pub(super) fn commit_delete_row(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_request_delete(path, false, window, cx);
    }

    /// 显式确认后的永久删除入口；只在回收站不可用的后续确认使用。
    pub(super) fn commit_force_delete_row(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_request_delete(path, true, window, cx);
    }

    fn commit_request_delete(
        &mut self,
        path: &CanonicalPath,
        permanent: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.commit else {
            return;
        };
        let Some(row) = runtime.row_for_path(path).cloned() else {
            return;
        };
        // 只有 untracked 与 staged added 可删除；其他形态必须先回滚或提交。
        if !(row.untracked || row.index == 'A') {
            return;
        }
        let Some(root) = runtime.repo_root.clone() else {
            return;
        };
        let project_id = runtime.project;
        self.dispatch_commit_intent(
            CommitIntent::Delete {
                project_id,
                root,
                row,
                permanent,
            },
            window,
            cx,
        );
    }

    /// 回滚入口：先产生确认意图，由 AppRoot 的同一决定面取得处置。
    pub(super) fn commit_rollback(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.commit else {
            return;
        };
        let Some(row) = runtime.row_for_path(path).cloned() else {
            return;
        };
        let Ok(kind) = rollback_availability(&row) else {
            return;
        };
        let Some(root) = runtime.repo_root.clone() else {
            return;
        };
        let project_id = runtime.project;
        self.dispatch_commit_intent(
            CommitIntent::Rollback {
                project_id,
                root,
                row,
                kind,
            },
            window,
            cx,
        );
    }

    /// 激活行：按状态映射打开 Document、Diff 或 GitShow。
    pub(super) fn commit_activate_row(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.commit else {
            return;
        };
        let Some(row) = runtime.row_for_path(path).cloned() else {
            return;
        };
        let Some(kind) = open_kind(&row) else {
            return;
        };
        let Some(root) = runtime.repo_root.clone() else {
            return;
        };
        let project_id = runtime.project;
        self.dispatch_commit_intent(
            CommitIntent::Open {
                project_id,
                root,
                row,
                kind,
            },
            window,
            cx,
        );
    }

    fn dispatch_commit_intent(
        &mut self,
        intent: CommitIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(root) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root.update(cx, |root, cx| {
                    root.handle_commit_intent(intent, window, cx);
                });
            });
        });
    }
}

/// 分组标题与行列表；折叠标题可点击切换。
pub(super) fn render_commit(view: &mut SideView, cx: &mut Context<'_, SideView>) -> AnyElement {
    let scroll_handle = view.commit_scroll_handle().cloned();
    let Some(runtime) = &view.commit else {
        return div().size_full().into_any_element();
    };
    let Some(scroll_handle) = scroll_handle else {
        return div().size_full().into_any_element();
    };
    let visible = runtime
        .visible_rows()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let collapsed = runtime.collapsed;
    let counts = CommitGroup::ALL.map(|group| {
        runtime
            .rows
            .iter()
            .filter(|row| {
                row.group == group && (group != CommitGroup::Ignored || runtime.show_ignored)
            })
            .filter(|row| runtime.matches_filter(row))
            .count()
    });
    let show_ignored = runtime.show_ignored;
    let filter_input = runtime.filter_input.clone();
    let loading = runtime.loading;
    let stale = runtime.stale;

    let mut list = div()
        .id("commit-list")
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .text_sm();

    if loading && runtime.rows.is_empty() {
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "commit-loading".to_owned())
                .child("加载中…"),
        );
    }
    if stale {
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "commit-stale".to_owned())
                .child("列表可能已过期"),
        );
    }

    for group in CommitGroup::ALL {
        if group == CommitGroup::Ignored && !show_ignored {
            continue;
        }
        let index = CommitRuntime::group_index(group);
        let is_collapsed = collapsed[index];
        let selector = format!("commit-group-{}", index);
        list = list.child(
            div()
                .id(("commit-group", index))
                .h(px(22.0))
                .px_2()
                .flex()
                .items_center()
                .text_color(cx.theme().foreground)
                .debug_selector(move || selector.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.commit_toggle_group(group, cx);
                    }),
                )
                .child(format!(
                    "{} {} ({})",
                    if is_collapsed { "▸" } else { "▾" },
                    group.title(),
                    counts[index]
                )),
        );
        if is_collapsed {
            continue;
        }
        for (row_index, row) in visible.iter().filter(|row| row.group == group).enumerate() {
            let path = row.path.clone();
            let relative = row.repo_relative.clone();
            let selector = format!("commit-row-{relative}");
            let mut label = format!("{} {} — {}", row.status_label, row.name, relative);
            if runtime.pending_permanent.as_ref() == Some(&row.path) {
                label.push_str("（回收站不可用，按 Delete 永久删除）");
            }
            list = list.child(
                div()
                    .id(("commit-row", index * 1024 + row_index))
                    .h(px(22.0))
                    .pl(px(16.0))
                    .text_color(cx.theme().muted_foreground)
                    .debug_selector(move || selector.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            if event.click_count >= 2 {
                                view.commit_activate_row(&path, window, cx);
                            } else {
                                view.commit_select_at(&path, cx);
                            }
                        }),
                    )
                    .child(label),
            );
        }
    }

    let mut header = div()
        .id("commit-header")
        .flex()
        .items_center()
        .gap(px(4.0))
        .px_2()
        .py_1();
    if let Some(input) = filter_input {
        header = header.child(
            div()
                .flex_grow(1.0)
                .min_w_0()
                .child(gpui_component::input::Input::new(&input)),
        );
    }
    header = header.child(
        div()
            .id("commit-show-ignored")
            .flex_none()
            .cursor_pointer()
            .debug_selector(|| "commit-show-ignored".to_owned())
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    let next = view
                        .commit
                        .as_ref()
                        .is_some_and(|runtime| !runtime.show_ignored);
                    view.commit_set_show_ignored(next, cx);
                }),
            )
            .child(if show_ignored {
                "ignored: on"
            } else {
                "ignored: off"
            }),
    );

    div()
        .id("commit-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(header)
        .child(list)
        .into_any_element()
}

impl AppRoot {
    /// Commit 意图在广播结束后执行；根变化后按同一状态读取链刷新。
    pub(super) fn handle_commit_intent(
        &mut self,
        intent: CommitIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match intent {
            CommitIntent::LoadStatus {
                project_id,
                root,
                generation,
            } => self.start_commit_status(project_id, root, generation, window, cx),
            CommitIntent::Rollback {
                project_id,
                root,
                row,
                kind,
            } => self.open_commit_confirmation(
                CommitWriteAction::Rollback {
                    project_id,
                    root,
                    row,
                    kind,
                },
                window,
                cx,
            ),
            CommitIntent::Delete {
                project_id,
                root,
                row,
                permanent,
            } => self.open_commit_confirmation(
                CommitWriteAction::Delete {
                    project_id,
                    root,
                    row,
                    permanent,
                },
                window,
                cx,
            ),
            CommitIntent::Open {
                project_id,
                root,
                row,
                kind,
            } => match kind {
                CommitOpenKind::Document => {
                    let target_group = match self.focused_group(project_id, cx) {
                        Ok(group) => group,
                        Err(_) => return,
                    };
                    let request =
                        match self.prepare_document_open(project_id, target_group, row.path, false)
                        {
                            Ok(request) => request,
                            Err(_) => return,
                        };
                    self.queue_document_open(request, None, window, cx);
                }
                CommitOpenKind::Diff => {
                    self.start_commit_diff(project_id, root, row.path, window, cx)
                }
                CommitOpenKind::GitShow => {
                    self.start_commit_gitshow(project_id, root, row, window, cx)
                }
            },
        }
    }

    fn start_commit_status(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitStatus) {
            if let Some(view) = self
                .side_views
                .instance(slterm_workspace::SideViewId::Commit)
                .cloned()
            {
                view.update(cx, |view, _| view.commit_defer_status(generation));
            }
            return;
        }
        let git = std::sync::Arc::clone(&self.git);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let project_root = root.clone();
        self.tasks
            .register_with(TaskId::CommitStatus, CancellationToken::new(), move || {
                let loading = executor.spawn(async move {
                    let grants = super::explorer::status_grants(&project_root)?;
                    let snapshot = git.status(&project_root, &grants, 0)?;
                    Ok::<_, crate::git::GitError>(snapshot)
                });
                cx.spawn(async move |this, cx| {
                    let result = loading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_commit_status(project_id, generation, result, window, cx)
                        });
                    });
                })
            })
            .expect("CommitStatus 单任务登记必须唯一");
    }

    fn finish_commit_status(
        &mut self,
        project_id: ProjectId,
        generation: u64,
        result: Result<std::sync::Arc<crate::git::StatusSnapshot>, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitStatus);
        let commit_view = self
            .side_views
            .instance(slterm_workspace::SideViewId::Commit)
            .cloned();
        let reload = commit_view
            .as_ref()
            .is_some_and(|view| view.update(cx, |view, _| view.commit_take_deferred_status()));
        // 回执只结算当前数据根；旧根结果丢弃后由合并位重投。
        let current = commit_view
            .as_ref()
            .and_then(|view| view.read(cx).commit_refresh_target());
        let matches = current
            .as_ref()
            .is_some_and(|(current_project, _, current_generation)| {
                *current_project == project_id && *current_generation == generation
            });
        if matches
            && let Some(view) = commit_view.as_ref()
            && let Some(project_root) = self
                .snapshot
                .registry()
                .project(project_id)
                .map(|project| project.root().clone())
        {
            // 数据根由视图保留的仓库根决定；回投只按当前视图根匹配。
            let repo_root = match &result {
                Ok(snapshot) => snapshot
                    .worktree_root
                    .as_ref()
                    .map(|root| root.canonical_path().clone())
                    .unwrap_or(project_root),
                Err(_) => project_root,
            };
            view.update(cx, |view, cx| {
                view.commit_apply_status(generation, repo_root, result.map_err(|_| ()), cx)
            });
        }
        if reload
            && let Some((project, root, generation)) = current
            && self.snapshot.registry().project(project).is_some()
        {
            self.start_commit_status(project, root, generation, window, cx);
        }
    }

    /// Diff 打开先在后台捕获仓库绑定，再复用共享 Document 打开链。
    fn start_commit_diff(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitDiff) {
            return;
        }
        let git = std::sync::Arc::clone(&self.git);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let binding_root = root.clone();
        self.tasks
            .register_with(TaskId::CommitDiff, CancellationToken::new(), move || {
                let loading = executor.spawn(async move {
                    let grants = super::explorer::status_grants(&binding_root)?;
                    let binding = git.repository_binding(&binding_root, &grants, 0)?;
                    Ok::<_, crate::git::GitError>(binding)
                });
                cx.spawn(async move |this, cx| {
                    let result = loading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_commit_diff(project_id, path, result, window, cx)
                        });
                    });
                })
            })
            .expect("CommitDiff 单任务登记必须唯一");
    }

    fn finish_commit_diff(
        &mut self,
        project_id: ProjectId,
        path: CanonicalPath,
        result: Result<slterm_document::RepositoryBinding, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitDiff);
        let Ok(binding) = result else {
            return;
        };
        if self
            .open_commit_diff_path(project_id, binding, path, window, cx)
            .is_err()
        {
            // 打开失败必须可见且可重试；行保持原样。
            super::events::notify_commit_diff_unavailable(cx);
        }
    }

    /// 以已捕获的仓库绑定打开共享 Document Diff；Document 未注册时排队等待打开完成。
    ///
    /// Commit 行激活与 Runtime `git.open_diff` 共用同一打开链与落点规则。
    pub(super) fn open_commit_diff_path(
        &mut self,
        project_id: ProjectId,
        binding: slterm_document::RepositoryBinding,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let repo_root = binding.worktree_root().canonical_path().clone();
        let repo_path =
            repo_path_from(&repo_root, &path).map_err(|_| AppRootError::UnexpectedParameters)?;
        let target_group = match self.focused_group(project_id, cx)? {
            Some(group) => group,
            // 空项目按规格创建首组，不退回隐藏项目。
            None => slterm_workspace::PanelGroupId::from_uuid(uuid::Uuid::now_v7())
                .expect("系统生成的 UUID 必须为 v7"),
        };
        if let Some(document_id) = self.document_id_at(&path) {
            self.open_diff(
                super::diff::DiffOpenRequest {
                    project_id,
                    target_group: Some(target_group),
                    document_id,
                    binding,
                    path: repo_path,
                    focus: true,
                    target_line: None,
                },
                window,
                cx,
            )?;
            return Ok(());
        }
        // Document 尚未注册：登记待打开 Diff，由打开完成链统一收尾。
        self.pending_commit_diffs.insert(
            path.clone(),
            PendingCommitDiff {
                project_id,
                target_group,
                binding,
                repo_path,
            },
        );
        let request = match self.prepare_document_open(project_id, Some(target_group), path, false)
        {
            Ok(request) => request,
            Err(error) => {
                self.pending_commit_diffs.clear();
                return Err(error);
            }
        };
        self.queue_document_open(request, None, window, cx);
        Ok(())
    }

    /// Document 打开成功后兑现同一路径的待打开 Diff；其它路径保持等待。
    pub(super) fn flush_pending_commit_diff(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(pending) = self.pending_commit_diffs.remove(path) else {
            return;
        };
        let Some(document_id) = self.document_id_at(path) else {
            return;
        };
        if self
            .open_diff(
                super::diff::DiffOpenRequest {
                    project_id: pending.project_id,
                    target_group: Some(pending.target_group),
                    document_id,
                    binding: pending.binding,
                    path: pending.repo_path,
                    focus: true,
                    target_line: None,
                },
                window,
                cx,
            )
            .is_err()
        {
            super::events::notify_commit_diff_unavailable(cx);
        }
    }

    /// 打开唯一确认面；已有确认面时只还焦，不叠加第二层模态。
    fn open_commit_confirmation(
        &mut self,
        action: CommitWriteAction,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.commit_confirmation.is_some() {
            if let Some(host) = &self.commit_confirmation {
                let token = host.registration.token();
                let _ = self.focus_manager.request_focus(token, window, cx);
            }
            return;
        }
        let _ = self.cancel_save_conflict(window, cx);
        let _ = self.cancel_encoding_prompt(window, cx);
        let dirty_documents = self
            .document_id_at(action.path())
            .filter(|id| {
                self.documents
                    .session(*id)
                    .is_some_and(|session| session.is_dirty())
            })
            .into_iter()
            .collect();
        let focus_handle = cx.focus_handle();
        let Ok(registration) = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        ) else {
            return;
        };
        if self
            .focus_manager
            .open_modal(
                self.focus_manager.workspace_token(),
                self.focus_manager.workspace_token(),
                registration.token(),
                window,
                cx,
            )
            .is_err()
        {
            let _ = self.focus_manager.unregister(registration.token());
            return;
        }
        self.commit_confirmation = Some(CommitConfirmation {
            action,
            dirty_documents,
            focus_handle,
            registration,
        });
        cx.notify();
    }

    /// 取消：只收合确认面，不触碰 Git 或文档。
    pub(super) fn cancel_commit_confirmation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.commit_confirmation.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 确认提交：保存／丢弃决定完成后才重新进入原 Git 写执行体。
    pub(super) fn resolve_commit_confirmation(
        &mut self,
        choice: CommitConfirmChoice,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.commit_confirmation.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        match choice {
            CommitConfirmChoice::Execute | CommitConfirmChoice::Discard => {
                self.dispatch_commit_write(host.action, window, cx);
                Ok(())
            }
            CommitConfirmChoice::Save => {
                if host.dirty_documents.is_empty() {
                    self.dispatch_commit_write(host.action, window, cx);
                    return Ok(());
                }
                let pending = host
                    .dirty_documents
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                self.commit_write_after_save = Some(CommitWriteAfterSave {
                    action: host.action,
                    pending: pending.clone(),
                    window_handle: window.window_handle(),
                    root: cx.entity().downgrade(),
                });
                for document_id in pending {
                    self.save_document(document_id, cx);
                }
                Ok(())
            }
        }
    }

    /// 无法取得保存回执时取消等待中的 Git 写，避免静默挂起。
    pub(super) fn abort_commit_write_after_save(&mut self, document_id: DocumentId) {
        if self
            .commit_write_after_save
            .as_ref()
            .is_some_and(|pending| pending.pending.contains(&document_id))
        {
            self.commit_write_after_save = None;
        }
    }

    /// 保存回投：全部成功才继续原 Git 写；任一失败保留现场并取消本次操作。
    pub(super) fn continue_commit_write_after_save(
        &mut self,
        document_id: DocumentId,
        success: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(pending) = self.commit_write_after_save.as_mut() else {
            return;
        };
        if !pending.pending.contains(&document_id) {
            return;
        }
        if !success {
            self.commit_write_after_save = None;
            return;
        }
        pending.pending.remove(&document_id);
        if !pending.pending.is_empty() {
            return;
        }
        let Some(pending) = self.commit_write_after_save.take() else {
            return;
        };
        let CommitWriteAfterSave {
            action,
            window_handle,
            root,
            ..
        } = pending;
        let _ = window_handle.update(cx, |_, window, cx| {
            let _ = root.update(cx, |root, cx| {
                root.dispatch_commit_write(action, window, cx);
            });
        });
    }

    /// 已确认的 Git 写复用既有后台执行体；状态与授权在后台重新复核。
    fn dispatch_commit_write(
        &mut self,
        action: CommitWriteAction,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match action {
            CommitWriteAction::Rollback {
                project_id,
                root,
                row,
                kind,
            } => self.start_commit_rollback(project_id, root, row, kind, window, cx),
            CommitWriteAction::Delete {
                project_id,
                root,
                row,
                permanent,
            } => self.start_commit_delete(project_id, root, row, permanent, window, cx),
        }
    }

    /// 删除先过脏 Document 守卫；staged added 先取消暂存，再回收站或永久删除。
    fn start_commit_delete(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        permanent: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitDelete) {
            return;
        }
        if self.snapshot.registry().active() != Some(project_id) {
            return;
        }
        // 写能力只来自项目根；仓库根兄弟位置仅可读。
        let Some(project_root) = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone())
        else {
            return;
        };
        if !row.path.as_path().starts_with(project_root.as_path()) {
            return;
        }
        // 删除/回滚已由确认面取得脏 Document 的显式处置，这里不再静默跳过。
        let service = std::sync::Arc::clone(&self.file_operations);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let repo_root = root.clone();
        let write_root = project_root.clone();
        let repo_relative = row.repo_relative.clone();
        let target_for_finish = row.path.clone();
        let kind = if row.index == 'A' {
            super::git_changes::DeleteChangeKind::StagedAdded
        } else {
            super::git_changes::DeleteChangeKind::Untracked
        };
        self.tasks
            .register_with(TaskId::CommitDelete, CancellationToken::new(), move || {
                let deleting = executor.spawn(async move {
                    // 唯一执行体：写前重检、取消暂存与回收站/永久删除都在同一处。
                    super::git_changes::delete_change(
                        &crate::git::GitFacade::default(),
                        &repo_root,
                        &write_root,
                        project_id,
                        &service,
                        &repo_relative,
                        kind,
                        permanent,
                        "commit",
                    )
                });
                cx.spawn(async move |this, cx| {
                    let result = deleting.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_commit_delete(
                                target_for_finish,
                                permanent,
                                result,
                                window,
                                cx,
                            )
                        });
                    });
                })
            })
            .expect("CommitDelete 单任务登记必须唯一");
    }

    fn finish_commit_delete(
        &mut self,
        path: CanonicalPath,
        permanent: bool,
        result: Result<(), super::git_changes::GitChangeError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitDelete);
        let retry_permanent = offers_permanent_retry(&result, permanent);
        let target = self
            .side_views
            .instance(slterm_workspace::SideViewId::Commit)
            .cloned();
        if let Some(target) = target {
            target.update(cx, |view, cx| {
                view.commit_set_pending_permanent(retry_permanent.then(|| path.clone()), cx);
            });
        }
        // 无论成功或失败都刷新真实现状；失败保留原行以支持显式永久确认。
        if let Some(project) = self.snapshot.registry().active() {
            let root = self
                .snapshot
                .registry()
                .project(project)
                .map(|project| project.root().clone());
            if let Some(root) = root {
                let generation = self
                    .side_views
                    .instance(slterm_workspace::SideViewId::Commit)
                    .and_then(|instance| instance.read(cx).commit_generation_for_refresh());
                if let Some(generation) = generation {
                    let _ = window;
                    self.handle_commit_intent(
                        CommitIntent::LoadStatus {
                            project_id: project,
                            root,
                            generation,
                        },
                        window,
                        cx,
                    );
                }
            }
        }
    }

    /// 回滚前先过脏 Document 守卫，再后台重检状态并只从索引恢复工作区。
    fn start_commit_rollback(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        kind: RollbackKind,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitRollback) {
            return;
        }
        if self.snapshot.registry().active() != Some(project_id) {
            return;
        }
        // 写能力只来自项目根；仓库根兄弟位置仅可读，不得借此扩大写授权。
        let Some(project_root) = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone())
        else {
            return;
        };
        if !row.path.as_path().starts_with(project_root.as_path()) {
            return;
        }
        // 删除/回滚已由确认面取得脏 Document 的显式处置，这里不再静默跳过。
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let repo_root = root.clone();
        let project_root = project_root.clone();
        let repo_relative = row.repo_relative.clone();
        let service = std::sync::Arc::clone(&self.file_operations);
        let kind = match kind {
            RollbackKind::Modified => super::git_changes::UnstagedChangeKind::Modified,
            RollbackKind::Deleted => super::git_changes::UnstagedChangeKind::Deleted,
        };
        self.tasks
            .register_with(
                TaskId::CommitRollback,
                CancellationToken::new(),
                move || {
                    let rolling_back = executor.spawn(async move {
                        // 唯一执行体：写前重检、镜像清理与写盘都在同一处。
                        super::git_changes::rollback_unstaged(
                            &crate::git::GitFacade::default(),
                            &repo_root,
                            &project_root,
                            project_id,
                            &service,
                            &repo_relative,
                            kind,
                            "commit",
                        )
                    });
                    cx.spawn(async move |this, cx| {
                        let result = rolling_back.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_commit_rollback(project_id, result, window, cx)
                            });
                        });
                    })
                },
            )
            .expect("CommitRollback 单任务登记必须唯一");
    }

    fn finish_commit_rollback(
        &mut self,
        project_id: ProjectId,
        result: Result<(), super::git_changes::GitChangeError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitRollback);
        let _ = (project_id, result);
        // 无论成功或失败都刷新真实现状；工作区重载由既有 watcher 外部变化链完成。
        if let Some(project) = self.snapshot.registry().active() {
            let root = self
                .snapshot
                .registry()
                .project(project)
                .map(|project| project.root().clone());
            if let Some(root) = root {
                let generation = self
                    .side_views
                    .instance(slterm_workspace::SideViewId::Commit)
                    .and_then(|instance| instance.read(cx).commit_generation_for_refresh());
                if let Some(generation) = generation {
                    let _ = window;
                    self.handle_commit_intent(
                        CommitIntent::LoadStatus {
                            project_id: project,
                            root,
                            generation,
                        },
                        window,
                        cx,
                    );
                }
            }
        }
    }

    fn start_commit_gitshow(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        row: CommitRow,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitGitShow) {
            return;
        }
        let git = std::sync::Arc::clone(&self.git);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let binding_root = root.clone();
        self.tasks
            .register_with(TaskId::CommitGitShow, CancellationToken::new(), move || {
                let loading = executor.spawn(async move {
                    let grants = super::explorer::status_grants(&binding_root)?;
                    let binding = git.repository_binding(&binding_root, &grants, 0)?;
                    Ok::<_, crate::git::GitError>(binding)
                });
                cx.spawn(async move |this, cx| {
                    let result = loading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_commit_gitshow(project_id, row.clone(), result, window, cx)
                        });
                    });
                })
            })
            .expect("CommitGitShow 单任务登记必须唯一");
    }

    fn finish_commit_gitshow(
        &mut self,
        project_id: ProjectId,
        row: CommitRow,
        result: Result<slterm_document::RepositoryBinding, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitGitShow);
        let Ok(binding) = result else {
            return;
        };
        let _ = self.open_commit_gitshow_path(project_id, binding, row.path, window, cx);
    }

    /// 以已捕获的仓库绑定打开固定版本；Commit 行激活与 Runtime `git.open_deleted` 共用。
    pub(super) fn open_commit_gitshow_path(
        &mut self,
        project_id: ProjectId,
        binding: slterm_document::RepositoryBinding,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let repo_root = binding.worktree_root().canonical_path().clone();
        let repo_path =
            repo_path_from(&repo_root, &path).map_err(|_| AppRootError::UnexpectedParameters)?;
        let target_group = match self.focused_group(project_id, cx)? {
            Some(group) => Some(group),
            None => Some(
                slterm_workspace::PanelGroupId::from_uuid(uuid::Uuid::now_v7())
                    .expect("系统生成的 UUID 必须为 v7"),
            ),
        };
        self.open_gitshow(
            super::git_show::GitShowOpenRequest {
                project_id,
                target_group,
                binding,
                path: repo_path,
                focus: true,
                activate_project: false,
            },
            window,
            cx,
        )?;
        Ok(())
    }

    /// Runtime Git 写完成后按真实状态刷新 Commit 视图；只刷新同一项目。
    pub(super) fn refresh_commit_view_for_project(
        &mut self,
        project_id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let target = self
            .side_views
            .instance(slterm_workspace::SideViewId::Commit)
            .and_then(|instance| instance.read(cx).commit_refresh_target());
        let Some((view_project, root, generation)) = target else {
            return;
        };
        if view_project != project_id {
            return;
        }
        let _ = window;
        self.handle_commit_intent(
            CommitIntent::LoadStatus {
                project_id,
                root,
                generation,
            },
            window,
            cx,
        );
    }

    fn document_id_at(&self, path: &CanonicalPath) -> Option<slterm_document::DocumentId> {
        self.snapshot
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == path).then_some(id))
    }
}

/// 仓库相对路径的规范转换；越界或非 Unicode 一律失败。
fn repo_path_from(
    repo_root: &CanonicalPath,
    path: &CanonicalPath,
) -> Result<slterm_document::RepoPath, ()> {
    let relative = path
        .as_path()
        .strip_prefix(repo_root.as_path())
        .map_err(|_| ())?;
    let text = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or(())?
        .join("/");
    text.parse().map_err(|_| ())
}

/// 只有「回收站不可用」可升级为永久删除；其他失败保留原因并如实显示。
pub(crate) fn offers_permanent_retry(
    result: &Result<(), super::git_changes::GitChangeError>,
    permanent: bool,
) -> bool {
    !permanent
        && matches!(
            result,
            Err(super::git_changes::GitChangeError::RecycleUnavailable)
        )
}

#[cfg(test)]
mod commit_rollback_tests {
    use super::*;
    use std::str::FromStr as _;

    fn row(
        index: char,
        worktree: char,
        conflict: bool,
        untracked: bool,
        ignored: bool,
    ) -> CommitRow {
        CommitRow {
            path: CanonicalPath::new("C:/repo/file.txt").unwrap(),
            repo_relative: "file.txt".to_owned(),
            name: "file.txt".to_owned(),
            group: if untracked {
                CommitGroup::Unversioned
            } else {
                CommitGroup::Changes
            },
            index,
            worktree,
            status_label: status_label(index, worktree, conflict),
            conflict,
            ignored,
            untracked,
            renamed_from: None,
        }
    }

    /// 在途状态读取期间出现的新数据根只保留合并位，回执后必须重投。
    #[test]
    fn commit_runtime_defers_reload_to_inflight_status_result() {
        let project = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut runtime = CommitRuntime::new(project);
        runtime.generation = 3;
        assert!(!runtime.take_deferred_status());
        runtime.defer_status();
        assert!(runtime.loading, "合并期间保持加载态");
        assert!(runtime.take_deferred_status());
        assert!(!runtime.take_deferred_status(), "合并位只能消费一次");
    }

    /// 回滚只覆盖精确的未暂存 modified/deleted；其余形态一律明确禁用。
    #[test]
    fn commit_rollback_disabled_shapes() {
        assert_eq!(
            rollback_availability(&row('.', 'M', false, false, false)),
            Ok(RollbackKind::Modified)
        );
        assert_eq!(
            rollback_availability(&row('.', 'D', false, false, false)),
            Ok(RollbackKind::Deleted)
        );
        assert_eq!(
            rollback_availability(&row('M', '.', false, false, false)),
            Err(RollbackDisabled::StagedOnly)
        );
        assert_eq!(
            rollback_availability(&row('D', '.', false, false, false)),
            Err(RollbackDisabled::StagedOnly)
        );
        assert_eq!(
            rollback_availability(&row('R', '.', false, false, false)),
            Err(RollbackDisabled::StagedRename)
        );
        assert_eq!(
            rollback_availability(&row('A', '.', false, false, false)),
            Err(RollbackDisabled::StagedOnly)
        );
        assert_eq!(
            rollback_availability(&row('U', 'U', true, false, false)),
            Err(RollbackDisabled::Conflict)
        );
        assert_eq!(
            rollback_availability(&row('?', '?', false, true, false)),
            Err(RollbackDisabled::NotTracked)
        );
        assert_eq!(
            rollback_availability(&row('!', '!', false, false, true)),
            Err(RollbackDisabled::NotTracked)
        );
    }
}
