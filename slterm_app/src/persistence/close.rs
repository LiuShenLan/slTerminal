//! 关闭意图的规范化持久记录与顺序提交端口。

use std::fmt;
use std::str::FromStr;

use rusqlite::types::FromSql;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use slterm_workspace::{
    CloseDecision, CloseIntent, CloseObject, CloseScope, ProjectId, ProjectSnapshot,
};

use super::database::DatabaseError;
use super::metadata::store_epoch_is_current;
use super::project::{PreparedProjectSnapshot, ProjectStoreError};
use super::workspace::WorkspaceDatabase;

/// 关闭事实的提交进度；只允许按声明顺序推进一步。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseCommitStatus {
    Collected,
    StateCommitted,
    PtyStopped,
    CleanupCommitted,
    Completed,
}

impl CloseCommitStatus {
    pub(crate) const ALL: [Self; 5] = [
        Self::Collected,
        Self::StateCommitted,
        Self::PtyStopped,
        Self::CleanupCommitted,
        Self::Completed,
    ];

    const fn successor(self) -> Option<Self> {
        match self {
            Self::Collected => Some(Self::StateCommitted),
            Self::StateCommitted => Some(Self::PtyStopped),
            Self::PtyStopped => Some(Self::CleanupCommitted),
            Self::CleanupCommitted => Some(Self::Completed),
            Self::Completed => None,
        }
    }
}

/// application 已构造对象身份的完整关闭事实；存储端口不解释对象类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersistedCloseIntent {
    pub(crate) close_id: String,
    pub(crate) scope: CloseScope,
    pub(crate) store_epoch: String,
    pub(crate) intent: CloseIntent<String>,
    pub(crate) commit_status: CloseCommitStatus,
}

/// 关闭意图读写失败；错误展示不携带对象、路径、SQL 或指纹正文。
#[derive(Debug)]
pub(crate) enum CloseStoreError {
    InvalidRegistration,
    CloseIdConflict,
    UnknownCloseIntent,
    CloseConflict,
    InvalidTransition,
    StoreEpochConflict,
    CorruptRecord,
    Database(DatabaseError),
}

impl fmt::Display for CloseStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRegistration => "关闭意图登记字段无效",
            Self::CloseIdConflict => "关闭意图 ID 已存在",
            Self::UnknownCloseIntent => "关闭意图不存在",
            Self::CloseConflict => "关闭意图前置事实已变化",
            Self::InvalidTransition => "关闭意图状态转换无效",
            Self::StoreEpochConflict => "关闭意图世代不是当前工作区世代",
            Self::CorruptRecord => "关闭意图持久记录损坏",
            Self::Database(_) => "关闭意图数据库操作失败",
        })
    }
}

impl std::error::Error for CloseStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

/// 项目快照与关闭意图组合提交失败；展示文本不携带快照、对象或 SQLite 细节。
#[derive(Debug)]
pub(crate) enum CloseCommitError {
    Project(ProjectStoreError),
    Close(CloseStoreError),
    Database(DatabaseError),
}

impl fmt::Display for CloseCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Project(_) => "关闭提交的项目快照保存失败",
            Self::Close(_) => "关闭提交的意图登记或状态推进失败",
            Self::Database(_) => "关闭提交数据库事务失败",
        })
    }
}

impl std::error::Error for CloseCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(error) => Some(error),
            Self::Close(error) => Some(error),
            Self::Database(error) => Some(error),
        }
    }
}

impl From<DatabaseError> for CloseCommitError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for CloseCommitError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl From<DatabaseError> for CloseStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for CloseStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    /// 在一个 Immediate 事务内登记完整头记录与全部对象，包括空关闭集合。
    pub(crate) fn register_close_intent(
        &mut self,
        registration: &PersistedCloseIntent,
    ) -> Result<(), CloseStoreError> {
        let encoded = EncodedRegistration::new(registration)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::register_close_intent_in_transaction(&transaction, &encoded)?;
        transaction.commit()?;
        Ok(())
    }

    /// 在调用方事务内登记已完成纯校验与编码的头记录及全部对象；本函数不自行提交。
    fn register_close_intent_in_transaction(
        transaction: &Connection,
        registration: &EncodedRegistration<'_>,
    ) -> Result<(), CloseStoreError> {
        if !store_epoch_is_current(transaction, registration.store_epoch)? {
            return Err(CloseStoreError::StoreEpochConflict);
        }
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM close_intents WHERE close_id = ?1)",
            [registration.close_id],
            |row| row.get::<_, bool>(0),
        )?;
        if exists {
            return Err(CloseStoreError::CloseIdConflict);
        }
        transaction.execute(
            "INSERT INTO close_intents \
             (close_id, scope, project_id, store_epoch, commit_fingerprint, commit_status) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'collected')",
            params![
                registration.close_id,
                registration.scope,
                registration.project_id,
                registration.store_epoch,
                registration.fingerprint.as_slice(),
            ],
        )?;
        for object in &registration.objects {
            transaction.execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    registration.close_id,
                    object.key,
                    object.generation,
                    object.decision,
                ],
            )?;
        }
        Ok(())
    }

    /// 在同一 Immediate 事务保存快照、登记关闭意图并推进到 StateCommitted。
    pub(crate) fn commit_close_intent(
        &mut self,
        snapshot: &ProjectSnapshot,
        registration: &PersistedCloseIntent,
    ) -> Result<PersistedCloseIntent, CloseCommitError> {
        let snapshot = PreparedProjectSnapshot::new(snapshot).map_err(CloseCommitError::Project)?;
        let registration =
            EncodedRegistration::new(registration).map_err(CloseCommitError::Close)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::save_project_snapshot_in_transaction(&transaction, &snapshot)
            .map_err(CloseCommitError::Project)?;
        Self::register_close_intent_in_transaction(&transaction, &registration)
            .map_err(CloseCommitError::Close)?;
        let updated = advance_close_intent_in_transaction(
            &transaction,
            registration.close_id,
            CloseCommitStatus::Collected,
            CloseCommitStatus::StateCommitted,
            registration.fingerprint,
            registration.store_epoch,
        )
        .map_err(CloseCommitError::Close)?;
        transaction.commit()?;
        Ok(updated)
    }

    /// 头与对象在同一 Immediate 事务读取，不混合并发提交世代。
    pub(crate) fn load_close_intent(
        &mut self,
        close_id: &str,
    ) -> Result<Option<PersistedCloseIntent>, CloseStoreError> {
        if close_id.is_empty() {
            return Err(CloseStoreError::InvalidRegistration);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let loaded = load_close_intent_from(&transaction, close_id)?;
        transaction.commit()?;
        Ok(loaded)
    }

    /// 按 close_id 与 object_key 确定性返回全部非 Completed 事实。
    pub(crate) fn unfinished_close_intents(
        &mut self,
    ) -> Result<Vec<PersistedCloseIntent>, CloseStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut statement =
            transaction.prepare("SELECT close_id FROM close_intents ORDER BY close_id")?;
        let ids = statement
            .query_map([], |row| {
                Ok(row
                    .get::<_, String>(0)
                    .map_err(|_| CloseStoreError::CorruptRecord))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut unfinished = Vec::new();
        for close_id in ids {
            let intent = load_close_intent_from(&transaction, &close_id)?
                .ok_or(CloseStoreError::CorruptRecord)?;
            if intent.commit_status != CloseCommitStatus::Completed {
                unfinished.push(intent);
            }
        }
        transaction.commit()?;
        Ok(unfinished)
    }

    /// 在同一 Immediate 事务核对身份、旧状态、指纹与当前世代后推进一步。
    pub(crate) fn advance_close_intent(
        &mut self,
        close_id: &str,
        expected_status: CloseCommitStatus,
        next: CloseCommitStatus,
        expected_fingerprint: &[u8; 32],
        current_store_epoch: &str,
    ) -> Result<PersistedCloseIntent, CloseStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = advance_close_intent_in_transaction(
            &transaction,
            close_id,
            expected_status,
            next,
            expected_fingerprint,
            current_store_epoch,
        )?;
        transaction.commit()?;
        Ok(updated)
    }
}

/// 供项目快照与关闭状态未来共享同一写事务；本函数不自行提交。
pub(super) fn advance_close_intent_in_transaction(
    connection: &Connection,
    close_id: &str,
    expected_status: CloseCommitStatus,
    next: CloseCommitStatus,
    expected_fingerprint: &[u8; 32],
    current_store_epoch: &str,
) -> Result<PersistedCloseIntent, CloseStoreError> {
    if close_id.is_empty() || current_store_epoch.is_empty() {
        return Err(CloseStoreError::InvalidRegistration);
    }
    let mut current =
        load_close_intent_from(connection, close_id)?.ok_or(CloseStoreError::UnknownCloseIntent)?;
    if current.store_epoch != current_store_epoch
        || !store_epoch_is_current(connection, current_store_epoch)?
    {
        return Err(CloseStoreError::StoreEpochConflict);
    }
    if current.commit_status != expected_status
        || current.intent.fingerprint() != expected_fingerprint
    {
        return Err(CloseStoreError::CloseConflict);
    }
    if expected_status.successor() != Some(next) {
        return Err(CloseStoreError::InvalidTransition);
    }
    let changed = connection.execute(
        "UPDATE close_intents SET commit_status = ?2 \
         WHERE close_id = ?1 AND commit_status = ?3 \
         AND commit_fingerprint = ?4 AND store_epoch = ?5",
        params![
            close_id,
            status_name(next),
            status_name(expected_status),
            expected_fingerprint.as_slice(),
            current_store_epoch,
        ],
    )?;
    if changed != 1 {
        return Err(CloseStoreError::CloseConflict);
    }
    current.commit_status = next;
    Ok(current)
}

struct EncodedRegistration<'a> {
    close_id: &'a str,
    store_epoch: &'a str,
    fingerprint: &'a [u8; 32],
    scope: &'static str,
    project_id: Option<String>,
    objects: Vec<EncodedCloseObject>,
}

struct EncodedCloseObject {
    key: String,
    generation: i64,
    decision: &'static str,
}

impl<'a> EncodedRegistration<'a> {
    fn new(registration: &'a PersistedCloseIntent) -> Result<Self, CloseStoreError> {
        if registration.close_id.is_empty()
            || registration.store_epoch.is_empty()
            || registration.commit_status != CloseCommitStatus::Collected
        {
            return Err(CloseStoreError::InvalidRegistration);
        }
        let (scope, project_id) = encode_scope(&registration.scope);
        let objects = registration
            .intent
            .objects()
            .map(|object| {
                if object.id.is_empty() {
                    return Err(CloseStoreError::InvalidRegistration);
                }
                Ok(EncodedCloseObject {
                    key: object.id.clone(),
                    generation: i64::try_from(object.generation)
                        .map_err(|_| CloseStoreError::InvalidRegistration)?,
                    decision: decision_name(object.decision),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            close_id: &registration.close_id,
            store_epoch: &registration.store_epoch,
            fingerprint: registration.intent.fingerprint(),
            scope,
            project_id,
            objects,
        })
    }
}

struct CloseHeadRow {
    close_id: String,
    scope: String,
    project_id: Option<String>,
    store_epoch: String,
    fingerprint: Vec<u8>,
    status: String,
}

struct CloseObjectRow {
    key: String,
    generation: i64,
    decision: String,
}

fn load_close_intent_from(
    connection: &Connection,
    close_id: &str,
) -> Result<Option<PersistedCloseIntent>, CloseStoreError> {
    let head = connection
        .query_row(
            "SELECT close_id, scope, project_id, store_epoch, commit_fingerprint, commit_status \
             FROM close_intents WHERE close_id = ?1",
            [close_id],
            |row| Ok(decode_head(row)),
        )
        .optional()?
        .transpose()?;
    let Some(head) = head else {
        return Ok(None);
    };
    if head.close_id.is_empty() || head.store_epoch.is_empty() {
        return Err(CloseStoreError::CorruptRecord);
    }
    let scope = decode_scope(&head.scope, head.project_id)?;
    let commit_status = decode_status(&head.status)?;
    let fingerprint: [u8; 32] = head
        .fingerprint
        .try_into()
        .map_err(|_| CloseStoreError::CorruptRecord)?;

    let mut statement = connection.prepare(
        "SELECT object_key, object_generation, decision FROM close_objects \
         WHERE close_id = ?1 ORDER BY object_key",
    )?;
    let rows = statement
        .query_map([&head.close_id], |row| Ok(decode_object(row)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let objects = rows
        .into_iter()
        .map(|object| {
            if object.key.is_empty() {
                return Err(CloseStoreError::CorruptRecord);
            }
            Ok(CloseObject {
                id: object.key,
                generation: u64::try_from(object.generation)
                    .map_err(|_| CloseStoreError::CorruptRecord)?,
                decision: decode_decision(&object.decision)?,
            })
        })
        .collect::<Result<Vec<_>, CloseStoreError>>()?;
    let intent =
        CloseIntent::new(objects, fingerprint).map_err(|_| CloseStoreError::CorruptRecord)?;
    Ok(Some(PersistedCloseIntent {
        close_id: head.close_id,
        scope,
        store_epoch: head.store_epoch,
        intent,
        commit_status,
    }))
}

fn decode_head(row: &Row<'_>) -> Result<CloseHeadRow, CloseStoreError> {
    Ok(CloseHeadRow {
        close_id: required(row, 0)?,
        scope: required(row, 1)?,
        project_id: required(row, 2)?,
        store_epoch: required(row, 3)?,
        fingerprint: required(row, 4)?,
        status: required(row, 5)?,
    })
}

fn decode_object(row: &Row<'_>) -> Result<CloseObjectRow, CloseStoreError> {
    Ok(CloseObjectRow {
        key: required(row, 0)?,
        generation: required(row, 1)?,
        decision: required(row, 2)?,
    })
}

fn required<T: FromSql>(row: &Row<'_>, index: usize) -> Result<T, CloseStoreError> {
    row.get(index).map_err(|_| CloseStoreError::CorruptRecord)
}

fn encode_scope(scope: &CloseScope) -> (&'static str, Option<String>) {
    match scope {
        CloseScope::Panels { project_id } => ("panels", Some(project_id.to_string())),
        CloseScope::Project { project_id } => ("project", Some(project_id.to_string())),
        CloseScope::ApplicationExit => ("application_exit", None),
        CloseScope::UpdateHandoff => ("update_handoff", None),
    }
}

fn decode_scope(scope: &str, project_id: Option<String>) -> Result<CloseScope, CloseStoreError> {
    fn required_project(project_id: Option<String>) -> Result<ProjectId, CloseStoreError> {
        let project_id = project_id.ok_or(CloseStoreError::CorruptRecord)?;
        if project_id.is_empty() {
            return Err(CloseStoreError::CorruptRecord);
        }
        ProjectId::from_str(&project_id).map_err(|_| CloseStoreError::CorruptRecord)
    }

    fn reject_project(project_id: Option<String>) -> Result<(), CloseStoreError> {
        if project_id.is_some() {
            Err(CloseStoreError::CorruptRecord)
        } else {
            Ok(())
        }
    }

    match scope {
        "panels" => Ok(CloseScope::Panels {
            project_id: required_project(project_id)?,
        }),
        "project" => Ok(CloseScope::Project {
            project_id: required_project(project_id)?,
        }),
        "application_exit" => {
            reject_project(project_id)?;
            Ok(CloseScope::ApplicationExit)
        }
        "update_handoff" => {
            reject_project(project_id)?;
            Ok(CloseScope::UpdateHandoff)
        }
        _ => Err(CloseStoreError::CorruptRecord),
    }
}

const fn decision_name(decision: CloseDecision) -> &'static str {
    match decision {
        CloseDecision::Save => "save",
        CloseDecision::Discard => "discard",
        CloseDecision::NotRequired => "not_required",
    }
}

fn decode_decision(decision: &str) -> Result<CloseDecision, CloseStoreError> {
    match decision {
        "save" => Ok(CloseDecision::Save),
        "discard" => Ok(CloseDecision::Discard),
        "not_required" => Ok(CloseDecision::NotRequired),
        _ => Err(CloseStoreError::CorruptRecord),
    }
}

const fn status_name(status: CloseCommitStatus) -> &'static str {
    match status {
        CloseCommitStatus::Collected => "collected",
        CloseCommitStatus::StateCommitted => "state_committed",
        CloseCommitStatus::PtyStopped => "pty_stopped",
        CloseCommitStatus::CleanupCommitted => "cleanup_committed",
        CloseCommitStatus::Completed => "completed",
    }
}

fn decode_status(status: &str) -> Result<CloseCommitStatus, CloseStoreError> {
    match status {
        "collected" => Ok(CloseCommitStatus::Collected),
        "state_committed" => Ok(CloseCommitStatus::StateCommitted),
        "pty_stopped" => Ok(CloseCommitStatus::PtyStopped),
        "cleanup_committed" => Ok(CloseCommitStatus::CleanupCommitted),
        "completed" => Ok(CloseCommitStatus::Completed),
        _ => Err(CloseStoreError::CorruptRecord),
    }
}

#[cfg(test)]
mod close_tests {
    use std::time::{Duration, Instant};

    use rusqlite::{Connection, params};
    use slterm_path::{AppPaths, CanonicalPath};
    use slterm_workspace::{
        CloseDecision, CloseObject, CloseScope, PanelGroup, PanelGroupId, ProjectId,
        ProjectRegistry, ProjectSnapshot, ProjectSnapshotError, TerminalId, ViewKey,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::project::ProjectStoreError;

    const EPOCH: &str = "epoch-close";
    const OTHER_EPOCH: &str = "epoch-other";
    const CLOSE_ID: &str = "close-a";
    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn store(temp: &TempDir) -> WorkspaceDatabase {
        let mut store = WorkspaceDatabase::open(&fixture(temp)).unwrap();
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: "run-close".to_owned(),
                    process_id: 17,
                    process_created_at_ms: 456,
                },
            )
            .unwrap();
        store
    }

    fn project_id(text: &str) -> ProjectId {
        text.parse().unwrap()
    }

    fn object(key: &str, generation: u64, decision: CloseDecision) -> CloseObject<String> {
        CloseObject {
            id: key.to_owned(),
            generation,
            decision,
        }
    }

    fn registration(
        close_id: &str,
        scope: CloseScope,
        objects: Vec<CloseObject<String>>,
    ) -> PersistedCloseIntent {
        PersistedCloseIntent {
            close_id: close_id.to_owned(),
            scope,
            store_epoch: EPOCH.to_owned(),
            intent: CloseIntent::new(objects, [7; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        }
    }

    fn empty_registration(close_id: &str) -> PersistedCloseIntent {
        registration(close_id, CloseScope::ApplicationExit, Vec::new())
    }

    fn object_registration(close_id: &str) -> PersistedCloseIntent {
        registration(
            close_id,
            CloseScope::Panels {
                project_id: project_id(PROJECT_A),
            },
            vec![object("terminal:a", 3, CloseDecision::Save)],
        )
    }

    fn close_snapshot(name: &str) -> ProjectSnapshot {
        let project_id = project_id(PROJECT_A);
        let mut registry = ProjectRegistry::new();
        registry
            .add_project(project_id, CanonicalPath::new("D:/work/close").unwrap())
            .unwrap();
        registry.rename_project(project_id, name).unwrap();
        ProjectSnapshot::restore(registry, Vec::new()).unwrap()
    }

    fn invalid_close_snapshot() -> ProjectSnapshot {
        let mut snapshot = close_snapshot("invalid");
        let project_id = project_id(PROJECT_A);
        let view = ViewKey::Terminal {
            project_id,
            terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a"
                .parse::<TerminalId>()
                .unwrap(),
        };
        snapshot
            .registry_mut()
            .layout_mut(project_id)
            .unwrap()
            .open_first_group(
                "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a"
                    .parse::<PanelGroupId>()
                    .unwrap(),
                PanelGroup::new(vec![view.clone()], view).unwrap(),
            )
            .unwrap();
        snapshot
    }

    fn advance(
        store: &mut WorkspaceDatabase,
        current: &PersistedCloseIntent,
        next: CloseCommitStatus,
    ) -> PersistedCloseIntent {
        store
            .advance_close_intent(
                &current.close_id,
                current.commit_status,
                next,
                current.intent.fingerprint(),
                EPOCH,
            )
            .unwrap()
    }

    fn complete(store: &mut WorkspaceDatabase, current: &PersistedCloseIntent) {
        let mut current = current.clone();
        for next in [
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            CloseCommitStatus::Completed,
        ] {
            current = advance(store, &current, next);
        }
    }

    fn count(store: &WorkspaceDatabase, table: &str) -> i64 {
        store
            .database
            .connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn assert_preflight_did_not_wait(elapsed: Duration) {
        assert!(
            elapsed < Duration::from_secs(1),
            "事务前纯校验不得等待 busy_timeout"
        );
    }

    #[test]
    fn close_intent_empty_and_multi_object_records_round_trip() {
        let empty_temp = tempfile::tempdir().unwrap();
        let paths = fixture(&empty_temp);
        let mut empty_store = store(&empty_temp);
        let empty = empty_registration(CLOSE_ID);
        empty_store.register_close_intent(&empty).unwrap();
        assert_eq!(count(&empty_store, "close_intents"), 1);
        assert_eq!(count(&empty_store, "close_objects"), 0);
        drop(empty_store);
        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(reopened.load_close_intent(CLOSE_ID).unwrap(), Some(empty));
        assert_eq!(reopened.load_close_intent("close-missing").unwrap(), None);

        let object_temp = tempfile::tempdir().unwrap();
        let mut store = store(&object_temp);
        let expected = registration(
            CLOSE_ID,
            CloseScope::Project {
                project_id: project_id(PROJECT_A),
            },
            vec![
                object("terminal:z", 9, CloseDecision::NotRequired),
                object("document:a", 2, CloseDecision::Discard),
                object("terminal:a", 3, CloseDecision::Save),
            ],
        );
        store.register_close_intent(&expected).unwrap();
        let loaded = store.load_close_intent(CLOSE_ID).unwrap().unwrap();
        assert_eq!(loaded, expected);
        assert_eq!(
            loaded
                .intent
                .objects()
                .map(|object| object.id.as_str())
                .collect::<Vec<_>>(),
            ["document:a", "terminal:a", "terminal:z"]
        );
    }

    #[test]
    fn close_id_conflict_preserves_original_header_and_objects() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let original = object_registration(CLOSE_ID);
        store.register_close_intent(&original).unwrap();
        let replacement = registration(
            CLOSE_ID,
            CloseScope::UpdateHandoff,
            vec![object("document:replacement", 99, CloseDecision::Discard)],
        );

        assert!(matches!(
            store.register_close_intent(&replacement),
            Err(CloseStoreError::CloseIdConflict)
        ));
        assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(original));
        assert_eq!(count(&store, "close_intents"), 1);
        assert_eq!(count(&store, "close_objects"), 1);
    }

    #[test]
    fn unfinished_close_intents_are_ordered_and_exclude_only_completed() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        for close_id in ["close-c", "close-a", "close-b"] {
            let current = registration(
                close_id,
                CloseScope::ApplicationExit,
                vec![
                    object("terminal:z", 1, CloseDecision::Discard),
                    object("document:a", 2, CloseDecision::Save),
                ],
            );
            store.register_close_intent(&current).unwrap();
            if close_id == "close-c" {
                complete(&mut store, &current);
            }
        }

        let unfinished = store.unfinished_close_intents().unwrap();
        assert_eq!(
            unfinished
                .iter()
                .map(|intent| intent.close_id.as_str())
                .collect::<Vec<_>>(),
            ["close-a", "close-b"]
        );
        for intent in &unfinished {
            assert_eq!(
                intent
                    .intent
                    .objects()
                    .map(|object| object.id.as_str())
                    .collect::<Vec<_>>(),
                ["document:a", "terminal:z"]
            );
        }
        assert_eq!(
            store
                .load_close_intent("close-c")
                .unwrap()
                .unwrap()
                .commit_status,
            CloseCommitStatus::Completed
        );
        assert_eq!(count(&store, "close_intents"), 3);
    }

    #[test]
    fn close_registration_preflight_precedes_write_lock() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&temp);
        let lock = WorkspaceDatabase::open(&paths).unwrap();
        lock.database
            .connection
            .execute_batch("BEGIN EXCLUSIVE")
            .unwrap();

        let started = Instant::now();
        let result = store.register_close_intent(&empty_registration(""));
        let elapsed = started.elapsed();

        lock.database.connection.execute_batch("ROLLBACK").unwrap();
        assert!(matches!(result, Err(CloseStoreError::InvalidRegistration)));
        assert_preflight_did_not_wait(elapsed);
    }

    #[test]
    fn invalid_registrations_write_no_partial_records() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let mut invalid = Vec::new();
        invalid.push(empty_registration(""));
        let mut empty_epoch = empty_registration(CLOSE_ID);
        empty_epoch.store_epoch.clear();
        invalid.push(empty_epoch);
        invalid.push(registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![object("", 0, CloseDecision::Save)],
        ));
        invalid.push(registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![object("terminal:a", u64::MAX, CloseDecision::Save)],
        ));
        let mut non_initial = empty_registration(CLOSE_ID);
        non_initial.commit_status = CloseCommitStatus::StateCommitted;
        invalid.push(non_initial);

        for candidate in invalid {
            assert!(matches!(
                store.register_close_intent(&candidate),
                Err(CloseStoreError::InvalidRegistration)
            ));
        }
        assert_eq!(count(&store, "close_intents"), 0);
        assert_eq!(count(&store, "close_objects"), 0);

        let stale = PersistedCloseIntent {
            store_epoch: OTHER_EPOCH.to_owned(),
            ..empty_registration(CLOSE_ID)
        };
        assert!(matches!(
            store.register_close_intent(&stale),
            Err(CloseStoreError::StoreEpochConflict)
        ));
        assert_eq!(count(&store, "close_intents"), 0);
    }

    #[test]
    fn close_object_failure_rolls_back_header_and_prior_objects() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_close_object BEFORE INSERT ON close_objects \
                 WHEN NEW.object_key = 'terminal:b' \
                 BEGIN SELECT RAISE(ABORT, 'injected close object failure'); END;",
            )
            .unwrap();
        let candidate = registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![
                object("terminal:a", 1, CloseDecision::Save),
                object("terminal:b", 2, CloseDecision::Discard),
            ],
        );

        assert!(matches!(
            store.register_close_intent(&candidate),
            Err(CloseStoreError::Database(_))
        ));
        assert_eq!(count(&store, "close_intents"), 0);
        assert_eq!(count(&store, "close_objects"), 0);
    }

    #[test]
    fn close_commit_saves_snapshot_and_advances_intent_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let snapshot = close_snapshot("committed");
        let collected = object_registration(CLOSE_ID);

        let committed = store.commit_close_intent(&snapshot, &collected).unwrap();

        assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
        assert_eq!(store.load_project_snapshot().unwrap(), snapshot);
        assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(committed));
    }

    #[test]
    fn close_commit_rejects_invalid_snapshot_without_registration() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);

        assert!(matches!(
            store.commit_close_intent(&invalid_close_snapshot(), &object_registration(CLOSE_ID)),
            Err(CloseCommitError::Project(_))
        ));
        assert_eq!(
            store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
        assert_eq!(count(&store, "close_intents"), 0);
        assert_eq!(count(&store, "close_objects"), 0);
    }

    #[test]
    fn close_commit_snapshot_preflight_precedes_write_lock() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&temp);
        let lock = WorkspaceDatabase::open(&paths).unwrap();
        lock.database
            .connection
            .execute_batch("BEGIN EXCLUSIVE")
            .unwrap();

        let started = Instant::now();
        let result =
            store.commit_close_intent(&invalid_close_snapshot(), &object_registration(CLOSE_ID));
        let elapsed = started.elapsed();

        lock.database.connection.execute_batch("ROLLBACK").unwrap();
        let terminal_id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a"
            .parse::<TerminalId>()
            .unwrap();
        assert!(matches!(
            result,
            Err(CloseCommitError::Project(ProjectStoreError::Snapshot(
                ProjectSnapshotError::MissingTerminal(missing)
            ))) if missing == terminal_id
        ));
        assert_preflight_did_not_wait(elapsed);
    }

    #[test]
    fn close_commit_registration_preflight_precedes_write_lock() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = store(&temp);
        let lock = WorkspaceDatabase::open(&paths).unwrap();
        lock.database
            .connection
            .execute_batch("BEGIN EXCLUSIVE")
            .unwrap();

        let started = Instant::now();
        let result = store.commit_close_intent(&close_snapshot("valid"), &empty_registration(""));
        let elapsed = started.elapsed();

        lock.database.connection.execute_batch("ROLLBACK").unwrap();
        assert!(matches!(
            result,
            Err(CloseCommitError::Close(
                CloseStoreError::InvalidRegistration
            ))
        ));
        assert_preflight_did_not_wait(elapsed);
    }

    #[test]
    fn close_commit_close_id_conflict_rolls_back_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let original_snapshot = close_snapshot("original");
        let original_intent = object_registration(CLOSE_ID);
        store.save_project_snapshot(&original_snapshot).unwrap();
        store.register_close_intent(&original_intent).unwrap();
        let replacement = registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![object("terminal:replacement", 8, CloseDecision::Discard)],
        );

        assert!(matches!(
            store.commit_close_intent(&close_snapshot("replacement"), &replacement),
            Err(CloseCommitError::Close(CloseStoreError::CloseIdConflict))
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original_snapshot);
        assert_eq!(
            store.load_close_intent(CLOSE_ID).unwrap(),
            Some(original_intent)
        );
    }

    #[test]
    fn close_commit_object_failure_rolls_back_snapshot_and_records() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_commit_close_object BEFORE INSERT ON close_objects \
                 WHEN NEW.object_key = 'terminal:b' \
                 BEGIN SELECT RAISE(ABORT, 'injected close commit failure'); END;",
            )
            .unwrap();
        let collected = registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![
                object("terminal:a", 1, CloseDecision::Save),
                object("terminal:b", 2, CloseDecision::Discard),
            ],
        );

        assert!(matches!(
            store.commit_close_intent(&close_snapshot("rolled-back"), &collected),
            Err(CloseCommitError::Close(CloseStoreError::Database(_)))
        ));
        assert_eq!(
            store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
        assert_eq!(count(&store, "close_intents"), 0);
        assert_eq!(count(&store, "close_objects"), 0);
    }

    #[test]
    fn close_commit_state_failure_rolls_back_snapshot_header_and_objects() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let original = close_snapshot("original");
        store.save_project_snapshot(&original).unwrap();
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_state_commit BEFORE UPDATE ON close_intents \
                 WHEN NEW.commit_status = 'state_committed' \
                 BEGIN SELECT RAISE(ABORT, 'injected state commit failure'); END;",
            )
            .unwrap();
        let collected = registration(
            CLOSE_ID,
            CloseScope::ApplicationExit,
            vec![
                object("terminal:a", 1, CloseDecision::Save),
                object("terminal:b", 2, CloseDecision::Discard),
            ],
        );

        assert!(matches!(
            store.commit_close_intent(&close_snapshot("rolled-back"), &collected),
            Err(CloseCommitError::Close(CloseStoreError::Database(_)))
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
        assert_eq!(count(&store, "close_intents"), 0);
        assert_eq!(count(&store, "close_objects"), 0);
    }

    #[test]
    fn close_commit_errors_do_not_echo_persisted_details() {
        let marker = "sensitive-close-commit-path-sql-fingerprint";
        let connection = Connection::open_in_memory().unwrap();
        let sqlite = connection
            .execute_batch(&format!("SELECT * FROM {marker}"))
            .unwrap_err();
        let database = DatabaseError::from(sqlite);
        let errors = [
            CloseCommitError::Project(ProjectStoreError::Database(database)),
            CloseCommitError::Close(CloseStoreError::CloseIdConflict),
        ];

        for error in errors {
            assert!(!error.to_string().contains(marker));
        }
    }

    #[test]
    fn corrupt_close_headers_are_reported_without_repair() {
        let cases = [
            (
                "unknown scope",
                "UPDATE close_intents SET scope = 'unknown'",
            ),
            (
                "unknown status",
                "UPDATE close_intents SET commit_status = 'unknown'",
            ),
            (
                "short fingerprint",
                "UPDATE close_intents SET commit_fingerprint = zeroblob(31)",
            ),
            ("empty epoch", "UPDATE close_intents SET store_epoch = ''"),
            (
                "global project mismatch",
                "UPDATE close_intents SET project_id = 'project-unexpected'",
            ),
            ("non-text scope", "UPDATE close_intents SET scope = x'ff'"),
        ];
        for (name, corruption) in cases {
            let temp = tempfile::tempdir().unwrap();
            let mut store = store(&temp);
            store
                .register_close_intent(&empty_registration(CLOSE_ID))
                .unwrap();
            store
                .database
                .connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            store.database.connection.execute(corruption, []).unwrap();

            assert!(
                matches!(
                    store.load_close_intent(CLOSE_ID),
                    Err(CloseStoreError::CorruptRecord)
                ),
                "{name} 必须报告损坏"
            );
            assert!(
                matches!(
                    store.unfinished_close_intents(),
                    Err(CloseStoreError::CorruptRecord)
                ),
                "{name} 不得在未完成枚举中被跳过"
            );
            assert_eq!(count(&store, "close_intents"), 1);
        }
    }

    #[test]
    fn corrupt_close_objects_and_project_scopes_are_reported() {
        let object_cases = [
            (
                "unknown decision",
                "UPDATE close_objects SET decision = 'unknown'",
            ),
            (
                "negative generation",
                "UPDATE close_objects SET object_generation = -1",
            ),
            (
                "empty object key",
                "UPDATE close_objects SET object_key = ''",
            ),
            (
                "non-text decision",
                "UPDATE close_objects SET decision = x'ff'",
            ),
        ];
        for (name, corruption) in object_cases {
            let temp = tempfile::tempdir().unwrap();
            let mut store = store(&temp);
            store
                .register_close_intent(&object_registration(CLOSE_ID))
                .unwrap();
            store
                .database
                .connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            store.database.connection.execute(corruption, []).unwrap();
            assert!(
                matches!(
                    store.load_close_intent(CLOSE_ID),
                    Err(CloseStoreError::CorruptRecord)
                ),
                "{name} 必须报告损坏"
            );
        }

        let project_cases = [
            ("missing project", None),
            ("invalid project", Some("not-a-project")),
        ];
        for (name, project) in project_cases {
            let temp = tempfile::tempdir().unwrap();
            let mut store = store(&temp);
            store
                .register_close_intent(&registration(
                    CLOSE_ID,
                    CloseScope::Project {
                        project_id: project_id(PROJECT_A),
                    },
                    Vec::new(),
                ))
                .unwrap();
            store
                .database
                .connection
                .execute("UPDATE close_intents SET project_id = ?1", [project])
                .unwrap();
            assert!(
                matches!(
                    store.load_close_intent(CLOSE_ID),
                    Err(CloseStoreError::CorruptRecord)
                ),
                "{name} 必须报告损坏"
            );
        }
    }

    #[test]
    fn empty_persisted_close_id_is_corrupt_and_input_id_is_validated() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .register_close_intent(&empty_registration(CLOSE_ID))
            .unwrap();
        store
            .database
            .connection
            .execute("UPDATE close_intents SET close_id = ''", [])
            .unwrap();

        assert!(matches!(
            store.unfinished_close_intents(),
            Err(CloseStoreError::CorruptRecord)
        ));
        assert!(matches!(
            store.load_close_intent(""),
            Err(CloseStoreError::InvalidRegistration)
        ));
    }

    #[test]
    fn close_status_advances_in_fixed_order_and_retains_history() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        assert_eq!(
            CloseCommitStatus::ALL,
            [
                CloseCommitStatus::Collected,
                CloseCommitStatus::StateCommitted,
                CloseCommitStatus::PtyStopped,
                CloseCommitStatus::CleanupCommitted,
                CloseCommitStatus::Completed,
            ]
        );
        let registered = object_registration(CLOSE_ID);
        store.register_close_intent(&registered).unwrap();
        let mut current = registered;
        for next in CloseCommitStatus::ALL.into_iter().skip(1) {
            let expected = current.commit_status;
            current = advance(&mut store, &current, next);
            assert_eq!(current.commit_status, next);
            assert_ne!(current.commit_status, expected);
            assert_eq!(
                store.load_close_intent(CLOSE_ID).unwrap(),
                Some(current.clone())
            );
        }
        assert!(store.unfinished_close_intents().unwrap().is_empty());
        assert_eq!(count(&store, "close_intents"), 1);
        assert_eq!(count(&store, "close_objects"), 1);
    }

    #[test]
    fn rejected_close_transitions_never_modify_records() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let original = object_registration(CLOSE_ID);
        store.register_close_intent(&original).unwrap();
        let fingerprint = original.intent.fingerprint();

        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::Collected,
                CloseCommitStatus::PtyStopped,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::InvalidTransition)
        ));
        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::Collected,
                CloseCommitStatus::Collected,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::InvalidTransition)
        ));
        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::StateCommitted,
                CloseCommitStatus::PtyStopped,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::CloseConflict)
        ));
        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::Collected,
                CloseCommitStatus::StateCommitted,
                &[8; 32],
                EPOCH,
            ),
            Err(CloseStoreError::CloseConflict)
        ));
        store
            .rotate_store_epoch("run-close", EPOCH, OTHER_EPOCH)
            .unwrap();
        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::Collected,
                CloseCommitStatus::StateCommitted,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::StoreEpochConflict)
        ));
        store
            .rotate_store_epoch("run-close", OTHER_EPOCH, EPOCH)
            .unwrap();
        assert!(matches!(
            store.advance_close_intent(
                "close-missing",
                CloseCommitStatus::Collected,
                CloseCommitStatus::StateCommitted,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::UnknownCloseIntent)
        ));
        assert_eq!(
            store.load_close_intent(CLOSE_ID).unwrap(),
            Some(original.clone())
        );

        let committed = advance(&mut store, &original, CloseCommitStatus::StateCommitted);
        assert!(matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::StateCommitted,
                CloseCommitStatus::Collected,
                fingerprint,
                EPOCH,
            ),
            Err(CloseStoreError::InvalidTransition)
        ));
        assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(committed));
    }

    #[test]
    fn close_schema_enforces_foreign_keys_nulls_and_value_constraints() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let connection = &store.database.connection;
        assert!(
            connection
                .execute(
                    "INSERT INTO close_objects \
                     (close_id, object_key, object_generation, decision) \
                     VALUES ('close-missing', 'terminal:a', 0, 'save')",
                    [],
                )
                .is_err()
        );
        connection
            .execute(
                "INSERT INTO close_intents \
                 (close_id, scope, project_id, store_epoch, commit_fingerprint, commit_status) \
                 VALUES (?1, 'application_exit', NULL, ?2, zeroblob(32), 'collected')",
                params![CLOSE_ID, EPOCH],
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO close_objects \
                     (close_id, object_key, object_generation, decision) \
                     VALUES (?1, NULL, 0, 'save')",
                    [CLOSE_ID],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO close_objects \
                     (close_id, object_key, object_generation, decision) \
                     VALUES (NULL, 'terminal:a', 0, 'save')",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO close_objects \
                     (close_id, object_key, object_generation, decision) \
                     VALUES (?1, 'terminal:a', -1, 'save')",
                    [CLOSE_ID],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO close_intents \
                     (close_id, scope, store_epoch, commit_fingerprint, commit_status) \
                     VALUES ('close-short', 'application_exit', ?1, zeroblob(31), 'collected')",
                    [EPOCH],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO close_intents \
                     (close_id, scope, store_epoch, commit_fingerprint, commit_status) \
                     VALUES (NULL, 'application_exit', ?1, zeroblob(32), 'collected')",
                    [EPOCH],
                )
                .is_err()
        );
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (?1, 'terminal:a', 0, 'save')",
                [CLOSE_ID],
            )
            .unwrap();
        assert!(
            connection
                .execute(
                    "INSERT INTO close_objects \
                     (close_id, object_key, object_generation, decision) \
                     VALUES (?1, 'terminal:a', 1, 'discard')",
                    [CLOSE_ID],
                )
                .is_err()
        );
    }

    #[test]
    fn close_store_errors_do_not_echo_persisted_details() {
        let marker = "sensitive-object-path-sql-fingerprint";
        let connection = Connection::open_in_memory().unwrap();
        let sqlite = connection
            .execute_batch(&format!("SELECT * FROM {marker}"))
            .unwrap_err();
        let errors = [
            CloseStoreError::InvalidRegistration,
            CloseStoreError::CloseIdConflict,
            CloseStoreError::UnknownCloseIntent,
            CloseStoreError::CloseConflict,
            CloseStoreError::InvalidTransition,
            CloseStoreError::StoreEpochConflict,
            CloseStoreError::CorruptRecord,
            CloseStoreError::Database(DatabaseError::from(sqlite)),
        ];
        for error in errors {
            assert!(!error.to_string().contains(marker));
        }
    }

    #[test]
    fn close_scope_round_trips_update_handoff_without_project() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let expected = registration(CLOSE_ID, CloseScope::UpdateHandoff, Vec::new());
        store.register_close_intent(&expected).unwrap();
        assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(expected));
        assert_eq!(
            store
                .database
                .connection
                .query_row(
                    "SELECT project_id FROM close_intents WHERE close_id = ?1",
                    [CLOSE_ID],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap(),
            None
        );
    }
}
