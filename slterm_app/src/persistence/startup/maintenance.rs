//! 启动维护期的固定顺序对账与安全闸门。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::persistence::close::CloseStoreError;
use crate::persistence::database::DatabaseError;
use crate::persistence::journal::{JournalError, JournalStatus, ManagedJournal};
use crate::persistence::maintenance_request::{
    MaintenanceRequest, MaintenanceRequestStatus, MaintenanceStoreError,
};
use crate::persistence::metadata::MetadataError;
use crate::persistence::operation::OperationError;
use crate::persistence::workspace::WorkspaceDatabase;

/// 启动维护阶段；顺序即对账顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaintenanceStage {
    Journal,
    MaintenanceRequests,
    CloseIntents,
}

/// 全部事实安全才放行；任一阶段需人工处理即停止。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaintenanceOutcome {
    Ready,
    NeedsAttention { stage: MaintenanceStage },
}

/// 启动维护错误保留来源分类，展示不携带持久事实细节。
#[derive(Debug)]
pub(crate) enum MaintenanceError {
    Journal(JournalError),
    Close(CloseStoreError),
    Database(DatabaseError),
    Metadata(MetadataError),
    Store(MaintenanceStoreError),
    Request(MaintenanceRequestError),
    Operation(OperationError),
    /// 无主 staged 目录清理失败；保留现场并阻断启动。
    Staged(io::Error),
}

impl fmt::Display for MaintenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Journal(_) => "启动 Journal 对账失败",
            Self::Close(_) => "启动关闭意图检查失败",
            Self::Database(_) => "启动维护数据库检查失败",
            Self::Metadata(_) => "启动运行世代检查失败",
            Self::Store(_) => "启动维护请求对账失败",
            Self::Request(_) => "启动维护请求恢复失败",
            Self::Operation(_) => "启动 Operation 对账失败",
            Self::Staged(_) => "启动维护暂存清理失败",
        })
    }
}

impl std::error::Error for MaintenanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Journal(error) => Some(error),
            Self::Close(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Request(error) => Some(error),
            Self::Operation(error) => Some(error),
            Self::Staged(error) => Some(error),
        }
    }
}

impl From<JournalError> for MaintenanceError {
    fn from(error: JournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<CloseStoreError> for MaintenanceError {
    fn from(error: CloseStoreError) -> Self {
        Self::Close(error)
    }
}

impl From<DatabaseError> for MaintenanceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<MetadataError> for MaintenanceError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<OperationError> for MaintenanceError {
    fn from(error: OperationError) -> Self {
        Self::Operation(error)
    }
}

impl From<MaintenanceStoreError> for MaintenanceError {
    fn from(error: MaintenanceStoreError) -> Self {
        Self::Store(error)
    }
}

/// 维护请求恢复器的所有权错误；细节只供诊断，展示不插值。
#[derive(Debug)]
pub(crate) struct MaintenanceRequestError {
    detail: String,
}

impl MaintenanceRequestError {
    pub(crate) fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    /// 脱敏诊断文本；绝不包含路径、正文或凭据。
    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for MaintenanceRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("维护请求恢复失败")
    }
}

impl std::error::Error for MaintenanceRequestError {}

/// 所属 kind 的恢复器独占实际转换；协调器不解释 Document 或 Agent 材料。
pub(crate) trait JournalRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        journal: &ManagedJournal,
    ) -> Result<(), JournalError>;
}

impl<F> JournalRecoverer for F
where
    F: Fn(&mut WorkspaceDatabase, &ManagedJournal) -> Result<(), JournalError>,
{
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        journal: &ManagedJournal,
    ) -> Result<(), JournalError> {
        self(store, journal)
    }
}

pub(crate) type JournalRecovererRegistry<'a> = BTreeMap<&'a str, &'a dyn JournalRecoverer>;

/// 历史隔离对账端口：在维护期读取清单并修正状态，但绝不升级为启动阻断。
pub(crate) trait HistoryQuarantineMaintainer {
    fn maintain(&self, store: &mut WorkspaceDatabase, now: SystemTime);
}

/// 维护请求 kind 的恢复器独占 staged 材料的应用或回滚；协调器只推动状态。
pub(crate) trait MaintenanceRequestRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), MaintenanceRequestError>;
}

impl<F> MaintenanceRequestRecoverer for F
where
    F: Fn(&mut WorkspaceDatabase, &MaintenanceRequest) -> Result<(), MaintenanceRequestError>,
{
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), MaintenanceRequestError> {
        self(store, request)
    }
}

pub(crate) type MaintenanceRequestRecovererRegistry<'a> =
    BTreeMap<&'a str, &'a dyn MaintenanceRequestRecoverer>;

/// 原始存在性查询只允许使用这里的封闭表名与固定 SQL。
#[derive(Clone, Copy)]
enum MaintenanceTable {
    RecoveryPoints,
}

impl MaintenanceTable {
    const fn exists_query(self) -> &'static str {
        match self {
            Self::RecoveryPoints => "SELECT EXISTS(SELECT 1 FROM recovery_points)",
        }
    }
}

/// staged 根目录名；启动清理只处理其下 UUID 命名的请求目录。
const STAGED_ROOT: &str = "maintenance";

fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

/// 清理没有维护请求行的 staged 目录。
///
/// 只接受 `local_root/maintenance/<uuid>/` 形态：根与候选都必须是普通目录、
/// 候选父目录恰为维护根；有请求行的目录（含 Prepared／Applying／已终结）
/// 一律保留。清理失败保留现场并上抛，不继续建立运行写线程。
pub(crate) fn cleanup_orphan_staged_material(
    local_root: &Path,
    request_ids: &BTreeSet<String>,
) -> Result<usize, io::Error> {
    let root = local_root.join(STAGED_ROOT);
    let root_metadata = match std::fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if !root_metadata.is_dir() || is_reparse(&root_metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "维护暂存根不是普通目录",
        ));
    }
    let mut removed = 0usize;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if uuid::Uuid::parse_str(name).is_err() || request_ids.contains(name) {
            continue;
        }
        let candidate = entry.path();
        if candidate.parent() != Some(root.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "维护暂存路径越界",
            ));
        }
        let metadata = std::fs::symlink_metadata(&candidate)?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "维护暂存目录不是普通目录",
            ));
        }
        std::fs::remove_dir_all(&candidate)?;
        removed += 1;
    }
    Ok(removed)
}

/// 单一启动维护入口；数据库打开与高版本／损坏分流仍归外层。
pub(crate) struct MaintenanceRunner<'a> {
    store: &'a mut WorkspaceDatabase,
    store_epoch: String,
    recoverers: &'a JournalRecovererRegistry<'a>,
    request_recoverers: &'a MaintenanceRequestRecovererRegistry<'a>,
    quarantine_maintainer: Option<&'a dyn HistoryQuarantineMaintainer>,
    local_root: Option<&'a Path>,
}

impl<'a> MaintenanceRunner<'a> {
    pub(crate) fn new(
        store: &'a mut WorkspaceDatabase,
        store_epoch: &str,
        recoverers: &'a JournalRecovererRegistry<'a>,
        request_recoverers: &'a MaintenanceRequestRecovererRegistry<'a>,
    ) -> Self {
        Self {
            store,
            store_epoch: store_epoch.to_owned(),
            recoverers,
            request_recoverers,
            quarantine_maintainer: None,
            local_root: None,
        }
    }

    /// 注入本地数据根；维护请求终结后清理无主 staged 目录。
    pub(crate) fn with_local_root(mut self, local_root: &'a Path) -> Self {
        self.local_root = Some(local_root);
        self
    }

    /// 注入历史隔离对账端口；隔离材料不是启动阻断，失败只保留待处理事实。
    pub(crate) fn with_history_quarantine(
        mut self,
        maintainer: &'a dyn HistoryQuarantineMaintainer,
    ) -> Self {
        self.quarantine_maintainer = Some(maintainer);
        self
    }

    pub(crate) fn run(&mut self) -> Result<MaintenanceOutcome, MaintenanceError> {
        let journal = self.reconcile_journals()?;
        if journal != MaintenanceOutcome::Ready {
            return Ok(journal);
        }
        let requests = self.reconcile_maintenance_requests()?;
        if requests != MaintenanceOutcome::Ready {
            return Ok(requests);
        }
        if let Some(local_root) = self.local_root {
            let request_ids = self.store.maintenance_request_ids()?;
            cleanup_orphan_staged_material(local_root, &request_ids)
                .map_err(MaintenanceError::Staged)?;
        }
        // Journal 与维护请求终结后，剩余中间态 Operation 只能收敛为可查询终态。
        self.store.reconcile_interrupted_operations()?;
        if !self.store.unfinished_close_intents()?.is_empty() {
            return Ok(MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::CloseIntents,
            });
        }
        // 隔离材料先于历史展示对账；结果写入批次逐项事实，不改变启动结论。
        if let Some(maintainer) = self.quarantine_maintainer {
            maintainer.maintain(self.store, SystemTime::now());
        }
        Ok(MaintenanceOutcome::Ready)
    }

    fn any_rows(&mut self, tables: &[MaintenanceTable]) -> Result<bool, MaintenanceError> {
        for table in tables {
            let exists = self
                .store
                .database
                .connection
                .query_row(table.exists_query(), [], |row| row.get::<_, bool>(0))
                .map_err(DatabaseError::from)?;
            if exists {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 维护请求应用或回滚终结后才放行；恢复点表遗留视为未清理材料。
    fn reconcile_maintenance_requests(&mut self) -> Result<MaintenanceOutcome, MaintenanceError> {
        for request in self.store.unfinished_maintenance_requests()? {
            // 恢复器可轮换世代；每条请求前重读当前世代，旧世代立即阻断。
            self.refresh_epoch()?;
            if request.store_epoch != self.store_epoch {
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::MaintenanceRequests,
                });
            }
            let Some(recoverer) = self.request_recoverers.get(request.kind.as_str()) else {
                if request.status != MaintenanceRequestStatus::NeedsAttention {
                    let _ = self.store.transition_maintenance_request(
                        &request.request_id,
                        request.status,
                        MaintenanceRequestStatus::NeedsAttention,
                    )?;
                }
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::MaintenanceRequests,
                });
            };
            recoverer
                .recover(self.store, &request)
                .map_err(MaintenanceError::Request)?;
            // Ok 只代表恢复器返回；必须重读持久事实，拒绝假成功。
            let recovered = self
                .store
                .maintenance_request(&request.request_id)?
                .ok_or(MaintenanceStoreError::InvalidRow)?;
            if !recovered.status.is_terminal() {
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::MaintenanceRequests,
                });
            }
        }
        // 已应用请求的恢复点是可撤回材料，不阻断启动；其余遗留仍阻断。
        if self.store.has_unresolved_recovery_points()? {
            return Ok(MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests,
            });
        }
        Ok(MaintenanceOutcome::Ready)
    }

    fn refresh_epoch(&mut self) -> Result<(), MaintenanceError> {
        let metadata = self
            .store
            .metadata()?
            .ok_or(MetadataError::InvalidMetadata)?;
        if metadata.store_epoch.is_empty() {
            return Err(MetadataError::InvalidMetadata.into());
        }
        self.store_epoch = metadata.store_epoch;
        Ok(())
    }

    fn reconcile_journals(&mut self) -> Result<MaintenanceOutcome, MaintenanceError> {
        for journal in self.store.unfinished_journals()? {
            if journal.store_epoch != self.store_epoch {
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::Journal,
                });
            }
            let Some(recoverer) = self.recoverers.get(journal.kind.as_str()) else {
                if journal.status != JournalStatus::NeedsAttention {
                    let _ = self.store.transition_journal(
                        &journal.journal_id,
                        JournalStatus::NeedsAttention,
                        None,
                    )?;
                }
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::Journal,
                });
            };
            recoverer.recover(self.store, &journal)?;
            // Ok 只代表恢复器返回；必须重读持久事实，拒绝假成功。
            let recovered = self
                .store
                .journal(&journal.journal_id)?
                .ok_or(JournalError::CorruptRecord)?;
            if !recovered.status.is_terminal() {
                return Ok(MaintenanceOutcome::NeedsAttention {
                    stage: MaintenanceStage::Journal,
                });
            }
        }
        Ok(MaintenanceOutcome::Ready)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::types::Value;
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_workspace::{CloseIntent, CloseScope};
    use tempfile::TempDir;

    use super::{
        JournalRecovererRegistry, MaintenanceError, MaintenanceOutcome, MaintenanceRequest,
        MaintenanceRequestError, MaintenanceRequestRecovererRegistry, MaintenanceRunner,
        MaintenanceStage,
    };
    use crate::persistence::close::{CloseCommitStatus, CloseStoreError, PersistedCloseIntent};
    use crate::persistence::database::DatabaseError;
    use crate::persistence::journal::{
        JournalError, JournalRegistration, JournalStatus, JournalStepRegistration, ManagedJournal,
    };
    use crate::persistence::maintenance_request::{
        MaintenanceRequestStatus, MaintenanceStoreError,
    };
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::operation::OperationRegistration;
    use crate::persistence::workspace::WorkspaceDatabase;
    use slterm_runtime_protocol::OperationStatus;

    const EPOCH: &str = "epoch-startup";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn begin_run(store: &mut WorkspaceDatabase) {
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: "run-startup".into(),
                    process_id: 1,
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
    }

    fn registration(journal_id: &str, kind: &str, store_epoch: &str) -> JournalRegistration {
        JournalRegistration {
            operation_id: None,
            journal_id: journal_id.into(),
            kind: kind.into(),
            store_epoch: store_epoch.into(),
            before_fingerprint: Some([1; 32]),
            steps: vec![JournalStepRegistration {
                step_key: "step-a".into(),
                kind: "replace".into(),
                target: json!({"version": 1}),
                authorization_generation: 1,
                before_fingerprint: Some([2; 32]),
            }],
        }
    }

    fn run(
        store: &mut WorkspaceDatabase,
        recoverers: &JournalRecovererRegistry<'_>,
    ) -> MaintenanceOutcome {
        let requests = MaintenanceRequestRecovererRegistry::default();
        MaintenanceRunner::new(store, EPOCH, recoverers, &requests)
            .run()
            .unwrap()
    }

    fn close_registration() -> PersistedCloseIntent {
        PersistedCloseIntent {
            close_id: "close-startup".into(),
            scope: CloseScope::ApplicationExit,
            store_epoch: EPOCH.into(),
            intent: CloseIntent::new(Vec::new(), [4; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        }
    }

    fn complete_close(store: &mut WorkspaceDatabase, close: &PersistedCloseIntent) {
        let mut current = close.clone();
        for next in [
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            CloseCommitStatus::Completed,
        ] {
            current = store
                .advance_close_intent(
                    &current.close_id,
                    current.commit_status,
                    next,
                    current.intent.fingerprint(),
                    EPOCH,
                )
                .unwrap();
        }
    }

    fn material_rows(store: &WorkspaceDatabase) -> Vec<Vec<Vec<Value>>> {
        const TABLES: [&str; 2] = ["maintenance_requests", "recovery_points"];
        TABLES
            .into_iter()
            .map(|table| {
                let mut statement = store
                    .database
                    .connection
                    .prepare(&format!("SELECT * FROM {table}"))
                    .unwrap();
                let columns = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..columns)
                            .map(|index| row.get::<_, Value>(index))
                            .collect()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            })
            .collect()
    }

    struct CountingQuarantineMaintainer(std::cell::Cell<usize>);

    impl super::HistoryQuarantineMaintainer for CountingQuarantineMaintainer {
        fn maintain(&self, _store: &mut WorkspaceDatabase, _now: std::time::SystemTime) {
            self.0.set(self.0.get() + 1);
        }
    }

    /// 隔离对账是维护期的非阻断步骤：失败也保持 Ready，且不跳过后续启动。
    #[test]
    fn history_quarantine_maintenance_never_blocks_startup() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        let maintainer = CountingQuarantineMaintainer(std::cell::Cell::new(0));
        let recoverers = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::default();
        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests)
            .with_history_quarantine(&maintainer)
            .run()
            .unwrap();
        assert_eq!(outcome, MaintenanceOutcome::Ready);
        assert_eq!(maintainer.0.get(), 1);
    }

    /// 隔离材料存在时，生产恢复器只修正记录，不把启动升格为需要人工处理。
    #[test]
    fn history_quarantine_rows_do_not_become_a_startup_blocker() {
        use crate::history_quarantine::persist::{
            BatchStage, QuarantineBatchRecord, QuarantineItemRecord, QuarantineItemState,
        };
        use crate::history_quarantine::{BatchStatus, QuarantineBatchId};

        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        let record = QuarantineBatchRecord {
            id: QuarantineBatchId(
                uuid::Uuid::parse_str("019bfd70-80aa-7000-8000-000000000002").unwrap(),
            ),
            status: BatchStatus::Completed,
            stage: BatchStage::Committed,
            created_at: std::time::UNIX_EPOCH,
            completed_at: Some(std::time::UNIX_EPOCH),
            manifest_name: "missing.manifest.json".into(),
            items: vec![QuarantineItemRecord {
                order: 0,
                state: QuarantineItemState::SourceRemoved,
                failure: None,
            }],
        };
        store.upsert_quarantine_batch(&record).unwrap();
        let maintainer = crate::persistence::startup::FileHistoryQuarantineMaintainer::new(&paths);
        let recoverers = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::default();

        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests)
            .with_history_quarantine(&maintainer)
            .run()
            .unwrap();

        assert_eq!(outcome, MaintenanceOutcome::Ready);
        let reconciled = store.quarantine_records().unwrap();
        assert_eq!(reconciled[0].status, BatchStatus::NeedsAttention);
        assert_eq!(
            reconciled[0].items[0].failure,
            Some(crate::history_quarantine::QuarantineFailurePhase::ReconcileMissingManifest)
        );
    }

    /// 恢复器在 Applying 与 Applied 之间轮换世代后中断：旧世代请求保留待处理。
    #[test]
    fn epoch_rotation_before_applied_blocks_and_preserves_facts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "test.rotate", EPOCH))
            .unwrap();
        let recoverer = |store: &mut WorkspaceDatabase, request: &MaintenanceRequest| {
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Prepared,
                    MaintenanceRequestStatus::Applying,
                )
                .unwrap();
            store
                .rotate_store_epoch("run-startup", EPOCH, "epoch-next")
                .unwrap();
            // 模拟 Applied 提交前进程中断：恢复器返回 Ok 但没有终态。
            Ok(())
        };
        let mut requests = MaintenanceRequestRecovererRegistry::default();
        requests.insert("test.rotate", &recoverer);
        let journals = JournalRecovererRegistry::default();

        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &journals, &requests)
            .run()
            .unwrap();

        assert_eq!(
            outcome,
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
        let request = store.maintenance_request("request-a").unwrap().unwrap();
        assert_eq!(request.status, MaintenanceRequestStatus::Applying);
        assert_eq!(request.store_epoch, EPOCH);
        assert_eq!(store.metadata().unwrap().unwrap().store_epoch, "epoch-next");

        // 重开同一库仍保留旧世代请求事实，不自动升级或删除。
        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applying
        );
    }

    /// 无主 staged 目录按 UUID 形状清理；有请求行、非 UUID 与外部链接不得删除。
    #[test]
    fn orphan_staged_directories_are_cleaned_within_the_maintenance_root() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let root = local.join("maintenance");
        let orphan = root.join("019bfd70-80aa-7000-8000-000000000101");
        let owned = root.join("019bfd70-80aa-7000-8000-000000000102");
        let foreign = root.join("not-a-request-id");
        for directory in [&orphan, &owned, &foreign] {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join("staged.restore"), b"material").unwrap();
        }
        let request_ids = std::collections::BTreeSet::from([owned
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()]);

        let removed = super::cleanup_orphan_staged_material(&local, &request_ids).unwrap();

        assert_eq!(removed, 1);
        assert!(!orphan.exists(), "无请求行的 staged 目录必须清理");
        assert!(owned.exists(), "有请求行的目录不得删除");
        assert!(foreign.exists(), "非 UUID 目录不属于 staged 布局");
        assert!(root.exists(), "维护根本身不得删除");
    }

    /// UUID 命名的 junction 指向根外时必须拒绝清理并保留目标。
    #[test]
    fn orphan_staged_cleanup_rejects_reparse_directories() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let root = local.join("maintenance");
        std::fs::create_dir_all(&root).unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep.txt"), b"keep").unwrap();
        let junction = root.join("019bfd70-80aa-7000-8000-000000000103");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .unwrap();
        if !status.success() {
            eprintln!("SKIP: 当前环境无法创建 junction");
            return;
        }

        let result =
            super::cleanup_orphan_staged_material(&local, &std::collections::BTreeSet::new());

        assert!(result.is_err(), "reparse 目录必须拒绝清理");
        assert_eq!(std::fs::read(outside.join("keep.txt")).unwrap(), b"keep");
    }

    /// 启动维护在请求终结后清理无主 staged 目录，有行目录保持原样。
    #[test]
    fn maintenance_cleans_orphan_staged_directories_after_requests_settle() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        let root = paths.local_root().join("maintenance");
        let orphan = root.join("019bfd70-80aa-7000-8000-000000000104");
        let owned = root.join("019bfd70-80aa-7000-8000-000000000105");
        for directory in [&orphan, &owned] {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join("staged.restore"), b"material").unwrap();
        }
        let owned_id = owned.file_name().unwrap().to_string_lossy().into_owned();
        store
            .database
            .connection
            .execute(
                "INSERT INTO maintenance_requests \
                 (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) \
                 VALUES (?1, 'backup.restore', 'Applied', zeroblob(32), ?2, ?3)",
                rusqlite::params![
                    owned_id.as_str(),
                    EPOCH,
                    format!("maintenance/{}/staged.restore", owned_id)
                ],
            )
            .unwrap();
        let recoverers = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::default();

        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests)
            .with_local_root(paths.local_root())
            .run()
            .unwrap();

        assert_eq!(outcome, MaintenanceOutcome::Ready);
        assert!(!orphan.exists(), "无主 staged 目录必须在启动维护期清理");
        assert!(owned.exists(), "有请求行的 staged 目录不得删除");
    }

    #[test]
    fn empty_database_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let recoverers = JournalRecovererRegistry::default();

        let requests = MaintenanceRequestRecovererRegistry::default();
        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests)
            .run()
            .unwrap();

        assert_eq!(outcome, MaintenanceOutcome::Ready);
    }

    /// 启动维护把崩溃遗留的 Operation 中间态收敛为可查询终态。
    #[test]
    fn maintenance_reconciles_interrupted_operations() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        for (operation_id, mutation_id) in [
            ("operation-queued", "mutation-q"),
            ("operation-running", "mutation-r"),
        ] {
            store
                .register_operation(OperationRegistration {
                    operation_id: operation_id.into(),
                    store_epoch: EPOCH.into(),
                    client_id: "client-startup".into(),
                    mutation_id: mutation_id.into(),
                    method: "panel.save".into(),
                    params_dto_version: 1,
                    params_hash: [7; 32],
                    resource_generations: Default::default(),
                })
                .unwrap();
        }
        store
            .transition_operation("operation-running", OperationStatus::Running, None, None)
            .unwrap();

        let recoverers = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::default();
        let outcome = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests)
            .run()
            .unwrap();

        assert_eq!(outcome, MaintenanceOutcome::Ready);
        assert_eq!(
            store.operation("operation-queued").unwrap().unwrap().status,
            OperationStatus::Failed
        );
        assert_eq!(
            store
                .operation("operation-running")
                .unwrap()
                .unwrap()
                .status,
            OperationStatus::OutcomeUnknown
        );
    }

    #[test]
    fn unknown_current_journal_becomes_needs_attention_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        let _ = store
            .register_journal(registration("journal-current", "unknown.current", EPOCH))
            .unwrap();
        let recoverers = JournalRecovererRegistry::default();

        assert_eq!(
            run(&mut store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::Journal
            }
        );
        assert_eq!(
            store.journal("journal-current").unwrap().unwrap().status,
            JournalStatus::NeedsAttention
        );
        let changes = store.database.connection.total_changes();
        assert_eq!(
            run(&mut store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::Journal
            }
        );
        assert_eq!(store.database.connection.total_changes(), changes);

        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            store.journal("journal-current").unwrap().unwrap().status,
            JournalStatus::NeedsAttention
        );
    }

    #[test]
    fn old_epoch_journal_is_preserved_without_recovery_or_rewrite() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        let original = store
            .register_journal(registration("journal-old", "unknown.old", EPOCH))
            .unwrap();
        store
            .rotate_store_epoch("run-startup", EPOCH, "epoch-next")
            .unwrap();
        let recoverers = JournalRecovererRegistry::default();
        let changes = store.database.connection.total_changes();

        let requests = MaintenanceRequestRecovererRegistry::default();
        let outcome = MaintenanceRunner::new(&mut store, "epoch-next", &recoverers, &requests)
            .run()
            .unwrap();

        assert_eq!(
            outcome,
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::Journal
            }
        );
        assert_eq!(store.journal("journal-old").unwrap(), Some(original));
        assert_eq!(store.database.connection.total_changes(), changes);
    }

    #[test]
    fn registered_recoverer_commits_after_reopen_then_reopened_store_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        let _ = store
            .register_journal(registration("journal-recovery", "test.commit", EPOCH))
            .unwrap();
        let _ = store
            .transition_journal("journal-recovery", JournalStatus::Applying, None)
            .unwrap();
        let _ = store
            .transition_journal("journal-recovery", JournalStatus::Committing, None)
            .unwrap();

        // 模拟提交窗口崩溃：只保留已落盘的 Committing 事实，重开后由注册恢复器完成。
        drop(store);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let recoverer = |store: &mut WorkspaceDatabase, journal: &ManagedJournal| {
            let _ = store.transition_journal(
                &journal.journal_id,
                JournalStatus::Committed,
                Some([3; 32]),
            )?;
            Ok(())
        };
        let mut recoverers = JournalRecovererRegistry::default();
        recoverers.insert("test.commit", &recoverer);
        assert_eq!(run(&mut store, &recoverers), MaintenanceOutcome::Ready);
        assert_eq!(
            store.journal("journal-recovery").unwrap().unwrap().status,
            JournalStatus::Committed
        );

        drop(store);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let empty = JournalRecovererRegistry::default();
        assert_eq!(run(&mut store, &empty), MaintenanceOutcome::Ready);
    }

    #[test]
    fn recoverer_ok_without_terminal_transition_is_not_ready() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        let _ = store
            .register_journal(registration("journal-fake", "test.fake", EPOCH))
            .unwrap();
        store
            .database
            .connection
            .execute("DROP TABLE maintenance_requests", [])
            .unwrap();
        let recoverer = |_: &mut WorkspaceDatabase, _: &ManagedJournal| Ok(());
        let mut recoverers = JournalRecovererRegistry::default();
        recoverers.insert("test.fake", &recoverer);

        assert_eq!(
            run(&mut store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::Journal
            }
        );
        assert_eq!(
            store.journal("journal-fake").unwrap().unwrap().status,
            JournalStatus::Prepared
        );
    }

    #[test]
    fn recoverer_error_is_propagated_without_continuing() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        let _ = store
            .register_journal(registration("journal-error", "test.error", EPOCH))
            .unwrap();
        store
            .database
            .connection
            .execute("DROP TABLE maintenance_requests", [])
            .unwrap();
        let recoverer =
            |_: &mut WorkspaceDatabase, _: &ManagedJournal| Err(JournalError::CorruptRecord);
        let mut recoverers = JournalRecovererRegistry::default();
        recoverers.insert("test.error", &recoverer);

        let requests = MaintenanceRequestRecovererRegistry::default();
        let result = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests).run();

        assert!(matches!(
            result,
            Err(MaintenanceError::Journal(JournalError::CorruptRecord))
        ));
        assert_eq!(
            store.journal("journal-error").unwrap().unwrap().status,
            JournalStatus::Prepared
        );
    }

    #[test]
    fn unfinished_close_intent_blocks_without_mutation_and_completed_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        let close = close_registration();
        store.register_close_intent(&close).unwrap();
        let recoverers = JournalRecovererRegistry::default();

        assert_eq!(
            run(&mut store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::CloseIntents
            }
        );
        assert_eq!(
            store.load_close_intent("close-startup").unwrap(),
            Some(close.clone())
        );

        complete_close(&mut store, &close);
        assert_eq!(run(&mut store, &recoverers), MaintenanceOutcome::Ready);
        assert_eq!(
            store
                .load_close_intent("close-startup")
                .unwrap()
                .unwrap()
                .commit_status,
            CloseCommitStatus::Completed
        );
    }

    #[test]
    fn every_material_table_blocks_without_modifying_facts() {
        let cases = [
            (
                "maintenance_requests",
                MaintenanceStage::MaintenanceRequests,
                "INSERT INTO maintenance_requests \
                 (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) \
                 VALUES ('request-a', 'restore', 'NeedsAttention', \
                 x'0101010101010101010101010101010101010101010101010101010101010101', \
                 'epoch-startup', 'staged')",
            ),
            (
                "recovery_points",
                MaintenanceStage::MaintenanceRequests,
                "INSERT INTO recovery_points \
                 (recovery_point_id, object_key, status, dpapi_path) \
                 VALUES ('recovery-a', 'document:a', 'Prepared', 'dpapi')",
            ),
        ];
        for (name, stage, insert) in cases {
            let temp = tempfile::tempdir().unwrap();
            let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
            begin_run(&mut store);
            store.database.connection.execute_batch(insert).unwrap();
            let before = material_rows(&store);
            let recoverers = JournalRecovererRegistry::default();

            assert_eq!(
                run(&mut store, &recoverers),
                MaintenanceOutcome::NeedsAttention { stage },
                "{name} 必须阻止启动"
            );
            assert_eq!(material_rows(&store), before, "{name} 不得被修改");
        }
    }

    fn staged_request(request_id: &str, kind: &str, epoch: &str) -> MaintenanceRequest {
        MaintenanceRequest {
            request_id: request_id.into(),
            kind: kind.into(),
            status: MaintenanceRequestStatus::Prepared,
            staged_fingerprint: [7; 32],
            store_epoch: epoch.into(),
            staged_path: "maintenance/staged.restore".into(),
        }
    }

    fn run_requests(
        store: &mut WorkspaceDatabase,
        recoverers: &MaintenanceRequestRecovererRegistry<'_>,
    ) -> MaintenanceOutcome {
        let journals = JournalRecovererRegistry::default();
        MaintenanceRunner::new(store, EPOCH, &journals, recoverers)
            .run()
            .unwrap()
    }

    #[test]
    fn registered_request_recoverer_completes_and_unblocks_startup() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "test.apply", EPOCH))
            .unwrap();
        let recoverer = |store: &mut WorkspaceDatabase, request: &MaintenanceRequest| {
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Prepared,
                    MaintenanceRequestStatus::Applying,
                )
                .unwrap();
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Applying,
                    MaintenanceRequestStatus::Applied,
                )
                .unwrap();
            Ok(())
        };
        let mut requests = MaintenanceRequestRecovererRegistry::default();
        requests.insert("test.apply", &recoverer);

        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::Ready
        );
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
    }

    #[test]
    fn request_recoverer_can_rotate_epoch_and_stale_epoch_requests_block() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "test.rotate", EPOCH))
            .unwrap();
        store
            .register_maintenance_request(&staged_request("request-b", "test.apply", EPOCH))
            .unwrap();
        let recoverer = |store: &mut WorkspaceDatabase, request: &MaintenanceRequest| {
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Prepared,
                    MaintenanceRequestStatus::Applying,
                )
                .unwrap();
            store
                .rotate_store_epoch("run-startup", EPOCH, "epoch-after-restore")
                .unwrap();
            store
                .transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Applying,
                    MaintenanceRequestStatus::Applied,
                )
                .unwrap();
            Ok(())
        };
        let mut requests = MaintenanceRequestRecovererRegistry::default();
        requests.insert("test.rotate", &recoverer);
        let apply_called = std::cell::Cell::new(false);
        let stale = |_: &mut WorkspaceDatabase, _: &MaintenanceRequest| {
            apply_called.set(true);
            Ok(())
        };
        requests.insert("test.apply", &stale);

        // 轮换后 request-b 的世代已不是当前世代：保留原记录并阻断，恢复器不得被调用。
        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
        assert!(!apply_called.get(), "旧世代请求不得触发恢复器");
        assert_eq!(
            store
                .maintenance_request("request-b")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Prepared
        );
        assert_eq!(
            store.metadata().unwrap().unwrap().store_epoch,
            "epoch-after-restore"
        );
    }

    #[test]
    fn unknown_request_kind_becomes_needs_attention_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "unknown.kind", EPOCH))
            .unwrap();
        let requests = MaintenanceRequestRecovererRegistry::default();

        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::NeedsAttention
        );
        let changes = store.database.connection.total_changes();
        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
        assert_eq!(store.database.connection.total_changes(), changes);
    }

    #[test]
    fn recoverer_ok_without_terminal_request_state_is_not_ready() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "test.fake", EPOCH))
            .unwrap();
        let recoverer = |_: &mut WorkspaceDatabase, _: &MaintenanceRequest| Ok(());
        let mut requests = MaintenanceRequestRecovererRegistry::default();
        requests.insert("test.fake", &recoverer);

        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Prepared
        );
    }

    #[test]
    fn leftover_recovery_point_blocks_after_requests_complete() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .database
            .connection
            .execute(
                "INSERT INTO recovery_points \
                 (recovery_point_id, object_key, status, dpapi_path) \
                 VALUES ('request-a', 'settings.json', 'Prepared', 'dpapi')",
                [],
            )
            .unwrap();
        let requests = MaintenanceRequestRecovererRegistry::default();

        assert_eq!(
            run_requests(&mut store, &requests),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );
    }

    #[test]
    fn request_recoverer_error_is_propagated_without_continuing() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .register_maintenance_request(&staged_request("request-a", "test.error", EPOCH))
            .unwrap();
        let recoverer = |_: &mut WorkspaceDatabase, _: &MaintenanceRequest| {
            Err(MaintenanceRequestError::new("test failure"))
        };
        let mut requests = MaintenanceRequestRecovererRegistry::default();
        requests.insert("test.error", &recoverer);
        let journals = JournalRecovererRegistry::default();

        let result = MaintenanceRunner::new(&mut store, EPOCH, &journals, &requests).run();

        assert!(matches!(result, Err(MaintenanceError::Request(_))));
        assert_eq!(
            store
                .maintenance_request("request-a")
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Prepared
        );
    }

    #[test]
    fn stage_order_short_circuits_before_querying_later_tables() {
        let recoverers = JournalRecovererRegistry::default();

        // Journal 未安全时，维护请求表即使已不可查询也不得被触碰。
        let journal_temp = tempfile::tempdir().unwrap();
        let mut journal_store = WorkspaceDatabase::open(&fixture(&journal_temp)).unwrap();
        begin_run(&mut journal_store);
        let _ = journal_store
            .register_journal(registration("journal-block", "unknown.block", EPOCH))
            .unwrap();
        journal_store
            .database
            .connection
            .execute("DROP TABLE maintenance_requests", [])
            .unwrap();
        assert_eq!(
            run(&mut journal_store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::Journal
            }
        );

        // 维护材料未安全时，不得读取后续关闭意图。
        let maintenance_temp = tempfile::tempdir().unwrap();
        let mut maintenance_store = WorkspaceDatabase::open(&fixture(&maintenance_temp)).unwrap();
        begin_run(&mut maintenance_store);
        maintenance_store
            .database
            .connection
            .execute_batch(
                "INSERT INTO maintenance_requests \
                 (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) \
                 VALUES ('request-a', 'restore', 'Prepared', \
                 x'0101010101010101010101010101010101010101010101010101010101010101', \
                 'epoch-startup', 'staged'); \
                 DROP TABLE close_objects; DROP TABLE close_intents;",
            )
            .unwrap();
        assert_eq!(
            run(&mut maintenance_store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::MaintenanceRequests
            }
        );

        // 关闭意图未安全时，后续阶段不得被查询。
        let close_temp = tempfile::tempdir().unwrap();
        let mut close_store = WorkspaceDatabase::open(&fixture(&close_temp)).unwrap();
        begin_run(&mut close_store);
        close_store
            .register_close_intent(&close_registration())
            .unwrap();
        assert_eq!(
            run(&mut close_store, &recoverers),
            MaintenanceOutcome::NeedsAttention {
                stage: MaintenanceStage::CloseIntents
            }
        );
    }

    #[test]
    fn raw_query_errors_keep_database_classification_and_display_is_redacted() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        begin_run(&mut store);
        store
            .database
            .connection
            .execute("DROP TABLE maintenance_requests", [])
            .unwrap();
        let recoverers = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::default();
        let result = MaintenanceRunner::new(&mut store, EPOCH, &recoverers, &requests).run();
        assert!(matches!(
            result,
            Err(MaintenanceError::Store(MaintenanceStoreError::Database(_)))
        ));

        let marker = "sensitive_object_path_sql_fingerprint";
        let sqlite_error = || {
            rusqlite::Connection::open_in_memory()
                .unwrap()
                .execute_batch(&format!("SELECT * FROM {marker}"))
                .unwrap_err()
        };
        let errors = [
            MaintenanceError::Journal(JournalError::Database(DatabaseError::from(sqlite_error()))),
            MaintenanceError::Close(CloseStoreError::Database(DatabaseError::from(
                sqlite_error(),
            ))),
            MaintenanceError::Database(DatabaseError::from(sqlite_error())),
            MaintenanceError::Store(MaintenanceStoreError::Database(DatabaseError::from(
                sqlite_error(),
            ))),
            MaintenanceError::Request(MaintenanceRequestError::new(marker)),
        ];
        for error in errors {
            assert!(!error.to_string().contains(marker));
            assert!(!error.to_string().contains("maintenance_requests"));
        }
    }
}
