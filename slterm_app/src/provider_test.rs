//! provider 连通性测试的语义化结果(08 篇)。
//!
//! 测试执行体只返回稳定语义与参数,本地化文案在 UI 展示边界生成
//! (06/08 篇呈现面归 M9.5);凭据查找内聚在执行体内,任何 UI 零接触明文。

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
}

#[cfg(test)]
mod provider_test_tests {
    use super::ProviderTestOutcome;

    #[test]
    fn only_success_is_success() {
        assert!(ProviderTestOutcome::Success { status: 200 }.is_success());
        assert!(!ProviderTestOutcome::AuthFailed { status: 401 }.is_success());
        assert!(!ProviderTestOutcome::Timeout.is_success());
        assert!(!ProviderTestOutcome::StartFailed { error: "x".into() }.is_success());
    }
}
