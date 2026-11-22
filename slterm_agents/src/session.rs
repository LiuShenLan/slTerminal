use std::fmt;

use serde::{Deserialize, Serialize};

use crate::AgentKind;
use crate::identity::valid_session_id;

/// 快照瞬间仍在 pane 前台运行的 AI CLI 对话身份。
/// 只保存来源、会话 ID 与原生会话文件，不保存正文或启动参数原文。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSession {
    /// hook 直报的 CLI 名；宽松落盘，恢复时严格归一。
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// 原生会话文件；只在所属 pane 环境内解释。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
}

impl AgentSession {
    /// 只经 AgentCatalog 的能力与身份校验构造可恢复命令。
    pub fn resume_command(&self) -> Option<String> {
        let agent = AgentKind::parse(&self.source)?;
        if agent == AgentKind::Codex
            && let Some(id) = self.session_file.as_deref().and_then(codex_rollout_id)
        {
            return agent.resume_command(id);
        }
        match self.session_id.as_deref() {
            // Pi 的 pid- 前缀是进程身份，不是可恢复会话身份。
            Some(id) if agent == AgentKind::Pi && id.starts_with("pid-") => None,
            Some(id) => agent.resume_command(id),
            None if agent == AgentKind::Claude => Some("claude --continue".to_owned()),
            _ => None,
        }
    }
}

/// 同时校验 Windows 与来宾绝对路径，不访问文件系统。
pub fn valid_native_session_file(path: &str) -> bool {
    path.len() <= 4096
        && path.ends_with(".jsonl")
        && !path.chars().any(char::is_control)
        && (path.starts_with('/')
            || path.starts_with(r"\\")
            || (path.as_bytes().get(1) == Some(&b':')
                && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && matches!(path.as_bytes().get(2), Some(b'/' | b'\\'))))
}

/// 从合法 codex rollout 文件名提取 `codex resume` 的线程 UUID。
pub fn codex_rollout_id(path: &str) -> Option<&str> {
    if !valid_native_session_file(path) {
        return None;
    }
    let filename = path.rsplit(['/', '\\']).next()?;
    let stem = filename.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let start = stem.len().checked_sub(36)?;
    let id = stem.get(start..)?;
    if start == 0 || stem.as_bytes().get(start - 1) != Some(&b'-') {
        return None;
    }
    id.bytes()
        .enumerate()
        .all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        .then_some(id)
}

/// 原生历史记录与活跃关联共用的会话身份。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentSessionKey {
    agent_kind: AgentKind,
    native_session_id: String,
    adapter_version: u32,
}

/// 原生会话 ID 不符合命令与持久身份的安全形状。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidNativeSessionId;

impl AgentSessionKey {
    /// 使用对应 Agent 的原生 ID 与历史适配器版本构造身份。
    pub fn new(
        agent_kind: AgentKind,
        native_session_id: &str,
        adapter_version: u32,
    ) -> Result<Self, InvalidNativeSessionId> {
        valid_session_id(native_session_id).ok_or(InvalidNativeSessionId)?;
        Ok(Self {
            agent_kind,
            native_session_id: native_session_id.to_owned(),
            adapter_version,
        })
    }

    pub const fn agent_kind(&self) -> AgentKind {
        self.agent_kind
    }

    pub fn native_session_id(&self) -> &str {
        &self.native_session_id
    }

    pub const fn adapter_version(&self) -> u32 {
        self.adapter_version
    }
}

impl fmt::Display for InvalidNativeSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("原生会话 ID 必须为 1–64 个 ASCII 字母、数字、点、连字符或下划线")
    }
}

impl std::error::Error for InvalidNativeSessionId {}

#[cfg(test)]
mod session_tests {
    use super::*;

    /// Agent、原生 ID 与适配器版本都参与身份相等性。
    #[test]
    fn agent_session_key_uses_all_three_fields_as_identity() {
        let key = AgentSessionKey::new(AgentKind::Claude, "session-1", 3).unwrap();
        assert_eq!(key.agent_kind(), AgentKind::Claude);
        assert_eq!(key.native_session_id(), "session-1");
        assert_eq!(key.adapter_version(), 3);
        assert_eq!(key.clone(), key);
        assert_ne!(
            key,
            AgentSessionKey::new(AgentKind::Codex, "session-1", 3).unwrap()
        );
        assert_ne!(
            key,
            AgentSessionKey::new(AgentKind::Claude, "session-2", 3).unwrap()
        );
        assert_ne!(
            key,
            AgentSessionKey::new(AgentKind::Claude, "session-1", 4).unwrap()
        );

        let boundary = "a".repeat(64);
        assert_eq!(
            AgentSessionKey::new(AgentKind::Claude, &boundary, 0)
                .unwrap()
                .native_session_id(),
            boundary
        );
    }

    /// 空、超长、空白、路径与 Shell 特殊字符不得进入会话身份。
    #[test]
    fn agent_session_key_rejects_invalid_native_session_ids() {
        for id in [
            "",
            "session id",
            "x;calc",
            "../session",
            "会话",
            "line\nbreak",
        ] {
            assert_eq!(
                AgentSessionKey::new(AgentKind::Claude, id, 1),
                Err(InvalidNativeSessionId),
                "{id:?}"
            );
        }
        assert_eq!(
            AgentSessionKey::new(AgentKind::Claude, &"a".repeat(65), 1),
            Err(InvalidNativeSessionId)
        );
    }

    #[test]
    fn resume_commands_require_catalog_capability_and_safe_identity() {
        let agent = |source: &str, id: Option<&str>| AgentSession {
            source: source.to_owned(),
            session_id: id.map(str::to_owned),
            session_file: None,
        };
        assert_eq!(
            agent("claude", Some("abc-123")).resume_command().as_deref(),
            Some("claude --resume abc-123")
        );
        assert_eq!(
            agent("claude", None).resume_command().as_deref(),
            Some("claude --continue")
        );
        assert_eq!(
            agent("codex", Some("abc-123")).resume_command().as_deref(),
            Some("codex resume abc-123")
        );
        assert_eq!(agent("codex", None).resume_command(), None);
        assert_eq!(agent("aider", Some("abc")).resume_command(), None);
        assert_eq!(agent("unknown-cli", Some("abc")).resume_command(), None);
        assert_eq!(agent("pi", Some("pid-42")).resume_command(), None);
        for id in ["", "a b", "id\nnext", "abc;calc"] {
            assert_eq!(agent("claude", Some(id)).resume_command(), None);
        }
    }

    #[test]
    fn codex_resume_uses_only_a_valid_native_rollout_identity() {
        const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";
        let agent = AgentSession {
            source: "codex".to_owned(),
            session_id: None,
            session_file: Some(format!(
                "C:/sessions/rollout-2026-09-21T10-00-00-{THREAD}.jsonl"
            )),
        };
        assert_eq!(
            agent.resume_command(),
            Some(format!("codex resume {THREAD}"))
        );
        for path in ["relative.jsonl", "C:/sessions/rollout-x;calc.jsonl"] {
            let agent = AgentSession {
                session_file: Some(path.to_owned()),
                ..agent.clone()
            };
            assert_eq!(agent.resume_command(), None);
        }
    }

    /// 校验错误说明允许形状，不回显不可信 ID。
    #[test]
    fn invalid_native_session_id_error_describes_the_contract() {
        fn require_error<T: std::error::Error>() {}
        require_error::<InvalidNativeSessionId>();
        assert_eq!(
            InvalidNativeSessionId.to_string(),
            "原生会话 ID 必须为 1–64 个 ASCII 字母、数字、点、连字符或下划线"
        );
    }
}
