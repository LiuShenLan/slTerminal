//! 文本文件视图(07 篇):M8.1 只读骨架,M8.2 起为双模态编辑器内核宿主。
//!
//! 本步只点亮只读臂:本地文件有界前缀读取 → [`TextSnapshot`](crate::text_document)
//! → 禁交互的代码输入体。gitshow 的 HEAD 字节经同一快照通路载入
//! (`decode_prefix(.., read_only=true, ..)`),encode 层拒绝写回——只读
//! contract 在类型层,不靠 UI 层自觉。
//!
//! 载入纪律:读盘/解码在后台 executor,前台只在 `apply_loaded` 落快照并
//! `set_value`;窗口句柄关闭时任务结果静默丢弃。M8.2 在此实体上接
//! Document/EditHistory/Outline/live_edit;M8.3 接渲染态与虚拟化。

use std::path::{Path, PathBuf};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputState};

use crate::document_io;
use crate::git;
use crate::i18n::Message;
use crate::text_document::{MAX_BYTES, TextSnapshot};

/// 快照来源。gitshow 与本地文件共用视图形态,只有数据源与只读意图不同。
#[derive(Debug, Clone, Copy)]
enum TextSource {
    /// 工作区/磁盘当前字节(有界前缀)。
    Local,
    /// 当前 HEAD 中的 blob 字节(只读)。
    GitHead,
}

/// 文件标题:文件名优先,取不到时退回完整路径。
fn file_title(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// M8.2 的保存链/编辑内核接缝;本步只读。
pub struct TextFileView {
    pub path: PathBuf,
    pub title: String,
    /// 源码态原生输入(gpui-component code editor)。M8.1 恒 disabled。
    input: Entity<InputState>,
    source: TextSource,
    /// 打开意图的只读位(gitshow/预览臂 true;M8.2 的 Code 可编辑态翻 false)。
    fixed_read_only: bool,
    snapshot: Option<TextSnapshot>,
    /// 原始错误文本(渲染时经 i18n `{error}` 格式化,不在后台线程取语言)。
    error: Option<String>,
    loading: bool,
}

impl TextFileView {
    /// 本地文件只读载入(M8.1 的 Document/Code 共用;M8.2 起 Code 传 false)。
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
        let mut view = Self {
            path,
            title,
            input,
            source,
            fixed_read_only,
            snapshot: None,
            error: None,
            loading: true,
        };
        view.spawn_load(window, cx);
        view
    }

    /// Tab 标题(脏标记「•」归 M8.2 的 Document 状态机)。
    pub fn tab_title(&self) -> String {
        self.title.clone()
    }

    /// 只读合同查询:固定只读位或快照自身只读(截断/非法编码/只读文件)。
    pub fn is_read_only(&self) -> bool {
        self.fixed_read_only
            || self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.read_only)
    }

    /// M8.2 脏标记数据源;只读骨架恒 false(尚无编辑面)。
    #[allow(dead_code)] // 消费点 = M8.2 tab 标题「•」与关闭确认。
    pub fn is_dirty(&self) -> bool {
        false
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }

    /// 测试专用:后台载入完成后的快照文本。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn text_for_test(&self) -> Option<String> {
        self.snapshot.as_ref().map(|snapshot| snapshot.text.clone())
    }

    /// 重新读盘(同路径再次打开 / 外部修改确认后)。M8.2 起保留草稿与撤销历史。
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.loading = true;
        self.error = None;
        self.snapshot = None;
        self.spawn_load(window, cx);
        cx.notify();
    }

    /// 后台载入:读盘/解码不占前台线程。
    fn spawn_load(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let path = self.path.clone();
        let source = self.source;
        let read_only = self.fixed_read_only;
        let task = cx
            .background_executor()
            .spawn(async move { load_snapshot(&path, source, read_only) });
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
        loaded: Result<TextSnapshot, String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match loaded {
            Ok(snapshot) => {
                self.input.update(cx, |input, cx| {
                    input.set_value(snapshot.text.clone(), window, cx);
                });
                self.snapshot = Some(snapshot);
            }
            Err(error) => self.error = Some(error),
        }
        self.loading = false;
        cx.notify();
    }

    /// 状态行文案:错误优先,其次截断/非法编码/显式只读。
    fn notice(&self, cx: &App) -> Option<String> {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        if let Some(error) = &self.error {
            return Some(language.format(Message::EditorReadFailed, &[("error", error)]));
        }
        let snapshot = self.snapshot.as_ref()?;
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
                        // 只读臂:M8.1 一切 tab 均不可编辑;M8.2 的 Code 可编辑态
                        // 去掉 disabled,改由 TextSnapshot.read_only 类型合同约束。
                        .disabled(true)
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

/// 快照载入:本地有界前缀 / HEAD 有界字节。错误保留原始文本供 i18n 包装。
fn load_snapshot(path: &Path, source: TextSource, read_only: bool) -> Result<TextSnapshot, String> {
    match source {
        TextSource::Local => {
            let (bytes, truncated) = document_io::read_path_prefix(path, MAX_BYTES)
                .map_err(|error| error.to_string())?;
            Ok(TextSnapshot::decode_prefix(bytes, read_only, truncated))
        }
        TextSource::GitHead => {
            let (bytes, truncated) =
                git::head_file(path, MAX_BYTES).map_err(|error| error.to_string())?;
            // HEAD 字节一律只读:gitshow 不提供写回语义(07 篇改造节 5)。
            Ok(TextSnapshot::decode_prefix(bytes, true, truncated))
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
