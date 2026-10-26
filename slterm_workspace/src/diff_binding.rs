use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use slterm_document::DiffBinding;

use crate::repository_dto::{self, RepositoryBindingDto, RepositoryDtoError};

const DIFF_BINDING_VERSION: u64 = 1;
const DIFF_BINDING_KIND: &str = "diff_binding";

/// DiffBinding 持久 DTO 的编码或恢复错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffBindingError {
    /// JSON、字段集合或领域值不符合当前结构。
    InvalidStructure,
    /// 类型名不是固定 Diff 绑定。
    UnknownKind,
    /// DTO 版本不是当前版本。
    UnsupportedVersion,
    /// 路径无法无损表示为 UTF-8 JSON 字符串。
    NonUnicodePath,
    /// 已构造的规范 JSON 值无法编码。
    Encode,
}

impl From<RepositoryDtoError> for DiffBindingError {
    fn from(error: RepositoryDtoError) -> Self {
        match error {
            RepositoryDtoError::InvalidStructure => Self::InvalidStructure,
            RepositoryDtoError::NonUnicodePath => Self::NonUnicodePath,
            RepositoryDtoError::Encode => Self::Encode,
        }
    }
}

/// 将固定 Diff 绑定序列化为独立版本的规范 JSON。
pub fn diff_binding_to_canonical_json(binding: &DiffBinding) -> Result<String, DiffBindingError> {
    let object = BTreeMap::from([
        ("head_path", Value::String(binding.head_path().to_string())),
        ("kind", Value::String(DIFF_BINDING_KIND.to_owned())),
        (
            "repository_binding",
            repository_dto::to_value(binding.repository_binding())?,
        ),
        ("version", Value::from(DIFF_BINDING_VERSION)),
    ]);
    serde_json::to_string(&object).map_err(|_| DiffBindingError::Encode)
}

/// 从独立版本的规范 JSON 恢复固定 Diff 绑定。
pub fn diff_binding_from_canonical_json(text: &str) -> Result<DiffBinding, DiffBindingError> {
    let envelope: DiffBindingEnvelope =
        serde_json::from_str(text).map_err(|_| DiffBindingError::InvalidStructure)?;
    if envelope.version != DIFF_BINDING_VERSION {
        return Err(DiffBindingError::UnsupportedVersion);
    }
    if envelope.kind != DIFF_BINDING_KIND {
        return Err(DiffBindingError::UnknownKind);
    }
    let dto: DiffBindingDto =
        serde_json::from_str(text).map_err(|_| DiffBindingError::InvalidStructure)?;
    dto.into_domain()
}

#[derive(Deserialize)]
struct DiffBindingEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiffBindingDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    head_path: String,
    repository_binding: RepositoryBindingDto,
}

impl DiffBindingDto {
    fn into_domain(self) -> Result<DiffBinding, DiffBindingError> {
        Ok(DiffBinding::new(
            self.repository_binding.into_domain()?,
            self.head_path
                .parse()
                .map_err(|_| DiffBindingError::InvalidStructure)?,
        ))
    }
}

impl fmt::Display for DiffBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "DiffBinding 持久结构非法",
            Self::UnknownKind => "未知 DiffBinding 类型",
            Self::UnsupportedVersion => "不支持的 DiffBinding DTO 版本",
            Self::NonUnicodePath => "DiffBinding 路径必须可无损编码为 UTF-8",
            Self::Encode => "DiffBinding 规范 JSON 编码失败",
        })
    }
}

impl std::error::Error for DiffBindingError {}

#[cfg(test)]
mod diff_binding_tests {
    use std::str::FromStr;

    use proptest::prelude::*;
    use slterm_document::{CommitId, RepoPath, RepositoryBinding};
    use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

    use super::*;

    const VALID_DIFF_BINDING_JSON: &str = r#"{"head_path":"dir/file.md","kind":"diff_binding","repository_binding":{"common_dir":{"identity":null,"path":"D:/git/common.git"},"git_dir":{"identity":{"file":"2","volume":7},"path":"D:/git/repo.git"},"head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","worktree_root":{"identity":{"file":"1","volume":7},"path":"D:/repo"}},"version":1}"#;
    const REPO_PATHS: [&str; 4] = [
        "README.md",
        ".gitignore",
        "dir/file name.md",
        "中文/文档.md",
    ];

    fn commit(nibble: char) -> CommitId {
        CommitId::from_str(&nibble.to_string().repeat(40)).unwrap()
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

    fn sample_binding() -> DiffBinding {
        DiffBinding::new(
            sample_repository(),
            RepoPath::from_str("dir/file.md").unwrap(),
        )
    }

    fn binding_strategy() -> impl Strategy<Value = DiffBinding> {
        (
            any::<u128>(),
            any::<u64>(),
            any::<u128>(),
            any::<bool>(),
            proptest::sample::select(REPO_PATHS.as_slice()),
        )
            .prop_map(|(worktree, volume, git, common, path)| {
                DiffBinding::new(
                    repository_with(worktree, volume, git, common),
                    RepoPath::from_str(path).unwrap(),
                )
            })
    }

    /// 独立 DTO 使用固定 kind、版本与确定键序，并可逐字节重放。
    #[test]
    fn diff_binding_canonical_json_round_trips_with_stable_fields() {
        let binding = sample_binding();
        assert_eq!(
            diff_binding_to_canonical_json(&binding).unwrap(),
            VALID_DIFF_BINDING_JSON
        );
        let restored = diff_binding_from_canonical_json(VALID_DIFF_BINDING_JSON).unwrap();
        assert_eq!(restored, binding);
        assert_eq!(
            diff_binding_to_canonical_json(&restored).unwrap(),
            VALID_DIFF_BINDING_JSON
        );

        let reordered = VALID_DIFF_BINDING_JSON
            .replace(r#"{"head_path""#, r#"{ "version": 1, "head_path""#)
            .replace(r#","version":1}"#, "}");
        assert_eq!(
            diff_binding_to_canonical_json(&diff_binding_from_canonical_json(&reordered).unwrap())
                .unwrap(),
            VALID_DIFF_BINDING_JSON
        );
    }

    /// 未知类型、版本不兼容和结构损坏分别报告。
    #[test]
    fn diff_binding_distinguishes_unknown_kinds_versions_and_invalid_structure() {
        for text in [
            r#"{"kind":"future_binding","version":1}"#,
            r#"{"kind":"future_binding","version":1,"anything":null}"#,
        ] {
            assert_eq!(
                diff_binding_from_canonical_json(text),
                Err(DiffBindingError::UnknownKind),
                "{text}"
            );
        }
        for version in [0, 2, u64::MAX] {
            let text = format!(r#"{{"kind":"diff_binding","version":{version}}}"#);
            assert_eq!(
                diff_binding_from_canonical_json(&text),
                Err(DiffBindingError::UnsupportedVersion)
            );
        }
        let cases: [String; 9] = [
            String::new(),
            "{".into(),
            "[]".into(),
            r#"{"kind":"diff_binding"}"#.into(),
            r#"{"version":1}"#.into(),
            r#"{"kind":"diff_binding","version":"1"}"#.into(),
            r#"{"kind":"diff_binding","version":1}"#.into(),
            VALID_DIFF_BINDING_JSON.replace(r#""version":1}"#, r#""version":1,"extra":true}"#),
            VALID_DIFF_BINDING_JSON.replace(r#""head_path":"dir/file.md""#, r#""head_path":null"#),
        ];
        for text in cases {
            assert_eq!(
                diff_binding_from_canonical_json(&text),
                Err(DiffBindingError::InvalidStructure),
                "{text}"
            );
        }
    }

    /// 恢复时重新校验 HEAD、固定旧路径、规范目录和文件身份。
    #[test]
    fn diff_binding_revalidates_repository_and_head_path() {
        for text in [
            VALID_DIFF_BINDING_JSON
                .replace(r#""head_path":"dir/file.md""#, r#""head_path":"../file""#),
            VALID_DIFF_BINDING_JSON.replace(r#""path":"D:/repo""#, r#""path":"repo""#),
            VALID_DIFF_BINDING_JSON.replace(&"a".repeat(40), "refs/heads/main"),
            VALID_DIFF_BINDING_JSON.replace(r#""file":"1""#, r#""file":"01""#),
            VALID_DIFF_BINDING_JSON.replace(
                r#""common_dir":{"identity":null,"path""#,
                r#""common_dir":{"path""#,
            ),
        ] {
            assert_eq!(
                diff_binding_from_canonical_json(&text),
                Err(DiffBindingError::InvalidStructure),
                "{text}"
            );
        }
    }

    /// 完整文件身份范围不经过 JSON 浮点或有损整数转换。
    #[test]
    fn diff_binding_preserves_the_full_file_identity_range() {
        let binding = DiffBinding::new(
            repository_with(u128::MAX, u64::MAX, 9, true),
            RepoPath::from_str("dir/file.md").unwrap(),
        );
        let json = diff_binding_to_canonical_json(&binding).unwrap();
        assert!(json.contains(&format!(r#""file":"{}""#, u128::MAX)));
        assert!(json.contains(&format!(r#""volume":{}"#, u64::MAX)));
        assert_eq!(diff_binding_from_canonical_json(&json).unwrap(), binding);
    }

    /// 非 Unicode 仓库路径编码失败，不做替换字符转换。
    #[test]
    fn diff_binding_serialization_rejects_non_unicode_paths_without_loss() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let original = sample_repository();
        let mut path = OsString::from("D:/");
        path.push(OsString::from_wide(&[0xd800]));
        let invalid = ResolvedPath::new(CanonicalPath::new(PathBuf::from(path)).unwrap(), None);
        let repository = RepositoryBinding::new(
            invalid,
            original.git_dir().clone(),
            original.common_dir().clone(),
            original.head().clone(),
        );
        let binding = DiffBinding::new(repository, RepoPath::from_str("dir/file.md").unwrap());
        assert_eq!(
            diff_binding_to_canonical_json(&binding),
            Err(DiffBindingError::NonUnicodePath)
        );
    }

    /// 错误文本稳定，不回显不可信 JSON 或路径内容。
    #[test]
    fn diff_binding_errors_describe_the_failure_without_echoing_input() {
        fn require_error<T: std::error::Error>() {}
        require_error::<DiffBindingError>();
        let cases = [
            (
                DiffBindingError::InvalidStructure,
                "DiffBinding 持久结构非法",
            ),
            (DiffBindingError::UnknownKind, "未知 DiffBinding 类型"),
            (
                DiffBindingError::UnsupportedVersion,
                "不支持的 DiffBinding DTO 版本",
            ),
            (
                DiffBindingError::NonUnicodePath,
                "DiffBinding 路径必须可无损编码为 UTF-8",
            ),
            (DiffBindingError::Encode, "DiffBinding 规范 JSON 编码失败"),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// 任意文件身份与规范旧路径都满足独立 DTO 往返属性。
        #[test]
        fn diff_binding_properties_round_trip_canonical_json(binding in binding_strategy()) {
            let json = diff_binding_to_canonical_json(&binding).unwrap();
            let restored = diff_binding_from_canonical_json(&json).unwrap();
            prop_assert_eq!(&restored, &binding);
            prop_assert_eq!(diff_binding_to_canonical_json(&restored).unwrap(), json);
        }
    }
}
