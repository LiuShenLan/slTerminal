//! 文本文件视图与编辑内核宿主:只读骨架 → 源码态可编辑
//! → 双模态预览与 chrome。
//!
//! 载入:本地文件经 [`Document`](document::Document) 有界读 + 快照解码;
//! gitshow 的 HEAD 字节经同一 `TextSnapshot` 通路,encode 层拒绝写回。
//!
//! 双模态:markdown 文档默认渲染态(`preview = true`),源码态与原生态输入
//! 共用同一 `InputState`;渲染态块聚焦换稳定 `LiveEdit` 输入,内容写回规范
//! 源码。两态共享 `TextDocument`/`EditHistory`,切换不丢编辑。
//!
//! 撤销统一:根视图捕获 `gpui_component::input::{Undo, Redo}` 并走
//! `EditHistory::travel`;输入实体内建栈在文件编辑上下文中被拦截(不再旁路)。
//!
//! 预览虚拟化:`Outline` 块清单 → `gpui::list` 只物化视口行;切走 tab 时
//! `set_render_active(false)` 释放块槽/图片激活,草稿与历史存活。

#[allow(dead_code)] // 消费点 = 预览态切走/切回(activity::set_render_active)。
mod activity;
#[allow(dead_code)] // 消费点 = live_edit 分段渲染与结构化块。
mod block_inline;
#[allow(dead_code)] // 消费点 = 结构化渲染/结构命令/outline 部件索引。
mod block_structure;
mod chrome;
mod document;
mod edit_history;
/// git gutter(行标)。
pub(crate) mod gutter;
#[allow(dead_code)] // 消费点 = 预览图片解码 LRU(虚拟行资产纪律)。
mod image_cache;
mod images;
mod info;
#[allow(dead_code)] // 消费点 = live_edit 投影/装饰与渲染态编辑。
mod inline_edit;
#[allow(dead_code)] // 消费点 = 相邻内联视图共享复制。
mod inline_selection;
#[allow(dead_code)] // 消费点 = live_edit 输入规则接线(apply_input_rule)。
mod input_rules;
mod live_commands;
mod live_edit;
#[allow(dead_code)] // 消费点 = 跨输入边界方向键导航。
mod live_navigation;
#[allow(dead_code)] // 消费点 = 行内源码显影跟随光标。
mod live_selection;
mod outline;
mod outline_view;
mod preview;
mod reader_presentation;
#[allow(dead_code)] // 消费点 = 结构化编辑命令(续列表/缩进/表格)。
mod structure_commands;
#[allow(dead_code)] // 消费点 = 混合段落内联 run 渲染。
mod structure_inline_view;
#[allow(dead_code)] // 消费点 = 表格/列表/代码/公式结构化渲染。
mod structure_view;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EntityInputHandler as _, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, ListAlignment, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render, SharedString,
    Styled as _, Subscription, Task, Window, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{
    self, Backspace, Copy, Enter, Escape, IndentInline, Input, InputEvent, InputState, MoveDown,
    MoveLeft, MoveRight, MoveToEnd, MoveToStart, MoveUp, OutdentInline, Position, SelectAll,
};
use gpui_component::text::{MarkdownExtensions, TextViewState};
pub(crate) use gpui_component::{h_flex, v_flex};

use crate::app_shell::config::Settings;
use crate::app_shell::editor_theme::EditorThemeTokens;
use crate::git;
use crate::i18n::Message;
use crate::text_document::{MAX_BYTES, SaveError, TextSnapshot};
use activity::EditCursor;
use document::Document;
use edit_history::EditHistory;
use gutter::{GitGutter, GutterMark};
use image_cache::DocumentImageCache;
use live_edit::LiveEdit;
use outline::Outline;

gpui::actions!(
    file_editor,
    [
        SaveFile,
        ToggleSource,
        FinishBlockEdit,
        BoldSelection,
        ItalicSelection
    ]
);

/// 编辑器对外事件(外层 code tab 转发):
/// gutter 点击 = 行标 → 差异对照/冲突面的动作入口。
#[derive(Clone, Debug)]
pub enum TextFileEvent {
    /// 行标点击:行号(1 基)与标记值集。
    GutterClicked { line: u32, mark: GutterMark },
}

/// 文件编辑上下文键绑定(`gpui_component::init` 之后调用一次)。
///
/// Ctrl+Z/Ctrl+Y 显式绑定到组件动作,再由根视图捕获转 `EditHistory`;
/// 渲染态粗体/斜体/块提交只在 `MarkdownLive` 上下文生效。
pub(super) fn init(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("ctrl-/", ToggleSource, Some("FileEditor")),
        KeyBinding::new("ctrl-z", input::Undo, Some("FileEditor")),
        KeyBinding::new("ctrl-y", input::Redo, Some("FileEditor")),
        KeyBinding::new("ctrl-shift-z", input::Redo, Some("FileEditor")),
        KeyBinding::new("ctrl-b", BoldSelection, Some("MarkdownLive")),
        KeyBinding::new("ctrl-i", ItalicSelection, Some("MarkdownLive")),
        KeyBinding::new("ctrl-enter", FinishBlockEdit, Some("MarkdownLive")),
        KeyBinding::new("escape", FinishBlockEdit, Some("MarkdownLive")),
        KeyBinding::new("ctrl-s", SaveFile, Some("FileEditor")),
        KeyBinding::new("ctrl-a", input::SelectAll, Some("FileEditor")),
        KeyBinding::new("ctrl-c", Copy, Some("FileEditor")),
    ]);
}

/// 快照来源。gitshow 与本地文件共用视图形态,只有数据源与只读意图不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextSource {
    /// 工作区/磁盘当前字节(有界前缀)。
    Local,
    /// 当前 HEAD 中的 blob 字节(只读)。
    GitHead,
}

/// 后台载入结果:本地文件带保存目标(`Document`),HEAD 只要快照。
enum LoadedDocument {
    Local(Document),
    Head(TextSnapshot),
}

/// 虚拟化块槽:屏外块置 None,不保留逐块视图。
type PreviewBlockSlots = Rc<RefCell<Vec<Option<Entity<TextViewState>>>>>;
/// 内联 run 视图弱句柄(只随挂载行存活)。
type InlineViewMap = Rc<RefCell<BTreeMap<(usize, usize), gpui::WeakEntity<TextViewState>>>>;

/// 视图内提示:载入失败、保存三态与预览上限。
enum EditorNotice {
    ReadFailed(String),
    SaveFailed(String),
    SaveChanged,
    SaveReadOnly,
    PreviewLimited,
}

/// gutter 行标 → 语义槽颜色(渲染代码零裸色值)。
fn gutter_color(tokens: &EditorThemeTokens, mark: GutterMark) -> gpui::Rgba {
    let color = match mark {
        GutterMark::Untracked => tokens.gutter_untracked,
        GutterMark::Added => tokens.gutter_added,
        GutterMark::Modified => tokens.gutter_modified,
        GutterMark::Deleted => tokens.gutter_deleted,
        GutterMark::Renamed => tokens.gutter_renamed,
        GutterMark::Conflicted => tokens.gutter_conflicted,
    };
    crate::gpui_terminal::colors::from_rgb8(color)
}

/// 文件标题:文件名优先,取不到时退回完整路径。
fn file_title(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub struct TextFileView {
    pub path: PathBuf,
    pub title: String,
    /// 源码态原生输入(gpui-component code editor)。
    input: Entity<InputState>,
    source: TextSource,
    /// 打开意图的只读位(gitshow true;普通源码/文档 false)。
    fixed_read_only: bool,
    /// 本地文档(保存目标 + 载入快照);HEAD 臂为 None。
    document: Option<Document>,
    /// HEAD 只读快照(gitshow)。
    head: Option<TextSnapshot>,
    /// 跨表示统一撤销历史(源码键入/渲染态提交/结构命令同栈)。
    history: EditHistory,
    /// 已保存文本(脏标记基线):载入/保存成功时更新。
    saved_text: Option<String>,
    dirty: bool,
    saving: bool,
    loading: bool,
    notice: Option<EditorNotice>,
    /// markdown 文档(M8.3 双模态前提)。
    markdown: bool,
    /// 当前显示渲染态(仅 markdown 有意义)。
    preview: bool,
    /// 渲染态块聚焦可编辑(预览文档 true;纯阅读臂 false)。
    live_mode: bool,
    live_edit: Option<LiveEdit>,
    render_active: bool,
    preview_stale: bool,
    last_edit_cursor: Option<EditCursor>,
    resume_edit: bool,
    show_details: bool,
    /// 聚焦阅读:隐藏工具条与详情栏,仅保留编辑器内容(视图内存,不持久化)。
    reader_focus: bool,
    info: bool,
    outline: Outline,
    blocks: PreviewBlockSlots,
    inline_views: InlineViewMap,
    preview_extensions: MarkdownExtensions,
    preview_images: Entity<DocumentImageCache>,
    scroll: ListState,
    /// 当帧实际物化的块下标(测试/诊断;每帧 render 开头清空)。
    rendered_blocks: Rc<RefCell<Vec<usize>>>,
    preview_selection_scroll_epoch: u64,
    preview_selection_scroll_active: bool,
    outline_scroll: gpui::ScrollHandle,
    preview_scrollbar_hovered: bool,
    outline_scrollbar_hovered: bool,
    details_width: f32,
    details_resize_anchor: Option<(f32, f32)>,
    collapsed_headings: HashSet<usize>,
    selected_heading: Option<usize>,
    all_selected: bool,
    revision: u64,
    content_revision: u64,
    preview_task: Option<Task<()>>,
    /// 程序化替换期间抑制输入订阅的历史记录(替换路径显式记录一次)。
    suppress_history: bool,
    /// git 行标(打开/保存/重载三触发点同步;只读文档恒空)。
    gutter: GitGutter,
    focus: FocusHandle,
    _input_subscription: Subscription,
}

impl gpui::EventEmitter<TextFileEvent> for TextFileView {}

impl TextFileView {
    /// 焦点句柄:渲染态优先活跃输入,其次阅读器根,最后源码输入。
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        if let Some(edit) = &self.live_edit {
            edit.input.read(cx).focus_handle(cx)
        } else if self.preview {
            self.focus.clone()
        } else {
            self.input.read(cx).focus_handle(cx)
        }
    }
}

impl Focusable for TextFileView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        TextFileView::focus_handle(self, cx)
    }
}

impl TextFileView {
    /// 本地源码文件载入(源码态;`read_only` = 打开意图固定只读)。
    pub fn new(
        path: PathBuf,
        language: &'static str,
        read_only: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_source(
            path,
            language,
            TextSource::Local,
            read_only,
            false,
            window,
            cx,
        )
    }

    /// Markdown 文档载入(渲染态打开,可编辑;快照只读仍由合同兜底)。
    pub fn new_document(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_source(path, "markdown", TextSource::Local, false, true, window, cx)
    }

    /// gitshow 只读臂:HEAD 字节 + 恒定只读(源码态)。
    pub fn new_git_head(
        path: PathBuf,
        language: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_source(path, language, TextSource::GitHead, true, false, window, cx)
    }

    fn new_with_source(
        path: PathBuf,
        language: &'static str,
        source: TextSource,
        fixed_read_only: bool,
        markdown: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let title = file_title(&path);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language)
                .line_number(true)
                .indent_guides(false)
                .soft_wrap(false)
        });
        // 源码键入热路径:先从 Rope 提取局部差量入历史,再用当前文本重算
        // 脏标记与预览防抖。`set_value`(载入/重载)与 `replace`(程序化
        // 替换)是静默更新,不会进入此订阅。
        let subscription = cx.subscribe_in(&input, window, |view, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                view.content_revision = view.content_revision.wrapping_add(1);
                if !view.suppress_history {
                    let text = view.input.read(cx).text().clone();
                    view.history.record_rope(&text);
                }
                let value = view.input.read(cx).value();
                view.dirty = view.saved_text.as_deref() != Some(value.as_ref());
                if view.markdown {
                    view.schedule_preview(cx);
                }
                cx.notify();
            }
        });
        let preview_extensions = preview::extensions(path.parent().map(PathBuf::from));
        let mut view = Self {
            path,
            title,
            input,
            source,
            fixed_read_only,
            document: None,
            head: None,
            history: EditHistory::default(),
            saved_text: None,
            dirty: false,
            saving: false,
            loading: true,
            notice: None,
            markdown,
            preview: markdown,
            // 渲染态默认接受块聚焦编辑;只读快照由 `preview_editable` 兜底。
            live_mode: markdown,
            live_edit: None,
            render_active: true,
            preview_stale: false,
            last_edit_cursor: None,
            resume_edit: false,
            show_details: markdown,
            reader_focus: false,
            info: false,
            outline: Outline::default(),
            blocks: Rc::default(),
            inline_views: Rc::default(),
            preview_extensions,
            preview_images: DocumentImageCache::new(cx.entity_id(), cx),
            scroll: ListState::new(0, ListAlignment::Top, px(500.0)),
            rendered_blocks: Rc::default(),
            preview_selection_scroll_epoch: 0,
            preview_selection_scroll_active: false,
            outline_scroll: gpui::ScrollHandle::new(),
            preview_scrollbar_hovered: false,
            outline_scrollbar_hovered: false,
            details_width: reader_presentation::OUTLINE_WIDTH,
            details_resize_anchor: None,
            collapsed_headings: Default::default(),
            selected_heading: None,
            all_selected: false,
            revision: 0,
            content_revision: 0,
            preview_task: None,
            suppress_history: false,
            gutter: GitGutter::default(),
            focus: cx.focus_handle(),
            _input_subscription: subscription,
        };
        view.spawn_load(window, cx);
        view
    }

    /// Tab 标题:脏文档追加「•」标记(侧栏/标题栏同源)。
    pub fn tab_title(&self) -> String {
        if self.dirty {
            format!("{} •", self.title)
        } else {
            self.title.clone()
        }
    }

    /// 只读合同查询:固定只读位、快照自身只读(截断/非法编码/只读文件),
    /// 或载入失败(无快照 = 没有可保存目标,不得接受编辑)。
    pub fn is_read_only(&self) -> bool {
        self.fixed_read_only || self.snapshot().is_none_or(|snapshot| snapshot.read_only)
    }

    /// 脏标记:tab 标题「•」与关闭确认的数据源。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 保存中:关闭确认在此态不弹窗,避免与在途写盘竞争。
    pub fn is_saving(&self) -> bool {
        self.saving
    }

    /// 测试专用:后台载入完成后的快照文本(本地与 HEAD 同源)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn text_for_test(&self) -> Option<String> {
        self.snapshot().map(|snapshot| snapshot.text.clone())
    }

    /// 测试专用:输入实体当前文本(编辑后的工作副本,含未保存草稿)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn input_text_for_test(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    /// 测试专用:当前是否渲染态。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn is_preview_for_test(&self) -> bool {
        self.preview
    }

    /// 测试专用:切换双模态(生产入口为工具栏按钮/ToggleSource 动作)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn toggle_preview_for_test(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.toggle_preview(window, cx);
    }

    /// 测试专用:预览块数(大纲块清单长度)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn block_count_for_test(&self) -> usize {
        self.outline.blocks.len()
    }

    /// 测试专用:预览是否待重排(250ms 防抖窗口内 true)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn preview_stale_for_test(&self) -> bool {
        self.preview_stale
    }

    /// 测试专用:已物化块槽数量(诊断与断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn materialized_block_count_for_test(&self) -> usize {
        self.blocks
            .borrow()
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// 测试专用:渲染激活位。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn render_active_for_test(&self) -> bool {
        self.render_active
    }

    /// 测试专用:gutter 行标集(按行升序)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn gutter_marks_for_test(&self) -> Vec<(u32, GutterMark)> {
        self.gutter.marks().to_vec()
    }

    /// 测试专用:撤销栈组数(撤销统一断言端;行动作必须让它下降)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn history_undo_depth_for_test(&self) -> usize {
        self.history.undo_depth_for_test()
    }

    /// 测试专用:在指定块开启渲染态输入。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn begin_live_edit_for_test(
        &mut self,
        block: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.begin_live_edit(block, window, cx);
    }

    /// 测试专用:详情侧栏宽度。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn details_width_for_test(&self) -> f32 {
        self.details_width
    }

    /// 重新读盘(同路径再次打开 / 外部修改确认后)。
    ///
    /// 脏/保存中/载入中一律拒绝:草稿与撤销历史不得被重读盘静默覆盖
    /// (同路径重开保留草稿)。
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.dirty || self.saving || self.loading {
            return;
        }
        self.loading = true;
        self.notice = None;
        self.spawn_load(window, cx);
        cx.notify();
    }

    /// 条件保存:返回是否成功落盘且保存完成后仍无新草稿。
    pub fn save(&mut self, cx: &mut Context<'_, Self>) -> Task<bool> {
        if self.saving || self.loading || self.is_read_only() {
            return Task::ready(false);
        }
        if !self.dirty {
            return Task::ready(true);
        }
        let Some(document) = self.document.clone() else {
            return Task::ready(false);
        };
        let path = self.path.clone();
        let text = self.input.read(cx).value().to_string();
        self.saving = true;
        self.notice = None;
        let task = cx
            .background_executor()
            .spawn(async move { document.save(&path, &text) });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |view, cx| view.apply_saved(result, cx))
                .unwrap_or(false)
        })
    }

    fn apply_saved(
        &mut self,
        result: Result<Document, SaveError>,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        self.saving = false;
        let success = match result {
            Ok(document) => {
                let saved = document.text.clone();
                self.document = Some(document);
                self.saved_text = Some(saved.clone());
                self.dirty = self.input.read(cx).value().as_ref() != saved;
                // 保存是撤销组边界:跨保存的键入不合并为一项。
                self.history.barrier();
                self.notice = None;
                // 保存落盘 = gutter 三触发点之一:后台重读状态,不阻塞保存回执。
                self.spawn_gutter_sync(cx);
                !self.dirty
            }
            Err(error) => {
                self.notice = Some(match error {
                    SaveError::Changed => EditorNotice::SaveChanged,
                    SaveError::ReadOnly => EditorNotice::SaveReadOnly,
                    SaveError::Io(error) => EditorNotice::SaveFailed(error.to_string()),
                });
                false
            }
        };
        cx.notify();
        success
    }

    /// 外部修改触发点(B8):watcher 检测到磁盘变更后重读 gutter 快照。
    ///
    /// 只重算行标,不触碰编辑器缓冲/草稿;只读/gitshow 由
    /// [`Self::spawn_gutter_sync`] 内部跳过。
    pub(crate) fn refresh_gutter_from_disk(&mut self, cx: &mut Context<'_, Self>) {
        self.spawn_gutter_sync(cx);
    }

    /// 后台重读 gutter 快照(保存落盘/显式重载后);只读文档不挂行标。
    fn spawn_gutter_sync(&mut self, cx: &mut Context<'_, Self>) {
        if self.source == TextSource::GitHead || self.fixed_read_only {
            return;
        }
        let path = self.path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { gutter::read_for_path(&path) });
        cx.spawn(async move |this, cx| {
            let next = task.await;
            let _ = this.update(cx, |view, cx| {
                if view.gutter.sync(next) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn snapshot(&self) -> Option<&TextSnapshot> {
        self.document.as_deref().or(self.head.as_ref())
    }

    fn source_slice(&self, range: std::ops::Range<usize>, cx: &App) -> Option<String> {
        self.input
            .read(cx)
            .text()
            .try_slice(range)
            .ok()
            .map(|slice| slice.to_string())
    }

    /// 后台载入:读盘/解码不占前台线程;markdown 同时预解析大纲。
    fn spawn_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let path = self.path.clone();
        let source = self.source;
        let markdown = self.markdown;
        let task = cx
            .background_executor()
            .spawn(async move { load_document(&path, source, markdown) });
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| view.apply_loaded(loaded, window, cx));
            });
        })
        .detach();
    }

    fn apply_loaded(
        &mut self,
        loaded: Result<(LoadedDocument, Option<Outline>, GitGutter), String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match loaded {
            Ok((LoadedDocument::Local(document), outline, gutter)) => {
                let text = document.text.clone();
                self.input.update(cx, |input, cx| {
                    input.set_value(text.clone(), window, cx);
                });
                self.saved_text = Some(text);
                self.document = Some(document);
                self.head = None;
                self.gutter = gutter;
                if let Some(outline) = outline {
                    self.apply_outline(outline, cx);
                }
            }
            Ok((LoadedDocument::Head(snapshot), _, _)) => {
                let text = snapshot.text.clone();
                self.input.update(cx, |input, cx| {
                    input.set_value(text.clone(), window, cx);
                });
                self.saved_text = Some(text);
                self.head = Some(snapshot);
                self.document = None;
                // 只读文档不挂 gutter:无编辑域即无行标意义。
                self.gutter = GitGutter::default();
            }
            Err(error) => self.notice = Some(EditorNotice::ReadFailed(error)),
        }
        self.history
            .reset(self.saved_text.as_deref().unwrap_or_default());
        self.dirty = false;
        self.loading = false;
        cx.notify();
    }

    /// 250ms 防抖重排:在途任务即取消;渲染未激活或块编辑中不内联重排。
    fn schedule_preview(&mut self, cx: &mut Context<'_, Self>) {
        self.revision = self.revision.wrapping_add(1);
        self.preview_stale = true;
        self.preview_task = None;
        if !self.preview || !self.render_active || self.live_edit.is_some() {
            return;
        }
        let revision = self.revision;
        let executor = cx.background_executor().clone();
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(Duration::from_millis(250)).await;
            let Ok((source, path)) = this.update(cx, |view, cx| {
                (view.input.read(cx).value(), view.path.clone())
            }) else {
                return;
            };
            let outline = executor
                .spawn(async move { Outline::prepare(&source, path.parent()) })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.render_active && view.revision == revision && view.live_edit.is_none() {
                    view.apply_outline(outline, cx);
                }
            });
        }));
    }

    /// 落大纲:块槽重建、滚动位置保留、折叠/选中复位。
    fn apply_outline(&mut self, outline: Outline, cx: &mut Context<'_, Self>) {
        self.preview_stale = false;
        self.inline_views.borrow_mut().clear();
        let top = self.scroll.logical_scroll_top();
        self.blocks = Rc::new(RefCell::new(vec![None; outline.blocks.len()]));
        self.scroll
            .reset(outline.blocks.len().max(usize::from(self.live_mode)));
        if !outline.blocks.is_empty() {
            self.scroll.scroll_to(gpui::ListOffset {
                item_ix: top.item_ix.min(outline.blocks.len() - 1),
                ..top
            });
        }
        self.outline = outline;
        self.collapsed_headings.clear();
        self.selected_heading = None;
        self.all_selected = false;
        cx.notify();
    }

    /// 渲染态 ↔ 源码态:先把块输入提交回规范源码,再切显示面。
    fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.finish_live_edit(cx);
        self.preview = !self.preview;
        self.preview_images.update(cx, |images, cx| {
            images.set_active(self.render_active && self.preview, window, cx);
        });
        if self.preview {
            if self.preview_stale {
                self.schedule_preview(cx);
            }
            self.focus.focus(window, cx);
        } else {
            self.inline_views.borrow_mut().clear();
            self.rendered_blocks.borrow_mut().clear();
            self.stop_preview_selection_scroll();
            for block in self.blocks.borrow_mut().iter_mut() {
                *block = None;
            }
            self.all_selected = false;
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    /// 聚焦阅读开关:隐藏工具条与详情栏,仅保留编辑器内容;
    /// 退出入口由右上角浮层提供。状态只存视图内存,不持久化。
    fn toggle_reader_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.reader_focus = !self.reader_focus;
        if self.reader_focus {
            // 详情栏随 chrome 一并隐藏,拖拽锚点必须失效。
            self.details_resize_anchor = None;
        }
        // 键盘焦点交还内容面,不留在工具条/浮层按钮上。
        let focus = self.focus_handle(cx);
        focus.focus(window, cx);
        cx.notify();
    }

    fn begin_details_resize(&mut self, event: &MouseDownEvent, cx: &mut Context<'_, Self>) {
        if !self.show_details {
            return;
        }
        self.details_resize_anchor = Some((f32::from(event.position.x), self.details_width));
        cx.notify();
    }

    fn update_details_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        let Some((start_x, start_width)) = self.details_resize_anchor else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.details_resize_anchor = None;
            cx.notify();
            return;
        }
        // 分隔条在面板左缘:指针左移使面板变宽。
        let width = reader_presentation::clamp_details_width(
            start_width + (start_x - f32::from(event.position.x)),
        );
        if (width - self.details_width).abs() >= 0.5 {
            self.details_width = width;
            cx.notify();
        }
    }

    fn finish_details_resize(&mut self, _event: &MouseUpEvent, cx: &mut Context<'_, Self>) {
        if self.details_resize_anchor.take().is_some() {
            cx.notify();
        }
    }

    /// git gutter 行条:只按可见行构造元素,色取 `EditorThemeTokens`。
    ///
    /// 滚动同步读输入实体的上一帧布局(`visible_row_range`/`scroll_offset`/
    /// `line_height`);组件库内部行号留白 8px 与源码态 `.py(8)` 同源。
    fn render_git_gutter(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        if self.gutter.marks().is_empty() {
            return div().into_any_element();
        }
        let tokens = cx.global::<Settings>().editor_theme;
        let (line_height, offset_y, visible) = {
            let input = self.input.read(cx);
            (
                input
                    .line_height()
                    .map(f32::from)
                    .unwrap_or(cx.theme().mono_font_size.as_f32() * 1.55),
                f32::from(input.scroll_offset().y),
                input.visible_row_range(),
            )
        };
        let marks: Vec<(u32, GutterMark)> = self
            .gutter
            .marks()
            .iter()
            .copied()
            .filter(|(line, _)| {
                visible
                    .as_ref()
                    .is_none_or(|range| range.contains(&((*line as usize).saturating_sub(1))))
            })
            .collect();
        let mut column = div()
            .id("file-gutter")
            .debug_selector(|| "file-gutter".to_owned())
            .w(px(6.0))
            .h_full()
            .flex_shrink_0()
            .relative()
            .overflow_hidden();
        for (line, mark) in marks {
            let top = offset_y + 8.0 + (line.saturating_sub(1) as f32) * line_height;
            let color = gutter_color(&tokens, mark);
            column = column.child(
                div()
                    .id(SharedString::from(format!("gutter-mark-{line}")))
                    .debug_selector(move || format!("gutter-mark-{line}"))
                    .absolute()
                    .left_0()
                    .top(px(top))
                    .w(px(4.0))
                    .h(px(line_height))
                    .bg(color)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.gutter_click(line, mark, window, cx);
                        }),
                    ),
            );
        }
        column.into_any_element()
    }

    /// 行标点击:光标跳到该行并向外层 code tab 发事件(打开差异对照面)。
    fn gutter_click(
        &mut self,
        line: u32,
        mark: GutterMark,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.input.update(cx, |input, cx| {
            input.set_cursor_position(Position::new(line.saturating_sub(1), 0), window, cx);
        });
        cx.emit(TextFileEvent::GutterClicked { line, mark });
        cx.notify();
    }

    /// 状态行文案:载入/保存失败优先,其次截断/非法编码/显式只读/预览上限。
    fn notice(&self, cx: &App) -> Option<String> {
        let language = cx.global::<Settings>().language;
        let mut notice = match &self.notice {
            Some(EditorNotice::ReadFailed(error)) => {
                Some(language.format(Message::EditorReadFailed, &[("error", error)]))
            }
            Some(EditorNotice::SaveFailed(error)) => {
                Some(language.format(Message::EditorSaveFailed, &[("error", error)]))
            }
            Some(EditorNotice::SaveChanged) => {
                Some(language.text(Message::EditorSaveChanged).to_owned())
            }
            Some(EditorNotice::SaveReadOnly) => {
                Some(language.text(Message::EditorSaveReadOnly).to_owned())
            }
            Some(EditorNotice::PreviewLimited) => {
                Some(language.text(Message::EditorPreviewLimited).to_owned())
            }
            None => None,
        };
        if notice.is_none()
            && let Some(snapshot) = self.snapshot()
        {
            if snapshot.truncated {
                notice = Some(language.text(Message::EditorTruncated).to_owned());
            } else if snapshot.invalid_encoding {
                notice = Some(language.text(Message::EditorInvalidEncoding).to_owned());
            } else if snapshot.read_only && self.fixed_read_only {
                notice = Some(language.text(Message::EditorReadOnly).to_owned());
            }
        }
        if self.preview && self.outline.limited {
            let detail = language.text(Message::EditorPreviewLimited);
            notice = Some(match notice {
                Some(notice) => format!("{notice} {detail}"),
                None => detail.to_owned(),
            });
        }
        notice
    }
}

impl Render for TextFileView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.restore_edit_cursor(window, cx);
        // 编辑器语义槽单源:正文底色/次要文字取自 `EditorThemeTokens`,
        // 不直接用组件库调色(编辑器语义槽消费纪律)。
        let tokens = cx.global::<Settings>().editor_theme;
        let background = crate::gpui_terminal::colors::from_rgb8(tokens.background);
        let border = cx.theme().border;
        let muted = crate::gpui_terminal::colors::from_rgb8(tokens.muted);
        let mono_font_family = cx.theme().mono_font_family.clone();
        let mono_font_size = cx.theme().mono_font_size;
        let editable = !self.loading && !self.is_read_only();
        let notice = self.notice(cx);
        let status = notice.map(|text| {
            div()
                .h(px(24.0))
                .flex_shrink_0()
                .px(px(10.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(border)
                .text_size(px(11.0))
                .text_color(muted)
                .child(text)
        });
        let content = if self.preview {
            self.render_markdown_preview(cx)
        } else {
            div()
                .id("file-source-content")
                .debug_selector(|| "file-source-content".to_owned())
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .child(self.render_git_gutter(cx))
                .child(
                    div().flex_1().min_w_0().h_full().child(
                        Input::new(&self.input)
                            .h_full()
                            // 只读/载入中禁用输入;可编辑性最终由 TextSnapshot
                            // 的 read_only 合同与保存链兜底。
                            .disabled(!editable)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .rounded(px(0.0))
                            .font_family(mono_font_family)
                            .text_size(mono_font_size)
                            .text_color(crate::gpui_terminal::colors::from_rgb8(tokens.foreground))
                            .line_height(relative(1.55))
                            .px(px(10.0))
                            .py(px(8.0)),
                    ),
                )
                .into_any_element()
        };
        // 聚焦阅读隐藏整条工具条;右上角浮层保留唯一的恢复入口。
        let toolbar = (!self.reader_focus).then(|| self.render_toolbar(editable, window, cx));
        let reader_focus_overlay = self
            .reader_focus
            .then(|| self.render_reader_focus_overlay(cx));
        div()
            .debug_selector(|| "slterm-text-view".to_owned())
            .id("text-file-editor")
            .key_context("FileEditor")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .relative()
            .overflow_hidden()
            .bg(background)
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                // 指针离开分隔线时可能收不到带按键的移动:只清理拖拽态,
                // 不影响正常阅读选区。
                if this.details_resize_anchor.is_some()
                    && event.pressed_button != Some(MouseButton::Left)
                {
                    this.details_resize_anchor = None;
                    cx.notify();
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, _, cx| {
                if this.details_resize_anchor.is_some() && event.button == MouseButton::Left {
                    this.finish_details_resize(event, cx);
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleSource, window, cx| {
                if this.markdown {
                    this.toggle_preview(window, cx);
                }
            }))
            // 撤销统一(源码态与渲染态全部文件 tab):捕获组件 Undo/Redo,
            // 先于输入实体内建栈处理,统一走共享 `EditHistory::travel`。
            .capture_action(cx.listener(|view, _: &input::Undo, window, cx| {
                let composing = view.live_edit.as_ref().is_some_and(|edit| {
                    edit.input.update(cx, |input, cx| {
                        input.marked_text_range(window, cx).is_some()
                    })
                });
                if composing {
                    cx.propagate();
                } else if view.document_input_focused(window, cx) {
                    view.travel_history(false, window, cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .capture_action(cx.listener(|view, _: &input::Redo, window, cx| {
                let composing = view.live_edit.as_ref().is_some_and(|edit| {
                    edit.input.update(cx, |input, cx| {
                        input.marked_text_range(window, cx).is_some()
                    })
                });
                if composing {
                    cx.propagate();
                } else if view.document_input_focused(window, cx) {
                    view.travel_history(true, window, cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .when(self.markdown, |root| {
                root.capture_action(cx.listener(|view, _: &Escape, window, cx| {
                    let composing = view.live_edit.as_ref().is_some_and(|edit| {
                        edit.input.update(cx, |input, cx| {
                            input.marked_text_range(window, cx).is_some()
                        })
                    });
                    if composing {
                        cx.propagate();
                    } else if view.live_edit.is_some() {
                        view.finish_live_edit(cx);
                        view.focus.focus(window, cx);
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, action: &Enter, window, cx| {
                    let composing = view.live_edit.as_ref().is_some_and(|edit| {
                        edit.input.update(cx, |input, cx| {
                            input.marked_text_range(window, cx).is_some()
                        })
                    });
                    if composing {
                        cx.propagate();
                    } else if action.secondary && view.live_edit.is_some() {
                        view.finish_live_edit(cx);
                        view.focus.focus(window, cx);
                        cx.stop_propagation();
                    } else if !action.shift
                        && (view.insert_block_on_enter(window, cx)
                            || view.navigate_table(false, true, window, cx)
                            || view.continue_list(window, cx)
                            || view.split_live_paragraph(window, cx))
                    {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &IndentInline, window, cx| {
                    if view.navigate_table(false, false, window, cx)
                        || view.indent_list(false, window, cx)
                    {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &OutdentInline, window, cx| {
                    if view.navigate_table(true, false, window, cx)
                        || view.indent_list(true, window, cx)
                    {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveLeft, window, cx| {
                    if view.move_live_edge(true, false, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveRight, window, cx| {
                    if view.move_live_edge(false, false, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveUp, window, cx| {
                    if view.move_live_edge(true, false, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveDown, window, cx| {
                    if view.move_live_edge(false, false, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveToStart, window, cx| {
                    if view.move_live_edge(true, true, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(|view, _: &MoveToEnd, window, cx| {
                    if view.move_live_edge(false, true, window, cx) {
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .capture_action(cx.listener(
                    |view, _: &Backspace, window, cx| {
                        if view.backspace_list(window, cx) || view.join_live_paragraph(window, cx) {
                            cx.stop_propagation();
                        } else {
                            cx.propagate();
                        }
                    },
                ))
            })
            .on_action(cx.listener(|this, _: &FinishBlockEdit, window, cx| {
                let composing = this.live_edit.as_ref().is_some_and(|edit| {
                    edit.input.update(cx, |input, cx| {
                        input.marked_text_range(window, cx).is_some()
                    })
                });
                if composing {
                    // IME 组合期:让原生实体自行收尾,不替换输入。
                    if let Some(edit) = &this.live_edit {
                        edit.input
                            .update(cx, |input, cx| input.unmark_text(window, cx));
                    }
                    cx.notify();
                } else {
                    this.finish_live_edit(cx);
                    this.focus.focus(window, cx);
                }
            }))
            .on_action(cx.listener(|view, _: &BoldSelection, window, cx| {
                view.format_live_selection("**", window, cx);
            }))
            .on_action(cx.listener(|view, _: &ItalicSelection, window, cx| {
                view.format_live_selection("_", window, cx);
            }))
            .when(self.preview && self.live_edit.is_none(), |root| {
                root.capture_action(cx.listener(|this, _: &SelectAll, _, cx| {
                    for block in this.blocks.borrow().iter().flatten() {
                        block.update(cx, |state, cx| state.select_all(cx));
                    }
                    this.all_selected = true;
                    cx.stop_propagation();
                    cx.notify();
                }))
                .capture_action(cx.listener(|this, _: &Copy, window, cx| {
                    if this.all_selected {
                        // 不为复制物化屏外块。
                        let text = this.input.read(cx).value().to_string();
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        cx.stop_propagation();
                    } else if let Some(text) = this.inline_selected_text(window, cx) {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.all_selected = false;
                        cx.notify();
                    }),
                )
            })
            .children(toolbar)
            .when_some(status, |root, status| root.child(status))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_start()
                    .child(content)
                    // 聚焦阅读同时隐藏详情栏,只留编辑器内容。
                    .when(self.show_details && !self.reader_focus, |row| {
                        row.child(if self.info {
                            self.render_info(cx)
                        } else if self.markdown {
                            self.render_outline(cx)
                        } else {
                            self.render_info(cx)
                        })
                    }),
            )
            .children(reader_focus_overlay)
            .when(self.details_resize_anchor.is_some(), |root| {
                root.child(
                    div()
                        .id("file-details-resize-overlay")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .cursor_col_resize()
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.update_details_resize(event, cx);
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseUpEvent, _, cx| {
                                this.finish_details_resize(event, cx);
                            }),
                        ),
                )
            })
    }
}

/// 快照载入:本地文档(带保存目标)/ HEAD 有界字节;markdown 预解析大纲。
/// 错误保留原始文本供 i18n 包装(后台线程不取语言)。
fn load_document(
    path: &Path,
    source: TextSource,
    markdown: bool,
) -> Result<(LoadedDocument, Option<Outline>, GitGutter), String> {
    match source {
        TextSource::Local => Document::load(path)
            .map(|document| {
                let outline = markdown.then(|| Outline::prepare(&document.text, path.parent()));
                // gutter 与解码同批后台完成,不留 UI 线程 git 子进程。
                let gutter = gutter::read_for_path(path);
                (LoadedDocument::Local(document), outline, gutter)
            })
            .map_err(|error| error.to_string()),
        TextSource::GitHead => {
            let (bytes, truncated) =
                git::head_file(path, MAX_BYTES).map_err(|error| error.to_string())?;
            // HEAD 字节一律只读:gitshow 不提供写回语义。
            Ok((
                LoadedDocument::Head(TextSnapshot::decode_prefix(bytes, true, truncated)),
                None,
                GitGutter::default(),
            ))
        }
    }
}

#[cfg(test)]
mod file_editor_tests {
    use super::file_title;
    use std::path::Path;

    /// 标题派生:文件名优先,无文件名回退完整路径。
    #[test]
    fn title_prefers_the_file_name() {
        assert_eq!(file_title(Path::new("D:/repo/src/main.rs")), "main.rs");
        assert_eq!(file_title(Path::new("D:/repo")), "repo");
    }
}
