//! `slterm_hooks` 提供事件目录、schema、草稿模型与托管识别。
//!
//! 本 crate 不写文件。

#![forbid(unsafe_code)]

/// CLI hooks 事件封闭集合；GUI、schema、保存校验与注入识别共用本目录。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    Setup,
    UserPromptSubmit,
    UserPromptExpansion,
    PreToolUse,
    PermissionRequest,
    PermissionDenied,
    PostToolUse,
    PostToolUseFailure,
    PostToolBatch,
    Notification,
    MessageDisplay,
    SubagentStart,
    SubagentStop,
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
    PreCompact,
    PostCompact,
    Stop,
    StopFailure,
    ConfigChange,
    CwdChanged,
    FileChanged,
    InstructionsLoaded,
    DirectoryAdded,
    WorktreeCreate,
    WorktreeRemove,
    Elicitation,
    ElicitationResult,
}

impl HookEvent {
    pub const ALL: [Self; 31] = [
        Self::SessionStart,
        Self::SessionEnd,
        Self::Setup,
        Self::UserPromptSubmit,
        Self::UserPromptExpansion,
        Self::PreToolUse,
        Self::PermissionRequest,
        Self::PermissionDenied,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::PostToolBatch,
        Self::Notification,
        Self::MessageDisplay,
        Self::SubagentStart,
        Self::SubagentStop,
        Self::TaskCreated,
        Self::TaskCompleted,
        Self::TeammateIdle,
        Self::PreCompact,
        Self::PostCompact,
        Self::Stop,
        Self::StopFailure,
        Self::ConfigChange,
        Self::CwdChanged,
        Self::FileChanged,
        Self::InstructionsLoaded,
        Self::DirectoryAdded,
        Self::WorktreeCreate,
        Self::WorktreeRemove,
        Self::Elicitation,
        Self::ElicitationResult,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::Setup => "Setup",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::UserPromptExpansion => "UserPromptExpansion",
            Self::PreToolUse => "PreToolUse",
            Self::PermissionRequest => "PermissionRequest",
            Self::PermissionDenied => "PermissionDenied",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::PostToolBatch => "PostToolBatch",
            Self::Notification => "Notification",
            Self::MessageDisplay => "MessageDisplay",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::TeammateIdle => "TeammateIdle",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
            Self::ConfigChange => "ConfigChange",
            Self::CwdChanged => "CwdChanged",
            Self::FileChanged => "FileChanged",
            Self::InstructionsLoaded => "InstructionsLoaded",
            Self::DirectoryAdded => "DirectoryAdded",
            Self::WorktreeCreate => "WorktreeCreate",
            Self::WorktreeRemove => "WorktreeRemove",
            Self::Elicitation => "Elicitation",
            Self::ElicitationResult => "ElicitationResult",
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
            _ => &[],
        }
    }
}

/// 编辑器与 schema 认的完整 Claude 事件目录（featureSpec 全表）。
pub const CLAUDE_EVENT_CATALOG: [&str; 31] = [
    HookEvent::SessionStart.as_str(),
    HookEvent::SessionEnd.as_str(),
    HookEvent::Setup.as_str(),
    HookEvent::UserPromptSubmit.as_str(),
    HookEvent::UserPromptExpansion.as_str(),
    HookEvent::PreToolUse.as_str(),
    HookEvent::PermissionRequest.as_str(),
    HookEvent::PermissionDenied.as_str(),
    HookEvent::PostToolUse.as_str(),
    HookEvent::PostToolUseFailure.as_str(),
    HookEvent::PostToolBatch.as_str(),
    HookEvent::Notification.as_str(),
    HookEvent::MessageDisplay.as_str(),
    HookEvent::SubagentStart.as_str(),
    HookEvent::SubagentStop.as_str(),
    HookEvent::TaskCreated.as_str(),
    HookEvent::TaskCompleted.as_str(),
    HookEvent::TeammateIdle.as_str(),
    HookEvent::PreCompact.as_str(),
    HookEvent::PostCompact.as_str(),
    HookEvent::Stop.as_str(),
    HookEvent::StopFailure.as_str(),
    HookEvent::ConfigChange.as_str(),
    HookEvent::CwdChanged.as_str(),
    HookEvent::FileChanged.as_str(),
    HookEvent::InstructionsLoaded.as_str(),
    HookEvent::DirectoryAdded.as_str(),
    HookEvent::WorktreeCreate.as_str(),
    HookEvent::WorktreeRemove.as_str(),
    HookEvent::Elicitation.as_str(),
    HookEvent::ElicitationResult.as_str(),
];

/// 安装器订阅的 Claude 事件白名单；注入器只注入该子集，顺序固定。
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

/// handler 字段值形态；表单控件与序列化共用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookFieldKind {
    /// 普通字符串。
    Text,
    /// 布尔开关。
    Flag,
    /// JSON 数组文本。
    JsonArray,
    /// JSON 对象文本。
    JsonObject,
    /// 非负整数秒。
    Timeout,
}

/// handler 字段封闭集合；表单、schema 与保存校验共用本目录。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HookHandlerField {
    Command,
    Args,
    Async,
    AsyncRewake,
    Shell,
    Url,
    Headers,
    AllowedEnvVars,
    Server,
    Tool,
    Input,
    Prompt,
    Model,
    ContinueOnBlock,
    If,
    Timeout,
    StatusMessage,
}

impl HookHandlerField {
    pub const ALL: [Self; 17] = [
        Self::Command,
        Self::Args,
        Self::Async,
        Self::AsyncRewake,
        Self::Shell,
        Self::Url,
        Self::Headers,
        Self::AllowedEnvVars,
        Self::Server,
        Self::Tool,
        Self::Input,
        Self::Prompt,
        Self::Model,
        Self::ContinueOnBlock,
        Self::If,
        Self::Timeout,
        Self::StatusMessage,
    ];

    /// hooks JSON 中的键名。
    pub const fn key(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Args => "args",
            Self::Async => "async",
            Self::AsyncRewake => "asyncRewake",
            Self::Shell => "shell",
            Self::Url => "url",
            Self::Headers => "headers",
            Self::AllowedEnvVars => "allowedEnvVars",
            Self::Server => "server",
            Self::Tool => "tool",
            Self::Input => "input",
            Self::Prompt => "prompt",
            Self::Model => "model",
            Self::ContinueOnBlock => "continueOnBlock",
            Self::If => "if",
            Self::Timeout => "timeout",
            Self::StatusMessage => "statusMessage",
        }
    }

    /// 值的 JSON 形态；表单据此选择控件与写回方式。
    pub const fn kind(self) -> HookFieldKind {
        match self {
            Self::Args | Self::AllowedEnvVars => HookFieldKind::JsonArray,
            Self::Headers | Self::Input => HookFieldKind::JsonObject,
            Self::Async | Self::AsyncRewake | Self::ContinueOnBlock => HookFieldKind::Flag,
            Self::Timeout => HookFieldKind::Timeout,
            _ => HookFieldKind::Text,
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

    /// 该类型可编辑的全部字段（含通用 if/timeout/statusMessage）。
    /// 表单渲染与保存校验共用本声明，不再维护第二张适用性表。
    pub const fn fields(self) -> &'static [HookHandlerField] {
        use HookHandlerField::{
            AllowedEnvVars, Args, Async, AsyncRewake, Command, ContinueOnBlock, Headers, If, Input,
            Model, Prompt, Server, Shell, StatusMessage, Timeout, Tool, Url,
        };
        const COMMAND: [HookHandlerField; 8] = [
            Command,
            Args,
            Async,
            AsyncRewake,
            Shell,
            If,
            Timeout,
            StatusMessage,
        ];
        const HTTP: [HookHandlerField; 6] =
            [Url, Headers, AllowedEnvVars, If, Timeout, StatusMessage];
        const MCP_TOOL: [HookHandlerField; 6] = [Server, Tool, Input, If, Timeout, StatusMessage];
        const PROMPT: [HookHandlerField; 6] =
            [Prompt, Model, ContinueOnBlock, If, Timeout, StatusMessage];
        const AGENT: [HookHandlerField; 5] = [Prompt, Model, If, Timeout, StatusMessage];
        match self {
            Self::Command => &COMMAND,
            Self::Http => &HTTP,
            Self::McpTool => &MCP_TOOL,
            Self::Prompt => &PROMPT,
            Self::Agent => &AGENT,
        }
    }

    /// 保存校验与表单共用的必填字段子集。
    pub const fn required_fields(self) -> &'static [HookHandlerField] {
        use HookHandlerField::{Command, Prompt, Server, Tool, Url};
        match self {
            Self::Command => &[Command],
            Self::Http => &[Url],
            Self::McpTool => &[Server, Tool],
            Self::Prompt | Self::Agent => &[Prompt],
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

    /// 编辑器目录覆盖 featureSpec 全表，且已含未安装订阅的事件。
    #[test]
    fn claude_event_catalog_covers_the_full_feature_spec() {
        let expected = [
            "SessionStart",
            "SessionEnd",
            "Setup",
            "UserPromptSubmit",
            "UserPromptExpansion",
            "PreToolUse",
            "PermissionRequest",
            "PermissionDenied",
            "PostToolUse",
            "PostToolUseFailure",
            "PostToolBatch",
            "Notification",
            "MessageDisplay",
            "SubagentStart",
            "SubagentStop",
            "TaskCreated",
            "TaskCompleted",
            "TeammateIdle",
            "PreCompact",
            "PostCompact",
            "Stop",
            "StopFailure",
            "ConfigChange",
            "CwdChanged",
            "FileChanged",
            "InstructionsLoaded",
            "DirectoryAdded",
            "WorktreeCreate",
            "WorktreeRemove",
            "Elicitation",
            "ElicitationResult",
        ];
        assert_eq!(CLAUDE_EVENT_CATALOG, expected);
        for event in CLAUDE_EVENT_CATALOG {
            assert!(HookEvent::parse(event).is_some(), "{event} 不在事件目录");
        }
        for installed in CLAUDE_EVENTS {
            assert!(
                CLAUDE_EVENT_CATALOG.contains(&installed),
                "{installed} 必须仍在编辑器目录内"
            );
        }
    }

    /// Claude 订阅白名单与事件目录同源且顺序固定。
    #[test]
    fn claude_events_derive_from_catalog() {
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
            for field in handler.required_fields() {
                assert!(
                    handler.fields().contains(field),
                    "{handler:?} 的必填字段 {field:?} 必须同时属于可编辑字段"
                );
            }
        }
        assert_eq!(names.len(), 5);
        assert_eq!(HookHandlerType::parse("shell"), None);
    }

    /// 字段目录封闭：键名唯一、类型与控件形态同源，字段只在声明它的类型下出现。
    #[test]
    fn handler_field_catalog_is_closed_and_shared() {
        let mut keys = std::collections::HashSet::new();
        for field in HookHandlerField::ALL {
            assert!(keys.insert(field.key()), "{field:?} 键名重复");
            assert!(!field.key().is_empty());
        }
        assert_eq!(keys.len(), HookHandlerField::ALL.len());
        assert_eq!(HookHandlerField::Timeout.kind(), HookFieldKind::Timeout);
        assert_eq!(HookHandlerField::Async.kind(), HookFieldKind::Flag);
        assert_eq!(HookHandlerField::Args.kind(), HookFieldKind::JsonArray);
        assert_eq!(HookHandlerField::Input.kind(), HookFieldKind::JsonObject);
        assert_eq!(HookHandlerField::Command.kind(), HookFieldKind::Text);

        for handler in HookHandlerType::ALL {
            let mut fields = std::collections::HashSet::new();
            for field in handler.fields() {
                assert!(fields.insert(*field), "{handler:?} 字段重复");
                assert!(
                    HookHandlerField::ALL.contains(field),
                    "{handler:?} 不得声明目录外字段"
                );
            }
        }
        assert_eq!(
            HookHandlerType::Command.fields(),
            &[
                HookHandlerField::Command,
                HookHandlerField::Args,
                HookHandlerField::Async,
                HookHandlerField::AsyncRewake,
                HookHandlerField::Shell,
                HookHandlerField::If,
                HookHandlerField::Timeout,
                HookHandlerField::StatusMessage,
            ]
        );
        assert!(
            !HookHandlerType::Http
                .fields()
                .contains(&HookHandlerField::Command)
        );
        assert!(
            HookHandlerType::Agent
                .fields()
                .contains(&HookHandlerField::If)
        );
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
