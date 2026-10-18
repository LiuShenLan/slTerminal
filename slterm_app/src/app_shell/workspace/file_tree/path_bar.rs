//! 路径栏(07 篇照抄点 24):路径框只拥有草稿,导航仍走文件树模型单入口。
//!
//! 提交语义:
//! - 磁盘检查只在**提交时**在后台执行;输入与绘制不触碰文件系统;
//! - `~` 展开、相对路径按当前树根解析、UNC `\\?\` 前缀规范化显示;
//! - 提交期间根/编辑器身份变了 → 丢弃旧结果,不得改动新的浏览上下文。
//!
//! 补全:设计稿无补全合同(上游 PathEditor 只有草稿/提交/校验),
//! 本步按签名稿实现,不另造建议弹层。

use std::io;
use std::path::{Path, PathBuf};

use gpui::{
    Context, Entity, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    ScrollStrategy, Styled as _, Subscription, Task, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{IconName, Sizable as _};

use super::*;
use crate::app_shell::config::Settings;
use crate::i18n::Message;

/// 编辑器创建时的浏览origin:根身份变了,旧提交结果必须丢弃。
#[derive(Clone, PartialEq)]
pub(super) struct PathOrigin {
    root: Option<PathBuf>,
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
            root: self.file_tree.panel.root().map(Path::to_path_buf),
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
                        let already_open = this.file_tree.panel.root() == Some(path.as_path());
                        let changed = this.file_tree.panel.browse_directory(path);
                        if changed || already_open {
                            this.file_tree.panel.set_file_search_query(String::new());
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
}
