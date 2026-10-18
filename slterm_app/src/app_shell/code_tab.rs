//! 代码文件 tab(07 篇):普通源码(可编辑)+ gitshow 只读臂 + 通用 diff
//! 只读臂 + 冲突三栏合并臂。
//!
//! 普通源码与 gitshow 共用 [`TextFileView`] 快照通路;两者的区别只在数据源
//! (磁盘前缀 / HEAD blob)与标题后缀(`<name> @ HEAD`)。M8.2 起普通源码
//! 源码态可编辑可存;M8.5 的通用 diff 落本视图的 Diff 模式(不新增 05 篇
//! tab 变体,07 篇边界条 3);冲突 gutter 点击落 Merge 模式(base/ours/theirs)。
//!
//! 扩展名 → 语言映射表是「视图值集」单点:`viewable_file` 与 openable_in_app
//! 的源码判定同源(07 篇照抄点 17,fileViewers 注册表值集并入)。

use std::path::{Path, PathBuf};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Task, Window, div,
};

use super::diff_tab::{DiffSource, DiffTabView};
use super::file_editor::gutter::GutterMark;
use super::file_editor::{TextFileEvent, TextFileView};
use super::merge_tab::{MergeSide, MergeTabEvent, MergeTabView};

/// code tab 对外事件:gutter 点击由工作区决定打开差异/冲突对照面,
/// merge tab 的采用动作经工作区脏文件门控后回写。
#[derive(Clone, Debug)]
pub enum CodeTabEvent {
    GutterClicked {
        path: PathBuf,
        line: u32,
        mark: GutterMark,
    },
    /// 冲突三栏:请求采用某一侧(工作区先查同路径脏草稿)。
    MergeAdopt { path: PathBuf, side: MergeSide },
    /// 冲突三栏:在工作区编辑器中打开工作区文件。
    MergeOpenInEditor { path: PathBuf },
    /// 冲突三栏:采用动作已成功写盘(gutter/树快照刷新触发点)。
    MergeWritten { path: PathBuf },
}

impl EventEmitter<CodeTabEvent> for CodeTabView {}

/// tab 内容模式:普通文本视图、通用 diff 双栏或冲突三栏合并。
enum CodeTabMode {
    File(Entity<TextFileView>),
    Diff(Entity<DiffTabView>),
    Merge(Entity<MergeTabView>),
}

/// 扩展名 → tree-sitter 语言 id(组件库 highlighter 的注册名)。
fn language_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "md" | "markdown" => "markdown",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "go" => "go",
        "java" => "java",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "bash",
        "ps1" | "psm1" | "psd1" => "powershell",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sql" => "sql",
        "zig" => "zig",
        "cmake" => "cmake",
        "dockerfile" => "dockerfile",
        "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" => "haskell",
        "ini" | "cfg" | "conf" => "ini",
        "vim" => "vim",
        "xml" => "xml",
        "json" | "jsonl" | "ndjson" => "json",
        "txt" | "log" | "text" => "text",
        _ => return None,
    })
}

fn language_for_path(path: &Path) -> &'static str {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(language_for_extension)
        .unwrap_or("text")
}

/// 文件树双击是否进代码 tab(与 markdown/图片路由并列)。
pub fn viewable_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| language_for_extension(extension).is_some())
}

/// 源码文件 tab:普通文件与 gitshow 只读臂共用一个快照视图;
/// Diff 模式承载通用 diff 对照面(只读臂,不新增 tab 变体)。
pub struct CodeTabView {
    pub path: PathBuf,
    /// gitshow 只读臂:标题追加 `@ HEAD`,复用同路径时不与普通 tab 混淆。
    git_show: bool,
    mode: CodeTabMode,
    /// 内层编辑器的变更转发:脏标记/保存完成时让外层 tab 也 notify,
    /// 工作区再经观察刷新侧栏标题与窗口标题。
    _file_observation: Subscription,
    /// gutter 点击转发(TextFileEvent → CodeTabEvent)。
    _gutter_subscription: Option<Subscription>,
    /// merge tab 事件转发(采用/编辑器打开/写盘完成 → CodeTabEvent)。
    _merge_subscription: Option<Subscription>,
}

impl CodeTabView {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_source(path, false, window, cx)
    }

    /// gitshow 只读臂:HEAD 字节 + 恒定只读(07 篇改造节 5)。
    pub fn new_git_show(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_source(path, true, window, cx)
    }

    /// 通用 diff 只读臂:左 HEAD / 右工作区,双栏对照(D07-2 已裁)。
    pub fn new_diff(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let diff = cx.new(|cx| {
            DiffTabView::open(
                DiffSource::HeadToWorktree { path: path.clone() },
                window,
                cx,
            )
        });
        let observation = cx.observe_in(&diff, window, |_, _, _, cx| {
            cx.notify();
        });
        Self {
            path,
            git_show: false,
            mode: CodeTabMode::Diff(diff),
            _file_observation: observation,
            _gutter_subscription: None,
            _merge_subscription: None,
        }
    }

    /// 冲突三栏合并臂:base/ours/theirs 只读对照 + 采用版本动作。
    pub fn new_merge(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let merge = cx.new(|cx| MergeTabView::open(path.clone(), window, cx));
        let observation = cx.observe_in(&merge, window, |_, _, _, cx| {
            cx.notify();
        });
        let merge_events = cx.subscribe_in(
            &merge,
            window,
            |this: &mut Self, _, event: &MergeTabEvent, _, cx| {
                match event {
                    MergeTabEvent::Adopt(side) => cx.emit(CodeTabEvent::MergeAdopt {
                        path: this.path.clone(),
                        side: *side,
                    }),
                    MergeTabEvent::OpenInEditor => cx.emit(CodeTabEvent::MergeOpenInEditor {
                        path: this.path.clone(),
                    }),
                    MergeTabEvent::Written => cx.emit(CodeTabEvent::MergeWritten {
                        path: this.path.clone(),
                    }),
                }
                cx.notify();
            },
        );
        Self {
            path,
            git_show: false,
            mode: CodeTabMode::Merge(merge),
            _file_observation: observation,
            _gutter_subscription: None,
            _merge_subscription: Some(merge_events),
        }
    }

    fn new_with_source(
        path: PathBuf,
        git_show: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let language = language_for_path(&path);
        let file_path = path.clone();
        let file = if git_show {
            cx.new(|cx| TextFileView::new_git_head(file_path, language, window, cx))
        } else {
            // M8.2:普通源码 = 可编辑源码态;只读仍由快照合同兜底。
            cx.new(|cx| TextFileView::new(file_path, language, false, window, cx))
        };
        let observation = cx.observe_in(&file, window, |_, _, _, cx| {
            // 内层脏标记/保存状态变化 → 外层重渲;工作区经 observe 外层
            // 刷新 tab 标题。
            cx.notify();
        });
        let gutter_subscription = cx.subscribe_in(
            &file,
            window,
            |this: &mut Self, _, event: &TextFileEvent, _, cx| {
                let TextFileEvent::GutterClicked { line, mark } = event;
                cx.emit(CodeTabEvent::GutterClicked {
                    path: this.path.clone(),
                    line: *line,
                    mark: *mark,
                });
                cx.notify();
            },
        );
        Self {
            path,
            git_show,
            mode: CodeTabMode::File(file),
            _file_observation: observation,
            _gutter_subscription: Some(gutter_subscription),
            _merge_subscription: None,
        }
    }

    fn file(&self) -> Option<&Entity<TextFileView>> {
        match &self.mode {
            CodeTabMode::File(file) => Some(file),
            CodeTabMode::Diff(_) | CodeTabMode::Merge(_) => None,
        }
    }

    fn diff(&self) -> Option<&Entity<DiffTabView>> {
        match &self.mode {
            CodeTabMode::Diff(diff) => Some(diff),
            CodeTabMode::File(_) | CodeTabMode::Merge(_) => None,
        }
    }

    /// 冲突三栏实体(工作区脏门控后经此发起采用写盘)。
    pub(crate) fn merge(&self) -> Option<&Entity<MergeTabView>> {
        match &self.mode {
            CodeTabMode::Merge(merge) => Some(merge),
            CodeTabMode::File(_) | CodeTabMode::Diff(_) => None,
        }
    }

    /// 脏标记(关闭确认数据源;gitshow/diff 恒 false)。
    pub fn is_dirty(&self, cx: &App) -> bool {
        self.file().is_some_and(|file| file.read(cx).is_dirty())
    }

    /// 保存中(关闭确认在此态不弹窗)。
    pub fn is_saving(&self, cx: &App) -> bool {
        self.file().is_some_and(|file| file.read(cx).is_saving())
    }

    /// 触发保存;只读臂(diff/gitshow/merge)恒无操作。
    pub fn save(&mut self, cx: &mut Context<'_, Self>) -> Task<bool> {
        match &self.mode {
            CodeTabMode::File(file) => {
                let file = file.clone();
                file.update(cx, |file, cx| file.save(cx))
            }
            CodeTabMode::Diff(_) | CodeTabMode::Merge(_) => Task::ready(true),
        }
    }

    /// 同路径普通源码 tab 的重开命中(草稿保留归 M8.2,本步只重读盘)。
    pub fn is_regular_path(&self, path: &Path) -> bool {
        self.file().is_some() && !self.git_show && self.path == path
    }

    /// 外部 watcher 兴趣读端(B8):仅 File 模式的普通源码返回磁盘路径;
    /// gitshow、diff 与 merge 只读臂不登记。
    pub(crate) fn editable_path(&self) -> Option<&Path> {
        match &self.mode {
            CodeTabMode::File(_) if !self.git_show => Some(self.path.as_path()),
            CodeTabMode::File(_) | CodeTabMode::Diff(_) | CodeTabMode::Merge(_) => None,
        }
    }

    /// 外部修改触发点(B8):重读磁盘 gutter 快照并刷新行标。
    /// 只读/gitshow 的跳过规则在 `TextFileView::refresh_gutter_from_disk`。
    pub(crate) fn refresh_gutter(&mut self, cx: &mut Context<'_, Self>) {
        let Some(file) = self.file() else {
            return;
        };
        let file = file.clone();
        file.update(cx, |file, cx| file.refresh_gutter_from_disk(cx));
    }

    /// 同路径 gitshow tab 的重开命中。
    pub fn is_git_show_path(&self, path: &Path) -> bool {
        self.file().is_some() && self.git_show && self.path == path
    }

    /// 同路径 HEAD↔工作区 diff tab 的重开命中。
    pub fn is_diff_path(&self, path: &Path) -> bool {
        self.diff().is_some() && self.path == path
    }

    /// 同路径冲突三栏 tab 的重开命中。
    pub fn is_merge_path(&self, path: &Path) -> bool {
        self.merge().is_some() && self.path == path
    }

    pub fn tab_title(&self, cx: &App) -> String {
        if let Some(diff) = self.diff() {
            return diff.read(cx).title.clone();
        }
        if let Some(merge) = self.merge() {
            return merge.read(cx).title.clone();
        }
        let Some(file) = self.file() else {
            return self.path.display().to_string();
        };
        let base = file.read(cx).tab_title();
        if self.git_show {
            format!("{base} @ HEAD")
        } else {
            base
        }
    }

    /// 只读合同(测试与关闭/保存路由共用)。
    pub fn is_read_only(&self, cx: &App) -> bool {
        match &self.mode {
            CodeTabMode::File(file) => file.read(cx).is_read_only(),
            CodeTabMode::Diff(diff) => diff.read(cx).is_read_only(),
            CodeTabMode::Merge(merge) => merge.read(cx).is_read_only(),
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.mode {
            CodeTabMode::File(file) => file.read(cx).focus_handle(cx),
            CodeTabMode::Diff(diff) => diff.read(cx).focus_handle(cx),
            CodeTabMode::Merge(merge) => merge.read(cx).focus_handle(cx),
        }
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        match &self.mode {
            CodeTabMode::File(file) => file.update(cx, |file, cx| file.reload(window, cx)),
            CodeTabMode::Diff(diff) => diff.update(cx, |diff, cx| diff.reload(window, cx)),
            CodeTabMode::Merge(merge) => merge.update(cx, |merge, cx| merge.reload(window, cx)),
        }
    }

    /// 渲染激活位:workspace 在 tab 切换时经此释放/恢复预览资产。
    pub(super) fn set_render_active(
        &mut self,
        active: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(file) = self.file() {
            file.update(cx, |file, cx| file.set_render_active(active, window, cx));
        }
    }

    /// 测试专用:已载入快照文本(HEAD/工作区字节经同一通路)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn text_for_test(&self, cx: &App) -> Option<String> {
        self.file().and_then(|file| file.read(cx).text_for_test())
    }

    /// 测试专用:输入实体当前文本(含未保存草稿)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn input_text_for_test(&self, cx: &App) -> String {
        self.file()
            .map(|file| file.read(cx).input_text_for_test(cx))
            .unwrap_or_default()
    }

    /// 测试专用:共享撤销栈组数。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn history_undo_depth_for_test(&self, cx: &App) -> usize {
        self.file()
            .map(|file| file.read(cx).history_undo_depth_for_test())
            .unwrap_or(0)
    }

    /// 测试专用:diff 模式的两栏 hunk 集与活动 hunk。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn diff_state_for_test(
        &self,
        cx: &App,
    ) -> Option<(Vec<super::diff_tab::DiffHunk>, Option<usize>)> {
        let diff = self.diff()?;
        let diff = diff.read(cx);
        Some((diff.hunks().to_vec(), diff.active_hunk()))
    }

    /// 测试专用:驱动 diff 的 hunk 导航。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn navigate_diff_hunk_for_test(&mut self, forward: bool, cx: &mut Context<'_, Self>) {
        if let Some(diff) = self.diff() {
            let diff = diff.clone();
            diff.update(cx, |diff, cx| {
                if forward {
                    diff.next_hunk(cx);
                } else {
                    diff.prev_hunk(cx);
                }
            });
        }
    }

    /// 测试专用:gutter 行标(File 模式)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn gutter_marks_for_test(&self, cx: &App) -> Vec<(u32, GutterMark)> {
        self.file()
            .map(|file| file.read(cx).gutter_marks_for_test())
            .unwrap_or_default()
    }

    /// 测试专用:merge 模式的三栏文本(非 merge 模式 None)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn merge_state_for_test(
        &self,
        cx: &App,
    ) -> Option<(Option<String>, Option<String>, Option<String>)> {
        self.merge().map(|merge| merge.read(cx).texts_for_test())
    }

    /// 测试专用:merge 模式的视图内提示。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn merge_notice_for_test(&self, cx: &App) -> Option<String> {
        self.merge()
            .and_then(|merge| merge.read(cx).notice_for_test())
    }
}

impl Render for CodeTabView {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "slterm-code-tab".to_owned())
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(match &self.mode {
                CodeTabMode::File(file) => file.clone().into_any_element(),
                CodeTabMode::Diff(diff) => diff.clone().into_any_element(),
                CodeTabMode::Merge(merge) => merge.clone().into_any_element(),
            })
    }
}

#[cfg(test)]
mod code_tab_tests {
    use super::*;

    /// 映射表覆盖源码/配置/脚本族,未知扩展名不认;大小写不敏感。
    #[test]
    fn viewable_file_maps_known_source_extensions_case_insensitively() {
        for path in [
            "main.rs",
            "README.MD",
            "script.PS1",
            "config.toml",
            "events.jsonl",
            "page.html",
            "notes.txt",
        ] {
            assert!(viewable_file(Path::new(path)), "{path} 必须可查看");
        }
        for path in ["archive.zip", "app.exe", "image.png", "no_extension"] {
            assert!(!viewable_file(Path::new(path)), "{path} 不得进代码 tab");
        }
    }

    /// 语言 id 映射:同一扩展名族归同一高亮语言。
    #[test]
    fn language_ids_group_extension_families() {
        assert_eq!(language_for_path(Path::new("a.rs")), "rust");
        assert_eq!(language_for_path(Path::new("a.MJS")), "javascript");
        assert_eq!(language_for_path(Path::new("a.yaml")), "yaml");
        assert_eq!(language_for_path(Path::new("a.unknown")), "text");
    }
}
