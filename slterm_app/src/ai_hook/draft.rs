//! Hooks 草稿的 JSON 子树解析：重复键、坏 JSON 与非法 UTF-8 一律拒绝。
//!
//! 解析只产出值树；基线指纹、层级与诊断状态由草稿宿主持有。未知但 schema
//! 合法的字段原样保留，不做修复式覆盖。

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DraftParseError {
    /// 文件不是合法 UTF-8（宿主在读取字节时判定）。
    InvalidUtf8,
    /// JSON 语法错误或根不是对象。
    InvalidJson(String),
    /// 同一对象内重复键；键序不得被静默合并。
    DuplicateKey(String),
}

/// 重复键经 serde_json 错误消息透传的哨兵；真实解析错误不含该序列。
const DUPLICATE_SENTINEL: &str = "\u{1}slterm-duplicate-key\u{1}";

/// 严格解析：整棵文档递归检测重复键，根必须是对象。
pub(crate) fn parse_hooks_json(text: &str) -> Result<Value, DraftParseError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = match serde::Deserializer::deserialize_any(&mut deserializer, StrictVisitor) {
        Ok(value) => value,
        Err(error) => return Err(classify(error)),
    };
    deserializer
        .end()
        .map_err(|error| DraftParseError::InvalidJson(error.to_string()))?;
    if !matches!(value, Value::Object(_)) {
        return Err(DraftParseError::InvalidJson("根必须是对象".to_owned()));
    }
    Ok(value)
}

/// 从文件字节解析：非法 UTF-8 与坏 JSON 分别拒绝，不当空配置。
pub(crate) fn parse_hooks_bytes(bytes: &[u8]) -> Result<Value, DraftParseError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DraftParseError::InvalidUtf8)?;
    parse_hooks_json(text)
}

fn classify(error: serde_json::Error) -> DraftParseError {
    let message = error.to_string();
    if let Some(index) = message.find(DUPLICATE_SENTINEL) {
        let rest = &message[index + DUPLICATE_SENTINEL.len()..];
        let key = rest.split('\u{1}').next().unwrap_or_default().to_owned();
        return DraftParseError::DuplicateKey(key);
    }
    DraftParseError::InvalidJson(message)
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("任意 JSON 值")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("非有限浮点数"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(value) = seq.next_element::<StrictWrapped>()? {
            items.push(value.0);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom(format!("{DUPLICATE_SENTINEL}{key}\u{1}")));
            }
            let value = map.next_value::<StrictWrapped>()?;
            object.insert(key, value.0);
        }
        Ok(Value::Object(object))
    }
}

/// 让嵌套值继续走严格 visitor，从而整棵文档递归检测重复键。
struct StrictWrapped(Value);

impl<'de> serde::Deserialize<'de> for StrictWrapped {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer
            .deserialize_any(StrictVisitor)
            .map(StrictWrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::{DraftParseError, parse_hooks_bytes, parse_hooks_json};
    use serde_json::json;

    /// 重复键在任意层级都拒绝，不静默合并为最后值。
    #[test]
    fn hooks_draft_detects_duplicate_keys() {
        let error = parse_hooks_json(r#"{"hooks":{"Stop":[{"a":1,"a":2}]}}"#).unwrap_err();
        assert!(
            matches!(error, DraftParseError::DuplicateKey(ref key) if key == "a"),
            "嵌套重复键必须拒绝: {error:?}"
        );
        assert!(matches!(
            parse_hooks_json(r#"{"x":1,"x":1}"#),
            Err(DraftParseError::DuplicateKey(_))
        ));
    }

    /// 未知但合法的字段原样保留；坏 JSON、根非对象与非法 UTF-8 分别拒绝。
    #[test]
    fn hooks_draft_preserves_unknown_fields_and_rejects_invalid() {
        let value = parse_hooks_json(r#"{"hooks":{},"future":{"flag":true}}"#).unwrap();
        assert_eq!(value["future"]["flag"], json!(true));

        assert!(matches!(
            parse_hooks_json("{not json"),
            Err(DraftParseError::InvalidJson(_))
        ));
        assert!(matches!(
            parse_hooks_json("[1,2]"),
            Err(DraftParseError::InvalidJson(_))
        ));
        assert_eq!(
            parse_hooks_bytes(&[0xff, 0xfe]),
            Err(DraftParseError::InvalidUtf8)
        );
        assert!(parse_hooks_json(r#"{"hooks":{"Stop":[]}}"#).is_ok());
    }
}
