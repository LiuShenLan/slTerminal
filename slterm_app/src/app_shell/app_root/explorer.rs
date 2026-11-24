//! Explorer 运行态：固定项目根、真实目录读取与打开意图。
//!
//! 树结构与持久状态分离：`FileTreeModel` 是唯一树事实，瞬态只在本模块；
//! 跨域动作只产生 `ExplorerIntent`，由 AppRoot 在广播结束后统一执行。

use std::collections::{BTreeSet, VecDeque};
use std::io;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, Task, WeakEntity, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_path::CanonicalPath;
use slterm_sidebar::{
    DirectoryReadRequest, DirectoryStatus, ExplorerProjectState, FileTreeEntry, FileTreeEntryKind,
    FileTreeError, FileTreeModel, FileTreeRoot, SideViewPath, SideViewState,
};
use slterm_workspace::{ProjectId, ProjectRootStatus, SideViewId};

use super::AppRoot;
use super::side_view::SideView;

/// Explorer 触发的跨域意图；只承载已解析的树行事实。
pub(super) enum ExplorerIntent {
    OpenDocument {
        project_id: ProjectId,
        path: CanonicalPath,
    },
}

/// Explorer 错误只区分根不可读与目录读取失败，不回显路径。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExplorerError {
    RootUnavailable,
    DirectoryRead,
}

/// 单视图运行态；目录读取串行，旧根回执由模型代次拒绝。
pub(super) struct ExplorerRuntime {
    project: ProjectId,
    tree: FileTreeModel,
    pending_reads: VecDeque<DirectoryReadRequest>,
    pending_expanded: BTreeSet<CanonicalPath>,
    restoring_row: Option<SideViewPath>,
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
        self.dirty = false;
        self.error = None;
        self.restore(restored);
    }

    /// 恢复只登记待展开目录与待选行；级联展开在真实批次回投后推进。
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
    }
}

/// 读取单层目录；非目录/文件条目（链接、设备等）不进入树。
fn read_directory(directory: &CanonicalPath) -> io::Result<Vec<FileTreeEntry>> {
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

fn join_relative(root: &CanonicalPath, relative: &str) -> Option<CanonicalPath> {
    let path = root
        .as_path()
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    CanonicalPath::new(path).ok()
}

/// 根相对正斜杠文本；非 Unicode 组件返回 None（不持久伪造路径）。
fn relative_text(root: &CanonicalPath, path: &CanonicalPath) -> Option<String> {
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

fn persistent_path(root: &CanonicalPath, path: &CanonicalPath) -> Option<SideViewPath> {
    SideViewPath::new(relative_text(root, path)?).ok()
}

impl AppRoot {
    /// 实例生命周期或活跃项目根变化后同步 Explorer 根；根未变时零副作用。
    pub(super) fn sync_side_view_roots(&mut self, cx: &mut Context<'_, Self>) {
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
                if id == SideViewId::Explorer {
                    match &target {
                        Some((project_id, root)) => {
                            view.sync_explorer_root(*project_id, root.clone(), cx)
                        }
                        None => view.clear_explorer_root(cx),
                    }
                }
            });
        }
        self.side_view_roots_generation = Some(self.snapshot_generation);
    }

    /// Explorer 意图在广播结束后执行；打开失败只保持现状，不伪造面板。
    pub(super) fn handle_explorer_intent(
        &mut self,
        intent: ExplorerIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let ExplorerIntent::OpenDocument { project_id, path } = intent;
        let target_group = match self.focused_group(project_id, cx) {
            Ok(group) => group,
            Err(_) => return,
        };
        let request = match self.prepare_document_open(project_id, target_group, path, false) {
            Ok(request) => request,
            Err(_) => return,
        };
        self.queue_document_open(request, None, window, cx);
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
            return;
        }
        self.checkpoint_explorer_state();
        match &mut self.explorer {
            Some(runtime) => runtime.reset(project, root, restored.as_ref()),
            None => {
                self.explorer = Some(ExplorerRuntime::new(project, root, restored.as_ref()));
            }
        }
        self.request_explorer_root(cx);
        cx.notify();
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
        let _ = self.edit_state(|state| {
            let SideViewState::Explorer(explorer) = state else {
                return;
            };
            let project = explorer.project_mut(project);
            project.set_expanded(expanded);
            project.set_current_row(current_row);
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
        self.start_explorer_reads(cx);
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
    let Some(runtime) = &view.explorer else {
        return div().size_full().into_any_element();
    };
    let root = runtime.tree.root_path().clone();
    let rows = runtime.tree.visible_rows();
    let error = runtime.error;
    let expanded = rows
        .iter()
        .filter(|row| runtime.tree.is_expanded(&row.path))
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    let mut list = div()
        .id("explorer-tree")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .overflow_hidden()
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
        let path = row.path;
        let text_color = if selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let selection = cx.theme().list_active;
        list = list.child(
            div()
                .id(("explorer-row", index))
                .h(px(22.0))
                .pl(px(indent))
                .when(selected, |element| element.bg(selection))
                .text_color(text_color)
                .debug_selector(move || selector.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        view.explorer_activate_at(&path, window, cx);
                    }),
                )
                .child(label),
        );
    }
    list.into_any_element()
}
