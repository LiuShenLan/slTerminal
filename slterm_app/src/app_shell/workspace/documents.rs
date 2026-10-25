//! 文件打开路由与文件 tab 生命周期。
//!
//! 路由单判定在 `doc_tabs::openable_in_app`;本模块只做「判定结果 → tab 变体」
//! 的落地与同路径重开命中:
//!
//! ```text
//! 图片 → WorkspaceTab::Image(BGRA 解码 + 占位缩放)
//! markdown → WorkspaceTab::Document（只读文本占位）
//! 可映射源码 → WorkspaceTab::Code（只读或可编辑）
//! 未命中 → 交系统处理器(file_tree 入口裁定)
//! ```
//!
//! 三个文件 tab 都不进 session 快照(快照只收 Terminal tab),
//! 关闭只走统一的 `close_tab_at`——纯视图 drop,无 PTY/进程生命周期。
//! gitshow 只读臂复用 `WorkspaceTab::Code`,数据源是 HEAD blob。

use std::path::PathBuf;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, PromptLevel, Styled as _, Task, Window, div,
};
use gpui_component::WindowExt as _;
use slterm_split::TabId;

use super::{SltermWorkspace, TabEntry, TabMeta, WorkspaceTab};
use crate::app_shell::code_tab::{CodeTabEvent, CodeTabView};
use crate::app_shell::config::Settings;
use crate::app_shell::doc_tabs::{self, DocTabView, ImageTabView};
#[cfg(all(test, feature = "gpui-test-support"))]
use crate::app_shell::file_editor::gutter::GutterMark;
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

/// Code/Document 两种可保存视图的统一句柄:脏关闭守卫与 Ctrl+S 共用。
#[derive(Clone)]
enum FileViewHandle {
    Code(Entity<CodeTabView>),
    Document(Entity<DocTabView>),
}

impl FileViewHandle {
    fn of(tab: &WorkspaceTab) -> Option<Self> {
        match tab {
            WorkspaceTab::Code { view } => Some(Self::Code(view.clone())),
            WorkspaceTab::Document { view } => Some(Self::Document(view.clone())),
            _ => None,
        }
    }

    fn is_saving(&self, cx: &App) -> bool {
        match self {
            Self::Code(view) => view.read(cx).is_saving(cx),
            Self::Document(view) => view.read(cx).is_saving(),
        }
    }

    fn is_dirty(&self, cx: &App) -> bool {
        match self {
            Self::Code(view) => view.read(cx).is_dirty(cx),
            Self::Document(view) => view.read(cx).is_dirty(),
        }
    }

    fn path(&self, cx: &App) -> PathBuf {
        match self {
            Self::Code(view) => view.read(cx).path.clone(),
            Self::Document(view) => view.read(cx).path.clone(),
        }
    }

    fn save<C: gpui::AppContext>(&self, cx: &mut C) -> Task<bool> {
        match self {
            Self::Code(view) => view.clone().update(cx, |view, cx| view.save(cx)),
            Self::Document(view) => view.clone().update(cx, |view, cx| view.save(cx)),
        }
    }
}

impl SltermWorkspace {
    /// 文件打开路由。
    #[allow(dead_code)] // 消费点 = file_tree 双击入口;出口用例经此直调。
    pub(crate) fn open_document_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 单判定门:应用内不能读的扩展名不产生 tab(交系统处理器是
        // 文件树入口的动作,路由层不静默执行)。
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
    #[allow(dead_code)] // 消费点 = gutter/diff 入口;出口用例经此直调。
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

    /// 通用 diff tab:HEAD↔工作区双栏只读臂(同路径重开命中)。
    pub(crate) fn open_diff_tab(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(index) = self.tabs.iter().position(|entry| {
            matches!(&entry.tab, WorkspaceTab::Code { view } if view.read(cx).is_diff_path(&path))
        }) {
            if let Some(view) = self.tabs.get(index).and_then(code_view) {
                view.clone()
                    .update(cx, |view, cx| view.reload(window, cx));
            }
            self.activate_tab(index, window, cx);
            return;
        }
        let view = cx.new(|cx| CodeTabView::new_diff(path, window, cx));
        self.push_file_tab(WorkspaceTab::Code { view }, window, cx);
    }

    /// code tab 的 gutter 点击统一打开只读双栏 diff。
    fn on_code_tab_event(
        &mut self,
        _view: &Entity<CodeTabView>,
        event: &CodeTabEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let CodeTabEvent::GutterClicked { path, .. } = event;
        self.open_diff_tab(path.clone(), window, cx);
    }

    /// 同一路径复用已开 tab（激活 + 重读盘；草稿保留）。
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
        // Document = 渲染态打开、可编辑;源码↔预览同源同一份文档。
        let view = cx.new(|cx| DocTabView::new_document(path, window, cx));
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
        let tab_events = match self.tabs.last().map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => {
                let view = view.clone();
                Some(cx.subscribe_in(&view, window, Self::on_code_tab_event))
            }
            _ => None,
        };
        if let Some(tab_events) = tab_events {
            self.file_tab_events.insert(tab_id, tab_events);
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
    /// `TabPresentation` 落地后,这一处取值面替换为其派生结果。
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

    /// 文件 tab 的焦点句柄:图片臂也有独立焦点(动作可到达工作区)。
    pub(super) fn file_tab_focus_handle(
        &self,
        tab: &WorkspaceTab,
        cx: &App,
    ) -> Option<FocusHandle> {
        match tab {
            WorkspaceTab::Document { view } => Some(view.read(cx).focus_handle(cx)),
            WorkspaceTab::Code { view } => Some(view.read(cx).focus_handle(cx)),
            WorkspaceTab::Image { view } => Some(view.read(cx).focus_handle(cx)),
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

    /// Ctrl+S 落点:活跃 Code/Document tab 触发保存;其余 tab 无操作。
    pub(super) fn save_active_file(&mut self, cx: &mut Context<'_, Self>) {
        let Some(entry) = self.tabs.get(self.active) else {
            return;
        };
        match &entry.tab {
            WorkspaceTab::Code { view } => {
                if view.read(cx).is_read_only(cx) {
                    return;
                }
                let task = view.clone().update(cx, |view, cx| view.save(cx));
                task.detach();
            }
            WorkspaceTab::Document { view } => {
                if view.read(cx).is_read_only() {
                    return;
                }
                let task = view.clone().update(cx, |view, cx| view.save(cx));
                task.detach();
            }
            _ => {}
        }
    }

    /// 渲染激活位同步:只有活跃文件 tab 物化预览资产;切走的 tab 释放
    /// 块槽/图片解码,草稿与撤销历史保留(预览虚拟化与资产释放)。
    pub(super) fn sync_file_render_activity(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let active = self.active;
        for (index, entry) in self.tabs.iter().enumerate() {
            let is_active = index == active;
            match &entry.tab {
                WorkspaceTab::Code { view } => {
                    view.update(cx, |view, cx| view.set_render_active(is_active, window, cx));
                }
                WorkspaceTab::Document { view } => {
                    view.update(cx, |view, cx| view.set_render_active(is_active, window, cx));
                }
                _ => {}
            }
        }
    }

    /// 关闭脏文件 tab 的守卫:true = 已拦截(保存中/确认在途/弹出确认框)。
    ///
    /// 确认交互照上游原生三键提示:保存 / 不保存 / 取消。保存失败或
    /// 取消都保留 tab 与草稿;保存成功且无新草稿才摘除 tab。
    /// Document 与 Code 共用同一守卫。
    pub(super) fn guard_file_tab_close(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(entry) = self.tabs.get(index) else {
            return false;
        };
        let Some(file) = FileViewHandle::of(&entry.tab) else {
            return false;
        };
        if file.is_saving(cx) {
            return true;
        }
        if !file.is_dirty(cx) {
            return false;
        }
        if self.pending_file_close.is_some() {
            return true;
        }
        let tab_id = entry.id;
        self.pending_file_close = Some(tab_id);
        let language = cx.global::<Settings>().language;
        let detail = file.path(cx).display().to_string();
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
                Ok(0) => file.save(cx).await,
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

    /// 整窗关闭脏守卫(问题 3):true = 可以继续关窗;false = 本轮已拦截。
    ///
    /// 与单 tab 关闭同一确认交互(保存 / 不保存 / 取消),但作用对象是
    /// 全部脏文件 tab:保存臂逐个落盘,全部成功才放行;任一失败保持窗口
    /// 打开(失败横幅已在编辑器内可见)。保存中在途 = 拦截本轮,等落盘后
    /// 用户再次关窗;确认框同一时刻只允许一个(pending_file_close 单槽)。
    pub(super) fn guard_file_window_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        // 异步确认已放行:remove_window 若再次触发钩子,不得重弹确认。
        if std::mem::take(&mut self.window_close_confirmed) {
            return true;
        }
        if self.pending_file_close.is_some() {
            return false;
        }
        let mut dirty = Vec::new();
        for entry in &self.tabs {
            let Some(file) = FileViewHandle::of(&entry.tab) else {
                continue;
            };
            if file.is_saving(cx) {
                // 保存中:本轮关窗直接拦截,下一次关窗再看结果。
                return false;
            }
            if file.is_dirty(cx) {
                dirty.push((entry.id, file));
            }
        }
        if dirty.is_empty() {
            return true;
        }
        let first_tab = dirty[0].0;
        self.pending_file_close = Some(first_tab);
        let handles: Vec<FileViewHandle> = dirty.into_iter().map(|(_, file)| file).collect();
        let language = cx.global::<Settings>().language;
        let detail = language.format(
            Message::EditorCloseWindowDetail,
            &[("count", &handles.len().to_string())],
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            language.text(Message::EditorCloseWindowTitle),
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
            let (close, failed) = match choice {
                // 保存全部:尽量保存每个文件;任一失败则保持窗口与草稿,
                // 并聚合报告失败数量(不再只在各自编辑器内提示)。
                Ok(0) => {
                    let mut failed = 0usize;
                    for file in &handles {
                        if !file.save(cx).await {
                            failed += 1;
                        }
                    }
                    (failed == 0, failed)
                }
                // 不保存:丢弃全部草稿。
                Ok(1) => (true, 0),
                // 取消/对话框被系统取消。
                _ => (false, 0),
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.pending_file_close = None;
                    if close {
                        workspace.window_close_confirmed = true;
                        workspace.save_on_window_close(window, cx);
                        window.remove_window();
                    } else {
                        if failed > 0 {
                            let language = cx.global::<Settings>().language;
                            let detail = language.format(
                                Message::EditorSaveAllFailedDetail,
                                &[("count", &failed.to_string())],
                            );
                            window.push_notification(
                                gpui_component::notification::Notification::error(
                                    language.text(Message::EditorSaveAllFailedTitle),
                                )
                                .message(detail),
                                cx,
                            );
                        }
                        cx.notify();
                    }
                });
            });
        })
        .detach();
        false
    }

    /// 测试专用:活跃 diff 模式 tab 的 (hunk 数, 活动 hunk 下标)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_diff_state_for_test(&self, cx: &App) -> Option<(usize, Option<usize>)> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => view
                .read(cx)
                .diff_state_for_test(cx)
                .map(|(hunks, active)| (hunks.len(), active)),
            _ => None,
        }
    }

    /// 测试专用:驱动活跃 diff 的上一/下一处更改。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn navigate_active_diff_for_test(
        &mut self,
        forward: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(WorkspaceTab::Code { view }) =
            self.tabs.get(self.active).map(|entry| &entry.tab)
        {
            let view = view.clone();
            view.update(cx, |view, cx| view.navigate_diff_hunk_for_test(forward, cx));
        }
    }

    /// 测试专用:活跃 code tab 的 gutter 行标(File 模式)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_gutter_marks_for_test(&self, cx: &App) -> Option<Vec<(u32, GutterMark)>> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Code { view }) => Some(view.read(cx).gutter_marks_for_test(cx)),
            _ => None,
        }
    }

    /// 测试专用:打开 diff 入口(生产入口同一条)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn open_diff_tab_for_test(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_diff_tab(path, window, cx);
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
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).is_dirty()),
            _ => None,
        }
    }

    /// 测试专用:活跃 Document tab 是否渲染态。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_preview_for_test(&self, cx: &gpui::App) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).is_preview_for_test()),
            _ => None,
        }
    }

    /// 测试专用:切换活跃 Document 的双模态。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn toggle_active_preview_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(WorkspaceTab::Document { view }) =
            self.tabs.get(self.active).map(|entry| &entry.tab)
        {
            view.clone()
                .update(cx, |view, cx| view.toggle_preview_for_test(window, cx));
        }
    }

    /// 测试专用:活跃 Document 的预览块数。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_block_count_for_test(&self, cx: &gpui::App) -> Option<usize> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).block_count_for_test()),
            _ => None,
        }
    }

    /// 测试专用:当帧物化的预览块下标(虚拟化断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_rendered_blocks_for_test(&self, cx: &gpui::App) -> Option<Vec<usize>> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).rendered_blocks_for_test()),
            _ => None,
        }
    }

    /// 测试专用:滚动活跃预览到指定块。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn scroll_active_preview_to_for_test(&self, block: usize, cx: &gpui::App) {
        if let Some(WorkspaceTab::Document { view }) =
            self.tabs.get(self.active).map(|entry| &entry.tab)
        {
            view.read(cx).scroll_preview_to_for_test(block);
        }
    }

    /// 测试专用:活跃 Document 的预览防抖位。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_preview_stale_for_test(&self, cx: &gpui::App) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).preview_stale_for_test()),
            _ => None,
        }
    }

    /// 测试专用:活跃文件 tab 的共享撤销栈组数。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_history_depth_for_test(&self, cx: &gpui::App) -> Option<usize> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => {
                Some(view.read(cx).history_undo_depth_for_test())
            }
            Some(WorkspaceTab::Code { view }) => {
                Some(view.read(cx).history_undo_depth_for_test(cx))
            }
            _ => None,
        }
    }

    /// 测试专用:在活跃 Document 的指定块开启渲染态输入。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn begin_active_live_edit_for_test(
        &mut self,
        block: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(WorkspaceTab::Document { view }) =
            self.tabs.get(self.active).map(|entry| &entry.tab)
        {
            view.clone().update(cx, |view, cx| {
                view.begin_live_edit_for_test(block, window, cx)
            });
        }
    }

    /// 测试专用:指定下标 Document tab 的渲染激活位与物化块数。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn document_render_state_for_test(
        &self,
        index: usize,
        cx: &gpui::App,
    ) -> Option<(bool, usize)> {
        match self.tabs.get(index).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some((
                view.read(cx).render_active_for_test(),
                view.read(cx).materialized_block_count_for_test(),
            )),
            _ => None,
        }
    }

    /// 测试专用:活跃 Document 的详情侧栏宽度。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_details_width_for_test(&self, cx: &gpui::App) -> Option<f32> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Document { view }) => Some(view.read(cx).details_width_for_test()),
            _ => None,
        }
    }

    /// 测试专用:按步进缩放活跃图片 tab(滚轮路径的确定性入口)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn zoom_active_image_for_test(&mut self, steps: f32, cx: &mut Context<'_, Self>) {
        if let Some(WorkspaceTab::Image { view }) =
            self.tabs.get(self.active).map(|entry| &entry.tab)
        {
            view.clone()
                .update(cx, |view, cx| view.zoom_for_test_steps(steps, cx));
        }
    }

    /// 测试专用:活跃图片 tab 是否持有键盘焦点(问题 1 回归端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_image_focused_for_test(
        &self,
        window: &Window,
        cx: &gpui::App,
    ) -> Option<bool> {
        match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Image { view }) => {
                Some(view.read(cx).focus_is_active_for_test(window, cx))
            }
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

    /// 测试专用:活跃图片 tab 的占位缩放值(前恒 1.0)。
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
    /// 动作分派到不了 workspace 根;关闭语义仍与键盘动作同一条)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn close_active_tab_for_test(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.close_active_tab(window, cx);
    }
}
