//! 文档打开路由判定。
//!
//! Markdown 文档由 AppRoot 的 `MarkdownView` 提供渲染态与源码态；图片与二进制
//! 按最终排除项交系统默认程序，不在应用内建立面板或路由。

use std::path::Path;

/// Markdown 文档判定（Document 打开链的 Markdown 路由臂）。
pub fn markdown_viewable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown")
    )
}

#[cfg(test)]
mod doc_tabs_tests {
    use super::*;

    /// Markdown 值集大小写不敏感，且不吞掉源码或其他文档扩展名。
    #[test]
    fn markdown_set_is_case_insensitive_and_excludes_source() {
        assert!(markdown_viewable(Path::new("README.md")));
        assert!(markdown_viewable(Path::new("D.MARKDOWN")));
        assert!(!markdown_viewable(Path::new("e.rs")));
        assert!(!markdown_viewable(Path::new("photo.png")));
    }
}
