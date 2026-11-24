//! `slterm_hooks` 提供事件目录、schema、草稿模型与托管识别。
//!
//! 本 crate 不写文件。

#![forbid(unsafe_code)]

/// CLI hooks 事件封闭集合；GUI、schema、保存校验与注入识别共用本目录。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PermissionRequest,
    Notification,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    PreCompact,
    SessionStart,
    SessionEnd,
}

impl HookEvent {
    pub const ALL: [Self; 10] = [
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PermissionRequest,
        Self::Notification,
        Self::UserPromptSubmit,
        Self::Stop,
        Self::SubagentStop,
        Self::PreCompact,
        Self::SessionStart,
        Self::SessionEnd,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PermissionRequest => "PermissionRequest",
            Self::Notification => "Notification",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Stop => "Stop",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
        }
    }

    /// 事件名严格匹配；未知事件不得被静默归入已知事件。
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == name)
    }

    /// 是否按工具名（matcher）过滤；通知与回合边界没有 matcher。
    pub const fn supports_matcher(self) -> bool {
        matches!(
            self,
            Self::PreToolUse | Self::PostToolUse | Self::PermissionRequest
        )
    }

    /// 事件声明的 payload 字段；GUI 与 schema 只认该集合。
    pub const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::PreToolUse => &["tool_name", "tool_input", "tool_use_id"],
            Self::PostToolUse => &["tool_name", "tool_input", "tool_response", "tool_use_id"],
            Self::PermissionRequest => &["tool_name", "tool_input", "tool_use_id"],
            Self::Notification => &["message", "title"],
            Self::UserPromptSubmit => &["prompt"],
            Self::Stop | Self::SubagentStop => &["stop_hook_active"],
            Self::PreCompact => &["trigger", "custom_instructions"],
            Self::SessionStart => &["source"],
            Self::SessionEnd => &["reason"],
        }
    }
}

/// handler 类型封闭集合；目录之外的类型一律拒绝保存。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookHandlerType {
    Command,
    Http,
    McpTool,
    Prompt,
    Agent,
}

impl HookHandlerType {
    pub const ALL: [Self; 5] = [
        Self::Command,
        Self::Http,
        Self::McpTool,
        Self::Prompt,
        Self::Agent,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Http => "http",
            Self::McpTool => "mcp_tool",
            Self::Prompt => "prompt",
            Self::Agent => "agent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|handler| handler.as_str() == value)
    }
}

/// 托管标识值：版本化，只由注入器写入 marker 字段。
pub const MANAGED_MARKER_VALUE: &str = "slterm-hook/1";

/// 托管识别只认精确 marker 值；命令里出现产品名不构成归属。
pub fn is_managed_marker(value: Option<&str>) -> bool {
    value == Some(MANAGED_MARKER_VALUE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 事件目录封闭、名称唯一且解析往返；未知事件不归入已知事件。
    #[test]
    fn hook_event_catalog_is_closed_and_unique() {
        let mut names = std::collections::HashSet::new();
        for event in HookEvent::ALL {
            assert!(names.insert(event.as_str()), "{event:?} 名称重复");
            assert_eq!(HookEvent::parse(event.as_str()), Some(event));
            assert!(!event.as_str().is_empty());
        }
        assert_eq!(names.len(), HookEvent::ALL.len());
        assert_eq!(HookEvent::parse("Unknown"), None);
        assert_eq!(HookEvent::parse("stop"), None, "事件名大小写严格");
    }

    /// handler 类型目录封闭且名称唯一。
    #[test]
    fn hook_handler_types_are_closed() {
        let mut names = std::collections::HashSet::new();
        for handler in HookHandlerType::ALL {
            assert!(names.insert(handler.as_str()));
            assert_eq!(HookHandlerType::parse(handler.as_str()), Some(handler));
        }
        assert_eq!(names.len(), 5);
        assert_eq!(HookHandlerType::parse("shell"), None);
    }

    /// matcher 只属于工具事件；其余事件不声明 matcher 能力。
    #[test]
    fn hook_matcher_capability_matches_tool_events() {
        for event in HookEvent::ALL {
            let expected = matches!(
                event,
                HookEvent::PreToolUse | HookEvent::PostToolUse | HookEvent::PermissionRequest
            );
            assert_eq!(event.supports_matcher(), expected, "{event:?}");
        }
        assert!(HookEvent::PreToolUse.fields().contains(&"tool_name"));
        assert!(!HookEvent::Stop.fields().contains(&"tool_name"));
    }

    /// 托管识别只认精确 marker；普通命令含产品名不得被认领。
    #[test]
    fn managed_identification_requires_exact_marker() {
        assert!(is_managed_marker(Some(MANAGED_MARKER_VALUE)));
        assert!(!is_managed_marker(None));
        assert!(!is_managed_marker(Some("slterm-hook/2")));
        assert!(
            !is_managed_marker(Some("C:/Tools/slterm-hook.exe --hooks=turns")),
            "命令文本不构成托管归属"
        );
    }
}
