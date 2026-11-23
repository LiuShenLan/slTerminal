//! 代码文件的语言路由；真实源码面板归 AppRoot 的 DocumentTextView。

use std::path::Path;

/// 路径映射唯一来自 core 的封闭 LanguageRegistry。
pub(crate) fn language_for_path(path: &Path) -> &'static str {
    slterm_document::LanguageRegistry::new()
        .for_path(path)
        .map(slterm_document::LanguageId::as_str)
        .unwrap_or("text")
}

/// 文件树双击是否进源码面板。
pub fn viewable_file(path: &Path) -> bool {
    slterm_document::LanguageRegistry::new()
        .for_path(path)
        .is_some()
}

#[cfg(test)]
mod code_tab_tests {
    use super::*;

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

    #[test]
    fn language_ids_group_extension_families() {
        assert_eq!(language_for_path(Path::new("a.rs")), "rust");
        assert_eq!(language_for_path(Path::new("a.MJS")), "javascript");
        assert_eq!(language_for_path(Path::new("a.yaml")), "yaml");
        assert_eq!(language_for_path(Path::new("a.unknown")), "text");
        assert_eq!(language_for_path(Path::new("Dockerfile")), "dockerfile");
        assert_eq!(language_for_path(Path::new("Makefile")), "make");
        assert_eq!(language_for_path(Path::new("config.JSONC")), "jsonc");
        for unsupported in ["script.rb", "page.php", "video.swift"] {
            assert_eq!(language_for_path(Path::new(unsupported)), "text");
            assert!(!viewable_file(Path::new(unsupported)));
        }
    }
}
