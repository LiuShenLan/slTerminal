//! 路径栏(照上游同构):路径框只拥有草稿,导航仍走文件树模型单入口。
//!
//! 提交语义:
//! - 磁盘检查只在**提交时**在后台执行;输入与绘制不触碰文件系统;
//! - `~` 展开、相对路径按当前树根解析、UNC `\\?\` 前缀规范化显示;
//! - 提交期间根/编辑器身份变了 → 丢弃旧结果,不得改动新的浏览上下文。
//!
//! 补全:设计稿无补全合同(上游 PathEditor 只有草稿/提交/校验),
//! 本模块按签名稿实现,不另造建议弹层。

use std::io;
use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    ScrollStrategy, Styled as _, Subscription, Task, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _};

use super::*;
use crate::app_shell::config::Settings;
use crate::i18n::Message;
use crate::platform::file_operations;

/// 编辑器创建时的浏览origin:根身份变了,旧提交结果必须丢弃。
#[derive(Clone, PartialEq)]
pub(super) struct PathOrigin {
    root: Option<PathBuf>,
}

/// 名字编辑的目标操作:新建落在目录内,重命名作用于旧路径。
#[derive(Clone, Debug)]
pub(crate) enum NameEditKind {
    NewFile { directory: PathBuf },
    NewFolder { directory: PathBuf },
    Rename { from: PathBuf },
}

impl NameEditKind {
    fn target(&self) -> &Path {
        match self {
            Self::NewFile { directory } | Self::NewFolder { directory } => directory,
            Self::Rename { from } => from,
        }
    }

    fn is_rename(&self) -> bool {
        matches!(self, Self::Rename { .. })
    }
}

/// 名字编辑器的身份守卫:树根或目标变了,旧校验结果必须丢弃。
#[derive(Clone, PartialEq)]
pub(super) struct NameOrigin {
    root: Option<PathBuf>,
    target: PathBuf,
}

/// 一次名字提交的失败分类;错误消息在渲染侧按当前语言落地。
#[derive(Debug)]
pub(super) enum NameSubmitError {
    Empty,
    Invalid,
    Exists,
    Io(io::Error),
}

impl NameSubmitError {
    fn from_io(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::AlreadyExists => Self::Exists,
            io::ErrorKind::InvalidInput => Self::Invalid,
            _ => Self::Io(error),
        }
    }
}

/// 文件树右键 CRUD 的名字输入草稿(交互形态与 [`PathEditor`] 同构):
/// 只拥有 `InputState` 与校验态,磁盘操作在提交时进后台线程。
pub(crate) struct NameEditor {
    pub(super) input: Entity<InputState>,
    pub(super) kind: NameEditKind,
    pub(super) origin: NameOrigin,
    /// 最近一次提交失败的用户可读消息。
    pub(super) error: Option<String>,
    /// 后台校验/执行在途;`Task` 必须存活,drop 即取消。
    pub(super) pending: Option<Task<()>>,
    _subscription: Subscription,
}

/// 后台执行一次名字提交:名字校验 + 磁盘操作都离开 GPUI 前台。
fn apply_name_edit(kind: &NameEditKind, raw_name: &str) -> Result<PathBuf, NameSubmitError> {
    let name = raw_name.trim();
    if name.is_empty() {
        return Err(NameSubmitError::Empty);
    }
    file_operations::validate_entry_name(name).map_err(|_| NameSubmitError::Invalid)?;
    match kind {
        NameEditKind::NewFile { directory } => {
            let path = directory.join(name);
            file_operations::create_file(&path).map_err(NameSubmitError::from_io)?;
            Ok(path)
        }
        NameEditKind::NewFolder { directory } => {
            let path = directory.join(name);
            file_operations::create_directory(&path).map_err(NameSubmitError::from_io)?;
            Ok(path)
        }
        NameEditKind::Rename { from } => {
            let parent = from.parent().ok_or_else(|| {
                NameSubmitError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Rename source has no parent directory",
                ))
            })?;
            let to = parent.join(name);
            // 原名提交是幂等 no-op,不必碰磁盘。
            if to == *from {
                return Ok(to);
            }
            file_operations::rename_path(from, &to).map_err(NameSubmitError::from_io)?;
            Ok(to)
        }
    }
}

/// 路径栏编辑草稿。只拥有 `InputState` 与校验态,不持有树状态。
pub(crate) struct PathEditor {
    input: Entity<InputState>,
    origin: PathOrigin,
    /// 最近一次提交失败的用户可读消息(路径不可用 / IO 错误)。
    error: Option<String>,
    /// 后台校验在途;`Task` 必须存活,drop 即取消。
    pending: Option<Task<()>>,
    _subscription: Subscription,
}

/// 规范化显示:去掉 Windows 扩展长度前缀。
///
/// `\\?\UNC\server\share` → `\\server\share`;`\\?\D:\x` → `D:\x`。
pub(super) fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
    }
}

/// 解析一个目录路径;`home` 显式传入(生产 = `USERPROFILE`,测试隔离)。
pub(super) fn resolve_directory_in(
    origin_root: Option<&Path>,
    text: &str,
    home: Option<&Path>,
) -> io::Result<PathBuf> {
    let text = text.trim();
    let text = text
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(text);
    if text.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty directory path",
        ));
    }
    let path = if text == "~" || text.starts_with("~/") || text.starts_with("~\\") {
        home.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory unavailable"))?
            .join(text.get(2..).unwrap_or(""))
    } else {
        PathBuf::from(text)
    };
    let path = if path.is_absolute() {
        path
    } else {
        origin_root
            .map(Path::to_path_buf)
            .or_else(|| home.map(Path::to_path_buf))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Enter an absolute directory path")
            })?
            .join(path)
    };
    if !std::fs::metadata(&path)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "Not a directory",
        ));
    }
    // 本地路径先消解 `..` 与链接,避免后续「返回上级」沿未规范化的拼写走错目录。
    Ok(PathBuf::from(display_path(&path.canonicalize()?)))
}

impl SltermWorkspace {
    fn file_tree_path_origin(&self) -> PathOrigin {
        PathOrigin {
            root: self.side_panel.root().map(Path::to_path_buf),
        }
    }

    pub(super) fn begin_file_tree_path_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let origin = self.file_tree_path_origin();
        let value = origin.root.as_deref().map(display_path).unwrap_or_default();
        let language = cx.global::<Settings>().language;
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder(language.text(Message::FilesPathPlaceholder));
            input.set_value(value.clone(), window, cx);
            input.set_selected_range(0..value.len(), cx);
            input
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.commit_file_tree_path(window, cx),
                InputEvent::Change => {
                    if let Some(edit) = this.file_tree.path_editor.as_mut() {
                        edit.error = None;
                    }
                    cx.notify();
                }
                _ => {}
            });
        self.file_tree.path_editor = Some(PathEditor {
            input: input.clone(),
            origin,
            error: None,
            pending: None,
            _subscription: subscription,
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(super) fn cancel_file_tree_path_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.file_tree.path_editor = None;
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(super) fn commit_file_tree_path(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self
            .file_tree
            .path_editor
            .as_ref()
            .filter(|editor| editor.pending.is_none())
        else {
            return;
        };
        let input = editor.input.clone();
        let origin = editor.origin.clone();
        let text = input.read(cx).value().to_string();
        let checked_origin = origin.clone();
        // 磁盘检查只在提交时后台执行。
        let check = cx.background_executor().spawn(async move {
            resolve_directory_in(
                checked_origin.root.as_deref(),
                &text,
                crate::platform::dirs::home_dir().as_deref(),
            )
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = check.await;
            let _ = this.update_in(cx, |this, window, cx| {
                // 关闭、取消、切换目录后,旧校验不得改变新的浏览上下文。
                if !this
                    .file_tree
                    .path_editor
                    .as_ref()
                    .is_some_and(|editor| editor.input == input)
                {
                    return;
                }
                if this
                    .file_tree
                    .path_editor
                    .as_ref()
                    .map(|editor| &editor.origin)
                    != Some(&origin)
                {
                    this.file_tree.path_editor = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(path) => {
                        let already_open = this.side_panel.root() == Some(path.as_path());
                        let changed = this.side_panel.browse_directory(path);
                        if changed || already_open {
                            this.side_panel.set_file_search_query(String::new());
                            this.file_tree
                                .search_input
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.file_tree.scroll.scroll_to_item(0, ScrollStrategy::Top);
                            this.file_tree.path_editor = None;
                            this.focus_active(window, cx);
                        } else if let Some(editor) = this.file_tree.path_editor.as_mut() {
                            editor.pending = None;
                            let language = cx.global::<Settings>().language;
                            editor.error =
                                Some(language.text(Message::FilesPathUnavailable).to_owned());
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = this.file_tree.path_editor.as_mut() {
                            editor.pending = None;
                            let language = cx.global::<Settings>().language;
                            editor.error = Some(language.format(
                                Message::FilesPathFailed,
                                &[("error", &error.to_string())],
                            ));
                        }
                    }
                }
                cx.notify();
            });
        });
        self.file_tree.path_editor.as_mut().unwrap().pending = Some(task);
        cx.notify();
    }

    /// 路径栏:只读按钮(点击进入编辑)或编辑态(输入 + 打开/取消)。
    pub(super) fn render_file_tree_path(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let origin = self.file_tree_path_origin();
        if self
            .file_tree
            .path_editor
            .as_ref()
            .is_some_and(|editor| editor.origin != origin)
        {
            self.file_tree.path_editor = None;
        }
        let language = cx.global::<Settings>().language;
        let Some(editor) = self.file_tree.path_editor.as_ref() else {
            let value = origin
                .root
                .as_deref()
                .map(display_path)
                .unwrap_or_else(|| language.text(Message::FilesEmptyNoRoot).to_owned());
            return Button::new("file-tree-path")
                .debug_selector(|| "file-tree-path".to_owned())
                .ghost()
                .small()
                .h(px(28.0))
                .w_full()
                .min_w_0()
                .px_1()
                .overflow_hidden()
                .tooltip(language.text(Message::FilesPathEdit))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_left()
                        .text_xs()
                        .child(value),
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.begin_file_tree_path_edit(window, cx)),
                )
                .into_any_element();
        };
        let busy = editor.pending.is_some();
        let input = editor.input.clone();
        div()
            .id("file-tree-path-editor")
            .debug_selector(|| "file-tree-path-editor".to_owned())
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            // 单行输入框会冒泡 Enter;提交动作必须在这里结束,避免换行回写
            // 清空全选路径。
            .on_action(|_: &gpui_component::input::Enter, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    cx.stop_propagation();
                    this.cancel_file_tree_path_edit(window, cx);
                }
            }))
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&input)
                        .h(px(28.0))
                        .text_size(px(12.0))
                        .disabled(busy),
                ),
            )
            .child(
                Button::new("file-tree-path-go")
                    .icon(IconName::Check)
                    .ghost()
                    .xsmall()
                    .disabled(busy)
                    .tooltip(language.text(Message::FilesPathOpen))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.commit_file_tree_path(window, cx)),
                    ),
            )
            .child(
                Button::new("file-tree-path-cancel")
                    .icon(IconName::Close)
                    .ghost()
                    .xsmall()
                    .tooltip(language.text(Message::EditorCancel))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.cancel_file_tree_path_edit(window, cx)
                    })),
            )
            .into_any_element()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn file_tree_path_error(&self) -> Option<String> {
        self.file_tree
            .path_editor
            .as_ref()
            .and_then(|editor| editor.error.clone())
    }
}

impl SltermWorkspace {
    /// 进入名字编辑态(右键新建/重命名);同一时刻只保留一个草稿。
    pub(super) fn begin_file_tree_name_edit(
        &mut self,
        kind: NameEditKind,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.file_tree.context_menu = None;
        self.file_tree.notice = None;
        let origin = NameOrigin {
            root: self.side_panel.root().map(Path::to_path_buf),
            target: kind.target().to_path_buf(),
        };
        let language = cx.global::<Settings>().language;
        let (placeholder, initial) = match &kind {
            NameEditKind::NewFile { .. } => (Message::FilesNewFilePlaceholder, String::new()),
            NameEditKind::NewFolder { .. } => (Message::FilesNewFolderPlaceholder, String::new()),
            NameEditKind::Rename { from } => (
                Message::FilesRenamePlaceholder,
                from.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
        };
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(language.text(placeholder));
            if !initial.is_empty() {
                let length = initial.len();
                input.set_value(initial, window, cx);
                input.set_selected_range(0..length, cx);
            }
            input
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.commit_file_tree_name(window, cx),
                InputEvent::Change => {
                    // 改名字即清掉旧错误,避免错误提示压在已修改的草稿上。
                    if let Some(editor) = this.file_tree.name_editor.as_mut() {
                        editor.error = None;
                    }
                    cx.notify();
                }
                _ => {}
            });
        self.file_tree.name_editor = Some(NameEditor {
            input: input.clone(),
            kind,
            origin,
            error: None,
            pending: None,
            _subscription: subscription,
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(super) fn cancel_file_tree_name_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.file_tree.name_editor = None;
        self.focus_active(window, cx);
        cx.notify();
    }

    /// 提交名字编辑:校验/磁盘操作在后台线程,结果回前台后守卫身份。
    pub(super) fn commit_file_tree_name(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(editor) = self
            .file_tree
            .name_editor
            .as_ref()
            .filter(|editor| editor.pending.is_none())
        else {
            return;
        };
        let input = editor.input.clone();
        let kind = editor.kind.clone();
        let origin = editor.origin.clone();
        let name = input.read(cx).value().to_string();
        // 重命名不得静默留下指向旧路径的已打开 tab。编辑器路径更新 API
        // 尚未开放(跨 file_editor 模块),这里安全拒绝并给出可读错误。
        if let NameEditKind::Rename { from } = &kind
            && self.file_tree_path_is_open(from, cx)
        {
            let language = cx.global::<Settings>().language;
            if let Some(editor) = self.file_tree.name_editor.as_mut() {
                editor.error = Some(language.text(Message::FilesRenameOpenTab).to_owned());
            }
            cx.notify();
            return;
        }
        let is_rename = kind.is_rename();
        let check = cx
            .background_executor()
            .spawn(async move { apply_name_edit(&kind, &name) });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = check.await;
            let _ = this.update_in(cx, |this, window, cx| {
                // 关闭、取消、重新打开后,旧校验不得写进新的编辑上下文。
                if !this
                    .file_tree
                    .name_editor
                    .as_ref()
                    .is_some_and(|editor| editor.input == input)
                {
                    return;
                }
                if this
                    .file_tree
                    .name_editor
                    .as_ref()
                    .map(|editor| &editor.origin)
                    != Some(&origin)
                {
                    this.file_tree.name_editor = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(path) => {
                        this.file_tree.name_editor = None;
                        this.file_tree.notice = None;
                        this.side_panel.selected = Some(path.clone());
                        // 搜索态下新条目不在平铺结果里;清过滤词让树形结果
                        // 回到可定位状态,再展开父目录。
                        if !this.side_panel.search.trim().is_empty() {
                            this.side_panel.set_file_search_query(String::new());
                            this.file_tree
                                .search_input
                                .update(cx, |input, cx| input.set_value("", window, cx));
                        }
                        this.reveal_file_tree_parent(&path);
                        this.side_panel.request_refresh();
                        this.sync_file_tree(cx);
                        this.focus_active(window, cx);
                    }
                    Err(error) => {
                        let language = cx.global::<Settings>().language;
                        let message = match error {
                            NameSubmitError::Empty => {
                                language.text(Message::FilesNameEmpty).to_owned()
                            }
                            NameSubmitError::Invalid => {
                                language.text(Message::FilesNameInvalid).to_owned()
                            }
                            NameSubmitError::Exists => {
                                language.text(Message::FilesNameExists).to_owned()
                            }
                            NameSubmitError::Io(error) => {
                                let template = if is_rename {
                                    Message::FilesRenameFailed
                                } else {
                                    Message::FilesCreateFailed
                                };
                                language.format(template, &[("error", &error.to_string())])
                            }
                        };
                        if let Some(editor) = this.file_tree.name_editor.as_mut() {
                            editor.pending = None;
                            editor.error = Some(message);
                        }
                    }
                }
                cx.notify();
            });
        });
        self.file_tree.name_editor.as_mut().unwrap().pending = Some(task);
        cx.notify();
    }

    /// 名字编辑行:Enter 提交、Escape 取消、Change 清错;忙碌时输入禁用。
    pub(super) fn render_file_tree_name_editor(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        let root = self.side_panel.root().map(Path::to_path_buf);
        if self
            .file_tree
            .name_editor
            .as_ref()
            .is_some_and(|editor| editor.origin.root != root)
        {
            self.file_tree.name_editor = None;
        }
        let editor = self.file_tree.name_editor.as_ref()?;
        let theme = cx.theme();
        let language = cx.global::<Settings>().language;
        let busy = editor.pending.is_some();
        let input = editor.input.clone();
        let error = editor.error.clone();
        Some(
            div()
                .id("file-tree-name-editor")
                .debug_selector(|| "file-tree-name-editor".to_owned())
                .w_full()
                .min_w_0()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_1()
                        // 单行输入框会冒泡 Enter;提交动作必须在这里结束。
                        .on_action(|_: &gpui_component::input::Enter, _, cx| cx.stop_propagation())
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                cx.stop_propagation();
                                this.cancel_file_tree_name_edit(window, cx);
                            }
                        }))
                        .child(
                            div().flex_1().min_w_0().child(
                                Input::new(&input)
                                    .h(px(28.0))
                                    .text_size(px(12.0))
                                    .disabled(busy),
                            ),
                        )
                        .child(
                            Button::new("file-tree-name-go")
                                .debug_selector(|| "file-tree-name-go".to_owned())
                                .icon(IconName::Check)
                                .ghost()
                                .xsmall()
                                .disabled(busy)
                                .tooltip(language.text(Message::FilesNameConfirm))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.commit_file_tree_name(window, cx)
                                })),
                        )
                        .child(
                            Button::new("file-tree-name-cancel")
                                .debug_selector(|| "file-tree-name-cancel".to_owned())
                                .icon(IconName::Close)
                                .ghost()
                                .xsmall()
                                .tooltip(language.text(Message::EditorCancel))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_file_tree_name_edit(window, cx)
                                })),
                        ),
                )
                .when_some(error, |column, error| {
                    column.child(
                        div()
                            .px(px(DRAWER_TEXT_INSET))
                            .text_xs()
                            .text_color(theme.danger)
                            .child(error),
                    )
                })
                .into_any_element(),
        )
    }

    /// 目标路径是否已被任一 Code tab 持有(普通源码/gitshow/diff 都算)。
    /// Windows 路径大小写不敏感,按 ASCII 大小写无关比较兜住盘符/拼写差异;
    /// 不做 canonicalize(前台禁阻塞 I/O)。
    fn file_tree_path_is_open(&self, path: &Path, cx: &gpui::App) -> bool {
        let expected = path.to_string_lossy();
        self.tabs.iter().any(|entry| match &entry.tab {
            WorkspaceTab::Code { view } => {
                let open = view.read(cx).path.to_string_lossy();
                open.eq_ignore_ascii_case(&expected)
            }
            _ => false,
        })
    }
}

#[cfg(test)]
mod path_bar_tests {
    use super::*;

    /// UNC `\\?\` 前缀规范化:共享路径回到 `\\server\share`,盘符路径回到 `X:\`。
    #[test]
    fn display_path_strips_windows_extended_length_prefixes() {
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\dir")),
            r"\\server\share\dir"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\D:\work\repo")),
            r"D:\work\repo"
        );
        assert_eq!(display_path(Path::new(r"D:\plain")), r"D:\plain");
    }

    /// `~` 展开与相对路径解析都走显式注入的 home,测试不碰真实用户目录。
    #[test]
    fn resolve_expands_home_and_relative_paths() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let work = home.join("work");
        std::fs::create_dir_all(&work).unwrap();

        assert_eq!(
            resolve_directory_in(None, "~", Some(home.as_path())).unwrap(),
            PathBuf::from(display_path(
                &work.parent().unwrap().canonicalize().unwrap()
            ))
        );
        assert_eq!(
            resolve_directory_in(None, "~/work", Some(home.as_path())).unwrap(),
            PathBuf::from(display_path(&work.canonicalize().unwrap()))
        );
        assert_eq!(
            resolve_directory_in(Some(home.as_path()), "work", Some(home.as_path())).unwrap(),
            PathBuf::from(display_path(&work.canonicalize().unwrap()))
        );
    }

    /// 引号包裹、非目录、空串与不存在的路径分别拒绝。
    #[test]
    fn resolve_rejects_empty_non_directories_and_missing_paths() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("file.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(
            resolve_directory_in(None, "", None)
                .expect_err("空串")
                .to_string()
                .contains("Empty")
        );
        assert!(
            resolve_directory_in(None, &file.to_string_lossy(), None)
                .expect_err("文件不是目录")
                .to_string()
                .contains("Not a directory")
        );
        assert!(resolve_directory_in(None, r"D:\does\not\exist\slterm", None).is_err());
        // 引号包裹与首尾空白按粘贴习惯宽容处理。
        assert_eq!(
            resolve_directory_in(None, &format!("\"{}\"", directory.path().display()), None)
                .unwrap(),
            PathBuf::from(display_path(&directory.path().canonicalize().unwrap()))
        );
    }

    /// 后台提交:新建/重命名真实落盘,失败分类可区分。
    #[test]
    fn apply_name_edit_creates_and_renames_entries() {
        let directory = tempfile::tempdir().unwrap();

        let file = apply_name_edit(
            &NameEditKind::NewFile {
                directory: directory.path().to_path_buf(),
            },
            "  created.txt  ",
        )
        .unwrap();
        assert_eq!(file, directory.path().join("created.txt"));
        assert!(file.is_file(), "首尾空白按名字裁剪");

        let folder = apply_name_edit(
            &NameEditKind::NewFolder {
                directory: directory.path().to_path_buf(),
            },
            "created_dir",
        )
        .unwrap();
        assert!(folder.is_dir());

        let renamed =
            apply_name_edit(&NameEditKind::Rename { from: file.clone() }, "renamed.txt").unwrap();
        assert_eq!(renamed, directory.path().join("renamed.txt"));
        assert!(!file.exists());
        assert!(renamed.is_file());
    }

    /// 空名/非法名/同名已存在分别给出可判别错误,且不落盘。
    #[test]
    fn apply_name_edit_rejects_empty_invalid_and_existing_names() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("taken.txt");
        std::fs::write(&existing, b"x").unwrap();
        let kind = NameEditKind::NewFile {
            directory: directory.path().to_path_buf(),
        };

        assert!(matches!(
            apply_name_edit(&kind, "   ").unwrap_err(),
            NameSubmitError::Empty
        ));
        assert!(matches!(
            apply_name_edit(&kind, "bad*name").unwrap_err(),
            NameSubmitError::Invalid
        ));
        assert!(matches!(
            apply_name_edit(&kind, "taken.txt").unwrap_err(),
            NameSubmitError::Exists
        ));
        assert_eq!(
            std::fs::read(&existing).unwrap(),
            b"x",
            "失败提交不得覆盖既有文件"
        );

        // 重命名到已存在目标同样拒绝,原名保持不动。
        let source = directory.path().join("source.txt");
        std::fs::write(&source, b"s").unwrap();
        assert!(matches!(
            apply_name_edit(
                &NameEditKind::Rename {
                    from: source.clone()
                },
                "taken.txt"
            )
            .unwrap_err(),
            NameSubmitError::Exists
        ));
        assert!(source.is_file());
    }
}
