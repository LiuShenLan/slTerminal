use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use serde_json::Value;
use slterm_document::{CommitId, DocumentId, RepoPath, RepositoryBinding};

use crate::repository_dto::{self, RepositoryBindingDto, RepositoryDtoError};
use crate::{ProjectId, SideViewId, TerminalId};

const VIEW_KEY_VERSION: u64 = 1;

/// 逻辑视图的封闭持久身份，不包含存活面板实例身份。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewKey {
    Terminal {
        project_id: ProjectId,
        terminal_id: TerminalId,
    },
    Editor {
        project_id: ProjectId,
        document_id: DocumentId,
    },
    Markdown {
        project_id: ProjectId,
        document_id: DocumentId,
    },
    Diff {
        project_id: ProjectId,
        document_id: DocumentId,
    },
    GitShow {
        project_id: ProjectId,
        repository_binding: Box<RepositoryBinding>,
        commit_id: CommitId,
        repo_path: RepoPath,
    },
    Settings {
        project_id: ProjectId,
    },
    SideView {
        side_view_id: SideViewId,
    },
    AppSettingsOverlay,
}

/// ViewKey 持久 DTO 的编码或恢复错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKeyError {
    /// JSON、字段集合或领域值不符合当前结构。
    InvalidStructure,
    /// 类型名未进入封闭注册点。
    UnknownKind,
    /// DTO 版本不是当前版本。
    UnsupportedVersion,
    /// 路径无法无损表示为 UTF-8 JSON 字符串。
    NonUnicodePath,
    /// 已构造的规范 JSON 值无法编码。
    Encode,
}

impl From<RepositoryDtoError> for ViewKeyError {
    fn from(error: RepositoryDtoError) -> Self {
        match error {
            RepositoryDtoError::InvalidStructure => Self::InvalidStructure,
            RepositoryDtoError::NonUnicodePath => Self::NonUnicodePath,
            RepositoryDtoError::Encode => Self::Encode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewKeyKind {
    Terminal,
    Editor,
    Markdown,
    Diff,
    GitShow,
    Settings,
    SideView,
    AppSettingsOverlay,
}

impl ViewKeyKind {
    const ALL: [Self; 8] = [
        Self::Terminal,
        Self::Editor,
        Self::Markdown,
        Self::Diff,
        Self::GitShow,
        Self::Settings,
        Self::SideView,
        Self::AppSettingsOverlay,
    ];

    const fn id_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Editor => "editor",
            Self::Markdown => "markdown",
            Self::Diff => "diff",
            Self::GitShow => "git_show",
            Self::Settings => "settings",
            Self::SideView => "side_view",
            Self::AppSettingsOverlay => "app_settings_overlay",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.id_str() == text)
    }
}

impl ViewKey {
    fn kind(&self) -> ViewKeyKind {
        match self {
            Self::Terminal { .. } => ViewKeyKind::Terminal,
            Self::Editor { .. } => ViewKeyKind::Editor,
            Self::Markdown { .. } => ViewKeyKind::Markdown,
            Self::Diff { .. } => ViewKeyKind::Diff,
            Self::GitShow { .. } => ViewKeyKind::GitShow,
            Self::Settings { .. } => ViewKeyKind::Settings,
            Self::SideView { .. } => ViewKeyKind::SideView,
            Self::AppSettingsOverlay => ViewKeyKind::AppSettingsOverlay,
        }
    }

    /// 返回封闭注册点中的规范类型名。
    pub fn kind_str(&self) -> &'static str {
        self.kind().id_str()
    }

    fn to_dto(&self) -> Result<BTreeMap<&'static str, Value>, ViewKeyError> {
        let mut object = BTreeMap::from([
            ("kind", Value::String(self.kind().id_str().to_owned())),
            ("version", Value::from(VIEW_KEY_VERSION)),
        ]);
        match self {
            Self::Terminal {
                project_id,
                terminal_id,
            } => {
                object.insert("project_id", Value::String(project_id.to_string()));
                object.insert("terminal_id", Value::String(terminal_id.to_string()));
            }
            Self::Editor {
                project_id,
                document_id,
            }
            | Self::Markdown {
                project_id,
                document_id,
            }
            | Self::Diff {
                project_id,
                document_id,
            } => {
                object.insert("project_id", Value::String(project_id.to_string()));
                object.insert("document_id", Value::String(document_id.to_string()));
            }
            Self::GitShow {
                project_id,
                repository_binding,
                commit_id,
                repo_path,
            } => {
                object.insert("commit_id", Value::String(commit_id.to_string()));
                object.insert("project_id", Value::String(project_id.to_string()));
                object.insert("repo_path", Value::String(repo_path.to_string()));
                object.insert(
                    "repository_binding",
                    repository_dto::to_value(repository_binding)?,
                );
            }
            Self::Settings { project_id } => {
                object.insert("project_id", Value::String(project_id.to_string()));
            }
            Self::SideView { side_view_id } => {
                object.insert("side_view_id", Value::String(side_view_id.to_string()));
            }
            Self::AppSettingsOverlay => {}
        }
        Ok(object)
    }

    /// 序列化为显式版本、确定键序且可逐字节重放的 JSON。
    pub fn to_canonical_json(&self) -> Result<String, ViewKeyError> {
        serde_json::to_string(&self.to_dto()?).map_err(|_| ViewKeyError::Encode)
    }

    /// 解析持久 JSON；未知类型与结构非法分别报告。
    pub fn from_canonical_json(text: &str) -> Result<Self, ViewKeyError> {
        let envelope: ViewKeyEnvelope =
            serde_json::from_str(text).map_err(|_| ViewKeyError::InvalidStructure)?;
        if envelope.version != VIEW_KEY_VERSION {
            return Err(ViewKeyError::UnsupportedVersion);
        }
        let kind = ViewKeyKind::parse(&envelope.kind).ok_or(ViewKeyError::UnknownKind)?;
        let dto: ViewKeyDto =
            serde_json::from_str(text).map_err(|_| ViewKeyError::InvalidStructure)?;
        dto.into_domain(kind)
    }
}

#[derive(Deserialize)]
struct ViewKeyEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewKeyDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    #[serde(default, deserialize_with = "deserialize_present")]
    project_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    terminal_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    document_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    repository_binding: Option<RepositoryBindingDto>,
    #[serde(default, deserialize_with = "deserialize_present")]
    commit_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    repo_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    side_view_id: Option<String>,
}

/// 区分字段缺失与显式 null：只有缺失由外层默认值表示。
fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

const PROJECT_FIELD: u16 = 1 << 0;
const TERMINAL_FIELD: u16 = 1 << 1;
const DOCUMENT_FIELD: u16 = 1 << 2;
const REPOSITORY_FIELD: u16 = 1 << 3;
const COMMIT_FIELD: u16 = 1 << 4;
const REPO_PATH_FIELD: u16 = 1 << 5;
const SIDE_VIEW_FIELD: u16 = 1 << 6;

impl ViewKeyDto {
    fn field_mask(&self) -> u16 {
        let mut mask = 0;
        for (present, field) in [
            (self.project_id.is_some(), PROJECT_FIELD),
            (self.terminal_id.is_some(), TERMINAL_FIELD),
            (self.document_id.is_some(), DOCUMENT_FIELD),
            (self.repository_binding.is_some(), REPOSITORY_FIELD),
            (self.commit_id.is_some(), COMMIT_FIELD),
            (self.repo_path.is_some(), REPO_PATH_FIELD),
            (self.side_view_id.is_some(), SIDE_VIEW_FIELD),
        ] {
            if present {
                mask |= field;
            }
        }
        mask
    }

    fn into_domain(self, kind: ViewKeyKind) -> Result<ViewKey, ViewKeyError> {
        let expected_fields = match kind {
            ViewKeyKind::Terminal => PROJECT_FIELD | TERMINAL_FIELD,
            ViewKeyKind::Editor | ViewKeyKind::Markdown | ViewKeyKind::Diff => {
                PROJECT_FIELD | DOCUMENT_FIELD
            }
            ViewKeyKind::GitShow => {
                PROJECT_FIELD | REPOSITORY_FIELD | COMMIT_FIELD | REPO_PATH_FIELD
            }
            ViewKeyKind::Settings => PROJECT_FIELD,
            ViewKeyKind::SideView => SIDE_VIEW_FIELD,
            ViewKeyKind::AppSettingsOverlay => 0,
        };
        if self.field_mask() != expected_fields {
            return Err(ViewKeyError::InvalidStructure);
        }

        let Self {
            project_id,
            terminal_id,
            document_id,
            repository_binding,
            commit_id,
            repo_path,
            side_view_id,
            ..
        } = self;
        match kind {
            ViewKeyKind::Terminal => Ok(ViewKey::Terminal {
                project_id: parse_required(project_id)?,
                terminal_id: parse_required(terminal_id)?,
            }),
            ViewKeyKind::Editor => Ok(ViewKey::Editor {
                project_id: parse_required(project_id)?,
                document_id: parse_required(document_id)?,
            }),
            ViewKeyKind::Markdown => Ok(ViewKey::Markdown {
                project_id: parse_required(project_id)?,
                document_id: parse_required(document_id)?,
            }),
            ViewKeyKind::Diff => Ok(ViewKey::Diff {
                project_id: parse_required(project_id)?,
                document_id: parse_required(document_id)?,
            }),
            ViewKeyKind::GitShow => Ok(ViewKey::GitShow {
                project_id: parse_required(project_id)?,
                repository_binding: Box::new(
                    repository_binding
                        .ok_or(ViewKeyError::InvalidStructure)?
                        .into_domain()?,
                ),
                commit_id: parse_required(commit_id)?,
                repo_path: parse_required(repo_path)?,
            }),
            ViewKeyKind::Settings => Ok(ViewKey::Settings {
                project_id: parse_required(project_id)?,
            }),
            ViewKeyKind::SideView => Ok(ViewKey::SideView {
                side_view_id: parse_required(side_view_id)?,
            }),
            ViewKeyKind::AppSettingsOverlay => Ok(ViewKey::AppSettingsOverlay),
        }
    }
}

fn parse_required<T>(value: Option<String>) -> Result<T, ViewKeyError>
where
    T: FromStr,
{
    value
        .ok_or(ViewKeyError::InvalidStructure)?
        .parse()
        .map_err(|_| ViewKeyError::InvalidStructure)
}

impl fmt::Display for ViewKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "ViewKey 持久结构非法",
            Self::UnknownKind => "未知 ViewKey 类型",
            Self::UnsupportedVersion => "不支持的 ViewKey DTO 版本",
            Self::NonUnicodePath => "ViewKey 路径必须可无损编码为 UTF-8",
            Self::Encode => "ViewKey 规范 JSON 编码失败",
        })
    }
}

impl std::error::Error for ViewKeyError {}

#[cfg(test)]
mod view_key_tests {
    use std::str::FromStr;

    use proptest::prelude::*;
    use slterm_document::{DocumentId, RepoPath};
    use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};
    use uuid::Uuid;

    use super::*;

    const PROJECT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const TERMINAL: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    const VALID_GIT_SHOW_JSON: &str = r#"{"commit_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","kind":"git_show","project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","repo_path":"dir/file.md","repository_binding":{"common_dir":{"identity":null,"path":"D:/git/common.git"},"git_dir":{"identity":{"file":"2","volume":7},"path":"D:/git/repo.git"},"head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","worktree_root":{"identity":{"file":"1","volume":7},"path":"D:/repo"}},"version":1}"#;

    fn project_id() -> ProjectId {
        ProjectId::from_str(PROJECT).unwrap()
    }

    fn terminal_id() -> TerminalId {
        TerminalId::from_str(TERMINAL).unwrap()
    }

    fn document_id() -> DocumentId {
        DocumentId::from_str(DOCUMENT).unwrap()
    }

    fn commit(nibble: char) -> CommitId {
        CommitId::from_str(&nibble.to_string().repeat(40)).unwrap()
    }

    fn repo_path() -> RepoPath {
        RepoPath::from_str("dir/file.md").unwrap()
    }

    fn resolved(path: &str, identity: Option<FileIdentity>) -> ResolvedPath {
        ResolvedPath::new(CanonicalPath::new(path).unwrap(), identity)
    }

    fn repository_with(
        worktree_file: u128,
        volume: u64,
        git_file: u128,
        include_common_identity: bool,
    ) -> RepositoryBinding {
        RepositoryBinding::new(
            resolved(
                "D:/repo",
                Some(FileIdentity {
                    volume,
                    file: worktree_file,
                }),
            ),
            resolved(
                "D:/git/repo.git",
                Some(FileIdentity {
                    volume,
                    file: git_file,
                }),
            ),
            resolved(
                "D:/git/common.git",
                include_common_identity.then_some(FileIdentity { volume, file: 3 }),
            ),
            commit('a'),
        )
    }

    fn sample_repository() -> RepositoryBinding {
        repository_with(1, 7, 2, false)
    }

    fn git_show(repository_binding: RepositoryBinding) -> ViewKey {
        ViewKey::GitShow {
            project_id: project_id(),
            repository_binding: Box::new(repository_binding),
            commit_id: commit('b'),
            repo_path: repo_path(),
        }
    }

    fn all_view_keys() -> [(ViewKey, &'static str); 8] {
        [
            (
                ViewKey::Terminal {
                    project_id: project_id(),
                    terminal_id: terminal_id(),
                },
                "terminal",
            ),
            (
                ViewKey::Editor {
                    project_id: project_id(),
                    document_id: document_id(),
                },
                "editor",
            ),
            (
                ViewKey::Markdown {
                    project_id: project_id(),
                    document_id: document_id(),
                },
                "markdown",
            ),
            (
                ViewKey::Diff {
                    project_id: project_id(),
                    document_id: document_id(),
                },
                "diff",
            ),
            (git_show(sample_repository()), "git_show"),
            (
                ViewKey::Settings {
                    project_id: project_id(),
                },
                "settings",
            ),
            (
                ViewKey::SideView {
                    side_view_id: SideViewId::AgentFiles,
                },
                "side_view",
            ),
            (ViewKey::AppSettingsOverlay, "app_settings_overlay"),
        ]
    }

    fn v7_uuid(mut value: u128) -> Uuid {
        let mut bytes = value.to_be_bytes();
        bytes[6] = (bytes[6] & 0x0f) | 0x70;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        value = u128::from_be_bytes(bytes);
        Uuid::from_u128(value)
    }

    fn uuid_strategy() -> impl Strategy<Value = Uuid> {
        any::<u128>().prop_map(v7_uuid)
    }

    fn view_key_strategy() -> impl Strategy<Value = ViewKey> {
        prop_oneof![
            (uuid_strategy(), uuid_strategy()).prop_map(|(project, terminal)| {
                ViewKey::Terminal {
                    project_id: ProjectId::from_uuid(project).unwrap(),
                    terminal_id: TerminalId::from_uuid(terminal).unwrap(),
                }
            }),
            (uuid_strategy(), uuid_strategy()).prop_map(|(project, document)| {
                ViewKey::Editor {
                    project_id: ProjectId::from_uuid(project).unwrap(),
                    document_id: DocumentId::from_uuid(document).unwrap(),
                }
            }),
            (uuid_strategy(), uuid_strategy()).prop_map(|(project, document)| {
                ViewKey::Markdown {
                    project_id: ProjectId::from_uuid(project).unwrap(),
                    document_id: DocumentId::from_uuid(document).unwrap(),
                }
            }),
            (uuid_strategy(), uuid_strategy()).prop_map(|(project, document)| {
                ViewKey::Diff {
                    project_id: ProjectId::from_uuid(project).unwrap(),
                    document_id: DocumentId::from_uuid(document).unwrap(),
                }
            }),
            (
                uuid_strategy(),
                any::<u128>(),
                any::<u64>(),
                any::<u128>(),
                any::<bool>(),
            )
                .prop_map(|(project, worktree, volume, git, common)| {
                    ViewKey::GitShow {
                        project_id: ProjectId::from_uuid(project).unwrap(),
                        repository_binding: Box::new(repository_with(
                            worktree, volume, git, common,
                        )),
                        commit_id: commit('b'),
                        repo_path: repo_path(),
                    }
                }),
            uuid_strategy().prop_map(|project| ViewKey::Settings {
                project_id: ProjectId::from_uuid(project).unwrap(),
            }),
            proptest::sample::select(SideViewId::ALL.as_slice())
                .prop_map(|side_view_id| ViewKey::SideView { side_view_id }),
            Just(ViewKey::AppSettingsOverlay),
        ]
    }

    fn registered_kind(kind: &str) -> bool {
        matches!(
            kind,
            "terminal"
                | "editor"
                | "markdown"
                | "diff"
                | "git_show"
                | "settings"
                | "side_view"
                | "app_settings_overlay"
        )
    }

    /// 封闭注册表中的每种视图都保留身份并可逐字节重放。
    #[test]
    fn view_key_registry_round_trips_every_closed_variant() {
        for (key, kind) in all_view_keys() {
            assert_eq!(key.kind_str(), kind);
            let json = key.to_canonical_json().unwrap();
            let restored = ViewKey::from_canonical_json(&json).unwrap();
            assert_eq!(restored, key);
            assert_eq!(restored.to_canonical_json().unwrap(), json);
            assert!(!json.contains("panel_id"));
            assert!(!json.contains("instance_epoch"));
        }
    }

    /// 规范输出使用版本、字段名和 BTreeMap 键序，不依赖输入键序。
    #[test]
    fn view_key_canonical_json_has_stable_version_fields_and_key_order() {
        assert_eq!(
            ViewKey::AppSettingsOverlay.to_canonical_json().unwrap(),
            r#"{"kind":"app_settings_overlay","version":1}"#
        );
        let terminal = ViewKey::Terminal {
            project_id: project_id(),
            terminal_id: terminal_id(),
        };
        let expected = format!(
            r#"{{"kind":"terminal","project_id":"{PROJECT}","terminal_id":"{TERMINAL}","version":1}}"#
        );
        assert_eq!(terminal.to_canonical_json().unwrap(), expected);

        let reordered = format!(
            r#"{{ "terminal_id": "{TERMINAL}", "version": 1, "kind": "terminal", "project_id": "{PROJECT}" }}"#
        );
        assert_eq!(
            ViewKey::from_canonical_json(&reordered)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            expected
        );
    }

    /// GitShow 固定仓库、提交与路径，绑定中的可空身份完整往返。
    #[test]
    fn view_key_git_show_uses_the_canonical_repository_dto() {
        let restored = ViewKey::from_canonical_json(VALID_GIT_SHOW_JSON).unwrap();
        assert_eq!(restored, git_show(sample_repository()));
        assert_eq!(restored.to_canonical_json().unwrap(), VALID_GIT_SHOW_JSON);
    }

    /// 文件身份使用十进制文本保存完整 u128，避免 JSON 数值精度边界。
    #[test]
    fn view_key_preserves_the_full_file_identity_range() {
        let key = git_show(repository_with(u128::MAX, u64::MAX, 9, true));
        let json = key.to_canonical_json().unwrap();
        assert!(json.contains(&format!(r#""file":"{}""#, u128::MAX)));
        assert!(json.contains(&format!(r#""volume":{}"#, u64::MAX)));
        assert_eq!(ViewKey::from_canonical_json(&json).unwrap(), key);
    }

    /// 未知类型先按类型分类，更高版本与结构损坏使用不同错误。
    #[test]
    fn view_key_distinguishes_unknown_kinds_versions_and_invalid_structure() {
        for text in [
            r#"{"kind":"future_view","version":1}"#,
            r#"{"kind":"future_view","version":1,"anything":null}"#,
        ] {
            assert_eq!(
                ViewKey::from_canonical_json(text),
                Err(ViewKeyError::UnknownKind),
                "{text}"
            );
        }
        for version in [0, 2, u64::MAX] {
            let text = format!(r#"{{"kind":"terminal","version":{version}}}"#);
            assert_eq!(
                ViewKey::from_canonical_json(&text),
                Err(ViewKeyError::UnsupportedVersion)
            );
        }
        for text in [
            "",
            "{",
            "[]",
            r#"{"kind":"terminal"}"#,
            r#"{"version":1}"#,
            r#"{"kind":"terminal","version":"1"}"#,
            r#"{"kind":"terminal","kind":"editor","version":1}"#,
        ] {
            assert_eq!(
                ViewKey::from_canonical_json(text),
                Err(ViewKeyError::InvalidStructure),
                "{text}"
            );
        }
    }

    /// 已知类型只接受本变体字段；未知字段、null 与错配字段均拒绝。
    #[test]
    fn view_key_rejects_unknown_missing_and_mismatched_fields() {
        let cases = [
            format!(r#"{{"kind":"terminal","project_id":"{PROJECT}","version":1}}"#),
            format!(
                r#"{{"document_id":"{DOCUMENT}","kind":"terminal","project_id":"{PROJECT}","terminal_id":"{TERMINAL}","version":1}}"#
            ),
            format!(
                r#"{{"kind":"terminal","project_id":"{PROJECT}","terminal_id":null,"version":1}}"#
            ),
            format!(
                r#"{{"kind":"terminal","project_id":"{PROJECT}","terminal_id":"{TERMINAL}","unknown":true,"version":1}}"#
            ),
            r#"{"kind":"settings","project_id":null,"version":1}"#.into(),
            format!(r#"{{"kind":"app_settings_overlay","project_id":"{PROJECT}","version":1}}"#),
            r#"{"kind":"side_view","side_view_id":"missing","version":1}"#.into(),
            VALID_GIT_SHOW_JSON.replace(
                r#""worktree_root":{"identity""#,
                r#""worktree_root":{"unexpected":0,"identity""#,
            ),
            VALID_GIT_SHOW_JSON.replace(
                r#""common_dir":{"identity":null,"path""#,
                r#""common_dir":{"path""#,
            ),
        ];
        for text in cases {
            assert_eq!(
                ViewKey::from_canonical_json(&text),
                Err(ViewKeyError::InvalidStructure),
                "{text}"
            );
        }
    }

    /// 领域值在 DTO 边界重新校验，不能把非法 ID 或路径带入恢复。
    #[test]
    fn view_key_revalidates_ids_repository_paths_and_file_identities() {
        let v4 = "550e8400-e29b-41d4-a716-446655440000";
        let cases = [
            format!(
                r#"{{"document_id":"{DOCUMENT}","kind":"editor","project_id":"{v4}","version":1}}"#
            ),
            VALID_GIT_SHOW_JSON.replace(r#""repo_path":"dir/file.md""#, r#""repo_path":"../file""#),
            VALID_GIT_SHOW_JSON.replace(r#""path":"D:/repo""#, r#""path":"repo""#),
            VALID_GIT_SHOW_JSON.replace(r#""file":"1""#, r#""file":"01""#),
            VALID_GIT_SHOW_JSON.replace(r#""file":"1""#, r#""file":"-1""#),
            VALID_GIT_SHOW_JSON.replace(&"b".repeat(40), "refs/heads/main"),
        ];
        for text in cases {
            assert_eq!(
                ViewKey::from_canonical_json(&text),
                Err(ViewKeyError::InvalidStructure),
                "{text}"
            );
        }
    }

    /// 非 Unicode 路径不得经有损转换进入持久 JSON。
    #[test]
    fn view_key_serialization_rejects_non_unicode_paths_without_loss() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let original = sample_repository();
        let mut path = OsString::from("D:/");
        path.push(OsString::from_wide(&[0xd800]));
        let invalid = ResolvedPath::new(CanonicalPath::new(PathBuf::from(path)).unwrap(), None);
        let binding = RepositoryBinding::new(
            invalid,
            original.git_dir().clone(),
            original.common_dir().clone(),
            original.head().clone(),
        );
        assert_eq!(
            git_show(binding).to_canonical_json(),
            Err(ViewKeyError::NonUnicodePath)
        );
    }

    /// 错误文本稳定且不插值不可信持久内容。
    #[test]
    fn view_key_errors_describe_the_failure_without_echoing_input() {
        fn require_error<T: std::error::Error>() {}
        require_error::<ViewKeyError>();
        let cases = [
            (ViewKeyError::InvalidStructure, "ViewKey 持久结构非法"),
            (ViewKeyError::UnknownKind, "未知 ViewKey 类型"),
            (
                ViewKeyError::UnsupportedVersion,
                "不支持的 ViewKey DTO 版本",
            ),
            (
                ViewKeyError::NonUnicodePath,
                "ViewKey 路径必须可无损编码为 UTF-8",
            ),
            (ViewKeyError::Encode, "ViewKey 规范 JSON 编码失败"),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// 任意稳定 ID、文件身份与封闭变体都满足规范往返属性。
        #[test]
        fn view_key_properties_round_trip_without_panel_identity(key in view_key_strategy()) {
            let json = key.to_canonical_json().unwrap();
            let restored = ViewKey::from_canonical_json(&json).unwrap();
            prop_assert_eq!(&restored, &key);
            prop_assert_eq!(restored.to_canonical_json().unwrap(), json.clone());
            prop_assert!(!json.contains("panel_id"));
            prop_assert!(!json.contains("instance_epoch"));
        }

        /// 任意未登记类型都按未知类型拒绝，而非误判为结构损坏。
        #[test]
        fn view_key_properties_reject_unregistered_kinds(
            kind in "[a-z][a-z0-9_]{0,31}".prop_filter("已登记类型", |kind| !registered_kind(kind)),
        ) {
            let text = format!(r#"{{"kind":"{kind}","version":1}}"#);
            prop_assert_eq!(ViewKey::from_canonical_json(&text), Err(ViewKeyError::UnknownKind));
        }

        /// 任意 UUID 位形改成 v4 后均不得穿过持久 ID 边界。
        #[test]
        fn view_key_properties_reject_non_v7_identifier_bytes(value in any::<u128>()) {
            let mut bytes = value.to_be_bytes();
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            let invalid = Uuid::from_bytes(bytes).hyphenated().to_string();
            let text = format!(
                r#"{{"kind":"terminal","project_id":"{invalid}","terminal_id":"{TERMINAL}","version":1}}"#
            );
            prop_assert_eq!(ViewKey::from_canonical_json(&text), Err(ViewKeyError::InvalidStructure));
        }
    }
}
