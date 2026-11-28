//! Agent 基础配置段：按 CLI 的展示过滤、名单、别名与按 AgentRoot 的运行时清单。
//!
//! 段只承载值事实与纯校验；真实目录检查、命令命名空间冲突与写盘归
//! application。未注册 CLI 的条目原样保留，不因页面未显示而删除。

use std::collections::BTreeMap;

use crate::identity::AgentRootId;
use crate::json::Value;

/// 域外顶层段名（settings.json 顶层键）。
pub const AGENT_FILES_SEGMENT: &str = "agent_files";

/// CLI 别名上限：字符数而非字节数，与用户可见规则一致。
pub const MAX_ALIAS_CHARS: usize = 32;

/// 根层展示模式：全部或只显示自定义名单。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentDisplayMode {
    #[default]
    All,
    Custom,
}

impl AgentDisplayMode {
    pub const ALL: [Self; 2] = [Self::All, Self::Custom];
    pub const VALUES: [&'static str; 2] = ["all", "custom"];

    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub const fn settings_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Custom => "custom",
        }
    }
}

/// 单个 CLI 的展示配置：模式、名单、别名与运行时开关。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCliConfig {
    pub display_mode: AgentDisplayMode,
    /// 自定义模式展示的根层完整名称；大小写不敏感唯一。
    pub custom_names: Vec<String>,
    /// 用户追加的 CLI 别名；与全部内置命令共享命名空间。
    pub aliases: Vec<String>,
    /// 全部模式下是否显示运行时清单命中的根层条目。
    pub show_runtime_files: bool,
}

impl Default for AgentCliConfig {
    fn default() -> Self {
        Self {
            display_mode: AgentDisplayMode::All,
            custom_names: Vec::new(),
            aliases: Vec::new(),
            // 运行时文件缺省隐藏；清单为空时没有任何可见影响。
            show_runtime_files: false,
        }
    }
}

impl AgentCliConfig {
    fn to_value(&self) -> Value {
        Value::Object(vec![
            (
                "display_mode".to_owned(),
                Value::String(self.display_mode.settings_value().to_owned()),
            ),
            (
                "custom_names".to_owned(),
                Value::Array(
                    self.custom_names
                        .iter()
                        .map(|name| Value::String(name.clone()))
                        .collect(),
                ),
            ),
            (
                "aliases".to_owned(),
                Value::Array(
                    self.aliases
                        .iter()
                        .map(|alias| Value::String(alias.clone()))
                        .collect(),
                ),
            ),
            (
                "show_runtime_files".to_owned(),
                Value::Bool(self.show_runtime_files),
            ),
        ])
    }

    /// 字段缺失回缺省；出现但类型或枚举值非法时整段拒绝，不生成部分配置。
    fn from_value(value: &Value) -> Result<Self, AgentFilesConfigError> {
        let Value::Object(_) = value else {
            return Err(AgentFilesConfigError::Invalid);
        };
        let display_mode = match value.get("display_mode") {
            None => AgentDisplayMode::All,
            Some(raw) => raw
                .as_str()
                .and_then(AgentDisplayMode::from_settings)
                .ok_or(AgentFilesConfigError::Invalid)?,
        };
        let custom_names =
            string_list(value.get("custom_names")).ok_or(AgentFilesConfigError::Invalid)?;
        let aliases = string_list(value.get("aliases")).ok_or(AgentFilesConfigError::Invalid)?;
        let show_runtime_files = match value.get("show_runtime_files") {
            None => false,
            Some(raw) => raw.as_bool().ok_or(AgentFilesConfigError::Invalid)?,
        };
        Ok(Self {
            display_mode,
            custom_names,
            aliases,
            show_runtime_files,
        })
    }
}

/// 单个 AgentRoot 的运行时文件清单。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRootRuntimeFiles {
    pub root_id: AgentRootId,
    pub names: Vec<String>,
}

impl AgentRootRuntimeFiles {
    fn to_value(&self) -> Value {
        Value::Object(vec![
            (
                "root_id".to_owned(),
                Value::String(self.root_id.to_string()),
            ),
            (
                "names".to_owned(),
                Value::Array(
                    self.names
                        .iter()
                        .map(|name| Value::String(name.clone()))
                        .collect(),
                ),
            ),
        ])
    }

    fn from_value(value: &Value) -> Result<Self, AgentFilesConfigError> {
        let Value::Object(_) = value else {
            return Err(AgentFilesConfigError::Invalid);
        };
        let root_id = value
            .get("root_id")
            .and_then(Value::as_str)
            .ok_or(AgentFilesConfigError::Invalid)?
            .parse::<AgentRootId>()
            .map_err(|_| AgentFilesConfigError::Invalid)?;
        let names = string_list(value.get("names")).ok_or(AgentFilesConfigError::Invalid)?;
        Ok(Self { root_id, names })
    }
}

/// 根层可见性投影：设置页与 Agent 树共用的纯判定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRootVisibility {
    pub display_mode: AgentDisplayMode,
    pub custom_names: Vec<String>,
    pub runtime_names: Vec<String>,
    pub show_runtime_files: bool,
}

impl AgentRootVisibility {
    /// 根层完整名称是否可见；只匹配名称，不支持路径、通配符或深层过滤。
    pub fn allows_entry(&self, name: &str) -> bool {
        match self.display_mode {
            AgentDisplayMode::Custom => contains_name(&self.custom_names, name),
            AgentDisplayMode::All => {
                self.show_runtime_files || !contains_name(&self.runtime_names, name)
            }
        }
    }

    /// 根层条目中被隐藏的数量；用于节点行披露过滤结果。
    pub fn hidden_count<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> usize {
        names
            .into_iter()
            .filter(|name| !self.allows_entry(name))
            .count()
    }
}

impl Default for AgentRootVisibility {
    fn default() -> Self {
        Self {
            display_mode: AgentDisplayMode::All,
            custom_names: Vec::new(),
            runtime_names: Vec::new(),
            show_runtime_files: true,
        }
    }
}

/// 全部 Agent 基础配置；未知 CLI 条目原样保留。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentFilesConfig {
    cli: BTreeMap<String, AgentCliConfig>,
    runtime: Vec<AgentRootRuntimeFiles>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentFilesConfigError {
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentNameError {
    Empty,
    Separator,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentAliasError {
    Empty,
    TooLong,
    Whitespace,
    Quote,
    LeadingDash,
    Duplicate,
}

impl AgentFilesConfig {
    /// 已登记 CLI 的配置；缺省返回 None，调用方回退 [`AgentCliConfig::default`]。
    pub fn cli(&self, slug: &str) -> Option<&AgentCliConfig> {
        self.cli.get(slug)
    }

    /// 写入单个 CLI 配置；保留其他 CLI（含未注册 CLI）的既有条目。
    pub fn set_cli(&mut self, slug: &str, config: AgentCliConfig) {
        self.cli.insert(slug.to_owned(), config);
    }

    /// 恢复单个 CLI 默认：移除条目等价于全部缺省。
    pub fn reset_cli(&mut self, slug: &str) {
        self.cli.remove(slug);
    }

    /// 按 CLI 与根身份计算根层可见性；`cli = None` 表示自定义根。
    pub fn visibility(&self, cli: Option<&str>, root_id: AgentRootId) -> AgentRootVisibility {
        let runtime_names = self.runtime_names(root_id).to_vec();
        let Some(slug) = cli else {
            // 自定义根不归属 CLI，只消费自身运行时清单。
            return AgentRootVisibility {
                display_mode: AgentDisplayMode::All,
                custom_names: Vec::new(),
                runtime_names,
                show_runtime_files: false,
            };
        };
        let config = self.cli.get(slug).cloned().unwrap_or_default();
        AgentRootVisibility {
            display_mode: config.display_mode,
            custom_names: config.custom_names,
            runtime_names,
            show_runtime_files: config.show_runtime_files,
        }
    }

    pub fn runtime_names(&self, root_id: AgentRootId) -> &[String] {
        self.runtime
            .iter()
            .find(|entry| entry.root_id == root_id)
            .map(|entry| entry.names.as_slice())
            .unwrap_or(&[])
    }

    /// 写入单个根的运行时清单；空清单等价于恢复默认。
    pub fn set_runtime_names(&mut self, root_id: AgentRootId, names: Vec<String>) {
        if names.is_empty() {
            self.runtime.retain(|entry| entry.root_id != root_id);
            return;
        }
        if let Some(entry) = self
            .runtime
            .iter_mut()
            .find(|entry| entry.root_id == root_id)
        {
            entry.names = names;
        } else {
            self.runtime.push(AgentRootRuntimeFiles { root_id, names });
        }
    }

    pub fn reset_runtime_names(&mut self, root_id: AgentRootId) {
        self.runtime.retain(|entry| entry.root_id != root_id);
    }

    /// 全部已持久化的 CLI 键；用于“未注册 CLI 原样保留”测试与导出。
    pub fn cli_slugs(&self) -> impl Iterator<Item = &str> {
        self.cli.keys().map(String::as_str)
    }

    /// 全部按根运行时清单；提交前逐项校验清单形状。
    pub fn runtime_entries(&self) -> impl Iterator<Item = (AgentRootId, &[String])> {
        self.runtime
            .iter()
            .map(|entry| (entry.root_id, entry.names.as_slice()))
    }

    /// 段解码：缺失段 = 空配置；结构非法整段拒绝，不返回部分配置。
    pub fn from_segment(value: Option<&Value>) -> Result<Self, AgentFilesConfigError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let Value::Object(_) = value else {
            return Err(AgentFilesConfigError::Invalid);
        };
        let mut config = Self::default();
        if let Some(cli) = value.get("cli") {
            let Value::Object(entries) = cli else {
                return Err(AgentFilesConfigError::Invalid);
            };
            for (slug, entry) in entries {
                config
                    .cli
                    .insert(slug.clone(), AgentCliConfig::from_value(entry)?);
            }
        }
        if let Some(runtime) = value.get("runtime") {
            let Value::Array(items) = runtime else {
                return Err(AgentFilesConfigError::Invalid);
            };
            for item in items {
                let entry = AgentRootRuntimeFiles::from_value(item)?;
                if config
                    .runtime
                    .iter()
                    .any(|existing| existing.root_id == entry.root_id)
                {
                    return Err(AgentFilesConfigError::Invalid);
                }
                config.runtime.push(entry);
            }
        }
        Ok(config)
    }

    /// 段编码：固定字段形态；CLI 键与根身份稳定排序，便于对账与测试。
    pub fn to_segment(&self) -> Value {
        let cli = self
            .cli
            .iter()
            .map(|(slug, config)| (slug.clone(), config.to_value()))
            .collect::<Vec<_>>();
        Value::Object(vec![
            ("cli".to_owned(), Value::Object(cli)),
            (
                "runtime".to_owned(),
                Value::Array(self.runtime.iter().map(|entry| entry.to_value()).collect()),
            ),
        ])
    }
}

/// 名单项形状：非空、不含路径分隔符；通配符与深层匹配由“只匹配完整名称”保证。
pub fn validate_agent_name(name: &str) -> Result<(), AgentNameError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AgentNameError::Empty);
    }
    if name.contains(['/', '\\']) {
        return Err(AgentNameError::Separator);
    }
    Ok(())
}

/// 名单整体：逐项形状合法且大小写不敏感唯一。
pub fn validate_agent_name_list(names: &[String]) -> Result<(), AgentNameError> {
    let mut seen = BTreeMap::new();
    for name in names {
        validate_agent_name(name)?;
        let key = name.trim().to_lowercase();
        if seen.insert(key, ()).is_some() {
            return Err(AgentNameError::Duplicate);
        }
    }
    Ok(())
}

/// 别名形状：非空、≤32 字符、无空白/引号/反引号、不以 `-` 开头。
pub fn validate_agent_alias(alias: &str) -> Result<(), AgentAliasError> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(AgentAliasError::Empty);
    }
    if alias.chars().count() > MAX_ALIAS_CHARS {
        return Err(AgentAliasError::TooLong);
    }
    if alias.chars().any(char::is_whitespace) {
        return Err(AgentAliasError::Whitespace);
    }
    if alias
        .chars()
        .any(|character| matches!(character, '"' | '\'' | '`'))
    {
        return Err(AgentAliasError::Quote);
    }
    if alias.starts_with('-') {
        return Err(AgentAliasError::LeadingDash);
    }
    Ok(())
}

/// 别名整体：逐项合法且大小写不敏感唯一。
pub fn validate_agent_alias_list(aliases: &[String]) -> Result<(), AgentAliasError> {
    let mut seen = BTreeMap::new();
    for alias in aliases {
        validate_agent_alias(alias)?;
        if seen.insert(alias.trim().to_lowercase(), ()).is_some() {
            return Err(AgentAliasError::Duplicate);
        }
    }
    Ok(())
}

fn contains_name(names: &[String], name: &str) -> bool {
    let name = name.trim().to_lowercase();
    names
        .iter()
        .any(|candidate| candidate.trim().to_lowercase() == name)
}

fn string_list(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect(),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_id() -> AgentRootId {
        AgentRootId::from_bytes([
            0x01, 0x8f, 0x2f, 0x7c, 0x8f, 0x5a, 0x7c, 0x3d, 0x9e, 0x2b, 0x4a, 0x6f, 0x8c, 0x1d,
            0x0e, 0x2c,
        ])
        .unwrap()
    }

    /// 段往返保留未知 CLI 与按根清单，缺省显示运行时文件。
    #[test]
    fn segment_round_trip_preserves_unknown_cli_and_runtime_lists() {
        let mut config = AgentFilesConfig::default();
        config.set_cli(
            "claude",
            AgentCliConfig {
                display_mode: AgentDisplayMode::Custom,
                custom_names: vec!["settings.json".into()],
                aliases: vec!["my-claude".into()],
                show_runtime_files: false,
            },
        );
        config.set_cli("retired-cli", AgentCliConfig::default());
        config.set_runtime_names(root_id(), vec!["history".into()]);

        let decoded = AgentFilesConfig::from_segment(Some(&config.to_segment())).unwrap();
        assert_eq!(decoded, config);
        assert!(decoded.cli("retired-cli").is_some(), "未注册 CLI 必须保留");
        assert_eq!(decoded.runtime_names(root_id()), ["history"]);
        assert!(!AgentCliConfig::default().show_runtime_files);
    }

    /// 非法结构整段拒绝，不静默生成部分配置。
    #[test]
    fn malformed_segment_is_rejected() {
        for text in [
            r#"{"cli":[]}"#,
            r#"{"cli":{"claude":{"display_mode":"deep"}}}"#,
            r#"{"cli":{"claude":{"custom_names":"a"}}}"#,
            r#"{"runtime":[{"root_id":"nope","names":[]}]}"#,
            r#"{"runtime":[{"root_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c","names":[]},{"root_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c","names":[]}]}"#,
        ] {
            let value = crate::json::parse(text.as_bytes()).unwrap();
            assert!(
                AgentFilesConfig::from_segment(Some(&value)).is_err(),
                "{text}"
            );
        }
        assert!(
            AgentFilesConfig::from_segment(None)
                .unwrap()
                .cli_slugs()
                .next()
                .is_none()
        );
    }

    /// 名单只接受单段名称且同清单大小写不敏感唯一。
    #[test]
    fn name_list_rejects_paths_and_duplicates() {
        assert!(validate_agent_name("settings.json").is_ok());
        for name in ["", "   ", "a/b", r"a\b"] {
            assert!(validate_agent_name(name).is_err(), "{name:?}");
        }
        assert_eq!(
            validate_agent_name_list(&["AGENTS.md".into(), "agents.md".into()]),
            Err(AgentNameError::Duplicate)
        );
        assert!(validate_agent_name_list(&["AGENTS.md".into(), "skills".into()]).is_ok());
    }

    /// 别名形状与大小写唯一；内置命名空间冲突归 application 叠加判定。
    #[test]
    fn alias_shape_and_list_uniqueness() {
        for alias in ["claude", "my-claude", "a_b.c"] {
            assert!(validate_agent_alias(alias).is_ok(), "{alias}");
        }
        assert_eq!(validate_agent_alias(""), Err(AgentAliasError::Empty));
        assert_eq!(
            validate_agent_alias(&"a".repeat(33)),
            Err(AgentAliasError::TooLong)
        );
        assert_eq!(
            validate_agent_alias("two words"),
            Err(AgentAliasError::Whitespace)
        );
        assert_eq!(
            validate_agent_alias("quoted\""),
            Err(AgentAliasError::Quote)
        );
        assert_eq!(
            validate_agent_alias("-flag"),
            Err(AgentAliasError::LeadingDash)
        );
        assert_eq!(
            validate_agent_alias_list(&["A".into(), "a".into()]),
            Err(AgentAliasError::Duplicate)
        );
    }

    /// 自定义模式只显示名单；全部模式按运行时开关隐藏；自定义根恒隐藏运行时项。
    #[test]
    fn visibility_semantics_follow_spec() {
        let runtime = vec!["settings.json".to_owned(), "history".to_owned()];
        let custom = AgentRootVisibility {
            display_mode: AgentDisplayMode::Custom,
            custom_names: vec!["skills".into()],
            runtime_names: runtime.clone(),
            show_runtime_files: true,
        };
        assert!(custom.allows_entry("Skills"), "名单大小写不敏感");
        assert!(
            !custom.allows_entry("settings.json"),
            "自定义模式忽略运行时开关"
        );

        let all_hidden = AgentRootVisibility {
            display_mode: AgentDisplayMode::All,
            custom_names: Vec::new(),
            runtime_names: runtime.clone(),
            show_runtime_files: false,
        };
        assert!(!all_hidden.allows_entry("HISTORY"));
        assert!(all_hidden.allows_entry("skills"));

        let all_shown = AgentRootVisibility {
            show_runtime_files: true,
            ..all_hidden.clone()
        };
        assert!(all_shown.allows_entry("history"));

        let custom_root = AgentRootVisibility {
            display_mode: AgentDisplayMode::All,
            custom_names: Vec::new(),
            runtime_names: runtime,
            show_runtime_files: false,
        };
        assert!(!custom_root.allows_entry("settings.json"));
        assert_eq!(custom_root.hidden_count(["settings.json", "notes"]), 1);
    }

    /// 按 CLI/根身份派生可见性；自定义根不归属 CLI。
    #[test]
    fn config_visibility_is_derived_per_cli_and_root() {
        let mut config = AgentFilesConfig::default();
        config.set_cli(
            "claude",
            AgentCliConfig {
                display_mode: AgentDisplayMode::Custom,
                custom_names: vec!["skills".into()],
                aliases: Vec::new(),
                show_runtime_files: true,
            },
        );
        config.set_runtime_names(root_id(), vec!["settings.json".into()]);
        let builtin = config.visibility(Some("claude"), root_id());
        assert!(builtin.allows_entry("skills"));
        assert!(!builtin.allows_entry("settings.json"));
        let custom = config.visibility(None, root_id());
        assert!(!custom.allows_entry("settings.json"));
        assert!(custom.allows_entry("skills"));
    }
}
