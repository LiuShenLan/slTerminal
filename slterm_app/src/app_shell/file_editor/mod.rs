//! 文本文件视图与编辑内核宿主(07 篇):M8.1 只读骨架 → M8.2 源码态可编辑。
//!
//! 载入:本地文件经 [`Document`](document::Document) 有界读 + 快照解码;
//! gitshow 的 HEAD 字节经同一 `TextSnapshot` 通路(`decode_prefix(..,
//! read_only=true, ..)`),encode 层拒绝写回——只读 contract 在类型层。
//!
//! 编辑与保存:输入订阅把每次变更记入 [`EditHistory`](edit_history::EditHistory)
//! 并重算脏标记(当前文本 vs 已保存快照);保存链 = 保存前磁盘字节比对
//! (`verify`)→ 编码(BOM/CRLF 往返)→ 原子写(`atomic_file`)。Changed /
//! ReadOnly / Io 三态经提示横幅上抛,失败不破坏磁盘。
//!
//! 内核族(照抄上游实现,消费点见各文件头):document / edit_history /
//! outline / inline_edit / block_structure / block_inline / input_rules。
//! M8.3 接渲染态预览与 live_edit 输入实体。

#[allow(dead_code)] // 消费点 = M8.3 内联分段渲染(live_edit)。
mod block_inline;
#[allow(dead_code)] // 消费点 = M8.3 结构化渲染与结构命令。
mod block_structure;
mod document;
mod edit_history;
#[allow(dead_code)] // 消费点 = M8.3 live_edit 投影与装饰(M8.2 内核 + 用例)。
mod inline_edit;
#[allow(dead_code)] // 消费点 = M8.3 live_edit 输入规则接线(M8.2 规则清单 + 用例)。
mod input_rules;
#[allow(dead_code)] // 消费点 = M8.3 预览虚拟化与大纲侧栏。
mod outline;

use std::path::{Path, PathBuf};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Task, Window, div, px,
    relative,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::app_shell::config::Settings;
use crate::git;
use crate::i18n::Message;
use crate::text_document::{MAX_BYTES, SaveError, TextSnapshot};
use document::Document;
use edit_history::EditHistory;

gpui::actions!(file_editor, [SaveFile]);

/// 快照来源。gitshow 与本地文件共用视图形态,只有数据源与只读意图不同。
#[derive(Debug, Clone, Copy)]
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

/// 视图内提示:M8.2 只覆盖载入失败与保存失败三态;文案在渲染时取语言。
enum EditorNotice {
    ReadFailed(String),
    SaveFailed(String),
    SaveChanged,
    SaveReadOnly,
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
    /// 打开意图的只读位(gitshow true;普通源码 false;预览臂 M8.3 再定)。
    fixed_read_only: bool,
    /// 本地文档(保存目标 + 载入快照);HEAD 臂为 None。
    document: Option<Document>,
    /// HEAD 只读快照(gitshow)。
    head: Option<TextSnapshot>,
    /// 统一撤销历史(M8.2 记录源码键入;M8.3 live_edit 同栈)。
    history: EditHistory,
    /// 已保存文本(脏标记基线):载入/保存成功时更新。
    saved_text: Option<String>,
    dirty: bool,
    saving: bool,
    loading: bool,
    notice: Option<EditorNotice>,
    _input_subscription: Subscription,
}

impl TextFileView {
    /// 本地文件载入(`read_only` = 打开意图固定只读;普通源码传 false)。
    pub fn new(
        path: PathBuf,
        language: &'static str,
        read_only: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_source(path, language, TextSource::Local, read_only, window, cx)
    }

    /// gitshow 只读臂:HEAD 字节 + 恒定只读。
    pub fn new_git_head(
        path: PathBuf,
        language: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_source(path, language, TextSource::GitHead, true, window, cx)
    }

    fn new_with_source(
        path: PathBuf,
        language: &'static str,
        source: TextSource,
        fixed_read_only: bool,
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
        // 键入热路径:先从 Rope 提取局部差量入历史,再用当前文本重算脏标记。
        // `set_value`(载入/重载)抑制 Change 事件,不会污染历史。
        let subscription = cx.subscribe_in(&input, window, |view, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let input = view.input.read(cx);
                view.history.record_rope(input.text());
                view.dirty = view.saved_text.as_deref() != Some(input.value().as_ref());
                cx.notify();
            }
        });
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

    /// 只读合同查询:固定只读位或快照自身只读(截断/非法编码/只读文件)。
    pub fn is_read_only(&self) -> bool {
        self.fixed_read_only || self.snapshot().is_some_and(|snapshot| snapshot.read_only)
    }

    /// 脏标记:tab 标题「•」与关闭确认的数据源。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 保存中:关闭确认在此态不弹窗,避免与在途写盘竞争。
    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
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

    /// 重新读盘(同路径再次打开 / 外部修改确认后)。
    ///
    /// 脏/保存中/载入中一律拒绝:草稿与撤销历史不得被重读盘静默覆盖
    /// (07 篇数据流「同路径重开保留草稿」)。
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

    fn snapshot(&self) -> Option<&TextSnapshot> {
        self.document.as_deref().or(self.head.as_ref())
    }

    /// 后台载入:读盘/解码不占前台线程。
    fn spawn_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let path = self.path.clone();
        let source = self.source;
        let task = cx
            .background_executor()
            .spawn(async move { load_document(&path, source) });
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
        loaded: Result<LoadedDocument, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match loaded {
            Ok(LoadedDocument::Local(document)) => {
                let text = document.text.clone();
                self.input.update(cx, |input, cx| {
                    input.set_value(text.clone(), window, cx);
                });
                self.saved_text = Some(text);
                self.document = Some(document);
                self.head = None;
            }
            Ok(LoadedDocument::Head(snapshot)) => {
                let text = snapshot.text.clone();
                self.input.update(cx, |input, cx| {
                    input.set_value(text.clone(), window, cx);
                });
                self.saved_text = Some(text);
                self.head = Some(snapshot);
                self.document = None;
            }
            Err(error) => self.notice = Some(EditorNotice::ReadFailed(error)),
        }
        self.history
            .reset(self.saved_text.as_deref().unwrap_or_default());
        self.dirty = false;
        self.loading = false;
        cx.notify();
    }

    /// 状态行文案:载入/保存失败优先,其次截断/非法编码/显式只读。
    fn notice(&self, cx: &App) -> Option<String> {
        let language = cx.global::<Settings>().language;
        match &self.notice {
            Some(EditorNotice::ReadFailed(error)) => {
                return Some(language.format(Message::EditorReadFailed, &[("error", error)]));
            }
            Some(EditorNotice::SaveFailed(error)) => {
                return Some(language.format(Message::EditorSaveFailed, &[("error", error)]));
            }
            Some(EditorNotice::SaveChanged) => {
                return Some(language.text(Message::EditorSaveChanged).to_owned());
            }
            Some(EditorNotice::SaveReadOnly) => {
                return Some(language.text(Message::EditorSaveReadOnly).to_owned());
            }
            None => {}
        }
        let snapshot = self.snapshot()?;
        if snapshot.truncated {
            return Some(language.text(Message::EditorTruncated).to_owned());
        }
        if snapshot.invalid_encoding {
            return Some(language.text(Message::EditorInvalidEncoding).to_owned());
        }
        (snapshot.read_only && self.fixed_read_only)
            .then(|| language.text(Message::EditorReadOnly).to_owned())
    }
}

impl Render for TextFileView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = cx.theme();
        let notice = self.notice(cx);
        let status = notice.map(|text| {
            div()
                .h(px(24.0))
                .flex_shrink_0()
                .px(px(10.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(11.0))
                .text_color(theme.muted_foreground)
                .child(text)
        });
        let disabled = self.is_read_only() || self.loading;
        div()
            .debug_selector(|| "slterm-text-view".to_owned())
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme.background)
            .children(status)
            .child(
                div().flex_1().min_h_0().child(
                    Input::new(&self.input)
                        // 只读/载入中禁用输入;可编辑性最终仍由 TextSnapshot
                        // 的 read_only 合同与保存链兜底。
                        .disabled(disabled)
                        .bordered(false)
                        .focus_bordered(false)
                        .rounded(px(0.0))
                        .font_family(theme.mono_font_family.clone())
                        .text_size(theme.mono_font_size)
                        .line_height(relative(1.55))
                        .px(px(10.0))
                        .py(px(8.0)),
                ),
            )
    }
}

/// 快照载入:本地文档(带保存目标)/ HEAD 有界字节。错误保留原始文本供
/// i18n 包装(后台线程不取语言)。
fn load_document(path: &Path, source: TextSource) -> Result<LoadedDocument, String> {
    match source {
        TextSource::Local => Document::load(path)
            .map(LoadedDocument::Local)
            .map_err(|error| error.to_string()),
        TextSource::GitHead => {
            let (bytes, truncated) =
                git::head_file(path, MAX_BYTES).map_err(|error| error.to_string())?;
            // HEAD 字节一律只读:gitshow 不提供写回语义(07 篇改造节 5)。
            Ok(LoadedDocument::Head(TextSnapshot::decode_prefix(
                bytes, true, truncated,
            )))
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
