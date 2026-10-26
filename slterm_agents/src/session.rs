use std::fmt;

use crate::AgentKind;
use crate::identity::valid_session_id;

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
