//! Explorer 运行态：固定项目根、真实目录读取与打开意图。
//!
//! 树结构与持久状态分离：`FileTreeModel` 是唯一树事实，瞬态只在本模块；
//! 跨域动作只产生 `ExplorerIntent`，由 AppRoot 在广播结束后统一执行。

use std::collections::{BTreeSet, VecDeque};
use std::io;

use gpui::{
    AppContext as _, Context, Focusable as _, InteractiveElement as _, MouseDownEvent,
    StatefulInteractiveElement as _, Styled as _, Task, WeakEntity, Window, div, px,
};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest};
use slterm_sidebar::{
    DirectoryReadRequest, DirectoryStatus, ExplorerProjectState, FileTreeEntry, FileTreeEntryKind,
    FileTreeError, FileTreeModel, FileTreeRoot, SideViewPath, SideViewState,
};
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::side_view::SideView;
use crate::i18n::Message;

mod ops;
mod render;

pub(super) use render::{render_explorer, render_explorer_paste_dialog};

gpui::actions!(
    slterm_explorer,
    [
        ExplorerRenameSelected,
        ExplorerOpenSelected,
        ExplorerCancelInlineEdit,
        ExplorerDeleteSelected,
        ExplorerRefresh,
        ExplorerCopySelected,
        ExplorerCutSelected,
        ExplorerPasteSelected,
        ExplorerSelectAll
    ]
);

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
    CreateEntry {
        project_id: ProjectId,
        root: CanonicalPath,
        directory: CanonicalPath,
        name: String,
        directory_entry: bool,
    },
    DeleteEntries {
        project_id: ProjectId,
        root: CanonicalPath,
        items: Vec<CanonicalPath>,
        permanent: bool,
    },
    PasteEntries {
        project_id: ProjectId,
        root: CanonicalPath,
        directory: CanonicalPath,
    },
    LoadStatus {
        project_id: ProjectId,
        root: CanonicalPath,
        generation: u64,
    },
}

/// 行菜单：打开、文件操作与内联输入都只从这里产生显式意图。
pub(super) enum ExplorerMenu {
    Actions {
        path: CanonicalPath,
        kind: FileTreeEntryKind,
        root: CanonicalPath,
        /// 右键落在多选集合内时保留多选；菜单打开动作只对单选可用。
        multi: bool,
    },
    Input {
        root: CanonicalPath,
        directory: CanonicalPath,
        target: Option<CanonicalPath>,
        kind: ExplorerInputKind,
        input: gpui::Entity<gpui_component::input::InputState>,
        _enter: gpui::Subscription,
    },
    ConfirmDelete {
        root: CanonicalPath,
        project_id: ProjectId,
        targets: Vec<CanonicalPath>,
        permanent: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExplorerInputKind {
    NewFile,
    NewFolder,
    Rename,
}

/// 已预检、等待冲突决定的 Explorer 粘贴；取消时零修改。
pub(super) struct ExplorerPastePending {
    prepared: crate::app_shell::paste_service::PreparedPaste,
    grants: Vec<PathGrant>,
    mode: slterm_sidebar::FileClipboardMode,
    target_dir: CanonicalPath,
    source_dirs: Vec<CanonicalPath>,
    rejections: usize,
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

/// 单条 Git 状态字母按固定优先级投影；只用于展示。
fn explorer_status_label(entry: &crate::git::StatusEntry) -> char {
    if entry.conflict {
        'U'
    } else if entry.ignored {
        'I'
    } else if entry.index == 'A' {
        'A'
    } else if entry.index == '?' {
        'U'
    } else if entry.index == 'R' || entry.worktree == 'R' {
        'R'
    } else if entry.worktree == 'D' || entry.index == 'D' {
        'D'
    } else {
        'M'
    }
}

/// 状态读取授权 = 项目根与只读父仓库根的并集；仓库根仅作读取。
pub(crate) fn status_grants(root: &CanonicalPath) -> Result<Vec<PathGrant>, crate::git::GitError> {
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
        expected_authorization_generation: crate::app_shell::authorization::current_generation(),
    };
    let grant = PathGrant::project_root(root.clone());
    let authorization = PathPolicy::authorize(
        &request,
        &resolved,
        std::slice::from_ref(&grant),
        crate::app_shell::authorization::current_generation(),
    )
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
    /// 绝对路径 → (Git 状态字母, 是否忽略)；只作展示与忽略过滤。
    status: std::collections::BTreeMap<CanonicalPath, (char, bool)>,
    /// 数据根切换代次；状态回执只结算同一代次。
    generation: u64,
    /// 在途状态读取期间的新数据根：只保留合并位，回执后重投。
    pending_status: bool,
    filter_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    _filter_change: Option<gpui::Subscription>,
    menu: Option<ExplorerMenu>,
    /// 只显示封闭操作结果，不携带路径或底层错误。
    notice: Option<Message>,
    /// 回收站不可用后等待再次确认永久删除的目标集合。
    pending_permanent_delete: Option<Vec<CanonicalPath>>,
    /// 双击序列的第一次点击来自多选：第二次点击不得打开文件。
    double_click_suppressed: bool,
    reading: bool,
    /// 最近一次目录读取是否因单目录上限被截断。
    truncated: bool,
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
            status: std::collections::BTreeMap::new(),
            generation: 1,
            pending_status: false,
            filter_input: None,
            _filter_change: None,
            menu: None,
            notice: None,
            pending_permanent_delete: None,
            double_click_suppressed: false,
            reading: false,
            truncated: false,
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

    fn defer_status(&mut self) {
        self.pending_status = true;
    }

    fn take_deferred_status(&mut self) -> bool {
        std::mem::take(&mut self.pending_status)
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
        self.status.clear();
        self.generation = self.generation.wrapping_add(1);
        self.pending_status = false;
        self.filter_input = None;
        self._filter_change = None;
        self.reading = false;
        self.truncated = false;
        self.menu = None;
        self.notice = None;
        self.pending_permanent_delete = None;
        self.double_click_suppressed = false;
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

/// 单目录条目上限；依赖目录等超大目录不得无界建节点。
pub(super) const EXPLORER_DIRECTORY_ENTRY_LIMIT: usize = 10_000;

/// 文件树行高与渲染窗口 overscan；渲染只建可见区间的元素。
const EXPLORER_ROW_HEIGHT_PX: f32 = 22.0;
const EXPLORER_RENDER_OVERSCAN: usize = 20;

/// 有界目录读取结果；超限时保留前 `limit` 项并显式报告截断。
pub(super) struct DirectoryReadOutcome {
    pub(super) entries: Vec<FileTreeEntry>,
    pub(super) truncated: bool,
}

/// 读取单层目录；非目录/文件条目（链接、设备等）不进入树。
pub(super) fn read_directory_limited(
    directory: &CanonicalPath,
    limit: usize,
) -> io::Result<DirectoryReadOutcome> {
    let mut entries = Vec::new();
    let mut truncated = false;
    for entry in std::fs::read_dir(directory.as_path())? {
        if entries.len() >= limit {
            truncated = true;
            break;
        }
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
    Ok(DirectoryReadOutcome { entries, truncated })
}

/// 生产目录读取统一走单目录上限。
pub(super) fn read_directory(directory: &CanonicalPath) -> io::Result<DirectoryReadOutcome> {
    read_directory_limited(directory, EXPLORER_DIRECTORY_ENTRY_LIMIT)
}

/// 计算需要建立元素的可见行区间；首尾用等高占位保持滚动范围。
fn explorer_render_window(
    total: usize,
    scroll_top_px: f32,
    viewport_px: f32,
) -> std::ops::Range<usize> {
    if total == 0 {
        return 0..0;
    }
    let row = EXPLORER_ROW_HEIGHT_PX;
    let top_row = (scroll_top_px.max(0.0) / row).floor() as usize;
    let visible = (viewport_px.max(row) / row).ceil() as usize;
    let start = top_row.saturating_sub(EXPLORER_RENDER_OVERSCAN).min(total);
    let end = start
        .saturating_add(visible)
        .saturating_add(EXPLORER_RENDER_OVERSCAN * 2)
        .min(total);
    start..end
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
        let generation = self
            .explorer
            .as_ref()
            .map(|runtime| runtime.generation)
            .unwrap_or(0);
        self.dispatch_explorer_intent(
            ExplorerIntent::LoadStatus {
                project_id: project,
                root: status_root,
                generation,
            },
            window,
            cx,
        );
        cx.notify();
    }

    pub(super) fn explorer_defer_status(&mut self, generation: u64) {
        if let Some(runtime) = &mut self.explorer
            && runtime.generation == generation
        {
            runtime.defer_status();
        }
    }

    pub(super) fn explorer_take_deferred_status(&mut self) -> bool {
        self.explorer
            .as_mut()
            .is_some_and(ExplorerRuntime::take_deferred_status)
    }

    pub(super) fn explorer_status_target(&self) -> Option<(ProjectId, CanonicalPath, u64)> {
        self.explorer.as_ref().map(|runtime| {
            (
                runtime.project,
                runtime.tree.root_path().clone(),
                runtime.generation,
            )
        })
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
        result: io::Result<DirectoryReadOutcome>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.reading = false;
        match result {
            Ok(outcome) => match runtime.tree.apply_batch(&request, outcome.entries, true) {
                Ok(_) => {
                    runtime.error = None;
                    runtime.truncated = outcome.truncated;
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
        let previous_selected = runtime.tree.selected().cloned();
        let previous_index = previous_selected.as_ref().and_then(|selected| {
            runtime
                .tree
                .visible_rows()
                .iter()
                .position(|row| &row.path == selected)
        });
        runtime.filter = text.to_owned();
        runtime.tree.set_filter(text);
        let visible = runtime
            .tree
            .visible_rows()
            .into_iter()
            .map(|row| row.path)
            .collect::<Vec<_>>();
        let visible_set = visible.iter().cloned().collect::<BTreeSet<_>>();
        runtime
            .multi_selection
            .retain(|path| visible_set.contains(path));
        // 当前行被过滤隐藏时迁移到原位置附近的下一个／上一个可见项。
        if runtime.tree.selected().is_none()
            && let Some(index) = previous_index
            && !visible.is_empty()
        {
            let target = visible
                .get(index.min(visible.len() - 1))
                .or_else(|| visible.last())
                .cloned();
            if let Some(target) = target {
                let _ = runtime.tree.select(&target);
            }
        }
        cx.notify();
    }

    pub(super) fn explorer_clear_filter(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.filter.clear();
        runtime.tree.set_filter("");
        if let Some(input) = &runtime.filter_input {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
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
        entries: Vec<(CanonicalPath, char, bool)>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.repo_root = repo_root;
        runtime.status_entries = entries.len();
        runtime.status = entries
            .into_iter()
            .map(|(path, label, ignored)| (path, (label, ignored)))
            .collect();
        // 忽略条目被隐藏后，当前行不得继续指向不可见项。
        if !runtime.show_ignored
            && runtime.tree.selected().is_some_and(|selected| {
                runtime
                    .status
                    .get(selected)
                    .is_some_and(|(_, ignored)| *ignored)
            })
        {
            // 当前行被忽略过滤隐藏时迁移到下一个可见表亲条目。
            let hidden = runtime.tree.selected().cloned();
            let index = hidden.as_ref().and_then(|hidden| {
                runtime
                    .tree
                    .visible_rows()
                    .iter()
                    .position(|row| &row.path == hidden)
            });
            let visible = runtime
                .tree
                .visible_rows()
                .into_iter()
                .filter(|row| {
                    runtime.show_ignored
                        || !runtime
                            .status
                            .get(&row.path)
                            .is_some_and(|(_, ignored)| *ignored)
                })
                .map(|row| row.path)
                .collect::<Vec<_>>();
            if let Some(index) = index
                && !visible.is_empty()
            {
                let target = visible.get(index.min(visible.len() - 1)).cloned();
                if let Some(target) = target {
                    let _ = runtime.tree.select(&target);
                }
            }
        }
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
    pub(super) fn explorer_menu_input_for_test(
        &self,
    ) -> Option<gpui::Entity<gpui_component::input::InputState>> {
        match self.explorer.as_ref()?.menu.as_ref()? {
            ExplorerMenu::Input { input, .. } => Some(input.clone()),
            _ => None,
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn explorer_status_char_for_test(&self, path: &CanonicalPath) -> Option<char> {
        self.explorer
            .as_ref()
            .and_then(|runtime| runtime.status.get(path).map(|(label, _)| *label))
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
    /// 多选时 Enter 不打开文件，避免把浏览动作升级为文件路由。
    pub(super) fn explorer_activate_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let multi = self
            .explorer
            .as_ref()
            .is_some_and(|runtime| !runtime.multi_selection.is_empty());
        if multi {
            return;
        }
        let Some(path) = self.explorer_selected().cloned() else {
            return;
        };
        self.explorer_activate_at(&path, window, cx);
    }

    /// 目录双击只切换展开；文件双击才走统一打开路由。
    pub(super) fn explorer_double_click_at(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let suppressed = self
            .explorer
            .as_mut()
            .map(|runtime| std::mem::take(&mut runtime.double_click_suppressed))
            .unwrap_or(false);
        if suppressed {
            // 双击序列始于多选：第二次点击不得把浏览动作变成打开。
            return;
        }
        self.explorer_activate_at(path, window, cx);
    }

    /// 记录单击时的多选事实；双击的回投据此决定是否允许打开。
    pub(super) fn explorer_note_single_click(&mut self) {
        if let Some(runtime) = &mut self.explorer {
            runtime.double_click_suppressed = !runtime.multi_selection.is_empty();
        }
    }

    /// 右键行菜单：多选集合内的行保留多选，集合外改为单选。
    pub(super) fn explorer_open_menu_at(
        &mut self,
        path: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(kind) = runtime.tree.kind_of(path) else {
            return;
        };
        if !runtime.multi_selection.contains(path) {
            if runtime.tree.select(path).is_err() {
                return;
            }
            runtime.multi_selection.clear();
        }
        runtime.menu = Some(ExplorerMenu::Actions {
            path: path.clone(),
            kind,
            root: runtime.tree.root_path().clone(),
            multi: !runtime.multi_selection.is_empty(),
        });
        cx.notify();
    }

    pub(super) fn explorer_close_menu(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.explorer
            && runtime.menu.take().is_some()
        {
            cx.notify();
        }
    }

    /// 菜单“打开”：只对单选文件生效，走统一文件路由。
    pub(super) fn explorer_menu_open(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(ExplorerMenu::Actions {
            path, kind, multi, ..
        }) = runtime.menu.take()
        else {
            return;
        };
        cx.notify();
        if multi || kind != FileTreeEntryKind::File {
            return;
        }
        self.explorer_activate_at(&path, window, cx);
    }

    /// 菜单“在终端中打开”：文件取父目录，目录取自身；与右键路径同一入口。
    pub(super) fn explorer_menu_open_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(ExplorerMenu::Actions { path, multi, .. }) = runtime.menu.take() else {
            return;
        };
        cx.notify();
        if multi {
            return;
        }
        self.explorer_open_terminal_at(&path, window, cx);
    }

    /// 菜单目标：多选集合内右键作用于整个集合，否则只作用于当前行。
    fn explorer_menu_targets(&self) -> Option<(Vec<CanonicalPath>, CanonicalPath, ProjectId)> {
        let runtime = self.explorer.as_ref()?;
        let ExplorerMenu::Actions {
            path, root, multi, ..
        } = runtime.menu.as_ref()?
        else {
            return None;
        };
        let mut targets = if *multi {
            runtime.multi_selection.iter().cloned().collect::<Vec<_>>()
        } else {
            vec![path.clone()]
        };
        targets.sort();
        targets.dedup();
        Some((targets, root.clone(), runtime.project))
    }

    fn explorer_menu_directory(&self) -> Option<CanonicalPath> {
        let runtime = self.explorer.as_ref()?;
        let ExplorerMenu::Actions { path, kind, .. } = runtime.menu.as_ref()? else {
            return None;
        };
        if *kind == FileTreeEntryKind::Directory {
            Some(path.clone())
        } else {
            CanonicalPath::new(path.as_path().parent()?.to_path_buf()).ok()
        }
    }

    fn explorer_open_menu_input(
        &mut self,
        kind: ExplorerInputKind,
        directory: CanonicalPath,
        target: Option<CanonicalPath>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.explorer else {
            return;
        };
        let root = runtime.tree.root_path().clone();
        let value = target
            .as_ref()
            .and_then(|path| path.as_path().file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut state = gpui_component::input::InputState::new(window, cx);
            state.set_value(value, window, cx);
            state
        });
        let enter = cx.subscribe_in(
            &input,
            window,
            |view, _, event: &gpui_component::input::InputEvent, window, cx| {
                if matches!(event, gpui_component::input::InputEvent::PressEnter { .. }) {
                    view.explorer_commit_menu_input(window, cx);
                }
            },
        );
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        if let Some(runtime) = &mut self.explorer {
            runtime.menu = Some(ExplorerMenu::Input {
                root,
                directory,
                target,
                kind,
                input,
                _enter: enter,
            });
        }
        cx.notify();
    }

    pub(super) fn explorer_menu_new(
        &mut self,
        directory_entry: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(directory) = self.explorer_menu_directory() else {
            return;
        };
        self.explorer_open_menu_input(
            if directory_entry {
                ExplorerInputKind::NewFolder
            } else {
                ExplorerInputKind::NewFile
            },
            directory,
            None,
            window,
            cx,
        );
    }

    pub(super) fn explorer_menu_rename(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some((targets, _, _)) = self.explorer_menu_targets() else {
            return;
        };
        if targets.len() != 1 {
            return;
        }
        let target = targets[0].clone();
        let Some(directory) = target
            .as_path()
            .parent()
            .and_then(|parent| CanonicalPath::new(parent.to_path_buf()).ok())
        else {
            return;
        };
        self.explorer_open_menu_input(
            ExplorerInputKind::Rename,
            directory,
            Some(target),
            window,
            cx,
        );
    }

    /// 显式提交输入才产生意图；非法名称保留输入并显示封闭提示。
    pub(super) fn explorer_commit_menu_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let intent = {
            let Some(runtime) = &mut self.explorer else {
                return;
            };
            let Some(ExplorerMenu::Input {
                root,
                directory,
                target,
                kind,
                input,
                ..
            }) = runtime.menu.take()
            else {
                return;
            };
            let name = input.read(cx).value().trim().to_owned();
            let valid = !name.is_empty()
                && name.len() <= 255
                && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
                && name != "."
                && name != ".."
                && !name.ends_with(['.', ' ']);
            if !valid {
                runtime.notice = Some(Message::ExplorerInvalidName);
                cx.notify();
                return;
            }
            runtime.notice = None;
            match kind {
                ExplorerInputKind::NewFile | ExplorerInputKind::NewFolder => {
                    ExplorerIntent::CreateEntry {
                        project_id: runtime.project,
                        root,
                        directory,
                        name,
                        directory_entry: kind == ExplorerInputKind::NewFolder,
                    }
                }
                ExplorerInputKind::Rename => {
                    let Some(target) = target else {
                        return;
                    };
                    let Ok(to) = CanonicalPath::new(directory.as_path().join(&name)) else {
                        return;
                    };
                    if to == target {
                        return;
                    }
                    ExplorerIntent::RenameFile {
                        project_id: runtime.project,
                        root,
                        from: target,
                        to,
                    }
                }
            }
        };
        self.dispatch_explorer_intent(intent, window, cx);
        cx.notify();
    }

    pub(super) fn explorer_cancel_menu_input(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.explorer
            && runtime.menu.take().is_some()
        {
            cx.notify();
        }
    }

    /// 删除先进入菜单内确认；回收站不可用后同一集合再次删除才是永久删除。
    pub(super) fn explorer_menu_delete(&mut self, cx: &mut Context<'_, Self>) {
        let Some((targets, root, project_id)) = self.explorer_menu_targets() else {
            return;
        };
        let permanent = self
            .explorer
            .as_ref()
            .and_then(|runtime| runtime.pending_permanent_delete.as_ref())
            .is_some_and(|pending| pending == &targets);
        if let Some(runtime) = &mut self.explorer {
            runtime.menu = Some(ExplorerMenu::ConfirmDelete {
                root,
                project_id,
                targets,
                permanent,
            });
        }
        cx.notify();
    }

    pub(super) fn explorer_confirm_delete(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let Some(ExplorerMenu::ConfirmDelete {
            root,
            project_id,
            targets,
            permanent,
        }) = runtime.menu.take()
        else {
            return;
        };
        let intent = ExplorerIntent::DeleteEntries {
            project_id,
            root,
            items: targets.clone(),
            permanent,
        };
        if let Some(runtime) = &mut self.explorer {
            runtime.pending_permanent_delete = None;
            runtime.notice = None;
        }
        self.dispatch_explorer_intent(intent, window, cx);
        cx.notify();
    }

    /// 菜单内复制/剪切只写应用会话剪贴板；真实副作用在粘贴执行体。
    pub(super) fn explorer_menu_clipboard(
        &mut self,
        mode: slterm_sidebar::FileClipboardMode,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((targets, _root, project_id)) = self.explorer_menu_targets() else {
            return;
        };
        let Some(runtime) = &self.explorer else {
            return;
        };
        let root_id = slterm_sidebar::FileTreeRoot::Project(project_id);
        let items = targets
            .into_iter()
            .filter_map(|path| {
                let kind = runtime.tree.kind_of(&path)?;
                Some(slterm_sidebar::FileClipboardItem {
                    root: root_id.clone(),
                    path,
                    kind,
                })
            })
            .collect::<Vec<_>>();
        if let Some(runtime) = &mut self.explorer {
            runtime.menu = None;
            runtime.notice = if items.is_empty() {
                Some(Message::ExplorerOpFailed)
            } else {
                None
            };
        }
        if (!items.is_empty())
            && let Some(root) = self.root_handle.clone()
        {
            let _ = root.update(cx, |root, _| {
                root.file_clipboard = Some(slterm_sidebar::FileClipboard::new(mode, items));
            });
        }
        cx.notify();
    }

    /// 菜单粘贴：目标为当前目录；真实预检与冲突决定由 AppRoot 执行。
    pub(super) fn explorer_menu_paste(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(directory) = self.explorer_menu_directory() else {
            return;
        };
        let Some(runtime) = &self.explorer else {
            return;
        };
        let root = runtime.tree.root_path().clone();
        let project_id = runtime.project;
        if let Some(runtime) = &mut self.explorer {
            runtime.menu = None;
            runtime.notice = None;
        }
        self.dispatch_explorer_intent(
            ExplorerIntent::PasteEntries {
                project_id,
                root,
                directory,
            },
            window,
            cx,
        );
        cx.notify();
    }

    /// 复制相对路径：只写系统文本剪贴板，不写内部文件剪贴板。
    pub(super) fn explorer_menu_copy_path(&mut self, cx: &mut Context<'_, Self>) {
        let Some((targets, root, _)) = self.explorer_menu_targets() else {
            return;
        };
        let text = targets
            .iter()
            .filter_map(|path| relative_text(&root, path))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        if let Some(runtime) = &mut self.explorer {
            runtime.menu = None;
        }
        cx.notify();
    }

    fn explorer_selection_targets(&self) -> Vec<CanonicalPath> {
        let Some(runtime) = &self.explorer else {
            return Vec::new();
        };
        let mut targets: Vec<CanonicalPath> = if runtime.multi_selection.is_empty() {
            runtime.tree.selected().cloned().into_iter().collect()
        } else {
            runtime.multi_selection.iter().cloned().collect()
        };
        targets.sort();
        targets.dedup();
        targets
    }

    /// Delete 键：对选中集打开确认面；与右键菜单共用同一删除意图。
    pub(super) fn explorer_delete_selection(&mut self, cx: &mut Context<'_, Self>) {
        let targets = self.explorer_selection_targets();
        if targets.is_empty() {
            return;
        }
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let permanent = runtime
            .pending_permanent_delete
            .as_ref()
            .is_some_and(|pending| pending == &targets);
        runtime.menu = Some(ExplorerMenu::ConfirmDelete {
            root: runtime.tree.root_path().clone(),
            project_id: runtime.project,
            targets,
            permanent,
        });
        cx.notify();
    }

    /// Ctrl+C / Ctrl+X：写入应用会话文件剪贴板。
    pub(super) fn explorer_clipboard_selection(
        &mut self,
        mode: slterm_sidebar::FileClipboardMode,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.explorer else {
            return;
        };
        let root_id = slterm_sidebar::FileTreeRoot::Project(runtime.project);
        let items = self
            .explorer_selection_targets()
            .into_iter()
            .filter_map(|path| {
                let kind = runtime.tree.kind_of(&path)?;
                Some(slterm_sidebar::FileClipboardItem {
                    root: root_id.clone(),
                    path,
                    kind,
                })
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return;
        }
        if let Some(root) = self.root_handle.clone() {
            let _ = root.update(cx, |root, _| {
                root.file_clipboard = Some(slterm_sidebar::FileClipboard::new(mode, items));
            });
        }
        cx.notify();
    }

    /// Ctrl+V：粘贴到当前选中目录；选中文件时使用其父目录。
    pub(super) fn explorer_paste_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.explorer else {
            return;
        };
        let Some(selected) = runtime.tree.selected().cloned() else {
            return;
        };
        let directory = if runtime.tree.kind_of(&selected) == Some(FileTreeEntryKind::Directory) {
            selected
        } else {
            let Some(parent) = selected
                .as_path()
                .parent()
                .and_then(|parent| CanonicalPath::new(parent.to_path_buf()).ok())
            else {
                return;
            };
            parent
        };
        let root = runtime.tree.root_path().clone();
        let project_id = runtime.project;
        self.dispatch_explorer_intent(
            ExplorerIntent::PasteEntries {
                project_id,
                root,
                directory,
            },
            window,
            cx,
        );
    }

    /// Ctrl+A：把全部可见行纳入多选；当前行保持不变。
    pub(super) fn explorer_select_all_visible(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.multi_selection = runtime
            .tree
            .visible_rows()
            .into_iter()
            .map(|row| row.path)
            .collect();
        cx.notify();
    }

    /// F5：重读当前目录或根。
    pub(super) fn explorer_refresh_selected(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        let directory = runtime
            .tree
            .selected()
            .cloned()
            .filter(|path| runtime.tree.kind_of(path) == Some(FileTreeEntryKind::Directory))
            .or_else(|| {
                runtime.tree.selected().and_then(|path| {
                    path.as_path()
                        .parent()
                        .and_then(|parent| CanonicalPath::new(parent.to_path_buf()).ok())
                })
            })
            .unwrap_or_else(|| runtime.tree.root_path().clone());
        match runtime.tree.refresh(&directory) {
            Ok(Some(request)) => runtime.pending_reads.push_back(request),
            Ok(None) => {}
            Err(_) => runtime.notice = Some(Message::ExplorerOpFailed),
        }
        self.start_explorer_reads(cx);
        cx.notify();
    }

    /// 菜单刷新：重读当前行所在目录；根行刷新整根。
    pub(super) fn explorer_menu_refresh(&mut self, cx: &mut Context<'_, Self>) {
        let Some(directory) = self.explorer_menu_directory() else {
            return;
        };
        let Some(runtime) = &mut self.explorer else {
            return;
        };
        runtime.menu = None;
        runtime.notice = None;
        match runtime.tree.refresh(&directory) {
            Ok(Some(request)) => runtime.pending_reads.push_back(request),
            Ok(None) => {}
            Err(_) => runtime.notice = Some(Message::ExplorerOpFailed),
        }
        self.start_explorer_reads(cx);
        cx.notify();
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
        let cleared_edit = runtime.inline_edit.take().is_some();
        let cleared_menu = runtime.menu.take().is_some();
        if cleared_edit || cleared_menu {
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
