//! DeepSeek/Kimi 套餐响应解析与展示值类型。
//!
//! 解析层只接受 `serde_json::Value`:不发现来源,不读取 CLI 配置或
//! 环境认证材料,不接触凭据、网络与调度。字段缺失或不可解析返回 Err,
//! 不把缺失数据伪造为 0;结果类型不承载 token。

pub(crate) mod deepseek;
pub(crate) mod kimi;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AmountInfo {
    /// 原样透传 total_balance。
    pub value: String,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowsInfo {
    pub five_hour: WindowInfo,
    pub seven_day: WindowInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowInfo {
    /// 已用百分比 = used/limit×100(used 缺失经 remaining 换算回退)。
    pub used_percent: u8,
    /// ISO 字符串,可缺失。
    pub resets_at: Option<String>,
}

/// 单来源套餐解析结果;来源身份、查询时间与错误状态由调用方持有。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanBalanceInfo {
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
}

/// 错过完整周期后才标记过期：从上次成功查询起算，一个周期前仍视为有效。
/// 时钟回拨不判过期，避免把未知时间误标为陈旧。
pub(crate) fn balance_stale(
    last_success: std::time::SystemTime,
    now: std::time::SystemTime,
    period: std::time::Duration,
) -> bool {
    match now.duration_since(last_success) {
        Ok(elapsed) => elapsed >= period,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::balance_stale;
    use std::time::{Duration, SystemTime};

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn plan_balance_stale_after_full_period() {
        let period = Duration::from_secs(60);
        assert!(
            !balance_stale(at(1_000), at(1_059), period),
            "未满一个周期不得标记过期"
        );
        assert!(
            balance_stale(at(1_000), at(1_060), period),
            "满一个完整周期必须标记过期"
        );
        assert!(
            balance_stale(at(1_000), at(2_000), period),
            "长间隔必须过期"
        );
        assert!(
            !balance_stale(at(1_000), at(900), period),
            "时钟回拨不得标记过期"
        );
    }
}

/// 套餐余额来源；每来源一个独立凭据目标，不共享 CLI 认证材料。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanBalanceSource {
    DeepSeek,
    Kimi,
}

impl PlanBalanceSource {
    pub(crate) const fn credential_target(self) -> &'static str {
        match self {
            Self::DeepSeek => "slterm/plan-balance/deepseek",
            Self::Kimi => "slterm/plan-balance/kimi",
        }
    }
}

/// 凭据只经凭据域读取；请求组装方拿到的是即用即焚句柄，不进入日志或 DTO。
pub(crate) struct PlanBalanceCredentials {
    store: std::sync::Arc<dyn crate::secrets::CredentialStore>,
}

impl PlanBalanceCredentials {
    pub(crate) fn new(store: std::sync::Arc<dyn crate::secrets::CredentialStore>) -> Self {
        Self { store }
    }

    pub(crate) fn load(
        &self,
        source: PlanBalanceSource,
    ) -> std::io::Result<Option<crate::secrets::SecretBytes>> {
        self.store.load(source.credential_target())
    }
}

#[cfg(test)]
mod credential_tests {
    use super::{PlanBalanceCredentials, PlanBalanceSource};
    use crate::secrets::{CredentialStore, SecretBytes};
    use std::io;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingStore {
        targets: Mutex<Vec<String>>,
        secret: Option<Vec<u8>>,
    }

    impl CredentialStore for RecordingStore {
        fn load(&self, target: &str) -> io::Result<Option<SecretBytes>> {
            self.targets.lock().unwrap().push(target.to_owned());
            Ok(self.secret.clone().map(SecretBytes::new))
        }

        fn store(&self, _target: &str, _secret: &[u8]) -> io::Result<()> {
            Ok(())
        }

        fn delete(&self, _target: &str) -> io::Result<()> {
            Ok(())
        }
    }

    /// 设置页 token 只写专用凭据目标；解析层不读取环境或 CLI 认证材料。
    #[test]
    fn settings_plan_token_not_cli_credentials() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for file in [
            "src/plan_balance/mod.rs",
            "src/plan_balance/deepseek.rs",
            "src/plan_balance/kimi.rs",
        ] {
            let source = std::fs::read_to_string(root.join(file)).unwrap();
            let production = source.split("#[cfg(test)]").next().unwrap();
            for forbidden in [
                "std::env",
                "ANTHROPIC_API_KEY",
                "OPENAI_API_KEY",
                "settings.json",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{file} 不得经 {forbidden} 读取认证材料"
                );
            }
        }
        for source in [PlanBalanceSource::DeepSeek, PlanBalanceSource::Kimi] {
            assert!(
                source
                    .credential_target()
                    .starts_with("slterm/plan-balance/"),
                "套餐凭据必须使用专用命名空间而非 CLI 凭据"
            );
        }
    }

    /// 套餐只使用专用 Windows 凭据目标，材料不含 token。
    #[test]
    fn plan_balance_uses_dedicated_credential() {
        let store = Arc::new(RecordingStore {
            targets: Mutex::new(Vec::new()),
            secret: Some(b"sk-test-plan-balance".to_vec()),
        });
        let credentials = PlanBalanceCredentials::new(store.clone());
        let secret = credentials
            .load(PlanBalanceSource::DeepSeek)
            .unwrap()
            .expect("专用目标必须提供凭据");
        assert_eq!(
            store.targets.lock().unwrap().as_slice(),
            ["slterm/plan-balance/deepseek"],
            "只允许读取来源专用凭据目标"
        );
        assert_eq!(secret.as_bytes(), b"sk-test-plan-balance");
        assert!(
            !format!("{secret:?}").contains("sk-test"),
            "调试输出不得包含凭据明文"
        );

        let kimi = credentials.load(PlanBalanceSource::Kimi).unwrap();
        assert!(kimi.is_some());
        assert_eq!(
            store.targets.lock().unwrap().last().map(String::as_str),
            Some("slterm/plan-balance/kimi"),
            "不同来源必须使用各自目标"
        );
    }
}
