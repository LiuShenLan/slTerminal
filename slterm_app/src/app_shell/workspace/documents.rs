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

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, PromptLevel, Styled as _, Window, div,
};
use slterm_split::TabId;

use super::{SltermWorkspace, TabEntry, TabMeta, WorkspaceTab};
use crate::app_shell::code_tab::CodeTabView;
use crate::app_shell::config::Settings;
use crate::app_shell::doc_tabs::{self, DocTabView, ImageTabView};
use crate::brand;
use crate::i18n::Message;

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

    /// 文件 tab 的统一入列点:分配稳定 TabId、激活、同步窗口标题,并挂
    /// 变更观察(键入/保存完成 → 刷新侧栏与窗口标题的脏标记「•」)。
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
        let observation = match self.tabs.last().map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => {
                let view = view.clone();
                Some(cx.observe_in(&view, window, |workspace, _, window, cx| {
                    workspace.sync_window_title(window, cx);
                    cx.notify();
                }))
            }
            Some(WorkspaceTab::Document { view }) => {
                let view = view.clone();
                Some(cx.observe_in(&view, window, |workspace, _, window, cx| {
                    workspace.sync_window_title(window, cx);
                    cx.notify();
                }))
            }
            _ => None,
        };
        if let Some(observation) = observation {
            self.file_observations.insert(tab_id, observation);
        }
        self.active = self.tabs.len() - 1;
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 窗口标题 = 活跃 tab 展示名(任务栏可见);无 tab 时回退品牌名。
    pub(super) fn sync_window_title(&self, window: &mut Window, cx: &App) {
        let title = self.active_tab_title(cx);
        window.set_window_title(&format!("{title} — {}", brand::NAME));
    }

    /// 活跃 tab 展示名:自定义名优先,否则取聚焦 pane 的既有终端标题。
    /// 07 篇 TabPresentation 落地后,这一处取值面替换为其派生结果。
    pub(super) fn active_tab_title(&self, cx: &App) -> String {
        self.tabs
            .get(self.active)
            .map(|_| self.tab_title(self.active, cx))
            .unwrap_or_else(|| brand::NAME.to_owned())
    }

    /// 单个 tab 的展示名(侧栏 tab 行与标题栏同源)。
    pub(super) fn tab_title(&self, tab_ix: usize, cx: &App) -> String {
        let Some(entry) = self.tabs.get(tab_ix) else {
            return brand::NAME.to_owned();
        };
        if let Some(custom) = &entry.meta.custom_name {
            return custom.clone();
        }
        match &entry.tab {
            WorkspaceTab::Terminal { panes, focused, .. } => panes
                .iter()
                .find(|pane| pane.id == *focused)
                .map(|pane| pane.view.read(cx).title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "shell".to_owned()),
            WorkspaceTab::Settings { view, .. } => view.read(cx).title(cx).to_string(),
            // 文件 tab 标题(含脏标记「•」)派生归本模块。
            WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => self.file_tab_title(&entry.tab, cx).unwrap_or_default(),
        }
    }

    /// 文件 tab 标题(含脏标记「•」):WorkspaceTab 三变体的展示面单点。
    pub(super) fn file_tab_title(&self, tab: &WorkspaceTab, cx: &App) -> Option<String> {
        match tab {
            WorkspaceTab::Document { view } => Some(view.read(cx).tab_title()),
            WorkspaceTab::Code { view } => Some(view.read(cx).tab_title(cx)),
            WorkspaceTab::Image { view } => Some(view.read(cx).title.clone()),
            _ => None,
        }
    }

    /// 文件 tab 的焦点句柄:图片臂无可编辑面(None = 不迁移焦点)。
    pub(super) fn file_tab_focus_handle(
        &self,
        tab: &WorkspaceTab,
        cx: &App,
    ) -> Option<FocusHandle> {
        match tab {
            WorkspaceTab::Document { view } => Some(view.read(cx).focus_handle(cx)),
            WorkspaceTab::Code { view } => Some(view.read(cx).focus_handle(cx)),
            _ => None,
        }
    }

    /// 文件 tab 的渲染分派:外层选择器独立于文本视图(测试/诊断按类型定位)。
    pub(super) fn render_file_tab(&self, tab: &WorkspaceTab) -> Option<AnyElement> {
        match tab {
            WorkspaceTab::Document { view } => Some(
                div()
                    .debug_selector(|| "slterm-document-tab".to_owned())
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .child(view.clone())
                    .into_any_element(),
            ),
            WorkspaceTab::Code { view } => Some(view.clone().into_any_element()),
            WorkspaceTab::Image { view } => Some(view.clone().into_any_element()),
            _ => None,
        }
    }

    /// Ctrl+S 落点:活跃 Code tab 才触发保存;其余 tab 无操作。
    pub(super) fn save_active_file(&mut self, cx: &mut Context<'_, Self>) {
        let Some(WorkspaceTab::Code { view }) = self.tabs.get(self.active).map(|entry| &entry.tab)
        else {
            return;
        };
        if view.read(cx).is_read_only(cx) {
            return;
        }
        let task = view.clone().update(cx, |view, cx| view.save(cx));
        task.detach();
    }

    /// 关闭脏文件 tab 的守卫:true = 已拦截(保存中/确认在途/弹出确认框)。
    ///
    /// 确认交互照上游原生三键提示:保存 / 不保存 / 取消。保存失败或
    /// 取消都保留 tab 与草稿;保存成功且无新草稿才摘除 tab。
    pub(super) fn guard_file_tab_close(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(entry) = self.tabs.get(index) else {
            return false;
        };
        let WorkspaceTab::Code { view } = &entry.tab else {
            return false;
        };
        if view.read(cx).is_saving(cx) {
            return true;
        }
        if !view.read(cx).is_dirty(cx) {
            return false;
        }
        if self.pending_file_close.is_some() {
            return true;
        }
        let tab_id = entry.id;
        let file = view.clone();
        self.pending_file_close = Some(tab_id);
        let language = cx.global::<Settings>().language;
        let detail = file.read(cx).path.display().to_string();
        let answer = window.prompt(
            PromptLevel::Warning,
            language.text(Message::EditorCloseTitle),
            Some(&detail),
            &[
                language.text(Message::EditorSave),
                language.text(Message::EditorDiscard),
                language.text(Message::EditorCancel),
            ],
            cx,
        );
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let choice = answer.await;
            let should_close = match choice {
                // 保存:等待落盘且保存完成后无新草稿。
                Ok(0) => {
                    let task = file.update(cx, |view, cx| view.save(cx));
                    task.await
                }
                // 不保存:直接丢弃草稿。
                Ok(1) => true,
                // 取消/对话框被系统取消。
                _ => false,
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.pending_file_close = None;
                    if should_close
                        && let Some(index) =
                            workspace.tabs.iter().position(|entry| entry.id == tab_id)
                    {
                        workspace.finish_close_tab(index, window, cx);
                        return;
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        true
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

    /// 测试专用:活跃文件 tab 的工作副本文本(含未保存草稿)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_editor_text_for_test(&self, cx: &gpui::App) -> Option<String> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => Some(view.read(cx).input_text_for_test(cx)),
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).input_text_for_test(cx)),
            _ => None,
        }
    }

    /// 测试专用:活跃文件 tab 的脏标记(保存/关闭确认断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_dirty_for_test(&self, cx: &gpui::App) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => Some(view.read(cx).is_dirty(cx)),
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

    /// 测试专用:把键盘焦点交活跃 tab(键入用例的确定性前置)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn focus_active_for_test(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus_active(window, cx);
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
