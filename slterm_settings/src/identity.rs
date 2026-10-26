use std::fmt;
use std::str::FromStr;

/// 零依赖设置核心内部使用的 UUIDv7 值。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct UuidV7([u8; 16]);

/// 设置域稳定身份的构造或解析错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StableIdError {
    /// 文本不是规范小写连字符 UUID。
    InvalidFormat,
    /// UUID 不是 RFC 4122 变体的 v7。
    ExpectedV7,
}

impl UuidV7 {
    fn from_bytes(bytes: [u8; 16]) -> Result<Self, StableIdError> {
        if bytes[6] >> 4 != 7 || bytes[8] & 0b1100_0000 != 0b1000_0000 {
            return Err(StableIdError::ExpectedV7);
        }
        Ok(Self(bytes))
    }

    fn parse(text: &str) -> Result<Self, StableIdError> {
        if text.len() != 36 || !text.is_ascii() {
            return Err(StableIdError::InvalidFormat);
        }
        let mut parsed = [0_u8; 16];
        let mut high_nibble = None;
        let mut byte_index = 0;
        for (index, byte) in text.bytes().enumerate() {
            if matches!(index, 8 | 13 | 18 | 23) {
                if byte != b'-' {
                    return Err(StableIdError::InvalidFormat);
                }
                continue;
            }
            let nibble = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => return Err(StableIdError::InvalidFormat),
            };
            if let Some(high) = high_nibble.take() {
                parsed[byte_index] = (high << 4) | nibble;
                byte_index += 1;
            } else {
                high_nibble = Some(nibble);
            }
        }
        Self::from_bytes(parsed)
    }
}

impl fmt::Display for UuidV7 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                formatter.write_str("-")?;
            }
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

macro_rules! define_uuid_v7_id {
    ($(#[$metadata:meta])* $name:ident) => {
        $(#[$metadata])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(UuidV7);

        impl $name {
            /// 使用 application 提供的 UUID 字节构造稳定身份。
            pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, StableIdError> {
                UuidV7::from_bytes(bytes).map(Self)
            }

            /// 返回底层 UUID 字节。
            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0.0
            }
        }

        impl FromStr for $name {
            type Err = StableIdError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                UuidV7::parse(text).map(Self)
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
    /// 主题的跨重启稳定身份。
    ThemeId
);
define_uuid_v7_id!(
    /// AgentRoot 的跨重启稳定身份。
    AgentRootId
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
    const V7_BYTES: [u8; 16] = [
        0x01, 0x8f, 0x2f, 0x7c, 0x8f, 0x5a, 0x7c, 0x3d, 0x9e, 0x2b, 0x4a, 0x6f, 0x8c, 0x1d, 0x0e,
        0x2b,
    ];
    const V4: &str = "550e8400-e29b-41d4-a716-446655440000";
    const V4_BYTES: [u8; 16] = [
        0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00,
        0x00,
    ];

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

    /// 设置域身份使用规范小写 UUID 文本。
    #[test]
    fn settings_ids_round_trip_canonical_uuid_text() {
        assert_canonical_roundtrip::<ThemeId>();
        assert_canonical_roundtrip::<AgentRootId>();
    }

    /// 设置域身份拒绝非规范文本与非 v7 值。
    #[test]
    fn settings_ids_reject_noncanonical_text_and_other_versions() {
        assert_text_rejections::<ThemeId>();
        assert_text_rejections::<AgentRootId>();
    }

    /// 应用提供的原始字节同样校验版本与变体。
    #[test]
    fn settings_ids_validate_application_supplied_bytes() {
        let theme = ThemeId::from_bytes(V7_BYTES).unwrap();
        let root = AgentRootId::from_bytes(V7_BYTES).unwrap();
        assert_eq!(theme.as_bytes(), &V7_BYTES);
        assert_eq!(root.as_bytes(), &V7_BYTES);
        assert_eq!(theme.to_string(), V7);
        assert_eq!(root.to_string(), V7);

        assert_eq!(
            ThemeId::from_bytes(V4_BYTES),
            Err(StableIdError::ExpectedV7)
        );
        assert_eq!(
            AgentRootId::from_bytes(V4_BYTES),
            Err(StableIdError::ExpectedV7)
        );

        let mut wrong_variant = V7_BYTES;
        wrong_variant[8] = 0;
        assert_eq!(
            ThemeId::from_bytes(wrong_variant),
            Err(StableIdError::ExpectedV7)
        );
        assert_eq!(
            AgentRootId::from_bytes(wrong_variant),
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
