//! 关闭意图的规范化持久记录与顺序提交端口。

use std::fmt;
use std::str::FromStr;

use rusqlite::types::FromSql;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use slterm_workspace::{
    CloseDecision, CloseIntent, CloseObject, CloseScope, ProjectId, ProjectSnapshot,
    ViewStateRegistry,
};

use super::database::DatabaseError;
use super::metadata::store_epoch_is_current;
use super::project::{PreparedProjectSnapshot, ProjectStoreError};
use super::view_state::{ViewStateStoreError, delete_project_view_states_in_transaction};
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

/// 项目关闭清理失败；展示文本按持久事实分类且不泄露存储细节。
#[derive(Debug)]
pub(crate) enum ProjectCleanupError {
    ViewState(ViewStateStoreError),
    Close(CloseStoreError),
    Database(DatabaseError),
}

impl fmt::Display for ProjectCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ViewState(_) => "项目关闭的 ViewState 清理失败",
            Self::Close(_) => "项目关闭的意图状态推进失败",
            Self::Database(_) => "项目关闭清理数据库事务失败",
        })
    }
}

impl std::error::Error for ProjectCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ViewState(error) => Some(error),
            Self::Close(error) => Some(error),
            Self::Database(error) => Some(error),
        }
    }
}

impl From<DatabaseError> for ProjectCleanupError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for ProjectCleanupError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
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
        if matches!(registration.scope, CloseScope::Project { .. }) {
            // 项目删除必须保留目标记录到专用 cleanup，不能走普通即时删除。
            return Err(CloseCommitError::Close(
                CloseStoreError::InvalidRegistration,
            ));
        }
        self.commit_close_intent_with_retention(snapshot, registration, None)
    }

    /// 项目关闭初始提交保存其他项目，但保留目标项目及其终端供后续清理。
    pub(crate) fn commit_project_close_intent(
        &mut self,
        snapshot: &ProjectSnapshot,
        registration: &PersistedCloseIntent,
    ) -> Result<PersistedCloseIntent, CloseCommitError> {
        let CloseScope::Project { project_id } = registration.scope else {
            return Err(CloseCommitError::Close(
                CloseStoreError::InvalidRegistration,
            ));
        };
        if snapshot.registry().project(project_id).is_some()
            || snapshot
                .terminals()
                .any(|terminal| terminal.project_id() == project_id)
        {
            return Err(CloseCommitError::Close(CloseStoreError::CloseConflict));
        }
        self.commit_close_intent_with_retention(snapshot, registration, Some(project_id))
    }

    fn commit_close_intent_with_retention(
        &mut self,
        snapshot: &ProjectSnapshot,
        registration: &PersistedCloseIntent,
        retained_project: Option<ProjectId>,
    ) -> Result<PersistedCloseIntent, CloseCommitError> {
        let snapshot = PreparedProjectSnapshot::new(snapshot).map_err(CloseCommitError::Project)?;
        let registration =
            EncodedRegistration::new(registration).map_err(CloseCommitError::Close)?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::save_project_snapshot_in_transaction(&transaction, &snapshot, retained_project)
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

    /// 在项目关闭清理事务中删除视图状态、项目聚合并推进到 CleanupCommitted。
    pub(crate) fn cleanup_project_close(
        &mut self,
        current: &PersistedCloseIntent,
        registry: &ViewStateRegistry,
    ) -> Result<PersistedCloseIntent, ProjectCleanupError> {
        let CloseScope::Project { project_id } = current.scope else {
            return Err(ProjectCleanupError::Close(
                CloseStoreError::InvalidRegistration,
            ));
        };
        if current.commit_status != CloseCommitStatus::PtyStopped {
            return Err(ProjectCleanupError::Close(CloseStoreError::CloseConflict));
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        delete_project_view_states_in_transaction(&transaction, project_id, registry)
            .map_err(ProjectCleanupError::ViewState)?;
        let deleted = transaction.execute(
            "DELETE FROM projects WHERE project_id = ?1",
            [project_id.to_string()],
        )?;
        if deleted != 1 {
            return Err(ProjectCleanupError::Close(CloseStoreError::CloseConflict));
        }
        let updated = advance_close_intent_in_transaction(
            &transaction,
            &current.close_id,
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            current.intent.fingerprint(),
            &current.store_epoch,
        )
        .map_err(ProjectCleanupError::Close)?;
        if updated.scope != current.scope {
            return Err(ProjectCleanupError::Close(CloseStoreError::CloseConflict));
        }
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
        if matches!(updated.scope, CloseScope::Project { .. })
            && expected_status == CloseCommitStatus::PtyStopped
            && next == CloseCommitStatus::CleanupCommitted
        {
            // 项目 CleanupCommitted 必须由实际清理事务证明，不能只推进头状态。
            return Err(CloseStoreError::InvalidTransition);
        }
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
#[path = "close_tests.rs"]
mod close_tests;
