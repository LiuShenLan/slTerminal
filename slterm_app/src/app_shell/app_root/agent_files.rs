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
use gpui_component::input::{Input, InputState};
use slterm_path::CanonicalPath;
use slterm_settings::{AgentRootCapabilities, AgentRootId, AgentRootVisibility};
use slterm_sidebar::{
    AgentFilesState, DirectoryReadRequest, DirectoryStatus, FileTreeEntryKind, FileTreeError,
    FileTreeModel, FileTreeRoot, SideViewPath, SideViewState,
};

use super::explorer::{join_relative, persistent_path, read_directory};
use super::side_view::SideView;
use crate::i18n::Message;

/// 单根运行态：内置根与自定义根共用同一形态。
pub(super) struct AgentFilesRoot {
    pub(super) root_id: AgentRootId,
    pub(super) display_name: String,
    pub(super) path: CanonicalPath,
    pub(super) tree: FileTreeModel,
    pub(super) expanded: bool,
    /// 根层展示过滤；只影响显示与读取调度，不是授权边界。
    pub(super) visibility: AgentRootVisibility,
    /// 菜单可用性快照；执行前仍由 AppRoot 重新判定授权。
    pub(super) capabilities: AgentRootCapabilities,
}

impl AgentFilesRoot {
    fn new(
        root_id: AgentRootId,
        display_name: String,
        path: CanonicalPath,
        visibility: AgentRootVisibility,
        capabilities: AgentRootCapabilities,
    ) -> Self {
        Self {
            root_id,
            display_name,
            path: path.clone(),
            tree: FileTreeModel::new(FileTreeRoot::Agent(root_id), path),
            expanded: true,
            visibility,
            capabilities,
        }
    }

    /// 根层第一个路径组件（顶层条目名）；根自身与其外路径返回 None。
    fn top_level_name<'a>(&self, path: &'a CanonicalPath) -> Option<&'a str> {
        let relative = path.as_path().strip_prefix(self.path.as_path()).ok()?;
        let component = relative.components().next()?;
        match component {
            std::path::Component::Normal(name) => name.to_str(),
            _ => None,
        }
    }

    /// 该路径的根层祖先是否可见；深层条目随其顶层目录一起隐藏。
    fn path_visible(&self, path: &CanonicalPath) -> bool {
        if path == &self.path {
            return true;
        }
        self.top_level_name(path)
            .is_some_and(|name| self.visibility.allows_entry(name))
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
    /// 行菜单与操作提示；瞬态，不进入 ViewState。
    menu: Option<AgentFilesMenu>,
    notice: Option<crate::i18n::Message>,
    /// 根层显示过滤：只影响渲染与读取调度，不是授权边界。
    filter: String,
    filter_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    _filter_change: Option<gpui::Subscription>,
    pub(super) scroll_handle: ScrollHandle,
    reading: bool,
    /// 最近一次目录读取是否因单目录上限被截断。
    truncated: bool,
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

/// 菜单输入动作：新建文件/文件夹或重命名。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AgentFilesInputKind {
    NewFile,
    NewFolder,
    Rename,
}

/// 行菜单的输入态；草稿只在运行期存在，取消或销毁即丢弃。
pub(super) struct AgentFilesMenuInput {
    pub(super) kind: AgentFilesInputKind,
    pub(super) root_id: AgentRootId,
    pub(super) directory: CanonicalPath,
    pub(super) target: Option<CanonicalPath>,
    pub(super) input: gpui::Entity<InputState>,
}

/// 当前打开的行菜单形态。
pub(super) enum AgentFilesMenu {
    Actions {
        root_id: AgentRootId,
        path: CanonicalPath,
        kind: FileTreeEntryKind,
    },
    Input(AgentFilesMenuInput),
    ConfirmDelete {
        targets: Vec<CanonicalPath>,
    },
}

/// Agent 树触发的跨域意图；只承载已解析的树行事实，由 AppRoot 统一执行。
#[derive(Clone, Debug)]
pub(super) enum AgentFilesIntent {
    CreateFile {
        root_id: AgentRootId,
        directory: CanonicalPath,
        name: String,
    },
    CreateFolder {
        root_id: AgentRootId,
        directory: CanonicalPath,
        name: String,
    },
    Rename {
        root_id: AgentRootId,
        from: CanonicalPath,
        to: CanonicalPath,
    },
    Delete {
        items: Vec<(AgentRootId, CanonicalPath)>,
    },
    Paste {
        root_id: AgentRootId,
        directory: CanonicalPath,
    },
    OpenTerminal {
        root_id: AgentRootId,
        cwd: CanonicalPath,
    },
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
            menu: None,
            notice: None,
            filter: String::new(),
            filter_input: None,
            _filter_change: None,
            scroll_handle: ScrollHandle::new(),
            reading: false,
            truncated: false,
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
    pub(super) fn set_roots(
        &mut self,
        roots: Vec<(
            AgentRootId,
            String,
            CanonicalPath,
            AgentRootVisibility,
            AgentRootCapabilities,
        )>,
    ) {
        let previous = std::mem::take(&mut self.roots)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let mut previous = previous;
        for (root_id, display_name, path, visibility, capabilities) in roots {
            let reusable = previous.iter().position(|root| {
                root.as_ref()
                    .is_some_and(|root| root.root_id == root_id && root.path == path)
            });
            if let Some(index) = reusable
                && let Some(mut root) = previous[index].take()
            {
                root.display_name = display_name;
                root.visibility = visibility;
                root.capabilities = capabilities;
                self.roots.push(root);
                continue;
            }
            self.roots.push(AgentFilesRoot::new(
                root_id,
                display_name,
                path,
                visibility,
                capabilities,
            ));
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
        self.menu = None;
        self.dirty = true;
    }

    fn root_for_id(&self, root_id: AgentRootId) -> Option<&AgentFilesRoot> {
        self.roots.iter().find(|root| root.root_id == root_id)
    }

    /// 展示配置变化后裁剪隐藏项的展开、待展开、选中与多选；
    /// 只调整显示与读取调度，不改变任何授权来源。
    pub(super) fn prune_hidden(&mut self) {
        for root in &mut self.roots {
            let mut expanded = root
                .tree
                .expanded_paths()
                .into_iter()
                .filter(|path| !root.path_visible(path))
                .collect::<Vec<_>>();
            // 先收合最浅层，后代随祖先一并消失。
            expanded.sort_by_key(|path| path.as_path().components().count());
            for path in expanded {
                let _ = root.tree.collapse(&path);
            }
        }
        let pending = std::mem::take(&mut self.pending_expanded);
        let retained = pending
            .into_iter()
            .filter(|(root_id, path)| {
                self.root_for_id(*root_id)
                    .is_some_and(|root| root.path_visible(path))
            })
            .collect();
        self.pending_expanded = retained;
        if self.selected.as_ref().is_some_and(|path| {
            self.root_for_path(path)
                .is_some_and(|root| !root.path_visible(path))
        }) {
            self.selected = None;
        }
        let selection = std::mem::take(&mut self.multi_selection);
        let retained = selection
            .into_iter()
            .filter(|path| {
                self.root_for_path(path)
                    .is_none_or(|root| root.path_visible(path))
            })
            .collect();
        self.multi_selection = retained;
        if let Some(AgentFilesMenu::Actions { path, .. }) = &self.menu
            && self
                .root_for_path(path)
                .is_some_and(|root| !root.path_visible(path))
        {
            self.menu = None;
        }
    }

    pub(super) fn set_menu(&mut self, menu: Option<AgentFilesMenu>) {
        self.menu = menu;
    }

    pub(super) fn set_notice(&mut self, notice: Option<crate::i18n::Message>) {
        self.notice = notice;
    }

    /// 操作目标：点击行属于当前多选时使用整个多选集，否则只用该行。
    pub(super) fn action_targets(&self, target: &CanonicalPath) -> Vec<CanonicalPath> {
        if self.multi_selection.contains(target) && self.multi_selection.len() > 1 {
            let mut targets = self.multi_selection.iter().cloned().collect::<Vec<_>>();
            targets.sort();
            targets
        } else {
            vec![target.clone()]
        }
    }

    pub(super) fn toggle_root(&mut self, root_id: AgentRootId) {
        if let Some(root) = self.roots.iter_mut().find(|root| root.root_id == root_id) {
            root.expanded = !root.expanded;
            self.dirty = true;
        }
    }

    /// Ctrl 点击：切换多选；隐藏项不可进入选择集。
    pub(super) fn toggle_multi_selection(&mut self, path: &CanonicalPath) {
        let Some(index) = self.root_index_for_path(path) else {
            return;
        };
        if !self.roots[index].path_visible(path) {
            return;
        }
        self.selected = Some(path.clone());
        if !self.multi_selection.remove(path) {
            self.multi_selection.insert(path.clone());
        }
        self.dirty = true;
    }

    /// 行点击：目录切换展开，文件只选中。隐藏路径不响应。
    pub(super) fn activate_path(&mut self, path: &CanonicalPath) {
        let Some(index) = self.root_index_for_path(path) else {
            return;
        };
        if !self.roots[index].path_visible(path) {
            return;
        }
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
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let scroll_handle = runtime.scroll_handle.clone();
    let filter_input = runtime.filter_input.clone();
    let roots = runtime
        .roots
        .iter()
        .filter(|root| runtime.matches_root(root))
        .map(|root| {
            let all_rows = root.tree.visible_rows();
            let top_level = |row: &&slterm_sidebar::FileTreeRow| {
                row.path.as_path().parent() == Some(root.path.as_path())
            };
            let raw_top_level = all_rows.iter().filter(top_level).count();
            let hidden_top_level = all_rows
                .iter()
                .filter(top_level)
                .filter(|row| !root.visibility.allows_entry(&row.name))
                .count();
            (
                root.root_id,
                root.display_name.clone(),
                root.expanded,
                hidden_top_level,
                raw_top_level,
                all_rows
                    .into_iter()
                    .filter(|row| root.path_visible(&row.path))
                    .collect::<Vec<_>>(),
                root.tree.directory_status(&root.path),
            )
        })
        .collect::<Vec<_>>();
    let selected = runtime.selected.clone();
    let multi_selection = runtime.multi_selection.clone();
    let has_error = runtime.error.is_some();
    let notice = runtime.notice;
    let clipboard_available = view
        .root_handle
        .as_ref()
        .and_then(|root| root.upgrade())
        .is_some_and(|root| root.read(cx).agent_files_clipboard.is_some());

    let mut panel = div()
        .id("agent-files-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .debug_selector(|| "agent-files-panel".to_owned());

    // 树内搜索只过滤根名；显示过滤配置归设置中心，二者都不是授权边界。
    if let Some(input) = filter_input {
        panel = panel.child(
            div()
                .px_2()
                .py_1()
                .debug_selector(|| "agent-files-filter".to_owned())
                .child(Input::new(&input)),
        );
    }

    if runtime.truncated {
        panel = panel.child(
            div()
                .px_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .debug_selector(|| "agent-files-truncated".to_owned())
                .child("部分目录超过条目上限，仅显示前若干项"),
        );
    }

    if let Some(message) = notice {
        let root = view.root_handle.clone();
        panel = panel.child(
            div()
                .px_2()
                .text_xs()
                .text_color(cx.theme().danger)
                .debug_selector(|| "agent-files-notice".to_owned())
                .flex()
                .items_center()
                .gap_2()
                .child(language.text(message))
                .child(
                    div()
                        .id("agent-files-notice-dismiss")
                        .debug_selector(|| "agent-files-notice-dismiss".to_owned())
                        .cursor_pointer()
                        .text_color(cx.theme().muted_foreground)
                        .child("×")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _, _, cx| {
                                let _ = &root;
                                if let Some(runtime) = &mut view.agent_files {
                                    runtime.set_notice(None);
                                }
                                cx.notify();
                            }),
                        ),
                ),
        );
    }

    if runtime.menu.is_some() {
        let mut menu_panel = div()
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .gap_1()
            .border_1()
            .border_color(cx.theme().border)
            .debug_selector(|| "agent-files-menu".to_owned());
        match runtime.menu.as_ref() {
            Some(AgentFilesMenu::Actions {
                root_id,
                path,
                kind,
            }) => {
                let root_id = *root_id;
                let path = path.clone();
                let kind = *kind;
                let capabilities = runtime
                    .root_for_id(root_id)
                    .map(|root| root.capabilities)
                    .unwrap_or(AgentRootCapabilities::READ_ONLY);
                let name = path
                    .as_path()
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.as_path().to_string_lossy().into_owned());
                menu_panel = menu_panel.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().foreground)
                        .child(name),
                );
                let mut actions = div().flex().flex_wrap().gap_2();
                let button = |id: &'static str, label: Message| {
                    div()
                        .id(gpui::ElementId::Name(id.into()))
                        .debug_selector(move || id.to_owned())
                        .px_1()
                        .text_xs()
                        .cursor_pointer()
                        .text_color(cx.theme().accent)
                        .child(language.text(label))
                };
                if kind == FileTreeEntryKind::File {
                    let open_path = path.clone();
                    actions = actions.child(
                        button("agent-files-menu-open", Message::SidebarAgentMenuOpen)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.agent_files_open_at(&open_path, window, cx);
                                    view.agent_files_close_menu(cx);
                                }),
                            ),
                    );
                }
                let terminal_allowed = capabilities.allows_terminal();
                let terminal_cwd = if kind == FileTreeEntryKind::Directory {
                    Some(path.clone())
                } else {
                    path.as_path()
                        .parent()
                        .and_then(|parent| CanonicalPath::new(parent.to_path_buf()).ok())
                };
                if let Some(cwd) = terminal_cwd {
                    let mut item = button(
                        "agent-files-menu-open-terminal",
                        Message::SidebarAgentMenuOpenTerminal,
                    );
                    if terminal_allowed {
                        item = item.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _, window, cx| {
                                view.agent_files_open_terminal(root_id, cwd.clone(), window, cx);
                            }),
                        );
                    } else {
                        item = item.text_color(cx.theme().muted_foreground);
                    }
                    actions = actions.child(item);
                }
                let write_allowed = capabilities.allows_write();
                if kind == FileTreeEntryKind::Directory {
                    for (input_kind, id, label) in [
                        (
                            AgentFilesInputKind::NewFile,
                            "agent-files-menu-new-file",
                            Message::SidebarAgentMenuNewFile,
                        ),
                        (
                            AgentFilesInputKind::NewFolder,
                            "agent-files-menu-new-folder",
                            Message::SidebarAgentMenuNewFolder,
                        ),
                    ] {
                        let mut item = button(id, label);
                        if write_allowed {
                            let directory = path.clone();
                            item = item.on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.agent_files_begin_input(
                                        input_kind,
                                        root_id,
                                        directory.clone(),
                                        None,
                                        window,
                                        cx,
                                    );
                                }),
                            );
                        } else {
                            item = item.text_color(cx.theme().muted_foreground);
                        }
                        actions = actions.child(item);
                    }
                }
                let mut rename = button("agent-files-menu-rename", Message::SidebarAgentMenuRename);
                if write_allowed {
                    let target = path.clone();
                    rename = rename.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, window, cx| {
                            view.agent_files_begin_input(
                                AgentFilesInputKind::Rename,
                                root_id,
                                target
                                    .as_path()
                                    .parent()
                                    .and_then(|parent| {
                                        CanonicalPath::new(parent.to_path_buf()).ok()
                                    })
                                    .unwrap_or_else(|| target.clone()),
                                Some(target.clone()),
                                window,
                                cx,
                            );
                        }),
                    );
                } else {
                    rename = rename.text_color(cx.theme().muted_foreground);
                }
                actions = actions.child(rename);
                let create_targets = runtime.action_targets(&path);
                let delete_targets = create_targets.clone();
                let mut delete_item =
                    button("agent-files-menu-delete", Message::SidebarAgentMenuDelete);
                if write_allowed {
                    delete_item = delete_item.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            view.agent_files_confirm_delete(delete_targets.clone(), cx);
                        }),
                    );
                } else {
                    delete_item = delete_item.text_color(cx.theme().muted_foreground);
                }
                actions = actions.child(delete_item);
                // 复制/剪切属于剪贴板，只要求源可读。
                for (id, label, cut) in [
                    (
                        "agent-files-menu-copy",
                        Message::SidebarAgentMenuCopy,
                        false,
                    ),
                    ("agent-files-menu-cut", Message::SidebarAgentMenuCut, true),
                ] {
                    let targets = create_targets.clone();
                    actions = actions.child(button(id, label).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            view.agent_files_copy_targets(targets.clone(), cut, cx);
                        }),
                    ));
                }
                if clipboard_available {
                    let paste_dir = if kind == FileTreeEntryKind::Directory {
                        Some(path.clone())
                    } else {
                        path.as_path()
                            .parent()
                            .and_then(|parent| CanonicalPath::new(parent.to_path_buf()).ok())
                    };
                    if let Some(directory) = paste_dir {
                        let mut paste =
                            button("agent-files-menu-paste", Message::SidebarAgentMenuPaste);
                        if write_allowed {
                            paste = paste.on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.agent_files_paste(root_id, directory.clone(), window, cx);
                                }),
                            );
                        } else {
                            paste = paste.text_color(cx.theme().muted_foreground);
                        }
                        actions = actions.child(paste);
                    }
                }
                let copy_paths = path.clone();
                actions = actions.child(
                    button(
                        "agent-files-menu-copy-path",
                        Message::SidebarAgentMenuCopyPath,
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            view.agent_files_copy_path(&copy_paths, cx);
                            view.agent_files_close_menu(cx);
                        }),
                    ),
                );
                menu_panel = menu_panel.child(actions);
                if !write_allowed {
                    menu_panel = menu_panel.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .debug_selector(|| "agent-files-menu-write-denied".to_owned())
                            .child(language.text(Message::SidebarAgentMenuWriteDenied)),
                    );
                }
                if !terminal_allowed {
                    menu_panel = menu_panel.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .debug_selector(|| "agent-files-menu-terminal-denied".to_owned())
                            .child(language.text(Message::SidebarAgentMenuTerminalDenied)),
                    );
                }
            }
            Some(AgentFilesMenu::Input(input)) => {
                let title = match input.kind {
                    AgentFilesInputKind::NewFile => Message::SidebarAgentMenuNewFile,
                    AgentFilesInputKind::NewFolder => Message::SidebarAgentMenuNewFolder,
                    AgentFilesInputKind::Rename => Message::SidebarAgentMenuRename,
                };
                let input_entity = input.input.clone();
                menu_panel = menu_panel
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().foreground)
                            .child(language.text(title)),
                    )
                    .child(
                        div()
                            .w(px(220.0))
                            .debug_selector(|| "agent-files-menu-input".to_owned())
                            .child(Input::new(&input_entity)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("agent-files-menu-save")
                                    .debug_selector(|| "agent-files-menu-save".to_owned())
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(cx.theme().accent)
                                    .child(language.text(Message::SidebarAgentConfirm))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, window, cx| {
                                            view.agent_files_commit_input(window, cx);
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("agent-files-menu-cancel")
                                    .debug_selector(|| "agent-files-menu-cancel".to_owned())
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(cx.theme().foreground)
                                    .child(language.text(Message::SidebarAgentCancel))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            view.agent_files_close_menu(cx);
                                        }),
                                    ),
                            ),
                    );
            }
            Some(AgentFilesMenu::ConfirmDelete { targets }) => {
                let count = targets.len().to_string();
                menu_panel = menu_panel
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().foreground)
                            .child(language.text(Message::SidebarAgentConfirmDeleteTitle)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text(Message::SidebarAgentConfirmDeleteBody)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(count),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("agent-files-menu-confirm")
                                    .debug_selector(|| "agent-files-menu-confirm".to_owned())
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(cx.theme().danger)
                                    .child(language.text(Message::SidebarAgentConfirm))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, window, cx| {
                                            view.agent_files_commit_delete(window, cx);
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("agent-files-menu-cancel")
                                    .debug_selector(|| "agent-files-menu-cancel".to_owned())
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(cx.theme().foreground)
                                    .child(language.text(Message::SidebarAgentCancel))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            view.agent_files_close_menu(cx);
                                        }),
                                    ),
                            ),
                    );
            }
            None => {}
        }
        panel = panel.child(menu_panel);
    }

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
                .child(language.text(Message::SidebarAgentOpFailed)),
        );
    }

    for (root_index, (root_id, display_name, expanded, hidden, raw_top_level, rows, status)) in
        roots.into_iter().enumerate()
    {
        let hidden_label = (hidden > 0).then(|| {
            let count = hidden.to_string();
            language.format(Message::SidebarAgentHiddenCount, &[("count", &count)])
        });
        list = list.child(
            div()
                .id(("agent-files-root", root_index))
                .h(px(22.0))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .text_color(cx.theme().foreground)
                .debug_selector(move || format!("agent-files-root-{root_index}"))
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
                ))
                .when_some(hidden_label, |element, label| {
                    element.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    )
                }),
        );
        if !expanded {
            continue;
        }
        let visible_empty = rows.is_empty() && matches!(status, DirectoryStatus::Loaded);
        if visible_empty {
            // 真实空目录不显示过滤文案；只有被过滤掉全部条目时才披露。
            if raw_top_level > 0 {
                list = list.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .debug_selector(move || format!("agent-files-filter-empty-{root_index}"))
                        .child(language.text(Message::SidebarAgentFilterEmpty)),
                );
            }
            continue;
        }
        for (row_index, row) in rows.into_iter().enumerate() {
            let path = row.path.clone();
            let left_path = path.clone();
            let right_path = path.clone();
            let is_selected = selected.as_ref() == Some(&row.path);
            let in_multi = multi_selection.contains(&row.path);
            let kind = row.kind;
            let label = match kind {
                FileTreeEntryKind::Directory => format!("▸ {}", row.name),
                FileTreeEntryKind::File => format!("  {}", row.name),
            };
            let row_name = row.name.clone();
            list = list.child(
                div()
                    .id(("agent-files-row", root_index * 4096 + row_index))
                    .h(px(22.0))
                    .pl(px(16.0 + row.depth as f32 * 12.0))
                    .debug_selector(move || {
                        format!("agent-files-row-{root_index}-{row_index}-{row_name}")
                    })
                    .when(is_selected || in_multi, |element| {
                        element.text_color(cx.theme().foreground)
                    })
                    .when(!is_selected && !in_multi, |element| {
                        element.text_color(cx.theme().muted_foreground)
                    })
                    .when(in_multi, |element| {
                        element.bg(cx.theme().accent.opacity(0.18))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                            if event.modifiers.control || event.modifiers.platform {
                                view.agent_files_toggle_multi(&left_path, cx);
                            } else if event.click_count >= 2 {
                                view.agent_files_open_at(&left_path, window, cx);
                            } else {
                                view.agent_files_activate_path(&left_path, cx);
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, _, _, cx| {
                            view.agent_files_open_menu(root_id, right_path.clone(), kind, cx);
                        }),
                    )
                    .child(label),
            );
        }
    }

    panel.child(list).into_any_element()
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
        roots: Vec<(
            AgentRootId,
            String,
            CanonicalPath,
            AgentRootVisibility,
            AgentRootCapabilities,
        )>,
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
        result: io::Result<super::explorer::DirectoryReadOutcome>,
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
            Ok(outcome) => match root.tree.apply_batch(request, outcome.entries, true) {
                Ok(_) => {
                    runtime.error = None;
                    runtime.truncated = outcome.truncated;
                }
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
            if kind != FileTreeEntryKind::Directory
                || runtime.roots[index].tree.is_expanded(&path)
                || !runtime.roots[index].path_visible(&path)
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
        if runtime.roots[index].tree.kind_of(&absolute).is_some()
            && runtime.roots[index].path_visible(&absolute)
        {
            runtime.selected = Some(absolute);
            runtime.restoring_row = None;
        } else if runtime.roots[index].tree.kind_of(&absolute).is_some() {
            // 隐藏或失效路径不恢复当前行。
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
        runtime.set_menu(None);
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

    pub(super) fn agent_files_toggle_multi(
        &mut self,
        path: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.toggle_multi_selection(path);
        }
        cx.notify();
    }

    /// 行菜单：右键先选中目标；多选命中时整个选择集参与批量动作。
    pub(super) fn agent_files_open_menu(
        &mut self,
        root_id: AgentRootId,
        path: CanonicalPath,
        kind: FileTreeEntryKind,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        if !runtime
            .root_for_id(root_id)
            .is_some_and(|root| root.path_visible(&path))
        {
            return;
        }
        runtime.selected = Some(path.clone());
        if !runtime.multi_selection.contains(&path) {
            runtime.multi_selection.clear();
        }
        runtime.set_menu(Some(AgentFilesMenu::Actions {
            root_id,
            path,
            kind,
        }));
        cx.notify();
    }

    pub(super) fn agent_files_close_menu(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_menu(None);
        }
        cx.notify();
    }

    /// 新建/重命名进入输入态；草稿输入实体只在运行期存活。
    pub(super) fn agent_files_begin_input(
        &mut self,
        kind: AgentFilesInputKind,
        root_id: AgentRootId,
        directory: CanonicalPath,
        target: Option<CanonicalPath>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let initial = target
            .as_ref()
            .and_then(|path| path.as_path().file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(initial, window, cx);
            state
        });
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_menu(Some(AgentFilesMenu::Input(AgentFilesMenuInput {
                kind,
                root_id,
                directory,
                target,
                input,
            })));
        }
        cx.notify();
    }

    /// 显式提交输入：空名、路径分隔符或未改变一律不产生意图。
    pub(super) fn agent_files_commit_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let Some(AgentFilesMenu::Input(menu)) = runtime.menu.take() else {
            return;
        };
        let name = menu.input.read(cx).value().trim().to_owned();
        if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
            runtime.set_notice(Some(crate::i18n::Message::SidebarAgentInvalidName));
            runtime.set_menu(Some(AgentFilesMenu::Input(menu)));
            cx.notify();
            return;
        }
        let target = match CanonicalPath::new(menu.directory.as_path().join(&name)) {
            Ok(target) => target,
            Err(_) => {
                runtime.set_notice(Some(crate::i18n::Message::SidebarAgentInvalidName));
                runtime.set_menu(Some(AgentFilesMenu::Input(menu)));
                cx.notify();
                return;
            }
        };
        let intent = match menu.kind {
            AgentFilesInputKind::NewFile => AgentFilesIntent::CreateFile {
                root_id: menu.root_id,
                directory: menu.directory,
                name,
            },
            AgentFilesInputKind::NewFolder => AgentFilesIntent::CreateFolder {
                root_id: menu.root_id,
                directory: menu.directory,
                name,
            },
            AgentFilesInputKind::Rename => {
                if menu.target.as_ref() == Some(&target) {
                    cx.notify();
                    return;
                }
                AgentFilesIntent::Rename {
                    root_id: menu.root_id,
                    from: menu.target.unwrap_or(target.clone()),
                    to: target,
                }
            }
        };
        runtime.set_notice(None);
        self.agent_files_dispatch(intent, window, cx);
        cx.notify();
    }

    pub(super) fn agent_files_confirm_delete(
        &mut self,
        targets: Vec<CanonicalPath>,
        cx: &mut Context<'_, Self>,
    ) {
        if targets.is_empty() {
            return;
        }
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_menu(Some(AgentFilesMenu::ConfirmDelete { targets }));
        }
        cx.notify();
    }

    pub(super) fn agent_files_commit_delete(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let Some(AgentFilesMenu::ConfirmDelete { targets }) = runtime.menu.take() else {
            return;
        };
        let mut items = Vec::new();
        for path in targets {
            if let Some(root) = runtime.root_for_path(&path) {
                items.push((root.root_id, path));
            }
        }
        if items.is_empty() {
            return;
        }
        runtime.set_notice(None);
        self.agent_files_dispatch(AgentFilesIntent::Delete { items }, window, cx);
        cx.notify();
    }

    /// 复制/剪切只登记应用会话剪贴板；执行时由粘贴链逐项重检授权。
    pub(super) fn agent_files_copy_targets(
        &mut self,
        targets: Vec<CanonicalPath>,
        cut: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        let mut items = Vec::new();
        for path in targets {
            let Some(root) = runtime.root_for_path(&path) else {
                continue;
            };
            let source_root = root.root_id;
            let Some(kind) = root.tree.kind_of(&path) else {
                continue;
            };
            // 多选可跨根，但每项保留自己的根身份；跨根剪切由剪贴板纯转换拒绝。
            items.push(slterm_sidebar::FileClipboardItem {
                root: FileTreeRoot::Agent(source_root),
                path,
                kind,
            });
        }
        if items.is_empty() {
            return;
        }
        let mode = if cut {
            slterm_sidebar::FileClipboardMode::Cut
        } else {
            slterm_sidebar::FileClipboardMode::Copy
        };
        runtime.set_menu(None);
        runtime.set_notice(None);
        let Some(root) = self.root_handle.clone() else {
            return;
        };
        let _ = root.update(cx, |root, cx| {
            root.agent_files_clipboard = Some(slterm_sidebar::FileClipboard::new(mode, items));
            cx.notify();
        });
        cx.notify();
    }

    pub(super) fn agent_files_paste(
        &mut self,
        root_id: AgentRootId,
        directory: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_menu(None);
            runtime.set_notice(None);
        }
        self.agent_files_dispatch(AgentFilesIntent::Paste { root_id, directory }, window, cx);
        cx.notify();
    }

    pub(super) fn agent_files_open_terminal(
        &mut self,
        root_id: AgentRootId,
        cwd: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_menu(None);
        }
        self.agent_files_dispatch(AgentFilesIntent::OpenTerminal { root_id, cwd }, window, cx);
        cx.notify();
    }

    /// 复制绝对路径只写系统剪贴板纯文本，不读取正文。
    pub(super) fn agent_files_copy_path(
        &mut self,
        target: &CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        let targets = self
            .agent_files
            .as_ref()
            .map(|runtime| runtime.action_targets(target))
            .unwrap_or_else(|| vec![target.clone()]);
        let text = targets
            .iter()
            .map(|path| path.as_path().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    }

    /// 操作回投：刷新受影响目录；根外或未加载目录不扩读。
    pub(super) fn agent_files_refresh_paths(
        &mut self,
        paths: &[CanonicalPath],
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.agent_files else {
            return;
        };
        for path in paths {
            let Some(index) = runtime.root_index_for_path(path) else {
                continue;
            };
            if runtime.roots[index].tree.kind_of(path) != Some(FileTreeEntryKind::Directory) {
                continue;
            }
            if let Ok(Some(request)) = runtime.roots[index].tree.refresh(path) {
                let root_id = runtime.roots[index].root_id;
                runtime.pending_reads.push_back((root_id, request));
            }
        }
        self.start_agent_files_reads(cx);
        cx.notify();
    }

    pub(super) fn agent_files_set_notice(
        &mut self,
        notice: Option<crate::i18n::Message>,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(runtime) = &mut self.agent_files {
            runtime.set_notice(notice);
        }
        cx.notify();
    }

    /// 广播结束后再交给 AppRoot，避免快照 reconcile 重入当前实体。
    fn agent_files_dispatch(
        &mut self,
        intent: AgentFilesIntent,
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
                    root.handle_agent_files_intent(intent, window, cx);
                });
            });
        });
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
            runtime.set_menu(None);
        }
        self.start_agent_files_reads(cx);
        cx.notify();
    }

    /// 应用级根同步入口：AppRoot 提供根列表后由本实体登记读取。
    pub(super) fn agent_files_sync(
        &mut self,
        roots: Vec<(
            AgentRootId,
            String,
            CanonicalPath,
            AgentRootVisibility,
            AgentRootCapabilities,
        )>,
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
            .map(|(root_id, _, path, _, _)| (*root_id, path.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        if previous_paths != next_paths {
            crate::app_shell::authorization::advance_generation();
        }
        // 配置中消失的根按撤销处理：运行态与持久分区一并清理，新根按新身份登记。
        let next = roots
            .iter()
            .map(|(root_id, _, _, _, _)| *root_id)
            .collect::<BTreeSet<_>>();
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
        if let Some(runtime) = &mut self.agent_files {
            runtime.prune_hidden();
        }
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

    pub(super) fn ensure_agent_files_filter_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self
            .agent_files
            .as_ref()
            .is_none_or(|runtime| runtime.filter_input.is_some())
        {
            return;
        }
        let placeholder = cx
            .global::<crate::app_shell::config::Settings>()
            .language
            .text(Message::SidebarAgentFilterPlaceholder)
            .to_owned();
        let input = cx
            .new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(placeholder));
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

    /// 根层可见条目名；与渲染共用同一可见性投影。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_visible_entries_for_test(&self, root_id: AgentRootId) -> Vec<String> {
        let Some(runtime) = self.agent_files.as_ref() else {
            return Vec::new();
        };
        let Some(root) = runtime.root_for_id(root_id) else {
            return Vec::new();
        };
        root.tree
            .visible_rows()
            .into_iter()
            .filter(|row| row.path.as_path().parent() == Some(root.path.as_path()))
            .filter(|row| root.visibility.allows_entry(&row.name))
            .map(|row| row.name)
            .collect()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_menu_input_for_test(&self) -> Option<gpui::Entity<InputState>> {
        match self.agent_files.as_ref()?.menu.as_ref()? {
            AgentFilesMenu::Input(input) => Some(input.input.clone()),
            _ => None,
        }
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn agent_files_notice_for_test(&self) -> Option<crate::i18n::Message> {
        self.agent_files.as_ref()?.notice
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
