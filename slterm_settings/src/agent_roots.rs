//! AgentRoot 配置段：稳定身份、显示名、词法绝对路径与独立能力。
//!
//! 内置根来自 AgentCatalog 的默认目录，不进入本段；本段只保存自定义根。
//! 默认只读且禁止终端；写与终端能力必须显式开启。

use crate::identity::AgentRootId;
use crate::json::Value;

/// 单根能力：读取是根的存在意义，写与终端各自独立授权。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentRootCapabilities {
    pub write: bool,
    pub terminal: bool,
}

impl AgentRootCapabilities {
    /// 新根默认：可读、不可写、不可作为终端 cwd。
    pub const READ_ONLY: Self = Self {
        write: false,
        terminal: false,
    };

    pub const fn allows_write(self) -> bool {
        self.write
    }

    pub const fn allows_terminal(self) -> bool {
        self.terminal
    }
}

/// 自定义 AgentRoot 的持久事实。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRoot {
    id: AgentRootId,
    display_name: String,
    path: String,
    capabilities: AgentRootCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentRootError {
    EmptyName,
    NameTooLong,
    InvalidPath,
    InvalidEncoding,
    MissingField,
}

/// 显示名上限；名称只是展示事实，不参与授权。
const MAX_NAME_CHARS: usize = 64;

impl AgentRoot {
    /// 注册自定义根：名称与词法路径立即校验，能力固定默认只读。
    pub fn custom(
        id: AgentRootId,
        display_name: String,
        path: String,
    ) -> Result<Self, AgentRootError> {
        validate_name(&display_name)?;
        validate_path(&path)?;
        Ok(Self {
            id,
            display_name,
            path,
            capabilities: AgentRootCapabilities::READ_ONLY,
        })
    }

    pub fn with_capabilities(mut self, capabilities: AgentRootCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// 改路径按撤销旧根＋登记新根处理：必须由调用方提供新身份。
    pub fn with_path(&self, id: AgentRootId, path: String) -> Result<Self, AgentRootError> {
        validate_path(&path)?;
        Ok(Self {
            id,
            display_name: self.display_name.clone(),
            path,
            capabilities: self.capabilities,
        })
    }

    pub fn id(&self) -> AgentRootId {
        self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn capabilities(&self) -> AgentRootCapabilities {
        self.capabilities
    }

    /// 持久形态：能力位独立字段，未知字段由段级合并原位保留。
    pub fn to_value(&self) -> Value {
        Value::Object(vec![
            ("id".to_owned(), Value::String(self.id.to_string())),
            ("name".to_owned(), Value::String(self.display_name.clone())),
            ("path".to_owned(), Value::String(self.path.clone())),
            ("write".to_owned(), Value::Bool(self.capabilities.write)),
            (
                "terminal".to_owned(),
                Value::Bool(self.capabilities.terminal),
            ),
        ])
    }

    /// 严格解码：缺字段或非法值拒绝，不生成部分根。
    pub fn from_value(value: &Value) -> Result<Self, AgentRootError> {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(AgentRootError::MissingField)?
            .parse::<AgentRootId>()
            .map_err(|_| AgentRootError::InvalidEncoding)?;
        let display_name = value
            .get("name")
            .and_then(Value::as_str)
            .ok_or(AgentRootError::MissingField)?
            .to_owned();
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .ok_or(AgentRootError::MissingField)?
            .to_owned();
        let capabilities = AgentRootCapabilities {
            write: value
                .get("write")
                .and_then(Value::as_bool)
                .ok_or(AgentRootError::MissingField)?,
            terminal: value
                .get("terminal")
                .and_then(Value::as_bool)
                .ok_or(AgentRootError::MissingField)?,
        };
        Ok(Self::custom(id, display_name, path)?.with_capabilities(capabilities))
    }
}

/// 自定义根段名（settings.json 顶层段）。
pub const AGENT_ROOTS_SEGMENT: &str = "agent_roots";

/// 段解码：缺失段或缺失 `roots` 视为空；结构非法整段拒绝，不返回部分根。
pub fn agent_roots_from_segment(value: Option<&Value>) -> Result<Vec<AgentRoot>, AgentRootError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(roots) = value.get("roots") else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = roots else {
        return Err(AgentRootError::MissingField);
    };
    items.iter().map(AgentRoot::from_value).collect()
}

/// 段编码：始终写出 `roots` 数组；空列表也写空数组。
pub fn agent_roots_to_segment(roots: &[AgentRoot]) -> Value {
    Value::Object(vec![(
        "roots".to_owned(),
        Value::Array(roots.iter().map(AgentRoot::to_value).collect()),
    )])
}

fn validate_name(name: &str) -> Result<(), AgentRootError> {
    if name.trim().is_empty() {
        return Err(AgentRootError::EmptyName);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(AgentRootError::NameTooLong);
    }
    Ok(())
}

/// 词法校验：绝对路径、无父级组件、无控制字符；真实查重由服务层负责。
fn validate_path(path: &str) -> Result<(), AgentRootError> {
    if path.is_empty() || path.chars().any(char::is_control) {
        return Err(AgentRootError::InvalidPath);
    }
    let bytes = path.as_bytes();
    let has_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    let rooted = path.starts_with('\\') || path.starts_with('/');
    if !has_drive && !rooted {
        return Err(AgentRootError::InvalidPath);
    }
    if path.split(['\\', '/']).any(|component| component == "..") {
        return Err(AgentRootError::InvalidPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AgentRoot, AgentRootCapabilities, AgentRootError};
    use crate::identity::AgentRootId;
    use crate::json::Value;

    fn root_id() -> AgentRootId {
        AgentRootId::from_bytes([
            0x01, 0x8f, 0x2f, 0x7c, 0x8f, 0x5a, 0x7c, 0x3d, 0x9e, 0x2b, 0x4a, 0x6f, 0x8c, 0x1d,
            0x0e, 0x2b,
        ])
        .unwrap()
    }

    fn second_root_id() -> AgentRootId {
        AgentRootId::from_bytes([
            0x01, 0x8f, 0x2f, 0x7c, 0x8f, 0x5a, 0x7c, 0x3d, 0x9e, 0x2b, 0x4a, 0x6f, 0x8c, 0x1d,
            0x0e, 0x2c,
        ])
        .unwrap()
    }

    /// 新根默认只读且禁止终端；写与终端必须显式开启。
    #[test]
    fn agent_root_defaults_to_read_only() {
        let root =
            AgentRoot::custom(root_id(), "工作区".to_owned(), r"C:\work".to_owned()).unwrap();
        assert_eq!(root.capabilities(), AgentRootCapabilities::READ_ONLY);
        assert!(!root.capabilities().allows_write());
        assert!(!root.capabilities().allows_terminal());
    }

    /// 名称与词法路径在构造时校验，非法输入不生成根。
    #[test]
    fn agent_root_rejects_invalid_name_and_path() {
        for name in ["", "   ", &"x".repeat(65)] {
            assert!(
                AgentRoot::custom(root_id(), name.to_owned(), r"C:\work".to_owned()).is_err(),
                "{name:?} 必须被拒绝"
            );
        }
        for path in ["relative/path", r"C:\work\..\secret", "C:/work/\0bad", ""] {
            assert!(
                AgentRoot::custom(root_id(), "工作区".to_owned(), path.to_owned()).is_err(),
                "{path:?} 必须被拒绝"
            );
        }
    }

    /// JSON 往返保真，能力位独立保存。
    #[test]
    fn agent_root_roundtrips_through_json() {
        let root = AgentRoot::custom(root_id(), "工作区".to_owned(), r"C:\work".to_owned())
            .unwrap()
            .with_capabilities(AgentRootCapabilities {
                write: true,
                terminal: false,
            });
        let value = root.to_value();
        let decoded = AgentRoot::from_value(&value).unwrap();
        assert_eq!(decoded, root);
        assert_eq!(decoded.id(), root.id());
        assert_eq!(decoded.path(), r"C:\work");
        assert!(decoded.capabilities().write);
        assert!(!decoded.capabilities().terminal);
    }

    /// 改路径必须换身份：旧根撤销、新根登记由调用方按两个 id 完成。
    #[test]
    fn agent_root_path_change_requires_new_identity() {
        let root =
            AgentRoot::custom(root_id(), "工作区".to_owned(), r"C:\work".to_owned()).unwrap();
        let moved = root
            .with_path(second_root_id(), r"D:\work".to_owned())
            .unwrap();
        assert_ne!(moved.id(), root.id());
        assert_eq!(moved.path(), r"D:\work");
        assert_eq!(moved.display_name(), root.display_name());
        assert_eq!(moved.capabilities(), root.capabilities());
        assert!(root.with_path(root_id(), "relative".to_owned()).is_err());
    }

    /// 段往返保真；缺失段与缺失 roots 都视为空配置。
    #[test]
    fn agent_root_segment_roundtrips_and_defaults_empty() {
        use super::{agent_roots_from_segment, agent_roots_to_segment};

        assert!(agent_roots_from_segment(None).unwrap().is_empty());
        assert!(
            agent_roots_from_segment(Some(&Value::empty_object()))
                .unwrap()
                .is_empty()
        );
        let root = AgentRoot::custom(root_id(), "工作区".to_owned(), r"C:\work".to_owned())
            .unwrap()
            .with_capabilities(AgentRootCapabilities {
                write: true,
                terminal: true,
            });
        let segment = agent_roots_to_segment(std::slice::from_ref(&root));
        assert_eq!(
            agent_roots_from_segment(Some(&segment)).unwrap(),
            vec![root]
        );
        assert_eq!(
            agent_roots_to_segment(&[]),
            Value::Object(vec![("roots".to_owned(), Value::Array(Vec::new()))])
        );
    }

    /// 段结构非法或元素残缺时整段拒绝，不返回部分根。
    #[test]
    fn agent_root_segment_rejects_malformed() {
        use super::agent_roots_from_segment;

        let wrong_shape = Value::Object(vec![(
            "roots".to_owned(),
            Value::String("not-an-array".to_owned()),
        )]);
        assert!(agent_roots_from_segment(Some(&wrong_shape)).is_err());

        let partial = Value::Object(vec![(
            "roots".to_owned(),
            Value::Array(vec![Value::empty_object()]),
        )]);
        assert_eq!(
            agent_roots_from_segment(Some(&partial)).unwrap_err(),
            AgentRootError::MissingField
        );
    }

    /// 域外段读取只放行白名单段；注册表段不能经此绕过字段校验。
    #[test]
    fn external_segment_reads_only_whitelisted_segments() {
        use super::{AGENT_ROOTS_SEGMENT, agent_roots_from_segment};
        use crate::RawSettings;

        let raw = RawSettings::from_json_bytes(
            br#"{"agent_roots":{"roots":[]},"appearance":{"theme":"linear"}}"#,
        );
        assert!(raw.external_segment(AGENT_ROOTS_SEGMENT).is_some());
        assert!(raw.external_segment("appearance").is_none());
        assert!(
            agent_roots_from_segment(raw.external_segment(AGENT_ROOTS_SEGMENT))
                .unwrap()
                .is_empty()
        );
    }

    /// 解码必须拒绝缺字段与非法值，不生成部分根。
    #[test]
    fn agent_root_decode_rejects_partial_values() {
        let missing = Value::Object(vec![(
            "id".to_owned(),
            Value::String(root_id().to_string()),
        )]);
        assert_eq!(
            AgentRoot::from_value(&missing).unwrap_err(),
            AgentRootError::MissingField
        );
    }
}
