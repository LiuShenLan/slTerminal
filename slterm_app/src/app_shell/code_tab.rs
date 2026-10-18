//! 代码文件 tab(07 篇):M8.1 只读臂 = 普通源码 + gitshow。
//!
//! 普通源码与 gitshow 共用 [`TextFileView`] 快照通路;两者的区别只在数据源
//! (磁盘前缀 / HEAD blob)与标题后缀(`<name> @ HEAD`)。可编辑态与三栏
//! Git 合并归 M8.2/M8.5,本步 `Input` 恒 disabled。
//!
//! 扩展名 → 语言映射表是「视图值集」单点:`viewable_file` 与 openable_in_app
//! 的源码判定同源(07 篇照抄点 17,fileViewers 注册表值集并入)。

use std::path::{Path, PathBuf};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div,
};

use super::file_editor::TextFileView;

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

/// 源码文件 tab:普通文件与 gitshow 只读臂共用一个快照视图。
pub struct CodeTabView {
    pub path: PathBuf,
    /// gitshow 只读臂:标题追加 `@ HEAD`,复用同路径时不与普通 tab 混淆。
    git_show: bool,
    file: Entity<TextFileView>,
}

impl CodeTabView {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_source(path, false, window, cx)
    }

    /// gitshow 只读臂:HEAD 字节 + 恒定只读(07 篇改造节 5)。
    pub fn new_git_show(path: PathBuf, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self::new_with_source(path, true, window, cx)
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
            // M8.1 只读臂:可编辑态在 M8.2 翻 false。
            cx.new(|cx| TextFileView::new(file_path, language, true, window, cx))
        };
        Self {
            path,
            git_show,
            file,
        }
    }

    /// 同路径普通源码 tab 的重开命中(草稿保留归 M8.2,本步只重读盘)。
    pub fn is_regular_path(&self, path: &Path) -> bool {
        !self.git_show && self.path == path
    }

    /// 同路径 gitshow tab 的重开命中。
    pub fn is_git_show_path(&self, path: &Path) -> bool {
        self.git_show && self.path == path
    }

    pub fn tab_title(&self, cx: &App) -> String {
        let base = self.file.read(cx).tab_title();
        if self.git_show {
            format!("{base} @ HEAD")
        } else {
            base
        }
    }

    /// 只读合同(测试与 M8.2 关闭确认共用)。
    #[allow(dead_code)] // 消费点 = M8.2 关闭确认;M8.1 出口用例经 workspace 只读断言。
    pub fn is_read_only(&self, cx: &App) -> bool {
        self.file.read(cx).is_read_only()
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.file.read(cx).focus_handle(cx)
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.file.update(cx, |file, cx| file.reload(window, cx));
    }

    /// 测试专用:已载入快照文本(HEAD/工作区字节经同一通路)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn text_for_test(&self, cx: &App) -> Option<String> {
        self.file.read(cx).text_for_test()
    }
}

impl Render for CodeTabView {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "slterm-code-tab".to_owned())
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(self.file.clone())
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
