//! 文件打开路由与文件 tab 生命周期(07 篇 M8.1)。
//!
//! 路由单判定在 `doc_tabs::openable_in_app`;本模块只做「判定结果 → tab 变体」
//! 的落地与同路径重开命中:
//!
//! ```text
//! 图片 → WorkspaceTab::Image(BGRA 解码 + 占位缩放)
//! markdown → WorkspaceTab::Document(只读文本占位;渲染态归 M8.3)
//! 可映射源码 → WorkspaceTab::Code(只读;可编辑态归 M8.2)
//! 未命中 → 交系统处理器(M8.4 file_tree 入口裁定)
//! ```
//!
//! 三个文件 tab 都不进 session 快照(快照只收 Terminal tab,05 篇合同),
//! 关闭只走统一的 `close_tab_at`——纯视图 drop,无 PTY/进程生命周期。
//! gitshow 只读臂复用 `WorkspaceTab::Code`,数据源是 HEAD blob。

use std::path::PathBuf;

use gpui::{AppContext as _, Context, Entity, Window};
use slterm_split::TabId;

use super::{SltermWorkspace, TabEntry, TabMeta, WorkspaceTab};
use crate::app_shell::code_tab::CodeTabView;
use crate::app_shell::doc_tabs::{self, DocTabView, ImageTabView};

/// 变体取视图的单点小工具:重开命中的激活路径只关心视图实体。
fn code_view(entry: &TabEntry) -> Option<&Entity<CodeTabView>> {
    match &entry.tab {
        WorkspaceTab::Code { view } => Some(view),
        _ => None,
    }
}

fn document_view(entry: &TabEntry) -> Option<&Entity<DocTabView>> {
    match &entry.tab {
        WorkspaceTab::Document { view } => Some(view),
        _ => None,
    }
}

fn image_view(entry: &TabEntry) -> Option<&Entity<ImageTabView>> {
    match &entry.tab {
        WorkspaceTab::Image { view } => Some(view),
        _ => None,
    }
}

impl SltermWorkspace {
    /// 文件打开路由(07 篇「打开路由」数据流)。
    #[allow(dead_code)] // 消费点 = M8.4 file_tree 双击入口;M8.1 出口用例经此直调。
    pub(crate) fn open_document_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 单判定门:应用内不能读的扩展名不产生 tab(交系统处理器是
        // M8.4 文件树入口的动作,路由层不静默执行)。
        if !doc_tabs::openable_in_app(&path) {
            return;
        }
        if doc_tabs::image_viewable(&path) {
            self.open_image_tab(path, window, cx);
        } else if doc_tabs::markdown_viewable(&path) {
            self.open_doc_tab(path, window, cx);
        } else {
            self.open_code_tab(path, window, cx);
        }
    }

    /// gitshow 只读臂:HEAD 字节 → Code tab(标题 `<name> @ HEAD`)。
    #[allow(dead_code)] // 消费点 = M8.5 gutter/diff 入口;M8.1 出口用例经此直调。
    pub(crate) fn open_git_show_tab(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(index) = self.tabs.iter().position(|entry| {
            matches!(&entry.tab, WorkspaceTab::Code { view } if view.read(cx).is_git_show_path(&path))
        }) {
            if let Some(view) = self.tabs.get(index).and_then(code_view) {
                view.clone()
                    .update(cx, |view, cx| view.reload(window, cx));
            }
            self.activate_tab(index, window, cx);
            return;
        }
        let view = cx.new(|cx| CodeTabView::new_git_show(path, window, cx));
        self.push_file_tab(WorkspaceTab::Code { view }, window, cx);
    }

    /// 同一路径复用已开 tab(激活 + 重读盘;草稿保留归 M8.2)。
    fn open_image_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(index) = self.tabs.iter().position(
            |entry| matches!(&entry.tab, WorkspaceTab::Image { view } if view.read(cx).path == path),
        ) {
            if let Some(view) = self.tabs.get(index).and_then(image_view) {
                view.clone().update(cx, |view, cx| view.reload(cx));
            }
            self.activate_tab(index, window, cx);
            return;
        }
        let view = cx.new(|cx| ImageTabView::new(path, cx));
        self.push_file_tab(WorkspaceTab::Image { view }, window, cx);
    }

    fn open_doc_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(index) = self.tabs.iter().position(
            |entry| matches!(&entry.tab, WorkspaceTab::Document { view } if view.read(cx).path == path),
        ) {
            if let Some(view) = self.tabs.get(index).and_then(document_view) {
                view.clone()
                    .update(cx, |view, cx| view.reload(window, cx));
            }
            self.activate_tab(index, window, cx);
            return;
        }
        // M8.1 预览占位:与 Code 共用只读文本视图;渲染态归 M8.3。
        let view = cx.new(|cx| DocTabView::new(path, "markdown", true, window, cx));
        self.push_file_tab(WorkspaceTab::Document { view }, window, cx);
    }

    fn open_code_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(index) = self.tabs.iter().position(|entry| {
            matches!(&entry.tab, WorkspaceTab::Code { view } if view.read(cx).is_regular_path(&path))
        }) {
            if let Some(view) = self.tabs.get(index).and_then(code_view) {
                view.clone()
                    .update(cx, |view, cx| view.reload(window, cx));
            }
            self.activate_tab(index, window, cx);
            return;
        }
        let view = cx.new(|cx| CodeTabView::new(path, window, cx));
        self.push_file_tab(WorkspaceTab::Code { view }, window, cx);
    }

    /// 文件 tab 的统一入列点:分配稳定 TabId、激活、同步窗口标题。
    fn push_file_tab(
        &mut self,
        tab: WorkspaceTab,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let tab_id = TabId(self.next_tab);
        self.next_tab = self.next_tab.saturating_add(1);
        self.tabs.push(TabEntry {
            id: tab_id,
            tab,
            meta: TabMeta::default(),
        });
        self.active = self.tabs.len() - 1;
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 测试专用:活跃 tab 的封闭变体短名(terminal/settings/document/code/image)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_tab_kind_for_test(&self) -> &'static str {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Terminal { .. }) => "terminal",
            Some(WorkspaceTab::Settings { .. }) => "settings",
            Some(WorkspaceTab::Document { .. }) => "document",
            Some(WorkspaceTab::Code { .. }) => "code",
            Some(WorkspaceTab::Image { .. }) => "image",
            None => "none",
        }
    }

    /// 测试专用:活跃文件 tab 的已载入文本(None = 载入中/失败/非文本 tab)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_text_for_test(&self, cx: &gpui::App) -> Option<String> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => view.read(cx).text_for_test(),
            Some(WorkspaceTab::Code { view }) => view.read(cx).text_for_test(cx),
            _ => None,
        }
    }

    /// 测试专用:活跃文件 tab 的只读合同(gitshow/预览臂用)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_read_only_for_test(&self, cx: &gpui::App) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).is_read_only()),
            Some(WorkspaceTab::Code { view }) => Some(view.read(cx).is_read_only(cx)),
            _ => None,
        }
    }

    /// 测试专用:活跃图片 tab 是否完成解码。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_image_loaded_for_test(&self, cx: &gpui::App) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Image { view }) => Some(view.read(cx).is_loaded_for_test()),
            _ => None,
        }
    }

    /// 测试专用:活跃图片 tab 的占位缩放值(M8.3 前恒 1.0)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_image_zoom_for_test(&self, cx: &gpui::App) -> Option<f32> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Image { view }) => Some(view.read(cx).zoom_for_test()),
            _ => None,
        }
    }

    /// 测试专用:走统一关闭入口摘除活跃 tab(图片 tab 无焦点路径,
    /// 动作分派到不了 workspace 根;关闭语义仍与动作/runtime 同一条)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn close_active_tab_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.close_active_tab(window, cx);
    }
}
