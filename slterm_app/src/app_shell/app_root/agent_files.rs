//! Agent 全局文件树：多根共享一棵树、一个滚动区与一份 ViewState。
//!
//! 每根独立 `FileTreeModel`，渲染按根顺序拼接；根标题行可折叠。读取串行，
//! 旧回执由各模型代次拒绝。运行态只消费目录读取与授权结果，不另建通道。

use std::collections::{BTreeSet, VecDeque};
use std::io;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, ScrollHandle, StatefulInteractiveElement as _, Styled as _, Task, Window,
    div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_path::{CanonicalPath, PathGrant};
use slterm_settings::AgentRootId;
use slterm_sidebar::{
    AgentFilesState, DirectoryReadRequest, DirectoryStatus, FileTreeEntry, FileTreeEntryKind,
    FileTreeError, FileTreeModel, FileTreeRoot, SideViewPath, SideViewState,
};
use slterm_workspace::SideViewId;

use super::AppRoot;
use super::explorer::{join_relative, persistent_path, read_directory};
use super::side_view::SideView;
use super::task_service::{CancellationToken, TaskId};

/// 单根运行态：内置根与自定义根共用同一形态。
pub(super) struct AgentFilesRoot {
    pub(super) root_id: AgentRootId,
    pub(super) display_name: String,
    pub(super) path: CanonicalPath,
    pub(super) tree: FileTreeModel,
    pub(super) expanded: bool,
}

impl AgentFilesRoot {
    fn new(root_id: AgentRootId, display_name: String, path: CanonicalPath) -> Self {
        Self {
            root_id,
            display_name,
            path: path.clone(),
            tree: FileTreeModel::new(FileTreeRoot::Agent(root_id), path),
            expanded: true,
        }
    }
}

/// 多根运行态；根列表变化时保留同身份根的展开快照。
pub(super) struct AgentFilesRuntime {
    pub(super) roots: Vec<AgentFilesRoot>,
    pending_reads: VecDeque<(AgentRootId, DirectoryReadRequest)>,
    pending_expanded: Vec<(AgentRootId, CanonicalPath)>,
    restoring_row: Option<(AgentRootId, Option<SideViewPath>)>,
    restoring_scroll: Option<f64>,
    selected: Option<CanonicalPath>,
    multi_selection: BTreeSet<CanonicalPath>,
    /// 根层显示过滤：只影响渲染与读取调度，不是授权边界。
    filter: String,
    filter_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    _filter_change: Option<gpui::Subscription>,
    pub(super) scroll_handle: ScrollHandle,
    reading: bool,
    /// 用户动作产生的持久变更；恢复本身不写盘。
    dirty: bool,
    task: Option<Task<()>>,
    error: Option<AgentFilesError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AgentFilesError {
    RootUnavailable,
    DirectoryRead,
}

impl AgentFilesRuntime {
    pub(super) fn new(restored: Option<&AgentFilesState>) -> Self {
        let mut runtime = Self {
            roots: Vec::new(),
            pending_reads: VecDeque::new(),
            pending_expanded: Vec::new(),
            restoring_row: None,
            restoring_scroll: None,
            selected: None,
            multi_selection: BTreeSet::new(),
            filter: String::new(),
            filter_input: None,
            _filter_change: None,
            scroll_handle: ScrollHandle::new(),
            reading: false,
            dirty: false,
            task: None,
            error: None,
        };
        if let Some(restored) = restored {
            runtime.restoring_row = restored
                .current_row()
                .map(|(root, path)| (root, path.cloned()));
            runtime.restoring_scroll = Some(restored.scroll());
        }
        runtime
    }

    /// 用最新根列表同步模型：保留同身份根的展开状态与已读批次。
    pub(super) fn set_roots(&mut self, roots: Vec<(AgentRootId, String, CanonicalPath)>) {
        let previous = std::mem::take(&mut self.roots)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let mut previous = previous;
        for (root_id, display_name, path) in roots {
            let reusable = previous.iter().position(|root| {
                root.as_ref()
                    .is_some_and(|root| root.root_id == root_id && root.path == path)
            });
            if let Some(index) = reusable
                && let Some(mut root) = previous[index].take()
            {
                root.display_name = display_name;
                self.roots.push(root);
                continue;
            }
            self.roots
                .push(AgentFilesRoot::new(root_id, display_name, path));
        }
        self.pending_reads
            .retain(|(root_id, _)| self.roots.iter().any(|root| root.root_id == *root_id));
    }

    /// 恢复待展开目录与当前行；根批次回投后逐步推进。
    pub(super) fn restore(&mut self, restored: Option<&AgentFilesState>) {
        let Some(restored) = restored else {
            return;
        };
        for root in &mut self.roots {
            let root_id = root.root_id;
            if let Some(expanded) = restored.root_expanded(root_id) {
                root.expanded = expanded;
            }
            for relative in restored.root_directories(root_id) {
                if let Some(path) = join_relative(&root.path, relative.as_str()) {
                    self.pending_expanded.push((root_id, path));
                }
            }
        }
        self.restoring_row = restored
            .current_row()
            .map(|(root, path)| (root, path.cloned()));
        self.restoring_scroll = Some(restored.scroll());
    }

    fn root_for_path(&self, path: &CanonicalPath) -> Option<&AgentFilesRoot> {
        self.roots
            .iter()
            .find(|root| path.as_path().starts_with(root.path.as_path()))
    }

    fn root_index_for_path(&self, path: &CanonicalPath) -> Option<usize> {
        self.roots
            .iter()
            .position(|root| path.as_path().starts_with(root.path.as_path()))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn selected(&self) -> Option<&CanonicalPath> {
        self.selected.as_ref()
    }

    pub(super) fn expanded(&self, root_id: AgentRootId) -> bool {
        self.roots
            .iter()
            .find(|root| root.root_id == root_id)
            .is_some_and(|root| root.expanded)
    }

    pub(super) fn current_row(&self) -> Option<(AgentRootId, Option<SideViewPath>)> {
        let path = self.selected.as_ref()?;
        let root = self.root_for_path(path)?;
        Some((root.root_id, persistent_path(&root.path, path)))
    }

    /// 根标题点击：折叠/展开整根。
    /// 撤销单根：移除模型、在途请求与属于该根的分区状态，选中清空。
    /// 只匹配根显示名（大小写不敏感）；空过滤显示全部根。
    pub(super) fn matches_root(&self, root: &AgentFilesRoot) -> bool {
        let filter = self.filter.trim().to_lowercase();
        filter.is_empty() || root.display_name.to_lowercase().contains(&filter)
    }

    pub(super) fn set_filter(&mut self, filter: &str) {
        self.filter.clear();
        self.filter.push_str(filter);
    }

    /// 当前显示根的显示名，调试与测试共用。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn visible_root_names(&self) -> Vec<String> {
        self.roots
            .iter()
            .filter(|root| self.matches_root(root))
            .map(|root| root.display_name.clone())
            .collect()
    }

    pub(super) fn remove_root(&mut self, root_id: AgentRootId) {
        self.roots.retain(|root| root.root_id != root_id);
        self.pending_reads
            .retain(|(pending_root, _)| *pending_root != root_id);
        self.pending_expanded
            .retain(|(pending_root, _)| *pending_root != root_id);
        if self
            .selected
            .as_ref()
            .is_some_and(|path| self.root_for_path(path).is_none())
        {
            self.selected = None;
        }
        self.dirty = true;
    }

    pub(super) fn toggle_root(&mut self, root_id: AgentRootId) {
        if let Some(root) = self.roots.iter_mut().find(|root| root.root_id == root_id) {
            root.expanded = !root.expanded;
            self.dirty = true;
        }
    }

    /// 行点击：目录切换展开，文件只选中。
    pub(super) fn activate_path(&mut self, path: &CanonicalPath) {
        let Some(index) = self.root_index_for_path(path) else {
            return;
        };
        let Some(kind) = self.roots[index].tree.kind_of(path) else {
            return;
        };
        self.selected = Some(path.clone());
        self.multi_selection.clear();
        self.dirty = true;
        if kind != FileTreeEntryKind::Directory {
            return;
        }
        let root_id = self.roots[index].root_id;
        match self.roots[index].tree.directory_status(path) {
            DirectoryStatus::Loaded | DirectoryStatus::Loading => {
                let _ = self.roots[index].tree.collapse(path);
            }
            _ => {
                if let Ok(Some(request)) = self.roots[index].tree.expand(path) {
                    self.pending_reads.push_back((root_id, request));
                }
            }
        }
    }

    fn scroll_offset(&self) -> f64 {
        (-self.scroll_handle.offset().y.as_f32() as f64).max(0.0)
    }

    fn checkpoint_state(&self) -> Option<AgentFilesState> {
        let scroll = self.scroll_offset();
        let scroll_changed = self
            .restoring_scroll
            .map(|restored| (restored - scroll).abs() > 0.5)
            .unwrap_or(scroll > 0.0);
        if !self.dirty && !scroll_changed {
            return None;
        }
        let mut state = AgentFilesState::default();
        for root in &self.roots {
            let directories = root
                .tree
                .expanded_paths()
                .into_iter()
                .filter_map(|path| persistent_path(&root.path, &path))
                .collect();
            state.set_root(root.root_id, root.expanded, directories);
        }
        if let Some((root_id, path)) = self.current_row() {
            state.set_current_row(root_id, path);
        }
        state.set_scroll(scroll);
        Some(state)
    }
}

/// 渲染：根标题行 + 展开根的可见行，共享一个滚动区。
pub(super) fn render_agent_files(
    view: &mut SideView,
    cx: &mut Context<'_, SideView>,
) -> gpui::AnyElement {
    let Some(runtime) = &view.agent_files else {
        return div().size_full().into_any_element();
    };
    let scroll_handle = runtime.scroll_handle.clone();
    let roots = runtime
        .roots
        .iter()
        .filter(|root| runtime.matches_root(root))
        .map(|root| {
            (
                root.root_id,
                root.display_name.clone(),
                root.expanded,
                root.tree.visible_rows(),
            )
        })
        .collect::<Vec<_>>();
    let selected = runtime.selected.clone();
    let has_error = runtime.error.is_some();

    let mut list = div()
        .id("agent-files-list")
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .debug_selector(|| "agent-files-list".to_owned())
        .text_sm();

    if has_error {
        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "agent-files-error".to_owned())
                .child("部分目录不可用"),
        );
    }

    for (root_index, (root_id, display_name, expanded, rows)) in roots.into_iter().enumerate() {
        list = list.child(
            div()
                .id(("agent-files-root", root_index))
                .h(px(22.0))
                .px_2()
                .flex()
                .items_center()
                .text_color(cx.theme().foreground)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        view.agent_files_toggle_root(root_id, cx);
                    }),
                )
                .child(format!(
                    "{} {}",
                    if expanded { "▾" } else { "▸" },
                    display_name
                )),
        );
        if !expanded {
            continue;
        }
        for (row_index, row) in rows.into_iter().enumerate() {
            let path = row.path.clone();
            let is_selected = selected.as_ref() == Some(&row.path);
            let label = match row.kind {
                FileTreeEntryKind::Directory => format!("▸ {}", row.name),
                FileTreeEntryKind::File => format!("  {}", row.name),
            };
            list = list.child(
                div()
                    .id(("agent-files-row", root_index * 4096 + row_index))
                    .h(px(22.0))
                    .pl(px(16.0 + row.depth as f32 * 12.0))
                    .when(is_selected, |element| {
                        element.text_color(cx.theme().foreground)
                    })
                    .when(!is_selected, |element| {
                        element.text_color(cx.theme().muted_foreground)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                            if event.click_count >= 2 {
                                view.agent_files_open_at(&path, window, cx);
                            } else {
                                view.agent_files_activate_path(&path, cx);
                            }
                        }),
                    )
                    .child(label),
            );
        }
    }
    div()
        .id("agent-files-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .debug_selector(|| "agent-files-panel".to_owned())
        .child(list)
        .into_any_element()
}

impl SideView {
    pub(super) fn ensure_agent_files(&mut self) {
        if self.agent_files.is_some() {
            return;
        }
        let restored = match &self.state {
            SideViewState::AgentFiles(state) => Some(state.clone()),
            _ => None,
        };
        self.agent_files = Some(AgentFilesRuntime::new(restored.as_ref()));
    }

    pub(super) fn agent_files_set_roots(
        &mut self,
        roots: Vec<(AgentRootId, String, CanonicalPath)>,
    ) {
        let restored = match &self.state {
            SideViewState::AgentFiles(state) => Some(state.clone()),
            _ => None,
        };
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        runtime.set_roots(roots);
        runtime.restore(restored.as_ref());
    }

    /// 根批次请求统一入队并启动串行读取。
    pub(super) fn agent_files_begin(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.agent_files {
            for index in 0..runtime.roots.len() {
                if !runtime.roots[index].expanded || !runtime.matches_root(&runtime.roots[index]) {
                    continue;
                }
                let root_id = runtime.roots[index].root_id;
                let path = runtime.roots[index].path.clone();
                let request = runtime.roots[index].tree.request_directory(&path);
                if let Some(request) = request {
                    runtime.pending_reads.push_back((root_id, request));
                }
            }
        }
        self.start_agent_files_reads(cx);
    }

    /// 单任务串行读取；任务句柄归本实体，关闭即取消。
    fn start_agent_files_reads(&mut self, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        if runtime.reading {
            return;
        }
        let Some((root_id, request)) = runtime.pending_reads.pop_front() else {
            return;
        };
        runtime.reading = true;
        let directory = request.directory.clone();
        let executor = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { read_directory(&directory) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.agent_files_apply_read(root_id, &request, result);
                view.agent_files_restore_pending();
                view.agent_files_apply_restored_scroll();
                if let Some(runtime) = &mut view.agent_files {
                    runtime.reading = false;
                }
                view.start_agent_files_reads(cx);
                cx.notify();
            });
        });
        if let Some(runtime) = &mut self.agent_files {
            runtime.task = Some(task);
        }
    }

    fn agent_files_apply_read(
        &mut self,
        root_id: AgentRootId,
        request: &DirectoryReadRequest,
        result: io::Result<Vec<FileTreeEntry>>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let Some(root) = runtime
            .roots
            .iter_mut()
            .find(|root| root.root_id == root_id)
        else {
            return;
        };
        let is_root = request.directory == root.path;
        match result {
            Ok(entries) => match root.tree.apply_batch(request, entries, true) {
                Ok(_) => runtime.error = None,
                Err(FileTreeError::StaleRoot | FileTreeError::StaleRead) => {}
                Err(_) => runtime.error = Some(AgentFilesError::DirectoryRead),
            },
            Err(_) => {
                if root.tree.fail_directory(request).is_ok() {
                    runtime.error = Some(if is_root {
                        AgentFilesError::RootUnavailable
                    } else {
                        AgentFilesError::DirectoryRead
                    });
                }
            }
        }
    }

    /// 级联恢复待展开目录与当前行；不可达路径保持待定直到父批次回投。
    fn agent_files_restore_pending(&mut self) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let pending = std::mem::take(&mut runtime.pending_expanded);
        for (root_id, path) in pending {
            let Some(index) = runtime
                .roots
                .iter()
                .position(|root| root.root_id == root_id)
            else {
                continue;
            };
            let Some(kind) = runtime.roots[index].tree.kind_of(&path) else {
                // 尚未读到的路径继续等待父批次。
                runtime.pending_expanded.push((root_id, path));
                continue;
            };
            if kind != FileTreeEntryKind::Directory || runtime.roots[index].tree.is_expanded(&path)
            {
                continue;
            }
            if let Ok(Some(request)) = runtime.roots[index].tree.expand(&path) {
                runtime.pending_reads.push_back((root_id, request));
            }
        }
        let Some((root_id, path)) = runtime.restoring_row.clone() else {
            return;
        };
        let Some(index) = runtime
            .roots
            .iter()
            .position(|root| root.root_id == root_id)
        else {
            runtime.restoring_row = None;
            return;
        };
        let Some(relative) = path else {
            runtime.restoring_row = None;
            return;
        };
        let Some(absolute) = join_relative(&runtime.roots[index].path, relative.as_str()) else {
            runtime.restoring_row = None;
            return;
        };
        if runtime.roots[index].tree.kind_of(&absolute).is_some() {
            runtime.selected = Some(absolute);
            runtime.restoring_row = None;
        }
    }

    /// 全部展开根的根批次就绪后才应用恢复滚动，避免被空内容夹紧。
    fn agent_files_apply_restored_scroll(&mut self) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let all_loaded = runtime
            .roots
            .iter()
            .filter(|root| root.expanded)
            .all(|root| root.tree.directory_status(&root.path) == DirectoryStatus::Loaded);
        if !all_loaded {
            return;
        }
        if let Some(scroll) = runtime.restoring_scroll.take()
            && scroll > 0.0
        {
            runtime
                .scroll_handle
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-(scroll as f32))));
        }
    }

    pub(super) fn agent_files_toggle_root(
        &mut self,
        root_id: AgentRootId,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        runtime.toggle_root(root_id);
        // 根展开后需要根批次；已加载根不重复请求。
        if let Some(runtime) = &mut self.agent_files
            && runtime.expanded(root_id)
            && let Some(index) = runtime
                .roots
                .iter()
                .position(|root| root.root_id == root_id)
        {
            let path = runtime.roots[index].path.clone();
            if let Some(request) = runtime.roots[index].tree.request_directory(&path) {
                runtime.pending_reads.push_back((root_id, request));
            }
        }
        self.start_agent_files_reads(cx);
        cx.notify();
    }

    /// 文件行打开：只对文件生效，意图在广播结束后由 AppRoot 统一打开。
    pub(super) fn agent_files_open_at(
        &mut self,
        path: &CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &self.agent_files else {
            return;
        };
        let Some(root) = runtime.root_for_path(path) else {
            return;
        };
        if root.tree.kind_of(path) != Some(FileTreeEntryKind::File) {
            return;
        }
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let path = path.clone();
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    root.handle_agent_files_open(path, window, cx);
                });
            });
        });
    }

    pub(super) fn agent_files_activate_path(
        &mut self,
        path: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.activate_path(path);
        }
        self.start_agent_files_reads(cx);
        cx.notify();
    }

    /// 应用级根同步入口：AppRoot 提供根列表后由本实体登记读取。
    pub(super) fn agent_files_sync(
        &mut self,
        roots: Vec<(AgentRootId, String, CanonicalPath)>,
        cx: &mut Context<'_, Self>,
    ) {
        self.ensure_agent_files();
        let previous_paths = self
            .agent_files
            .as_ref()
            .map(|runtime| {
                runtime
                    .roots
                    .iter()
                    .map(|root| (root.root_id, root.path.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let next_paths = roots
            .iter()
            .map(|(root_id, _, path)| (*root_id, path.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        if previous_paths != next_paths {
            crate::app_shell::authorization::advance_generation();
        }
        // 配置中消失的根按撤销处理：运行态与持久分区一并清理，新根按新身份登记。
        let next = roots
            .iter()
            .map(|(root_id, _, _)| *root_id)
            .collect::<BTreeSet<_>>();
        let next_paths = roots
            .iter()
            .map(|(root_id, _, path)| (*root_id, path.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let removed = self
            .agent_files
            .as_ref()
            .map(|runtime| {
                runtime
                    .roots
                    .iter()
                    // 同 id 改路径同样是新身份：旧分区与运行态必须先撤销。
                    .filter(|root| {
                        !next.contains(&root.root_id)
                            || next_paths
                                .get(&root.root_id)
                                .is_some_and(|path| path != &root.path)
                    })
                    .map(|root| root.root_id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for root_id in removed {
            self.agent_files_remove_root(root_id, cx);
        }
        self.agent_files_set_roots(roots);
        self.agent_files_begin(cx);
    }

    /// 根改路径回执：撤销旧身份的运行态与持久分区，新根按新身份登记。
    pub(super) fn agent_files_remove_root(
        &mut self,
        root_id: AgentRootId,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.remove_root(root_id);
        }
        let _ = self.edit_state(|state| {
            state.remove_agent_root(root_id);
        });
        cx.notify();
    }

    /// 根层过滤变化：只重排显示与读取调度，不改变任何授权来源。
    pub(super) fn agent_files_set_filter(&mut self, filter: &str, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_filter(filter);
        }
        self.agent_files_begin(cx);
        cx.notify();
    }

    fn ensure_agent_files_filter_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self
            .agent_files
            .as_ref()
            .is_none_or(|runtime| runtime.filter_input.is_some())
        {
            return;
        }
        let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx));
        let change = cx.subscribe_in(
            &input,
            window,
            |view, input, event: &gpui_component::input::InputEvent, _, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    view.agent_files_set_filter(&text, cx);
                }
            },
        );
        if let Some(runtime) = &mut self.agent_files {
            runtime.filter_input = Some(input);
            runtime._filter_change = Some(change);
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_root_status_for_test(
        &self,
        root_id: AgentRootId,
    ) -> Option<DirectoryStatus> {
        self.agent_files
            .as_ref()?
            .roots
            .iter()
            .find(|root| root.root_id == root_id)
            .map(|root| root.tree.directory_status(&root.path))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_visible_roots_for_test(&self) -> Vec<String> {
        self.agent_files
            .as_ref()
            .map(|runtime| runtime.visible_root_names())
            .unwrap_or_default()
    }

    /// 关闭/退出共用检查点出口；无用户动作时不写盘。
    pub(super) fn checkpoint_agent_files_state(&mut self) {
        let Some(runtime) = &self.agent_files else {
            return;
        };
        let Some(state) = runtime.checkpoint_state() else {
            return;
        };
        let _ = self.edit_state(|view_state| {
            if let SideViewState::AgentFiles(existing) = view_state {
                *existing = state.clone();
            }
        });
        if let Some(runtime) = &mut self.agent_files {
            runtime.dirty = false;
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_expanded_for_test(&self, root_id: AgentRootId) -> bool {
        self.agent_files
            .as_ref()
            .is_some_and(|runtime| runtime.expanded(root_id))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_selected_text_for_test(&self) -> Option<String> {
        let runtime = self.agent_files.as_ref()?;
        let path = runtime.selected()?;
        let root = runtime.root_for_path(path)?;
        Some(
            persistent_path(&root.path, path)
                .map(|relative| relative.as_str().to_owned())
                .unwrap_or_default(),
        )
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_scroll_handle_for_test(&self) -> Option<ScrollHandle> {
        self.agent_files
            .as_ref()
            .map(|runtime| runtime.scroll_handle.clone())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_multi_selection_is_empty_for_test(&self) -> bool {
        self.agent_files
            .as_ref()
            .is_some_and(|runtime| runtime.multi_selection.is_empty())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_set_multi_selection_for_test(&mut self, path: CanonicalPath) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.multi_selection.insert(path);
        }
    }
}

impl AppRoot {
    /// Agent 树文件打开：活跃项目聚焦组 + 统一 Document 打开链。
    pub(super) fn handle_agent_files_open(
        &mut self,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(project_id) = self.snapshot.registry().active() else {
            return;
        };
        let target_group = match self.focused_group(project_id, cx) {
            Ok(group) => group,
            Err(_) => return,
        };
        let mut request = match self.prepare_document_open(project_id, target_group, path, false) {
            Ok(request) => request,
            Err(_) => return,
        };
        request.read_grants = self.agent_root_grants_for_path(&request.path);
        self.queue_document_open(request, None, window, cx);
    }

    /// 覆盖该路径的 Agent 根只读／可写授权；根快照未就绪时为空。
    pub(super) fn agent_root_grants_for_path(&self, path: &CanonicalPath) -> Vec<PathGrant> {
        if !self.agent_roots_loaded {
            return Vec::new();
        }
        let mut grants = Vec::new();
        for dir in self.agent_global_dirs.iter().flatten() {
            if let Ok(root_path) = CanonicalPath::new(dir.path.clone())
                && path.as_path().starts_with(root_path.as_path())
            {
                // 内置根固定只读、禁止终端。
                grants.push(PathGrant::agent_root(root_path, false, false));
            }
        }
        for root in &self.custom_agent_roots {
            if let Ok(root_path) = CanonicalPath::new(root.path())
                && path.as_path().starts_with(root_path.as_path())
            {
                grants.push(PathGrant::agent_root(
                    root_path,
                    root.capabilities().allows_write(),
                    root.capabilities().allows_terminal(),
                ));
            }
        }
        grants
    }

    /// Document 打开／保存授权：项目根并集再叠加 Agent 根能力。
    pub(super) fn document_grants(
        &self,
        document_id: slterm_document::DocumentId,
    ) -> Vec<PathGrant> {
        let mut grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        let path = self
            .snapshot
            .documents()
            .find(|(id, _)| *id == document_id)
            .map(|(_, location)| location.canonical_path().clone());
        if let Some(path) = path {
            grants.extend(self.agent_root_grants_for_path(&path));
        }
        grants
    }

    /// 视图打开/根同步：安装运行态并交给视图自行读取。
    pub(super) fn sync_agent_files_roots(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let roots = self.agent_files_roots();
        let Some(view) = self.side_views.instance(SideViewId::AgentFiles).cloned() else {
            return;
        };
        view.update(cx, |view, cx| {
            view.ensure_agent_files_filter_input(window, cx);
            view.agent_files_sync(roots, cx);
        });
    }

    /// 后台根快照回投：只同步根与读取，输入框由下一次带窗口的同步安装。
    pub(super) fn sync_agent_files_roots_after_load(&mut self, cx: &mut Context<'_, Self>) {
        let roots = self.agent_files_roots();
        let Some(view) = self.side_views.instance(SideViewId::AgentFiles).cloned() else {
            return;
        };
        view.update(cx, |view, cx| view.agent_files_sync(roots, cx));
        self.reconcile_fs_watch();
    }

    pub(super) fn agent_files_roots(&self) -> Vec<(AgentRootId, String, CanonicalPath)> {
        if !self.agent_roots_loaded {
            return Vec::new();
        }
        let mut roots = self
            .agent_global_dirs
            .iter()
            .flatten()
            .filter_map(|dir| {
                let path = CanonicalPath::new(dir.path.clone()).ok()?;
                Some((dir.root_id, dir.display_name.clone(), path))
            })
            .collect::<Vec<_>>();
        roots.extend(self.custom_agent_roots.iter().filter_map(|root| {
            let path = CanonicalPath::new(root.path()).ok()?;
            Some((root.id(), root.display_name().to_owned(), path))
        }));
        roots
    }

    /// 首帧后单任务加载 Agent 根快照；前台不触碰 home 与设置文件。
    pub(super) fn ensure_agent_roots_loading(&mut self, cx: &mut Context<'_, Self>) {
        if self.agent_roots_loaded || self.tasks.is_registered(TaskId::AgentRootsLoad) {
            return;
        }
        let executor = cx.background_executor().clone();
        self.tasks
            .register_with(
                TaskId::AgentRootsLoad,
                CancellationToken::new(),
                move || {
                    let loading = executor.spawn(async move {
                        let dirs = crate::agent_dirs::agent_global_dirs();
                        let roots =
                            crate::agent_root_service::load_agent_roots().unwrap_or_default();
                        (dirs, roots)
                    });
                    cx.spawn(async move |this, cx| {
                        let (dirs, roots) = loading.await;
                        let _ = this.update(cx, |root, cx| {
                            // 已注入或更早完成的快照优先，迟到回投不得覆盖。
                            if root.agent_roots_loaded {
                                return;
                            }
                            root.agent_global_dirs = Some(dirs);
                            root.custom_agent_roots = roots;
                            root.agent_roots_loaded = true;
                            root.sync_agent_files_roots_after_load(cx);
                            cx.notify();
                        });
                    })
                },
            )
            .expect("AgentRootsLoad 单任务登记必须唯一");
    }
}
