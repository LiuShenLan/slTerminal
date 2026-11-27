//! 备份恢复的 staged 维护请求：启动维护期按类别应用，失败或中断全回滚。
//!
//! 运行期只投影所选类别并原子暂存，不落口令、不写目标文件；staged 条目与
//! 恢复点正文都经 DPAPI CurrentUser 加密，应用中断后重启只回滚、不重放未知
//! 副作用。应用成功才在同一事务内更换 `store_epoch` 并推进 `Applied`，使
//! 旧世代的 Journal／Operation 写全部失效。

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use slterm_path::AppPaths;
use zeroize::Zeroizing;

use super::recovery::{
    OriginalFile, RestoreTarget, decode_point, read_original, write_originals, write_point,
};
use super::{
    ARCHIVE_VERSION, BackupArchive, BackupCategory, BackupEntry, BackupManifest, BackupSelection,
    category_root, restore_path, validate_archive,
};
use crate::persistence::maintenance_request::{
    MaintenanceRequest, MaintenanceRequestStatus, RecoveryPointRecord, RecoveryPointStatus,
};
use crate::persistence::startup::{MaintenanceRequestError, MaintenanceRequestRecoverer};
use crate::persistence::workspace::WorkspaceDatabase;

/// 备份恢复维护请求的 kind；只有本模块恢复器解释 staged 材料。
pub(crate) const BACKUP_RESTORE_KIND: &str = "backup.restore";

/// 测试注入的应用故障点（仅测试构建存在）；`None` = 关闭。
#[cfg(test)]
static MAINTENANCE_APPLY_FAILPOINT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[derive(Serialize, Deserialize)]
struct StagedRestore {
    entries: Vec<BackupEntry>,
}

const fn category_key(category: BackupCategory) -> &'static str {
    match category {
        BackupCategory::Settings => "settings",
        BackupCategory::Workspace => "workspace",
    }
}

/// 恢复点行的对象键：类别前缀 + 根内相对名，禁止跨根同名混叠。
pub(super) fn object_key(target: &RestoreTarget) -> String {
    format!("{}:{}", category_key(target.category), target.name)
}

fn point_relative(request_id: &str) -> String {
    format!("backup-recovery/{request_id}.slterm-recovery")
}

fn point_file_name(request_id: &str) -> String {
    format!("{request_id}.slterm-recovery")
}

fn local_path(paths: &AppPaths, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("不安全的维护材料路径: {relative:?}"));
    }
    Ok(paths.local_root().join(path))
}

fn remove_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清理维护材料失败: {error}")),
    }
}

/// 运行期入口：全量校验所选类别 → 加密并登记 Prepared 请求 → 原子写暂存。
///
/// 口令与解密明文不落盘；暂存内容经导入侧敏感键复验，外部含凭据字段的
/// 归档整单拒绝。先有 owner 行再有材料，失败时显式撤销 owner。
pub(crate) fn stage_restore(
    store: &mut WorkspaceDatabase,
    paths: &AppPaths,
    archive: &BackupArchive,
    selection: BackupSelection,
) -> Result<MaintenanceRequest, String> {
    validate_archive(archive)?;
    let categories: HashSet<_> = selection.categories().collect();
    if categories.is_empty() {
        return Err("至少选择一个恢复类别".to_owned());
    }
    let entries: Vec<BackupEntry> = archive
        .entries
        .iter()
        .filter(|entry| categories.contains(&entry.category))
        .cloned()
        .collect();
    if entries.is_empty() {
        return Err("所选类别没有可恢复文件".to_owned());
    }
    let request_id = uuid::Uuid::now_v7().to_string();
    let relative = format!("maintenance/{request_id}/staged.restore");
    let path = local_path(paths, &relative)?;
    let maintenance_root = local_path(paths, "maintenance")?;
    crate::atomic_file::create_dir_current_user(&maintenance_root)
        .map_err(|error| format!("创建维护目录失败: {error}"))?;
    let request_directory = path.parent().ok_or("暂存路径无效")?;
    crate::atomic_file::create_dir_current_user(request_directory)
        .map_err(|error| format!("创建暂存目录失败: {error}"))?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&StagedRestore { entries })
            .map_err(|error| format!("序列化暂存材料失败: {error}"))?,
    );
    // 暂存件不得以明文携带解密条目：与恢复点同一 DPAPI CurrentUser 保护。
    let payload = crate::platform::dpapi::protect(&plaintext)
        .map_err(|error| format!("加密暂存材料失败: {error}"))?;
    let epoch = store
        .metadata()
        .map_err(|error| format!("读取工作区世代失败: {error}"))?
        .ok_or("工作区运行元数据缺失")?
        .store_epoch;
    let request = MaintenanceRequest {
        request_id,
        kind: BACKUP_RESTORE_KIND.to_owned(),
        status: MaintenanceRequestStatus::Prepared,
        staged_fingerprint: Sha256::digest(&payload).into(),
        store_epoch: epoch,
        staged_path: relative,
    };
    // owner 行先落定，材料随后写入：崩溃只留下可判定的 Prepared 缺失材料，
    // 不会产生无主文件；写失败时显式撤掉该 owner。
    if let Err(error) = store.register_maintenance_request(&request) {
        audit_backup(
            "backup_restore_stage",
            &path,
            crate::audit::AuditResult::Error,
        );
        return Err(format!("登记维护请求失败: {error}"));
    }
    if let Err(error) = crate::atomic_file::write_current_user(&path, &payload) {
        audit_backup(
            "backup_restore_stage",
            &path,
            crate::audit::AuditResult::Error,
        );
        let cleanup = store.discard_prepared_maintenance_request(&request.request_id);
        return Err(match cleanup {
            Ok(()) => format!("写入暂存材料失败: {error}"),
            Err(cleanup) => format!("写入暂存材料失败: {error}; 撤销维护请求失败: {cleanup}"),
        });
    }
    audit_backup("backup_restore_stage", &path, crate::audit::AuditResult::Ok);
    Ok(request)
}

fn audit_backup(operation: &'static str, path: &Path, result: crate::audit::AuditResult) {
    crate::audit::record(
        crate::audit::AuditEvent::new(operation, "encrypted_backup::maintenance", result)
            .with_path(path),
    );
}

/// 应用单个条目：设置文件经统一 settings_store 合并写，其余为原子文件写。
///
/// 设置合并保留本机未知命名空间；大小与导出白名单已在归档校验时锁定。
fn write_backup_entry(target: &Path, entry: &BackupEntry) -> Result<(), String> {
    if entry.name == "settings.json" {
        let raw = slterm_settings::RawSettings::from_json_bytes(&entry.bytes);
        let updates = slterm_settings::keys::KEYS
            .iter()
            .filter(|def| def.exportable && !def.sensitive)
            .filter_map(|def| {
                raw.persisted_value(def.key)
                    .map(|value| (def.key, value.clone()))
            })
            .collect::<Vec<_>>();
        crate::settings_store::save_settings_blocking_at(target, &updates)
            .map_err(|error| format!("应用 {} 失败: {error}", entry.name))?;
    } else {
        crate::atomic_file::write(target, &entry.bytes)
            .map_err(|error| format!("应用 {} 失败: {error}", entry.name))?;
    }
    Ok(())
}

/// 启动维护期恢复器：Prepared 应用，Applying 中断回滚，其他交由人工处理。
pub(crate) struct BackupRestoreRecoverer {
    paths: AppPaths,
}

impl BackupRestoreRecoverer {
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    fn apply(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), String> {
        let staged_path = local_path(&self.paths, &request.staged_path)?;
        let staged_bytes =
            fs::read(&staged_path).map_err(|error| format!("读取暂存材料失败: {error}"))?;
        let fingerprint: [u8; 32] = Sha256::digest(&staged_bytes).into();
        if fingerprint != request.staged_fingerprint {
            return self.block(store, request, "暂存材料指纹不符");
        }
        let staged_plaintext = match crate::platform::dpapi::unprotect(&staged_bytes) {
            Ok(plaintext) => Zeroizing::new(plaintext),
            Err(_) => return self.block(store, request, "暂存材料无法解密"),
        };
        let staged: StagedRestore = match serde_json::from_slice(&staged_plaintext) {
            Ok(staged) => staged,
            Err(_) => return self.block(store, request, "暂存材料内容无效"),
        };
        let mut categories = Vec::new();
        for entry in &staged.entries {
            if !categories.contains(&entry.category) {
                categories.push(entry.category);
            }
        }
        let archive = BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories,
            },
            entries: staged.entries,
        };
        if let Err(error) = validate_archive(&archive) {
            return self.block(store, request, &error);
        }
        // 原现场必须在任何目标写入之前落定并加密保存。
        let originals = archive
            .entries
            .iter()
            .map(|entry| read_original(&self.paths, entry))
            .collect::<Result<Vec<_>, _>>()?;
        store
            .clear_recovery_points(&request.request_id)
            .map_err(|error| format!("清理恢复点登记失败: {error}"))?;
        write_point(
            &self.paths,
            &point_file_name(&request.request_id),
            &originals,
        )?;
        for original in &originals {
            store
                .record_recovery_point(&RecoveryPointRecord {
                    recovery_point_id: request.request_id.clone(),
                    object_key: object_key(&original.target),
                    status: RecoveryPointStatus::Prepared,
                    before_fingerprint: original
                        .bytes
                        .as_deref()
                        .map(|bytes| Sha256::digest(bytes).into()),
                    after_fingerprint: None,
                    dpapi_path: point_relative(&request.request_id),
                })
                .map_err(|error| format!("登记恢复点失败: {error}"))?;
        }
        store
            .transition_maintenance_request(
                &request.request_id,
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .map_err(|error| format!("推进维护请求失败: {error}"))?;

        for entry in &archive.entries {
            #[cfg(test)]
            if MAINTENANCE_APPLY_FAILPOINT
                .lock()
                .expect("应用故障点锁")
                .as_deref()
                == Some(entry.name.as_str())
            {
                return self.roll_back_after_failure(
                    store,
                    request,
                    &originals,
                    "应用故障注入".to_owned(),
                );
            }
            if entry.category == BackupCategory::Workspace {
                // 工作区条目在维护期的写库恢复归后续切片；接入前明确拒绝，
                // 不得把逻辑投影当文件写到本地根。
                return self.roll_back_after_failure(
                    store,
                    request,
                    &originals,
                    "维护期工作区恢复尚未接入".to_owned(),
                );
            }
            let target = restore_path(category_root(&self.paths, entry.category), &entry.name)?;
            match write_backup_entry(&target, entry) {
                Ok(()) => audit_backup(
                    "backup_restore_apply",
                    &target,
                    crate::audit::AuditResult::Ok,
                ),
                Err(error) => {
                    audit_backup(
                        "backup_restore_apply",
                        &target,
                        crate::audit::AuditResult::Error,
                    );
                    return self.roll_back_after_failure(store, request, &originals, error);
                }
            }
        }

        // 应用成功才更换世代：旧 Journal／Operation 写从此失效。
        // 轮换与 `Applied` 必须原子提交，否则中间崩溃会让重启看到
        // 「请求 Applying + 当前世代已更换」而永久阻断。
        let metadata = store
            .metadata()
            .map_err(|error| format!("读取工作区运行事实失败: {error}"))?
            .ok_or("工作区运行元数据缺失")?;
        let staged = local_path(&self.paths, &request.staged_path)?;
        if let Err(error) = store.complete_maintenance_apply(
            &request.request_id,
            &metadata.run.run_id,
            &request.store_epoch,
            &uuid::Uuid::now_v7().to_string(),
        ) {
            audit_backup(
                "backup_restore_commit",
                &staged,
                crate::audit::AuditResult::Error,
            );
            return Err(format!("完成维护应用失败: {error}"));
        }
        audit_backup(
            "backup_restore_commit",
            &staged,
            crate::audit::AuditResult::Ok,
        );
        self.cleanup(store, request)
    }

    /// 应用中途失败：全回滚；回滚失败保留恢复点材料并转入人工处理。
    fn roll_back_after_failure(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
        originals: &[OriginalFile],
        failure: String,
    ) -> Result<(), String> {
        match write_originals(&self.paths, originals) {
            Ok(()) => {
                store
                    .transition_maintenance_request(
                        &request.request_id,
                        MaintenanceRequestStatus::Applying,
                        MaintenanceRequestStatus::RolledBack,
                    )
                    .map_err(|error| format!("{failure}; 标记回滚失败: {error}"))?;
                self.cleanup(store, request)?;
                Err(failure)
            }
            Err(rollback) => {
                let _ = store.transition_maintenance_request(
                    &request.request_id,
                    MaintenanceRequestStatus::Applying,
                    MaintenanceRequestStatus::NeedsAttention,
                );
                Err(format!("{failure}; 回滚失败: {rollback}"))
            }
        }
    }

    /// 重启后的 Applying 请求：按恢复点全量回滚，不重放未知副作用。
    fn roll_back(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), String> {
        let rows = store
            .recovery_points(&request.request_id)
            .map_err(|error| format!("读取恢复点登记失败: {error}"))?;
        if rows.is_empty() {
            return self.block(store, request, "中断的应用缺少恢复点");
        }
        let expected_point = point_relative(&request.request_id);
        if rows.iter().any(|row| row.dpapi_path != expected_point) {
            return self.block(store, request, "恢复点登记指向外部位置");
        }
        let point = local_path(&self.paths, &expected_point)?;
        let packet = match fs::read(&point) {
            Ok(packet) => packet,
            Err(_) => return self.block(store, request, "恢复点材料缺失或不可读"),
        };
        let originals = match decode_point(&packet) {
            Ok(originals) => originals,
            Err(_) => return self.block(store, request, "恢复点材料无法解密"),
        };
        let expected: BTreeSet<String> = rows.iter().map(|row| row.object_key.clone()).collect();
        let actual: BTreeSet<String> = originals
            .iter()
            .map(|original| object_key(&original.target))
            .collect();
        if expected != actual {
            return self.block(store, request, "恢复点集合与登记事实不一致");
        }
        if let Err(error) = write_originals(&self.paths, &originals) {
            return self.block(store, request, &format!("中断恢复回滚失败: {error}"));
        }
        store
            .transition_maintenance_request(
                &request.request_id,
                MaintenanceRequestStatus::Applying,
                MaintenanceRequestStatus::RolledBack,
            )
            .map_err(|error| format!("标记回滚失败: {error}"))?;
        self.cleanup(store, request)
    }

    /// 不可安全推进的请求显式标记人工处理；材料保留供诊断。
    fn block(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
        detail: &str,
    ) -> Result<(), String> {
        let _ = store.transition_maintenance_request(
            &request.request_id,
            request.status,
            MaintenanceRequestStatus::NeedsAttention,
        );
        Err(detail.to_owned())
    }

    fn cleanup(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), String> {
        // 先删材料再清登记：删文件失败时登记仍在，留下可判定的阻断事实；
        // 反过来会在清理失败后丢失材料身份。
        let staged = local_path(&self.paths, &request.staged_path)?;
        let result = (|| -> Result<(), String> {
            remove_file(&local_path(
                &self.paths,
                &point_relative(&request.request_id),
            )?)?;
            remove_file(&staged)?;
            if let Some(parent) = staged.parent() {
                let _ = fs::remove_dir(parent);
            }
            store
                .clear_recovery_points(&request.request_id)
                .map_err(|error| format!("清理恢复点登记失败: {error}"))
        })();
        audit_backup(
            "backup_restore_cleanup",
            &staged,
            if result.is_ok() {
                crate::audit::AuditResult::Ok
            } else {
                crate::audit::AuditResult::Error
            },
        );
        result
    }
}

impl MaintenanceRequestRecoverer for BackupRestoreRecoverer {
    fn recover(
        &self,
        store: &mut WorkspaceDatabase,
        request: &MaintenanceRequest,
    ) -> Result<(), MaintenanceRequestError> {
        let outcome = match request.status {
            MaintenanceRequestStatus::Prepared => self.apply(store, request),
            MaintenanceRequestStatus::Applying => self.roll_back(store, request),
            MaintenanceRequestStatus::NeedsAttention => Err("维护请求需要人工处理".to_owned()),
            MaintenanceRequestStatus::Applied | MaintenanceRequestStatus::RolledBack => Ok(()),
        };
        outcome.map_err(MaintenanceRequestError::new)
    }
}

#[cfg(test)]
mod maintenance_restore_tests {
    use std::fs;

    use sha2::{Digest as _, Sha256};
    use slterm_path::AppPaths;
    use tempfile::TempDir;

    use super::super::recovery::{OriginalFile, RestoreTarget, write_point};
    use super::super::{
        BackupArchive, BackupCategory, BackupEntry, BackupManifest, BackupSelection,
    };
    use super::{BACKUP_RESTORE_KIND, BackupRestoreRecoverer, stage_restore};
    use crate::persistence::journal::{JournalError, JournalRegistration, JournalStepRegistration};
    use crate::persistence::maintenance_request::{MaintenanceRequestStatus, RecoveryPointRecord};
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::startup::{
        JournalRecovererRegistry, MaintenanceError, MaintenanceOutcome,
        MaintenanceRequestRecoverer, MaintenanceRequestRecovererRegistry, MaintenanceRunner,
    };
    use crate::persistence::workspace::WorkspaceDatabase;

    const EPOCH: &str = "epoch-1";
    const OLD_SETTINGS: &[u8] = br#"{"appearance":{"theme":"Linear"}}"#;
    const NEW_SETTINGS: &[u8] = br#"{"appearance":{"theme":"Nord"}}"#;
    const NEW_THEME: &str = "themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn assert_current_user_only(path: &std::path::Path) {
        let dacl = crate::atomic_file::dacl_sddl(path).expect("读取 DACL");
        let (control, aces) = dacl.split_once('(').expect("SDDL 必须包含 ACE");
        assert!(matches!(control, "D:P" | "D:PAI"), "必须禁止继承: {dacl}");
        assert!(
            aces.starts_with("A;") && !aces.contains(")("),
            "唯一显式 ACE: {dacl}"
        );
    }

    fn theme_bytes() -> Vec<u8> {
        crate::theme_library::builtin_documents()
            .first()
            .expect("内置主题文档非空")
            .to_json_bytes()
            .expect("内置主题文档可序列化")
    }

    fn begin_run(store: &mut WorkspaceDatabase) {
        store
            .begin_run(
                EPOCH,
                RunMarker {
                    run_id: "run-1".into(),
                    process_id: 1,
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
    }

    fn archive() -> BackupArchive {
        BackupArchive {
            manifest: BackupManifest {
                version: super::super::ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                    bytes: NEW_SETTINGS.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: NEW_THEME.into(),
                    bytes: theme_bytes(),
                },
            ],
        }
    }

    fn selection() -> BackupSelection {
        BackupSelection::from_categories([BackupCategory::Settings])
    }

    fn maintenance(
        store: &mut WorkspaceDatabase,
        paths: &AppPaths,
    ) -> Result<MaintenanceOutcome, MaintenanceError> {
        let recoverer = BackupRestoreRecoverer::new(paths.clone());
        let journals = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::from([(
            BACKUP_RESTORE_KIND,
            &recoverer as &dyn MaintenanceRequestRecoverer,
        )]);
        let epoch = store.metadata().unwrap().unwrap().store_epoch;
        MaintenanceRunner::new(store, &epoch, &journals, &requests).run()
    }

    fn setup(temp: &TempDir) -> (AppPaths, WorkspaceDatabase) {
        let paths = fixture(temp);
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(paths.settings_path(), OLD_SETTINGS).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        begin_run(&mut store);
        (paths, store)
    }

    fn point_relative(request_id: &str) -> String {
        format!("backup-recovery/{request_id}.slterm-recovery")
    }

    fn seed_interrupted_apply(
        store: &mut WorkspaceDatabase,
        paths: &AppPaths,
        request_id: &str,
        originals: &[OriginalFile],
    ) {
        write_point(paths, &format!("{request_id}.slterm-recovery"), originals).unwrap();
        for original in originals {
            store
                .record_recovery_point(&RecoveryPointRecord {
                    recovery_point_id: request_id.to_owned(),
                    object_key: super::object_key(&original.target),
                    status: crate::persistence::maintenance_request::RecoveryPointStatus::Prepared,
                    before_fingerprint: None,
                    after_fingerprint: None,
                    dpapi_path: point_relative(request_id),
                })
                .unwrap();
        }
        store
            .transition_maintenance_request(
                request_id,
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
    }

    #[test]
    fn staged_restore_is_applied_at_startup_and_rotates_store_epoch() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        assert_eq!(request.status, MaintenanceRequestStatus::Prepared);
        assert!(paths.local_root().join(&request.staged_path).is_file());

        assert_eq!(
            maintenance(&mut store, &paths).unwrap(),
            MaintenanceOutcome::Ready
        );
        let restored = slterm_settings::RawSettings::from_json_bytes(
            &fs::read(paths.settings_path()).unwrap(),
        );
        assert_eq!(restored.value("theme"), Some("Nord"));
        assert_eq!(
            fs::read(paths.config_root().join(NEW_THEME)).unwrap(),
            theme_bytes()
        );
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
        assert!(
            store
                .recovery_points(&request.request_id)
                .unwrap()
                .is_empty()
        );
        assert_ne!(store.metadata().unwrap().unwrap().store_epoch, EPOCH);
        assert!(!paths.local_root().join(&request.staged_path).exists());
        assert!(
            !paths
                .local_root()
                .join(point_relative(&request.request_id))
                .exists()
        );

        // 应用成功后旧代次写全部失效：Journal 登记必须使用新世代。
        let stale = JournalRegistration {
            operation_id: None,
            journal_id: "journal-stale".into(),
            kind: "test.stale".into(),
            store_epoch: EPOCH.into(),
            before_fingerprint: Some([1; 32]),
            steps: vec![JournalStepRegistration {
                step_key: "step-a".into(),
                kind: "replace".into(),
                target: serde_json::json!({"version": 1}),
                authorization_generation: 1,
                before_fingerprint: Some([2; 32]),
            }],
        };
        assert!(matches!(
            store.register_journal(stale),
            Err(JournalError::StoreEpochConflict)
        ));
    }

    /// 设置恢复经 settings_store 合并：覆盖导出字段，同时保留本机未知命名空间。
    #[test]
    fn settings_restore_merges_exported_values_and_preserves_local_fields() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear","font_family":"LocalFont"},"handmade":{"note":"keep"}}"#,
        )
        .unwrap();
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        assert_eq!(
            maintenance(&mut store, &paths).unwrap(),
            MaintenanceOutcome::Ready
        );
        let restored = fs::read_to_string(paths.settings_path()).unwrap();
        assert!(restored.contains("Nord"), "导出字段必须覆盖: {restored}");
        assert!(
            restored.contains("LocalFont"),
            "未入包的已登记字段保持本机值: {restored}"
        );
        assert!(
            restored.contains("handmade") && restored.contains("keep"),
            "未知命名空间必须原位保留: {restored}"
        );
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
    }

    /// 设置条目超过设置写通道 1 MiB 上限时整单拒绝。
    #[test]
    fn oversized_settings_entry_is_rejected_before_staging() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let mut oversized = archive();
        let padding = "x".repeat(slterm_settings::MAX_PERSIST_BYTES);
        oversized.entries[0].bytes =
            format!(r#"{{"appearance":{{"font_family":"{padding}"}}}}"#).into_bytes();
        assert!(stage_restore(&mut store, &paths, &oversized, selection()).is_err());
        assert!(store.unfinished_maintenance_requests().unwrap().is_empty());
    }

    /// 暂存、应用、提交与清理都产生 audit 事件；事件不含内容或凭据。
    #[test]
    fn backup_restore_emits_audit_events_for_each_security_write() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let captured = crate::audit::test_support::capture_events();
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        assert_eq!(
            maintenance(&mut store, &paths).unwrap(),
            MaintenanceOutcome::Ready
        );
        let text = captured.text();
        for operation in [
            "backup_restore_stage",
            "backup_restore_apply",
            "backup_restore_commit",
            "backup_restore_cleanup",
            "backup_recovery_point",
        ] {
            assert!(
                text.contains(&format!("operation={operation}"))
                    || text.contains(&format!("operation=\"{operation}\"")),
                "缺少审计事件 {operation}: {text}"
            );
        }
        assert!(
            text.contains("target=audit") || text.contains("audit"),
            "{text}"
        );
        assert!(!text.contains("Nord"), "审计不得携带设置内容: {text}");
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
    }

    /// 撤销 owner 只允许未开始的 Prepared 请求。
    #[test]
    fn discard_prepared_request_never_removes_started_material() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let first = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        store
            .discard_prepared_maintenance_request(&first.request_id)
            .unwrap();
        assert!(
            store
                .maintenance_request(&first.request_id)
                .unwrap()
                .is_none(),
            "Prepared 请求可撤销"
        );

        let second = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        store
            .transition_maintenance_request(
                &second.request_id,
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
        assert!(
            store
                .discard_prepared_maintenance_request(&second.request_id)
                .is_err(),
            "已开始应用的事实不得删除"
        );
        assert!(
            store
                .maintenance_request(&second.request_id)
                .unwrap()
                .is_some()
        );
    }

    /// 暂存件以 DPAPI 保护，不得在磁盘上泄露解密条目。
    #[test]
    fn staged_restore_is_dpapi_protected_and_carries_no_plaintext() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let mut candidate = archive();
        candidate.entries[0].bytes = br#"{"appearance":{"font_family":"SecretMarker"}}"#.to_vec();
        let request = stage_restore(&mut store, &paths, &candidate, selection()).unwrap();
        let staged_path = paths.local_root().join(&request.staged_path);
        assert_current_user_only(&staged_path);
        assert_current_user_only(staged_path.parent().unwrap());
        assert_current_user_only(&paths.local_root().join("maintenance"));
        let staged = fs::read(&staged_path).unwrap();
        assert!(
            !staged
                .windows("SecretMarker".len())
                .any(|window| window == b"SecretMarker"),
            "暂存件不得以明文携带条目"
        );
        let plaintext = crate::platform::dpapi::unprotect(&staged).expect("暂存件可由当前用户解密");
        let decoded: super::StagedRestore = serde_json::from_slice(&plaintext).unwrap();
        assert!(
            decoded.entries[0]
                .bytes
                .windows("SecretMarker".len())
                .any(|window| window == b"SecretMarker"),
            "DPAPI 明文应还原原始条目"
        );
        assert_eq!(
            request.staged_fingerprint,
            <[u8; 32]>::from(Sha256::digest(&staged)),
            "指纹绑定 DPAPI 密文"
        );
    }

    /// 世代轮换与 Applied 在同一事务提交；错误旧世代不得留下半提交事实。
    #[test]
    fn apply_completion_rotates_and_transitions_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        store
            .transition_maintenance_request(
                &request.request_id,
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();
        let before = store.metadata().unwrap().unwrap();
        assert!(
            store
                .complete_maintenance_apply(
                    &request.request_id,
                    &before.run.run_id,
                    "wrong-epoch",
                    "epoch-after",
                )
                .is_err(),
            "旧世代不符必须拒绝"
        );
        let unchanged = store.metadata().unwrap().unwrap();
        assert_eq!(
            unchanged.store_epoch, before.store_epoch,
            "失败不得轮换世代"
        );
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applying,
            "失败不得推进终态"
        );

        store
            .complete_maintenance_apply(
                &request.request_id,
                &before.run.run_id,
                &request.store_epoch,
                "epoch-after",
            )
            .expect("原子提交应用完成");
        assert_eq!(
            store.metadata().unwrap().unwrap().store_epoch,
            "epoch-after"
        );
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
    }

    #[test]
    fn interrupted_apply_is_rolled_back_on_restart() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        let originals = vec![
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                },
                bytes: Some(OLD_SETTINGS.to_vec()),
                after: None,
            },
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Settings,
                    name: NEW_THEME.into(),
                },
                bytes: None,
                after: None,
            },
        ];
        seed_interrupted_apply(&mut store, &paths, &request.request_id, &originals);
        // 中断现场：第一项已写为新内容。
        fs::write(paths.settings_path(), NEW_SETTINGS).unwrap();

        assert_eq!(
            maintenance(&mut store, &paths).unwrap(),
            MaintenanceOutcome::Ready
        );
        assert_eq!(fs::read(paths.settings_path()).unwrap(), OLD_SETTINGS);
        assert!(!paths.config_root().join(NEW_THEME).exists());
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::RolledBack
        );
        assert!(
            store
                .recovery_points(&request.request_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            !paths
                .local_root()
                .join(point_relative(&request.request_id))
                .exists()
        );
        assert!(!paths.local_root().join(&request.staged_path).exists());
        assert_eq!(store.metadata().unwrap().unwrap().store_epoch, EPOCH);
    }

    #[test]
    fn apply_failure_rolls_back_in_process() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();

        *super::MAINTENANCE_APPLY_FAILPOINT.lock().unwrap() = Some(NEW_THEME.into());
        let result = maintenance(&mut store, &paths);
        *super::MAINTENANCE_APPLY_FAILPOINT.lock().unwrap() = None;

        assert!(matches!(result, Err(MaintenanceError::Request(_))));
        assert_eq!(fs::read(paths.settings_path()).unwrap(), OLD_SETTINGS);
        assert!(!paths.config_root().join(NEW_THEME).exists());
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::RolledBack
        );
        assert!(
            store
                .recovery_points(&request.request_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            !paths
                .local_root()
                .join(point_relative(&request.request_id))
                .exists()
        );
        assert!(!paths.local_root().join(&request.staged_path).exists());
    }

    #[test]
    fn stage_restore_rejects_unsanitized_or_empty_material_without_writes() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);

        let mut unsanitized = archive();
        unsanitized.entries[0].bytes = br#"{"api_key":"sk-test"}"#.to_vec();
        assert!(stage_restore(&mut store, &paths, &unsanitized, selection()).is_err());
        assert!(store.unfinished_maintenance_requests().unwrap().is_empty());
        assert!(!paths.local_root().join("maintenance").exists());

        assert!(
            stage_restore(
                &mut store,
                &paths,
                &archive(),
                BackupSelection::from_categories([])
            )
            .is_err()
        );
        assert!(store.unfinished_maintenance_requests().unwrap().is_empty());
    }

    #[test]
    fn tampered_staged_material_blocks_and_marks_needs_attention() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        let staged = paths.local_root().join(&request.staged_path);
        let mut bytes = fs::read(&staged).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        fs::write(&staged, bytes).unwrap();

        assert!(maintenance(&mut store, &paths).is_err());
        assert_eq!(fs::read(paths.settings_path()).unwrap(), OLD_SETTINGS);
        assert!(!paths.config_root().join(NEW_THEME).exists());
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::NeedsAttention
        );
        assert!(staged.exists(), "待人工处理的材料必须保留");
    }

    #[test]
    fn interrupted_apply_without_recovery_point_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();
        store
            .transition_maintenance_request(
                &request.request_id,
                MaintenanceRequestStatus::Prepared,
                MaintenanceRequestStatus::Applying,
            )
            .unwrap();

        assert!(maintenance(&mut store, &paths).is_err());
        assert_eq!(fs::read(paths.settings_path()).unwrap(), OLD_SETTINGS);
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::NeedsAttention
        );
    }

    #[test]
    fn maintenance_restore_leaves_credentials_and_unlisted_files_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let (paths, mut store) = setup(&temp);
        let credentials = paths.config_root().join("credentials-guard.json");
        let sessions = paths.local_root().join("sessions.db");
        fs::write(&credentials, b"placeholder").unwrap();
        fs::write(&sessions, b"placeholder").unwrap();
        let request = stage_restore(&mut store, &paths, &archive(), selection()).unwrap();

        assert_eq!(
            maintenance(&mut store, &paths).unwrap(),
            MaintenanceOutcome::Ready
        );
        assert_eq!(fs::read(&credentials).unwrap(), b"placeholder");
        assert_eq!(fs::read(&sessions).unwrap(), b"placeholder");
        assert!(
            !fs::read_to_string(paths.settings_path())
                .unwrap()
                .contains("api_key")
        );
        assert_eq!(
            store
                .maintenance_request(&request.request_id)
                .unwrap()
                .unwrap()
                .status,
            MaintenanceRequestStatus::Applied
        );
    }
}
