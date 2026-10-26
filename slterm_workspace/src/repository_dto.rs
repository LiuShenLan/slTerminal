use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use slterm_document::RepositoryBinding;
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

/// 仓库绑定共享 DTO 的内部错误，由所属持久对象映射为公开错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RepositoryDtoError {
    InvalidStructure,
    NonUnicodePath,
    Encode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryBindingDto {
    worktree_root: RepositoryPathDto,
    git_dir: RepositoryPathDto,
    common_dir: RepositoryPathDto,
    head: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryPathDto {
    path: String,
    #[serde(deserialize_with = "deserialize_nullable")]
    identity: Option<FileIdentityDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileIdentityDto {
    volume: u64,
    file: String,
}

/// 可空字段仍必须出现，避免缺失字段被静默当成空身份。
fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl RepositoryBindingDto {
    pub(crate) fn into_domain(self) -> Result<RepositoryBinding, RepositoryDtoError> {
        Ok(RepositoryBinding::new(
            self.worktree_root.into_domain()?,
            self.git_dir.into_domain()?,
            self.common_dir.into_domain()?,
            self.head
                .parse()
                .map_err(|_| RepositoryDtoError::InvalidStructure)?,
        ))
    }
}

impl RepositoryPathDto {
    fn into_domain(self) -> Result<ResolvedPath, RepositoryDtoError> {
        let canonical_path =
            CanonicalPath::new(self.path).map_err(|_| RepositoryDtoError::InvalidStructure)?;
        let identity = self
            .identity
            .map(FileIdentityDto::into_domain)
            .transpose()?;
        Ok(ResolvedPath::new(canonical_path, identity))
    }
}

impl FileIdentityDto {
    fn into_domain(self) -> Result<FileIdentity, RepositoryDtoError> {
        let file = self
            .file
            .parse::<u128>()
            .map_err(|_| RepositoryDtoError::InvalidStructure)?;
        if self.file != file.to_string() {
            return Err(RepositoryDtoError::InvalidStructure);
        }
        Ok(FileIdentity {
            volume: self.volume,
            file,
        })
    }
}

fn json_object(
    entries: impl IntoIterator<Item = (&'static str, Value)>,
) -> Result<Value, RepositoryDtoError> {
    let ordered: BTreeMap<_, _> = entries.into_iter().collect();
    serde_json::to_value(ordered).map_err(|_| RepositoryDtoError::Encode)
}

fn resolved_path_to_value(path: &ResolvedPath) -> Result<Value, RepositoryDtoError> {
    let path_text = path
        .canonical_path()
        .as_path()
        .to_str()
        .ok_or(RepositoryDtoError::NonUnicodePath)?;
    let identity = match path.file_identity() {
        Some(identity) => json_object([
            ("file", Value::String(identity.file.to_string())),
            ("volume", Value::from(identity.volume)),
        ])?,
        None => Value::Null,
    };
    json_object([
        ("identity", identity),
        ("path", Value::String(path_text.to_owned())),
    ])
}

pub(crate) fn to_value(binding: &RepositoryBinding) -> Result<Value, RepositoryDtoError> {
    json_object([
        ("common_dir", resolved_path_to_value(binding.common_dir())?),
        ("git_dir", resolved_path_to_value(binding.git_dir())?),
        ("head", Value::String(binding.head().to_string())),
        (
            "worktree_root",
            resolved_path_to_value(binding.worktree_root())?,
        ),
    ])
}
