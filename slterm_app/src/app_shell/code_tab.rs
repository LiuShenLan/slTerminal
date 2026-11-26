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
/// 已知二进制/封装格式不进入文本 Editor；其余（含未知扩展名与无扩展名）
/// 按纯文本尝试，是否为文本由读取后的 NUL 探测与编码解码裁决。
const NON_TEXT_EXTENSIONS: &[&str] = &[
    // 归档、安装包与镜像
    "zip", "7z", "rar", "gz", "tgz", "bz2", "xz", "zst", "tar", "msi", "msix", "appx", "iso", "cab",
    "dmg", // 可执行、库与中间产物
    "exe", "dll", "sys", "pdb", "obj", "o", "class", "jar", "pyc",
    "wasm", // 图像与音视频
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "mp3", "wav", "flac", "ogg",
    "mp4", "mkv", "avi", "mov", "webm", // 文档容器、数据库与字体
    "pdf", "docx", "xlsx", "pptx", "odt", "ods", "sqlite", "sqlite3", "db", "bin", "ttf", "otf",
    "woff", "woff2",
];

pub fn viewable_file(path: &Path) -> bool {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => {
            let extension = extension.to_ascii_lowercase();
            !NON_TEXT_EXTENSIONS.contains(&extension.as_str())
        }
        None => true,
    }
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
        for path in ["archive.zip", "app.exe", "image.png"] {
            assert!(!viewable_file(Path::new(path)), "{path} 不得进代码 tab");
        }
        // 无扩展名与未知扩展名按纯文本尝试，内容判定归读取后的编码探测。
        for path in ["no_extension", "script.rb", "notes.log", "data.dat"] {
            assert!(
                viewable_file(Path::new(path)),
                "{path} 必须可尝试按文本打开"
            );
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
            assert!(
                viewable_file(Path::new(unsupported)),
                "未知语言必须回落纯文本而不是拒绝打开"
            );
        }
    }
}
