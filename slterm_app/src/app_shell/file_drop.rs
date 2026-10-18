//! 文件树 → 终端 / 系统的拖放契约(照上游同构)。
//!
//! 载荷住在文件树与终端都能引用的地方:文件树
//! (`workspace::file_tree`)产生它,终端 pane 容器消费它,两边互相看不见
//! 对方的私有模块。
//!
//! GPUI 拖放引擎原生管理按压阈值/指针捕获/落点命中,所以本模块的载荷是
//! **纯数据**;「路径写进 PTY 的规则」(拒绝控制字符、含空白加引号、尾随
//! 空格)在 `side_panel::drop_text_for_path`,与旧壳同一合同。
//!
//! 拖出窗口 → 系统文件拖放(把行拖到 Explorer/桌面)由
//! [`ExternalDragPayload::Files`] 承载,平台侧渲染系统图标。

use gpui::{
    Context, ExternalDragPayload, FileDragPaths, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;

/// 一次从文件树拖向终端/系统的条目。
#[derive(Clone, Debug)]
pub(crate) struct FileTreeDrag {
    /// 宿主文件系统身份(系统文件拖放与文件传输用)。
    pub local_path: std::path::PathBuf,
    /// 落进 PTY 的路径**原文**(shell 拼写,可能与 `local_path` 的展示形态不同)。
    pub path_text: String,
    /// 跟着指针走的标签。
    pub name: String,
    /// 系统文件拖放需要目录元数据;在拖起时捕获,避免平台反问文件系统。
    pub is_dir: bool,
}

impl FileTreeDrag {
    /// 系统文件拖放载荷:真实磁盘路径 + 目录标记。
    pub(crate) fn external_payload(&self) -> ExternalDragPayload {
        ExternalDragPayload::Files(FileDragPaths::new([(self.local_path.clone(), self.is_dir)]))
    }
}

/// 跟随指针的拖动预览。
///
/// `on_drag` 要一个 `Entity<impl Render>` 而非普通元素——预览独立于源元素
/// 生命周期:拖行过程中源行可能已经因树刷新而不存在。
pub(crate) struct FileDragGhost {
    name: String,
}

impl FileDragGhost {
    pub(crate) fn new(name: String) -> Self {
        Self { name }
    }
}

impl Render for FileDragGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.foreground)
            .text_sm()
            .max_w(px(320.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .child(self.name.clone())
    }
}

#[cfg(test)]
mod file_drop_tests {
    use super::*;
    use gpui::ExternalDragPayload;

    /// 载荷纯数据:宿主路径、PTY 原文、名称、目录位各自独立保存。
    #[test]
    fn payload_keeps_host_path_shell_text_and_directory_bit() {
        let drag = FileTreeDrag {
            local_path: std::path::PathBuf::from(r"D:\项目 空间\a.txt"),
            path_text: "D:\\项目 空间\\a.txt".to_owned(),
            name: "a.txt".to_owned(),
            is_dir: false,
        };
        assert_eq!(drag.name, "a.txt");
        assert!(!drag.is_dir);
        assert_eq!(drag.path_text, "D:\\项目 空间\\a.txt");
        assert_eq!(
            drag.local_path,
            std::path::PathBuf::from(r"D:\项目 空间\a.txt")
        );

        match drag.external_payload() {
            ExternalDragPayload::Files(paths) => {
                assert_eq!(
                    paths.entries(),
                    &[(std::path::PathBuf::from(r"D:\项目 空间\a.txt"), false)]
                );
            }
        }
    }

    /// 幽灵预览只持有名称:源行消失后仍可独立渲染。
    #[test]
    fn ghost_is_independent_from_the_source_row() {
        let ghost = FileDragGhost::new("gone.txt".to_owned());
        assert_eq!(ghost.name, "gone.txt");
    }
}
