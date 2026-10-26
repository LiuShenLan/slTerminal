use std::fmt;
use std::str::FromStr;

use uuid::{Uuid, Variant, Version};

/// Document 的稳定身份；UUID 值由 application 提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(Uuid);

/// 可由撤销／重做返回并用于保存对账的文本状态身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextStateId(Uuid);

/// 稳定身份的构造或解析错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StableIdError {
    /// 文本不是规范小写连字符 UUID。
    InvalidFormat,
    /// UUID 不是 RFC 4122 变体的 v7。
    ExpectedV7,
}

fn validate_uuid_v7(value: Uuid) -> Result<Uuid, StableIdError> {
    if value.get_version() != Some(Version::SortRand) || value.get_variant() != Variant::RFC4122 {
        return Err(StableIdError::ExpectedV7);
    }
    Ok(value)
}

fn parse_uuid_v7(text: &str) -> Result<Uuid, StableIdError> {
    let value = Uuid::parse_str(text).map_err(|_| StableIdError::InvalidFormat)?;
    if text != value.hyphenated().to_string() {
        return Err(StableIdError::InvalidFormat);
    }
    validate_uuid_v7(value)
}

impl DocumentId {
    /// 使用 application 提供的 UUID 构造稳定身份。
    pub fn from_uuid(value: Uuid) -> Result<Self, StableIdError> {
        validate_uuid_v7(value).map(Self)
    }

    /// 返回底层 UUID 值。
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for DocumentId {
    type Err = StableIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_uuid_v7(text).map(Self)
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl TextStateId {
    /// 使用 application 提供的 UUID 构造文本状态身份。
    pub fn from_uuid(value: Uuid) -> Result<Self, StableIdError> {
        validate_uuid_v7(value).map(Self)
    }

    /// 返回底层 UUID 值。
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for TextStateId {
    type Err = StableIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_uuid_v7(text).map(Self)
    }
}

impl fmt::Display for TextStateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl fmt::Display for StableIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFormat => "稳定 ID 必须为规范小写 UUID 字符串",
            Self::ExpectedV7 => "稳定 ID 必须为 UUIDv7",
        })
    }
}

impl std::error::Error for StableIdError {}

#[cfg(test)]
mod identity_tests {
    use super::*;

    const V7: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const V4: &str = "550e8400-e29b-41d4-a716-446655440000";

    /// 持久文本使用规范小写连字符 UUID。
    #[test]
    fn document_id_round_trips_canonical_uuid_text() {
        let id = DocumentId::from_str(V7).unwrap();
        assert_eq!(id.to_string(), V7);
        assert_eq!(DocumentId::from_str(&id.to_string()).unwrap(), id);
    }

    /// 非规范文本与非 v7 值不得进入持久身份。
    #[test]
    fn document_id_rejects_noncanonical_text_and_other_versions() {
        assert_eq!(DocumentId::from_str(V4), Err(StableIdError::ExpectedV7));
        for text in [V7.to_uppercase(), V7.replace('-', ""), "not-a-uuid".into()] {
            assert_eq!(
                DocumentId::from_str(&text),
                Err(StableIdError::InvalidFormat),
                "{text}"
            );
        }
    }

    /// 应用注入的值同样校验版本和变体。
    #[test]
    fn document_id_validates_application_supplied_uuid() {
        let value = Uuid::parse_str(V7).unwrap();
        let id = DocumentId::from_uuid(value).unwrap();
        assert_eq!(id.as_uuid(), &value);
        assert_eq!(
            DocumentId::from_uuid(Uuid::parse_str(V4).unwrap()),
            Err(StableIdError::ExpectedV7)
        );

        let mut wrong_variant = *value.as_bytes();
        wrong_variant[8] = 0;
        assert_eq!(
            DocumentId::from_uuid(Uuid::from_bytes(wrong_variant)),
            Err(StableIdError::ExpectedV7)
        );
    }

    /// 文本状态身份在快照与 DTO 中使用规范 UUID 文本。
    #[test]
    fn text_state_id_round_trips_canonical_uuid_text() {
        let id = TextStateId::from_str(V7).unwrap();
        assert_eq!(id.to_string(), V7);
        assert_eq!(TextStateId::from_str(&id.to_string()).unwrap(), id);
    }

    /// 损坏快照中的非规范文本与其他 UUID 版本不得成为状态身份。
    #[test]
    fn text_state_id_rejects_noncanonical_text_and_other_versions() {
        assert_eq!(TextStateId::from_str(V4), Err(StableIdError::ExpectedV7));
        for text in [V7.to_uppercase(), V7.replace('-', ""), "not-a-uuid".into()] {
            assert_eq!(
                TextStateId::from_str(&text),
                Err(StableIdError::InvalidFormat),
                "{text}"
            );
        }
    }

    /// 新编辑状态的随机源由应用注入，但版本与变体仍在核心校验。
    #[test]
    fn text_state_id_validates_application_supplied_uuid() {
        let value = Uuid::parse_str(V7).unwrap();
        let id = TextStateId::from_uuid(value).unwrap();
        assert_eq!(id.as_uuid(), &value);
        assert_eq!(
            TextStateId::from_uuid(Uuid::parse_str(V4).unwrap()),
            Err(StableIdError::ExpectedV7)
        );

        let mut wrong_variant = *value.as_bytes();
        wrong_variant[8] = 0;
        assert_eq!(
            TextStateId::from_uuid(Uuid::from_bytes(wrong_variant)),
            Err(StableIdError::ExpectedV7)
        );
    }

    /// 解析错误提供明确原因并实现标准错误接口。
    #[test]
    fn stable_id_errors_describe_the_rejection() {
        fn require_error<T: std::error::Error>() {}
        require_error::<StableIdError>();
        assert_eq!(
            StableIdError::InvalidFormat.to_string(),
            "稳定 ID 必须为规范小写 UUID 字符串"
        );
        assert_eq!(
            StableIdError::ExpectedV7.to_string(),
            "稳定 ID 必须为 UUIDv7"
        );
    }
}
