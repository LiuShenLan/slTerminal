//! 套餐查询注册表（规格 §4.2/§10）：URL 匹配集 + fetch trait；静态切片（U2）

use super::FetchOutcome;
use crate::error::AppError;

/// 套餐查询 trait：新增套餐 = 新实现 + QUERIES 一行注册
pub trait PlanQuery: Send + Sync + std::fmt::Debug {
    fn plan_id(&self) -> &'static str;
    /// URL 匹配集（元素须为已归一化形态：小写、无尾斜杠）
    fn base_urls(&self) -> &'static [&'static str];
    /// 阻塞 HTTP 查询（调用方负责 spawn_blocking，硬约束 #3）；
    /// 错误消息禁止含 token/Authorization（红线）
    fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError>;
}

static DEEPSEEK: super::deepseek::DeepSeekQuery = super::deepseek::DeepSeekQuery;
static KIMI: super::kimi::KimiQuery = super::kimi::KimiQuery;

/// 按注册序（emit 数组顺序即此序）
pub(crate) static QUERIES: &[&dyn PlanQuery] = &[&DEEPSEEK, &KIMI];

/// URL 归一化（规格 §4.2 字面）：小写化 + 去尾部斜杠
pub(crate) fn normalize_base_url(url: &str) -> String {
    url.to_lowercase().trim_end_matches('/').to_string()
}

/// 归一化后与匹配集逐项精确相等（参数化查找供 L1 注入，照 lookup_provider 先例）
pub(crate) fn find_query_by_url<'a>(
    base_url: &str,
    queries: &'a [&'a dyn PlanQuery],
) -> Option<&'a dyn PlanQuery> {
    let normalized = normalize_base_url(base_url);
    queries
        .iter()
        .copied()
        .find(|q| q.base_urls().contains(&normalized.as_str()))
}

/// ureq agent 工厂（D10）：timeout_global 覆盖连接/读写全程；4xx/5xx 默认即 Err
pub(crate) fn http_agent(timeout: std::time::Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// 查询错误统一映射（消息只含 planId + 错误类别，禁止拼 token）
pub(crate) fn query_err(plan_id: &str, e: ureq::Error) -> AppError {
    let kind = match &e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::Timeout(_) => "超时".to_string(),
        _ => "网络错误".to_string(),
    };
    tracing::warn!(plan = plan_id, error = %e, "套餐余量 HTTP 查询失败");
    AppError::Unknown(format!("套餐 {plan_id} 查询失败: {kind}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_base_url（4 例，F10；规格字面：仅小写 + 去尾斜杠，不加 trim） ──

    /// 大写宿主 → 全小写
    #[test]
    fn normalize_lowercases_host() {
        assert_eq!(
            normalize_base_url("https://API.DEEPSEEK.COM/anthropic"),
            "https://api.deepseek.com/anthropic"
        );
    }

    /// 单尾斜杠 → 去除
    #[test]
    fn normalize_strips_single_trailing_slash() {
        assert_eq!(
            normalize_base_url("https://api.kimi.com/coding/"),
            "https://api.kimi.com/coding"
        );
    }

    /// 多尾斜杠 → 全部去除
    #[test]
    fn normalize_strips_all_trailing_slashes() {
        assert_eq!(
            normalize_base_url("https://api.kimi.com/coding///"),
            "https://api.kimi.com/coding"
        );
    }

    /// 已归一形态 → 保持不变
    #[test]
    fn normalize_keeps_normalized_unchanged() {
        assert_eq!(
            normalize_base_url("https://api.deepseek.com/anthropic"),
            "https://api.deepseek.com/anthropic"
        );
    }

    // ── find_query_by_url（4 例，参数化注入） ──

    /// deepseek base_url 命中
    #[test]
    fn find_hits_deepseek() {
        let q = find_query_by_url("https://api.deepseek.com/anthropic", QUERIES).unwrap();
        assert_eq!(q.plan_id(), "deepseek");
    }

    /// kimi base_url 命中
    #[test]
    fn find_hits_kimi() {
        let q = find_query_by_url("https://api.kimi.com/coding", QUERIES).unwrap();
        assert_eq!(q.plan_id(), "kimi");
    }

    /// 大小写 + 尾斜杠归一后仍命中（匹配前统一归一化）
    #[test]
    fn find_matches_after_normalization() {
        let q = find_query_by_url("HTTPS://API.DEEPSEEK.COM/ANTHROPIC/", QUERIES).unwrap();
        assert_eq!(q.plan_id(), "deepseek");
    }

    /// 未命中（其他厂商 base_url）→ None
    #[test]
    fn find_unmatched_returns_none() {
        assert!(find_query_by_url("https://api.other.com/v1", QUERIES).is_none());
    }

    // ── 注册表序（1 例，emit 数组顺序即注册序） ──

    /// QUERIES plan_id 序恒为 ["deepseek", "kimi"]（前端渲染顺序依赖此序）
    #[test]
    fn queries_registry_order() {
        let ids: Vec<&str> = QUERIES.iter().map(|q| q.plan_id()).collect();
        assert_eq!(ids, ["deepseek", "kimi"]);
    }
}
