//! 零依赖 JSON 值层——settings.json 持久化契约的自包含载体。
//!
//! 本 crate 负零生产依赖契约(architecture 门禁机械强制),serde/serde_json
//! 不可引入;JSON 值树、解析器与序列化器在此自实现。对象成员用保序 Vec
//! 存储:写通道浅合并时未知键/用户手改段在文件原位保留,与人手编辑友好。
//!
//! 值域只覆盖设置/主题文档所需:Null/Bool/Number(f64)/String/Array/Object。
//! 解析是严格 JSON(宽容在 `RawSettings::from_json_bytes` 一层:解析失败
//! 收敛为空树);序列化固定两空格缩进 pretty 形态。

use std::fmt;

/// 解析深度上限:设置文件是浅层文档,超限拒绝(防深嵌套栈溢出)。
pub const MAX_DEPTH: usize = 128;

/// JSON 值树。对象成员保序——读-改-写的「原位保留」语义由此承载。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Default for Value {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl Value {
    /// 空对象(设置文档的根形态)。
    pub fn empty_object() -> Self {
        Self::Object(Vec::new())
    }

    /// 对象成员查找;非对象或键缺失返回 None。
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// 字符串值;非字符串返回 None。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// 数值;非数字返回 None。
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    /// 真布尔直取;非布尔(含字符串 "true")返回 None——类型不符不炸、
    /// 回默认,JSON 世界不兼容拼写宽容。
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// 对象键遍历;非对象返回空迭代。
    pub fn object_keys(&self) -> impl Iterator<Item = &str> {
        let members: &[(String, Value)] = match self {
            Self::Object(members) => members,
            _ => &[],
        };
        members.iter().map(|(name, _)| name.as_str())
    }

    /// 对象原位替换或尾部追加;非对象调用方不得触碰( debug 构建断言)。
    pub fn set(&mut self, key: &str, value: Value) {
        let Self::Object(members) = self else {
            debug_assert!(false, "set 只作用于对象值");
            return;
        };
        match members.iter_mut().find(|(name, _)| name == key) {
            Some((_, slot)) => *slot = value,
            None => members.push((key.to_owned(), value)),
        }
    }

    /// 删除对象成员;不存在则无事发生。
    pub fn remove(&mut self, key: &str) {
        if let Self::Object(members) = self {
            members.retain(|(name, _)| name != key);
        }
    }

    /// 取可变对象成员;非对象返回 None。
    pub fn object_mut(&mut self) -> Option<&mut Vec<(String, Value)>> {
        match self {
            Self::Object(members) => Some(members),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(flag: bool) -> Self {
        Self::Bool(flag)
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Self::String(text.to_owned())
    }
}

impl From<String> for Value {
    fn from(text: String) -> Self {
        Self::String(text)
    }
}

impl From<f64> for Value {
    fn from(number: f64) -> Self {
        Self::Number(number)
    }
}

impl From<usize> for Value {
    fn from(number: usize) -> Self {
        Self::Number(number as f64)
    }
}

/// 解析失败定位:字节偏移 + 静态说明。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

/// 严格 JSON 解析;输入须为 UTF-8。宽容收敛由调用方选择(见模块头)。
pub fn parse(bytes: &[u8]) -> Result<Value, ParseError> {
    let text = std::str::from_utf8(bytes).map_err(|error| ParseError {
        offset: error.valid_up_to(),
        message: "not UTF-8",
    })?;
    let mut parser = Parser {
        text,
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("trailing content"));
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &'static str) -> ParseError {
        ParseError {
            offset: self.pos,
            message,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), ParseError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error("unexpected character"))
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth >= MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        match self.peek() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(self.error("unexpected character")),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: Value) -> Result<Value, ParseError> {
        if self.bytes[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.error("invalid literal"))
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.expect(b'{')?;
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_whitespace();
            let name = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            members.push((name, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("unterminated string"));
            };
            self.pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => out.push(self.parse_escape()?),
                0x00..=0x1f => return Err(self.error("control character in string")),
                _ => {
                    // UTF-8 已整体验证;按字符边界截取多字节序列。
                    let start = self.pos - 1;
                    let width = utf8_width(byte);
                    self.pos += width - 1;
                    out.push_str(&self.text[start..self.pos]);
                }
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, ParseError> {
        let Some(byte) = self.peek() else {
            return Err(self.error("unterminated escape"));
        };
        self.pos += 1;
        let escaped = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.parse_unicode_escape(),
            _ => return Err(self.error("invalid escape")),
        };
        Ok(escaped)
    }

    fn parse_unicode_escape(&mut self) -> Result<char, ParseError> {
        let first = self.parse_hex4()?;
        let code = if (0xd800..0xdc00).contains(&first) {
            // 高代理必须紧跟 \uXXXX 低代理,否则不是合法标量。
            if self.peek() == Some(b'\\') {
                self.pos += 1;
                self.expect(b'u')?;
                let second = self.parse_hex4()?;
                if !(0xdc00..0xe000).contains(&second) {
                    return Err(self.error("invalid surrogate pair"));
                }
                0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
            } else {
                return Err(self.error("lone surrogate"));
            }
        } else if (0xdc00..0xe000).contains(&first) {
            return Err(self.error("lone surrogate"));
        } else {
            first
        };
        char::from_u32(code).ok_or_else(|| self.error("invalid unicode scalar"))
    }

    fn parse_hex4(&mut self) -> Result<u32, ParseError> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.error("truncated unicode escape"));
        }
        let digits = &self.text[self.pos..self.pos + 4];
        let code =
            u32::from_str_radix(digits, 16).map_err(|_| self.error("invalid unicode escape"))?;
        self.pos += 4;
        Ok(code)
    }

    fn parse_number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        // 严格 JSON:整数部分不允前导零;小数/指数段必须有数字。
        if self.peek() == Some(b'0') && matches!(self.bytes.get(self.pos + 1), Some(b'0'..=b'9')) {
            return Err(self.error("leading zero in number"));
        }
        self.consume_digits();
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("missing fraction digits"));
            }
            self.consume_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("missing exponent digits"));
            }
            self.consume_digits();
        }
        let text = &self.text[start..self.pos];
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.error("invalid number"))
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }
}

/// UTF-8 首字节长度;输入已经 `str::from_utf8` 验证,不会越界。
fn utf8_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// 两空格缩进的 pretty 序列化——settings.json 落盘形态。
pub fn to_string_pretty(value: &Value) -> String {
    let mut out = String::with_capacity(256);
    write_value(&mut out, value, 0);
    out.push('\n');
    out
}

fn write_value(out: &mut String, value: &Value, indent: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        // 非有限值不是合法 JSON;防御性写 null(设置值域经 from_raw 钳制,
        // 正常路径不会到达)。
        Value::Number(number) if !number.is_finite() => out.push_str("null"),
        Value::Number(number) => out.push_str(&format!("{number}")),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
            } else {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent + 1));
                    write_value(out, item, indent + 1);
                }
                out.push('\n');
                out.push_str(&"  ".repeat(indent));
                out.push(']');
            }
        }
        Value::Object(members) => {
            if members.is_empty() {
                out.push_str("{}");
            } else {
                out.push('{');
                for (index, (name, member)) in members.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent + 1));
                    write_string(out, name);
                    out.push_str(": ");
                    write_value(out, member, indent + 1);
                }
                out.push('\n');
                out.push_str(&"  ".repeat(indent));
                out.push('}');
            }
        }
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod json_tests {
    use super::*;

    fn parse_text(text: &str) -> Value {
        parse(text.as_bytes()).expect("fixture parses")
    }

    #[test]
    fn parse_and_pretty_print_round_trip_preserves_member_order() {
        let text = "{\n  \"appearance\": {\n    \"font_size\": 14,\n    \"theme\": \"Linear\"\n  },\n  \"scrolling\": {\n    \"scrollback_lines\": 10000\n  }\n}\n";
        let value = parse_text(text);
        assert_eq!(to_string_pretty(&value), text);
        let appearance = value.get("appearance").unwrap();
        assert_eq!(appearance.get("font_size"), Some(&Value::Number(14.0)));
        assert_eq!(
            appearance.get("theme").and_then(Value::as_str),
            Some("Linear")
        );
    }

    #[test]
    fn parse_handles_escapes_unicode_and_surrogate_pairs() {
        let value = parse_text(r#""a\"b\\c\/d\b\f\n\r\tA中文😀""#);
        assert_eq!(
            value.as_str(),
            Some("a\"b\\c/d\u{8}\u{c}\n\r\tA中文\u{1f600}")
        );
        // 序列化再解析,往返相等。
        let reparsed = parse(to_string_pretty(&value).as_bytes()).unwrap();
        assert_eq!(reparsed, value);
    }

    #[test]
    fn parse_rejects_lone_surrogates_and_bad_escapes() {
        for text in [
            r#""\ud800""#,
            r#""\ud800x""#,
            r#""\udc00""#,
            r#""\q""#,
            "\"abc",
        ] {
            assert!(parse(text.as_bytes()).is_err(), "{text}");
        }
    }

    #[test]
    fn parse_rejects_malformed_documents_without_panicking() {
        for bytes in [
            &b""[..],
            b"{",
            b"[1,",
            b"{\"a\":}",
            b"{\"a\":1,}",
            b"[1,]",
            b"nul",
            b"truE",
            b"01",
            b"--1",
            b"{\"a\":1} extra",
            &[0xff, 0xfe][..],
            b"\"bad\x01char\"",
            b"1.",
            b"1e",
            b"-",
        ] {
            assert!(parse(bytes).is_err(), "{bytes:?}");
        }
    }

    #[test]
    fn nesting_depth_is_bounded() {
        let deep = format!("{}{}{}", "[".repeat(MAX_DEPTH), "1", "]".repeat(MAX_DEPTH));
        assert!(parse(deep.as_bytes()).is_err());
        let allowed = format!(
            "{}{}{}",
            "[".repeat(MAX_DEPTH - 1),
            "1",
            "]".repeat(MAX_DEPTH - 1)
        );
        assert!(parse(allowed.as_bytes()).is_ok());
    }

    #[test]
    fn numbers_serialize_without_trailing_fraction_and_nonfinite_as_null() {
        assert_eq!(to_string_pretty(&Value::Number(14.0)), "14\n");
        assert_eq!(to_string_pretty(&Value::Number(0.65)), "0.65\n");
        assert_eq!(to_string_pretty(&Value::Number(-0.25)), "-0.25\n");
        assert_eq!(to_string_pretty(&Value::Number(f64::NAN)), "null\n");
        assert_eq!(to_string_pretty(&Value::Number(f64::INFINITY)), "null\n");
    }

    #[test]
    fn empty_containers_stay_on_one_line() {
        let mut root = Value::empty_object();
        root.set("empty_object", Value::empty_object());
        root.set("empty_array", Value::Array(Vec::new()));
        assert_eq!(
            to_string_pretty(&root),
            "{\n  \"empty_object\": {},\n  \"empty_array\": []\n}\n"
        );
    }

    #[test]
    fn set_replaces_in_place_and_appends_missing() {
        let mut root = parse_text("{\"b\": 1, \"a\": {\"x\": true}}");
        root.set("b", Value::Bool(false));
        root.set("c", Value::Null);
        root.object_mut().unwrap()[1].1.set("y", Value::Number(2.0));
        assert_eq!(
            to_string_pretty(&root),
            "{\n  \"b\": false,\n  \"a\": {\n    \"x\": true,\n    \"y\": 2\n  },\n  \"c\": null\n}\n"
        );
        root.remove("b");
        root.remove("missing");
        assert_eq!(root.object_keys().collect::<Vec<_>>(), ["a", "c"]);
    }

    #[test]
    fn typed_accessors_reject_wrong_kinds() {
        let value = parse_text("{\"s\": \"x\", \"n\": 1.5, \"b\": true, \"z\": null}");
        assert_eq!(value.get("s").and_then(Value::as_f64), None);
        assert_eq!(value.get("n").and_then(Value::as_bool), None);
        // 字符串 "true" 不是布尔——类型不符回默认,无拼写宽容。
        let legacy = parse_text("{\"flag\": \"true\"}");
        assert_eq!(legacy.get("flag").and_then(Value::as_bool), None);
        assert_eq!(value.get("b").and_then(Value::as_bool), Some(true));
        assert_eq!(value.get("missing"), None);
        assert_eq!(Value::Null.get("anything"), None);
    }
}
