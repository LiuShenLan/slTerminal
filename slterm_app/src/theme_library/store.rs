//! 自定义主题库持久化。
//!
//! 内置主题保留为 `slterm-settings` 的虚拟值;只有用户主题是文件。store
//! 刻意同步且走冷路径:调用方在后台执行器运行,并持有返回的
//! `ThemeDocument`/`ThemeDefinition` 供渲染,不回读文件。
//!
//! 乐观并发(`RevisionPrecondition`):保存对照编辑动作前观察到的修订
//! 或整份快照;外部写入者不会被静默覆盖。目录与文件名去品牌:
//! 数据目录 `themes/`,文件后缀 `.slterm-theme.json`。

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use slterm_settings::ThemeName;

use super::document::{DocumentError, MAX_DOCUMENT_BYTES, ThemeDocument, builtin_documents};

/// 自定义主题文件数上限(契约数值):超限拒绝新建。
pub const MAX_CUSTOM_THEMES: usize = 128;
/// 自定义主题文件后缀。
pub const THEME_FILE_SUFFIX: &str = ".slterm-theme.json";

/// 保存前置条件:只允许在观测到的事实上写入。四态由 06 篇锚定;
/// 生产编辑器当前用 `Absent`/`Unchanged` 两态,`Exact`/`Any` 由测试与
/// 后续导入器消费。
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevisionPrecondition {
    /// 仅当该 id 尚无文件时创建。
    Absent,
    /// 仅当当前文档修订号等于该值时更新。
    Exact(u64),
    /// 允许创建或更新,修订链由 store 续写。
    Any,
    /// 要求整份文档快照未变——能抓住「修订号相同但内容被外部改过」。
    Unchanged(ThemeDocument),
}

/// 主题库错误。
#[derive(Debug)]
pub enum StoreError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Document(DocumentError),
    InvalidId(String),
    InvalidDocument(String),
    NotFound(String),
    BuiltinReadOnly(String),
    Busy,
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    Limit {
        limit: usize,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "theme store I/O at {}: {source}", path.display())
            }
            Self::Document(error) => error.fmt(f),
            Self::InvalidId(id) => write!(f, "invalid theme id: {id}"),
            Self::InvalidDocument(message) => write!(f, "invalid stored theme: {message}"),
            Self::NotFound(id) => write!(f, "theme not found: {id}"),
            Self::BuiltinReadOnly(id) => write!(f, "built-in theme is read-only: {id}"),
            Self::Busy => f.write_str("theme library is busy; retry the operation"),
            Self::Conflict { expected, actual } => {
                write!(
                    f,
                    "theme revision conflict: expected {expected:?}, found {actual:?}"
                )
            }
            Self::Limit { limit } => write!(f, "custom theme limit reached ({limit})"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DocumentError> for StoreError {
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

/// 单个损坏/跳过条目的诊断(不阻断整个目录列表)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

/// 目录快照:内置 + 自定义 + 诊断。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeLibrarySnapshot {
    pub builtins: Vec<ThemeDocument>,
    pub custom: Vec<ThemeDocument>,
    pub diagnostics: Vec<StoreDiagnostic>,
}

/// 主题库文件根。
#[derive(Clone, Debug)]
pub struct ThemeLibraryStore {
    root: PathBuf,
}

impl Default for ThemeLibraryStore {
    fn default() -> Self {
        // 开放问题 2 / D3:数据目录下 `themes/` 子目录,文件名不带品牌。
        Self::new(slterm_settings::settings_dir().join("themes"))
    }
}

impl ThemeLibraryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 某自定义主题的精确路径(诊断/测试共用同一构造规则)。
    pub fn path_for(&self, id: &str) -> Result<PathBuf, StoreError> {
        validate_id(id)?;
        Ok(self.root.join(format!("{id}{THEME_FILE_SUFFIX}")))
    }

    /// 扫描目录:内置文档恒在;自定义按名排序;单个坏文件只进诊断。
    pub fn list(&self) -> Result<ThemeLibrarySnapshot, StoreError> {
        let mut snapshot = ThemeLibrarySnapshot {
            builtins: builtin_documents(),
            ..ThemeLibrarySnapshot::default()
        };
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(snapshot),
            Err(error) => return Err(io_error(&self.root, error)),
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    snapshot.diagnostics.push(StoreDiagnostic {
                        path: self.root.clone(),
                        message: format!("unable to inspect theme entry: {error}"),
                    });
                    continue;
                }
            };
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(id) = file_name.strip_suffix(THEME_FILE_SUFFIX) else {
                continue;
            };
            if let Err(error) = validate_id(id) {
                snapshot.diagnostics.push(StoreDiagnostic {
                    path,
                    message: error.to_string(),
                });
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    snapshot.diagnostics.push(StoreDiagnostic {
                        path,
                        message: format!("unable to inspect file type: {error}"),
                    });
                    continue;
                }
            };
            if file_type.is_symlink() {
                snapshot.diagnostics.push(StoreDiagnostic {
                    path,
                    message: "symbolic links are not accepted as theme files".to_owned(),
                });
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            match read_document(&path, id) {
                Ok(Some(document)) => snapshot.custom.push(document),
                Ok(None) => snapshot.diagnostics.push(StoreDiagnostic {
                    path,
                    message: "theme file disappeared while scanning".to_owned(),
                }),
                Err(error) => snapshot.diagnostics.push(StoreDiagnostic {
                    path,
                    message: error.to_string(),
                }),
            }
        }

        snapshot.custom.sort_by(|left, right| {
            left.name()
                .cmp(right.name())
                .then_with(|| left.id().cmp(&right.id()))
        });
        Ok(snapshot)
    }

    /// 按 id 或显示名加载;内置主题可直接命中。
    pub fn load(&self, id: &str) -> Result<ThemeDocument, StoreError> {
        if let Some(document) = builtin_documents()
            .into_iter()
            .find(|document| document.id() == Some(id) || document.name() == id)
        {
            return Ok(document);
        }
        let path = self.path_for(id)?;
        read_document(&path, id)?.ok_or_else(|| StoreError::NotFound(id.to_owned()))
    }

    /// 条件保存:修订/快照不匹配即 `Conflict`,绝不清默覆盖。
    pub fn save(
        &self,
        document: &ThemeDocument,
        precondition: RevisionPrecondition,
    ) -> Result<ThemeDocument, StoreError> {
        if document.is_builtin() {
            return Err(StoreError::BuiltinReadOnly(document.name().to_owned()));
        }
        let id = document
            .id()
            .ok_or_else(|| StoreError::InvalidDocument("custom themes require an id".to_owned()))?;
        let path = self.path_for(id)?;
        // 取锁前先解析一次:全部运行时标量语义已校验,持锁期间不做 JSON 工作。
        document.definition()?;
        let _global_lock = self.acquire_global_lock()?;
        let _target_lock = self.acquire_target_lock(&path)?;
        let current = read_document(&path, id)?;
        let actual = current.as_ref().map(ThemeDocument::revision);
        let expected = match precondition {
            RevisionPrecondition::Absent => {
                if current.is_some() {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual,
                    });
                }
                None
            }
            RevisionPrecondition::Exact(expected) => {
                if actual != Some(expected) {
                    return Err(StoreError::Conflict {
                        expected: Some(expected),
                        actual,
                    });
                }
                Some(expected)
            }
            RevisionPrecondition::Any => actual,
            RevisionPrecondition::Unchanged(expected_document) => {
                if expected_document.id() != Some(id) {
                    return Err(StoreError::InvalidDocument(
                        "unchanged precondition belongs to another theme id".to_owned(),
                    ));
                }
                if current.as_ref() != Some(&expected_document) {
                    return Err(StoreError::Conflict {
                        expected: Some(expected_document.revision()),
                        actual,
                    });
                }
                Some(expected_document.revision())
            }
        };

        if current.is_none() && self.custom_file_count()? >= MAX_CUSTOM_THEMES {
            return Err(StoreError::Limit {
                limit: MAX_CUSTOM_THEMES,
            });
        }
        let revision = expected.map_or(1, |revision| revision.saturating_add(1));
        let stored = document.with_id_and_revision(id.to_owned(), revision);
        let bytes = stored.to_json_bytes()?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(StoreError::Document(DocumentError::TooLarge {
                bytes: bytes.len(),
            }));
        }
        fs::create_dir_all(&self.root).map_err(|error| io_error(&self.root, error))?;
        // 持锁后复查:符号链接换入不能被原子替换成普通主题文件。
        reject_symlink(&path)?;
        crate::atomic_file::write(&path, &bytes).map_err(|error| io_error(&path, error))?;
        Ok(stored)
    }

    /// 克隆内置主题并立即保存:返回的自定义主题从修订 1 起。
    #[allow(dead_code)] // 消费点 = M6.5 主题库管理 UI(测试已用)。
    pub fn fork_builtin(&self, theme: ThemeName, name: &str) -> Result<ThemeDocument, StoreError> {
        let builtin = builtin_documents()
            .into_iter()
            .find(|document| document.name() == theme.prompt_name())
            .ok_or_else(|| StoreError::NotFound(theme.prompt_name().to_owned()))?;
        let snapshot = self.list()?;
        let name = unique_name(name, &snapshot)?;
        let id = self.unique_id(&name, &snapshot)?;
        let candidate = builtin.fork(id, name)?;
        self.save(&candidate, RevisionPrecondition::Absent)
    }

    /// 导入已校验文档为新自定义主题。原生 id 被忽略,导入不可能覆盖
    /// 既有文件;显示名冲突自动消歧。
    pub fn import(&self, document: &ThemeDocument) -> Result<ThemeDocument, StoreError> {
        let snapshot = self.list()?;
        let name = unique_name(document.name(), &snapshot)?;
        let id = self.unique_id(&name, &snapshot)?;
        let candidate = document.with_name(name)?.with_id_and_revision(id, 0);
        self.save(&candidate, RevisionPrecondition::Absent)
    }

    /// 条件删除:修订不匹配拒绝(外部编辑不可被静默删掉)。
    #[allow(dead_code)] // 消费点 = M6.5 主题库管理 UI(测试已用)。
    pub fn delete(&self, id: &str, expected_revision: u64) -> Result<(), StoreError> {
        if builtin_documents()
            .into_iter()
            .any(|document| document.id() == Some(id) || document.name() == id)
        {
            return Err(StoreError::BuiltinReadOnly(id.to_owned()));
        }
        let path = self.path_for(id)?;
        let _global_lock = self.acquire_global_lock()?;
        let _target_lock = self.acquire_target_lock(&path)?;
        let current = read_document(&path, id)?;
        let Some(document) = current else {
            return Err(StoreError::Conflict {
                expected: Some(expected_revision),
                actual: None,
            });
        };
        if document.revision() != expected_revision {
            return Err(StoreError::Conflict {
                expected: Some(expected_revision),
                actual: Some(document.revision()),
            });
        }
        fs::remove_file(&path).map_err(|error| io_error(&path, error))
    }

    fn acquire_global_lock(&self) -> Result<crate::atomic_file::LifetimeFileLock, StoreError> {
        let path = self.root.join(".slterm-theme-library");
        crate::atomic_file::try_lifetime_lock(&path)
            .map_err(|error| io_error(&path, error))?
            .ok_or(StoreError::Busy)
    }

    fn acquire_target_lock(
        &self,
        path: &Path,
    ) -> Result<crate::atomic_file::LifetimeFileLock, StoreError> {
        crate::atomic_file::try_lifetime_lock(path)
            .map_err(|error| io_error(path, error))?
            .ok_or(StoreError::Busy)
    }

    fn custom_file_count(&self) -> Result<usize, StoreError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(io_error(&self.root, error)),
        };
        let mut count = 0;
        for entry in entries {
            let entry = entry.map_err(|error| io_error(&self.root, error))?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(id) = file_name.strip_suffix(THEME_FILE_SUFFIX) else {
                continue;
            };
            if validate_id(id).is_err()
                || entry
                    .file_type()
                    .map_err(|error| io_error(&path, error))?
                    .is_symlink()
            {
                continue;
            }
            count += 1;
        }
        Ok(count)
    }

    fn unique_id(&self, name: &str, snapshot: &ThemeLibrarySnapshot) -> Result<String, StoreError> {
        let base = slug(name);
        for suffix in 1..=MAX_CUSTOM_THEMES + 1 {
            let candidate = if suffix == 1 {
                base.clone()
            } else {
                format!("{base}-{suffix}")
            };
            if validate_id(&candidate).is_err() {
                continue;
            }
            if snapshot
                .custom
                .iter()
                .all(|document| document.id() != Some(candidate.as_str()))
                && !self.path_for(&candidate)?.exists()
            {
                return Ok(candidate);
            }
        }
        Err(StoreError::Limit {
            limit: MAX_CUSTOM_THEMES,
        })
    }
}

fn read_document(path: &Path, expected_id: &str) -> Result<Option<ThemeDocument>, StoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(StoreError::InvalidDocument(format!(
            "symbolic links are not accepted: {}",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(StoreError::InvalidDocument(format!(
            "theme path is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(StoreError::Document(DocumentError::TooLarge {
            bytes: metadata.len() as usize,
        }));
    }
    let mut file = fs::File::open(path).map_err(|error| io_error(path, error))?;
    if fs::symlink_metadata(path)
        .map_err(|error| io_error(path, error))?
        .file_type()
        .is_symlink()
    {
        return Err(StoreError::InvalidDocument(format!(
            "symbolic links are not accepted: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(StoreError::Document(DocumentError::TooLarge {
            bytes: bytes.len(),
        }));
    }
    let document = ThemeDocument::from_json_bytes(&bytes)?;
    if document.is_builtin() {
        return Err(StoreError::InvalidDocument(
            "custom theme files cannot be marked builtin".to_owned(),
        ));
    }
    if document.id() != Some(expected_id) {
        return Err(StoreError::InvalidDocument(format!(
            "document id {:?} does not match file id {expected_id}",
            document.id()
        )));
    }
    Ok(Some(document))
}

fn reject_symlink(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(StoreError::InvalidDocument(
            format!("symbolic links are not accepted: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
    }
}

fn validate_id(id: &str) -> Result<(), StoreError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(StoreError::InvalidId(id.to_owned()));
    }
    Ok(())
}

const MAX_THEME_NAME_BYTES: usize = 128;

/// 显示名去重:大小写不敏感;冲突时追加 ` (2)`、` (3)`…;长名先截断
/// 再拼后缀,防截断后碰撞。
fn unique_name(requested: &str, snapshot: &ThemeLibrarySnapshot) -> Result<String, StoreError> {
    let base = requested.trim();
    let base = if base.is_empty() {
        "Imported theme"
    } else {
        base
    };
    let occupied = |name: &str| {
        snapshot
            .builtins
            .iter()
            .chain(snapshot.custom.iter())
            .any(|document| document.name().eq_ignore_ascii_case(name))
    };
    for suffix_number in 0..=(MAX_CUSTOM_THEMES + 1) {
        let suffix = if suffix_number == 0 {
            String::new()
        } else {
            format!(" ({})", suffix_number + 1)
        };
        let base_budget = MAX_THEME_NAME_BYTES.saturating_sub(suffix.len());
        let candidate = format!("{}{}", truncate_name(base, base_budget), suffix);
        if !candidate.is_empty() && !occupied(&candidate) {
            return Ok(candidate);
        }
    }
    Err(StoreError::Limit {
        limit: MAX_CUSTOM_THEMES,
    })
}

fn truncate_name(value: &str, max_bytes: usize) -> String {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "theme".to_owned()
    } else {
        output.chars().take(128).collect()
    }
}

fn io_error(path: &Path, source: io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;
    use slterm_settings::ThemeDefinition;

    fn fixture() -> (tempfile::TempDir, ThemeLibraryStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = ThemeLibraryStore::new(directory.path().join("themes"));
        (directory, store)
    }

    fn custom_document(store: &ThemeLibraryStore, name: &str) -> ThemeDocument {
        let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        definition.name = name.to_owned();
        let document = super::super::document::from_definition(&definition).unwrap();
        store.import(&document).unwrap()
    }

    #[test]
    fn first_save_forks_and_revisions_increment() {
        let (_directory, store) = fixture();
        let forked = store.fork_builtin(ThemeName::Nord, "My Nord").unwrap();
        assert_eq!(forked.id(), Some("my-nord"));
        assert_eq!(forked.revision(), 1);
        assert!(!forked.is_builtin());
        assert!(store.path_for("my-nord").unwrap().exists());

        let updated = forked
            .with_name("My Nord 2")
            .unwrap()
            .with_id_and_revision("my-nord", 1);
        let stored = store
            .save(&updated, RevisionPrecondition::Unchanged(forked.clone()))
            .unwrap();
        assert_eq!(stored.revision(), 2);
        assert_eq!(store.load("my-nord").unwrap().revision(), 2);
        assert_eq!(store.load("my-nord").unwrap().name(), "My Nord 2");
    }

    #[test]
    fn external_edits_are_never_silently_overwritten() {
        let (_directory, store) = fixture();
        let forked = store.fork_builtin(ThemeName::Nord, "Guarded").unwrap();
        // 外部写入者把文件改成修订 2(同 id)。
        let external = forked
            .with_name("Guarded external")
            .unwrap()
            .with_id_and_revision("guarded", 2);
        store
            .save(&external, RevisionPrecondition::Exact(1))
            .unwrap();

        // 编辑器手里还是修订 1 的快照:整份快照与精确修订都应拒绝。
        let stale = forked.with_name("Guarded edit").unwrap();
        let error = store
            .save(&stale, RevisionPrecondition::Unchanged(forked.clone()))
            .unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                expected: Some(1),
                actual: Some(2)
            }
        ));
        let error = store
            .save(&stale, RevisionPrecondition::Exact(1))
            .unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                expected: Some(1),
                actual: Some(2)
            }
        ));
        assert_eq!(store.load("guarded").unwrap().name(), "Guarded external");

        // 外部编辑后再读出的快照允许继续保存。
        let current = store.load("guarded").unwrap();
        let next = current.with_name("Guarded next").unwrap();
        let stored = store
            .save(&next, RevisionPrecondition::Unchanged(current))
            .unwrap();
        assert_eq!(stored.revision(), 3);
    }

    #[test]
    fn absent_precondition_refuses_to_replace_an_existing_file() {
        let (_directory, store) = fixture();
        let forked = store
            .fork_builtin(ThemeName::MintDark, "Mint copy")
            .unwrap();
        let error = store
            .save(&forked, RevisionPrecondition::Absent)
            .unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                expected: None,
                actual: Some(1)
            }
        ));
        assert_eq!(store.load("mint-copy").unwrap().revision(), 1);
    }

    #[test]
    fn names_and_ids_are_disambiguated_on_import() {
        let (_directory, store) = fixture();
        let builtin = super::super::document::builtin_document(ThemeName::Nord).unwrap();
        let first = store.import(&builtin).unwrap();
        let second = store.import(&builtin).unwrap();
        // 内置目录已占用 "Nord":导入第一次就消歧,第二次继续顺延。
        assert_eq!(first.name(), "Nord (2)");
        assert_eq!(second.name(), "Nord (3)");
        assert_ne!(first.id(), second.id());
        assert_eq!(first.id(), Some("nord-2"));
        assert_eq!(second.id(), Some("nord-3"));
    }

    #[test]
    fn delete_requires_the_observed_revision() {
        let (_directory, store) = fixture();
        store.fork_builtin(ThemeName::Nord, "Deletable").unwrap();
        let error = store.delete("deletable", 99).unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                expected: Some(99),
                actual: Some(1)
            }
        ));
        store.delete("deletable", 1).unwrap();
        assert!(matches!(
            store.load("deletable"),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn list_reports_builtins_and_skips_foreign_files() {
        let (directory, store) = fixture();
        custom_document(&store, "Custom");
        fs::write(directory.path().join("themes").join("notes.txt"), "x").unwrap();
        let snapshot = store.list().unwrap();
        assert_eq!(snapshot.builtins.len(), ThemeName::BUILTIN.len());
        assert_eq!(snapshot.custom.len(), 1);
        assert_eq!(snapshot.custom[0].name(), "Custom");
        assert!(snapshot.diagnostics.is_empty());
    }

    #[test]
    fn builtins_are_read_only() {
        let (_directory, store) = fixture();
        let builtin = super::super::document::builtin_document(ThemeName::Nord).unwrap();
        let error = store.save(&builtin, RevisionPrecondition::Any).unwrap_err();
        assert!(matches!(error, StoreError::BuiltinReadOnly(_)));
    }
}
