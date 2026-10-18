//! 文档图片 URL 重写(07 篇 M8.3 照抄点 13)。
//!
//! 本地 GIF / 动画 WebP 压成缓存里的单帧 PNG;http(s)/data: 原样透传;
//! 静态 PNG/JPEG 不重读(长文档里每次解析都重读大图不可接受)。
//!
//! 因果链:不压帧则 gpui 会播 GIF,且 Markdown 多图共用 element id 时
//! 越界 panic——压帧是正确性措施,不是性能优化。

use std::path::Path;
use std::sync::LazyLock;

/// Markdown 图片语法 `![alt](url)`;URL 到第一个空白/右括号为止。
static MARKDOWN_IMG: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(!\[[^\]]*\]\()([^)\s]+)"#).unwrap());
/// HTML `<img src="…">` 双引号形态(regex 不支持反向引用,单双引号分开写)。
static HTML_IMG_DQ: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?i)(<img\b[^>]*?\bsrc\s*=\s*")([^"]+)""#).unwrap());
static HTML_IMG_SQ: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?i)(<img\b[^>]*?\bsrc\s*=\s*')([^']+)'"#).unwrap());
/// 引用式图片用法 `![alt][id]`(M8.3 问题 2:引用式动图压帧扩展)。
static MD_IMG_REF: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"!\[[^\]]*\]\[([^\]]+)\]"#).unwrap());
/// 链接引用定义 `[id]: url`(只重写被图片引用命中的 id)。
static MD_REF_DEF: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?m)^( {0,3})\[([^\]]+)\]:[ \t]*(\S+)"#).unwrap());

/// 文档级图片 URL 重写:`base` = 文档所在目录(相对路径解析基址)。
pub(super) fn rewrite_doc_images(text: &str, base: Option<&Path>) -> String {
    let rewritten = MARKDOWN_IMG.replace_all(text, |caps: &regex::Captures<'_>| {
        format!("{}{}", &caps[1], flatten_local_image_url(&caps[2], base))
    });
    let rewritten = HTML_IMG_DQ.replace_all(&rewritten, |caps: &regex::Captures<'_>| {
        format!(
            r#"{}{}""#,
            &caps[1],
            flatten_local_image_url(&caps[2], base)
        )
    });
    let rewritten = HTML_IMG_SQ
        .replace_all(&rewritten, |caps: &regex::Captures<'_>| {
            format!("{}{}'", &caps[1], flatten_local_image_url(&caps[2], base))
        })
        .into_owned();
    // 引用式图片:`![alt][id]` + `[id]: anim.gif`。只改被图片引用的定义,
    // 纯链接引用保持原样(链接语义不被压帧改写)。
    let referenced: std::collections::HashSet<String> = MD_IMG_REF
        .captures_iter(&rewritten)
        .map(|caps| caps[1].to_ascii_lowercase())
        .collect();
    if referenced.is_empty() {
        return rewritten;
    }
    MD_REF_DEF
        .replace_all(&rewritten, |caps: &regex::Captures<'_>| {
            if !referenced.contains(&caps[2].to_ascii_lowercase()) {
                return caps[0].to_owned();
            }
            format!(
                "{}[{}]: {}",
                &caps[1],
                &caps[2],
                flatten_local_image_url(&caps[3], base)
            )
        })
        .into_owned()
}

/// 单个 URL 的压帧:网络/内联原样;仅本地 GIF/动图 WebP 进缓存。
fn flatten_local_image_url(url: &str, base: Option<&Path>) -> String {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:") {
        return url.to_owned();
    }
    let extension = Path::new(url)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("gif") && !extension.eq_ignore_ascii_case("webp") {
        return url.to_owned();
    }
    let path = Path::new(url);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match base {
            Some(base) => base.join(path),
            None => return url.to_owned(),
        }
    };
    let Ok(bytes) = std::fs::read(&resolved) else {
        return url.to_owned();
    };
    let Some(png) = flatten_animated_to_png(&bytes) else {
        return url.to_owned();
    };
    let cache = std::env::temp_dir().join("slterm-md-img");
    if std::fs::create_dir_all(&cache).is_err() {
        return url.to_owned();
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&bytes, &mut hasher);
    let dest = cache.join(format!("{:016x}.png", std::hash::Hasher::finish(&hasher)));
    if !dest.exists() && std::fs::write(&dest, png).is_err() {
        return url.to_owned();
    }
    dest.to_string_lossy().replace('\\', "/")
}

/// GIF(含单帧)与动画 WebP → 单帧 PNG;其余格式返回 None(不重读)。
fn flatten_animated_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    if is_gif(bytes) || is_animated_webp(bytes) {
        return Some(first_frame_png(bytes));
    }
    None
}

fn is_gif(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || matches!(image::guess_format(bytes), Ok(image::ImageFormat::Gif))
}

fn is_animated_webp(bytes: &[u8]) -> bool {
    image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(bytes))
        .map(|decoder| decoder.has_animation())
        .unwrap_or(false)
}

fn first_frame_png(bytes: &[u8]) -> Vec<u8> {
    encode_png(bytes).unwrap_or_else(placeholder_png)
}

fn encode_png(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::ImageEncoder as _;
    let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(out)
}

/// 解码失败也要给出一张合法 PNG:占位 1×1 透明图,不再触发重试风暴。
fn placeholder_png() -> Vec<u8> {
    use image::ImageEncoder as _;
    let rgba = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
    let mut out = Vec::new();
    let _ = image::codecs::png::PngEncoder::new(&mut out).write_image(
        rgba.as_raw(),
        1,
        1,
        image::ExtendedColorType::Rgba8,
    );
    out
}

#[cfg(test)]
mod images_tests {
    use super::*;

    /// 网络/内联/静态图原样透传:长文档不得反复重读大图。
    #[test]
    fn network_inline_and_static_images_pass_through_unchanged() {
        let source =
            "![a](https://example.com/a.gif) ![b](data:image/png;base64,AAAA) ![c](photo.png)";
        assert_eq!(
            rewrite_doc_images(source, Some(Path::new("D:/docs"))),
            source
        );
    }

    /// 本地 GIF 压成缓存 PNG(同一内容二次改写命中同一缓存文件)。
    #[test]
    fn local_gif_is_flattened_to_one_cached_png() {
        let directory = tempfile::tempdir().unwrap();
        let gif = directory.path().join("anim.gif");
        let frame = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode(frame.as_raw(), 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        std::fs::write(&gif, &bytes).unwrap();
        let rewritten = rewrite_doc_images("![anim](anim.gif)", Some(directory.path()));
        assert!(rewritten.starts_with("![anim]("), "{rewritten}");
        assert!(rewritten.ends_with(".png)"), "{rewritten}");
        let cached = rewritten
            .trim_start_matches("![anim](")
            .trim_end_matches(')')
            .replace('/', "\\");
        let path = std::path::PathBuf::from(cached);
        assert!(path.exists(), "缓存帧必须落盘: {path:?}");
        let cached_bytes = std::fs::read(&path).unwrap();
        assert!(cached_bytes.starts_with(b"\x89PNG"));
        let again = rewrite_doc_images("![anim](anim.gif)", Some(directory.path()));
        assert_eq!(rewritten, again, "同一内容复用同一缓存文件");
    }

    /// 引用式动图:`![alt][id]` 命中的定义压帧,纯链接引用不动(M8.3 问题 2)。
    #[test]
    fn referenced_gif_definitions_are_flattened_only_for_image_references() {
        let directory = tempfile::tempdir().unwrap();
        let gif = directory.path().join("anim.gif");
        let frame = image::RgbaImage::from_pixel(2, 2, image::Rgba([9, 8, 7, 255]));
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode(frame.as_raw(), 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        std::fs::write(&gif, &bytes).unwrap();
        let source = "![anim][figure]\n\n[figure]: anim.gif\n[link]: anim.gif\n";
        let rewritten = rewrite_doc_images(source, Some(directory.path()));
        assert!(
            rewritten.contains("[figure]: ") && rewritten.contains(".png"),
            "图片引用的定义必须压帧: {rewritten}"
        );
        assert!(
            rewritten.contains("[link]: anim.gif"),
            "纯链接引用不得被改写: {rewritten}"
        );
    }

    /// HTML `<img>` 双/单引号形态都重写;缺少文件时留在原样。
    #[test]
    fn html_image_sources_are_rewritten_only_when_the_file_exists() {
        let directory = tempfile::tempdir().unwrap();
        let png = directory.path().join("static.png");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]))
            .save(&png)
            .unwrap();
        let source = r#"<p><img src="static.png"></p>"#;
        assert_eq!(
            rewrite_doc_images(source, Some(directory.path())),
            source,
            "静态 PNG 不重写"
        );
        let missing = r#"<img src='missing.gif'>"#;
        assert_eq!(rewrite_doc_images(missing, Some(directory.path())), missing);
    }
}
