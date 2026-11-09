use std::{str::FromStr, time::Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use slterm_workspace::{
    CloseIntent, CloseScope, InvalidViewState, PanelGroup, PanelGroupId, ProjectId,
    ProjectSnapshot, RunIntent, SideViewId, TerminalId, TerminalRecord, ViewKeyKind,
    ViewStateRegistry, ViewStateSchema,
};
use tempfile::TempDir;

use super::*;
use crate::persistence::close::{
    CloseCommitError, CloseCommitStatus, CloseStoreError, PersistedCloseIntent,
};
use crate::persistence::metadata::RunMarker;
use crate::persistence::workspace::WorkspaceDatabase;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistentState {
    position: u64,
}

fn decode_persistent_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
    let state: PersistentState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
    serde_json::to_value(state).map_err(|_| InvalidViewState)
}

fn registry() -> ViewStateRegistry {
    ViewStateRegistry::new(
        &[ViewKeyKind::SideView, ViewKeyKind::AppSettingsOverlay]
            .map(|kind| ViewStateSchema::new(kind, decode_persistent_state)),
    )
    .unwrap()
}

pub(super) fn fixture(temp: &TempDir) -> AppPaths {
    AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
}

fn key_for(side_view_id: SideViewId) -> ViewKey {
    ViewKey::SideView { side_view_id }
}

pub(super) fn key() -> ViewKey {
    key_for(SideViewId::Nav)
}

fn state_for(key: &ViewKey, position: u64) -> ViewState {
    ViewState::new(key, json!({"position": position}), &registry()).unwrap()
}

pub(super) fn state(position: u64) -> ViewState {
    state_for(&key(), position)
}

pub(super) fn project_snapshot(name: &str) -> ProjectSnapshot {
    let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let mut registry = slterm_workspace::ProjectRegistry::new();
    registry
        .add_project(
            project_id,
            slterm_path::CanonicalPath::new("D:/work/a").unwrap(),
        )
        .unwrap();
    registry.rename_project(project_id, name).unwrap();
    ProjectSnapshot::restore(registry, Vec::new()).unwrap()
}

pub(super) fn close_registration(close_id: &str) -> PersistedCloseIntent {
    PersistedCloseIntent {
        close_id: close_id.to_owned(),
        scope: CloseScope::ApplicationExit,
        store_epoch: "epoch-writer-close".to_owned(),
        intent: CloseIntent::new(Vec::new(), [9; 32]).unwrap(),
        commit_status: CloseCommitStatus::Collected,
    }
}

pub(super) fn begin_close_run(store: &mut WorkspaceDatabase) {
    store
        .begin_run(
            "epoch-writer-close",
            RunMarker {
                run_id: "run-writer-close".to_owned(),
                process_id: 23,
                process_created_at_ms: 789,
            },
        )
        .unwrap();
}

fn terminal_project_snapshot() -> ProjectSnapshot {
    let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let terminal_id = TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a").unwrap();
    let mut registry = project_snapshot("terminal").registry().clone();
    assert_eq!(registry.allocate_terminal_number(project_id), Ok(0));
    let view = ViewKey::Terminal {
        project_id,
        terminal_id,
    };
    registry
        .layout_mut(project_id)
        .unwrap()
        .open_first_group(
            PanelGroupId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a").unwrap(),
            PanelGroup::new(vec![view.clone()], view).unwrap(),
        )
        .unwrap();
    let record = TerminalRecord::new(
        terminal_id,
        project_id,
        0,
        slterm_path::CanonicalPath::new("D:/work/terminal").unwrap(),
        slterm_path::CanonicalPath::new("C:/shell/pwsh.exe").unwrap(),
        Some("构建".to_owned()),
        RunIntent::Exited,
    )
    .unwrap();
    ProjectSnapshot::restore(registry, vec![record]).unwrap()
}

fn wait_for_project_name(store: &mut WorkspaceDatabase, name: &str) -> Option<ProjectSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Ok(snapshot) = store.load_project_snapshot()
            && snapshot
                .registry()
                .projects()
                .first()
                .is_some_and(|project| project.display_name() == name)
        {
            return Some(snapshot);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_position_for(store: &WorkspaceDatabase, key: &ViewKey) -> Option<(u64, u64)> {
    store.view_state(key, &registry()).unwrap().map(|stored| {
        let position = stored.state.state()["position"].as_u64().unwrap();
        (position, stored.revision)
    })
}

fn read_position(store: &WorkspaceDatabase) -> Option<(u64, u64)> {
    read_position_for(store, &key())
}

fn wait_for_position_for(store: &WorkspaceDatabase, key: &ViewKey) -> Option<(u64, u64)> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let current = read_position_for(store, key);
        if current.is_some() || Instant::now() >= deadline {
            return current;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_position(store: &WorkspaceDatabase) -> Option<(u64, u64)> {
    wait_for_position_for(store, &key())
}

fn wait_for_position_value(store: &WorkspaceDatabase, position: u64) -> Option<(u64, u64)> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let current = read_position(store);
        if current.is_some_and(|(current, _)| current == position) {
            return current;
        }
        if Instant::now() >= deadline {
            return current;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_sidebar_state(store: &WorkspaceDatabase) -> Option<SidebarState> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let current = store
            .sidebar_state()
            .unwrap()
            .map(|recovery| recovery.state);
        if current.is_some() || Instant::now() >= deadline {
            return current;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_failure(writer: &WorkspaceWriter) -> Option<ViewStateWriteFailure> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match writer.try_recv_failure() {
            Some(WorkspaceWriteFailure::ViewState(failure)) => return Some(failure),
            Some(WorkspaceWriteFailure::ProjectSnapshot(_)) => {
                panic!("ViewState 测试不得收到项目聚合失败")
            }
            Some(WorkspaceWriteFailure::SidebarState(_)) => {
                panic!("ViewState 测试不得收到侧栏组织状态失败")
            }
            None if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            None => return None,
        }
    }
}

/// 最终冲刷诊断同时覆盖 ViewState 与项目聚合，不误报为单一状态类型。
#[test]
fn workspace_writer_flush_failure_display_covers_workspace_state() {
    assert_eq!(
        WorkspaceWriterError::FlushFailed.to_string(),
        "工作区状态最终冲刷失败"
    );
}

/// 每个 ViewKey 只写最新状态，且在 500 毫秒去抖到期前不落盘。
#[test]
fn workspace_writer_debounces_view_state_to_latest() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();

    assert_eq!(VIEW_STATE_DEBOUNCE, Duration::from_millis(500));
    for position in 1..=3 {
        writer.save_view_state(&key(), &state(position)).unwrap();
    }
    assert_eq!(read_position(&observer), None, "去抖到期前不得写盘");

    assert_eq!(wait_for_position(&observer), Some((3, 0)));
    assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
    writer.shutdown().unwrap();
}

/// 侧栏组织状态只写最新版本，且在 500 毫秒去抖到期前不落盘。
#[test]
fn workspace_writer_debounces_sidebar_state_to_latest() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();

    assert_eq!(SIDEBAR_STATE_DEBOUNCE, Duration::from_millis(500));
    for width in [260, 300, 333] {
        let mut state = SidebarState::default();
        state.set_width(width);
        client.save_sidebar_state(&state).unwrap();
    }
    assert!(
        observer.sidebar_state().unwrap().is_none(),
        "侧栏组织状态去抖到期前不得写盘"
    );

    let restored = wait_for_sidebar_state(&observer).expect("去抖到期后必须写盘");
    assert_eq!(restored.width(), 333);
    assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
    writer.shutdown().unwrap();
}

/// 侧栏写失败保留最新副本，后续编辑与 shutdown 均可重试。
#[test]
fn sidebar_state_save_failure_retains_latest() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer
        .save_sidebar_state(&SidebarState::default())
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    observer
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_sidebar_write BEFORE UPDATE ON app_state \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();

    let mut changed = SidebarState::default();
    changed.set_width(333);
    client.save_sidebar_state(&changed).unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 0,
            failed: 1,
        }
    );
    assert!(matches!(
        writer.try_recv_failure(),
        Some(WorkspaceWriteFailure::SidebarState(
            SidebarStateStoreError::Database(_)
        ))
    ));
    assert_eq!(
        observer.sidebar_state().unwrap().unwrap().state.width(),
        250
    );

    let mut latest = SidebarState::default();
    latest.set_width(344);
    client.save_sidebar_state(&latest).unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 0,
            failed: 1,
        }
    );
    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_sidebar_write")
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(
        observer.sidebar_state().unwrap().unwrap().state.width(),
        344
    );
}

/// UI client 与外部生命周期 owner 共用同一有界命令队列。
#[test]
fn workspace_writer_client_shares_command_channel() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();

    client.save_view_state(&key(), &state(7)).unwrap();

    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((7, 0)));
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_commit_returns_committed_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let snapshot = project_snapshot("close-committed");
    let collected = close_registration("close-writer");

    let response = client.commit_close_intent(&snapshot, &collected).unwrap();
    let committed = response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();

    assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
    assert_eq!(observer.load_project_snapshot().unwrap(), snapshot);
    assert_eq!(
        observer.load_close_intent("close-writer").unwrap(),
        Some(committed)
    );
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_commit_supersedes_pending_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let closed = project_snapshot("closed-after-pending");
    client
        .save_project_snapshot(&project_snapshot("old-pending"))
        .unwrap();
    let response = client
        .commit_close_intent(&closed, &close_registration("close-pending"))
        .unwrap();
    response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();

    writer.flush().unwrap();

    assert_eq!(observer.load_project_snapshot().unwrap(), closed);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_commit_supersedes_final_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let closed = project_snapshot("closed-after-final");
    client
        .save_final_project_snapshot(&project_snapshot("old-final"))
        .unwrap();
    let response = client
        .commit_close_intent(&closed, &close_registration("close-final"))
        .unwrap();
    response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();

    writer.flush().unwrap();

    assert_eq!(observer.load_project_snapshot().unwrap(), closed);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_failed_close_commit_preserves_pending_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let conflicting = close_registration("close-conflict");
    observer.register_close_intent(&conflicting).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let pending = project_snapshot("pending-after-conflict");
    client.save_project_snapshot(&pending).unwrap();
    let response = client
        .commit_close_intent(&project_snapshot("closed-conflict"), &conflicting)
        .unwrap();

    assert!(matches!(
        response.recv_timeout(Duration::from_secs(3)).unwrap(),
        Err(CloseCommitError::Close(CloseStoreError::CloseIdConflict))
    ));
    writer.flush().unwrap();
    assert_eq!(observer.load_project_snapshot().unwrap(), pending);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_failed_close_commit_preserves_final_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let conflicting = close_registration("close-final-conflict");
    observer.register_close_intent(&conflicting).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let final_snapshot = project_snapshot("final-after-conflict");
    client.save_final_project_snapshot(&final_snapshot).unwrap();
    let response = client
        .commit_close_intent(&project_snapshot("closed-final-conflict"), &conflicting)
        .unwrap();

    assert!(matches!(
        response.recv_timeout(Duration::from_secs(3)).unwrap(),
        Err(CloseCommitError::Close(CloseStoreError::CloseIdConflict))
    ));
    writer.flush().unwrap();
    assert_eq!(observer.load_project_snapshot().unwrap(), final_snapshot);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_commit_queue_full_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    observer
        .database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();
    client.save_view_state(&key(), &state(1)).unwrap();
    std::thread::sleep(VIEW_STATE_DEBOUNCE + Duration::from_millis(100));
    let snapshot = project_snapshot("queued-close");
    let mut responses = Vec::new();
    for index in 0..COMMAND_CAPACITY {
        responses.push(
            client
                .commit_close_intent(
                    &snapshot,
                    &close_registration(&format!("close-queued-{index}")),
                )
                .unwrap(),
        );
    }

    assert!(matches!(
        client.commit_close_intent(&snapshot, &close_registration("close-overflow")),
        Err(WorkspaceWriterError::QueueFull)
    ));

    observer
        .database
        .connection
        .execute_batch("ROLLBACK")
        .unwrap();
    writer.shutdown().unwrap();
    for response in responses {
        assert_eq!(
            response.recv().unwrap().unwrap().commit_status,
            CloseCommitStatus::StateCommitted
        );
    }
}

#[test]
fn workspace_writer_close_commit_disconnect_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    writer.shutdown().unwrap();

    assert!(matches!(
        client.commit_close_intent(
            &project_snapshot("disconnected-close"),
            &close_registration("close-disconnected"),
        ),
        Err(WorkspaceWriterError::Stopped)
    ));
}

#[test]
fn operation_query_queue_full_and_disconnect_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    observer
        .database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();
    client.save_view_state(&key(), &state(1)).unwrap();
    std::thread::sleep(VIEW_STATE_DEBOUNCE + Duration::from_millis(100));
    let responses = (0..COMMAND_CAPACITY)
        .map(|_| client.query_operation("missing").unwrap())
        .collect::<Vec<_>>();

    assert!(matches!(
        client.query_operation("overflow"),
        Err(WorkspaceWriterError::QueueFull)
    ));

    observer
        .database
        .connection
        .execute_batch("ROLLBACK")
        .unwrap();
    drop(responses);
    writer.shutdown().unwrap();
    assert!(matches!(
        client.query_operation("disconnected"),
        Err(WorkspaceWriterError::Stopped)
    ));
}

/// 项目聚合只保留最新快照，并在 2 秒去抖后一次落盘。
#[test]
fn workspace_writer_flushes_terminal_metadata_with_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let expected = terminal_project_snapshot();

    writer.client().save_project_snapshot(&expected).unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(observer.load_project_snapshot().unwrap(), expected);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_debounces_project_snapshot_to_latest() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();

    assert_eq!(PROJECT_SNAPSHOT_DEBOUNCE, Duration::from_secs(2));
    for name in ["first", "second", "third"] {
        client
            .save_project_snapshot(&project_snapshot(name))
            .unwrap();
    }
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        ProjectSnapshot::default(),
        "项目聚合去抖到期前不得写盘"
    );

    assert_eq!(
        wait_for_project_name(&mut observer, "third"),
        Some(project_snapshot("third"))
    );
    assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
    writer.shutdown().unwrap();
}

/// 项目聚合写失败保留最新副本，后续提交重试，shutdown 最终冲刷。
#[test]
fn project_snapshot_save_failure_retains_latest_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer
        .save_project_snapshot(&project_snapshot("original"))
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    observer
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_project_write BEFORE UPDATE ON projects \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();

    writer
        .save_project_snapshot(&project_snapshot("changed"))
        .unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 0,
            failed: 1,
        }
    );
    assert!(matches!(
        writer.try_recv_failure(),
        Some(WorkspaceWriteFailure::ProjectSnapshot(
            ProjectStoreError::Database(_)
        ))
    ));
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("original")
    );

    writer
        .save_project_snapshot(&project_snapshot("latest"))
        .unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 0,
            failed: 1,
        }
    );
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("original")
    );

    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_project_write")
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("latest")
    );
}

/// 项目快照提交遇到有界队列满时同步拒收，排水后可重试最新副本。
#[test]
fn project_snapshot_queue_full_allows_later_retry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer
        .save_project_snapshot(&project_snapshot("original"))
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    observer
        .database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();
    client
        .save_project_snapshot(&project_snapshot("blocked"))
        .unwrap();
    std::thread::sleep(PROJECT_SNAPSHOT_DEBOUNCE + Duration::from_millis(100));

    for index in 0..COMMAND_CAPACITY {
        client
            .save_project_snapshot(&project_snapshot(&format!("queued-{index}")))
            .unwrap();
    }
    assert!(matches!(
        client.save_project_snapshot(&project_snapshot("overflow")),
        Err(WorkspaceWriterError::QueueFull)
    ));

    observer
        .database
        .connection
        .execute_batch("ROLLBACK")
        .unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot(&format!("queued-{}", COMMAND_CAPACITY - 1))
    );

    client
        .save_project_snapshot(&project_snapshot("after-drain"))
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("after-drain")
    );
}

/// 普通队列未接收的项目终态仍经独立通道在退出时提交。
#[test]
fn final_project_snapshot_is_submitted_at_shutdown() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer
        .save_project_snapshot(&project_snapshot("original"))
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();

    client
        .save_final_project_snapshot(&project_snapshot("final"))
        .unwrap();
    writer.shutdown().unwrap();

    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("final")
    );
}

/// 命令发送端全部断开时，写线程仍在退出前冲刷未到期项目快照。
#[test]
fn project_snapshot_disconnect_flushes_pending_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (failures, _failure_receiver) = mpsc::sync_channel(1);
    let (startup, started) = mpsc::sync_channel(1);
    let (_final_views, final_view_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (_final_sidebar, final_sidebar_receiver) = mpsc::sync_channel(1);
    let (_final_project, final_project_receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        run(
            paths,
            receiver,
            failures,
            startup,
            FinalStateReceivers {
                view_states: final_view_receiver,
                sidebar_state: final_sidebar_receiver,
                project_snapshot: final_project_receiver,
            },
        )
    });
    started.recv().unwrap().unwrap();
    commands
        .send(Command::SaveProjectSnapshot {
            snapshot: Box::new(project_snapshot("disconnected")),
        })
        .unwrap();

    drop(commands);
    worker.join().unwrap();

    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("disconnected")
    );
}

/// 项目失败不撤销 ViewState 的独立定时唤醒，混合冲刷分别计数。
#[test]
fn project_failure_preserves_view_state_schedule_and_mixed_report() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_view_state(&key(), &state(7)).unwrap();
    observer
        .save_project_snapshot(&project_snapshot("original"))
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    observer
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_project_write BEFORE UPDATE ON projects \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();
    writer.save_view_state(&key(), &state(8)).unwrap();
    writer
        .save_project_snapshot(&project_snapshot("changed"))
        .unwrap();

    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 1,
        }
    );
    assert_eq!(read_position(&observer), Some((8, 1)));
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("original")
    );
    assert!(matches!(
        writer.try_recv_failure(),
        Some(WorkspaceWriteFailure::ProjectSnapshot(
            ProjectStoreError::Database(_)
        ))
    ));

    writer.save_view_state(&key(), &state(9)).unwrap();
    assert_eq!(wait_for_position_value(&observer, 9), Some((9, 2)));
    assert!(writer.try_recv_failure().is_none());
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("original")
    );

    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_project_write")
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((9, 2)));
    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        project_snapshot("changed")
    );
}

/// 转发器逐条发出通知，并在失败通道关闭后退出。
#[test]
fn workspace_failure_forwarder_relays_and_stops() {
    let (failure_tx, failure_rx) = mpsc::sync_channel(2);
    let (event_tx, event_rx) = mpsc::channel();
    let forwarder = spawn_workspace_failure_forwarder(failure_rx, move || {
        let _ = event_tx.send(());
    })
    .unwrap();

    let failures = [
        WorkspaceWriteFailure::ViewState(ViewStateWriteFailure {
            key: key(),
            error: ViewStateStoreError::CorruptRecord,
        }),
        WorkspaceWriteFailure::ProjectSnapshot(ProjectStoreError::CorruptRecord),
    ];
    for failure in failures {
        failure_tx.send(failure).unwrap();
        event_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    }
    drop(failure_tx);

    assert!(event_rx.try_recv().is_err());
    forwarder.join().unwrap();
}

/// 失败接收端可移交壳事件转发器，writer owner 仍负责退出冲刷。
#[test]
fn workspace_writer_failure_receiver_is_transferable() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_view_state(&key(), &state(7)).unwrap();
    let mut writer = WorkspaceWriter::start(paths).unwrap();
    observer
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();
    let failures = writer.take_failure_receiver().unwrap();
    assert!(writer.try_recv_failure().is_none());

    writer.save_view_state(&key(), &state(8)).unwrap();

    let failure = failures.recv_timeout(Duration::from_secs(3)).unwrap();
    let WorkspaceWriteFailure::ViewState(failure) = failure else {
        panic!("ViewState 测试不得收到项目聚合失败");
    };
    assert_eq!(failure.key(), &key());
    assert!(matches!(failure.error(), ViewStateStoreError::Database(_)));
    drop(failures);
    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_view_state_write")
        .unwrap();
    writer.shutdown().unwrap();
    assert_eq!(read_position(&observer), Some((8, 1)));
}

/// 写失败保留最新待写副本；后续修改重试，shutdown 在退出前再次冲刷。
#[test]
fn view_state_save_failure_retains_memory() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_view_state(&key(), &state(7)).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    observer
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        )
        .unwrap();

    writer.save_view_state(&key(), &state(8)).unwrap();
    let failure = wait_for_failure(&writer);
    assert!(failure.is_some(), "后台写失败必须产生非阻塞通知");
    let failure = failure.unwrap();
    assert_eq!(failure.key(), &key());
    assert!(matches!(failure.error(), ViewStateStoreError::Database(_)));
    assert_eq!(read_position(&observer), Some((7, 0)));

    writer.save_view_state(&key(), &state(9)).unwrap();
    assert!(
        wait_for_failure(&writer).is_some(),
        "后续修改必须重试仍待写的最新状态"
    );
    assert_eq!(read_position(&observer), Some((7, 0)));
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 0,
            failed: 1,
        }
    );
    assert_eq!(read_position(&observer), Some((7, 0)));

    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_view_state_write")
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((9, 1)));
}

/// 键与 DTO 类型不匹配时同步拒绝，不能进入后台待写队列。
#[test]
fn workspace_writer_rejects_mismatched_state_before_queueing() {
    let temp = tempfile::tempdir().unwrap();
    let writer = WorkspaceWriter::start(fixture(&temp)).unwrap();
    let mismatched = state_for(&ViewKey::AppSettingsOverlay, 1);

    assert!(matches!(
        writer.save_view_state(&key(), &mismatched),
        Err(WorkspaceWriterError::ViewState(ViewStateError::KeyMismatch))
    ));
    assert_eq!(writer.flush().unwrap(), WorkspaceFlushReport::default());
    writer.shutdown().unwrap();
}

/// 数据库在写线程内打开失败时，启动方收到分类错误且线程被回收。
#[test]
fn workspace_writer_start_reports_open_failure() {
    let temp = tempfile::tempdir().unwrap();
    let local = temp.path().join("local");
    std::fs::write(&local, "not a directory").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), local).unwrap();

    assert!(matches!(
        WorkspaceWriter::start(paths),
        Err(WorkspaceWriterError::Open(DatabaseError::Io(_)))
    ));
}

/// 有界队列满时非阻塞拒收；调用方可保留原状态并在排水后重试。
#[test]
fn workspace_writer_queue_full_allows_later_retry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    observer.save_view_state(&key(), &state(1)).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    observer
        .database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();
    writer.save_view_state(&key(), &state(2)).unwrap();
    std::thread::sleep(VIEW_STATE_DEBOUNCE + Duration::from_millis(100));

    for position in 3..3 + COMMAND_CAPACITY as u64 {
        writer.save_view_state(&key(), &state(position)).unwrap();
    }
    assert!(matches!(
        writer.save_view_state(&key(), &state(999)),
        Err(WorkspaceWriterError::QueueFull)
    ));

    observer
        .database
        .connection
        .execute_batch("ROLLBACK")
        .unwrap();
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((66, 2)));
    assert!(writer.try_recv_failure().is_none());

    writer.save_view_state(&key(), &state(999)).unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((999, 3)));
}

/// 单键写失败保留自身待写副本，不阻塞或驱逐其他 ViewKey 的保存。
#[test]
fn workspace_writer_failure_isolated_to_one_view_key() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let explorer = key_for(SideViewId::Explorer);
    observer.save_view_state(&key(), &state(7)).unwrap();
    observer
        .save_view_state(&explorer, &state_for(&explorer, 20))
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let trigger = format!(
        "CREATE TRIGGER reject_nav_write BEFORE UPDATE ON view_states \
         WHEN OLD.view_key = '{}' \
         BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
        key().to_canonical_json().unwrap()
    );
    observer
        .database
        .connection
        .execute_batch(&trigger)
        .unwrap();

    writer.save_view_state(&key(), &state(8)).unwrap();
    writer
        .save_view_state(&explorer, &state_for(&explorer, 21))
        .unwrap();
    let failure = wait_for_failure(&writer);
    assert!(failure.is_some(), "失败视图必须产生通知");
    assert_eq!(failure.unwrap().key(), &key());
    assert_eq!(wait_for_position_for(&observer, &explorer), Some((21, 1)));
    assert_eq!(read_position(&observer), Some((7, 0)));

    observer
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_nav_write")
        .unwrap();
    assert_eq!(
        writer.shutdown().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        }
    );
    assert_eq!(read_position(&observer), Some((8, 1)));
    assert_eq!(read_position_for(&observer, &explorer), Some((21, 1)));
}

/// 未显式 shutdown 时，Drop 仍先冲刷待写状态并回收线程。
#[test]
fn workspace_writer_drop_flushes_pending_state() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    writer.save_view_state(&key(), &state(11)).unwrap();

    drop(writer);

    assert_eq!(read_position(&observer), Some((11, 0)));
}
