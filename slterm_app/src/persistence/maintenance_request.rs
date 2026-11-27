//! 维护请求与恢复点的持久事实：启动维护期据此应用或回滚 staged 材料。

use std::fmt;

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::database::DatabaseError;
use super::metadata::store_epoch_is_current;
use super::workspace::WorkspaceDatabase;

/// 维护请求的显式状态机；只有恢复器能推动合法边。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaintenanceRequestStatus {
    Prepared,
    Applying,
    Applied,
    RolledBack,
    NeedsAttention,
}

impl MaintenanceRequestStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "Prepared",
            Self::Applying => "Applying",
            Self::Applied => "Applied",
            Self::RolledBack => "RolledBack",
            Self::NeedsAttention => "NeedsAttention",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "Prepared" => Some(Self::Prepared),
            "Applying" => Some(Self::Applying),
            "Applied" => Some(Self::Applied),
            "RolledBack" => Some(Self::RolledBack),
            "NeedsAttention" => Some(Self::NeedsAttention),
            _ => None,
        }
    }

    /// 终态请求不再进入未完成枚举；NeedsAttention 仍需人工处理。
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Applied | Self::RolledBack)
    }

    const fn allows(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Prepared,
                Self::Applying | Self::RolledBack | Self::NeedsAttention
            ) | (
                Self::Applying,
                Self::Applied | Self::RolledBack | Self::NeedsAttention
            ) | (Self::NeedsAttention, Self::RolledBack)
        )
    }
}

/// 恢复点行的状态；只服务诊断与清理，不参与应用判定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryPointStatus {
    Prepared,
    Applied,
    NeedsAttention,
}

impl RecoveryPointStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "Prepared",
            Self::Applied => "Applied",
            Self::NeedsAttention => "NeedsAttention",
        }
    }
}

/// 一次 staged 恢复请求；staged 材料在恢复器应用或回滚后由所属恢复器清理。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MaintenanceRequest {
    pub(crate) request_id: String,
    pub(crate) kind: String,
    pub(crate) status: MaintenanceRequestStatus,
    pub(crate) staged_fingerprint: [u8; 32],
    pub(crate) store_epoch: String,
    pub(crate) staged_path: String,
}

/// 恢复点表行；`recovery_point_id` 即所属请求，多对象指向同一 DPAPI 文件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveryPointRecord {
    pub(crate) recovery_point_id: String,
    pub(crate) object_key: String,
    pub(crate) status: RecoveryPointStatus,
    pub(crate) before_fingerprint: Option<[u8; 32]>,
    pub(crate) after_fingerprint: Option<[u8; 32]>,
    pub(crate) dpapi_path: String,
}

/// 维护请求存储错误；展示只给封闭类别，不回显路径、SQL 或正文。
#[derive(Debug)]
pub(crate) enum MaintenanceStoreError {
    InvalidRequest,
    InvalidRow,
    EpochConflict,
    Conflict,
    NotFound,
    Database(DatabaseError),
}

impl fmt::Display for MaintenanceStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "维护请求无效",
            Self::InvalidRow => "维护请求记录损坏",
            Self::EpochConflict => "维护请求世代不是当前工作区世代",
            Self::Conflict => "维护请求状态或身份冲突",
            Self::NotFound => "维护请求不存在",
            Self::Database(_) => "维护请求存储失败",
        })
    }
}

impl std::error::Error for MaintenanceStoreError {}

impl From<DatabaseError> for MaintenanceStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for MaintenanceStoreError {
    /// 行解码失败属于损坏事实；SQL 执行失败保留数据库分类。
    fn from(error: rusqlite::Error) -> Self {
        match error {
            rusqlite::Error::InvalidColumnType(..)
            | rusqlite::Error::FromSqlConversionFailure(..) => Self::InvalidRow,
            other => Self::Database(DatabaseError::from(other)),
        }
    }
}

impl WorkspaceDatabase {
    /// 登记 staged 恢复请求：只允许当前世代与 Prepared 初始状态，重复 id 冲突。
    pub(crate) fn register_maintenance_request(
        &mut self,
        request: &MaintenanceRequest,
    ) -> Result<(), MaintenanceStoreError> {
        if request.request_id.is_empty()
            || request.kind.is_empty()
            || request.store_epoch.is_empty()
            || request.staged_path.is_empty()
            || request.status != MaintenanceRequestStatus::Prepared
        {
            return Err(MaintenanceStoreError::InvalidRequest);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !store_epoch_is_current(&transaction, &request.store_epoch)? {
            return Err(MaintenanceStoreError::EpochConflict);
        }
        let existing: Option<String> = transaction
            .query_row(
                "SELECT request_id FROM maintenance_requests WHERE request_id = ?1",
                [&request.request_id],
                |row| row.get(0),
            )
            .optional()?;
        if existing.is_some() {
            return Err(MaintenanceStoreError::Conflict);
        }
        transaction.execute(
            "INSERT INTO maintenance_requests \
             (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &request.request_id,
                &request.kind,
                request.status.as_str(),
                request.staged_fingerprint.to_vec(),
                &request.store_epoch,
                &request.staged_path
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn maintenance_request(
        &self,
        request_id: &str,
    ) -> Result<Option<MaintenanceRequest>, MaintenanceStoreError> {
        read_request(
            &self.database.connection,
            "SELECT request_id, kind, status, staged_fingerprint, store_epoch, staged_path \
             FROM maintenance_requests WHERE request_id = ?1",
            [request_id],
        )
    }

    /// 未完成请求按 id 排序返回；未知状态值视为损坏并阻断。
    pub(crate) fn unfinished_maintenance_requests(
        &self,
    ) -> Result<Vec<MaintenanceRequest>, MaintenanceStoreError> {
        let mut statement = self.database.connection.prepare(
            "SELECT request_id, kind, status, staged_fingerprint, store_epoch, staged_path \
             FROM maintenance_requests \
             WHERE status NOT IN ('Applied', 'RolledBack') ORDER BY request_id",
        )?;
        let rows = statement.query_map([], decode_request_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(MaintenanceStoreError::from)
    }

    /// 显式状态推进：预期旧状态与合法边双重核对，失败零改写。
    pub(crate) fn transition_maintenance_request(
        &mut self,
        request_id: &str,
        expected: MaintenanceRequestStatus,
        next: MaintenanceRequestStatus,
    ) -> Result<MaintenanceRequest, MaintenanceStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_request(
            &transaction,
            "SELECT request_id, kind, status, staged_fingerprint, store_epoch, staged_path \
             FROM maintenance_requests WHERE request_id = ?1",
            [request_id],
        )?
        .ok_or(MaintenanceStoreError::NotFound)?;
        if current.status != expected || !expected.allows(next) {
            return Err(MaintenanceStoreError::Conflict);
        }
        let changed = transaction.execute(
            "UPDATE maintenance_requests SET status = ?1 \
             WHERE request_id = ?2 AND status = ?3",
            params![next.as_str(), request_id, expected.as_str()],
        )?;
        if changed != 1 {
            return Err(MaintenanceStoreError::Conflict);
        }
        transaction.commit()?;
        Ok(MaintenanceRequest {
            status: next,
            ..current
        })
    }

    /// 维护期应用成功后的唯一提交点：更换 `store_epoch` 与
    /// `Applying → Applied` 在同一 Immediate 事务内完成。
    ///
    /// 两个事实必须原子落定：先轮换世代再单独推进终态会留下
    /// 「请求仍为 Applying、当前世代已更换」的不可恢复窗口。
    pub(crate) fn complete_maintenance_apply(
        &mut self,
        request_id: &str,
        run_id: &str,
        expected_store_epoch: &str,
        replacement_store_epoch: &str,
    ) -> Result<MaintenanceRequest, MaintenanceStoreError> {
        if request_id.is_empty()
            || run_id.is_empty()
            || expected_store_epoch.is_empty()
            || replacement_store_epoch.is_empty()
            || expected_store_epoch == replacement_store_epoch
        {
            return Err(MaintenanceStoreError::InvalidRequest);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_request(
            &transaction,
            "SELECT request_id, kind, status, staged_fingerprint, store_epoch, staged_path \
             FROM maintenance_requests WHERE request_id = ?1",
            [request_id],
        )?
        .ok_or(MaintenanceStoreError::NotFound)?;
        if current.status != MaintenanceRequestStatus::Applying {
            return Err(MaintenanceStoreError::Conflict);
        }
        let (current_run, current_epoch): (String, String) = transaction
            .query_row(
                "SELECT run_id, store_epoch FROM workspace_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(MaintenanceStoreError::EpochConflict)?;
        if current_run != run_id || current_epoch != expected_store_epoch {
            return Err(MaintenanceStoreError::EpochConflict);
        }
        let rotated = transaction.execute(
            "UPDATE workspace_meta SET store_epoch = ?1 \
             WHERE singleton = 1 AND run_id = ?2 AND store_epoch = ?3",
            params![replacement_store_epoch, run_id, expected_store_epoch],
        )?;
        if rotated != 1 {
            return Err(MaintenanceStoreError::EpochConflict);
        }
        let applied = transaction.execute(
            "UPDATE maintenance_requests SET status = ?1 \
             WHERE request_id = ?2 AND status = ?3",
            params![
                MaintenanceRequestStatus::Applied.as_str(),
                request_id,
                MaintenanceRequestStatus::Applying.as_str()
            ],
        )?;
        if applied != 1 {
            return Err(MaintenanceStoreError::Conflict);
        }
        transaction.commit()?;
        Ok(MaintenanceRequest {
            status: MaintenanceRequestStatus::Applied,
            ..current
        })
    }

    /// 撤销尚未获得材料的 Prepared 请求；只允许该状态，避免删除已应用事实。
    pub(crate) fn discard_prepared_maintenance_request(
        &mut self,
        request_id: &str,
    ) -> Result<(), MaintenanceStoreError> {
        let removed = self.database.connection.execute(
            "DELETE FROM maintenance_requests WHERE request_id = ?1 AND status = ?2",
            params![request_id, MaintenanceRequestStatus::Prepared.as_str()],
        )?;
        if removed != 1 {
            return Err(MaintenanceStoreError::Conflict);
        }
        Ok(())
    }

    pub(crate) fn record_recovery_point(
        &mut self,
        record: &RecoveryPointRecord,
    ) -> Result<(), MaintenanceStoreError> {
        if record.recovery_point_id.is_empty()
            || record.object_key.is_empty()
            || record.dpapi_path.is_empty()
        {
            return Err(MaintenanceStoreError::InvalidRequest);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT object_key FROM recovery_points \
                 WHERE recovery_point_id = ?1 AND object_key = ?2",
                params![&record.recovery_point_id, &record.object_key],
                |row| row.get(0),
            )
            .optional()?;
        if existing.is_some() {
            return Err(MaintenanceStoreError::Conflict);
        }
        transaction.execute(
            "INSERT INTO recovery_points \
             (recovery_point_id, object_key, status, before_fingerprint, after_fingerprint, dpapi_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &record.recovery_point_id,
                &record.object_key,
                record.status.as_str(),
                record.before_fingerprint.map(|value| value.to_vec()),
                record.after_fingerprint.map(|value| value.to_vec()),
                &record.dpapi_path
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn recovery_points(
        &self,
        recovery_point_id: &str,
    ) -> Result<Vec<RecoveryPointRecord>, MaintenanceStoreError> {
        let mut statement = self.database.connection.prepare(
            "SELECT recovery_point_id, object_key, status, before_fingerprint, \
             after_fingerprint, dpapi_path FROM recovery_points \
             WHERE recovery_point_id = ?1 ORDER BY object_key",
        )?;
        let rows = statement.query_map([recovery_point_id], decode_recovery_point_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(MaintenanceStoreError::from)
    }

    /// 应用成功后回填该对象的恢复后指纹并标记为 Applied（可撤回材料）。
    pub(crate) fn mark_recovery_point_applied(
        &mut self,
        recovery_point_id: &str,
        object_key: &str,
        after_fingerprint: Option<[u8; 32]>,
    ) -> Result<(), MaintenanceStoreError> {
        let changed = self.database.connection.execute(
            "UPDATE recovery_points SET status = 'Applied', after_fingerprint = ?3 \
             WHERE recovery_point_id = ?1 AND object_key = ?2",
            params![
                recovery_point_id,
                object_key,
                after_fingerprint.map(|value| value.to_vec())
            ],
        )?;
        if changed != 1 {
            return Err(MaintenanceStoreError::Conflict);
        }
        Ok(())
    }

    /// 已完成恢复且尚未撤回的恢复点身份；只返回请求已 Applied 的行。
    pub(crate) fn applied_recovery_points(&self) -> Result<Vec<String>, MaintenanceStoreError> {
        let mut statement = self.database.connection.prepare(
            "SELECT p.recovery_point_id FROM recovery_points p \
             JOIN maintenance_requests r ON r.request_id = p.recovery_point_id \
             WHERE r.status = 'Applied' GROUP BY p.recovery_point_id \
             ORDER BY p.recovery_point_id",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(MaintenanceStoreError::from)
    }

    /// 是否存在不属于已终结请求的恢复点；这类遗留才是阻断事实。
    pub(crate) fn has_unresolved_recovery_points(&self) -> Result<bool, MaintenanceStoreError> {
        let exists: i64 = self.database.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM recovery_points p \
             LEFT JOIN maintenance_requests r ON r.request_id = p.recovery_point_id \
             WHERE r.request_id IS NULL \
                OR r.status NOT IN ('Applied', 'RolledBack'))",
            [],
            |row| row.get(0),
        )?;
        Ok(exists != 0)
    }

    pub(crate) fn clear_recovery_points(
        &mut self,
        recovery_point_id: &str,
    ) -> Result<(), MaintenanceStoreError> {
        self.database.connection.execute(
            "DELETE FROM recovery_points WHERE recovery_point_id = ?1",
            [recovery_point_id],
        )?;
        Ok(())
    }
}

fn read_request<P: rusqlite::Params>(
    connection: &rusqlite::Connection,
    sql: &str,
    params: P,
) -> Result<Option<MaintenanceRequest>, MaintenanceStoreError> {
    connection
        .query_row(sql, params, decode_request_row)
        .optional()
        .map_err(MaintenanceStoreError::from)
}

fn decode_request_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MaintenanceRequest> {
    let status: String = row.get(2)?;
    let fingerprint: Vec<u8> = row.get(3)?;
    Ok(MaintenanceRequest {
        request_id: row.get(0)?,
        kind: row.get(1)?,
        status: MaintenanceRequestStatus::parse(&status).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(2, status, rusqlite::types::Type::Text)
        })?,
        staged_fingerprint: fingerprint.try_into().map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                3,
                "staged_fingerprint".into(),
                rusqlite::types::Type::Blob,
            )
        })?,
        store_epoch: row.get(4)?,
        staged_path: row.get(5)?,
    })
}

fn decode_recovery_point_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecoveryPointRecord> {
    let status: String = row.get(2)?;
    Ok(RecoveryPointRecord {
        recovery_point_id: row.get(0)?,
        object_key: row.get(1)?,
        status: match status.as_str() {
            "Prepared" => RecoveryPointStatus::Prepared,
            "Applied" => RecoveryPointStatus::Applied,
            "NeedsAttention" => RecoveryPointStatus::NeedsAttention,
            _ => {
                return Err(rusqlite::Error::InvalidColumnType(
                    2,
                    status,
                    rusqlite::types::Type::Text,
                ));
            }
        },
        before_fingerprint: decode_fingerprint(row.get(3)?)?,
        after_fingerprint: decode_fingerprint(row.get(4)?)?,
        dpapi_path: row.get(5)?,
    })
}

fn decode_fingerprint(value: Option<Vec<u8>>) -> rusqlite::Result<Option<[u8; 32]>> {
    value
        .map(|bytes| {
            bytes.try_into().map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    3,
                    "fingerprint".into(),
                    rusqlite::types::Type::Blob,
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod maintenance_request_tests {
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    use super::{
        MaintenanceRequest, MaintenanceRequestStatus, MaintenanceStoreError, RecoveryPointRecord,
        RecoveryPointStatus,
    };
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::workspace::WorkspaceDatabase;

    const EPOCH: &str = "epoch-maintenance";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn begin_run(store: &mut WorkspaceDatabase) {
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: "run-request".into(),
                    process_id: 7,
                    process_created_at_ms: 7,
                },
            )
            .unwrap();
    }

    fn request(request_id: &str, status: MaintenanceRequestStatus) -> MaintenanceRequest {
        MaintenanceRequest {
            request_id: request_id.into(),
            kind: "backup.restore".into(),
            status,
            staged_fingerprint: [5; 32],
            store_epoch: EPOCH.into(),
            staged_path: "maintenance/staged.restore".into(),
        }
    }

    fn recovery_point(object_key: &str) -> RecoveryPointRecord {
        RecoveryPointRecord {
            recovery_point_id: "request-a".into(),
            object_key: object_key.into(),
            status: RecoveryPointStatus::Prepared,
            before_fingerprint: Some([1; 32]),
            after_fingerprint: None,
            dpapi_path: "backup-recovery/request-a.slterm-recovery".into(),
        }
    }

    #[test]
    fn maintenance_request_round_trips_and_requires_the_current_epoch() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();

        // 无运行元数据时登记必须失败，且不留下半行。
        assert!(matches!(
            store.register_maintenance_request(&request(
                "request-a",
                MaintenanceRequestStatus::Prepared
            )),
            Err(MaintenanceStoreError::EpochConflict)
        ));
        begin_run(&mut store);
        let prepared = request("request-a", MaintenanceRequestStatus::Prepared);
        store.register_maintenance_request(&prepared).unwrap();
        assert_eq!(
            store.maintenance_request("request-a").unwrap(),
            Some(prepared.clone())
        );

        // 旧世代登记同样拒绝；错误分类不与「当前世代」混淆。
        let mut stale = request("request-stale", MaintenanceRequestStatus::Prepared);
        stale.store_epoch = "epoch-old".into();
        assert!(matches!(
            store.register_maintenance_request(&stale),
            Err(MaintenanceStoreError::EpochConflict)
        ));
        assert_eq!(store.maintenance_request("request-stale").unwrap(), None);

        // 同 id 重复登记冲突，已有事实不被改写。
        let duplicate = request("request-a", MaintenanceRequestStatus::Prepared);
        assert!(matches!(
            store.register_maintenance_request(&duplicate),
            Err(MaintenanceStoreError::Conflict)
        ));
        assert_eq!(
            store.maintenance_request("request-a").unwrap(),
            Some(prepared)
        );
    }

    #[test]
    fn maintenance_request_transitions_follow_explicit_edges_and_freeze_terminal_states() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&request("request-a", MaintenanceRequestStatus::Prepared))
            .unwrap();

        // 未经过 Applying 不得直接 Applied。
        assert!(matches!(
            store.transition_maintenance_request(
                "request-a",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applied
            ),
            Err(MaintenanceStoreError::Conflict)
        ));
        let applying = store
            .transition_maintenance_request(
                "request-a",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
        assert_eq!(applying.status, MaintenanceRequestStatus::Applying);
        let applied = store
            .transition_maintenance_request(
                "request-a",
                MaintenanceRequestStatus::Applying,
                MaintenanceRequestStatus::Applied,
            )
            .unwrap();
        assert_eq!(applied.status, MaintenanceRequestStatus::Applied);

        // 终态冻结；预期旧状态不符同样冲突且不改写。
        for next in [
            MaintenanceRequestStatus::Applying,
            MaintenanceRequestStatus::RolledBack,
            MaintenanceRequestStatus::NeedsAttention,
        ] {
            assert!(matches!(
                store.transition_maintenance_request(
                    "request-a",
                    MaintenanceRequestStatus::Applied,
                    next
                ),
                Err(MaintenanceStoreError::Conflict)
            ));
        }
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );

        // 人工确认后的清理路径：NeedsAttention 允许显式转 RolledBack。
        store
            .register_maintenance_request(&request("request-b", MaintenanceRequestStatus::Prepared))
            .unwrap();
        store
            .transition_maintenance_request(
                "request-b",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::NeedsAttention,
            )
            .unwrap();
        store
            .transition_maintenance_request(
                "request-b",
                MaintenanceRequestStatus::NeedsAttention,
                MaintenanceRequestStatus::RolledBack,
            )
            .unwrap();
        assert!(matches!(
            store.transition_maintenance_request(
                "missing",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying
            ),
            Err(MaintenanceStoreError::NotFound)
        ));
    }

    #[test]
    fn unfinished_maintenance_requests_exclude_terminal_states() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        for id in ["request-a", "request-b", "request-c"] {
            store
                .register_maintenance_request(&request(id, MaintenanceRequestStatus::Prepared))
                .unwrap();
        }
        store
            .transition_maintenance_request(
                "request-a",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
        store
            .transition_maintenance_request(
                "request-a",
                MaintenanceRequestStatus::Applying,
                MaintenanceRequestStatus::Applied,
            )
            .unwrap();
        store
            .transition_maintenance_request(
                "request-b",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
        store
            .transition_maintenance_request(
                "request-c",
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::RolledBack,
            )
            .unwrap();

        let unfinished = store.unfinished_maintenance_requests().unwrap();
        assert_eq!(
            unfinished
                .iter()
                .map(|request| request.request_id.as_str())
                .collect::<Vec<_>>(),
            ["request-b"]
        );
    }

    #[test]
    fn recovery_points_are_scoped_to_their_request_and_cleared_explicitly() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .record_recovery_point(&recovery_point("themes/b.json"))
            .unwrap();
        store
            .record_recovery_point(&recovery_point("settings.json"))
            .unwrap();
        let mut other = recovery_point("session.json");
        other.recovery_point_id = "request-b".into();
        store.record_recovery_point(&other).unwrap();

        let points = store.recovery_points("request-a").unwrap();
        assert_eq!(
            points
                .iter()
                .map(|point| point.object_key.as_str())
                .collect::<Vec<_>>(),
            ["settings.json", "themes/b.json"]
        );
        store.clear_recovery_points("request-a").unwrap();
        assert!(store.recovery_points("request-a").unwrap().is_empty());
        assert_eq!(store.recovery_points("request-b").unwrap(), vec![other]);
    }

    #[test]
    fn corrupt_request_rows_block_instead_of_guessing() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .database
            .connection
            .execute(
                "INSERT INTO maintenance_requests \
                 (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) \
                 VALUES ('request-corrupt', 'backup.restore', 'Unknown', x'01', ?1, 'staged')",
                [EPOCH],
            )
            .unwrap();
        assert!(matches!(
            store.maintenance_request("request-corrupt"),
            Err(MaintenanceStoreError::InvalidRow)
        ));
        assert!(matches!(
            store.unfinished_maintenance_requests(),
            Err(MaintenanceStoreError::InvalidRow)
        ));
    }
}
