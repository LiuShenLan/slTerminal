//! Hooks 草稿的 JSON 子树解析：重复键、坏 JSON 与非法 UTF-8 一律拒绝。
//!
//! 解析只产出值树；基线指纹、层级与诊断状态由草稿宿主持有。未知但 schema
//! 合法的字段原样保留，不做修复式覆盖。

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::fmt;
use std::io;
use std::path::Path;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DraftSaveOutcome {
    /// 已原子写入；调用方用新基线替换草稿基线并清除草稿。
    Saved { baseline: [u8; 32] },
    /// 外部修改（含外部删除）：零写入，草稿与文件均保持原样供用户重载或显式覆盖。
    ExternalModified,
}

/// 保存前重读完整文件并比较基线指纹，一致才原子写入。
///
/// 权限与锁失败按 `Err` 传播，不折成 [`DraftSaveOutcome::ExternalModified`]；
/// 不做自动合并或覆盖，也不在外部删除后静默重建文件。
pub(crate) fn save_hooks_draft(
    path: &Path,
    baseline: Option<[u8; 32]>,
    contents: &[u8],
) -> io::Result<DraftSaveOutcome> {
    let current = read_fingerprint(path)?;
    if super::draft_write_guard(baseline, current) == super::DraftWriteGuard::ExternalModified {
        return Ok(DraftSaveOutcome::ExternalModified);
    }
    crate::atomic_file::write(path, contents)?;
    Ok(DraftSaveOutcome::Saved {
        baseline: Sha256::digest(contents).into(),
    })
}

fn read_fingerprint(path: &Path) -> io::Result<Option<[u8; 32]>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(Sha256::digest(&bytes).into())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
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
    use super::{
        DraftParseError, DraftSaveOutcome, parse_hooks_bytes, parse_hooks_json, save_hooks_draft,
    };
    use serde_json::json;
    use sha2::{Digest as _, Sha256};
    use std::fs;

    fn sha(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    /// 外部修改后保存必须拒绝且不覆盖文件；外部删除不得被当成空配置重建。
    #[test]
    fn hooks_draft_save_refuses_external_modification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, br#"{"hooks":{}}"#).unwrap();
        let baseline = Some(sha(br#"{"hooks":{}}"#));
        fs::write(&path, br#"{"hooks":{},"external":true}"#).unwrap();

        let outcome = save_hooks_draft(&path, baseline, br#"{"hooks":{"Stop":[]}}"#).unwrap();
        assert!(matches!(outcome, DraftSaveOutcome::ExternalModified));
        assert_eq!(
            fs::read(&path).unwrap(),
            br#"{"hooks":{},"external":true}"#,
            "外部修改必须原样保留"
        );

        fs::remove_file(&path).unwrap();
        let outcome = save_hooks_draft(&path, baseline, br#"{"hooks":{"Stop":[]}}"#).unwrap();
        assert!(matches!(outcome, DraftSaveOutcome::ExternalModified));
        assert!(!path.exists(), "被外部删除的文件不得被静默重建");
    }

    /// 基线一致才写入；成功后返回新基线，首次创建按需建立父目录。
    #[test]
    fn hooks_draft_save_writes_only_with_matching_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let outcome = save_hooks_draft(&path, None, br#"{"hooks":{"Stop":[]}}"#).unwrap();
        let DraftSaveOutcome::Saved { baseline } = outcome else {
            panic!("首次保存应成功: {outcome:?}");
        };
        assert_eq!(baseline, sha(br#"{"hooks":{"Stop":[]}}"#));

        let next = br#"{"hooks":{}}"#;
        let outcome = save_hooks_draft(&path, Some(baseline), next).unwrap();
        let DraftSaveOutcome::Saved { baseline } = outcome else {
            panic!("基线一致应写入: {outcome:?}");
        };
        assert_eq!(baseline, sha(next));
        assert_eq!(fs::read(&path).unwrap(), next);
    }

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
