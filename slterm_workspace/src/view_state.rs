//! ViewState 规范 DTO、所属视图 schema 注册与持久作用域。

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::{ProjectId, ViewKey, ViewKeyKind};

const VIEW_STATE_VERSION: u64 = 1;

/// 单个视图状态的原始 JSON 未通过所属 schema 校验。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidViewState;

impl fmt::Display for InvalidViewState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("视图状态字段或领域值非法")
    }
}

impl std::error::Error for InvalidViewState {}

/// 所属视图注册的精确状态 schema。
#[derive(Clone, Copy, Debug)]
pub struct ViewStateSchema {
    kind: ViewKeyKind,
    decode: fn(&ViewKey, &str) -> Result<Value, InvalidViewState>,
}

impl ViewStateSchema {
    /// 登记封闭视图类型及按完整 ViewKey 分派的原始 JSON 精确解码器。
    pub const fn new(
        kind: ViewKeyKind,
        decode: fn(&ViewKey, &str) -> Result<Value, InvalidViewState>,
    ) -> Self {
        Self { kind, decode }
    }
}

/// 显式拥有的不可变 schema 集合；未登记视图不能扩展持久字段。
#[derive(Debug)]
pub struct ViewStateRegistry {
    schemas: BTreeMap<ViewKeyKind, ViewStateSchema>,
}

/// schema 注册表的构造错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewStateRegistryError {
    /// 同一视图类型声明了第二份状态事实。
    DuplicateKind(ViewKeyKind),
}

impl ViewStateRegistry {
    /// 由各所有者显式注册 schema；重复类型立即拒绝，不保留覆盖结果。
    pub fn new(schemas: &[ViewStateSchema]) -> Result<Self, ViewStateRegistryError> {
        let mut registered = BTreeMap::new();
        for schema in schemas {
            if registered.contains_key(&schema.kind) {
                return Err(ViewStateRegistryError::DuplicateKind(schema.kind));
            }
            registered.insert(schema.kind, *schema);
        }
        Ok(Self {
            schemas: registered,
        })
    }

    fn schema(&self, kind: ViewKeyKind) -> Result<ViewStateSchema, ViewStateError> {
        self.schemas
            .get(&kind)
            .copied()
            .ok_or(ViewStateError::UnregisteredKind)
    }
}

impl fmt::Display for ViewStateRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKind(kind) => write!(formatter, "重复注册 ViewState schema：{kind:?}"),
        }
    }
}

impl std::error::Error for ViewStateRegistryError {}

/// ViewState DTO 的编码、注册或恢复错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewStateError {
    /// JSON、版本信封或字段集合不符合当前结构。
    InvalidStructure,
    /// 类型名未进入封闭 ViewKey 注册点。
    UnknownKind,
    /// DTO 版本不是当前版本。
    UnsupportedVersion,
    /// 已知视图没有声明持久状态 schema。
    UnregisteredKind,
    /// 所属 schema 拒绝状态字段或领域值。
    InvalidState,
    /// 状态与用于写入、恢复或清理的 ViewKey 身份不同。
    KeyMismatch,
    /// 已构造的规范 JSON 值无法编码。
    Encode,
}

impl fmt::Display for ViewStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "ViewState 持久信封结构非法",
            Self::UnknownKind => "未知 ViewState 视图类型",
            Self::UnsupportedVersion => "不支持的 ViewState DTO 版本",
            Self::UnregisteredKind => "视图未注册 ViewState schema",
            Self::InvalidState => "ViewState 状态字段或领域值非法",
            Self::KeyMismatch => "ViewState 与 ViewKey 身份不匹配",
            Self::Encode => "ViewState 规范 JSON 编码失败",
        })
    }
}

impl std::error::Error for ViewStateError {}

/// 持久状态的生命周期作用域，由 ViewKey 推导而非重复存储。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewStateScope {
    Project(ProjectId),
    Application,
}

/// 经所属 schema 校验并绑定完整稳定身份的规范视图状态。
#[derive(Clone, Debug, PartialEq)]
pub struct ViewState {
    key: ViewKey,
    state: Value,
}

impl ViewState {
    /// 使用已登记 schema 校验并规范化内存状态。
    pub fn new(
        key: &ViewKey,
        state: Value,
        registry: &ViewStateRegistry,
    ) -> Result<Self, ViewStateError> {
        let schema = registry.schema(key.kind())?;
        let raw = serde_json::to_string(&state).map_err(|_| ViewStateError::Encode)?;
        let state = (schema.decode)(key, &raw).map_err(|_| ViewStateError::InvalidState)?;
        Ok(Self {
            key: key.clone(),
            state,
        })
    }

    /// 从独立版本的规范 JSON 恢复；schema 直接读取原始状态以拒绝重复字段。
    pub fn from_canonical_json(
        text: &str,
        key: &ViewKey,
        registry: &ViewStateRegistry,
    ) -> Result<Self, ViewStateError> {
        let envelope: ViewStateEnvelope =
            serde_json::from_str(text).map_err(|_| ViewStateError::InvalidStructure)?;
        if envelope.version != VIEW_STATE_VERSION {
            return Err(ViewStateError::UnsupportedVersion);
        }
        let kind = ViewKeyKind::parse(&envelope.kind).ok_or(ViewStateError::UnknownKind)?;
        let dto: ViewStateDto =
            serde_json::from_str(text).map_err(|_| ViewStateError::InvalidStructure)?;
        if kind != key.kind() {
            return Err(ViewStateError::KeyMismatch);
        }
        let schema = registry.schema(kind)?;
        let state =
            (schema.decode)(key, dto.state.get()).map_err(|_| ViewStateError::InvalidState)?;
        Ok(Self {
            key: key.clone(),
            state,
        })
    }

    /// 序列化为确定键序且可逐字节重放的 JSON。
    pub fn to_canonical_json(&self) -> Result<String, ViewStateError> {
        let object = BTreeMap::from([
            ("kind", Value::String(self.key.kind().id_str().to_owned())),
            ("state", self.state.clone()),
            ("version", Value::from(VIEW_STATE_VERSION)),
        ]);
        serde_json::to_string(&object).map_err(|_| ViewStateError::Encode)
    }

    /// 返回封闭视图类型。
    pub fn kind(&self) -> ViewKeyKind {
        self.key.kind()
    }

    /// 返回经 schema 规范化的状态值。
    pub const fn state(&self) -> &Value {
        &self.state
    }

    /// 取出状态值，交由所属视图转换为自己的强类型状态。
    pub fn into_state(self) -> Value {
        self.state
    }

    /// 校验完整 ViewKey 绑定并推导生命周期作用域，不重复持久化作用域事实。
    pub fn scope(&self, key: &ViewKey) -> Result<ViewStateScope, ViewStateError> {
        if key != &self.key {
            return Err(ViewStateError::KeyMismatch);
        }
        Ok(key
            .project_id()
            .map_or(ViewStateScope::Application, |project_id| {
                ViewStateScope::Project(project_id)
            }))
    }
}

#[derive(Deserialize)]
struct ViewStateEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewStateDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    state: Box<RawValue>,
}

#[cfg(test)]
mod view_state_tests {
    use std::str::FromStr;

    use proptest::prelude::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use slterm_document::{CommitId, DocumentId, RepoPath, RepositoryBinding};
    use slterm_path::{CanonicalPath, ResolvedPath};

    use super::*;
    use crate::{SideViewId, TerminalId};

    const PROJECT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const TERMINAL: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestState {
        persistent: bool,
        position: u64,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NavState {
        position: u64,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExplorerState {
        expanded: bool,
    }

    fn decode_test_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
        let state: TestState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
        serde_json::to_value(state).map_err(|_| InvalidViewState)
    }

    fn decode_side_view_test_state(key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
        let ViewKey::SideView { side_view_id } = key else {
            return Err(InvalidViewState);
        };
        match side_view_id {
            SideViewId::Nav => {
                let state: NavState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
                serde_json::to_value(state).map_err(|_| InvalidViewState)
            }
            SideViewId::Explorer => {
                let state: ExplorerState =
                    serde_json::from_str(text).map_err(|_| InvalidViewState)?;
                serde_json::to_value(state).map_err(|_| InvalidViewState)
            }
            SideViewId::Commit | SideViewId::AgentFiles => Err(InvalidViewState),
        }
    }

    fn side_view_registry() -> ViewStateRegistry {
        ViewStateRegistry::new(&[ViewStateSchema::new(
            ViewKeyKind::SideView,
            decode_side_view_test_state,
        )])
        .unwrap()
    }

    fn side_view_key(side_view_id: SideViewId) -> ViewKey {
        ViewKey::SideView { side_view_id }
    }

    fn registry() -> ViewStateRegistry {
        let schemas = [
            ViewKeyKind::Editor,
            ViewKeyKind::Markdown,
            ViewKeyKind::Diff,
            ViewKeyKind::GitShow,
            ViewKeyKind::Settings,
            ViewKeyKind::SideView,
            ViewKeyKind::AppSettingsOverlay,
        ]
        .map(|kind| ViewStateSchema::new(kind, decode_test_state));
        ViewStateRegistry::new(&schemas).unwrap()
    }

    fn project_id() -> ProjectId {
        ProjectId::from_str(PROJECT).unwrap()
    }

    fn document_id() -> DocumentId {
        DocumentId::from_str(DOCUMENT).unwrap()
    }

    fn editor_key() -> ViewKey {
        ViewKey::Editor {
            project_id: project_id(),
            document_id: document_id(),
        }
    }

    fn resolved(path: &str) -> ResolvedPath {
        ResolvedPath::new(CanonicalPath::new(path).unwrap(), None)
    }

    fn all_view_keys() -> [ViewKey; 8] {
        let repository = RepositoryBinding::new(
            resolved("D:/repo"),
            resolved("D:/repo/.git"),
            resolved("D:/repo/.git"),
            CommitId::from_str(&"a".repeat(40)).unwrap(),
        );
        [
            ViewKey::Terminal {
                project_id: project_id(),
                terminal_id: TerminalId::from_str(TERMINAL).unwrap(),
            },
            editor_key(),
            ViewKey::Markdown {
                project_id: project_id(),
                document_id: document_id(),
            },
            ViewKey::Diff {
                project_id: project_id(),
                document_id: document_id(),
            },
            ViewKey::GitShow {
                project_id: project_id(),
                repository_binding: Box::new(repository),
                commit_id: CommitId::from_str(&"a".repeat(40)).unwrap(),
                repo_path: RepoPath::from_str("dir/file.md").unwrap(),
            },
            ViewKey::Settings {
                project_id: project_id(),
            },
            ViewKey::SideView {
                side_view_id: SideViewId::Explorer,
            },
            ViewKey::AppSettingsOverlay,
        ]
    }

    /// 注册 schema 的原始状态经精确解码后，以独立版本和确定键序逐字节重放。
    #[test]
    fn view_state_round_trips_registered_schema_with_canonical_json() {
        let registry = registry();
        let expected = r#"{"kind":"editor","state":{"persistent":true,"position":7},"version":1}"#;
        let state = ViewState::new(
            &editor_key(),
            json!({"position":7,"persistent":true}),
            &registry,
        )
        .unwrap();
        assert_eq!(state.kind(), ViewKeyKind::Editor);
        assert_eq!(state.state(), &json!({"persistent":true,"position":7}));
        assert_eq!(state.to_canonical_json().unwrap(), expected);

        let reordered = r#"{"state":{"position":7,"persistent":true},"version":1,"kind":"editor"}"#;
        let restored = ViewState::from_canonical_json(reordered, &editor_key(), &registry).unwrap();
        assert_eq!(restored, state);
        assert_eq!(restored.to_canonical_json().unwrap(), expected);
        assert_eq!(
            restored.into_state(),
            json!({"persistent":true,"position":7})
        );
    }

    /// 未知视图、版本不支持、未注册 schema 与信封损坏分别报告。
    #[test]
    fn view_state_distinguishes_envelope_failures() {
        let registry = registry();
        assert_eq!(
            ViewState::from_canonical_json(
                r#"{"kind":"future_view","state":{},"version":1}"#,
                &editor_key(),
                &registry,
            ),
            Err(ViewStateError::UnknownKind)
        );
        for version in [0, 2, u64::MAX] {
            let text = format!(r#"{{"kind":"editor","state":{{}},"version":{version}}}"#);
            assert_eq!(
                ViewState::from_canonical_json(&text, &editor_key(), &registry),
                Err(ViewStateError::UnsupportedVersion),
                "{text}"
            );
        }
        for text in [
            String::new(),
            "{".into(),
            "[]".into(),
            r#"{"kind":"editor","version":1}"#.into(),
            r#"{"state":{},"version":1}"#.into(),
            r#"{"kind":"editor","state":{}}"#.into(),
            r#"{"kind":"editor","state":{},"version":"1"}"#.into(),
            r#"{"kind":"editor","state":{},"version":1,"extra":true}"#.into(),
            r#"{"kind":"editor","kind":"editor","state":{},"version":1}"#.into(),
            r#"{"kind":"editor","state":{},"state":{},"version":1}"#.into(),
        ] {
            assert_eq!(
                ViewState::from_canonical_json(&text, &editor_key(), &registry),
                Err(ViewStateError::InvalidStructure),
                "{text}"
            );
        }
        assert_eq!(
            ViewState::from_canonical_json(
                r#"{"kind":"terminal","state":{"persistent":true,"position":0},"version":1}"#,
                &ViewKey::Terminal {
                    project_id: project_id(),
                    terminal_id: TerminalId::from_str(TERMINAL).unwrap(),
                },
                &registry,
            ),
            Err(ViewStateError::UnregisteredKind)
        );

        for (error, message) in [
            (
                ViewStateError::InvalidStructure,
                "ViewState 持久信封结构非法",
            ),
            (ViewStateError::UnknownKind, "未知 ViewState 视图类型"),
            (
                ViewStateError::UnsupportedVersion,
                "不支持的 ViewState DTO 版本",
            ),
            (
                ViewStateError::UnregisteredKind,
                "视图未注册 ViewState schema",
            ),
            (
                ViewStateError::InvalidState,
                "ViewState 状态字段或领域值非法",
            ),
            (
                ViewStateError::KeyMismatch,
                "ViewState 与 ViewKey 身份不匹配",
            ),
            (ViewStateError::Encode, "ViewState 规范 JSON 编码失败"),
        ] {
            assert_eq!(error.to_string(), message);
        }
    }

    /// schema 直接解码原始状态，未知、重复、缺失与瞬态字段都不能落盘。
    #[test]
    fn view_state_schema_rejects_unregistered_and_transient_fields() {
        let registry = registry();
        for state in [
            r#"{"persistent":true}"#,
            r#"{"position":1}"#,
            r#"{"persistent":true,"position":1,"search":"secret"}"#,
            r#"{"persistent":true,"position":1,"focus":true}"#,
            r#"{"persistent":true,"persistent":false,"position":1}"#,
            r#"{"persistent":true,"position":1,"position":2}"#,
            r#"{"persistent":"yes","position":1}"#,
            r#"{"persistent":true,"position":-1}"#,
        ] {
            let text = format!(r#"{{"kind":"editor","state":{state},"version":1}}"#);
            assert_eq!(
                ViewState::from_canonical_json(&text, &editor_key(), &registry),
                Err(ViewStateError::InvalidState),
                "{text}"
            );
        }
        assert_eq!(
            ViewState::new(
                &editor_key(),
                json!({"persistent":true,"position":1,"menu":true}),
                &registry,
            ),
            Err(ViewStateError::InvalidState)
        );
        assert_eq!(
            ViewState::new(
                &ViewKey::Terminal {
                    project_id: project_id(),
                    terminal_id: TerminalId::from_str(TERMINAL).unwrap(),
                },
                json!({}),
                &registry,
            ),
            Err(ViewStateError::UnregisteredKind)
        );
        assert_eq!(InvalidViewState.to_string(), "视图状态字段或领域值非法");
    }

    /// 作用域完全由匹配的 ViewKey 推导，项目与应用状态不能错绑。
    #[test]
    fn view_state_scope_follows_the_matching_view_key() {
        let registry = registry();
        for key in all_view_keys() {
            if key.kind() == ViewKeyKind::Terminal {
                continue;
            }
            let state =
                ViewState::new(&key, json!({"persistent":true,"position":1}), &registry).unwrap();
            let expected = match key {
                ViewKey::SideView { .. } | ViewKey::AppSettingsOverlay => {
                    ViewStateScope::Application
                }
                _ => ViewStateScope::Project(project_id()),
            };
            assert_eq!(state.scope(&key), Ok(expected), "{}", key.kind_str());

            let mismatched = if key.kind() == ViewKeyKind::Editor {
                ViewKey::Markdown {
                    project_id: project_id(),
                    document_id: document_id(),
                }
            } else {
                editor_key()
            };
            assert_eq!(state.scope(&mismatched), Err(ViewStateError::KeyMismatch));
        }
    }

    /// 同一视图类型内的不同稳定身份不能交叉使用状态。
    #[test]
    fn view_state_scope_rejects_same_kind_different_identity() {
        let registry = registry();
        let nav = ViewKey::SideView {
            side_view_id: crate::SideViewId::Nav,
        };
        let explorer = ViewKey::SideView {
            side_view_id: crate::SideViewId::Explorer,
        };
        let state =
            ViewState::new(&nav, json!({"persistent":true,"position":1}), &registry).unwrap();

        assert_eq!(state.scope(&nav), Ok(ViewStateScope::Application));
        assert_eq!(state.scope(&explorer), Err(ViewStateError::KeyMismatch));
    }

    /// SideView 的内存状态按完整 ViewKey 选择精确 schema。
    #[test]
    fn side_view_schema_uses_view_key_for_memory_states() {
        let registry = side_view_registry();
        let nav = side_view_key(SideViewId::Nav);
        let explorer = side_view_key(SideViewId::Explorer);

        assert_eq!(
            ViewState::new(&nav, json!({"position":7}), &registry)
                .unwrap()
                .state(),
            &json!({"position":7})
        );
        assert_eq!(
            ViewState::new(&explorer, json!({"expanded":true}), &registry)
                .unwrap()
                .state(),
            &json!({"expanded":true})
        );
        assert_eq!(
            ViewState::new(&nav, json!({"expanded":true}), &registry),
            Err(ViewStateError::InvalidState)
        );
        assert_eq!(
            ViewState::new(&explorer, json!({"position":7}), &registry),
            Err(ViewStateError::InvalidState)
        );
    }

    /// SideView 的持久状态按完整 ViewKey 选择精确 schema。
    #[test]
    fn side_view_schema_uses_view_key_for_restored_states() {
        let registry = side_view_registry();
        let nav = side_view_key(SideViewId::Nav);
        let explorer = side_view_key(SideViewId::Explorer);
        let nav_json = r#"{"kind":"side_view","state":{"position":7},"version":1}"#;
        let explorer_json = r#"{"kind":"side_view","state":{"expanded":true},"version":1}"#;

        assert_eq!(
            ViewState::from_canonical_json(nav_json, &nav, &registry)
                .unwrap()
                .state(),
            &json!({"position":7})
        );
        assert_eq!(
            ViewState::from_canonical_json(explorer_json, &explorer, &registry)
                .unwrap()
                .state(),
            &json!({"expanded":true})
        );
        assert_eq!(
            ViewState::from_canonical_json(explorer_json, &nav, &registry),
            Err(ViewStateError::InvalidState)
        );
        assert_eq!(
            ViewState::from_canonical_json(nav_json, &explorer, &registry),
            Err(ViewStateError::InvalidState)
        );
    }

    /// 注册表拒绝同一视图类型的第二份 schema，不以后注册覆盖先注册。
    #[test]
    fn view_state_registry_rejects_duplicate_schema() {
        let error = ViewStateRegistry::new(&[
            ViewStateSchema::new(ViewKeyKind::Editor, decode_test_state),
            ViewStateSchema::new(ViewKeyKind::Editor, decode_test_state),
        ])
        .unwrap_err();
        assert_eq!(
            error,
            ViewStateRegistryError::DuplicateKind(ViewKeyKind::Editor)
        );
        assert_eq!(error.to_string(), "重复注册 ViewState schema：Editor");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// 任意完整合法状态均满足规范 JSON 往返与完整整数精度属性。
        #[test]
        fn view_state_properties_round_trip_canonical_json(
            persistent in any::<bool>(),
            position in any::<u64>(),
        ) {
            let registry = registry();
            let state = ViewState::new(
                &editor_key(),
                json!({"position":position,"persistent":persistent}),
                &registry,
            )
            .unwrap();
            prop_assert_eq!(
                state.state(),
                &json!({"persistent":persistent,"position":position})
            );
            let encoded = state.to_canonical_json().unwrap();
            let restored =
                ViewState::from_canonical_json(&encoded, &editor_key(), &registry).unwrap();
            prop_assert_eq!(&restored, &state);
            prop_assert_eq!(restored.to_canonical_json().unwrap(), encoded);
        }
    }
}
