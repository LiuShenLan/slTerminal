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

use slterm_settings::{ThemeId, ThemeName};

use super::document::{DocumentError, MAX_DOCUMENT_BYTES, ThemeDocument, builtin_documents};

/// 自定义主题文件数上限(契约数值):超限拒绝新建。
pub const MAX_CUSTOM_THEMES: usize = 128;

/// 删除当前启用主题的处置：必须先另选一个主题才能删除。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeDeletion {
    Delete,
    RequiresReplacement,
}

/// 纯判定：删除目标正是当前启用主题时拒绝，须用户先另选。
pub fn resolve_theme_deletion(active_theme: Option<&str>, deleting: &str) -> ThemeDeletion {
    if active_theme == Some(deleting) {
        ThemeDeletion::RequiresReplacement
    } else {
        ThemeDeletion::Delete
    }
}
/// 自定义主题文件后缀。
pub const THEME_FILE_SUFFIX: &str = ".slterm-theme.json";

/// 保存前置条件:只允许在观测到的事实上写入。四态由设置域锚定;
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
    NameConflict(String),
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
            Self::NameConflict(name) => {
                write!(f, "theme name is already in use: {name}")
            }
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
        // 开放问题 2 / 数据目录下 `themes/` 子目录,文件名不带品牌。
        Self::new(crate::platform::dirs::config_dir().join("themes"))
    }
}

impl ThemeLibraryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 某自定义主题的精确路径(诊断/测试共用同一构造规则)。
    pub fn path_for(&self, id: ThemeId) -> Result<PathBuf, StoreError> {
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
            let Some(id) = file_name
                .strip_suffix(THEME_FILE_SUFFIX)
                .and_then(|stem| stem.parse::<ThemeId>().ok())
            else {
                continue;
            };
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

    /// 按稳定身份加载自定义主题;内置主题按显示名命中。
    pub fn load(&self, id: &str) -> Result<ThemeDocument, StoreError> {
        if let Some(document) = builtin_documents()
            .into_iter()
            .find(|document| document.name() == id)
        {
            return Ok(document);
        }
        let theme_id = parse_id(id)?;
        let path = self.path_for(theme_id)?;
        read_document(&path, theme_id)?.ok_or_else(|| StoreError::NotFound(id.to_owned()))
    }

    /// 条件保存:修订/快照不匹配即 `Conflict`,绝不清默覆盖。
    pub fn save(
        &self,
        document: &ThemeDocument,
        precondition: RevisionPrecondition,
    ) -> Result<ThemeDocument, StoreError> {
        self.save_internal(document, precondition, None)
    }

    fn save_internal(
        &self,
        document: &ThemeDocument,
        precondition: RevisionPrecondition,
        import_source: Option<&Path>,
    ) -> Result<ThemeDocument, StoreError> {
        if document.is_builtin() {
            return Err(StoreError::BuiltinReadOnly(document.name().to_owned()));
        }
        let id = document.theme_id().ok_or_else(|| {
            StoreError::InvalidDocument("custom themes require a UUIDv7 id".to_owned())
        })?;
        let path = self.path_for(id)?;
        if let Some(source) = import_source {
            self.reject_import_source_locks(source, &path)?;
        }
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
                if expected_document.theme_id() != Some(id) {
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
        let stored = document.with_id_and_revision(id, revision);
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
    #[allow(dead_code)] // 消费点 = 主题库管理 UI(测试已用)。
    pub fn fork_builtin(&self, theme: ThemeName, name: &str) -> Result<ThemeDocument, StoreError> {
        let builtin = builtin_documents()
            .into_iter()
            .find(|document| document.name() == theme.prompt_name())
            .ok_or_else(|| StoreError::NotFound(theme.prompt_name().to_owned()))?;
        let snapshot = self.list()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidDocument(
                "theme name must not be empty".to_owned(),
            ));
        }
        reject_name_conflict(name, &snapshot)?;
        let candidate = builtin.fork(fresh_theme_id(), name)?;
        self.save(&candidate, RevisionPrecondition::Absent)
    }

    /// 导入已校验文档为新自定义主题。原生 id 被忽略,导入不可能覆盖
    /// 既有文件;显示名冲突自动消歧。
    pub fn import(&self, document: &ThemeDocument) -> Result<ThemeDocument, StoreError> {
        self.import_internal(document, None)
    }

    fn import_internal(
        &self,
        document: &ThemeDocument,
        source: Option<&Path>,
    ) -> Result<ThemeDocument, StoreError> {
        let snapshot = self.list()?;
        let name = document.name().trim();
        if name.is_empty() {
            return Err(StoreError::InvalidDocument(
                "theme name must not be empty".to_owned(),
            ));
        }
        // 同身份 = 同一主题：明确冲突并零写入；否则保留来源身份（跨机恢复），
        // 无身份的手写主题由 application 注入新的 UUIDv7。
        let id = match document.theme_id() {
            Some(id) => {
                if let Some(existing) = snapshot
                    .custom
                    .iter()
                    .find(|stored| stored.theme_id() == Some(id))
                {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual: Some(existing.revision()),
                    });
                }
                id
            }
            None => fresh_theme_id(),
        };
        reject_name_conflict(name, &snapshot)?;
        let candidate = document.with_name(name)?.with_id_and_revision(id, 0);
        self.save_internal(&candidate, RevisionPrecondition::Absent, source)
    }

    /// 从本次授权的单文件 JSON 导入；来源身份不进入库内，失败不产生部分写入。
    pub fn import_json_file(&self, path: &Path) -> Result<ThemeDocument, StoreError> {
        let bytes = read_document_file(path)?.ok_or_else(|| {
            io_error(
                path,
                io::Error::new(io::ErrorKind::NotFound, "theme file not found"),
            )
        })?;
        let document = ThemeDocument::from_json_bytes(&bytes)?;
        self.import_internal(&document, Some(path))
    }

    /// 条件删除:修订不匹配拒绝(外部编辑不可被静默删掉)。
    #[allow(dead_code)] // 消费点 = 主题库管理 UI(测试已用)。
    pub fn delete(&self, id: &str, expected_revision: u64) -> Result<(), StoreError> {
        if builtin_documents()
            .into_iter()
            .any(|document| document.name() == id)
        {
            return Err(StoreError::BuiltinReadOnly(id.to_owned()));
        }
        let theme_id = parse_id(id)?;
        let path = self.path_for(theme_id)?;
        let _global_lock = self.acquire_global_lock()?;
        let _target_lock = self.acquire_target_lock(&path)?;
        let current = read_document(&path, theme_id)?;
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

    fn reject_import_source_locks(&self, source: &Path, target: &Path) -> Result<(), StoreError> {
        let lock_paths = [
            crate::atomic_file::lock_path(&self.root.join(".slterm-theme-library")),
            crate::atomic_file::lock_path(target),
        ];
        for lock_path in lock_paths {
            if paths_refer_to_same_file(source, &lock_path)? {
                return Err(StoreError::InvalidDocument(format!(
                    "import source conflicts with theme library lock: {}",
                    lock_path.display()
                )));
            }
        }
        Ok(())
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
            let valid_id = file_name
                .strip_suffix(THEME_FILE_SUFFIX)
                .is_some_and(|stem| stem.parse::<ThemeId>().is_ok());
            if !valid_id
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
}

fn read_document(path: &Path, expected_id: ThemeId) -> Result<Option<ThemeDocument>, StoreError> {
    let Some(bytes) = read_document_file(path)? else {
        return Ok(None);
    };
    let document = ThemeDocument::from_json_bytes(&bytes)?;
    if document.is_builtin() {
        return Err(StoreError::InvalidDocument(
            "custom theme files cannot be marked builtin".to_owned(),
        ));
    }
    if document.theme_id() != Some(expected_id) {
        return Err(StoreError::InvalidDocument(format!(
            "document id {:?} does not match file id {expected_id}",
            document.id()
        )));
    }
    Ok(Some(document))
}

/// 库内文件与外部导入共用的有界读取；缺失由调用方按各自语义处理。
fn read_document_file(path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
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
    Ok(Some(bytes))
}

/// 同时按规范路径与卷内文件身份判同，覆盖路径别名和硬链接。
fn paths_refer_to_same_file(left: &Path, right: &Path) -> Result<bool, StoreError> {
    let left_canonical = fs::canonicalize(left).map_err(|error| io_error(left, error))?;
    let right_canonical = match fs::canonicalize(right) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error(right, error)),
    };
    if left_canonical == right_canonical {
        return Ok(true);
    }
    let left_identity = file_identity(&left_canonical).map_err(|error| io_error(left, error))?;
    let right_identity = file_identity(&right_canonical).map_err(|error| io_error(right, error))?;
    Ok(left_identity == right_identity)
}

fn file_identity(path: &Path) -> io::Result<(u32, u64)> {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let file = fs::File::open(path)?;
    // Win32 会完整初始化该 POD；文件句柄在调用期间保持有效。
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Ok((info.dwVolumeSerialNumber, index))
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

fn fresh_theme_id() -> ThemeId {
    let uuid = uuid::Uuid::now_v7();
    ThemeId::from_bytes(*uuid.as_bytes()).expect("UUIDv7 字节一定合法")
}

fn parse_id(id: &str) -> Result<ThemeId, StoreError> {
    id.parse().map_err(|_| StoreError::InvalidId(id.to_owned()))
}

/// 显示名与内置/自定义主题冲突即拒绝：不自动改名，避免静默复制。
fn reject_name_conflict(name: &str, snapshot: &ThemeLibrarySnapshot) -> Result<(), StoreError> {
    let occupied = snapshot
        .builtins
        .iter()
        .chain(snapshot.custom.iter())
        .any(|document| document.name().eq_ignore_ascii_case(name));
    if occupied {
        return Err(StoreError::NameConflict(name.to_owned()));
    }
    Ok(())
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

    fn import_source_bytes(name: &str) -> Vec<u8> {
        let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        definition.name = name.to_owned();
        super::super::document::from_definition(&definition)
            .unwrap()
            .to_json_bytes()
            .unwrap()
    }

    #[test]
    fn first_save_forks_and_revisions_increment() {
        let (_directory, store) = fixture();
        let forked = store.fork_builtin(ThemeName::Nord, "My Nord").unwrap();
        let id = forked.theme_id().unwrap();
        assert_eq!(forked.revision(), 1);
        assert!(!forked.is_builtin());
        assert!(store.path_for(id).unwrap().exists());

        let updated = forked
            .with_name("My Nord 2")
            .unwrap()
            .with_id_and_revision(id, 1);
        let stored = store
            .save(&updated, RevisionPrecondition::Unchanged(forked.clone()))
            .unwrap();
        assert_eq!(stored.revision(), 2);
        assert_eq!(store.load(&id.to_string()).unwrap().revision(), 2);
        assert_eq!(store.load(&id.to_string()).unwrap().name(), "My Nord 2");
    }

    #[test]
    fn external_edits_are_never_silently_overwritten() {
        let (_directory, store) = fixture();
        let forked = store.fork_builtin(ThemeName::Nord, "Guarded").unwrap();
        let id = forked.theme_id().unwrap();
        // 外部写入者把文件改成修订 2(同 id)。
        let external = forked
            .with_name("Guarded external")
            .unwrap()
            .with_id_and_revision(id, 2);
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
        assert_eq!(
            store.load(&id.to_string()).unwrap().name(),
            "Guarded external"
        );

        // 外部编辑后再读出的快照允许继续保存。
        let current = store.load(&id.to_string()).unwrap();
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
        assert_eq!(
            store
                .load(&forked.theme_id().unwrap().to_string())
                .unwrap()
                .revision(),
            1
        );
    }

    #[test]
    fn import_rejects_a_display_name_conflict_without_writing() {
        let (_directory, store) = fixture();
        let builtin = super::super::document::builtin_document(ThemeName::Nord).unwrap();
        // 内置目录已占用 "Nord":导入同名主题必须明确拒绝，不能自动改名。
        let error = store.import(&builtin).unwrap_err();
        assert!(
            matches!(error, StoreError::NameConflict(ref name) if name == "Nord"),
            "同名导入必须拒绝: {error}"
        );
        assert!(store.list().unwrap().custom.is_empty(), "冲突导入零写入");
    }

    fn identified_document(id: &str, name: &str) -> ThemeDocument {
        let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        definition.name = name.to_owned();
        super::super::document::from_definition(&definition)
            .unwrap()
            .with_id_and_revision(id.parse().unwrap(), 0)
    }

    #[test]
    fn import_rejects_an_identity_that_already_exists() {
        let (_directory, store) = fixture();
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
        let first = identified_document(id, "Identity One");
        let stored = store.import(&first).unwrap();
        assert_eq!(stored.id(), Some(id));
        let again = identified_document(id, "Identity Two");
        let error = store.import(&again).unwrap_err();
        assert!(
            matches!(
                error,
                StoreError::Conflict {
                    actual: Some(1),
                    ..
                }
            ),
            "相同身份必须拒绝: {error}"
        );
        assert_eq!(store.list().unwrap().custom.len(), 1, "冲突导入零写入");
        assert_eq!(store.load(id).unwrap().name(), "Identity One");
    }

    #[test]
    fn fork_assigns_a_uuid_v7_identity_independent_from_the_name() {
        use std::str::FromStr as _;

        let (_directory, store) = fixture();
        let forked = store.fork_builtin(ThemeName::Nord, "My Nord").unwrap();
        let id = forked.id().expect("自定义主题必须有身份");
        assert!(
            slterm_settings::ThemeId::from_str(id).is_ok(),
            "新身份必须是 UUIDv7: {id}"
        );
        assert_eq!(forked.name(), "My Nord");
        assert!(
            store
                .path_for(slterm_settings::ThemeId::from_str(id).unwrap())
                .unwrap()
                .exists()
        );
    }

    /// 完整主题才可导入；缺项、未知字段与非法色值全部零写入。
    #[test]
    fn theme_import_complete_or_zero_write() {
        let (directory, store) = fixture();
        let complete = directory.path().join("complete.json");
        fs::write(&complete, import_source_bytes("Complete")).unwrap();
        let imported = store.import_json_file(&complete).unwrap();
        assert_eq!(imported.name(), "Complete");
        assert!(imported.id().is_some());
        let baseline = store.list().unwrap().custom.len();
        assert_eq!(baseline, 1);

        let document: serde_json::Value =
            serde_json::from_slice(&import_source_bytes("Broken")).unwrap();
        let mut missing = document.clone();
        missing["ui"].as_object_mut().unwrap().remove("accent");
        let mut unknown = document.clone();
        unknown["ui"]["future_slot"] = serde_json::json!("#123456");
        let mut invalid = document.clone();
        invalid["terminal"]["background"] = serde_json::json!("not-a-color");

        let mut accepted = Vec::new();
        for (tag, value) in [
            ("missing", missing),
            ("unknown", unknown),
            ("invalid", invalid),
        ] {
            let path = directory.path().join(format!("{tag}.json"));
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            if let Ok(document) = store.import_json_file(&path) {
                accepted.push(format!("{tag} -> {}", document.name()));
            }
            assert_eq!(
                store.list().unwrap().custom.len(),
                baseline,
                "{tag} 失败不得产生库内部分写入"
            );
        }
        assert!(
            accepted.is_empty(),
            "缺项、未知字段与非法色值主题不得导入: {accepted:?}"
        );
        let imported_id = imported.theme_id().unwrap().to_string();
        assert_eq!(
            resolve_theme_deletion(Some(&imported_id), &imported_id),
            ThemeDeletion::RequiresReplacement,
            "删除当前使用中的主题必须先另选"
        );
        assert_eq!(
            resolve_theme_deletion(Some(&imported_id), "other"),
            ThemeDeletion::Delete
        );
        assert_eq!(
            resolve_theme_deletion(None, &imported_id),
            ThemeDeletion::Delete
        );
    }

    #[test]
    fn json_file_import_uses_the_existing_store_path() {
        let (directory, store) = fixture();
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
        let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        definition.name = "External".to_owned();
        let source = super::super::document::from_definition(&definition)
            .unwrap()
            .with_id_and_revision(id.parse().unwrap(), 9);
        let bytes = source.to_json_bytes().unwrap();
        let path = directory.path().join("source.json");
        fs::write(&path, &bytes).unwrap();

        let imported = store.import_json_file(&path).unwrap();

        assert_eq!(imported.name(), "External");
        assert_eq!(imported.id(), Some(id), "跨机恢复保留来源身份");
        assert_eq!(imported.revision(), 1);
        assert_eq!(store.load(id).unwrap(), imported);
        assert_eq!(fs::read(&path).unwrap(), bytes, "导入不得改写来源文件");
        // 同身份重导 = 冲突；零写入且不改名。
        let error = store.import_json_file(&path).unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                actual: Some(1),
                ..
            }
        ));
        assert_eq!(store.list().unwrap().custom.len(), 1);
    }

    #[test]
    fn invalid_json_file_does_not_touch_the_library() {
        let (directory, store) = fixture();
        let path = directory.path().join("broken.json");
        fs::write(&path, b"not valid JSON").unwrap();

        let error = store.import_json_file(&path).unwrap_err();

        assert!(matches!(
            error,
            StoreError::Document(DocumentError::Json(_))
        ));
        assert!(store.list().unwrap().custom.is_empty());
        assert!(
            !directory.path().join("themes").exists(),
            "失败不得创建主题库目录"
        );
    }

    #[test]
    fn oversized_json_file_is_rejected_before_import() {
        let (directory, store) = fixture();
        let path = directory.path().join("oversized.json");
        fs::write(&path, vec![b' '; MAX_DOCUMENT_BYTES + 1]).unwrap();

        let error = store.import_json_file(&path).unwrap_err();

        assert!(matches!(
            error,
            StoreError::Document(DocumentError::TooLarge { .. })
        ));
        assert!(store.list().unwrap().custom.is_empty());
        assert!(
            !directory.path().join("themes").exists(),
            "超限输入不得创建主题库目录"
        );
    }

    #[test]
    fn json_file_import_rejects_global_lock_source() {
        let (_directory, store) = fixture();
        fs::create_dir_all(&store.root).unwrap();
        let path = store.root.join(".slterm-theme-library.slterm-lock");
        let bytes = import_source_bytes("External");
        fs::write(&path, &bytes).unwrap();

        let result = store.import_json_file(&path);

        assert!(
            matches!(result, Err(StoreError::InvalidDocument(_))),
            "来源与全局锁重合时必须拒绝导入"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes, "来源内容必须保持不变");
        assert!(store.list().unwrap().custom.is_empty());
    }

    #[test]
    fn json_file_import_rejects_target_lock_source() {
        let (_directory, store) = fixture();
        fs::create_dir_all(&store.root).unwrap();
        let id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
        let target = store.path_for(id.parse().unwrap()).unwrap();
        let path = crate::atomic_file::lock_path(&target);
        let mut definition = ThemeDefinition::from_builtin(ThemeName::Nord);
        definition.name = "External".to_owned();
        let bytes = super::super::document::from_definition(&definition)
            .unwrap()
            .with_id_and_revision(id.parse().unwrap(), 0)
            .to_json_bytes()
            .unwrap();
        fs::write(&path, &bytes).unwrap();

        let result = store.import_json_file(&path);

        assert!(
            matches!(result, Err(StoreError::InvalidDocument(_))),
            "来源与目标锁重合时必须拒绝导入"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes, "来源内容必须保持不变");
        assert!(!target.exists());
        assert!(store.list().unwrap().custom.is_empty());
    }

    #[test]
    fn json_file_import_rejects_hard_link_to_global_lock() {
        let (directory, store) = fixture();
        fs::create_dir_all(&store.root).unwrap();
        let lock_path = store.root.join(".slterm-theme-library.slterm-lock");
        let bytes = import_source_bytes("External");
        fs::write(&lock_path, &bytes).unwrap();
        let source = directory.path().join("source.json");
        fs::hard_link(&lock_path, &source).unwrap();

        let result = store.import_json_file(&source);

        assert!(
            matches!(result, Err(StoreError::InvalidDocument(_))),
            "来源与全局锁指向同一文件时必须拒绝导入"
        );
        assert_eq!(fs::read(&source).unwrap(), bytes, "来源内容必须保持不变");
        assert_eq!(fs::read(&lock_path).unwrap(), bytes);
        assert!(store.list().unwrap().custom.is_empty());
    }

    #[test]
    fn delete_requires_the_observed_revision() {
        let (_directory, store) = fixture();
        let id = store
            .fork_builtin(ThemeName::Nord, "Deletable")
            .unwrap()
            .theme_id()
            .unwrap()
            .to_string();
        let error = store.delete(&id, 99).unwrap_err();
        assert!(matches!(
            error,
            StoreError::Conflict {
                expected: Some(99),
                actual: Some(1)
            }
        ));
        store.delete(&id, 1).unwrap();
        assert!(matches!(store.load(&id), Err(StoreError::NotFound(_))));
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
