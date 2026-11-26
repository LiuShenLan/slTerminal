//! SVG 安全 XML 投影；只允许无外部解析的静态矢量子集。

use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

const MAX_SVG_BYTES: usize = 20 * 1024 * 1024;
const MAX_SVG_NODES: usize = 4096;
const MAX_SVG_DEPTH: usize = 64;
const MAX_SVG_ATTRIBUTES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedSvg(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvgError {
    TooLarge,
    Unsafe,
    Malformed,
}

impl SanitizedSvg {
    /// 流式校验并重建 XML；任何不安全或超限事实都使整图失败。
    pub fn parse(source: &str) -> Result<Self, SvgError> {
        if source.len() > MAX_SVG_BYTES {
            return Err(SvgError::TooLarge);
        }
        let mut reader = Reader::from_str(source);
        reader.config_mut().check_comments = true;
        let mut output = Vec::with_capacity(source.len());
        let mut writer = Writer::new(&mut output);
        let mut depth = 0usize;
        let mut nodes = 0usize;
        let mut root_seen = false;
        loop {
            let event = reader.read_event().map_err(|_| SvgError::Malformed)?;
            match event {
                Event::Start(start) => {
                    let cleaned = clean_element(&start, reader.decoder(), depth == 0)?;
                    count_node(&mut nodes)?;
                    if depth >= MAX_SVG_DEPTH {
                        return Err(SvgError::TooLarge);
                    }
                    if depth == 0 {
                        if root_seen {
                            return Err(SvgError::Unsafe);
                        }
                        root_seen = true;
                    }
                    depth += 1;
                    writer
                        .write_event(Event::Start(cleaned))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::Empty(start) => {
                    let cleaned = clean_element(&start, reader.decoder(), depth == 0)?;
                    count_node(&mut nodes)?;
                    if depth >= MAX_SVG_DEPTH {
                        return Err(SvgError::TooLarge);
                    }
                    if depth == 0 {
                        if root_seen {
                            return Err(SvgError::Unsafe);
                        }
                        root_seen = true;
                    }
                    writer
                        .write_event(Event::Empty(cleaned))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::End(end) => {
                    depth = depth.checked_sub(1).ok_or(SvgError::Malformed)?;
                    writer
                        .write_event(Event::End(end.into_owned()))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::Text(text) => {
                    count_node(&mut nodes)?;
                    writer
                        .write_event(Event::Text(text.into_owned()))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::CData(text) => {
                    count_node(&mut nodes)?;
                    let text = text.decode().map_err(|_| SvgError::Malformed)?;
                    writer
                        .write_event(Event::Text(BytesText::new(&text)))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::GeneralRef(reference) => {
                    if depth == 0 {
                        return Err(SvgError::Unsafe);
                    }
                    let name = reference.decode().map_err(|_| SvgError::Malformed)?;
                    let numeric = reference
                        .resolve_char_ref()
                        .map_err(|_| SvgError::Malformed)?
                        .is_some();
                    if !numeric && !matches!(name.as_ref(), "lt" | "gt" | "amp" | "apos" | "quot") {
                        return Err(SvgError::Unsafe);
                    }
                    count_node(&mut nodes)?;
                    writer
                        .write_event(Event::GeneralRef(reference.into_owned()))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::Decl(declaration) => {
                    if root_seen || depth != 0 {
                        return Err(SvgError::Unsafe);
                    }
                    writer
                        .write_event(Event::Decl(declaration.into_owned()))
                        .map_err(|_| SvgError::Malformed)?;
                }
                Event::Comment(_) | Event::PI(_) => {}
                Event::DocType(_) => return Err(SvgError::Unsafe),
                Event::Eof => break,
            }
        }
        if !root_seen || depth != 0 || output.len() > MAX_SVG_BYTES {
            return Err(if output.len() > MAX_SVG_BYTES {
                SvgError::TooLarge
            } else {
                SvgError::Malformed
            });
        }
        let xml = String::from_utf8(output).map_err(|_| SvgError::Malformed)?;
        Ok(Self(xml))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn count_node(nodes: &mut usize) -> Result<(), SvgError> {
    *nodes = nodes.saturating_add(1);
    if *nodes > MAX_SVG_NODES {
        return Err(SvgError::TooLarge);
    }
    Ok(())
}

fn clean_element(
    start: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    root: bool,
) -> Result<BytesStart<'static>, SvgError> {
    let local = start.local_name();
    let local = std::str::from_utf8(local.as_ref()).map_err(|_| SvgError::Malformed)?;
    if !allowed_element(local) || (root && local != "svg") {
        return Err(SvgError::Unsafe);
    }
    let raw_name = start.name();
    let name = std::str::from_utf8(raw_name.as_ref()).map_err(|_| SvgError::Malformed)?;
    let attributes = start
        .attributes()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SvgError::Malformed)?;
    if attributes.len() > MAX_SVG_ATTRIBUTES {
        return Err(SvgError::TooLarge);
    }
    let mut cleaned = BytesStart::new(name.to_owned());
    for attribute in attributes {
        let key = attribute.key.local_name();
        let key = std::str::from_utf8(key.as_ref()).map_err(|_| SvgError::Malformed)?;
        let key_lower = key.to_ascii_lowercase();
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|_| SvgError::Malformed)?;
        if !safe_attribute(&key_lower, &value) {
            return Err(SvgError::Unsafe);
        }
        let raw_key =
            std::str::from_utf8(attribute.key.as_ref()).map_err(|_| SvgError::Malformed)?;
        cleaned.push_attribute((raw_key, value.as_ref()));
    }
    Ok(cleaned)
}

fn allowed_element(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "svg"
            | "g"
            | "defs"
            | "use"
            | "symbol"
            | "path"
            | "rect"
            | "circle"
            | "ellipse"
            | "line"
            | "polyline"
            | "polygon"
            | "lineargradient"
            | "radialgradient"
            | "stop"
            | "clippath"
            | "mask"
            | "marker"
            | "title"
            | "desc"
    )
}

fn safe_attribute(name: &str, value: &str) -> bool {
    if name.starts_with("on") || matches!(name, "style" | "filter" | "target") {
        return false;
    }
    if matches!(name, "href" | "src" | "srcset" | "poster" | "data") && !valid_fragment(value) {
        return false;
    }
    local_paint_urls(value)
}

fn valid_fragment(value: &str) -> bool {
    value.len() > 1
        && value.starts_with('#')
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}

fn local_paint_urls(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(found) = lower[offset..].find("url") {
        let start = offset + found + 3;
        let whitespace = lower[start..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(lower.len() - start);
        let open = start + whitespace;
        if !lower[open..].starts_with('(') {
            offset = start;
            continue;
        }
        let Some(close) = lower[open + 1..].find(')') else {
            return false;
        };
        let close = open + 1 + close;
        let reference = value[open + 1..close].trim();
        let reference = reference
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| {
                reference
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
            })
            .unwrap_or(reference);
        if !valid_fragment(reference) {
            return false;
        }
        offset = close + 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_static_svg_preserves_local_paint_and_predefined_entities() {
        let source = r##"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><defs><linearGradient id="paint"><stop offset="0" stop-color="#fff"/></linearGradient></defs><title>A &amp; B</title><rect width="2" height="2" fill="url(#paint)"/></svg>"##;
        let sanitized = SanitizedSvg::parse(source).unwrap();
        assert!(sanitized.as_str().contains("url(#paint)"));
        assert!(sanitized.as_str().contains("A &amp; B"));
    }

    #[test]
    fn svg_dtd_external_references_and_active_content_are_rejected() {
        let cases = [
            r##"<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///outside.txt">]><svg xmlns="http://www.w3.org/2000/svg"><title>&xxe;</title></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">html</div></foreignObject></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/external.png"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,AAAA"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><use href="https://example.com/external.svg#shape"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="alert(1)"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(https://example.com/paint.svg#p)"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import url(https://example.com/font.css);</style></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><animate attributeName="x" to="10"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><filter id="f"><feGaussianBlur stdDeviation="1000"/></filter></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><title>&custom;</title></svg>"##,
            r##"<html xmlns="http://www.w3.org/1999/xhtml"><script>alert(1)</script></html>"##,
            r##"<g xmlns="http://www.w3.org/2000/svg"/>"##,
        ];
        for source in cases {
            assert_eq!(SanitizedSvg::parse(source), Err(SvgError::Unsafe));
        }
    }

    #[test]
    fn attribute_values_are_escaped_so_reparsing_preserves_the_attribute_set() {
        // 单引号属性内的字面双引号在重建时必须转义；否则重建文本会多出
        // 一个白名单从未检查过的属性。
        let source =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect fill='#fff" onload="alert(1)'/></svg>"#;
        let sanitized = SanitizedSvg::parse(source).expect("单引号属性是合法 XML");
        let mut reader = Reader::from_str(sanitized.as_str());
        let mut rect_attributes: Option<Vec<(String, String)>> = None;
        loop {
            match reader.read_event().expect("重建输出必须是合法 XML") {
                Event::Start(start) | Event::Empty(start)
                    if start.local_name().as_ref() == b"rect" =>
                {
                    let attributes = start
                        .attributes()
                        .map(|attribute| {
                            let attribute = attribute.expect("属性必须可解析");
                            let key = String::from_utf8(attribute.key.as_ref().to_vec()).unwrap();
                            let value = attribute
                                .decoded_and_normalized_value(
                                    XmlVersion::Implicit1_0,
                                    reader.decoder(),
                                )
                                .expect("属性值必须可解码")
                                .into_owned();
                            (key, value)
                        })
                        .collect();
                    rect_attributes = Some(attributes);
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(
            rect_attributes,
            Some(vec![("fill".to_owned(), r#"#fff" onload="alert(1)"#.to_owned())]),
            "重建不得引入新属性，原值必须完整保留"
        );
        assert!(
            !sanitized.as_str().contains(r#"onload="alert(1)"#),
            "注入形态不得以属性语法出现在重建输出中"
        );
    }

    #[test]
    fn svg_budgets_and_malformed_documents_fail_closed() {
        let attributes = (0..65)
            .map(|index| format!("data-value-{index}=\"x\""))
            .collect::<Vec<_>>()
            .join(" ");
        let cases = [
            "<svg xmlns=\"http://www.w3.org/2000/svg\">".to_owned()
                + &"<g>".repeat(65)
                + &"</g>".repeat(65)
                + "</svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\">".to_owned()
                + &"<path/>".repeat(4097)
                + "</svg>",
            format!("<svg xmlns=\"http://www.w3.org/2000/svg\" {attributes}/>"),
            "x".repeat(20 * 1024 * 1024 + 1),
        ];
        for source in cases {
            assert_eq!(SanitizedSvg::parse(&source), Err(SvgError::TooLarge));
        }
        assert_eq!(
            SanitizedSvg::parse("<svg><g></svg>"),
            Err(SvgError::Malformed)
        );
        assert_eq!(SanitizedSvg::parse(""), Err(SvgError::Malformed));
    }
}
