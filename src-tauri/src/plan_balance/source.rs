//! 余量来源（规格 §4.1/§10）：trait + 静态切片注册表（U2：照 hooks/provider.rs
//! 先例，偏离 #13 可变单例——Rust 无 side-effect import，测试经参数化注入）
//!
//! home 解析照 hooks/claude/mod.rs home_dir()/HomeDirGuard 模式自建（D2：
//! 硬约束 #2 模块不穿透，不跨模块调用 hooks::claude）

use std::path::PathBuf;

/// 余量来源 trait：输入 = 无，输出 = Option<(baseUrl, token)>；解析格式由各来源自定
pub trait PlanSource: Send + Sync + std::fmt::Debug {
    /// 来源标识（DTO source_id，按注册序 emit）
    fn source_id(&self) -> &'static str;
    /// 解析 (baseUrl, token)；文件缺失/env 缺失/字段缺失/token 为空 → None（静默降级 §8.3）
    fn resolve(&self) -> Option<(String, String)>;
}

/// claude user 层 settings.json 来源（v1 唯一来源）
#[derive(Debug)]
pub struct ClaudeUserSettingsSource;

static CLAUDE_USER_SOURCE: ClaudeUserSettingsSource = ClaudeUserSettingsSource;

/// 静态注册表（新增来源 = 新实现 + 此处一行）
pub(crate) static SOURCES: &[&dyn PlanSource] = &[&CLAUDE_USER_SOURCE];

impl PlanSource for ClaudeUserSettingsSource {
    fn source_id(&self) -> &'static str {
        "claude"
    }
    fn resolve(&self) -> Option<(String, String)> {
        resolve_env(&claude_settings_content()?)
    }
}

fn claude_settings_content() -> Option<String> {
    let path = home_dir()?.join(".claude").join("settings.json");
    std::fs::read_to_string(path).ok() // 不存在/不可读 → None（静默降级）
}

/// env 段提取（纯函数，L1 全测）：JSON 损坏/BASE_URL 缺失/token 缺失或为空 → None
fn resolve_env(content: &str) -> Option<(String, String)> {
    let root: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "claude settings.json 解析失败，余量来源静默降级");
            return None;
        }
    };
    let env = root.get("env")?;
    let base_url = env.get("ANTHROPIC_BASE_URL")?.as_str()?;
    let token = env.get("ANTHROPIC_AUTH_TOKEN")?.as_str()?;
    if token.is_empty() {
        return None;
    }
    Some((base_url.to_string(), token.to_string()))
}

// ── home 解析（照 hooks/claude/mod.rs 先例模式，cfg(test) 注入守卫） ──
#[cfg(test)]
static HOME_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
#[cfg(test)]
pub(crate) struct HomeDirGuard(Option<PathBuf>);
#[cfg(test)]
impl HomeDirGuard {
    pub(crate) fn set(dir: &std::path::Path) -> Self {
        let mut slot = HOME_DIR_OVERRIDE.lock().unwrap();
        let prev = slot.clone();
        *slot = Some(dir.to_path_buf());
        HomeDirGuard(prev)
    }
}
#[cfg(test)]
impl Drop for HomeDirGuard {
    fn drop(&mut self) {
        *HOME_DIR_OVERRIDE.lock().unwrap() = self.0.clone();
    }
}
fn home_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(d) = HOME_DIR_OVERRIDE.lock().unwrap().clone() {
            return Some(d);
        }
    }
    dirs::home_dir()
}

#[cfg(test)]
mod source_tests {
    use super::*;

    // ── resolve_env 纯函数（6 例，F10） ──

    /// 正常：env 含 base_url + token → Some，token 原样返回
    #[test]
    fn resolve_env_ok_returns_base_url_and_token() {
        let content = r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.deepseek.com/anthropic","ANTHROPIC_AUTH_TOKEN":"sk-test-123"}}"#;
        let (base_url, token) = resolve_env(content).expect("合法 env 应解析成功");
        assert_eq!(base_url, "https://api.deepseek.com/anthropic");
        assert_eq!(token, "sk-test-123", "token 应原样返回");
    }

    /// JSON 损坏 → None（静默降级）
    #[test]
    fn resolve_env_invalid_json_returns_none() {
        assert!(resolve_env("not json {{{").is_none());
    }

    /// env 键缺失 → None
    #[test]
    fn resolve_env_missing_env_returns_none() {
        assert!(resolve_env(r#"{"other": 1}"#).is_none());
    }

    /// ANTHROPIC_BASE_URL 缺失 → None
    #[test]
    fn resolve_env_missing_base_url_returns_none() {
        let content = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-test"}}"#;
        assert!(resolve_env(content).is_none());
    }

    /// ANTHROPIC_AUTH_TOKEN 缺失 → None
    #[test]
    fn resolve_env_missing_token_returns_none() {
        let content = r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com"}}"#;
        assert!(resolve_env(content).is_none());
    }

    /// token 空串 → None（红线：空 token 不发起请求）
    #[test]
    fn resolve_env_empty_token_returns_none() {
        let content =
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com","ANTHROPIC_AUTH_TOKEN":""}}"#;
        assert!(resolve_env(content).is_none());
    }

    // ── resolve 命令层（2 例，HomeDirGuard 注入 tempdir 隔离，F10） ──

    /// 无 .claude/settings.json → None（静默降级）
    #[test]
    fn claude_source_resolve_missing_settings_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        assert!(CLAUDE_USER_SOURCE.resolve().is_none());
    }

    /// 落盘合法 settings → Some 且 token 原样返回
    #[test]
    fn claude_source_resolve_valid_settings_returns_token() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = HomeDirGuard::set(dir.path());
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                "ANTHROPIC_AUTH_TOKEN": "sk-secret-xyz",
            }
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string(&settings).unwrap(),
        )
        .unwrap();
        let (base_url, token) = CLAUDE_USER_SOURCE
            .resolve()
            .expect("落盘合法 settings 应解析成功");
        assert_eq!(base_url, "https://api.deepseek.com/anthropic");
        assert_eq!(token, "sk-secret-xyz");
    }
}
