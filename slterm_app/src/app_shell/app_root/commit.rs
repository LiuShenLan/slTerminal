//! Commit 视图运行态：仓库状态分组、打开映射与展示参数。
//!
//! 数据根固定为活跃项目所在仓库的 RepoRoot；行只投影状态事实，
//! 打开动作统一产生 `CommitIntent`，由 AppRoot 复用既有打开链。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Task, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_path::CanonicalPath;
use slterm_sidebar::SideViewState;
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::side_view::SideView;
use super::task_service::{CancellationToken, TaskId};

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
    pub status_label: String,
    pub conflict: bool,
    pub ignored: bool,
    pub untracked: bool,
    pub renamed_from: Option<String>,
}

/// 打开映射只区分三种落点；unknown 缺失文件不产生意图。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CommitOpenKind {
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

/// 打开映射：ignored 不打开；untracked/added 走普通文档；
/// deleted 走固定版本；modified/renamed/conflict 走共享 Diff。
fn open_kind(row: &CommitRow) -> Option<CommitOpenKind> {
    if row.ignored {
        return None;
    }
    if row.conflict || row.status_label.contains('M') || row.status_label.contains('R') {
        return Some(CommitOpenKind::Diff);
    }
    if row.status_label.contains('D') {
        return Some(CommitOpenKind::GitShow);
    }
    Some(CommitOpenKind::Document)
}

pub(super) struct CommitRuntime {
    project: ProjectId,
    root: Option<CanonicalPath>,
    rows: Vec<CommitRow>,
    filter: String,
    show_ignored: bool,
    collapsed: [bool; 3],
    scroll_handle: gpui::ScrollHandle,
    loading: bool,
    stale: bool,
    task: Option<Task<()>>,
}

impl CommitRuntime {
    fn new(project: ProjectId) -> Self {
        Self {
            project,
            root: None,
            rows: Vec::new(),
            filter: String::new(),
            show_ignored: false,
            collapsed: [false, false, false],
            scroll_handle: gpui::ScrollHandle::new(),
            loading: false,
            stale: false,
            task: None,
        }
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
            runtime.project == project && runtime.root.as_ref() == Some(&root)
        });
        if unchanged {
            return;
        }
        let previous = self.commit.take();
        let mut runtime = CommitRuntime::new(project);
        runtime.root = Some(root.clone());
        runtime.filter.clear();
        runtime.rows.clear();
        runtime.loading = true;
        if let Some(restored) = restored {
            runtime.show_ignored = restored.show_ignored();
            runtime.collapsed = [
                restored.changes_collapsed(),
                restored.unversioned_collapsed(),
                restored.ignored_collapsed(),
            ];
        }
        // 旧任务随旧运行态释放；迟到回执由视图代次拒绝。
        runtime.task = previous.and_then(|mut old| old.task.take());
        self.commit = Some(runtime);
        self.dispatch_commit_intent(
            CommitIntent::LoadStatus {
                project_id: project,
                root,
            },
            window,
            cx,
        );
        cx.notify();
    }

    pub(super) fn clear_commit_root(&mut self, cx: &mut Context<'_, Self>) {
        if self.commit.take().is_some() {
            cx.notify();
        }
    }

    /// 状态回执只替换列表；失败保留旧列表并标记可能过期。
    pub(super) fn commit_apply_status(
        &mut self,
        repo_root: CanonicalPath,
        snapshot: Result<std::sync::Arc<crate::git::StatusSnapshot>, ()>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.commit else {
            return;
        };
        if runtime.root.as_ref() != Some(&repo_root) {
            return;
        }
        runtime.loading = false;
        match snapshot {
            Ok(snapshot) => {
                let root = runtime.root.clone().expect("数据根必须存在");
                runtime.rows = build_rows(&root, &snapshot);
                runtime.stale = false;
            }
            Err(()) => {
                runtime.stale = !runtime.rows.is_empty();
            }
        }
        cx.notify();
    }

    pub(super) fn commit_scroll_handle(&self) -> Option<&gpui::ScrollHandle> {
        self.commit.as_ref().map(|runtime| &runtime.scroll_handle)
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
        let Some(root) = runtime.root.clone() else {
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
                        if let Some(runtime) = &mut view.commit {
                            runtime.collapsed[index] = !runtime.collapsed[index];
                            let collapsed = runtime.collapsed;
                            let show_ignored = runtime.show_ignored;
                            let _ = view.edit_state(|state| {
                                let SideViewState::Commit(state) = state else {
                                    return;
                                };
                                state.set_changes_collapsed(collapsed[0]);
                                state.set_unversioned_collapsed(collapsed[1]);
                                state.set_ignored_collapsed(collapsed[2]);
                                state.set_show_ignored(show_ignored);
                            });
                        }
                        cx.notify();
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
            let label = format!("{} {} — {}", row.status_label, row.name, relative);
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
                    if let Some(runtime) = &mut view.commit {
                        runtime.show_ignored = next;
                    }
                    let _ = view.edit_state(|state| {
                        let SideViewState::Commit(state) = state else {
                            return;
                        };
                        state.set_show_ignored(next);
                    });
                    cx.notify();
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
            CommitIntent::LoadStatus { project_id, root } => {
                self.start_commit_status(project_id, root, window, cx)
            }
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
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::CommitStatus) {
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
                            root.finish_commit_status(project_id, result, window, cx)
                        });
                    });
                })
            })
            .expect("CommitStatus 单任务登记必须唯一");
    }

    fn finish_commit_status(
        &mut self,
        project_id: ProjectId,
        result: Result<std::sync::Arc<crate::git::StatusSnapshot>, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::CommitStatus);
        let _ = window;
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let Some(explorer) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Commit)
            .cloned()
        else {
            return;
        };
        let root = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone());
        let Some(project_root) = root else {
            return;
        };
        // 数据根由视图保留的仓库根决定；回投只按当前视图根匹配。
        let repo_root = match &result {
            Ok(snapshot) => snapshot
                .worktree_root
                .as_ref()
                .map(|root| root.canonical_path().clone())
                .unwrap_or(project_root),
            Err(_) => project_root,
        };
        explorer.update(cx, |view, cx| {
            view.commit_apply_status(repo_root, result.map_err(|_| ()), cx)
        });
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
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let repo_root = binding.worktree_root().canonical_path().clone();
        let Ok(repo_path) = repo_path_from(&repo_root, &path) else {
            return;
        };
        let target_group = match self.focused_group(project_id, cx) {
            Ok(Some(group)) => group,
            _ => return,
        };
        if let Some(document_id) = self.document_id_at(&path) {
            let _ = self.open_diff(
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
            );
            return;
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
            Err(_) => {
                self.pending_commit_diffs.clear();
                return;
            }
        };
        self.queue_document_open(request, None, window, cx);
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
        let _ = self.open_diff(
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
        );
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
        let repo_root = binding.worktree_root().canonical_path().clone();
        let Ok(repo_path) = repo_path_from(&repo_root, &row.path) else {
            return;
        };
        let target_group = match self.focused_group(project_id, cx) {
            Ok(group) => group,
            Err(_) => return,
        };
        let _ = self.open_gitshow(
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
