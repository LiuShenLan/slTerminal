//! 启动维护期 hook Journal 恢复的接线测试。

use std::sync::mpsc;

use serde_json::json;
use slterm_path::AppPaths;

use super::{ControlPlaneCleanup, WorkspaceWriterRuntime};
use crate::persistence::journal::{JournalRegistration, JournalStepRegistration};
use crate::persistence::workspace::WorkspaceDatabase;

/// hook 跨文件 Journal 在 writer 取得所有权前由维护期恢复，不阻断启动。
#[test]
fn hooks_journal_recovery_runs_before_writer_ownership() {
    use sha2::{Digest as _, Sha256};

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let files = tempfile::tempdir().unwrap();
    let hooks = files.path().join("hooks.json");
    let marker = files.path().join("marker.json");
    std::fs::write(&hooks, b"old-hooks").unwrap();
    std::fs::write(&marker, b"marker").unwrap();
    let fingerprint = |bytes: &[u8]| {
        let mut value = [0; 32];
        value.copy_from_slice(&Sha256::digest(bytes));
        value
    };
    let hex = |bytes: &[u8]| {
        fingerprint(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let journal_id = "hooks-maintenance";
    let backup_root = paths
        .local_root()
        .join(crate::ai_hook::HOOK_JOURNAL_DIR)
        .join(journal_id);
    std::fs::create_dir_all(&backup_root).unwrap();
    std::fs::write(backup_root.join("0.before"), b"old-hooks").unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    super::shell_event_tests::seed_maintenance_run(&mut store);
    store
        .register_journal(JournalRegistration {
            operation_id: None,
            journal_id: journal_id.into(),
            kind: crate::ai_hook::HOOKS_INSTALL_KIND.into(),
            store_epoch: "epoch-maintenance".into(),
            before_fingerprint: Some([1; 32]),
            steps: vec![
                JournalStepRegistration {
                    step_key: "hooks".into(),
                    kind: "file".into(),
                    target: json!({
                        "version": 1,
                        "path": hooks.to_str().unwrap(),
                        "backup": "0.before",
                        "before_absent": false,
                        "after_absent": false,
                        "expected_after": hex(b"new-hooks"),
                    }),
                    authorization_generation: 0,
                    before_fingerprint: Some(fingerprint(b"old-hooks")),
                },
                JournalStepRegistration {
                    step_key: "marker".into(),
                    kind: "file".into(),
                    target: json!({
                        "version": 1,
                        "path": marker.to_str().unwrap(),
                        "backup": "1.before",
                        "before_absent": true,
                        "after_absent": false,
                        "expected_after": hex(b"marker"),
                    }),
                    authorization_generation: 0,
                    before_fingerprint: None,
                },
            ],
        })
        .unwrap();
    drop(store);
    let (events, _event_rx) = mpsc::sync_channel(super::shell_events::SHELL_EVENT_CAPACITY);

    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();

    assert_eq!(std::fs::read(&hooks).unwrap(), b"old-hooks");
    assert!(!marker.exists(), "混合状态必须回滚到安装前");
    runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
}
