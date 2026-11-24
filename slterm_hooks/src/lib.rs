//! `slterm_hooks` 提供事件目录、schema、草稿模型与托管识别。
//!
//! 本 crate 不写文件。

#![forbid(unsafe_code)]

/// CLI hooks 事件封闭集合；GUI、schema、保存校验与注入识别共用本目录。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    Notification,
    PermissionRequest,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    StopFailure,
    SessionEnd,
}

impl HookEvent {
    pub const ALL: [Self; 10] = [
        Self::SessionStart,
        Self::UserPromptSubmit,
        Self::Notification,
        Self::PermissionRequest,
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::Stop,
        Self::StopFailure,
        Self::SessionEnd,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Notification => "Notification",
            Self::PermissionRequest => "PermissionRequest",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
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
            Self::PreToolUse
                | Self::PostToolUse
                | Self::PostToolUseFailure
                | Self::PermissionRequest
        )
    }

    /// 事件声明的 payload 字段；GUI 与 schema 只认该集合。
    pub const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::PreToolUse | Self::PermissionRequest => {
                &["tool_name", "tool_input", "tool_use_id"]
            }
            Self::PostToolUse => &["tool_name", "tool_input", "tool_response", "tool_use_id"],
            Self::PostToolUseFailure => &["tool_name", "tool_input", "error", "tool_use_id"],
            Self::Notification => &["message", "title"],
            Self::UserPromptSubmit => &["prompt"],
            Self::Stop | Self::StopFailure => &["stop_hook_active"],
            Self::SessionStart => &["source"],
            Self::SessionEnd => &["reason"],
        }
    }
}

/// 安装器订阅的 Claude 事件白名单；注入器与编辑器共用同一顺序。
pub const CLAUDE_EVENTS: [&str; 10] = [
    HookEvent::SessionStart.as_str(),
    HookEvent::UserPromptSubmit.as_str(),
    HookEvent::Notification.as_str(),
    HookEvent::PermissionRequest.as_str(),
    HookEvent::PreToolUse.as_str(),
    HookEvent::PostToolUse.as_str(),
    HookEvent::PostToolUseFailure.as_str(),
    HookEvent::Stop.as_str(),
    HookEvent::StopFailure.as_str(),
    HookEvent::SessionEnd.as_str(),
];

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

    /// 保存校验与表单共用的必填字段矩阵。
    pub const fn required_fields(self) -> &'static [&'static str] {
        match self {
            Self::Command => &["command"],
            Self::Http => &["url"],
            Self::McpTool => &["server", "tool"],
            Self::Prompt | Self::Agent => &["prompt"],
        }
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

    /// Claude 订阅白名单与事件目录同源且顺序固定。
    #[test]
    fn claude_events_derive_from_catalog() {
        assert_eq!(CLAUDE_EVENTS.len(), HookEvent::ALL.len());
        for name in CLAUDE_EVENTS {
            assert!(HookEvent::parse(name).is_some(), "{name} 不在事件目录");
        }
        assert_eq!(CLAUDE_EVENTS[0], "SessionStart", "安装器事件顺序不得漂移");
    }

    /// handler 类型目录封闭且名称唯一；必填字段来自同一矩阵。
    #[test]
    fn hook_handler_types_are_closed() {
        let mut names = std::collections::HashSet::new();
        for handler in HookHandlerType::ALL {
            assert!(names.insert(handler.as_str()));
            assert_eq!(HookHandlerType::parse(handler.as_str()), Some(handler));
            assert!(!handler.required_fields().is_empty());
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
                HookEvent::PreToolUse
                    | HookEvent::PostToolUse
                    | HookEvent::PostToolUseFailure
                    | HookEvent::PermissionRequest
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
