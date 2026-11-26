//! 设置页 hooks 编辑域(双轨缝合)。
//!
//! 双轨不交叉的判据 = marker 归属:含 slterm-hook 命令的 matcher 组归
//! 安装器专属,编辑器只读、保存时从磁盘原样保留;编辑域限用户/第三方
//! handler。写入前跑语义校验(事件名白名单 + 事件类型必填字段矩阵),
//! 校验失败零写盘;非法 JSON / 非对象根一律拒绝编辑,绝不「修复式覆盖」。
//!
//! 编辑面是 claude 用户层 `~/.claude/settings.json`(设置页无 project 上下文;
//! user/project/local 三层完整编辑面归 AI 集成域)。数据源与安装动作归
//! `integrations`/`local`,本模块只做文档级读-改-写。

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

use crate::i18n::{Message, UiLanguage};

use super::{CLAUDE_EVENTS, DraftWriteGuard, draft_write_guard};

/// handler 矩阵单源在 `slterm_hooks`；校验与表单共用同一类型锚点。
pub(crate) use slterm_hooks::HookHandlerType;

/// 编辑器可选事件(与安装器订阅白名单同源)。
pub(crate) fn hook_events() -> &'static [&'static str] {
    &CLAUDE_EVENTS
}

/// 结构化表单字段(文本输入或布尔开关);键名与 hooks JSON 一致。
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum HandlerField {
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

impl HandlerField {
    pub(crate) const ALL: [Self; 17] = [
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

    pub(crate) const fn key(self) -> &'static str {
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

    pub(crate) const fn message(self) -> Message {
        match self {
            Self::Command => Message::SettingsAgentsFieldCommand,
            Self::Args => Message::SettingsAgentsFieldArgs,
            Self::Async => Message::SettingsAgentsFieldAsync,
            Self::AsyncRewake => Message::SettingsAgentsFieldAsyncRewake,
            Self::Shell => Message::SettingsAgentsFieldShell,
            Self::Url => Message::SettingsAgentsFieldUrl,
            Self::Headers => Message::SettingsAgentsFieldHeaders,
            Self::AllowedEnvVars => Message::SettingsAgentsFieldAllowedEnvVars,
            Self::Server => Message::SettingsAgentsFieldServer,
            Self::Tool => Message::SettingsAgentsFieldTool,
            Self::Input => Message::SettingsAgentsFieldInput,
            Self::Prompt => Message::SettingsAgentsFieldPrompt,
            Self::Model => Message::SettingsAgentsFieldModel,
            Self::ContinueOnBlock => Message::SettingsAgentsFieldContinueOnBlock,
            Self::If => Message::SettingsAgentsFieldIf,
            Self::Timeout => Message::SettingsAgentsFieldTimeout,
            Self::StatusMessage => Message::SettingsAgentsFieldStatusText,
        }
    }

    /// 布尔开关字段;其余字段为文本输入。
    pub(crate) const fn is_flag(self) -> bool {
        matches!(
            self,
            Self::Async | Self::AsyncRewake | Self::ContinueOnBlock
        )
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }
}

/// 每型专属字段(通用字段 if/timeout/statusMessage 另列)。
pub(crate) fn type_fields(kind: HookHandlerType) -> &'static [HandlerField] {
    static COMMAND: [HandlerField; 5] = [
        HandlerField::Command,
        HandlerField::Args,
        HandlerField::Async,
        HandlerField::AsyncRewake,
        HandlerField::Shell,
    ];
    static HTTP: [HandlerField; 3] = [
        HandlerField::Url,
        HandlerField::Headers,
        HandlerField::AllowedEnvVars,
    ];
    static MCP_TOOL: [HandlerField; 3] = [
        HandlerField::Server,
        HandlerField::Tool,
        HandlerField::Input,
    ];
    static PROMPT: [HandlerField; 3] = [
        HandlerField::Prompt,
        HandlerField::Model,
        HandlerField::ContinueOnBlock,
    ];
    static AGENT: [HandlerField; 2] = [HandlerField::Prompt, HandlerField::Model];
    match kind {
        HookHandlerType::Command => &COMMAND,
        HookHandlerType::Http => &HTTP,
        HookHandlerType::McpTool => &MCP_TOOL,
        HookHandlerType::Prompt => &PROMPT,
        HookHandlerType::Agent => &AGENT,
    }
}

pub(crate) const COMMON_FIELDS: [HandlerField; 3] = [
    HandlerField::If,
    HandlerField::Timeout,
    HandlerField::StatusMessage,
];

/// 字段是否适用于该 handler 型(通用字段恒适用)。
pub(crate) fn field_applies(kind: HookHandlerType, field: HandlerField) -> bool {
    COMMON_FIELDS.contains(&field) || type_fields(kind).contains(&field)
}

pub(crate) fn handler_type_message(kind: HookHandlerType) -> Message {
    match kind {
        HookHandlerType::Command => Message::SettingsAgentsTypeCommand,
        HookHandlerType::Http => Message::SettingsAgentsTypeHttp,
        HookHandlerType::McpTool => Message::SettingsAgentsTypeMcpTool,
        HookHandlerType::Prompt => Message::SettingsAgentsTypePrompt,
        HookHandlerType::Agent => Message::SettingsAgentsTypeAgent,
    }
}

/// 表单加载/序列化的技术细节包装(路径或字段标识;非用户文案主句)。
pub(crate) fn form_load_error(language: UiLanguage, detail: &str) -> String {
    language.format(Message::SettingsAgentsFormLoadFailed, &[("error", detail)])
}

/// 单个字段的显示文本;JSON 形态字段保留紧凑 JSON 文本。
pub(crate) fn load_field_text(
    handler: &Map<String, Value>,
    field: HandlerField,
    language: UiLanguage,
    path: &str,
) -> Result<String, String> {
    match field {
        HandlerField::Args
        | HandlerField::Headers
        | HandlerField::AllowedEnvVars
        | HandlerField::Input => match handler.get(field.key()) {
            Some(value) => serde_json::to_string(value).map_err(|error| {
                form_load_error(language, &format!("{path}.{}: {error}", field.key()))
            }),
            None => Ok(String::new()),
        },
        HandlerField::Timeout => match handler.get(field.key()) {
            Some(Value::Number(number)) => Ok(number.to_string()),
            Some(Value::String(text)) => Ok(text.clone()),
            Some(_) => Err(form_load_error(
                language,
                &format!("{path}.{}", field.key()),
            )),
            None => Ok(String::new()),
        },
        _ => match handler.get(field.key()) {
            Some(Value::String(text)) => Ok(text.clone()),
            Some(_) => Err(form_load_error(
                language,
                &format!("{path}.{}", field.key()),
            )),
            None => Ok(String::new()),
        },
    }
}

pub(crate) fn load_flag(
    handler: &Map<String, Value>,
    field: HandlerField,
    language: UiLanguage,
    path: &str,
) -> Result<bool, String> {
    match handler.get(field.key()) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(form_load_error(
            language,
            &format!("{path}.{}", field.key()),
        )),
        None => Ok(false),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JsonShape {
    Array,
    Object,
}

pub(crate) fn invalid_field(language: UiLanguage, field: HandlerField, error: &str) -> String {
    language.format(
        Message::SettingsAgentsFormInvalidField,
        &[("field", language.text(field.message())), ("error", error)],
    )
}

pub(crate) fn insert_string_field(object: &mut Map<String, Value>, key: &str, value: String) {
    if !value.trim().is_empty() {
        object.insert(key.to_owned(), Value::String(value));
    }
}

/// JSON 文本字段(args/headers/allowedEnvVars/input):空文本省略,非法形态拒绝。
pub(crate) fn insert_json_field(
    object: &mut Map<String, Value>,
    key: &str,
    raw: String,
    shape: JsonShape,
    field: HandlerField,
    language: UiLanguage,
) -> Result<(), String> {
    if raw.trim().is_empty() {
        return Ok(());
    }
    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| invalid_field(language, field, &error.to_string()))?;
    let matches = match shape {
        JsonShape::Array => value.is_array(),
        JsonShape::Object => value.is_object(),
    };
    if !matches {
        let message = match shape {
            JsonShape::Array => Message::SettingsAgentsFormExpectedArray,
            JsonShape::Object => Message::SettingsAgentsFormExpectedObject,
        };
        return Err(invalid_field(language, field, language.text(message)));
    }
    object.insert(key.to_owned(), value);
    Ok(())
}

/// Hooks 文档读写的封闭失败类别；UI 文案由 typed `Message` 映射，
/// 编辑域不产出可直接显示的自然语言。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HooksEditError {
    ConfigDirUnavailable,
    ConfigDirMissing,
    RootNotObject,
    HooksNotObject,
    EventNotArray(String),
    MatcherShapeInvalid(String),
    MatcherNotObject(String),
    MatcherHooksMissing(String),
    HandlerNotObject(String),
    HandlerTypeInvalid { event: String, allowed: String },
    HandlerFieldEmpty { event: String, field: &'static str },
    EventNameInvalid { event: String, allowed: String },
    JsonSyntax(String),
    InvalidUtf8,
    DuplicateKey(String),
    ExternalModified,
    LockBusy,
    BackupFailed(String),
    Io(String),
    Serialize(String),
}

impl std::fmt::Display for HooksEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigDirUnavailable => {
                formatter.write_str("claude config directory unavailable")
            }
            Self::ConfigDirMissing => formatter.write_str("claude config directory missing"),
            Self::RootNotObject => formatter.write_str("config root is not an object"),
            Self::HooksNotObject => formatter.write_str("hooks subtree is not an object"),
            Self::EventNotArray(event) => write!(formatter, "event {event} is not an array"),
            Self::MatcherShapeInvalid(event) => {
                write!(formatter, "event {event} matcher group is invalid")
            }
            Self::MatcherNotObject(event) => {
                write!(formatter, "event {event} matcher group is not an object")
            }
            Self::MatcherHooksMissing(event) => {
                write!(formatter, "event {event} matcher group lacks hooks")
            }
            Self::HandlerNotObject(event) => {
                write!(formatter, "event {event} handler is not an object")
            }
            Self::HandlerTypeInvalid { event, allowed } => {
                write!(
                    formatter,
                    "event {event} handler type invalid (allowed: {allowed})"
                )
            }
            Self::HandlerFieldEmpty { event, field } => {
                write!(formatter, "event {event} handler field {field} is empty")
            }
            Self::EventNameInvalid { event, allowed } => {
                write!(
                    formatter,
                    "event {event} is not allowed (allowed: {allowed})"
                )
            }
            Self::JsonSyntax(detail) => write!(formatter, "hooks json syntax error: {detail}"),
            Self::InvalidUtf8 => formatter.write_str("hooks file is not valid utf-8"),
            Self::DuplicateKey(key) => write!(formatter, "hooks file contains duplicate key {key}"),
            Self::ExternalModified => formatter.write_str("hooks file changed on disk"),
            Self::LockBusy => formatter.write_str("hooks file is busy"),
            Self::BackupFailed(detail) => write!(formatter, "hooks backup failed: {detail}"),
            Self::Io(detail) => write!(formatter, "hooks io failed: {detail}"),
            Self::Serialize(detail) => write!(formatter, "hooks serialize failed: {detail}"),
        }
    }
}

impl std::error::Error for HooksEditError {}

/// 一份 hooks 编辑文档:可编辑子树 + 安装器认领组只读快照。
///
/// `editable` / `owned` 都是「事件名 → matcher 组数组」对象,只在有组的
/// 事件上保留键。保存走 read-modify-write:磁盘上的认领组原样保留,非
/// 认领组整段替换为编辑结果。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HooksDocument {
    path: PathBuf,
    editable: Value,
    owned: Value,
    /// 加载时的完整文件指纹；保存前重读比较，外部修改一律拒绝。
    baseline: Option<[u8; 32]>,
}

impl HooksDocument {
    /// 生产路径:claude 用户层 settings.json。目录不存在(未装 Claude)时
    /// 拒绝进入编辑,不为用户 scaffold 配置树。
    pub(crate) fn load_user() -> Result<Self, HooksEditError> {
        let dir = super::local::claude_config_dir().ok_or(HooksEditError::ConfigDirUnavailable)?;
        if !dir.is_dir() {
            return Err(HooksEditError::ConfigDirMissing);
        }
        Self::load_at(&dir.join("settings.json"))
    }

    /// 显式路径加载(设置页注入端与测试 tempdir 假 home)。
    pub(crate) fn load_at(path: &Path) -> Result<Self, HooksEditError> {
        let (root, baseline) = read_root_with_fingerprint(path)?;
        let root = root.as_object().ok_or(HooksEditError::RootNotObject)?;
        let hooks = match root.get("hooks") {
            None => serde_json::Map::new(),
            Some(hooks) => hooks
                .as_object()
                .cloned()
                .ok_or(HooksEditError::HooksNotObject)?,
        };
        let mut editable = serde_json::Map::new();
        let mut owned = serde_json::Map::new();
        for (event, groups) in hooks {
            let groups = groups
                .as_array()
                .ok_or_else(|| HooksEditError::EventNotArray(event.clone()))?;
            let mut editable_groups = Vec::new();
            let mut owned_groups = Vec::new();
            for group in groups {
                if group_is_installer_owned(group)
                    .ok_or_else(|| HooksEditError::MatcherShapeInvalid(event.clone()))?
                {
                    owned_groups.push(group.clone());
                } else {
                    editable_groups.push(group.clone());
                }
            }
            if !editable_groups.is_empty() {
                editable.insert(event.clone(), Value::Array(editable_groups));
            }
            if !owned_groups.is_empty() {
                owned.insert(event, Value::Array(owned_groups));
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            editable: Value::Object(editable),
            owned: Value::Object(owned),
            baseline,
        })
    }

    /// 载入时的完整文件指纹；草稿宿主据此判断外部修改。
    pub(crate) fn baseline(&self) -> Option<[u8; 32]> {
        self.baseline
    }

    /// 可编辑子树的 JSON 文本(编辑器显示值;事件名 → 用户 matcher 组)。
    pub(crate) fn editable_json(&self) -> String {
        serde_json::to_string_pretty(&self.editable).unwrap_or_else(|_| "{}".to_owned())
    }

    /// 可编辑子树的结构化视图(表单加载用;只读借用)。
    pub(crate) fn editable_value(&self) -> &Value {
        &self.editable
    }

    /// 安装器认领组数量(只读展示;双轨判据的可见面)。
    pub(crate) fn owned_group_count(&self) -> usize {
        self.owned
            .as_object()
            .map(|events| {
                events
                    .values()
                    .filter_map(Value::as_array)
                    .map(Vec::len)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// 认领组按事件汇总(只读展示;顺序随 JSON map)。
    pub(crate) fn owned_events(&self) -> Vec<(String, usize)> {
        self.owned
            .as_object()
            .map(|events| {
                events
                    .iter()
                    .filter_map(|(event, groups)| Some((event.clone(), groups.as_array()?.len())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 提交编辑:解析 → 校验 → 锁内重读并比较基线指纹 → 与磁盘现状合并
    /// (认领组原样保留) → 原子写。任何失败零写盘，草稿由调用方保留。
    pub(crate) fn save_editable(&self, raw: &str) -> Result<(), HooksEditError> {
        let editable: Value = serde_json::from_str(raw)
            .map_err(|error| HooksEditError::JsonSyntax(error.to_string()))?;
        validate_editable(&editable)?;
        write_editable_at(&self.path, self.baseline, &editable)
    }

    /// 保存成功后重读(认领组可能因安装器并发自愈而变化)。
    pub(crate) fn reload(&self) -> Result<Self, HooksEditError> {
        Self::load_at(&self.path)
    }
}

/// matcher 组归属:`Some(true)` = 安装器认领(含 slterm-hook 命令);
/// `Some(false)` = 用户/第三方;`None` = 形态非法(加载/保存都拒绝)。
fn group_is_installer_owned(group: &Value) -> Option<bool> {
    let handlers = group.get("hooks")?.as_array()?;
    Some(handlers.iter().any(super::local::is_our_claude_hook))
}

/// 语义校验(纯函数):事件名白名单 + 事件类型矩阵的必填字段。
/// 未知 type、缺失/空白必填字段与形态非法一律拒绝。
fn validate_editable(hooks: &Value) -> Result<(), HooksEditError> {
    let events = hooks.as_object().ok_or(HooksEditError::HooksNotObject)?;
    for (event, groups) in events {
        if !CLAUDE_EVENTS.contains(&event.as_str()) {
            return Err(HooksEditError::EventNameInvalid {
                event: event.clone(),
                allowed: CLAUDE_EVENTS.join(" / "),
            });
        }
        let groups = groups
            .as_array()
            .ok_or_else(|| HooksEditError::EventNotArray(event.clone()))?;
        for group in groups {
            let group = group
                .as_object()
                .ok_or_else(|| HooksEditError::MatcherNotObject(event.clone()))?;
            let handlers = group
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| HooksEditError::MatcherHooksMissing(event.clone()))?;
            for handler in handlers {
                let handler = handler
                    .as_object()
                    .ok_or_else(|| HooksEditError::HandlerNotObject(event.clone()))?;
                let Some(kind) = handler
                    .get("type")
                    .and_then(Value::as_str)
                    .and_then(HookHandlerType::parse)
                else {
                    return Err(HooksEditError::HandlerTypeInvalid {
                        event: event.clone(),
                        allowed: HookHandlerType::ALL
                            .iter()
                            .map(|kind| kind.as_str())
                            .collect::<Vec<_>>()
                            .join(" / "),
                    });
                };
                for field in kind.required_fields() {
                    let value = handler
                        .get(*field)
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if value.trim().is_empty() {
                        return Err(HooksEditError::HandlerFieldEmpty {
                            event: event.clone(),
                            field,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// 根对象、完整文件指纹与原文三元组；基线是内容代次前置条件。
type RootRead = (Value, Option<[u8; 32]>, Option<String>);

/// 读根对象、完整文件指纹与原文:缺失 = 首次创建(空对象、无基线、无原文);
/// 损坏/IO 错误向上传播，不当空配置。
fn read_root_text(path: &Path) -> Result<RootRead, HooksEditError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let baseline = Some(Sha256::digest(&bytes).into());
            let text = String::from_utf8(bytes).map_err(|_| HooksEditError::InvalidUtf8)?;
            // 与草稿解析共用严格解析：重复键不得静默覆盖，否则加载结果
            // 与文件语义不一致，保存后旧组永久消失。
            let root = super::draft::parse_hooks_json(&text).map_err(|error| match error {
                super::draft::DraftParseError::InvalidUtf8 => HooksEditError::InvalidUtf8,
                super::draft::DraftParseError::InvalidJson(message) => {
                    HooksEditError::JsonSyntax(message)
                }
                super::draft::DraftParseError::DuplicateKey(key) => {
                    HooksEditError::DuplicateKey(key)
                }
            })?;
            Ok((root, baseline, Some(text)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((json!({}), None, None)),
        Err(error) => Err(HooksEditError::Io(error.to_string())),
    }
}

fn read_root_with_fingerprint(path: &Path) -> Result<(Value, Option<[u8; 32]>), HooksEditError> {
    read_root_text(path).map(|(root, baseline, _)| (root, baseline))
}

/// 读-改-写主体:锁内重读并比较基线 → 合并认领组与编辑组 →
/// 只替换 hooks 子树字节范围 → 先写上一代 .bak → 原子写。
/// hooks 以外的字段、键序与空白字节原样保留。
fn write_editable_at(
    path: &Path,
    baseline: Option<[u8; 32]>,
    editable: &Value,
) -> Result<(), HooksEditError> {
    let _lock = crate::atomic_file::try_lock(path)
        .map_err(|error| HooksEditError::Io(error.to_string()))?
        .ok_or(HooksEditError::LockBusy)?;
    let (mut root, current, original_text) = read_root_text(path)?;
    if draft_write_guard(baseline, current) == DraftWriteGuard::ExternalModified {
        return Err(HooksEditError::ExternalModified);
    }
    let object = root.as_object_mut().ok_or(HooksEditError::RootNotObject)?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or(HooksEditError::HooksNotObject)?;
    for (event, groups) in hooks.iter_mut() {
        let groups = groups
            .as_array_mut()
            .ok_or_else(|| HooksEditError::EventNotArray(event.clone()))?;
        // 安装器认领组原位保留;旧用户组整段让位给本次编辑结果。
        groups.retain(|group| group_is_installer_owned(group).unwrap_or(true));
    }
    hooks.retain(|_, groups| !groups.as_array().is_some_and(Vec::is_empty));
    let events = editable.as_object().ok_or(HooksEditError::HooksNotObject)?;
    for (event, groups) in events {
        let groups = groups
            .as_array()
            .ok_or_else(|| HooksEditError::EventNotArray(event.clone()))?;
        let current = hooks.entry(event.clone()).or_insert_with(|| json!([]));
        let current = current
            .as_array_mut()
            .ok_or_else(|| HooksEditError::EventNotArray(event.clone()))?;
        current.extend(groups.iter().cloned());
    }
    let text = replace_hooks_subtree(original_text.as_deref(), &root)?;
    if let Some(original) = &original_text {
        let backup = backup_path(path);
        crate::atomic_file::write(&backup, original.as_bytes())
            .map_err(|error| HooksEditError::BackupFailed(error.to_string()))?;
    }
    crate::atomic_file::write(path, text.as_bytes())
        .map_err(|error| HooksEditError::Io(error.to_string()))
}

/// 与 settings_store 同名的 `.bak` 上一代备份。
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// 只替换根对象中 hooks 值的字节范围；缺失时在根对象闭合前追加，
/// 其余字节（键序、空白、其他字段）一律原样保留。
fn replace_hooks_subtree(original: Option<&str>, root: &Value) -> Result<String, HooksEditError> {
    let hooks = root.get("hooks").cloned().unwrap_or_else(|| json!({}));
    let subtree = serde_json::to_string_pretty(&hooks)
        .map_err(|error| HooksEditError::Serialize(error.to_string()))?;
    let Some(original) = original else {
        return Ok(format!("{{\"hooks\":{subtree}}}\n"));
    };
    let scan = scan_root_object(original).ok_or(HooksEditError::RootNotObject)?;
    if let Some(range) = scan.hooks {
        return Ok(format!(
            "{}{}{}",
            &original[..range.start],
            subtree,
            &original[range.end..]
        ));
    }
    let separator = if scan.has_members { "," } else { "" };
    Ok(format!(
        "{}{separator}\"hooks\":{subtree}{}",
        &original[..scan.end],
        &original[scan.end..]
    ))
}

struct RootObjectScan {
    /// 根对象闭合 `}` 的字节位置。
    end: usize,
    /// hooks 值的字节范围（含两端结构）。
    hooks: Option<std::ops::Range<usize>>,
    /// 根对象是否有成员（决定追加时是否需要逗号）。
    has_members: bool,
}

/// 轻量 JSON 扫描：只为根对象定位 hooks 值范围与闭合位置，不重排或修复。
fn scan_root_object(text: &str) -> Option<RootObjectScan> {
    let bytes = text.as_bytes();
    let mut index = skip_json_whitespace(bytes, 0)?;
    if bytes.get(index) != Some(&b'{') {
        return None;
    }
    index += 1;
    let mut hooks = None;
    let mut has_members = false;
    loop {
        index = skip_json_whitespace(bytes, index)?;
        match bytes.get(index)? {
            b'}' => {
                return Some(RootObjectScan {
                    end: index,
                    hooks,
                    has_members,
                });
            }
            b',' => index += 1,
            b'"' => {
                has_members = true;
                let key_start = index;
                let key_end = scan_json_string(bytes, index)?;
                let key = &text[key_start + 1..key_end - 1];
                index = skip_json_whitespace(bytes, key_end)?;
                if bytes.get(index) != Some(&b':') {
                    return None;
                }
                index = skip_json_whitespace(bytes, index + 1)?;
                let value_start = index;
                index = scan_json_value(bytes, index)?;
                if key == "hooks" {
                    hooks = Some(value_start..index);
                }
            }
            _ => return None,
        }
    }
}

fn skip_json_whitespace(bytes: &[u8], mut index: usize) -> Option<usize> {
    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t' | b'\r' | b'\n') {
        index += 1;
    }
    (index < bytes.len()).then_some(index)
}

/// 返回字符串结束后的位置；转义跳过两个字节。
fn scan_json_string(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

fn scan_json_value(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'"' => scan_json_string(bytes, start),
        b'{' | b'[' => {
            let mut depth = 0_usize;
            let mut index = start;
            while index < bytes.len() {
                match bytes[index] {
                    b'"' => {
                        index = scan_json_string(bytes, index)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(index + 1);
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
            None
        }
        _ => {
            let mut index = start;
            while index < bytes.len()
                && !matches!(
                    bytes[index],
                    b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n'
                )
            {
                index += 1;
            }
            (index > start).then_some(index)
        }
    }
}

#[cfg(test)]
mod editor_tests {
    use super::*;

    const HELPER: &str = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        (directory, path)
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// 加载路径必须与草稿解析同样拒绝重复键：后写静默覆盖前写会让用户
    /// 看到与文件内容不一致的语义，保存后旧组被永久吞掉。
    #[test]
    fn load_rejects_duplicate_keys_instead_of_silently_overwriting() {
        let (_directory, path) = fixture();
        write(
            &path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"a"}]}]},"hooks":{"SessionStart":[]}}"#,
        );
        let error = HooksDocument::load_at(&path).unwrap_err();
        assert!(
            matches!(error, HooksEditError::DuplicateKey(ref key) if key == "hooks"),
            "实际: {error}"
        );
    }

    #[test]
    fn load_marks_marker_groups_read_only_and_keeps_user_domains_editable() {
        let (_directory, path) = fixture();
        write(
            &path,
            r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {"matcher": "user", "hooks": [{"type": "command", "command": "user-notifier"}]},
      {"hooks": [{"type": "command", "command": "C:/old/slterm-hook.exe", "args": ["claude"]}]}
    ],
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "C:/new/slterm-hook.exe", "args": ["claude"]}]}
    ]
  }
}"#,
        );
        let document = HooksDocument::load_at(&path).unwrap();
        let editable: Value = serde_json::from_str(&document.editable_json()).unwrap();
        assert_eq!(
            editable["Stop"].as_array().map(Vec::len),
            Some(1),
            "用户组留在编辑域"
        );
        assert_eq!(editable["Stop"][0]["matcher"], json!("user"));
        assert!(
            editable.get("SessionStart").is_none(),
            "纯认领事件不进编辑域"
        );
        assert_eq!(document.owned_group_count(), 2);
        let mut owned_events = document.owned_events();
        owned_events.sort();
        assert_eq!(
            owned_events,
            vec![("SessionStart".to_owned(), 1), ("Stop".to_owned(), 1)],
            "只读展示按事件汇总"
        );
    }

    #[test]
    fn save_preserves_marker_groups_byte_for_byte_and_replaces_user_groups() {
        let (_directory, path) = fixture();
        write(
            &path,
            r#"{"model":"opus","hooks":{"Stop":[
                {"matcher":"user","hooks":[{"type":"command","command":"old-user"}]},
                {"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]}
            ]}}"#,
        );
        let before = read(&path);
        let marker = before["hooks"]["Stop"][1].clone();
        let document = HooksDocument::load_at(&path).unwrap();
        let edited = json!({
            "Stop": [
                {"matcher": "user", "hooks": [
                    {"type": "command", "command": "new-user"},
                    {"type": "command", "command": "second-user", "timeout": 5}
                ]}
            ],
            "UserPromptSubmit": [
                {"hooks": [{"type": "command", "command": "prompt-user"}]}
            ]
        });
        document.save_editable(&edited.to_string()).unwrap();
        let after = read(&path);
        assert_eq!(after["model"], json!("opus"), "hooks 以外的顶层键原样保留");
        let groups = after["hooks"]["Stop"].as_array().unwrap();
        assert!(groups.contains(&marker), "marker 认领组必须逐字不动");
        let user = groups
            .iter()
            .find(|group| *group != &marker)
            .expect("编辑后的用户组必须在场");
        assert_eq!(user["hooks"][0]["command"], json!("new-user"));
        assert_eq!(user["hooks"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            after["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
            json!("prompt-user")
        );
    }

    /// 保存只替换 hooks 子树：其余字段与空白字节原样保留，并留下上一代 .bak。
    #[test]
    fn hooks_save_replaces_only_the_hooks_subtree_and_keeps_a_backup() {
        let (_directory, path) = fixture();
        let original = "{\n  \"model\":  \"opus\",\n  \"env\": {\"A\": 1},\n  \"hooks\": { \"Stop\": [] }\n}\n";
        write(&path, original);
        let document = HooksDocument::load_at(&path).unwrap();
        let edited = json!({
            "Stop": [{"hooks": [{"type": "command", "command": "new-user"}]}]
        });
        document.save_editable(&edited.to_string()).unwrap();

        let saved = std::fs::read_to_string(&path).unwrap();
        let prefix = "{\n  \"model\":  \"opus\",\n  \"env\": {\"A\": 1},\n  \"hooks\": ";
        assert!(
            saved.starts_with(prefix),
            "hooks 之前的字节必须原样保留: {saved}"
        );
        assert!(saved.ends_with("\n}\n"), "hooks 之后的字节必须原样保留");
        let parsed: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(parsed["model"], json!("opus"));
        assert_eq!(parsed["env"]["A"], json!(1));
        assert_eq!(
            parsed["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("new-user")
        );
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("settings.json.bak")).unwrap(),
            original,
            "保存前必须留下上一代有效备份"
        );
    }

    /// hooks 缺失时在根对象末尾追加子树，不改动既有字段与结尾字节。
    #[test]
    fn hooks_save_appends_missing_subtree_without_rewriting_other_bytes() {
        let (_directory, path) = fixture();
        let original = "{\n  \"model\": \"opus\"\n}\n";
        write(&path, original);
        let document = HooksDocument::load_at(&path).unwrap();
        let edited = json!({
            "Stop": [{"hooks": [{"type": "command", "command": "added"}]}]
        });
        document.save_editable(&edited.to_string()).unwrap();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            saved.starts_with("{\n  \"model\": \"opus\"\n"),
            "既有字段字节不得重写: {saved}"
        );
        assert!(saved.contains("\"hooks\":"), "缺失的 hooks 必须被追加");
        assert!(saved.ends_with("}\n"));
        let parsed: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(parsed["model"], json!("opus"));
        assert_eq!(
            parsed["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("added")
        );
    }

    /// 备份失败必须保留原文件与草稿，不得继续写入主文件。
    #[test]
    fn hooks_save_backup_failure_keeps_original_untouched() {
        let (_directory, path) = fixture();
        let original = "{\"model\":\"opus\",\"hooks\":{}}\n";
        write(&path, original);
        let document = HooksDocument::load_at(&path).unwrap();
        std::fs::create_dir(path.with_file_name("settings.json.bak")).unwrap();

        let error = document
            .save_editable(r#"{"Stop":[{"hooks":[{"type":"command","command":"x"}]}]}"#)
            .unwrap_err();
        assert!(
            matches!(error, HooksEditError::BackupFailed(_)),
            "错误必须是备份失败类别: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original,
            "备份失败不得覆盖原文件"
        );
    }

    /// 外部修改含删除后，旧草稿保存必须拒绝且文件原样保留；重新加载才可保存。
    #[test]
    fn hooks_external_modification_guard_blocks_stale_save() {
        let (_directory, path) = fixture();
        write(&path, r#"{"hooks":{}}"#);
        let document = HooksDocument::load_at(&path).unwrap();
        let external = r#"{"model":"external","hooks":{"Stop":[{"hooks":[{"type":"command","command":"external"}]}]}}"#;
        write(&path, external);

        let error = document
            .save_editable(r#"{"Stop":[{"hooks":[{"type":"command","command":"stale"}]}]}"#)
            .unwrap_err();
        assert!(
            matches!(error, HooksEditError::ExternalModified),
            "错误必须是外部修改类别: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            external,
            "外部内容必须原样保留"
        );

        std::fs::remove_file(&path).unwrap();
        let error = document
            .save_editable(r#"{"Stop":[{"hooks":[{"type":"command","command":"stale"}]}]}"#)
            .unwrap_err();
        assert!(
            matches!(error, HooksEditError::ExternalModified),
            "外部删除同样拒绝: {error}"
        );
        assert!(!path.exists(), "被外部删除的文件不得被静默重建");

        write(&path, external);
        let reloaded = HooksDocument::load_at(&path).unwrap();
        reloaded
            .save_editable(r#"{"Stop":[{"hooks":[{"type":"command","command":"fresh"}]}]}"#)
            .unwrap();
        assert_eq!(read(&path)["model"], json!("external"), "其他顶层键保留");
    }

    #[test]
    fn save_rejects_unknown_events_and_invalid_handlers_without_writing() {
        let (_directory, path) = fixture();
        write(&path, r#"{"hooks":{}}"#);
        let before = std::fs::read(&path).unwrap();
        let document = HooksDocument::load_at(&path).unwrap();
        type ErrorMatcher = fn(&HooksEditError) -> bool;
        let cases: [(Value, ErrorMatcher); 6] = [
            (
                json!({"NotAnEvent": [{"hooks": [{"type": "command", "command": "x"}]}]}),
                |error| matches!(error, HooksEditError::EventNameInvalid { event, .. } if event == "NotAnEvent"),
            ),
            (
                json!({"Stop": [{"hooks": [{"type": "webhook", "url": "https://x"}]}]}),
                |error| matches!(error, HooksEditError::HandlerTypeInvalid { .. }),
            ),
            (
                json!({"Stop": [{"hooks": [{"type": "command", "command": "   "}]}]}),
                |error| matches!(error, HooksEditError::HandlerFieldEmpty { field, .. } if *field == "command"),
            ),
            (json!({"Stop": [{"hooks": [{"command": "x"}]}]}), |error| {
                matches!(error, HooksEditError::HandlerTypeInvalid { .. })
            }),
            (
                json!({"Stop": [{"hooks": [{"type": "command"}]}]}),
                |error| matches!(error, HooksEditError::HandlerFieldEmpty { field, .. } if *field == "command"),
            ),
            (
                json!({"Stop": {"hooks": []}}),
                |error| matches!(error, HooksEditError::EventNotArray(event) if event == "Stop"),
            ),
        ];
        for (invalid, expected) in cases {
            let error = document.save_editable(&invalid.to_string()).unwrap_err();
            assert!(expected(&error), "错误类别不符: {error}");
            assert_eq!(std::fs::read(&path).unwrap(), before, "校验失败不得落盘");
        }
    }

    /// 事件类型各一条合法 handler 必须通过校验(必填字段齐全)。
    #[test]
    fn every_handler_type_passes_validation_with_required_fields() {
        for (kind, handler) in [
            (
                "command",
                json!({
                    "type": "command",
                    "command": "notify",
                    "args": ["claude"],
                    "async": true,
                    "asyncRewake": true,
                    "shell": "pwsh"
                }),
            ),
            (
                "http",
                json!({
                    "type": "http",
                    "url": "https://example.test/hook",
                    "headers": {"Authorization": "Bearer sk-test"},
                    "allowedEnvVars": ["TOKEN"]
                }),
            ),
            (
                "mcp_tool",
                json!({
                    "type": "mcp_tool",
                    "server": "filesystem",
                    "tool": "read_file",
                    "input": {"path": "C:/tmp/x"}
                }),
            ),
            (
                "prompt",
                json!({
                    "type": "prompt",
                    "prompt": "Summarize the turn",
                    "model": "haiku",
                    "continueOnBlock": true
                }),
            ),
            (
                "agent",
                json!({
                    "type": "agent",
                    "prompt": "Review the diff",
                    "model": "sonnet"
                }),
            ),
        ] {
            let hooks = json!({
                "Stop": [{"matcher": "user", "hooks": [handler]}]
            });
            validate_editable(&hooks).unwrap_or_else(|error| panic!("{kind}: {error}"));
        }
    }

    /// 每型缺少必填字段(或必填为空白)必须被拒。
    #[test]
    fn every_handler_type_rejects_missing_required_fields() {
        for (handler, field) in [
            (json!({"type": "command"}), "command"),
            (json!({"type": "command", "command": "   "}), "command"),
            (json!({"type": "http"}), "url"),
            (json!({"type": "http", "url": ""}), "url"),
            (json!({"type": "mcp_tool", "server": "filesystem"}), "tool"),
            (json!({"type": "mcp_tool", "tool": "read_file"}), "server"),
            (json!({"type": "prompt"}), "prompt"),
            (json!({"type": "agent", "prompt": " "}), "prompt"),
        ] {
            let hooks = json!({"Stop": [{"hooks": [handler]}]});
            let error = validate_editable(&hooks).unwrap_err();
            assert!(
                matches!(&error, HooksEditError::HandlerFieldEmpty { field: empty, .. } if *empty == field),
                "错误必须点明缺失字段 {field}: {error}"
            );
        }
    }

    /// 未知 type(含非字符串 type)一律拒绝。
    #[test]
    fn unknown_handler_types_are_rejected() {
        for handler in [
            json!({"type": "webhook", "url": "https://x"}),
            json!({"type": "COMMAND", "command": "x"}),
            json!({"type": 7, "command": "x"}),
        ] {
            let hooks = json!({"Stop": [{"hooks": [handler]}]});
            let error = validate_editable(&hooks).unwrap_err();
            assert!(
                matches!(error, HooksEditError::HandlerTypeInvalid { .. }),
                "{error}"
            );
        }
    }

    #[test]
    fn corrupted_documents_are_never_repaired_by_the_editor() {
        let (_directory, path) = fixture();
        write(&path, "broken JSON");
        let error = HooksDocument::load_at(&path).unwrap_err();
        assert!(
            matches!(error, HooksEditError::JsonSyntax(_)),
            "损坏 JSON 必须映射语法错误类别: {error}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken JSON");
        write(&path, r#"["not", "an", "object"]"#);
        assert!(HooksDocument::load_at(&path).is_err());
    }
}

#[cfg(test)]
mod dual_track_tests {
    use super::*;
    use slterm_settings::AgentHook;

    const HELPER: &str = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";

    /// 双轨联调:设置段写盘 → 安装权威读决策 → 接入/卸载 → inspect 读回
    /// 三态;用户组始终保留,marker 组由安装器独占。
    #[test]
    fn settings_segment_drives_install_and_uninstall_and_inspect_reads_back() {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let claude_path = claude_dir.join("settings.json");
        std::fs::write(
            &claude_path,
            r#"{"hooks":{"Stop":[{"matcher":"user","hooks":[{"type":"command","command":"user-notifier"}]}]}}"#,
        )
        .unwrap();
        let settings_path = temp.path().join("settings.json");

        // 轨一:预置偏好(显式开启)经设置写通道落盘。
        crate::settings_store::save_settings_blocking_at(
            &settings_path,
            &[(
                AgentHook::Claude.settings_key(),
                slterm_settings::json::Value::Bool(true),
            )],
        )
        .unwrap();
        let loaded = crate::settings_store::load_settings_at(&settings_path).unwrap();
        let enabled = AgentHook::Claude.enabled(&loaded.data);
        assert!(enabled, "预置偏好必须可被安装权威读回");
        super::super::local::settings::apply_claude_at(&claude_dir, HELPER, enabled).unwrap();

        // inspect 三态:installed + current,且 helper 在位。
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(state.installed && !state.needs_repair && !state.helper_missing);
        assert_eq!(state.config_path.as_deref(), Some(claude_path.as_path()));
        // helper 缺失态独立可见(与 installed 双事实分离)。
        let missing = super::super::local::settings::inspect_claude_at(&claude_dir, None);
        assert!(missing.helper_missing && missing.installed);

        // 用户组保留;认领组由安装器维护。
        let root: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("user-notifier")
        );
        let installed = HooksDocument::load_at(&claude_path).unwrap();
        assert!(installed.owned_group_count() >= CLAUDE_EVENTS.len());

        // 轨一:显式关闭 → 卸载路径只移除认领组。
        crate::settings_store::save_settings_blocking_at(
            &settings_path,
            &[(
                AgentHook::Claude.settings_key(),
                slterm_settings::json::Value::Bool(false),
            )],
        )
        .unwrap();
        let loaded = crate::settings_store::load_settings_at(&settings_path).unwrap();
        let enabled = AgentHook::Claude.enabled(&loaded.data);
        assert!(!enabled);
        super::super::local::settings::apply_claude_at(&claude_dir, HELPER, enabled).unwrap();
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(!state.installed);
        let root: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["Stop"],
            json!([{"matcher":"user","hooks":[{"type":"command","command":"user-notifier"}]}]),
            "卸载必须恢复 marker 之前的样子"
        );
        assert_eq!(
            HooksDocument::load_at(&claude_path)
                .unwrap()
                .owned_group_count(),
            0
        );
    }

    /// 需修复态:认领组存在但命令是旧 helper 路径时,needs_repair = true;
    /// 编辑器保存不会顺带「修好」它(安装器专属)。
    #[test]
    fn stale_helper_path_reports_needs_repair_and_editor_leaves_it_alone() {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let claude_path = claude_dir.join("settings.json");
        std::fs::write(
            &claude_path,
            r#"{"hooks":{"Stop":[
                {"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]},
                {"hooks":[{"type":"command","command":"user-notifier"}]}
            ]}}"#,
        )
        .unwrap();
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(state.installed && state.needs_repair);
        let before: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        let document = HooksDocument::load_at(&claude_path).unwrap();
        document.save_editable(&document.editable_json()).unwrap();
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            after["hooks"]["Stop"][0], before["hooks"]["Stop"][0],
            "编辑器不得触碰 marker 组(修复归安装器)"
        );
    }
}
