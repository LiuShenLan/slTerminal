//! provider 连通性测试的语义化结果。
//!
//! 测试执行体只返回稳定语义与参数,本地化文案在 UI 展示边界生成
//! (呈现面归终端视图);凭据查找内聚在执行体内,任何 UI 零接触明文。

use crate::i18n::{Message, UiLanguage};

/// 连通性测试结果;`is_success` 是 UI 唯一需要的快捷判定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderTestOutcome {
    Success { status: u16 },
    InvalidEndpoint,
    MissingModel,
    MissingApiKey,
    CredentialReadFailed,
    InvalidCredentialEncoding,
    Timeout,
    HostNotFound,
    ConnectionFailed,
    Io { kind: String },
    Tls,
    RequestFailed,
    AuthFailed { status: u16 },
    EndpointNotFound { status: u16 },
    RateLimited { status: u16 },
    HttpStatus { status: u16 },
    StartFailed { error: String },
}

impl ProviderTestOutcome {
    pub(crate) const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// 语义 → typed 文案 id。穷尽匹配:新增变体必须同步补本地化目录,
    /// 不允许 `_` 兜底把新语义静默降级成旧文案。
    pub(crate) fn message(self, _language: UiLanguage) -> Message {
        match self {
            Self::Success { .. } => Message::ProviderTestSuccess,
            Self::InvalidEndpoint => Message::ProviderTestInvalidEndpoint,
            Self::MissingModel => Message::ProviderTestMissingModel,
            Self::MissingApiKey => Message::ProviderTestMissingApiKey,
            Self::CredentialReadFailed => Message::ProviderTestCredentialReadFailed,
            Self::InvalidCredentialEncoding => Message::ProviderTestInvalidCredentialEncoding,
            Self::Timeout => Message::ProviderTestTimeout,
            Self::HostNotFound => Message::ProviderTestHostNotFound,
            Self::ConnectionFailed => Message::ProviderTestConnectionFailed,
            Self::Io { .. } => Message::ProviderTestIo,
            Self::Tls => Message::ProviderTestTls,
            Self::RequestFailed => Message::ProviderTestRequestFailed,
            Self::AuthFailed { .. } => Message::ProviderTestAuthFailed,
            Self::EndpointNotFound { .. } => Message::ProviderTestEndpointNotFound,
            Self::RateLimited { .. } => Message::ProviderTestRateLimited,
            Self::HttpStatus { .. } => Message::ProviderTestHttpStatus,
            Self::StartFailed { .. } => Message::ProviderTestStartFailed,
        }
    }

    /// 展示文案:状态码/底层错误串保留原文,模板只加可读前缀与说明。
    pub(crate) fn text(self, language: UiLanguage) -> String {
        match self {
            Self::Success { status } => {
                let status = status.to_string();
                language.format(Message::ProviderTestSuccess, &[("status", &status)])
            }
            Self::InvalidEndpoint => language
                .text(Message::ProviderTestInvalidEndpoint)
                .to_owned(),
            Self::MissingModel => language.text(Message::ProviderTestMissingModel).to_owned(),
            Self::MissingApiKey => language.text(Message::ProviderTestMissingApiKey).to_owned(),
            Self::CredentialReadFailed => language
                .text(Message::ProviderTestCredentialReadFailed)
                .to_owned(),
            Self::InvalidCredentialEncoding => language
                .text(Message::ProviderTestInvalidCredentialEncoding)
                .to_owned(),
            Self::Timeout => language.text(Message::ProviderTestTimeout).to_owned(),
            Self::HostNotFound => language.text(Message::ProviderTestHostNotFound).to_owned(),
            Self::ConnectionFailed => language
                .text(Message::ProviderTestConnectionFailed)
                .to_owned(),
            Self::Io { kind } => language.format(Message::ProviderTestIo, &[("error", &kind)]),
            Self::Tls => language.text(Message::ProviderTestTls).to_owned(),
            Self::RequestFailed => language.text(Message::ProviderTestRequestFailed).to_owned(),
            Self::AuthFailed { status } => {
                let status = status.to_string();
                language.format(Message::ProviderTestAuthFailed, &[("status", &status)])
            }
            Self::EndpointNotFound { status } => {
                let status = status.to_string();
                language.format(
                    Message::ProviderTestEndpointNotFound,
                    &[("status", &status)],
                )
            }
            Self::RateLimited { status } => {
                let status = status.to_string();
                language.format(Message::ProviderTestRateLimited, &[("status", &status)])
            }
            Self::HttpStatus { status } => {
                let status = status.to_string();
                language.format(Message::ProviderTestHttpStatus, &[("status", &status)])
            }
            Self::StartFailed { error } => {
                language.format(Message::ProviderTestStartFailed, &[("error", &error)])
            }
        }
    }
}

#[cfg(test)]
mod provider_test_tests {
    use crate::i18n::UiLanguage;

    use super::ProviderTestOutcome;

    /// 全变体样本(新增变体必须同步此处,穷尽性由编译期 match 兜底)。
    fn all_outcomes() -> Vec<ProviderTestOutcome> {
        vec![
            ProviderTestOutcome::Success { status: 200 },
            ProviderTestOutcome::InvalidEndpoint,
            ProviderTestOutcome::MissingModel,
            ProviderTestOutcome::MissingApiKey,
            ProviderTestOutcome::CredentialReadFailed,
            ProviderTestOutcome::InvalidCredentialEncoding,
            ProviderTestOutcome::Timeout,
            ProviderTestOutcome::HostNotFound,
            ProviderTestOutcome::ConnectionFailed,
            ProviderTestOutcome::Io {
                kind: "kind-7".into(),
            },
            ProviderTestOutcome::Tls,
            ProviderTestOutcome::RequestFailed,
            ProviderTestOutcome::AuthFailed { status: 401 },
            ProviderTestOutcome::EndpointNotFound { status: 404 },
            ProviderTestOutcome::RateLimited { status: 429 },
            ProviderTestOutcome::HttpStatus { status: 500 },
            ProviderTestOutcome::StartFailed {
                error: "spawn boom".into(),
            },
        ]
    }

    #[test]
    fn only_success_is_success() {
        assert!(ProviderTestOutcome::Success { status: 200 }.is_success());
        assert!(!ProviderTestOutcome::AuthFailed { status: 401 }.is_success());
        assert!(!ProviderTestOutcome::Timeout.is_success());
        assert!(!ProviderTestOutcome::StartFailed { error: "x".into() }.is_success());
    }

    /// 每个变体在两语下都有非空且互不相同的面向用户文案。
    #[test]
    fn every_outcome_text_is_nonempty_and_language_specific() {
        for outcome in all_outcomes() {
            let zh = outcome.clone().text(UiLanguage::ZhCn);
            let en = outcome.clone().text(UiLanguage::EnUs);
            assert!(!zh.trim().is_empty(), "{outcome:?} 缺中文文案");
            assert!(!en.trim().is_empty(), "{outcome:?} 缺英文文案");
            assert_ne!(zh, en, "{outcome:?} 中英文案不得相同");
            assert!(
                !zh.contains("ProviderTest") && !en.contains("ProviderTest"),
                "{outcome:?} 不得原样输出 Debug 变体名"
            );
        }
    }

    /// 无参数变体的文案 = typed 文案 id 的查表结果(展示面接线同一入口)。
    #[test]
    fn static_outcome_text_matches_its_message_id() {
        for outcome in [
            ProviderTestOutcome::InvalidEndpoint,
            ProviderTestOutcome::MissingModel,
            ProviderTestOutcome::MissingApiKey,
            ProviderTestOutcome::Timeout,
            ProviderTestOutcome::Tls,
            ProviderTestOutcome::RequestFailed,
        ] {
            for language in [UiLanguage::ZhCn, UiLanguage::EnUs] {
                let message = outcome.clone().message(language);
                assert_eq!(outcome.clone().text(language), language.text(message));
            }
        }
    }

    /// 带状态码的变体必须把数字插值进文案。
    #[test]
    fn status_variants_embed_the_numeric_status() {
        for (outcome, status) in [
            (ProviderTestOutcome::Success { status: 200 }, 200),
            (ProviderTestOutcome::AuthFailed { status: 401 }, 401),
            (ProviderTestOutcome::EndpointNotFound { status: 404 }, 404),
            (ProviderTestOutcome::RateLimited { status: 429 }, 429),
            (ProviderTestOutcome::HttpStatus { status: 500 }, 500),
        ] {
            for language in [UiLanguage::ZhCn, UiLanguage::EnUs] {
                let text = outcome.clone().text(language);
                assert!(
                    text.contains(&status.to_string()),
                    "{outcome:?} 文案缺状态码 {status}: {text}"
                );
            }
        }
    }

    /// 底层错误串保留原文,前缀只负责可读性。
    #[test]
    fn low_level_errors_keep_the_original_text() {
        let io = ProviderTestOutcome::Io {
            kind: "network unreachable".into(),
        };
        let start = ProviderTestOutcome::StartFailed {
            error: "CreateProcessW failed".into(),
        };
        for language in [UiLanguage::ZhCn, UiLanguage::EnUs] {
            assert!(io.clone().text(language).contains("network unreachable"));
            assert!(
                start
                    .clone()
                    .text(language)
                    .contains("CreateProcessW failed")
            );
        }
    }
}
