//! Explorer 运行态：固定项目根、真实目录读取与打开意图。
//!
//! 树结构与持久状态分离：`FileTreeModel` 是唯一树事实，瞬态只在本模块；
//! 跨域动作只产生 `ExplorerIntent`，由 AppRoot 在广播结束后统一执行。

use std::collections::{BTreeSet, VecDeque};
use std::io;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Focusable as _, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    Task, WeakEntity, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest};
use slterm_sidebar::{
    DirectoryReadRequest, DirectoryStatus, ExplorerProjectState, FileTreeEntry, FileTreeEntryKind,
    FileTreeError, FileTreeModel, FileTreeRoot, SideViewPath, SideViewState,
};
use slterm_workspace::{ProjectId, ProjectRootStatus, SideViewId};

use super::AppRoot;
use super::side_view::SideView;
use super::task_service::{CancellationToken, TaskId};

/// Explorer 触发的跨域意图；只承载已解析的树行事实。
pub(super) enum ExplorerIntent {
    OpenDocument {
        project_id: ProjectId,
        path: CanonicalPath,
    },
    OpenTerminal {
        project_id: ProjectId,
        root: CanonicalPath,
        cwd: CanonicalPath,
    },
    RenameFile {
        project_id: ProjectId,
        root: CanonicalPath,
        from: CanonicalPath,
        to: CanonicalPath,
    },
    LoadStatus {
        project_id: ProjectId,
        root: CanonicalPath,
    },
}

/// 未确认的内联编辑只存在于运行态；视图销毁即丢弃，不产生文件副作用。
pub(super) struct InlineEdit {
    target: CanonicalPath,
    input: gpui::Entity<gpui_component::input::InputState>,
    _enter: gpui::Subscription,
}

/// Explorer 错误只区分根不可读与目录读取失败，不回显路径。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExplorerError {
    RootUnavailable,
    DirectoryRead,
}

/// 终端 cwd 后台预检只对调用方区分失败类别；不进入用户可见错误文本。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExplorerCwdError {
    Unavailable,
    Unauthorized,
}

/// 状态读取授权 = 项目根与只读父仓库根的并集；仓库根仅作读取。
pub(super) fn status_grants(root: &CanonicalPath) -> Result<Vec<PathGrant>, crate::git::GitError> {
    let mut grants = vec![PathGrant::project_root(root.clone())];
    let repo_root = crate::git::repository_root_cached(root.as_path())?;
    let repo_root = CanonicalPath::new(repo_root)
        .map_err(|_| crate::git::GitError::Command("仓库根不是规范绝对路径".to_owned()))?;
    if repo_root != *root {
        grants.push(PathGrant::repo_root(repo_root));
    }
    Ok(grants)
}

/// 在后台验证真实目录身份与 cwd 授权；不触碰快照或 launcher。
fn verify_terminal_cwd(
    resolver: &std::sync::Arc<dyn crate::platform::document_paths::DocumentPathResolver>,
    root: &CanonicalPath,
    cwd: &CanonicalPath,
) -> Result<CanonicalPath, ExplorerCwdError> {
    let resolved = resolver
        .resolve(cwd)
        .map_err(|_| ExplorerCwdError::Unavailable)?;
    if !resolved.canonical_path().as_path().is_dir() {
        return Err(ExplorerCwdError::Unavailable);
    }
    let request = PathRequest {
        operation: PathOperation::TerminalCwd,
        lexical_path: cwd.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    let grant = PathGrant::project_root(root.clone());
    let authorization = PathPolicy::authorize(&request, &resolved, std::slice::from_ref(&grant), 0)
        .map_err(|_| ExplorerCwdError::Unauthorized)?;
    Ok(authorization.resolved_path().canonical_path().clone())
}

/// 单视图运行态；目录读取串行，旧根回执由模型代次拒绝。
pub(super) struct ExplorerRuntime {
    project: ProjectId,
    tree: FileTreeModel,
    pending_reads: VecDeque<DirectoryReadRequest>,
    pending_expanded: BTreeSet<CanonicalPath>,
    restoring_row: Option<SideViewPath>,
    /// 待根批次回投后应用的持久滚动偏移。
    restoring_scroll: Option<f64>,
    inline_edit: Option<InlineEdit>,
    filter: String,
    show_ignored: bool,
    multi_selection: BTreeSet<CanonicalPath>,
    scroll_handle: gpui::ScrollHandle,
    /// 发现到的父仓库根与状态条目数；只作展示，不参与树根。
    repo_root: Option<CanonicalPath>,
    status_entries: usize,
    filter_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    _filter_change: Option<gpui::Subscription>,
    reading: bool,
    /// 用户动作产生的持久变更；恢复本身不写盘。
    dirty: bool,
    task: Option<Task<()>>,
    error: Option<ExplorerError>,
}

impl ExplorerRuntime {
    fn new(
        project: ProjectId,
        root: CanonicalPath,
        restored: Option<&ExplorerProjectState>,
    ) -> Self {
        let mut runtime = Self {
            project,
            tree: FileTreeModel::new(FileTreeRoot::Project(project), root),
            pending_reads: VecDeque::new(),
            pending_expanded: BTreeSet::new(),
            restoring_row: None,
            restoring_scroll: None,
            inline_edit: None,
            filter: String::new(),
            show_ignored: false,
            multi_selection: BTreeSet::new(),
            scroll_handle: gpui::ScrollHandle::new(),
            repo_root: None,
            status_entries: 0,
            filter_input: None,
            _filter_change: None,
            reading: false,
            dirty: false,
            task: None,
            error: None,
        };
        runtime.restore(restored);
        runtime
    }

    fn matches(&self, project: ProjectId, root: &CanonicalPath) -> bool {
        self.project == project && self.tree.root_path() == root
    }

    fn reset(
        &mut self,
        project: ProjectId,
        root: CanonicalPath,
        restored: Option<&ExplorerProjectState>,
    ) {
        self.project = project;
        self.tree.replace_root(FileTreeRoot::Project(project), root);
        self.pending_reads.clear();
        self.pending_expanded.clear();
        self.restoring_row = None;
        self.restoring_scroll = None;
        self.inline_edit = None;
        self.filter.clear();
        self.multi_selection.clear();
        self.show_ignored = false;
        self.scroll_handle = gpui::ScrollHandle::new();
        self.repo_root = None;
        self.status_entries = 0;
        self.filter_input = None;
        self._filter_change = None;
        self.dirty = false;
        self.error = None;
        self.restore(restored);
    }

    /// 恢复只登记待展开目录、待选行与滚动；级联展开在真实批次回投后推进。
    fn restore(&mut self, restored: Option<&ExplorerProjectState>) {
        let Some(restored) = restored else {
            return;
        };
        let root = self.tree.root_path().clone();
        self.pending_expanded = restored
            .expanded()
            .filter_map(|path| join_relative(&root, path.as_str()))
            .collect();
        self.restoring_row = restored.current_row().cloned();
        self.show_ignored = restored.show_ignored();
        self.restoring_scroll = Some(restored.scroll());
    }
}

/// 读取单层目录；非目录/文件条目（链接、设备等）不进入树。
pub(super) fn read_directory(directory: &CanonicalPath) -> io::Result<Vec<FileTreeEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(directory.as_path())? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let kind = if file_type.is_dir() {
            FileTreeEntryKind::Directory
        } else if file_type.is_file() {
            FileTreeEntryKind::File
        } else {
            // 链接与重解析点不跟随，避免越过树根或形成环。
            continue;
        };
        let Ok(path) = CanonicalPath::new(entry.path()) else {
            continue;
        };
        entries.push(FileTreeEntry::new(path, kind));
    }
    Ok(entries)
}

pub(super) fn join_relative(root: &CanonicalPath, relative: &str) -> Option<CanonicalPath> {
    let path = root
        .as_path()
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    CanonicalPath::new(path).ok()
}

/// 根相对正斜杠文本；非 Unicode 组件返回 None（不持久伪造路径）。
pub(super) fn relative_text(root: &CanonicalPath, path: &CanonicalPath) -> Option<String> {
    let relative = path.as_path().strip_prefix(root.as_path()).ok()?;
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

pub(super) fn persistent_path(root: &CanonicalPath, path: &CanonicalPath) -> Option<SideViewPath> {
    SideViewPath::new(relative_text(root, path)?).ok()
}

impl AppRoot {
    /// 实例生命周期或活跃项目根变化后同步 Explorer 根；根未变时零副作用。
    pub(super) fn sync_side_view_roots(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let active = self.snapshot.registry().active();
        let target = active.and_then(|project_id| {
            let project = self.snapshot.registry().project(project_id)?;
            (project.root_status() == ProjectRootStatus::Ready)
                .then(|| (project_id, project.root().clone()))
        });
        let weak = cx.entity().downgrade();
        let instances = SideViewId::ALL
            .into_iter()
            .filter_map(|id| self.side_views.instance(id).map(|view| (id, view.clone())))
            .collect::<Vec<_>>();
        for (id, instance) in instances {
            instance.update(cx, |view, cx| {
                view.set_root_handle(weak.clone());
                match id {
                    SideViewId::Explorer => match &target {
                        Some((project_id, root)) => {
                            view.sync_explorer_root(*project_id, root.clone(), window, cx)
                        }
                        None => view.clear_explorer_root(cx),
                    },
                    SideViewId::Commit => match &target {
                        Some((project_id, root)) => {
                            view.sync_commit_root(*project_id, root.clone(), window, cx)
                        }
                        None => view.clear_commit_root(cx),
                    },
                    SideViewId::Nav | SideViewId::AgentFiles => {}
                }
            });
        }
        self.ensure_agent_roots_loading(cx);
        self.sync_agent_files_roots(window, cx);
        self.sync_nav_projects(cx);
        self.side_view_roots_generation = Some(self.snapshot_generation);
    }

    /// Explorer 意图在广播结束后执行；打开失败只保持现状，不伪造面板。
    pub(super) fn handle_explorer_intent(
        &mut self,
        intent: ExplorerIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match intent {
            ExplorerIntent::OpenDocument { project_id, path } => {
                let target_group = match self.focused_group(project_id, cx) {
                    Ok(group) => group,
                    Err(_) => return,
                };
                let request =
                    match self.prepare_document_open(project_id, target_group, path, false) {
                        Ok(request) => request,
                        Err(_) => return,
                    };
                self.queue_document_open(request, None, window, cx);
            }
            ExplorerIntent::OpenTerminal {
                project_id,
                root,
                cwd,
            } => self.start_explorer_terminal_cwd(project_id, root, cwd, window, cx),
            ExplorerIntent::RenameFile {
                project_id,
                root,
                from,
                to,
            } => self.start_explorer_rename(project_id, root, from, to, window, cx),
            ExplorerIntent::LoadStatus { project_id, root } => {
                self.start_explorer_status(project_id, root, window, cx)
            }
        }
    }

    /// 状态读取先发现父仓库根，再以 ProjectRoot 与只读 RepoRoot 并集授权。
    fn start_explorer_status(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerStatus) {
            return;
        }
        let git = std::sync::Arc::clone(&self.git);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let project_root = root.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerStatus,
                CancellationToken::new(),
                move || {
                    let loading = executor.spawn(async move {
                        let grants = status_grants(&project_root)?;
                        let snapshot = git.status(&project_root, &grants, 0)?;
                        Ok::<_, crate::git::GitError>(snapshot)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = loading.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_status(project_id, result, window, cx);
                            });
                        });
                    })
                },
            )
            .expect("ExplorerStatus 单任务登记必须唯一");
    }

    fn finish_explorer_status(
        &mut self,
        project_id: ProjectId,
        result: Result<std::sync::Arc<crate::git::StatusSnapshot>, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerStatus);
        let _ = window;
        let Ok(snapshot) = result else {
            return;
        };
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let repo_root = snapshot
            .worktree_root
            .as_ref()
            .map(|root| root.canonical_path().clone());
        let entries = snapshot.entries.len();
        if let Some(explorer) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Explorer)
            .cloned()
        {
            explorer.update(cx, |view, cx| {
                view.explorer_set_status(repo_root, entries, cx)
            });
        }
    }

    /// 重命名在后台重新解析、授权并执行；成功后只重读原父目录。
    fn start_explorer_rename(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        from: CanonicalPath,
        to: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerFileOperation) {
            return;
        }
        let service = std::sync::Arc::clone(&self.file_operations);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task_from = from.clone();
        let task_to = to.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerFileOperation,
                CancellationToken::new(),
                move || {
                    let executing = executor.spawn(async move {
                        let grants = [PathGrant::project_root(root.clone())];
                        let plan = service.plan_rename(
                            FileTreeRoot::Project(project_id),
                            &task_from,
                            &task_to,
                            &grants,
                            0,
                        )?;
                        service.execute(&plan, &grants, 0)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = executing.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_rename(
                                    project_id, from, to, result, window, cx,
                                );
                            });
                        });
                    })
                },
            )
            .expect("ExplorerFileOperation 单任务登记必须唯一");
    }

    fn finish_explorer_rename(
        &mut self,
        project_id: ProjectId,
        from: CanonicalPath,
        to: CanonicalPath,
        result: Result<
            crate::app_shell::file_operation_service::FileOperationOutcome,
            crate::app_shell::file_operation_service::FileOperationError,
        >,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerFileOperation);
        let _ = window;
        if result.is_err() {
            return;
        }
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let Some(parent) = from.as_path().parent().map(|parent| parent.to_path_buf()) else {
            return;
        };
        let Ok(parent) = CanonicalPath::new(parent) else {
            return;
        };
        if let Some(explorer) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Explorer)
            .cloned()
        {
            explorer.update(cx, |view, cx| view.explorer_refresh_directory(&parent, cx));
        }
        let _ = to;
    }

    /// cwd 预检只在后台解析真实目录与授权，回前台再复核项目身份。
    fn start_explorer_terminal_cwd(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        cwd: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerTerminalCwd) {
            return;
        }
        let resolver = std::sync::Arc::clone(&self.path_resolver);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let requested_root = root.clone();
        let verify_root = root.clone();
        let verify_cwd = cwd.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerTerminalCwd,
                CancellationToken::new(),
                move || {
                    let verifying = executor.spawn(async move {
                        verify_terminal_cwd(&resolver, &verify_root, &verify_cwd)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = verifying.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_terminal_cwd(
                                    project_id,
                                    requested_root,
                                    result,
                                    window,
                                    cx,
                                );
                            });
                        });
                    })
                },
            )
            .expect("ExplorerTerminalCwd 单任务登记必须唯一");
    }

    fn finish_explorer_terminal_cwd(
        &mut self,
        project_id: ProjectId,
        requested_root: CanonicalPath,
        result: Result<CanonicalPath, ExplorerCwdError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerTerminalCwd);
        let Ok(cwd) = result else {
            return;
        };
        // 解析期间项目关闭或根重定位时授权不再成立。
        let Some(project) = self.snapshot.registry().project(project_id) else {
            return;
        };
        if project.root() != &requested_root
            || self.snapshot.registry().active() != Some(project_id)
        {
            return;
        }
        let _ = self.new_terminal_from_request(
            self.shell.clone(),
            Some(requested_root),
            Some(cwd),
            window,
            cx,
        );
    }
}

impl SideView {
    /// AppRoot 在实例创建后注入弱引用；迟到的根变化由 defer 串行执行。
    pub(super) fn set_root_handle(&mut self, root: WeakEntity<AppRoot>) {
        self.root_handle = Some(root);
    }

    /// 活跃项目根变化时重建树；持久分区来自唯一 SideViewState。
    pub(super) fn sync_explorer_root(
        &mut self,
        project: ProjectId,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let restored = match &self.state {
            SideViewState::Explorer(state) => state.project(project).cloned(),
            _ => None,
        };
        let unchanged = self
            .explorer
            .as_ref()
            .is_some_and(|runtime| runtime.matches(project, &root));
        if unchanged {
            self.ensure_explorer_filter_input(window, cx);
            return;
        }
        let status_root = root.clone();
        self.checkpoint_explorer_state();
        match &mut self.explorer {
            Some(runtime) => runtime.reset(project, root, restored.as_ref()),
            None => {
                self.explorer = Some(ExplorerRuntime::new(project, root, restored.as_ref()));
            }
        }
        self.ensure_explorer_filter_input(window, cx);
        self.request_explorer_root(cx);
        self.dispatch_explorer_intent(
            ExplorerIntent::LoadStatus {
                project_id: project,
                root: status_root,
            },
            window,
            cx,
        );
        cx.notify();
    }

    /// 过滤输入实体随视图生命周期存在；关闭重开即清空草稿。
    fn ensure_explorer_filter_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &self.explorer else {
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
                    view.explorer_set_filter(&text, cx);
                }
            },
        );
        if let Some(runtime) = &mut self.explorer {
            runtime.filter_input = Some(input);
            runtime._filter_change = Some(change);
        }
    }

    /// 无活跃项目时清空树，不保留上一项目的可操作行。
    pub(super) fn clear_explorer_root(&mut self, cx: &mut Context<'_, Self>) {
        if self.explorer.is_none() {
            return;
        }
        self.checkpoint_explorer_state();
        self.explorer = None;
        cx.notify();
    }

    /// 把当前树快照写回本视图持久分区；仅用户动作产生写盘。
    pub(super) fn checkpoint_explorer_state(&mut self) {
        let Some(runtime) = &self.explorer else {
            return;
        };
        if !runtime.dirty {
            return;
        }
        let project = runtime.project;
        let root = runtime.tree.root_path().clone();
        let expanded = runtime
            .tree
            .expanded_paths()
            .iter()
            .filter_map(|path| persistent_path(&root, path))
            .collect::<Vec<_>>();
        let current_row = runtime
            .tree
            .selected()
            .and_then(|path| persistent_path(&root, path));
        // GPUI 滚动偏移为负值；持久语义用向下滚动的正像素数。
        let scroll = f64::from(-runtime.scroll_handle.offset().y.as_f32()).max(0.0);
        let show_ignored = runtime.show_ignored;
        let _ = self.edit_state(|state| {
            let SideViewState::Explorer(explorer) = state else {
                return;
            };
            let project = explorer.project_mut(project);
            project.set_expanded(expanded);
            project.set_current_row(current_row);
            project.set_scroll(scroll);
            project.set_show_ignored(show_ignored);
        });
        if let Some(runtime) = &mut self.explorer {
            runtime.dirty = false;
        }
    }

    fn request_explorer_root(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let root = runtime.tree.root_path().clone();
        if let Some(request) = runtime.tree.request_directory(&root) {
            runtime.pending_reads.push_back(request);
        }
        self.start_explorer_reads(cx);
    }

    /// 单任务串行读取；任务句柄归本实体，Drop 即取消。
    fn start_explorer_reads(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.reading {
            return;
        }
        let Some(request) = runtime.pending_reads.pop_front() else {
            return;
        };
        runtime.reading = true;
        let directory = request.directory.clone();
        let executor = cx.background_executor().clone();
        let this = cx.entity().downgrade();
        let task = cx.spawn(async move |_, cx| {
            let result = executor
                .spawn(async move { read_directory(&directory) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.finish_explorer_read(request, result, cx)
            });
        });
        if let Some(runtime) = &mut self.explorer {
            runtime.task = Some(task);
        }
    }

    pub(super) fn finish_explorer_read(
        &mut self,
        request: DirectoryReadRequest,
        result: io::Result<Vec<FileTreeEntry>>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.reading = false;
        match result {
            Ok(entries) => match runtime.tree.apply_batch(&request, entries, true) {
                Ok(_) => {
                    runtime.error = None;
                }
                Err(FileTreeError::StaleRoot | FileTreeError::StaleRead) => {}
                Err(_) => {
                    runtime.error = Some(ExplorerError::DirectoryRead);
                }
            },
            Err(_) => {
                // 根读取失败与子目录失败同样只标记，不伪装为空目录。
                if runtime.tree.fail_directory(&request).is_ok() {
                    runtime.error = Some(if request.directory == *runtime.tree.root_path() {
                        ExplorerError::RootUnavailable
                    } else {
                        ExplorerError::DirectoryRead
                    });
                }
            }
        }
        self.restore_explorer_pending(cx);
        self.start_explorer_reads(cx);
        cx.notify();
    }

    /// 级联恢复待展开目录与待选行；不可达路径保持待定直到根或父批次回投。
    fn restore_explorer_pending(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let candidates = runtime.pending_expanded.iter().cloned().collect::<Vec<_>>();
        for path in candidates {
            let Some(kind) = runtime.tree.kind_of(&path) else {
                continue;
            };
            if kind != FileTreeEntryKind::Directory {
                runtime.pending_expanded.remove(&path);
                continue;
            }
            match runtime.tree.directory_status(&path) {
                DirectoryStatus::Loaded | DirectoryStatus::Loading => {
                    runtime.pending_expanded.remove(&path);
                }
                _ => match runtime.tree.expand(&path) {
                    Ok(request) => {
                        runtime.pending_expanded.remove(&path);
                        if let Some(request) = request {
                            runtime.pending_reads.push_back(request);
                        }
                    }
                    Err(_) => {
                        runtime.pending_expanded.remove(&path);
                    }
                },
            }
        }
        if let Some(row) = runtime.restoring_row.clone()
            && let Some(path) = join_relative(runtime.tree.root_path(), row.as_str())
            && runtime
                .tree
                .visible_rows()
                .iter()
                .any(|visible| visible.path == path)
        {
            let _ = runtime.tree.select(&path);
            runtime.restoring_row = None;
        }
        // 根批次已回投后才应用滚动，避免被空内容夹紧。
        if runtime.tree.directory_status(runtime.tree.root_path()) == DirectoryStatus::Loaded
            && let Some(scroll) = runtime.restoring_scroll.take()
            && scroll > 0.0
        {
            runtime
                .scroll_handle
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-(scroll as f32))));
        }
        self.start_explorer_reads(cx);
    }

    /// 单击选中：清空多选并记录当前行；不产生跨域意图。
    pub(super) fn explorer_select_at(&mut self, path: &CanonicalPath, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.tree.select(path).is_err() {
            return;
        }
        runtime.multi_selection.clear();
        runtime.dirty = true;
        cx.notify();
    }

    /// Ctrl 点击：在当前行之外切换多选集合，瞬态不写盘。
    pub(super) fn explorer_toggle_multi_selection(
        &mut self,
        path: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.tree.kind_of(path).is_none() {
            return;
        }
        if !runtime.multi_selection.remove(path) {
            runtime.multi_selection.insert(path.clone());
        }
        cx.notify();
    }

    /// 过滤只作用于已加载投影，瞬态不进入 ViewState。
    pub(super) fn explorer_set_filter(&mut self, text: &str, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.filter == text {
            return;
        }
        runtime.filter = text.to_owned();
        runtime.tree.set_filter(text);
        cx.notify();
    }

    /// 忽略开关是持久展示偏好；真实过滤归后续 Git 状态投影。
    pub(super) fn explorer_set_show_ignored(&mut self, show: bool, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.show_ignored == show {
            return;
        }
        runtime.show_ignored = show;
        runtime.dirty = true;
        cx.notify();
    }

    pub(super) fn explorer_scroll_handle(&self) -> Option<&gpui::ScrollHandle> {
        self.explorer.as_ref().map(|runtime| &runtime.scroll_handle)
    }

    /// 状态回执只更新展示事实；树根与已加载行保持原样。
    pub(super) fn explorer_set_status(
        &mut self,
        repo_root: Option<CanonicalPath>,
        entries: usize,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.repo_root = repo_root;
        runtime.status_entries = entries;
        cx.notify();
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_repo_root(&self) -> Option<&CanonicalPath> {
        self.explorer
            .as_ref()
            .and_then(|runtime| runtime.repo_root.as_ref())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_tree_root(&self) -> Option<&CanonicalPath> {
        self.explorer
            .as_ref()
            .map(|runtime| runtime.tree.root_path())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_visible_paths(&self) -> Vec<CanonicalPath> {
        self.explorer
            .as_ref()
            .map(|runtime| {
                runtime
                    .tree
                    .visible_rows()
                    .into_iter()
                    .map(|row| row.path)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_status_entries(&self) -> usize {
        self.explorer
            .as_ref()
            .map(|runtime| runtime.status_entries)
            .unwrap_or_default()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_is_expanded(&self, path: &CanonicalPath) -> bool {
        self.explorer
            .as_ref()
            .is_some_and(|runtime| runtime.tree.is_expanded(path))
    }

    pub(super) fn explorer_selected(&self) -> Option<&CanonicalPath> {
        self.explorer
            .as_ref()
            .and_then(|runtime| runtime.tree.selected())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_filter(&self) -> &str {
        self.explorer
            .as_ref()
            .map(|runtime| runtime.filter.as_str())
            .unwrap_or_default()
    }

    pub(super) fn explorer_show_ignored(&self) -> bool {
        self.explorer
            .as_ref()
            .is_some_and(|runtime| runtime.show_ignored)
    }

    pub(super) fn explorer_multi_selection(&self) -> Option<&BTreeSet<CanonicalPath>> {
        self.explorer
            .as_ref()
            .map(|runtime| &runtime.multi_selection)
    }

    /// 单击或键盘激活树行：目录只改展开，文件只产生打开意图。
    pub(super) fn explorer_activate_at(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(kind) = runtime.tree.kind_of(path) else {
            return;
        };
        runtime.dirty = true;
        match kind {
            FileTreeEntryKind::File => {
                let _ = runtime.tree.select(path);
                let intent = ExplorerIntent::OpenDocument {
                    project_id: runtime.project,
                    path: path.clone(),
                };
                self.dispatch_explorer_intent(intent, window, cx);
            }
            FileTreeEntryKind::Directory => {
                let expanded = runtime.tree.is_expanded(path);
                let outcome = if expanded {
                    runtime.tree.collapse(path).map(|_| None)
                } else {
                    runtime.tree.expand(path)
                };
                if let Ok(Some(request)) = outcome {
                    runtime.pending_reads.push_back(request);
                }
                self.start_explorer_reads(cx);
            }
        }
        cx.notify();
    }

    /// 开始内联重命名；草稿输入实体由运行态独占，销毁即丢弃。
    pub(super) fn explorer_begin_inline_edit(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.tree.kind_of(path).is_none() {
            return;
        }
        if runtime
            .inline_edit
            .as_ref()
            .is_some_and(|edit| &edit.target == path)
        {
            return;
        }
        let name = path
            .as_path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut state = gpui_component::input::InputState::new(window, cx);
            state.set_value(name, window, cx);
            state
        });
        let enter = cx.subscribe_in(
            &input,
            window,
            |view, _, event: &gpui_component::input::InputEvent, window, cx| {
                if matches!(event, gpui_component::input::InputEvent::PressEnter { .. }) {
                    view.explorer_commit_inline_edit(window, cx);
                }
            },
        );
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        runtime.inline_edit = Some(InlineEdit {
            target: path.clone(),
            input,
            _enter: enter,
        });
        cx.notify();
    }

    /// 渲染或测试读取当前草稿输入实体；未编辑时为 None。
    pub(super) fn explorer_inline_edit_input(
        &self,
    ) -> Option<gpui::Entity<gpui_component::input::InputState>> {
        self.explorer
            .as_ref()
            .and_then(|runtime| runtime.inline_edit.as_ref())
            .map(|edit| edit.input.clone())
    }

    /// Enter 入口：激活当前选中行（文件打开、目录切换展开）。
    pub(super) fn explorer_activate_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(path) = self.explorer_selected().cloned() else {
            return;
        };
        self.explorer_activate_at(&path, window, cx);
    }

    /// F2 入口：对当前选中行开始内联重命名。
    pub(super) fn explorer_begin_rename_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.explorer else {
            return;
        };
        let Some(path) = runtime.tree.selected().cloned() else {
            return;
        };
        self.explorer_begin_inline_edit(&path, window, cx);
    }

    /// 显式提交才产生文件操作意图；空名、含分隔符或未改变一律视为取消。
    pub(super) fn explorer_commit_inline_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(edit) = runtime.inline_edit.take() else {
            return;
        };
        let name = edit.input.read(cx).value().to_string();
        let name = name.trim();
        if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
            cx.notify();
            return;
        }
        let Some(parent) = edit.target.as_path().parent() else {
            cx.notify();
            return;
        };
        let Ok(to) = CanonicalPath::new(parent.join(name)) else {
            cx.notify();
            return;
        };
        if to == edit.target {
            cx.notify();
            return;
        }
        let intent = ExplorerIntent::RenameFile {
            project_id: runtime.project,
            root: runtime.tree.root_path().clone(),
            from: edit.target.clone(),
            to,
        };
        self.dispatch_explorer_intent(intent, window, cx);
        cx.notify();
    }

    /// 显式取消或失焦放弃；不触碰任何持久事实。
    pub(super) fn explorer_cancel_inline_edit(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if runtime.inline_edit.take().is_some() {
            cx.notify();
        }
    }

    /// 文件操作成功后重读目录；根外与未加载目录不扩读。
    pub(super) fn explorer_refresh_directory(
        &mut self,
        directory: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        if !directory
            .as_path()
            .starts_with(runtime.tree.root_path().as_path())
        {
            return;
        }
        if runtime.tree.kind_of(directory) != Some(FileTreeEntryKind::Directory) {
            return;
        }
        match runtime.tree.refresh(directory) {
            Ok(Some(request)) => runtime.pending_reads.push_back(request),
            Ok(None) => {}
            Err(_) => return,
        }
        self.start_explorer_reads(cx);
        cx.notify();
    }

    /// 在终端打开目录（文件行取父目录）；真实位置与授权由 AppRoot 后台预检。
    pub(super) fn explorer_open_terminal_at(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(kind) = runtime.tree.kind_of(path) else {
            return;
        };
        let cwd = match kind {
            FileTreeEntryKind::Directory => path.clone(),
            FileTreeEntryKind::File => {
                let Some(parent) = path.as_path().parent() else {
                    return;
                };
                let Ok(parent) = CanonicalPath::new(parent.to_path_buf()) else {
                    return;
                };
                parent
            }
        };
        let intent = ExplorerIntent::OpenTerminal {
            project_id: runtime.project,
            root: runtime.tree.root_path().clone(),
            cwd,
        };
        self.dispatch_explorer_intent(intent, window, cx);
    }

    /// 广播结束后再交给 AppRoot，避免快照 reconcile 重入当前实体。
    fn dispatch_explorer_intent(
        &mut self,
        intent: ExplorerIntent,
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
                    root.handle_explorer_intent(intent, window, cx);
                });
            });
        });
    }
}

/// 文件树画面只读运行态快照；点击行复用同一激活入口。
pub(super) fn render_explorer(view: &mut SideView, cx: &mut Context<'_, SideView>) -> AnyElement {
    let inline_input = view.explorer_inline_edit_input();
    let scroll_handle = view.explorer_scroll_handle().cloned();
    let multi_selection = view.explorer_multi_selection().cloned().unwrap_or_default();
    let Some(runtime) = &view.explorer else {
        return div().size_full().into_any_element();
    };
    let Some(scroll_handle) = scroll_handle else {
        return div().size_full().into_any_element();
    };
    let root = runtime.tree.root_path().clone();
    let rows = runtime.tree.visible_rows();
    let error = runtime.error;
    let inline_target = runtime.inline_edit.as_ref().map(|edit| edit.target.clone());
    let filter_input = runtime.filter_input.clone();
    let show_ignored = runtime.show_ignored;
    let status_label = runtime
        .repo_root
        .as_ref()
        .map(|_| format!("changes: {}", runtime.status_entries));
    let expanded = rows
        .iter()
        .filter(|row| runtime.tree.is_expanded(&row.path))
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    let mut list = div()
        .id("explorer-tree")
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .text_sm();
    if let Some(error) = error {
        let message = match error {
            ExplorerError::RootUnavailable => "无法读取项目根目录",
            ExplorerError::DirectoryRead => "无法读取目录",
        };
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "explorer-error".to_owned())
                .child(message),
        );
    }
    for (index, row) in rows.into_iter().enumerate() {
        let selected = row.selected;
        let is_directory = row.kind == FileTreeEntryKind::Directory;
        let open = expanded.contains(&row.path);
        let relative = relative_text(&root, &row.path).unwrap_or_default();
        let label = if is_directory {
            format!("{} {}", if open { "▾" } else { "▸" }, row.name)
        } else {
            row.name.clone()
        };
        let indent = 8.0 + row.depth as f32 * 12.0;
        let selector = format!("explorer-row-{relative}");
        let editing = inline_target.as_ref() == Some(&row.path);
        let multi = multi_selection.contains(&row.path);
        let path = row.path;
        let text_color = if selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let selection = cx.theme().list_active;
        let editing_input = editing.then(|| inline_input.clone()).flatten();
        let mut row_element = div()
            .id(("explorer-row", index))
            .h(px(22.0))
            .pl(px(indent))
            .when(selected || multi, |element| element.bg(selection))
            .text_color(text_color)
            .debug_selector(move || selector.clone())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    if event.click_count >= 2 {
                        if is_directory {
                            view.explorer_open_terminal_at(&path, window, cx);
                        } else {
                            view.explorer_activate_at(&path, window, cx);
                        }
                    } else if event.modifiers.control {
                        view.explorer_toggle_multi_selection(&path, cx);
                    } else if is_directory {
                        view.explorer_select_at(&path, cx);
                        view.explorer_activate_at(&path, window, cx);
                    } else {
                        view.explorer_select_at(&path, cx);
                    }
                }),
            );
        row_element = match editing_input {
            Some(input) => row_element.child(gpui_component::input::Input::new(&input)),
            None => row_element.child(label),
        };
        list = list.child(row_element);
    }

    let mut header = div()
        .id("explorer-header")
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
    if let Some(label) = status_label {
        header = header.child(
            div()
                .flex_none()
                .debug_selector(|| "explorer-status".to_owned())
                .text_color(cx.theme().muted_foreground)
                .child(label),
        );
    }
    header = header.child(
        div()
            .id("explorer-show-ignored")
            .flex_none()
            .cursor_pointer()
            .debug_selector(|| "explorer-show-ignored".to_owned())
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    let next = !view.explorer_show_ignored();
                    view.explorer_set_show_ignored(next, cx);
                }),
            )
            .child(if show_ignored {
                "ignored: on"
            } else {
                "ignored: off"
            }),
    );

    div()
        .id("explorer-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(header)
        .child(list)
        .into_any_element()
}
