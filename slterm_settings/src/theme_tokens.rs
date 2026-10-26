//! 完整暗色主题语义 token 与 ANSI 映射。
//!
//! 主题只承载颜色；JSON 对象必须恰好声明全部槽位，不能从缺省主题补色。

use std::fmt;

use crate::{Rgb8, Rgba8, json::Value};

/// 主题文档或 token 校验失败的原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeTokenError {
    InvalidStructure,
    MissingToken(&'static str),
    UnknownToken,
    DuplicateToken(&'static str),
    InvalidColor(&'static str),
    InvalidAnsi16,
}

impl fmt::Display for ThemeTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStructure => formatter.write_str("主题 token 必须是 JSON 对象"),
            Self::MissingToken(field) => write!(formatter, "主题缺少颜色 token：{field}"),
            Self::UnknownToken => formatter.write_str("主题包含未知字段"),
            Self::DuplicateToken(field) => write!(formatter, "主题颜色 token 重复：{field}"),
            Self::InvalidColor(field) => write!(formatter, "主题颜色 token 非法：{field}"),
            Self::InvalidAnsi16 => formatter.write_str("ANSI 映射必须恰好包含 16 个合法颜色"),
        }
    }
}

impl std::error::Error for ThemeTokenError {}

/// 全部 UI 与终端语义颜色；字段即封闭槽位目录。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeTokens {
    pub shell: Rgb8,
    pub background: Rgb8,
    pub foreground: Rgb8,
    pub muted: Rgb8,
    pub accent: Rgb8,
    pub selected: Rgba8,
    pub line: Rgba8,
    pub red: Rgb8,
    pub green: Rgb8,
    pub yellow: Rgb8,
    pub blue: Rgb8,
    pub purple: Rgb8,
    pub cyan: Rgb8,
    pub frame: Rgb8,
    pub terminal_background: Rgb8,
    pub terminal_foreground: Rgb8,
    pub terminal_cursor: Rgb8,
    pub terminal_cursor_text: Rgb8,
    pub terminal_cursor_stroke: Rgb8,
    pub terminal_selection_foreground: Rgb8,
    pub terminal_selection_background: Rgb8,
}

impl ThemeTokens {
    const FIELDS: [&'static str; 21] = [
        "shell",
        "background",
        "foreground",
        "muted",
        "accent",
        "selected",
        "line",
        "red",
        "green",
        "yellow",
        "blue",
        "purple",
        "cyan",
        "frame",
        "terminal_background",
        "terminal_foreground",
        "terminal_cursor",
        "terminal_cursor_text",
        "terminal_cursor_stroke",
        "terminal_selection_foreground",
        "terminal_selection_background",
    ];

    /// 从完整 JSON 对象读取所有 token；未知、重复与缺失字段均拒绝。
    pub fn from_value(value: &Value) -> Result<Self, ThemeTokenError> {
        let Value::Object(members) = value else {
            return Err(ThemeTokenError::InvalidStructure);
        };
        if members
            .iter()
            .any(|(name, _)| !Self::FIELDS.contains(&name.as_str()))
        {
            return Err(ThemeTokenError::UnknownToken);
        }
        for field in Self::FIELDS {
            let mut occurrences = members.iter().filter(|(name, _)| name == field);
            if occurrences.next().is_none() {
                return Err(ThemeTokenError::MissingToken(field));
            }
            if occurrences.next().is_some() {
                return Err(ThemeTokenError::DuplicateToken(field));
            }
        }
        Ok(Self {
            shell: parse_slot(value, "shell", parse_rgb)?,
            background: parse_slot(value, "background", parse_rgb)?,
            foreground: parse_slot(value, "foreground", parse_rgb)?,
            muted: parse_slot(value, "muted", parse_rgb)?,
            accent: parse_slot(value, "accent", parse_rgb)?,
            selected: parse_slot(value, "selected", parse_rgba)?,
            line: parse_slot(value, "line", parse_rgba)?,
            red: parse_slot(value, "red", parse_rgb)?,
            green: parse_slot(value, "green", parse_rgb)?,
            yellow: parse_slot(value, "yellow", parse_rgb)?,
            blue: parse_slot(value, "blue", parse_rgb)?,
            purple: parse_slot(value, "purple", parse_rgb)?,
            cyan: parse_slot(value, "cyan", parse_rgb)?,
            frame: parse_slot(value, "frame", parse_rgb)?,
            terminal_background: parse_slot(value, "terminal_background", parse_rgb)?,
            terminal_foreground: parse_slot(value, "terminal_foreground", parse_rgb)?,
            terminal_cursor: parse_slot(value, "terminal_cursor", parse_rgb)?,
            terminal_cursor_text: parse_slot(value, "terminal_cursor_text", parse_rgb)?,
            terminal_cursor_stroke: parse_slot(value, "terminal_cursor_stroke", parse_rgb)?,
            terminal_selection_foreground: parse_slot(
                value,
                "terminal_selection_foreground",
                parse_rgb,
            )?,
            terminal_selection_background: parse_slot(
                value,
                "terminal_selection_background",
                parse_rgb,
            )?,
        })
    }

    /// 生成规范小写颜色文本的完整 JSON 对象。
    pub fn to_value(self) -> Value {
        Value::Object(vec![
            rgb_member("shell", self.shell),
            rgb_member("background", self.background),
            rgb_member("foreground", self.foreground),
            rgb_member("muted", self.muted),
            rgb_member("accent", self.accent),
            rgba_member("selected", self.selected),
            rgba_member("line", self.line),
            rgb_member("red", self.red),
            rgb_member("green", self.green),
            rgb_member("yellow", self.yellow),
            rgb_member("blue", self.blue),
            rgb_member("purple", self.purple),
            rgb_member("cyan", self.cyan),
            rgb_member("frame", self.frame),
            rgb_member("terminal_background", self.terminal_background),
            rgb_member("terminal_foreground", self.terminal_foreground),
            rgb_member("terminal_cursor", self.terminal_cursor),
            rgb_member("terminal_cursor_text", self.terminal_cursor_text),
            rgb_member("terminal_cursor_stroke", self.terminal_cursor_stroke),
            rgb_member(
                "terminal_selection_foreground",
                self.terminal_selection_foreground,
            ),
            rgb_member(
                "terminal_selection_background",
                self.terminal_selection_background,
            ),
        ])
    }
}

fn parse_slot<T>(
    value: &Value,
    field: &'static str,
    parser: fn(&str) -> Option<T>,
) -> Result<T, ThemeTokenError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .and_then(parser)
        .ok_or(ThemeTokenError::InvalidColor(field))
}

fn rgb_member(field: &'static str, color: Rgb8) -> (String, Value) {
    (
        field.to_owned(),
        Value::String(crate::format_hex_rgb(color)),
    )
}

fn rgba_member(field: &'static str, color: Rgba8) -> (String, Value) {
    (
        field.to_owned(),
        Value::String(format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color[0], color[1], color[2], color[3]
        )),
    )
}

fn parse_rgb(text: &str) -> Option<Rgb8> {
    let channels = decode_hex(text, 3)?;
    Some([channels[0], channels[1], channels[2]])
}

fn parse_rgba(text: &str) -> Option<Rgba8> {
    if text.strip_prefix('#').is_some_and(|hex| hex.len() == 6) {
        let rgb = parse_rgb(text)?;
        return Some([rgb[0], rgb[1], rgb[2], 255]);
    }
    decode_hex(text, 4)
}

/// 解码 `#RGB[A]` 或对应的六位／八位形态；空白与外部引用不接受。
fn decode_hex(text: &str, channels: usize) -> Option<[u8; 4]> {
    let hex = text.strip_prefix('#')?;
    if !matches!(hex.len(), length if length == channels || length == channels * 2)
        || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    let short = hex.len() == channels;
    let mut output = [0; 4];
    for (index, slot) in output.iter_mut().enumerate().take(channels) {
        let (start, width) = if short { (index, 1) } else { (index * 2, 2) };
        *slot = u8::from_str_radix(&hex[start..start + width], 16).ok()?;
        if short {
            *slot *= 17;
        }
    }
    Some(output)
}

/// ANSI 0–15 的完整主题映射；其余索引色按 xterm 公式派生。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ansi16(pub [Rgb8; 16]);

impl Ansi16 {
    /// 从恰好十六项的颜色数组读取映射。
    pub fn from_value(value: &Value) -> Result<Self, ThemeTokenError> {
        let Value::Array(values) = value else {
            return Err(ThemeTokenError::InvalidAnsi16);
        };
        if values.len() != 16 {
            return Err(ThemeTokenError::InvalidAnsi16);
        }
        let mut colors = [[0; 3]; 16];
        for (slot, value) in colors.iter_mut().zip(values) {
            *slot = value
                .as_str()
                .and_then(parse_rgb)
                .ok_or(ThemeTokenError::InvalidAnsi16)?;
        }
        Ok(Self(colors))
    }

    /// 生成十六项规范颜色文本数组。
    pub fn to_value(self) -> Value {
        Value::Array(
            self.0
                .into_iter()
                .map(|color| Value::String(crate::format_hex_rgb(color)))
                .collect(),
        )
    }

    /// 解析完整 xterm 256 色索引；16 色以外不保存第二份色表。
    pub fn color(self, index: u8) -> Rgb8 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        match index {
            0..=15 => self.0[usize::from(index)],
            16..=231 => {
                let cube = usize::from(index - 16);
                [LEVELS[cube / 36], LEVELS[(cube / 6) % 6], LEVELS[cube % 6]]
            }
            232..=255 => {
                let level = 8 + (index - 232) * 10;
                [level; 3]
            }
        }
    }
}

#[cfg(test)]
mod theme_tokens_tests {
    use super::*;
    use crate::json;

    const SAMPLE: &str = r##"{
        "shell": "#101012",
        "background": "#0a0a0b",
        "foreground": "#b3aea6",
        "muted": "#8a857d",
        "accent": "#6e9ff2",
        "selected": "#6e9ff221",
        "line": "#ffffff0e",
        "red": "#d9706b",
        "green": "#86bb7a",
        "yellow": "#d6b25e",
        "blue": "#6e9ff2",
        "purple": "#b48ce0",
        "cyan": "#6fbfc4",
        "frame": "#141416",
        "terminal_background": "#0a0a0b",
        "terminal_foreground": "#cfcac1",
        "terminal_cursor": "#6e9ff2",
        "terminal_cursor_text": "#0a0a0b",
        "terminal_cursor_stroke": "#6e9ff2",
        "terminal_selection_foreground": "#f0ede8",
        "terminal_selection_background": "#26344c"
    }"##;

    const FIELDS: [&str; 21] = [
        "shell",
        "background",
        "foreground",
        "muted",
        "accent",
        "selected",
        "line",
        "red",
        "green",
        "yellow",
        "blue",
        "purple",
        "cyan",
        "frame",
        "terminal_background",
        "terminal_foreground",
        "terminal_cursor",
        "terminal_cursor_text",
        "terminal_cursor_stroke",
        "terminal_selection_foreground",
        "terminal_selection_background",
    ];

    fn sample_value() -> Value {
        json::parse(SAMPLE.as_bytes()).unwrap()
    }

    fn ansi_value() -> Value {
        Value::Array(
            (0..16)
                .map(|index| Value::String(format!("#{index:02x}{index:02x}{index:02x}")))
                .collect(),
        )
    }

    #[test]
    fn theme_tokens_round_trip_every_canonical_slot() {
        let value = sample_value();
        let parsed = ThemeTokens::from_value(&value);
        assert!(parsed.is_ok(), "{parsed:?}");
        assert_eq!(parsed.unwrap().to_value(), value);
    }

    #[test]
    fn theme_tokens_reject_missing_unknown_and_duplicate_fields() {
        assert_eq!(
            ThemeTokens::from_value(&Value::Null),
            Err(ThemeTokenError::InvalidStructure)
        );
        for field in FIELDS {
            let mut value = sample_value();
            value.remove(field);
            assert_eq!(
                ThemeTokens::from_value(&value),
                Err(ThemeTokenError::MissingToken(field)),
                "{field}"
            );

            let mut value = sample_value();
            value
                .object_mut()
                .unwrap()
                .push((field.to_owned(), Value::String("#000000".to_owned())));
            assert_eq!(
                ThemeTokens::from_value(&value),
                Err(ThemeTokenError::DuplicateToken(field)),
                "{field}"
            );
        }
        for (field, value) in [
            ("vendor", Value::String("#000000".to_owned())),
            (
                "resource",
                Value::String("https://example.invalid/a.png".to_owned()),
            ),
        ] {
            let mut tokens = sample_value();
            tokens.set(field, value);
            assert_eq!(
                ThemeTokens::from_value(&tokens),
                Err(ThemeTokenError::UnknownToken)
            );
        }
    }

    #[test]
    fn theme_tokens_reject_invalid_colors_without_partial_fallback() {
        for field in FIELDS {
            for invalid in [
                Value::Null,
                Value::String("#12".to_owned()),
                Value::String("url(https://example.invalid/a.png)".to_owned()),
                Value::String(" #010203".to_owned()),
                Value::String("010203".to_owned()),
            ] {
                let mut value = sample_value();
                value.set(field, invalid.clone());
                assert_eq!(
                    ThemeTokens::from_value(&value),
                    Err(ThemeTokenError::InvalidColor(field)),
                    "{field}: {invalid:?}"
                );
            }
        }

        let mut value = sample_value();
        value.set(
            "red",
            Value::String("https://example.invalid/secret".to_owned()),
        );
        let error = ThemeTokens::from_value(&value).unwrap_err();
        assert!(!error.to_string().contains("example.invalid"));
    }

    #[test]
    fn theme_tokens_normalize_supported_hex_forms() {
        let mut value = sample_value();
        value.set("red", Value::String("#ABC".to_owned()));
        value.set("selected", Value::String("#ABCD".to_owned()));
        value.set("line", Value::String("#AABBCC".to_owned()));
        let tokens = ThemeTokens::from_value(&value).unwrap();
        assert_eq!(tokens.red, [0xaa, 0xbb, 0xcc]);
        assert_eq!(tokens.selected, [0xaa, 0xbb, 0xcc, 0xdd]);
        assert_eq!(tokens.line, [0xaa, 0xbb, 0xcc, 0xff]);
        let normalized = tokens.to_value();
        assert_eq!(normalized.get("red").unwrap().as_str(), Some("#aabbcc"));
        assert_eq!(
            normalized.get("selected").unwrap().as_str(),
            Some("#aabbccdd")
        );
        assert_eq!(normalized.get("line").unwrap().as_str(), Some("#aabbccff"));
    }

    #[test]
    fn theme_tokens_ansi_requires_exactly_sixteen_valid_colors() {
        assert!(Ansi16::from_value(&ansi_value()).is_ok());
        for length in [0, 15, 17, 256] {
            let value = Value::Array(vec![Value::String("#010203".to_owned()); length]);
            assert_eq!(
                Ansi16::from_value(&value),
                Err(ThemeTokenError::InvalidAnsi16),
                "{length}"
            );
        }
        for value in [
            Value::Null,
            Value::empty_object(),
            Value::Array(vec![Value::String("#010203".to_owned()); 16]),
        ] {
            let mut invalid = value;
            if let Value::Array(items) = &mut invalid {
                items[7] = Value::String("file:///tmp/color".to_owned());
            }
            assert_eq!(
                Ansi16::from_value(&invalid),
                Err(ThemeTokenError::InvalidAnsi16)
            );
        }
    }

    #[test]
    fn theme_tokens_ansi_derives_standard_cube_and_grayscale() {
        let ansi = Ansi16::from_value(&ansi_value()).unwrap();
        assert_eq!(ansi.color(0), [0, 0, 0]);
        assert_eq!(ansi.color(15), [15, 15, 15]);
        for (index, expected) in [
            (16, [0, 0, 0]),
            (17, [0, 0, 95]),
            (18, [0, 0, 135]),
            (19, [0, 0, 175]),
            (20, [0, 0, 215]),
            (21, [0, 0, 255]),
            (22, [0, 95, 0]),
            (23, [0, 95, 95]),
            (231, [255, 255, 255]),
            (232, [8, 8, 8]),
            (255, [238, 238, 238]),
        ] {
            assert_eq!(ansi.color(index), expected, "{index}");
        }
        let encoded = ansi.to_value();
        assert_eq!(encoded, ansi_value());
    }
}
