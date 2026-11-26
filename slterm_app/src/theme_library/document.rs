//! 版本化主题文档与 JSON 适配层。
//!
//! 「设置层管值、应用层管格式」:终端/主题语义权威在 `slterm-settings`;
//! 本模块只拥有 JSON 信封——schema 版本、有界尺寸、未知字段保留。
//! 未知字段留在值树里,未来 schema 修订仍可取回;已知字段的标量范围
//! 校验统一委托 `ThemeDefinition::validate`。
//!
//! 仅暗色:上游外观字段与亮暗分支不迁,信封没有 `appearance` 键。

use std::fmt;

use serde_json::{Map, Value, json};
use slterm_settings::{Rgb8, Rgba8, ThemeDefinition, ThemeName};

/// 当前主题文档 schema 版本;不匹配即拒收(升级走新版本号)。
pub const SCHEMA_VERSION: u64 = 1;
/// 单个主题文档上限(契约数值):超限不解析。
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024;

const TERMINAL_COLORS: &[&str] = &[
    "background",
    "foreground",
    "cursor",
    "cursor_text",
    "cursor_stroke",
    "selection_background",
    "selection_foreground",
];
const UI_COLORS: &[&str] = &[
    "background",
    "sidebar",
    "foreground",
    "muted",
    "accent",
    "border",
    "selected",
    "success",
    "warning",
    "danger",
    "frame",
    "info",
];

/// 一个已校验的主题文档(值树 + 身份/修订元数据)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeDocument {
    value: Value,
}

/// 文档解析/校验错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    Json(String),
    TooLarge { bytes: usize },
    Invalid(String),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(f, "invalid theme JSON: {message}"),
            Self::TooLarge { bytes } => {
                write!(
                    f,
                    "theme document is {bytes} bytes; limit is {MAX_DOCUMENT_BYTES}"
                )
            }
            Self::Invalid(message) => write!(f, "invalid theme document: {message}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl ThemeDocument {
    /// 有界字节入口:超限即拒,不做部分解析。
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DocumentError> {
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge { bytes: bytes.len() });
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|error| DocumentError::Json(error.to_string()))?;
        Self::from_value(value)
    }

    /// 值树入口:信封校验 → 规范化 → 运行时定义校验。
    pub fn from_value(value: Value) -> Result<Self, DocumentError> {
        let mut value = value;
        validate_and_normalize(&mut value)?;
        let bytes =
            serde_json::to_vec(&value).map_err(|error| DocumentError::Json(error.to_string()))?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge { bytes: bytes.len() });
        }
        let document = Self { value };
        // 标量范围与派生语义由设置层值模型裁决;JSON 层只守信封与
        // 无损颜色表示。
        document.definition()?;
        Ok(document)
    }

    pub fn to_value(&self) -> Value {
        self.value.clone()
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, DocumentError> {
        let bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| DocumentError::Json(error.to_string()))?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge { bytes: bytes.len() });
        }
        Ok(bytes)
    }

    pub fn name(&self) -> &str {
        self.value.get("name").and_then(Value::as_str).unwrap_or("")
    }

    pub fn id(&self) -> Option<&str> {
        self.value.get("id").and_then(Value::as_str)
    }

    pub fn revision(&self) -> u64 {
        self.value
            .get("revision")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    }

    pub fn is_builtin(&self) -> bool {
        self.value
            .get("builtin")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// 值树 → 运行时值模型。返回的定义自含全部字符串与数组,渲染可长期
    /// 持有,不必回读 JSON。
    pub fn definition(&self) -> Result<ThemeDefinition, DocumentError> {
        let base = self
            .value
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("base"))
            .and_then(Value::as_str)
            .and_then(ThemeName::from_prompt_name)
            .or_else(|| ThemeName::from_prompt_name(self.name()))
            .unwrap_or_default();
        let mut definition = ThemeDefinition::from_builtin(base);
        definition.name = self.name().to_owned();

        let terminal = object(&self.value, "terminal")?;
        definition.terminal.background = color(terminal, "background")?;
        definition.terminal.foreground = color(terminal, "foreground")?;
        definition.terminal.cursor = optional_color(terminal, "cursor")?;
        definition.terminal.cursor_text = optional_color(terminal, "cursor_text")?;
        definition.terminal.cursor_stroke = optional_color(terminal, "cursor_stroke")?;
        definition.terminal.selection_background =
            optional_color(terminal, "selection_background")?;
        definition.terminal.selection_foreground =
            optional_color(terminal, "selection_foreground")?;

        let palette = terminal
            .get("palette")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DocumentError::Invalid("terminal.palette must be an array".to_owned())
            })?;
        for (index, value) in palette.iter().enumerate() {
            definition.terminal.ansi[index] = parse_hex_rgb(value.as_str().ok_or_else(|| {
                DocumentError::Invalid(format!("terminal.palette[{index}] must be a color"))
            })?)
            .ok_or_else(|| {
                DocumentError::Invalid(format!("terminal.palette[{index}] must be a color"))
            })?;
        }

        let ui = object(&self.value, "ui")?;
        definition.ui.derive = ui.get("derive").and_then(Value::as_bool).unwrap_or(true);
        definition.ui.background = color(ui, "background")?;
        definition.ui.shell = color(ui, "sidebar")?;
        definition.ui.foreground = color(ui, "foreground")?;
        definition.ui.muted = color(ui, "muted")?;
        definition.ui.accent = color(ui, "accent")?;
        definition.ui.selection = rgba(ui, "selected")?;
        definition.ui.line = rgba(ui, "border")?;
        definition.ui.frame =
            optional_color(ui, "frame")?.unwrap_or(rgb_from_rgba(definition.ui.line));
        definition.ui.error = color(ui, "danger")?;
        definition.ui.success = color(ui, "success")?;
        definition.ui.warning = color(ui, "warning")?;
        definition.ui.info = optional_color(ui, "info")?.unwrap_or(definition.terminal.ansi[14]);

        definition
            .validate()
            .map_err(|error| DocumentError::Invalid(error.to_string()))?;
        Ok(definition)
    }

    /// 面向「解析」与「运行时解析」消费方的别名。
    pub fn resolve(&self) -> Result<ThemeDefinition, DocumentError> {
        self.definition()
    }

    /// 用新定义替换已知字段;未知厂商字段与文档身份/修订保持在信封里。
    pub fn with_definition(&self, definition: &ThemeDefinition) -> Result<Self, DocumentError> {
        let replacement = from_definition(definition)?;
        let mut value = self.value.clone();
        for key in ["name", "terminal", "ui"] {
            if let Some(next) = replacement.value.get(key) {
                if let Some(current) = value.get_mut(key) {
                    merge_value(current, next);
                } else {
                    value[key] = next.clone();
                }
            }
        }
        if let Some(base) = replacement
            .value
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("base"))
        {
            if !value.get("metadata").is_some_and(Value::is_object) {
                value["metadata"] = json!({});
            }
            value["metadata"]["base"] = base.clone();
        }
        Self::from_value(value)
    }

    pub fn with_id_and_revision(&self, id: impl Into<String>, revision: u64) -> Self {
        let mut value = self.value.clone();
        if let Some(object) = value.as_object_mut() {
            object.insert("id".to_owned(), Value::String(id.into()));
            object.insert("revision".to_owned(), Value::Number(revision.into()));
            object.insert("builtin".to_owned(), Value::Bool(false));
        }
        Self { value }
    }

    pub fn with_name(&self, name: impl Into<String>) -> Result<Self, DocumentError> {
        let mut value = self.value.clone();
        value["name"] = Value::String(name.into());
        Self::from_value(value)
    }

    /// 首次保存恒 fork 的载体:复制定义与未知字段,换身份/名字并记下
    /// 基于哪份文档;修订号从 0 起(由 store 落盘时分配 1)。
    #[allow(dead_code)] // 消费点 = 主题库 fork_builtin 与编辑器保存路径的替代入口。
    pub fn fork(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self, DocumentError> {
        let mut value = self.value.clone();
        if let Some(object) = value.as_object_mut() {
            object.insert("id".to_owned(), Value::String(id.into()));
            object.insert("revision".to_owned(), Value::Number(0.into()));
            object.insert("builtin".to_owned(), Value::Bool(false));
            object.insert("name".to_owned(), Value::String(name.into()));
            object
                .entry("metadata".to_owned())
                .or_insert_with(|| json!({}));
            if let Some(metadata) = object.get_mut("metadata").and_then(Value::as_object_mut) {
                metadata.insert("based_on".to_owned(), json!(self.name()));
            }
        }
        Self::from_value(value)
    }

    /// 取某个区段的颜色文本值。`section` = `terminal` / `ui`。
    #[allow(dead_code)] // 消费点 = 取色器/导出预览按区段读值(测试已用)。
    pub fn color(&self, section: &str, key: &str) -> Option<Rgb8> {
        self.value
            .get(section)
            .and_then(Value::as_object)
            .and_then(|object| object.get(key))
            .and_then(Value::as_str)
            .and_then(|value| {
                parse_hex_rgb(value).or_else(|| parse_hex_rgba(value).map(rgb_from_rgba))
            })
    }

    /// ANSI 0..15 镜像(文档契约恰好 16 项)。
    #[allow(dead_code)] // 消费点 = 主题选择器色块预览(测试已用)。
    pub fn palette(&self) -> Option<[[u8; 3]; 16]> {
        let array = self.value.get("terminal")?.get("palette")?.as_array()?;
        if array.len() != 16 {
            return None;
        }
        let mut output = [[0; 3]; 16];
        for (index, value) in array.iter().enumerate() {
            output[index] = parse_hex_rgb(value.as_str()?)?;
        }
        Some(output)
    }
}

/// 目录内全部内置主题的文档形态(选择器/template 列表消费)。
pub fn builtin_documents() -> Vec<ThemeDocument> {
    ThemeName::BUILTIN
        .into_iter()
        .filter_map(|theme| builtin_document(theme).ok())
        .collect()
}

/// 内置主题的种子文档:身份 = `builtin-<slug>`,只读。
pub fn builtin_document(theme: ThemeName) -> Result<ThemeDocument, DocumentError> {
    let definition = ThemeDefinition::from_builtin(theme);
    let mut document = from_definition(&definition)?;
    let mut value = document.to_value();
    value["id"] = Value::String(format!("builtin-{}", slug(theme.prompt_name())));
    value["builtin"] = Value::Bool(true);
    value["revision"] = Value::Number(0.into());
    value["metadata"]["origin"] = Value::String("builtin".to_owned());
    value["metadata"]["base"] = Value::String(theme.prompt_name().to_owned());
    document = ThemeDocument::from_value(value)?;
    Ok(document)
}

/// 外部终端色 → 完整文档种子:其余值取默认主题,背景/前景逐字节显式。
/// 仅暗色定位:不推断亮暗外观。
#[allow(dead_code)] // 消费点 = 外部终端配色导入入口(测试已用)。
pub fn seed_document(
    name: &str,
    background: Rgb8,
    foreground: Rgb8,
) -> Result<ThemeDocument, DocumentError> {
    let mut definition = ThemeDefinition::from_builtin(ThemeName::default());
    definition.name = name.to_owned();
    definition.terminal.background = background;
    definition.terminal.foreground = foreground;
    from_definition(&definition)
}

/// 运行时定义 → 完整信封(未知字段由调用方经 `with_definition` 保留)。
pub fn from_definition(definition: &ThemeDefinition) -> Result<ThemeDocument, DocumentError> {
    let terminal = &definition.terminal;
    let ui = &definition.ui;
    let palette = terminal
        .ansi
        .iter()
        .copied()
        .map(format_hex_rgb)
        .collect::<Vec<_>>();
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "revision": 0,
        "builtin": false,
        "name": definition.name.as_str(),
        "terminal": {
            "background": format_hex_rgb(terminal.background),
            "foreground": format_hex_rgb(terminal.foreground),
            "cursor": terminal.cursor.map(format_hex_rgb),
            "cursor_text": terminal.cursor_text.map(format_hex_rgb),
            "cursor_stroke": terminal.cursor_stroke.map(format_hex_rgb),
            "selection_background": terminal.selection_background.map(format_hex_rgb),
            "selection_foreground": terminal.selection_foreground.map(format_hex_rgb),
            "palette": palette,
        },
        "ui": {
            "derive": ui.derive,
            "background": format_hex_rgb(ui.background),
            "sidebar": format_hex_rgb(ui.shell),
            "foreground": format_hex_rgb(ui.foreground),
            "muted": format_hex_rgb(ui.muted),
            "accent": format_hex_rgb(ui.accent),
            "border": format_hex_rgba(ui.line),
            "selected": format_hex_rgba(ui.selection),
            "frame": format_hex_rgb(ui.frame),
            "success": format_hex_rgb(ui.success),
            "warning": format_hex_rgb(ui.warning),
            "danger": format_hex_rgb(ui.error),
            "info": format_hex_rgb(ui.info),
        },
        "metadata": { "base": definition.base.prompt_name() },
        "extensions": {},
    });
    ThemeDocument::from_value(value)
}

fn validate_and_normalize(value: &mut Value) -> Result<(), DocumentError> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| DocumentError::Invalid("root must be an object".to_owned()))?;
    reject_reserved_keys(root)?;
    reject_unknown_fields(
        root,
        &[
            "schema_version",
            "revision",
            "builtin",
            "name",
            "id",
            "terminal",
            "ui",
            "metadata",
            "extensions",
        ],
        "root",
    )?;
    if root.get("schema_version").and_then(Value::as_u64) != Some(SCHEMA_VERSION) {
        return Err(DocumentError::Invalid(format!(
            "schema_version must be {SCHEMA_VERSION}"
        )));
    }
    let name = root
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| missing("name"))?;
    validate_name(name)?;
    if let Some(id) = root.get("id").and_then(Value::as_str) {
        validate_id(id)?;
    } else if root.contains_key("id") {
        return Err(DocumentError::Invalid("id must be a string".to_owned()));
    }
    if root.contains_key("revision") && root.get("revision").and_then(Value::as_u64).is_none() {
        return Err(DocumentError::Invalid(
            "revision must be a non-negative integer".to_owned(),
        ));
    }
    validate_terminal(
        root.get_mut("terminal")
            .ok_or_else(|| missing("terminal"))?,
    )?;
    validate_ui(root.get_mut("ui").ok_or_else(|| missing("ui"))?)?;
    for key in ["metadata", "extensions"] {
        if let Some(value) = root.get(key)
            && !value.is_object()
        {
            return Err(DocumentError::Invalid(format!("{key} must be an object")));
        }
    }
    Ok(())
}

fn validate_terminal(value: &mut Value) -> Result<(), DocumentError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| DocumentError::Invalid("terminal must be an object".to_owned()))?;
    reject_reserved_keys(object)?;
    for (key, _) in object.iter() {
        if !TERMINAL_COLORS.contains(&key.as_str()) && key != "palette" {
            return Err(DocumentError::Invalid(format!(
                "terminal.{key} is not a known field"
            )));
        }
    }
    for key in TERMINAL_COLORS {
        if object.get(*key).is_some_and(|value| !value.is_null()) {
            normalize_color(object, key)?;
        } else if matches!(*key, "background" | "foreground") {
            return Err(missing(&format!("terminal.{key}")));
        }
    }
    let palette = object
        .get_mut("palette")
        .ok_or_else(|| missing("terminal.palette"))?;
    let values = palette
        .as_array_mut()
        .ok_or_else(|| DocumentError::Invalid("terminal.palette must be an array".to_owned()))?;
    if values.len() != 16 {
        return Err(DocumentError::Invalid(
            "terminal.palette must contain exactly 16 colors".to_owned(),
        ));
    }
    for (index, value) in values.iter_mut().enumerate() {
        normalize_color_value(value, &format!("terminal.palette[{index}]"), false)?;
    }
    Ok(())
}

fn validate_ui(value: &mut Value) -> Result<(), DocumentError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| DocumentError::Invalid("ui must be an object".to_owned()))?;
    reject_reserved_keys(object)?;
    // 未知 UI 槽必须拒绝：不生成部分主题，也不静默忽略未来字段拼写错误。
    for (key, _) in object.iter() {
        if key != "derive" && !UI_COLORS.contains(&key.as_str()) {
            return Err(DocumentError::Invalid(format!(
                "ui.{key} is not a known slot"
            )));
        }
    }
    if object.get("derive").and_then(Value::as_bool).is_none() {
        return Err(DocumentError::Invalid(
            "ui.derive must be boolean".to_owned(),
        ));
    }
    for key in UI_COLORS {
        if object.get(*key).is_some_and(|value| !value.is_null()) {
            normalize_color_value(
                object.get_mut(*key).expect("checked above"),
                &format!("ui.{key}"),
                matches!(*key, "selected" | "border"),
            )?;
        }
    }
    for key in [
        "background",
        "sidebar",
        "foreground",
        "muted",
        "accent",
        "selected",
        "border",
        "success",
        "warning",
        "danger",
    ] {
        if !object.contains_key(key) {
            return Err(missing(&format!("ui.{key}")));
        }
    }
    Ok(())
}

fn normalize_color(object: &mut Map<String, Value>, key: &str) -> Result<(), DocumentError> {
    let value = object.get_mut(key).ok_or_else(|| missing(key))?;
    normalize_color_value(value, key, false)
}

fn normalize_color_value(value: &mut Value, path: &str, alpha: bool) -> Result<(), DocumentError> {
    let text = value
        .as_str()
        .ok_or_else(|| DocumentError::Invalid(format!("{path} must be a #RRGGBB color")))?;
    if alpha {
        let color = parse_hex_rgba(text).ok_or_else(|| {
            DocumentError::Invalid(format!("{path} must be a #RRGGBB or #RRGGBBAA color"))
        })?;
        *value = Value::String(format_hex_rgba(color));
    } else {
        let color = parse_hex_rgb(text)
            .ok_or_else(|| DocumentError::Invalid(format!("{path} must be a #RRGGBB color")))?;
        *value = Value::String(format_hex_rgb(color));
    }
    Ok(())
}

/// 对象递归合并:替换已知字段、保留替换侧未出现的未知字段。
fn merge_value(current: &mut Value, replacement: &Value) {
    match (current, replacement) {
        (Value::Object(current), Value::Object(replacement)) => {
            for (key, value) in replacement {
                if let Some(current) = current.get_mut(key) {
                    merge_value(current, value);
                } else {
                    current.insert(key.clone(), value.clone());
                }
            }
        }
        (current, replacement) => *current = replacement.clone(),
    }
}

fn required_bool(object: &Map<String, Value>, key: &str) -> Result<bool, DocumentError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| DocumentError::Invalid(format!("{key} must be boolean")))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<&'a str, DocumentError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| DocumentError::Invalid(format!("{key} must be a string")))?;
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(DocumentError::Invalid(format!(
            "{key} must be 1..{max} characters without controls"
        )));
    }
    Ok(value)
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => {
            optional_string_with_limit(object, key, 256).map(|value| value.map(str::to_owned))
        }
    }
}

fn optional_string_with_limit<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<&'a str>, DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let value = value
                .as_str()
                .ok_or_else(|| DocumentError::Invalid(format!("{key} must be a string or null")))?;
            if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(DocumentError::Invalid(format!(
                    "{key} must be 1..{max} characters without controls"
                )));
            }
            Ok(Some(value))
        }
    }
}

fn bounded_number(
    object: &Map<String, Value>,
    key: &str,
    min: f64,
    max: f64,
) -> Result<(), DocumentError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| DocumentError::Invalid(format!("{key} must be a number")))?;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(DocumentError::Invalid(format!(
            "{key} must be in {min}..={max}"
        )));
    }
    Ok(())
}

fn optional_number(object: &Map<String, Value>, key: &str) -> Result<Option<f64>, DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => number(object, key).map(Some),
    }
}

fn optional_bounded_number(
    object: &Map<String, Value>,
    key: &str,
    min: f64,
    max: f64,
) -> Result<(), DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(()),
        Some(_) => bounded_number(object, key, min, max),
    }
}

fn optional_bounded_integer(
    object: &Map<String, Value>,
    key: &str,
    min: u64,
    max: u64,
) -> Result<(), DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(()),
        Some(value) => {
            let number = value.as_u64().ok_or_else(|| {
                DocumentError::Invalid(format!("{key} must be an integer or null"))
            })?;
            if !(min..=max).contains(&number) {
                return Err(DocumentError::Invalid(format!(
                    "{key} must be in {min}..={max}"
                )));
            }
            Ok(())
        }
    }
}

fn validate_name(name: &str) -> Result<(), DocumentError> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(DocumentError::Invalid(
            "name must be 1..128 characters without controls".to_owned(),
        ));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), DocumentError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(DocumentError::Invalid(
            "id contains unsupported characters".to_owned(),
        ));
    }
    Ok(())
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    known: &[&str],
    section: &str,
) -> Result<(), DocumentError> {
    if let Some(key) = object.keys().find(|key| !known.contains(&key.as_str())) {
        return Err(DocumentError::Invalid(format!(
            "{section}.{key} is not a known field"
        )));
    }
    Ok(())
}

fn reject_reserved_keys(object: &Map<String, Value>) -> Result<(), DocumentError> {
    if let Some(key) = object
        .keys()
        .find(|key| matches!(key.as_str(), "__proto__" | "constructor" | "prototype"))
    {
        return Err(DocumentError::Invalid(format!(
            "reserved key {key} is not allowed"
        )));
    }
    Ok(())
}

fn missing(key: &str) -> DocumentError {
    DocumentError::Invalid(format!("missing {key}"))
}

/// 主题文档颜色文本解析(`#rgb` / `#rrggbb`)。
pub fn parse_hex_rgb(value: &str) -> Option<Rgb8> {
    let text = value.strip_prefix('#')?;
    let text = match text.len() {
        3 => text
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>(),
        6 => text.to_owned(),
        _ => return None,
    };
    if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
    ])
}

/// 颜色文本解析(支持 alpha:`#rgba` / `#rrggbbaa`)。
pub fn parse_hex_rgba(value: &str) -> Option<Rgba8> {
    let text = value.strip_prefix('#')?;
    let text = match text.len() {
        4 => text
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>(),
        6 => format!("{text}ff"),
        8 => text.to_owned(),
        _ => return None,
    };
    if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
        u8::from_str_radix(&text[6..8], 16).ok()?,
    ])
}

pub fn format_hex_rgb(value: Rgb8) -> String {
    format!("#{:02x}{:02x}{:02x}", value[0], value[1], value[2])
}

/// alpha = 255 时省略 alpha 段(与 `#RRGGBB` 等价)。
pub fn format_hex_rgba(value: Rgba8) -> String {
    if value[3] == 255 {
        format_hex_rgb([value[0], value[1], value[2]])
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            value[0], value[1], value[2], value[3]
        )
    }
}

fn object<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>, DocumentError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| DocumentError::Invalid(format!("missing {key} object")))
}

fn color(object: &Map<String, Value>, key: &str) -> Result<Rgb8, DocumentError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .and_then(parse_hex_rgb)
        .ok_or_else(|| DocumentError::Invalid(format!("{key} must be a #RGB or #RRGGBB color")))
}

fn optional_color(object: &Map<String, Value>, key: &str) -> Result<Option<Rgb8>, DocumentError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .and_then(parse_hex_rgb)
            .map(Some)
            .ok_or_else(|| {
                DocumentError::Invalid(format!("{key} must be a #RGB or #RRGGBB color"))
            }),
    }
}

fn rgba(object: &Map<String, Value>, key: &str) -> Result<Rgba8, DocumentError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .and_then(parse_hex_rgba)
        .ok_or_else(|| {
            DocumentError::Invalid(format!("{key} must be a #RGB[A] or #RRGGBB[AA] color"))
        })
}

fn number(object: &Map<String, Value>, key: &str) -> Result<f64, DocumentError> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| DocumentError::Invalid(format!("{key} must be a finite number")))
}

fn rgb_from_rgba(value: Rgba8) -> Rgb8 {
    [value[0], value[1], value[2]]
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "theme".to_owned()
    } else {
        slug.to_owned()
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    #[test]
    fn pure_color_documents_are_accepted_without_non_color_sections() {
        let mut value = builtin_document(ThemeName::Nord).unwrap().to_value();
        let object = value.as_object_mut().unwrap();
        object.remove("typography");
        object.remove("layout");
        object.remove("effects");
        let document = ThemeDocument::from_value(value).expect("纯颜色主题必须可导入");
        assert_eq!(document.definition().unwrap().name, "Nord");
    }

    #[test]
    fn generated_documents_carry_only_colors_and_ansi() {
        let document = builtin_document(ThemeName::Linear).unwrap();
        let value = document.to_value();
        for legacy in ["typography", "layout", "effects"] {
            assert!(
                value.get(legacy).is_none(),
                "主题文档不得携带非颜色区段 {legacy}"
            );
        }
    }

    #[test]
    fn legacy_non_color_sections_are_rejected_as_unknown_fields() {
        let mut value = builtin_document(ThemeName::Nord).unwrap().to_value();
        value["typography"] = json!({ "enabled": false, "ligatures": true });
        let error = ThemeDocument::from_value(value).unwrap_err();
        assert!(
            error.to_string().contains("typography"),
            "旧非颜色区段必须按未知字段拒绝: {error}"
        );
    }

    #[test]
    fn extended_indexed_colors_are_derived_not_persisted() {
        // 16–255 只按 xterm 公式派生：文档不得写出第二份索引色表。
        let document = builtin_document(ThemeName::Nord).unwrap();
        assert!(
            document.to_value()["terminal"].get("indexed").is_none(),
            "主题文档不得持久化 16–255 索引色表"
        );
        // 旧文档携带 indexed 视为未知字段。
        let mut legacy = document.to_value();
        legacy["terminal"]["indexed"] = json!({ "16": "#000000" });
        assert!(matches!(
            ThemeDocument::from_value(legacy),
            Err(DocumentError::Invalid(message)) if message.contains("indexed")
        ));
        // 派生值与 xterm 常量一致。
        let definition = document.definition().unwrap();
        assert_eq!(definition.terminal.indexed(21), Some([0, 0, 255]));
        assert_eq!(definition.terminal.indexed(232), Some([8, 8, 8]));
        assert_eq!(definition.terminal.indexed(255), Some([238, 238, 238]));
    }

    #[test]
    fn builtins_produce_one_valid_document_per_catalog_entry() {
        let documents = builtin_documents();
        assert_eq!(documents.len(), ThemeName::BUILTIN.len());
        let linear = documents
            .iter()
            .find(|document| document.name() == "Linear")
            .expect("默认主题必须在目录内");
        assert_eq!(linear.id(), Some("builtin-linear"));
        assert!(linear.is_builtin());
        assert!(
            documents
                .iter()
                .all(|document| document.palette().is_some())
        );
    }

    #[test]
    fn envelope_rejects_wrong_schema_versions_and_malformed_roots() {
        let mut value = builtin_document(ThemeName::Nord).unwrap().to_value();
        value["schema_version"] = json!(2);
        assert!(matches!(
            ThemeDocument::from_value(value),
            Err(DocumentError::Invalid(message)) if message.contains("schema_version")
        ));
        assert!(ThemeDocument::from_value(json!([1, 2, 3])).is_err());
        let mut value = builtin_document(ThemeName::Nord).unwrap().to_value();
        value["__proto__"] = json!({});
        assert!(ThemeDocument::from_value(value).is_err());
    }

    #[test]
    fn oversized_documents_and_unknown_fields_have_distinct_fates() {
        let oversized = vec![b'x'; MAX_DOCUMENT_BYTES + 1];
        assert!(matches!(
            ThemeDocument::from_json_bytes(&oversized),
            Err(DocumentError::TooLarge { .. })
        ));
        // 未知字段一律拒绝：不生成部分主题，也不静默忽略拼写错误。
        let mut unknown_root = builtin_documents().remove(0).to_value();
        unknown_root["vendor"] = json!({ "future": true });
        assert!(ThemeDocument::from_value(unknown_root).is_err());
        let mut unknown_terminal = builtin_documents().remove(0).to_value();
        unknown_terminal["terminal"]["vendor_color"] = json!("keep");
        assert!(ThemeDocument::from_value(unknown_terminal).is_err());
        // 扩展命名空间是显式开放对象，内容不参与主题语义。
        let mut extension = builtin_documents().remove(0).to_value();
        extension["extensions"]["vendor"] = json!({ "future": true });
        let document = ThemeDocument::from_value(extension.clone()).unwrap();
        assert_eq!(
            document.to_value()["extensions"],
            extension["extensions"],
            "显式扩展命名空间原样保留"
        );
    }

    #[test]
    fn definition_round_trips_through_the_envelope_losslessly() {
        let mut source = ThemeDefinition::from_builtin(ThemeName::CatppuccinMocha);
        source.terminal.cursor = Some([1, 2, 3]);
        source.terminal.selection_background = Some([4, 5, 6]);
        source.ui.selection = [9, 8, 7, 0x40];
        let document = from_definition(&source).unwrap();
        let decoded = document.definition().unwrap();
        assert_eq!(decoded.name, source.name);
        assert_eq!(decoded.base, source.base);
        assert_eq!(decoded.terminal, source.terminal);
        assert_eq!(decoded.ui, source.ui);
        assert_eq!(
            document.color("terminal", "background"),
            Some(source.terminal.background)
        );
        // 未知字段在 with_definition 后仍在原位。
        let mut value = document.to_value();
        value["extensions"] = json!({"vendor": {"x": 1}});
        let document = ThemeDocument::from_value(value).unwrap();
        let edited = source.with_terminal_foreground([1, 2, 3]);
        let updated = document.with_definition(&edited).unwrap();
        assert_eq!(updated.definition().unwrap().terminal.foreground, [1, 2, 3]);
        assert_eq!(updated.to_value()["extensions"]["vendor"]["x"], 1);
    }

    #[test]
    fn forks_get_a_fresh_identity_and_keep_the_source_name() {
        let builtin = builtin_document(ThemeName::Nord).unwrap();
        let fork = builtin.fork("my-nord", "My Nord").unwrap();
        assert_eq!(fork.id(), Some("my-nord"));
        assert_eq!(fork.revision(), 0);
        assert!(!fork.is_builtin());
        assert_eq!(fork.to_value()["metadata"]["based_on"], "Nord");
    }

    #[test]
    fn seed_document_keeps_explicit_terminal_colors() {
        let document = seed_document("Imported", [0x10, 0x20, 0x30], [0xf0, 0xe0, 0xd0]).unwrap();
        let definition = document.definition().unwrap();
        assert_eq!(definition.terminal.background, [0x10, 0x20, 0x30]);
        assert_eq!(definition.terminal.foreground, [0xf0, 0xe0, 0xd0]);
        assert_eq!(definition.base, ThemeName::default());
    }
}
