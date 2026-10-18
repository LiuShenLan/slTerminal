//! 文件树图标字形(开放问题 4 选型落地)。
//!
//! 选型 = **内嵌 Nerd Font 字形**(Maple Mono NF CN,`REQUIRED_FONT_FAMILY`),
//! 不引入 svg/位图资源:形归实现、色归主题 token(渲染侧继承行文字色,
//! 本文件不含任何颜色)。每个码位都已在该字体的 cmap 内(上游逐个核对过)。

pub(crate) const ICON_FOLDER: &str = "\u{f114}";
pub(crate) const ICON_FOLDER_OPEN: &str = "\u{f115}";
pub(crate) const ICON_TERMINAL: &str = "\u{ea85}";
pub(crate) const ICON_FILE: &str = "\u{ea7b}";
pub(crate) const ICON_CHEVRON_RIGHT: &str = "\u{eab6}";
pub(crate) const ICON_CHEVRON_DOWN: &str = "\u{eab4}";

pub(crate) fn folder_icon(expanded: bool) -> &'static str {
    if expanded {
        ICON_FOLDER_OPEN
    } else {
        ICON_FOLDER
    }
}

pub(crate) fn chevron_icon(expanded: bool) -> &'static str {
    if expanded {
        ICON_CHEVRON_DOWN
    } else {
        ICON_CHEVRON_RIGHT
    }
}

/// 按扩展名取文件类型图标;未命中回通用文件字形。
pub(crate) fn file_type_icon(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with(".git") {
        return "\u{e65d}";
    }
    let ext = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match ext {
        "md" | "markdown" => "\u{eb1d}",
        "json" | "jsonl" | "ndjson" => "\u{eb0f}",
        "toml" => "\u{e6b2}",
        "yml" | "yaml" => "\u{e6a8}",
        "xml" => "\u{e619}",
        "rs" => "\u{e68b}",
        "py" => "\u{e606}",
        "js" | "mjs" | "cjs" | "jsx" => "\u{e60c}",
        "ts" | "tsx" => "\u{e628}",
        "html" | "htm" => "\u{e60e}",
        "css" | "scss" | "less" => "\u{e614}",
        "c" | "h" => "\u{e61e}",
        "cpp" | "cc" | "cxx" | "hpp" => "\u{e61d}",
        "cs" => "\u{e648}",
        "java" => "\u{e66d}",
        "go" => "\u{e627}",
        "sh" | "bash" | "zsh" => "\u{e691}",
        "ps1" | "psm1" | "psd1" => "\u{e683}",
        "bat" | "cmd" => ICON_TERMINAL,
        "sql" | "db" | "sqlite" => "\u{e64d}",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" => "\u{e60d}",
        "zip" | "7z" | "rar" | "gz" | "tar" | "xz" | "zst" => "\u{f1c6}",
        "pdf" => "\u{f1c1}",
        "lock" => "\u{e672}",
        "log" => "\u{f4ed}",
        "txt" => "\u{f0f6}",
        _ => ICON_FILE,
    }
}

#[cfg(test)]
mod icons_tests {
    use super::{ICON_FILE, ICON_FOLDER, file_type_icon, folder_icon};

    #[test]
    fn shared_icons_cover_directories_known_files_and_fallbacks() {
        assert_eq!(folder_icon(false), ICON_FOLDER);
        assert_ne!(file_type_icon("main.rs"), ICON_FILE);
        assert_eq!(file_type_icon("README.unknown"), ICON_FILE);
    }
}
