//! Codex hooks 跨文件注入／卸载的 Journal 与启动恢复。
//!
//! 一次安装要改 hooks.json、归属 marker 与 config.toml；任何一步崩溃都
//! 不能留下无法卸载的残留。写前先把每个文件的前值备份到本地根，并在
//! 唯一工作区库登记步骤与前后指纹；运行期按步骤记录 outcome，启动维护
//! 期由本模块的恢复器按真实文件指纹选择提交、回滚或待处理。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::persistence::journal::{
    JournalError, JournalRegistration, JournalStatus, JournalStepRegistration, ManagedJournal,
};
use crate::persistence::startup::JournalRecoverer;
use crate::persistence::workspace::WorkspaceDatabase;
use crate::persistence::workspace_writer::WorkspaceWriterClient;

use super::super::installation::{
    HooksJournalKind, HooksRecovery, HooksRecoveryFacts, hooks_recovery,
};

/// 注入 Journal kind；恢复器只解释本 kind 的步骤材料。
pub(crate) const HOOKS_INSTALL_KIND: &str = "hooks.install";
/// 卸载 Journal kind。
pub(crate) const HOOKS_UNINSTALL_KIND: &str = "hooks.uninstall";
/// 本地根下的备份目录名。
pub(crate) const HOOK_JOURNAL_DIR: &str = "hook-journal";

const TARGET_VERSION: u64 = 1;
const STEP_KIND: &str = "file";
const JOURNAL_TIMEOUT: Duration = Duration::from_secs(10);
const ABSENT_FINGERPRINT: [u8; 32] = [0; 32];

/// 运行期 Journal 端口：唯一写线程、当前世代与备份根。
#[derive(Clone)]
pub(crate) struct HookJournalPort {
    writer: WorkspaceWriterClient,
    store_epoch: String,
    root: PathBuf,
}

impl HookJournalPort {
    pub(crate) fn new(writer: WorkspaceWriterClient, store_epoch: String, root: PathBuf) -> Self {
        Self {
            writer,
            store_epoch,
            root,
        }
    }
}

/// 单个文件的预期变更；`after=None` 表示删除。
#[derive(Clone)]
pub(crate) struct HookFilePlan {
    pub(crate) key: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) before: Option<Vec<u8>>,
    pub(crate) after: Option<Vec<u8>>,
}

struct RuntimeStep {
    path: PathBuf,
    before: Option<Vec<u8>>,
}

/// 已登记的运行期事务；每个实际写盘后必须记录一次 outcome。
pub(crate) struct HookJournal {
    writer: WorkspaceWriterClient,
    journal_id: String,
    root: PathBuf,
    steps: Vec<RuntimeStep>,
    after: BTreeMap<&'static str, Option<[u8; 32]>>,
}

impl HookJournal {
    /// 写备份并登记 Journal；失败时清理备份且零改配置。
    pub(crate) fn begin(
        port: &HookJournalPort,
        kind: &'static str,
        plans: &[HookFilePlan],
    ) -> Result<Self, String> {
        if plans.is_empty() {
            return Err("hook journal requires at least one step".to_owned());
        }
        let journal_id = uuid::Uuid::now_v7().to_string();
        let root = port.root.join(&journal_id);
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let prepared = plans
            .iter()
            .enumerate()
            .map(|(index, plan)| {
                let path = plan
                    .path
                    .to_str()
                    .ok_or_else(|| "hook journal path is not valid UTF-8".to_owned())?;
                let backup = format!("{index}.before");
                if let Some(before) = &plan.before {
                    std::fs::write(root.join(&backup), before)
                        .map_err(|error| error.to_string())?;
                }
                let expected_after = plan.after.as_deref().map(|bytes| hex(&fingerprint(bytes)));
                Ok(JournalStepRegistration {
                    step_key: plan.key.to_owned(),
                    kind: STEP_KIND.to_owned(),
                    target: json!({
                        "version": TARGET_VERSION,
                        "path": path,
                        "backup": backup,
                        "before_absent": plan.before.is_none(),
                        "after_absent": plan.after.is_none(),
                        "expected_after": expected_after,
                    }),
                    authorization_generation: 0,
                    before_fingerprint: plan.before.as_deref().map(fingerprint),
                })
            })
            .collect::<Result<Vec<_>, String>>();
        let steps = match prepared {
            Ok(steps) => steps,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&root);
                return Err(error);
            }
        };
        let registration = JournalRegistration {
            operation_id: None,
            journal_id: journal_id.clone(),
            kind: kind.to_owned(),
            store_epoch: port.store_epoch.clone(),
            before_fingerprint: Some(combined_fingerprint(plans, true)),
            steps,
        };
        if let Err(error) = register(port, registration) {
            let _ = std::fs::remove_dir_all(&root);
            return Err(error);
        }
        let after = plans
            .iter()
            .map(|plan| (plan.key, plan.after.as_deref().map(fingerprint)))
            .collect();
        let runtime = plans
            .iter()
            .map(|plan| RuntimeStep {
                path: plan.path.clone(),
                before: plan.before.clone(),
            })
            .collect();
        Ok(Self {
            writer: port.writer.clone(),
            journal_id,
            root,
            steps: runtime,
            after,
        })
    }

    /// 记录一步已经落盘；与登记时的预期指纹不一致由 Journal 拒绝。
    /// 删除步骤没有可记录的字节指纹，恢复按 target 的 after-absent 判定。
    pub(crate) fn record(&self, key: &'static str) -> Result<(), String> {
        let Some(after) = self.after.get(key) else {
            return Err("unknown hook journal step".to_owned());
        };
        let Some(after) = after else {
            return Ok(());
        };
        let response = self
            .writer
            .record_journal_step(&self.journal_id, key, *after)
            .map_err(|error| error.to_string())?;
        match response.recv_timeout(JOURNAL_TIMEOUT) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("hook journal step timed out".to_owned()),
        }
    }

    /// 全部步骤落盘后提交，并清理已无恢复用途的备份。
    pub(crate) fn commit(self) -> Result<(), String> {
        self.transition(JournalStatus::Applying, None)?;
        self.transition(JournalStatus::Committing, None)?;
        let after = combined_after(&self.after);
        self.transition(JournalStatus::Committed, Some(after))?;
        cleanup_backups(&self.root);
        Ok(())
    }

    fn transition(&self, next: JournalStatus, after: Option<[u8; 32]>) -> Result<(), String> {
        let response = self
            .writer
            .transition_journal(&self.journal_id, next, after)
            .map_err(|error| error.to_string())?;
        match response.recv_timeout(JOURNAL_TIMEOUT) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("hook journal transition timed out".to_owned()),
        }
    }

    /// 任一步失败时回滚已写文件；回滚失败保留 Journal 交启动恢复。
    pub(crate) fn rollback(self) -> Result<(), String> {
        let response = self
            .writer
            .transition_journal(&self.journal_id, JournalStatus::RollingBack, None)
            .map_err(|error| error.to_string())?;
        match response.recv_timeout(JOURNAL_TIMEOUT) {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(error.to_string()),
            Err(_) => return Err("hook journal rollback timed out".to_owned()),
        }
        for step in self.steps.iter().rev() {
            restore(&step.path, step.before.as_deref()).map_err(|error| error.to_string())?;
        }
        let response = self
            .writer
            .transition_journal(&self.journal_id, JournalStatus::RolledBack, None)
            .map_err(|error| error.to_string())?;
        match response.recv_timeout(JOURNAL_TIMEOUT) {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(error.to_string()),
            Err(_) => return Err("hook journal rollback timed out".to_owned()),
        }
        cleanup_backups(&self.root);
        Ok(())
    }
}

fn register(port: &HookJournalPort, registration: JournalRegistration) -> Result<(), String> {
    let response = port
        .writer
        .register_journal(registration)
        .map_err(|error| error.to_string())?;
    match response.recv_timeout(JOURNAL_TIMEOUT) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("hook journal registration timed out".to_owned()),
    }
}

fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut value = [0; 32];
    value.copy_from_slice(&digest);
    value
}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

fn combined_fingerprint(plans: &[HookFilePlan], before: bool) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for plan in plans {
        hasher.update(plan.key.as_bytes());
        match if before { &plan.before } else { &plan.after } {
            Some(bytes) => {
                hasher.update([1]);
                hasher.update(fingerprint(bytes));
            }
            None => hasher.update([0]),
        }
    }
    let mut value = [0; 32];
    value.copy_from_slice(&hasher.finalize());
    value
}

fn combined_after(after: &BTreeMap<&'static str, Option<[u8; 32]>>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for (key, value) in after {
        hasher.update(key.as_bytes());
        hasher.update(value.unwrap_or(ABSENT_FINGERPRINT));
    }
    let mut value = [0; 32];
    value.copy_from_slice(&hasher.finalize());
    value
}

fn restore(path: &Path, before: Option<&[u8]>) -> std::io::Result<()> {
    match before {
        Some(bytes) => crate::atomic_file::write(path, bytes),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

fn cleanup_backups(root: &Path) {
    if let Err(error) = std::fs::remove_dir_all(root) {
        crate::audit::record(crate::audit::AuditEvent::new(
            "hook_journal_cleanup",
            "ai_hook::local::hook_journal",
            crate::audit::AuditResult::Error,
        ));
        log::warn!("hook journal backup cleanup failed: {error}");
    }
}

struct RecoveryStep {
    path: PathBuf,
    backup: PathBuf,
    before_absent: bool,
    before: Option<[u8; 32]>,
    after: Option<[u8; 32]>,
}

fn recovery_step(
    root: &Path,
    step: &crate::persistence::journal::JournalStep,
) -> Option<RecoveryStep> {
    let target = step.target.as_object()?;
    if target.get("version").and_then(Value::as_u64) != Some(TARGET_VERSION) {
        return None;
    }
    let path = PathBuf::from(target.get("path")?.as_str()?);
    let backup = root.join(target.get("backup")?.as_str()?);
    let before_absent = target.get("before_absent")?.as_bool()?;
    // 删除步骤的 after 不存在由 expected_after 为空表达；字段仍参与格式校验。
    let _after_absent = target.get("after_absent")?.as_bool()?;
    let expected = match target.get("expected_after") {
        Some(Value::String(value)) => Some(parse_hex(value)?),
        Some(Value::Null) | None => None,
        Some(_) => return None,
    };
    Some(RecoveryStep {
        path,
        backup,
        before_absent,
        before: step.before_fingerprint,
        after: step.after_fingerprint.or(expected),
    })
}

fn current_fingerprint(path: &Path) -> Result<Option<[u8; 32]>, ()> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(fingerprint(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

/// 启动维护期恢复器：按 Journal 步骤与真实文件指纹结算。
pub(crate) struct HooksJournalRecoverer {
    backups_root: PathBuf,
}

impl HooksJournalRecoverer {
    pub(crate) fn new(local_root: &Path) -> Self {
        Self {
            backups_root: local_root.join(HOOK_JOURNAL_DIR),
        }
    }

    /// 维护放行后清理已无未完成 Journal 的备份目录；提交后删除失败的
    /// 材料在下一次启动收敛。清理失败只审计，不阻断工作区。
    pub(crate) fn cleanup_orphans(&self, store: &WorkspaceDatabase) {
        let Ok(entries) = std::fs::read_dir(&self.backups_root) else {
            return;
        };
        let unfinished = match store.unfinished_journals() {
            Ok(journals) => journals
                .into_iter()
                .map(|journal| journal.journal_id)
                .collect::<std::collections::BTreeSet<_>>(),
            Err(error) => {
                log::warn!("hook journal orphan scan skipped: {error}");
                return;
            }
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !unfinished.contains(name) {
                cleanup_backups(&entry.path());
            }
        }
    }
}

impl JournalRecoverer for HooksJournalRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        journal: &ManagedJournal,
    ) -> Result<(), JournalError> {
        let kind = if journal.kind == HOOKS_INSTALL_KIND {
            HooksJournalKind::Install
        } else {
            HooksJournalKind::Uninstall
        };
        let root = self.backups_root.join(&journal.journal_id);
        let mut steps = Vec::with_capacity(journal.steps.len());
        let mut classifications = Vec::with_capacity(journal.steps.len());
        for step in &journal.steps {
            let Some(recovered) = recovery_step(&root, step) else {
                return needs_attention(store, journal);
            };
            let Ok(current) = current_fingerprint(&recovered.path) else {
                return needs_attention(store, journal);
            };
            classifications.push(hooks_recovery(HooksRecoveryFacts {
                kind,
                before_fingerprint: recovered.before,
                after_fingerprint: recovered.after,
                current_fingerprint: current,
            }));
            steps.push(recovered);
        }
        if classifications.contains(&HooksRecovery::NeedsAttention) {
            return needs_attention(store, journal);
        }
        if classifications
            .iter()
            .all(|recovery| matches!(recovery, HooksRecovery::Committed(_)))
        {
            advance_commit(store, journal)?;
            cleanup_backups(&root);
            return Ok(());
        }
        if journal.status != JournalStatus::RollingBack {
            store.transition_journal(&journal.journal_id, JournalStatus::RollingBack, None)?;
        }
        for step in &steps {
            let before = if step.before_absent {
                None
            } else {
                match std::fs::read(&step.backup) {
                    Ok(bytes) => Some(bytes),
                    Err(_) => return needs_attention(store, journal),
                }
            };
            if let Err(error) = restore(&step.path, before.as_deref()) {
                log::warn!("hook journal rollback failed: {error}");
                return needs_attention(store, journal);
            }
            let restored =
                current_fingerprint(&step.path).map_err(|_| JournalError::CorruptRecord)?;
            if restored != step.before {
                return needs_attention(store, journal);
            }
        }
        store.transition_journal(&journal.journal_id, JournalStatus::RolledBack, None)?;
        cleanup_backups(&root);
        Ok(())
    }
}

fn needs_attention(
    store: &mut WorkspaceDatabase,
    journal: &ManagedJournal,
) -> Result<(), JournalError> {
    if journal.status != JournalStatus::NeedsAttention {
        store.transition_journal(&journal.journal_id, JournalStatus::NeedsAttention, None)?;
    }
    Ok(())
}

fn advance_commit(
    store: &mut WorkspaceDatabase,
    journal: &ManagedJournal,
) -> Result<(), JournalError> {
    let mut status = journal.status;
    if status == JournalStatus::Prepared {
        status = store
            .transition_journal(&journal.journal_id, JournalStatus::Applying, None)?
            .status;
    }
    if status != JournalStatus::Committing && status != JournalStatus::Committed {
        store.transition_journal(&journal.journal_id, JournalStatus::Committing, None)?;
    }
    if status != JournalStatus::Committed {
        store.transition_journal(&journal.journal_id, JournalStatus::Committed, None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use slterm_path::AppPaths;

    use super::*;
    use crate::persistence::journal::JournalStepRegistration;
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::startup::{
        MaintenanceOutcome, MaintenanceRequestRecovererRegistry, MaintenanceRunner,
    };
    use crate::persistence::workspace_writer::WorkspaceWriter;

    const EPOCH: &str = "epoch-hook-journal";

    fn paths(temp: &tempfile::TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn open(temp: &tempfile::TempDir) -> WorkspaceDatabase {
        let mut store = WorkspaceDatabase::open(&paths(temp)).unwrap();
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: uuid::Uuid::now_v7().to_string(),
                    process_id: std::process::id(),
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
        store
    }

    struct Spec<'a> {
        key: &'a str,
        path: &'a Path,
        before: Option<&'a [u8]>,
        after: Option<&'a [u8]>,
    }

    fn registration(
        journal_id: &str,
        kind: &str,
        specs: &[Spec<'_>],
        backup_root: &Path,
    ) -> JournalRegistration {
        std::fs::create_dir_all(backup_root).unwrap();
        let steps = specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let backup = format!("{index}.before");
                if let Some(before) = spec.before {
                    std::fs::write(backup_root.join(&backup), before).unwrap();
                }
                JournalStepRegistration {
                    step_key: spec.key.to_owned(),
                    kind: STEP_KIND.to_owned(),
                    target: json!({
                        "version": TARGET_VERSION,
                        "path": spec.path.to_str().unwrap(),
                        "backup": backup,
                        "before_absent": spec.before.is_none(),
                        "after_absent": spec.after.is_none(),
                        "expected_after": spec
                            .after
                            .map(|bytes| hex(&fingerprint(bytes))),
                    }),
                    authorization_generation: 0,
                    before_fingerprint: spec.before.map(fingerprint),
                }
            })
            .collect();
        JournalRegistration {
            operation_id: None,
            journal_id: journal_id.to_owned(),
            kind: kind.to_owned(),
            store_epoch: EPOCH.to_owned(),
            before_fingerprint: Some([1; 32]),
            steps,
        }
    }

    fn recover(store: &mut WorkspaceDatabase, temp: &tempfile::TempDir) -> MaintenanceOutcome {
        let recoverer = HooksJournalRecoverer::new(paths(temp).local_root());
        let recoverers = BTreeMap::from([
            (HOOKS_INSTALL_KIND, &recoverer as &dyn JournalRecoverer),
            (HOOKS_UNINSTALL_KIND, &recoverer as &dyn JournalRecoverer),
        ]);
        let requests = MaintenanceRequestRecovererRegistry::default();
        MaintenanceRunner::new(store, EPOCH, &recoverers, &requests)
            .run()
            .unwrap()
    }

    /// 提交边界崩溃：hooks 已写，marker/config 未写 → 启动恢复回滚真实文件。
    #[test]
    fn recovery_rolls_back_partial_install_from_backups() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        let marker = files.path().join("hooks.slterm-managed.json");
        let config = files.path().join("config.toml");
        std::fs::write(&hooks, b"old-hooks").unwrap();
        std::fs::write(&config, b"old-config").unwrap();
        let journal_id = "hook-install-partial";
        let backup_root = paths(&temp)
            .local_root()
            .join(HOOK_JOURNAL_DIR)
            .join(journal_id);
        let mut store = open(&temp);
        store
            .register_journal(registration(
                journal_id,
                HOOKS_INSTALL_KIND,
                &[
                    Spec {
                        key: "hooks",
                        path: &hooks,
                        before: Some(b"old-hooks"),
                        after: Some(b"new-hooks"),
                    },
                    Spec {
                        key: "marker",
                        path: &marker,
                        before: None,
                        after: Some(b"marker"),
                    },
                    Spec {
                        key: "config",
                        path: &config,
                        before: Some(b"old-config"),
                        after: Some(b"new-config"),
                    },
                ],
                &backup_root,
            ))
            .unwrap();
        std::fs::write(&hooks, b"new-hooks").unwrap();
        store
            .record_step_after(journal_id, "hooks", fingerprint(b"new-hooks"))
            .unwrap();

        assert_eq!(recover(&mut store, &temp), MaintenanceOutcome::Ready);
        assert_eq!(std::fs::read(&hooks).unwrap(), b"old-hooks");
        assert!(!marker.exists(), "未应用的创建步骤必须保持不存在");
        assert_eq!(std::fs::read(&config).unwrap(), b"old-config");
        assert_eq!(
            store.journal(journal_id).unwrap().unwrap().status,
            JournalStatus::RolledBack
        );
        assert!(!backup_root.exists(), "回滚后备份必须清理");
    }

    /// 崩溃发生在写后、记录 outcome 前：真实文件已等于预期 after → 提交。
    #[test]
    fn recovery_commits_after_crash_before_outcome_records() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        let marker = files.path().join("marker.json");
        std::fs::write(&hooks, b"new-hooks").unwrap();
        std::fs::write(&marker, b"new-marker").unwrap();
        let journal_id = "hook-install-written";
        let backup_root = paths(&temp)
            .local_root()
            .join(HOOK_JOURNAL_DIR)
            .join(journal_id);
        let mut store = open(&temp);
        store
            .register_journal(registration(
                journal_id,
                HOOKS_INSTALL_KIND,
                &[
                    Spec {
                        key: "hooks",
                        path: &hooks,
                        before: Some(b"old-hooks"),
                        after: Some(b"new-hooks"),
                    },
                    Spec {
                        key: "marker",
                        path: &marker,
                        before: None,
                        after: Some(b"new-marker"),
                    },
                ],
                &backup_root,
            ))
            .unwrap();

        assert_eq!(recover(&mut store, &temp), MaintenanceOutcome::Ready);
        assert_eq!(std::fs::read(&hooks).unwrap(), b"new-hooks");
        assert_eq!(std::fs::read(&marker).unwrap(), b"new-marker");
        assert_eq!(
            store.journal(journal_id).unwrap().unwrap().status,
            JournalStatus::Committed
        );
        assert!(!backup_root.exists());
    }

    /// 外部改写既非前值也非预期后值：不得猜测，保留材料并阻断启动。
    #[test]
    fn recovery_blocks_when_file_matches_neither_fingerprint() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        std::fs::write(&hooks, b"foreign-edit").unwrap();
        let journal_id = "hook-install-foreign";
        let backup_root = paths(&temp)
            .local_root()
            .join(HOOK_JOURNAL_DIR)
            .join(journal_id);
        let mut store = open(&temp);
        store
            .register_journal(registration(
                journal_id,
                HOOKS_INSTALL_KIND,
                &[Spec {
                    key: "hooks",
                    path: &hooks,
                    before: Some(b"old-hooks"),
                    after: Some(b"new-hooks"),
                }],
                &backup_root,
            ))
            .unwrap();

        assert!(matches!(
            recover(&mut store, &temp),
            MaintenanceOutcome::NeedsAttention { .. }
        ));
        assert_eq!(std::fs::read(&hooks).unwrap(), b"foreign-edit");
        assert_eq!(
            store.journal(journal_id).unwrap().unwrap().status,
            JournalStatus::NeedsAttention
        );
        assert!(backup_root.exists(), "待处理材料不得清理");
    }

    /// 卸载中途崩溃：已删 marker、hooks 尚未改 → 回滚恢复 marker。
    #[test]
    fn recovery_restores_deleted_marker_during_partial_uninstall() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        let marker = files.path().join("marker.json");
        std::fs::write(&hooks, b"old-hooks").unwrap();
        let journal_id = "hook-uninstall-partial";
        let backup_root = paths(&temp)
            .local_root()
            .join(HOOK_JOURNAL_DIR)
            .join(journal_id);
        let mut store = open(&temp);
        store
            .register_journal(registration(
                journal_id,
                HOOKS_UNINSTALL_KIND,
                &[
                    Spec {
                        key: "hooks",
                        path: &hooks,
                        before: Some(b"old-hooks"),
                        after: Some(b"stripped"),
                    },
                    Spec {
                        key: "marker",
                        path: &marker,
                        before: Some(b"marker"),
                        after: None,
                    },
                ],
                &backup_root,
            ))
            .unwrap();
        // marker 已被删除，hooks 仍是前值。
        assert_eq!(std::fs::read(&hooks).unwrap(), b"old-hooks");

        assert_eq!(recover(&mut store, &temp), MaintenanceOutcome::Ready);
        assert_eq!(std::fs::read(&marker).unwrap(), b"marker");
        assert_eq!(std::fs::read(&hooks).unwrap(), b"old-hooks");
        assert_eq!(
            store.journal(journal_id).unwrap().unwrap().status,
            JournalStatus::RolledBack
        );
    }

    /// 启动清理只删无未完成 Journal 的备份，登记的未完成材料保留。
    #[test]
    fn orphan_cleanup_keeps_materials_of_unfinished_journals() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let path = files.path().join("hooks.json");
        std::fs::write(&path, b"old").unwrap();
        let root = paths(&temp).local_root().join(HOOK_JOURNAL_DIR);
        let orphan = root.join("orphan");
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(orphan.join("0.before"), b"orphan").unwrap();
        let active = root.join("active");
        let mut store = open(&temp);
        store
            .register_journal(registration(
                "active",
                HOOKS_INSTALL_KIND,
                &[Spec {
                    key: "hooks",
                    path: &path,
                    before: Some(b"old"),
                    after: Some(b"new"),
                }],
                &active,
            ))
            .unwrap();

        HooksJournalRecoverer::new(paths(&temp).local_root()).cleanup_orphans(&store);

        assert!(!orphan.exists(), "无 Journal 的备份必须清理");
        assert!(active.exists(), "未完成 Journal 的材料必须保留");
    }

    #[test]
    fn runtime_transaction_commits_through_the_unique_writer() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        std::fs::write(&hooks, b"old-hooks").unwrap();
        drop(open(&temp));
        let writer = WorkspaceWriter::start(paths(&temp)).unwrap();
        let port = HookJournalPort::new(
            writer.client(),
            EPOCH.to_owned(),
            paths(&temp).local_root().join(HOOK_JOURNAL_DIR),
        );
        let journal = HookJournal::begin(
            &port,
            HOOKS_INSTALL_KIND,
            &[HookFilePlan {
                key: "hooks",
                path: hooks.clone(),
                before: Some(b"old-hooks".to_vec()),
                after: Some(b"new-hooks".to_vec()),
            }],
        )
        .unwrap();
        std::fs::write(&hooks, b"new-hooks").unwrap();
        journal.record("hooks").unwrap();
        journal.commit().unwrap();
        writer.shutdown().unwrap();

        let store = open(&temp);
        assert!(
            store
                .unfinished_journals()
                .unwrap()
                .into_iter()
                .all(|journal| journal.kind != HOOKS_INSTALL_KIND),
            "提交后不得残留未完成 Journal"
        );
        assert_eq!(std::fs::read(&hooks).unwrap(), b"new-hooks");
    }

    #[test]
    fn runtime_rollback_restores_the_original_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let hooks = files.path().join("hooks.json");
        std::fs::write(&hooks, b"old-hooks").unwrap();
        drop(open(&temp));
        let writer = WorkspaceWriter::start(paths(&temp)).unwrap();
        let port = HookJournalPort::new(
            writer.client(),
            EPOCH.to_owned(),
            paths(&temp).local_root().join(HOOK_JOURNAL_DIR),
        );
        let journal = HookJournal::begin(
            &port,
            HOOKS_INSTALL_KIND,
            &[HookFilePlan {
                key: "hooks",
                path: hooks.clone(),
                before: Some(b"old-hooks".to_vec()),
                after: Some(b"new-hooks".to_vec()),
            }],
        )
        .unwrap();
        std::fs::write(&hooks, b"new-hooks").unwrap();
        journal.record("hooks").unwrap();
        journal.rollback().unwrap();
        writer.shutdown().unwrap();

        assert_eq!(std::fs::read(&hooks).unwrap(), b"old-hooks");
        let store = open(&temp);
        assert!(
            store
                .unfinished_journals()
                .unwrap()
                .into_iter()
                .all(|journal| journal.kind != HOOKS_INSTALL_KIND)
        );
    }
}
