//! Explorer 粘贴的规范 Journal 记录与保守恢复；不自动重放未结算副作用。

use std::io::{self, Read as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

use crate::persistence::{
    journal::{
        JournalError, JournalRegistration, JournalStatus, JournalStep, JournalStepRegistration,
        ManagedJournal,
    },
    startup::JournalRecoverer,
    workspace::WorkspaceDatabase,
};

pub(crate) const PASTE_JOURNAL_KIND: &str = "explorer.paste";
const PASTE_JOURNAL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PasteOperationKind {
    Copy,
    Cut,
    ReplaceCopy,
    ReplaceCut,
}

impl PasteOperationKind {
    fn step_kind(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Cut => "cut",
            Self::ReplaceCopy => "replace_copy",
            Self::ReplaceCut => "replace_cut",
        }
    }

    fn is_move(self) -> bool {
        matches!(self, Self::Cut | Self::ReplaceCut)
    }

    fn is_replacement(self) -> bool {
        matches!(self, Self::ReplaceCopy | Self::ReplaceCut)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PasteSnapshot {
    pub(crate) lexical_path: CanonicalPath,
    pub(crate) resolved: ResolvedPath,
    pub(crate) fingerprint: [u8; 32],
    pub(crate) authorization_generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct PastePath {
    pub(crate) lexical_path: CanonicalPath,
    pub(crate) canonical_path: CanonicalPath,
}

pub(crate) struct PasteJournalRecord;

impl PasteJournalRecord {
    pub(crate) fn registration(
        journal_id: &str,
        operation: PasteOperationKind,
        source: &PasteSnapshot,
        destination: &PastePath,
        replacement: Option<&PasteSnapshot>,
        store_epoch: &str,
    ) -> Result<JournalRegistration, JournalError> {
        if operation.is_replacement() != replacement.is_some() {
            return Err(JournalError::InvalidRegistration);
        }
        let target = TargetDto::new(operation, source, destination, replacement)?;
        let target = serde_json::to_value(target).map_err(|_| JournalError::InvalidRegistration)?;
        let before = Sha256::digest(
            serde_json::to_vec(&target).map_err(|_| JournalError::InvalidRegistration)?,
        )
        .into();
        Ok(JournalRegistration {
            operation_id: None,
            journal_id: journal_id.to_owned(),
            kind: PASTE_JOURNAL_KIND.to_owned(),
            store_epoch: store_epoch.to_owned(),
            before_fingerprint: Some(before),
            steps: vec![JournalStepRegistration {
                step_key: journal_id.to_owned(),
                kind: operation.step_kind().to_owned(),
                target,
                authorization_generation: source.authorization_generation,
                before_fingerprint: Some(source.fingerprint),
            }],
        })
    }
}

#[derive(Debug)]
pub(crate) enum PasteJournalError {
    Unavailable,
    Journal(JournalError),
}

/// 粘贴执行体使用的同步 Journal 端口；实现负责在后台等待真实持久化回执。
pub(crate) trait PasteJournal: Send + Sync {
    fn begin(&self, registration: JournalRegistration) -> Result<(), PasteJournalError>;
    fn mark_applying(&self, journal_id: &str) -> Result<(), PasteJournalError>;
    fn record_after(
        &self,
        journal_id: &str,
        step_key: &str,
        after: [u8; 32],
    ) -> Result<(), PasteJournalError>;
    fn commit(&self, journal_id: &str, after: [u8; 32]) -> Result<(), PasteJournalError>;
}

impl PasteJournal for crate::persistence::workspace_writer::WorkspaceWriterClient {
    fn begin(&self, registration: JournalRegistration) -> Result<(), PasteJournalError> {
        let result = self
            .register_journal(registration)
            .map_err(|_| PasteJournalError::Unavailable)?;
        wait_journal(result).map(|_| ())
    }

    fn mark_applying(&self, journal_id: &str) -> Result<(), PasteJournalError> {
        let result = self
            .transition_journal(journal_id, JournalStatus::Applying, None)
            .map_err(|_| PasteJournalError::Unavailable)?;
        wait_journal(result).map(|_| ())
    }

    fn record_after(
        &self,
        journal_id: &str,
        step_key: &str,
        after: [u8; 32],
    ) -> Result<(), PasteJournalError> {
        let result = self
            .record_journal_step(journal_id, step_key, after)
            .map_err(|_| PasteJournalError::Unavailable)?;
        wait_journal(result).map(|_| ())
    }

    fn commit(&self, journal_id: &str, after: [u8; 32]) -> Result<(), PasteJournalError> {
        let committing = self
            .transition_journal(journal_id, JournalStatus::Committing, None)
            .map_err(|_| PasteJournalError::Unavailable)?;
        wait_journal(committing)?;
        let committed = self
            .transition_journal(journal_id, JournalStatus::Committed, Some(after))
            .map_err(|_| PasteJournalError::Unavailable)?;
        wait_journal(committed).map(|_| ())
    }
}

fn wait_journal<T>(
    result: std::sync::mpsc::Receiver<Result<T, JournalError>>,
) -> Result<T, PasteJournalError> {
    result
        .recv()
        .map_err(|_| PasteJournalError::Unavailable)?
        .map_err(PasteJournalError::Journal)
}

pub(crate) struct PasteJournalRecoverer;

impl JournalRecoverer for PasteJournalRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        journal: &ManagedJournal,
    ) -> Result<(), JournalError> {
        match journal.status {
            JournalStatus::Prepared => {
                store.transition_journal(&journal.journal_id, JournalStatus::Failed, None)?;
                return Ok(());
            }
            JournalStatus::NeedsAttention => return Ok(()),
            JournalStatus::RollingBack => {
                store.transition_journal(
                    &journal.journal_id,
                    JournalStatus::NeedsAttention,
                    None,
                )?;
                return Ok(());
            }
            JournalStatus::Applying | JournalStatus::Committing => {}
            JournalStatus::Committed | JournalStatus::RolledBack | JournalStatus::Failed => {
                return Ok(());
            }
        }
        let step = single_step(journal)?;
        let target = decode_target(step)?;
        if step.kind != target.operation.step_kind() {
            return Err(JournalError::CorruptRecord);
        }
        validate_registration_facts(journal, step, &target)?;
        if target.operation.is_move() && !move_is_complete(&target) {
            return needs_attention(store, journal);
        }
        if let Some(after) = step.after_fingerprint {
            return match outcome_fingerprint(target.destination.lexical.as_path()) {
                Ok(actual) if actual == after => commit(store, journal, after),
                Ok(_) | Err(_) => needs_attention(store, journal),
            };
        }
        if target.operation.is_move() && move_is_complete(&target) {
            let after = match outcome_fingerprint(target.destination.lexical.as_path()) {
                Ok(after) => after,
                Err(_) => return needs_attention(store, journal),
            };
            store.record_step_after(&journal.journal_id, &step.step_key, after)?;
            return commit(store, journal, after);
        }
        needs_attention(store, journal)
    }
}

fn single_step(journal: &ManagedJournal) -> Result<&JournalStep, JournalError> {
    if journal.kind != PASTE_JOURNAL_KIND || journal.steps.len() != 1 {
        return Err(JournalError::CorruptRecord);
    }
    let step = &journal.steps[0];
    if step.step_key != journal.journal_id {
        return Err(JournalError::CorruptRecord);
    }
    Ok(step)
}

fn decode_target(step: &JournalStep) -> Result<TargetDto, JournalError> {
    let target: TargetDto =
        serde_json::from_value(step.target.clone()).map_err(|_| JournalError::CorruptRecord)?;
    if target.version != PASTE_JOURNAL_VERSION
        || target.operation.is_replacement() != target.replacement.is_some()
    {
        return Err(JournalError::CorruptRecord);
    }
    Ok(target)
}

fn validate_registration_facts(
    journal: &ManagedJournal,
    step: &JournalStep,
    target: &TargetDto,
) -> Result<(), JournalError> {
    let value = serde_json::to_value(target).map_err(|_| JournalError::CorruptRecord)?;
    let bytes = serde_json::to_vec(&value).map_err(|_| JournalError::CorruptRecord)?;
    let expected_before: [u8; 32] = Sha256::digest(bytes).into();
    if journal.before_fingerprint != Some(expected_before)
        || step.before_fingerprint != Some(target.source.fingerprint)
        || step.authorization_generation != target.source.authorization_generation
    {
        return Err(JournalError::CorruptRecord);
    }
    Ok(())
}

fn commit(
    store: &mut WorkspaceDatabase,
    journal: &ManagedJournal,
    after: [u8; 32],
) -> Result<(), JournalError> {
    if journal.status == JournalStatus::Applying {
        store.transition_journal(&journal.journal_id, JournalStatus::Committing, None)?;
    }
    store.transition_journal(&journal.journal_id, JournalStatus::Committed, Some(after))?;
    Ok(())
}

fn needs_attention(
    store: &mut WorkspaceDatabase,
    journal: &ManagedJournal,
) -> Result<(), JournalError> {
    store.transition_journal(&journal.journal_id, JournalStatus::NeedsAttention, None)?;
    Ok(())
}

fn move_is_complete(target: &TargetDto) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    use crate::platform::document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver};

    if !matches!(
        std::fs::symlink_metadata(target.source.lexical.as_path()),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    ) {
        return false;
    }
    if std::fs::symlink_metadata(target.destination.lexical.as_path())
        .is_ok_and(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    {
        return false;
    }
    SystemDocumentPathResolver
        .resolve(&target.destination.lexical)
        .ok()
        .filter(|destination| destination.canonical_path() == &target.destination.canonical)
        .and_then(|destination| destination.file_identity())
        .zip(target.source.resolved.file_identity.as_ref())
        .is_some_and(|(actual, expected)| *expected == actual)
}

/// 对实际目标 outcome 取指纹；目录按排序后的完整后代计算，reparse point 保守失败。
pub(crate) fn outcome_fingerprint(path: &std::path::Path) -> io::Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(b"slterminal.paste.outcome.v1");
    hash_outcome_entry(&mut hasher, path, 0)?;
    Ok(hasher.finalize().into())
}

fn hash_outcome_entry(hasher: &mut Sha256, path: &std::path::Path, depth: usize) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::fs::MetadataExt as _;
    use std::time::UNIX_EPOCH;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    const MAX_OUTCOME_DEPTH: usize = 256;
    if depth > MAX_OUTCOME_DEPTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "目录 outcome 超出深度预算",
        ));
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            hasher.update([0]);
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "outcome 不得跟随 reparse point",
        ));
    }
    hasher.update([1]);
    hasher.update([u8::from(metadata.is_dir())]);
    hasher.update(metadata.len().to_le_bytes());
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "文件时间早于 Unix epoch"))?;
    hasher.update(modified.as_nanos().to_le_bytes());
    if metadata.is_file() {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    } else if metadata.is_dir() {
        let mut children = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            children.push((
                entry.file_name().encode_wide().collect::<Vec<_>>(),
                entry.path(),
            ));
        }
        children.sort_by(|left, right| left.0.cmp(&right.0));
        hasher.update((children.len() as u64).to_le_bytes());
        for (name, child) in children {
            hasher.update((name.len() as u64).to_le_bytes());
            for unit in name {
                hasher.update(unit.to_le_bytes());
            }
            hash_outcome_entry(hasher, &child, depth + 1)?;
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "outcome 仅支持普通文件与目录",
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetDto {
    version: u32,
    operation: PasteOperationKind,
    source: SnapshotDto,
    destination: PathDto,
    replacement: Option<SnapshotDto>,
}

impl TargetDto {
    fn new(
        operation: PasteOperationKind,
        source: &PasteSnapshot,
        destination: &PastePath,
        replacement: Option<&PasteSnapshot>,
    ) -> Result<Self, JournalError> {
        Ok(Self {
            version: PASTE_JOURNAL_VERSION,
            operation,
            source: SnapshotDto::new(source)?,
            destination: PathDto::new(destination)?,
            replacement: replacement.map(SnapshotDto::new).transpose()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathDto {
    #[serde(with = "canonical_path_serde")]
    lexical: CanonicalPath,
    #[serde(with = "canonical_path_serde")]
    canonical: CanonicalPath,
}

impl PathDto {
    fn new(path: &PastePath) -> Result<Self, JournalError> {
        Ok(Self {
            lexical: path.lexical_path.clone(),
            canonical: path.canonical_path.clone(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotDto {
    #[serde(with = "canonical_path_serde")]
    lexical: CanonicalPath,
    resolved: ResolvedPathDto,
    fingerprint: [u8; 32],
    authorization_generation: u64,
}

impl SnapshotDto {
    fn new(snapshot: &PasteSnapshot) -> Result<Self, JournalError> {
        Ok(Self {
            lexical: snapshot.lexical_path.clone(),
            resolved: ResolvedPathDto::new(&snapshot.resolved)?,
            fingerprint: snapshot.fingerprint,
            authorization_generation: snapshot.authorization_generation,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedPathDto {
    #[serde(with = "canonical_path_serde")]
    canonical: CanonicalPath,
    file_identity: Option<FileIdentityDto>,
}

impl ResolvedPathDto {
    fn new(resolved: &ResolvedPath) -> Result<Self, JournalError> {
        Ok(Self {
            canonical: resolved.canonical_path().clone(),
            file_identity: resolved.file_identity().map(FileIdentityDto::from),
        })
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileIdentityDto {
    volume: u64,
    file: String,
}

impl PartialEq<FileIdentity> for FileIdentityDto {
    fn eq(&self, other: &FileIdentity) -> bool {
        self.volume == other.volume && self.file == other.file.to_string()
    }
}

mod canonical_path_serde {
    use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _};
    use slterm_path::CanonicalPath;

    pub(super) fn serialize<S>(path: &CanonicalPath, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = path
            .as_path()
            .to_str()
            .ok_or_else(|| serde::ser::Error::custom("规范路径必须能无损表示为 Unicode"))?;
        serializer.serialize_str(value)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<CanonicalPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        CanonicalPath::new(value).map_err(D::Error::custom)
    }
}

impl From<FileIdentity> for FileIdentityDto {
    fn from(identity: FileIdentity) -> Self {
        Self {
            volume: identity.volume,
            file: identity.file.to_string(),
        }
    }
}
