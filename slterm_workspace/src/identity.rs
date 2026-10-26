use std::fmt;
use std::str::FromStr;

use uuid::{Uuid, Variant, Version};

/// 工作区稳定身份的构造或解析错误。
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

macro_rules! define_uuid_v7_id {
    ($(#[$metadata:meta])* $name:ident) => {
        $(#[$metadata])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// 使用 application 提供的 UUID 构造稳定身份。
            pub fn from_uuid(value: Uuid) -> Result<Self, StableIdError> {
                validate_uuid_v7(value).map(Self)
            }

            /// 返回底层 UUID 值。
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = StableIdError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                parse_uuid_v7(text).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }
    };
}

define_uuid_v7_id!(
    /// Project 的跨重启稳定身份。
    ProjectId
);
define_uuid_v7_id!(
    /// PanelGroup 的跨重启稳定身份。
    PanelGroupId
);
define_uuid_v7_id!(
    /// 逻辑终端的跨重启稳定身份，与 PTY 代次和进程 PID 无关。
    TerminalId
);

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

    fn assert_canonical_roundtrip<T>()
    where
        T: FromStr<Err = StableIdError> + fmt::Display + fmt::Debug + PartialEq,
    {
        let id = T::from_str(V7).unwrap();
        assert_eq!(id.to_string(), V7);
        assert_eq!(T::from_str(&id.to_string()).unwrap(), id);
    }

    fn assert_text_rejections<T>()
    where
        T: FromStr<Err = StableIdError> + fmt::Debug + PartialEq,
    {
        assert_eq!(T::from_str(V4), Err(StableIdError::ExpectedV7));
        for text in [V7.to_uppercase(), V7.replace('-', ""), "not-a-uuid".into()] {
            assert_eq!(
                T::from_str(&text),
                Err(StableIdError::InvalidFormat),
                "{text}"
            );
        }
    }

    /// 三类工作区身份都使用规范小写 UUID 文本。
    #[test]
    fn workspace_ids_round_trip_canonical_uuid_text() {
        assert_canonical_roundtrip::<ProjectId>();
        assert_canonical_roundtrip::<PanelGroupId>();
        assert_canonical_roundtrip::<TerminalId>();
    }

    /// 三类工作区身份都拒绝非规范文本与非 v7 值。
    #[test]
    fn workspace_ids_reject_noncanonical_text_and_other_versions() {
        assert_text_rejections::<ProjectId>();
        assert_text_rejections::<PanelGroupId>();
        assert_text_rejections::<TerminalId>();
    }

    /// 应用注入的 UUID 同样校验版本与变体。
    #[test]
    fn workspace_ids_validate_application_supplied_uuid() {
        let value = Uuid::parse_str(V7).unwrap();
        assert_eq!(ProjectId::from_uuid(value).unwrap().as_uuid(), &value);
        assert_eq!(PanelGroupId::from_uuid(value).unwrap().as_uuid(), &value);
        assert_eq!(TerminalId::from_uuid(value).unwrap().as_uuid(), &value);

        let v4 = Uuid::parse_str(V4).unwrap();
        assert_eq!(ProjectId::from_uuid(v4), Err(StableIdError::ExpectedV7));
        assert_eq!(PanelGroupId::from_uuid(v4), Err(StableIdError::ExpectedV7));
        assert_eq!(TerminalId::from_uuid(v4), Err(StableIdError::ExpectedV7));

        let mut wrong_variant = *value.as_bytes();
        wrong_variant[8] = 0;
        let wrong_variant = Uuid::from_bytes(wrong_variant);
        assert_eq!(
            ProjectId::from_uuid(wrong_variant),
            Err(StableIdError::ExpectedV7)
        );
        assert_eq!(
            PanelGroupId::from_uuid(wrong_variant),
            Err(StableIdError::ExpectedV7)
        );
        assert_eq!(
            TerminalId::from_uuid(wrong_variant),
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
