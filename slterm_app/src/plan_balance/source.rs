//! 余量来源(10 篇 M10.11,F10 语义原样):trait + 静态切片注册表。
//!
//! 双通道裁定(10 篇):plan_balance 只读 user 层 `~/.claude/settings.json` 的
//! `env` 段(`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`),不读 Windows
//! 凭据管理器;provider 凭据库通道归 ai_providers,两者不共享凭据本体。
//! 文件缺失/env 缺失/token 空 → None 静默降级(不发请求)。
//!
//! 注册表形态照 hooks provider 先例(静态切片):Rust 无 side-effect import,
//! 测试经参数化注入;「一行注册」目标不变(新增来源 = 新实现 + SOURCES 一行)。

use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::platform::dirs::home_dir;
use crate::secrets::SecretBytes;

/// 余量来源 trait:输入 = 无,输出 = Option<(baseUrl, token 句柄)>。
pub(crate) trait PlanSource: Send + Sync + std::fmt::Debug {
    /// 来源标识(DTO source_id,按注册序排列)。
    fn source_id(&self) -> &'static str;
    /// 解析 (baseUrl, token);文件缺失/env 缺失/字段缺失/token 为空 → None。
    fn resolve(&self) -> Option<(String, SecretBytes)>;
}

/// claude user 层 settings.json 来源(v1 唯一来源)。
#[derive(Debug)]
pub(crate) struct ClaudeUserSettingsSource;

static CLAUDE_USER_SOURCE: ClaudeUserSettingsSource = ClaudeUserSettingsSource;

/// 静态注册表(新增来源 = 新实现 + 此处一行)。
pub(crate) static SOURCES: &[&dyn PlanSource] = &[&CLAUDE_USER_SOURCE];

impl PlanSource for ClaudeUserSettingsSource {
    fn source_id(&self) -> &'static str {
        "claude"
    }

    fn resolve(&self) -> Option<(String, SecretBytes)> {
        resolve_settings_path(&settings_path()?)
    }
}

/// user 层 settings.json 路径;测试经 [`test_support::SettingsPathGuard`] 覆盖。
fn settings_path() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(path) = test_support::overridden_settings_path() {
        return Some(path);
    }
    Some(home_dir()?.join(".claude").join("settings.json"))
}

/// 显式路径形态:生产 resolve 与 L1 tempdir 用例共用同一条读取链。
fn resolve_settings_path(path: &Path) -> Option<(String, SecretBytes)> {
    // 文件内容含 token:整份缓冲 Zeroizing,解析出句柄后随作用域擦除。
    let content = Zeroizing::new(std::fs::read_to_string(path).ok()?);
    resolve_env(&content)
}

/// env 段提取(纯函数,L1 全测):JSON 损坏/BASE_URL 缺失/token 缺失或为空 → None。
fn resolve_env(content: &str) -> Option<(String, SecretBytes)> {
    let root: serde_json::Value = match serde_json::from_str(content) {
        Ok(value) => value,
        Err(error) => {
            log::warn!("claude settings.json 解析失败，余量来源静默降级: {error}");
            return None;
        }
    };
    let env = root.get("env")?;
    let base_url = env.get("ANTHROPIC_BASE_URL")?.as_str()?;
    let token = env.get("ANTHROPIC_AUTH_TOKEN")?.as_str()?;
    if token.is_empty() {
        return None;
    }
    // token 先复制进句柄,原文件缓冲(content)随 Zeroizing 一起擦除。
    let token_bytes = token.as_bytes().to_vec();
    Some((base_url.to_owned(), SecretBytes::new(token_bytes)))
}

/// 测试专用路径覆盖:进程级静态 + Drop 还原,依赖 `--test-threads=1` 串行纪律。
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::Mutex;

    static OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

    /// 覆盖 user 层 settings.json 路径的守卫;Drop 即还原。
    pub(crate) struct SettingsPathGuard;

    impl SettingsPathGuard {
        pub(crate) fn set(path: PathBuf) -> Self {
            *OVERRIDE.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(path);
            SettingsPathGuard
        }
    }

    impl Drop for SettingsPathGuard {
        fn drop(&mut self) {
            *OVERRIDE.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
        }
    }

    pub(super) fn overridden_settings_path() -> Option<PathBuf> {
        OVERRIDE
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    fn token_text(secret: SecretBytes) -> String {
        secret
            .into_string()
            .expect("测试 token 为 UTF-8")
            .as_str()
            .to_owned()
    }

    // ── resolve_env 纯函数(6 例) ──

    /// 正常:env 含 base_url + token → Some,token 原样返回。
    #[test]
    fn resolve_env_ok_returns_base_url_and_token() {
        let content = r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.deepseek.com/anthropic","ANTHROPIC_AUTH_TOKEN":"sk-test-123"}}"#;
        let (base_url, token) = resolve_env(content).expect("合法 env 应解析成功");
        assert_eq!(base_url, "https://api.deepseek.com/anthropic");
        assert_eq!(token_text(token), "sk-test-123", "token 应原样返回");
    }

    /// JSON 损坏 → None(静默降级)。
    #[test]
    fn resolve_env_invalid_json_returns_none() {
        assert!(resolve_env("not json {{{").is_none());
    }

    /// env 键缺失 → None。
    #[test]
    fn resolve_env_missing_env_returns_none() {
        assert!(resolve_env(r#"{"other": 1}"#).is_none());
    }

    /// ANTHROPIC_BASE_URL 缺失 → None。
    #[test]
    fn resolve_env_missing_base_url_returns_none() {
        let content = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-test"}}"#;
        assert!(resolve_env(content).is_none());
    }

    /// ANTHROPIC_AUTH_TOKEN 缺失 → None。
    #[test]
    fn resolve_env_missing_token_returns_none() {
        let content = r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com"}}"#;
        assert!(resolve_env(content).is_none());
    }

    /// token 空串 → None(红线:空 token 不发起请求)。
    #[test]
    fn resolve_env_empty_token_returns_none() {
        let content =
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com","ANTHROPIC_AUTH_TOKEN":""}}"#;
        assert!(resolve_env(content).is_none());
    }

    // ── 文件读取链(tempdir 隔离) ──

    /// 文件不存在 → None(静默降级)。
    #[test]
    fn resolve_settings_path_missing_file_returns_none() {
        let dir = tempfile::tempdir().expect("创建临时目录");
        assert!(resolve_settings_path(&dir.path().join(".claude").join("settings.json")).is_none());
    }

    /// 落盘合法 settings → Some 且 token 原样返回。
    #[test]
    fn resolve_settings_path_valid_settings_returns_token() {
        let dir = tempfile::tempdir().expect("创建临时目录");
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).expect("创建 .claude 目录");
        let settings = serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                "ANTHROPIC_AUTH_TOKEN": "sk-test-user-layer",
            }
        });
        let path = claude_dir.join("settings.json");
        std::fs::write(&path, serde_json::to_string(&settings).expect("序列化夹具")).expect("写盘");

        let (base_url, token) = resolve_settings_path(&path).expect("合法 settings 应解析成功");
        assert_eq!(base_url, "https://api.deepseek.com/anthropic");
        assert_eq!(token_text(token), "sk-test-user-layer");
    }

    /// 来源标识恒 "claude"(DTO source_id 契约)。
    #[test]
    fn claude_source_id_is_stable() {
        assert_eq!(CLAUDE_USER_SOURCE.source_id(), "claude");
        assert_eq!(SOURCES.len(), 1, "v1 唯一来源");
        assert_eq!(SOURCES[0].source_id(), "claude");
    }
}
