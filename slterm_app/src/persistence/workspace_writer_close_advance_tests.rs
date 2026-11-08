use std::sync::Arc;

use serde_json::{Value, json};
use slterm_path::CanonicalPath;
use slterm_sidebar::side_view_state_schema;
use slterm_workspace::{
    CloseIntent, CloseScope, InvalidViewState, PanelGroup, PanelGroupId, ProjectId,
    ProjectRegistry, RunIntent, SideViewId, TerminalId, TerminalRecord, ViewKeyKind,
    ViewStateRegistry, ViewStateSchema,
};

use super::workspace_writer_tests::{
    begin_close_run, close_registration, fixture, key, project_snapshot, state,
};
use super::*;
use crate::persistence::close::ProjectCleanupError;

fn cleanup_project(
    client: &WorkspaceWriterClient,
    current: &PersistedCloseIntent,
) -> Result<PersistedCloseIntent, ProjectCleanupError> {
    client
        .cleanup_project_close(current, Arc::new(project_cleanup_registry()))
        .unwrap()
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
}

fn committed(
    client: &WorkspaceWriterClient,
    close_id: &str,
    snapshot_name: &str,
) -> PersistedCloseIntent {
    let response = client
        .commit_close_intent(
            &project_snapshot(snapshot_name),
            &close_registration(close_id),
        )
        .unwrap();
    let committed = response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
    committed
}

fn advance(
    client: &WorkspaceWriterClient,
    current: &PersistedCloseIntent,
    next: CloseCommitStatus,
) -> Result<PersistedCloseIntent, CloseStoreError> {
    client
        .advance_close_intent(current, next)
        .unwrap()
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
}

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2c";

fn project_id(text: &str) -> ProjectId {
    text.parse().unwrap()
}

fn project_close_snapshot() -> ProjectSnapshot {
    let mut registry = ProjectRegistry::new();
    let mut records = Vec::new();
    for (project, group, terminal) in [
        (PROJECT_A, GROUP_A, TERMINAL_A),
        (PROJECT_B, GROUP_B, TERMINAL_B),
    ] {
        let project_id = project_id(project);
        let root = CanonicalPath::new(format!("D:/work/writer-{project}")).unwrap();
        registry.add_project(project_id, root.clone()).unwrap();
        let view = ViewKey::Terminal {
            project_id,
            terminal_id: terminal.parse::<TerminalId>().unwrap(),
        };
        registry
            .layout_mut(project_id)
            .unwrap()
            .open_first_group(
                group.parse::<PanelGroupId>().unwrap(),
                PanelGroup::new(vec![view.clone()], view).unwrap(),
            )
            .unwrap();
        let number = registry.allocate_terminal_number(project_id).unwrap();
        records.push(
            TerminalRecord::new(
                terminal.parse().unwrap(),
                project_id,
                number,
                root,
                CanonicalPath::new("C:/shell/pwsh.exe").unwrap(),
                None,
                RunIntent::Exited,
            )
            .unwrap(),
        );
    }
    registry.activate_project(project_id(PROJECT_A)).unwrap();
    ProjectSnapshot::restore(registry, records).unwrap()
}

fn project_close_registration(close_id: &str) -> PersistedCloseIntent {
    PersistedCloseIntent {
        close_id: close_id.to_owned(),
        scope: CloseScope::Project {
            project_id: project_id(PROJECT_A),
        },
        store_epoch: "epoch-writer-close".to_owned(),
        intent: CloseIntent::new(Vec::new(), [9; 32]).unwrap(),
        commit_status: CloseCommitStatus::Collected,
    }
}

fn decode_settings_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
    let state: Value = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
    if state["position"].as_u64().is_some() {
        Ok(state)
    } else {
        Err(InvalidViewState)
    }
}

fn project_cleanup_registry() -> ViewStateRegistry {
    ViewStateRegistry::new(&[
        side_view_state_schema(),
        ViewStateSchema::new(ViewKeyKind::Settings, decode_settings_state),
    ])
    .unwrap()
}

fn settings_key(project: &str) -> ViewKey {
    ViewKey::Settings {
        project_id: project_id(project),
    }
}

fn settings_state(project: &str, position: u64) -> ViewState {
    ViewState::new(
        &settings_key(project),
        json!({"position": position}),
        &project_cleanup_registry(),
    )
    .unwrap()
}

fn side_view_key(side_view_id: SideViewId) -> ViewKey {
    ViewKey::SideView { side_view_id }
}

fn nav_state(scroll: f64) -> ViewState {
    ViewState::new(
        &side_view_key(SideViewId::Nav),
        json!({
            "projects": [
                {"project_id": PROJECT_A, "expanded": true, "history_expanded": false},
                {"project_id": PROJECT_B, "expanded": false, "history_expanded": true}
            ],
            "unassigned_expanded": true,
            "scroll": scroll
        }),
        &project_cleanup_registry(),
    )
    .unwrap()
}

fn explorer_state() -> ViewState {
    ViewState::new(
        &side_view_key(SideViewId::Explorer),
        json!({
            "projects": [
                {
                    "project_id": PROJECT_A,
                    "expanded": ["src"],
                    "current_row": "src/main.rs",
                    "scroll": 24.0,
                    "show_ignored": true
                },
                {
                    "project_id": PROJECT_B,
                    "expanded": ["docs"],
                    "current_row": null,
                    "scroll": 48.0,
                    "show_ignored": false
                }
            ]
        }),
        &project_cleanup_registry(),
    )
    .unwrap()
}

fn commit_state(scroll: f64) -> ViewState {
    ViewState::new(
        &side_view_key(SideViewId::Commit),
        json!({
            "changes_collapsed": false,
            "unversioned_collapsed": true,
            "ignored_collapsed": false,
            "show_ignored": true,
            "scroll": scroll
        }),
        &project_cleanup_registry(),
    )
    .unwrap()
}

fn seed_project_close(observer: &mut WorkspaceDatabase) -> ProjectSnapshot {
    begin_close_run(observer);
    let original = project_close_snapshot();
    observer.save_project_snapshot(&original).unwrap();
    for (project, position) in [(PROJECT_A, 1), (PROJECT_B, 2)] {
        observer
            .save_view_state(&settings_key(project), &settings_state(project, position))
            .unwrap();
    }
    observer
        .save_view_state(&side_view_key(SideViewId::Nav), &nav_state(12.0))
        .unwrap();
    observer
        .save_view_state(&side_view_key(SideViewId::Explorer), &explorer_state())
        .unwrap();
    observer
        .save_view_state(&side_view_key(SideViewId::Commit), &commit_state(8.0))
        .unwrap();
    original
}

fn project_rows(store: &WorkspaceDatabase, table: &str, project: &str) -> i64 {
    store
        .database
        .connection
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1"),
            [project],
            |row| row.get(0),
        )
        .unwrap()
}

fn stopped_project_registration(close_id: &str) -> PersistedCloseIntent {
    PersistedCloseIntent {
        commit_status: CloseCommitStatus::PtyStopped,
        ..project_close_registration(close_id)
    }
}

#[test]
fn workspace_writer_close_advance_returns_each_committed_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let mut current = committed(&client, "close-advance", "close-advance");

    for next in [
        CloseCommitStatus::PtyStopped,
        CloseCommitStatus::CleanupCommitted,
        CloseCommitStatus::Completed,
    ] {
        current = advance(&client, &current, next).unwrap();
        assert_eq!(current.commit_status, next);
        assert_eq!(
            observer.load_close_intent("close-advance").unwrap(),
            Some(current.clone())
        );
    }
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_advance_rejects_conflicts_without_disk_changes() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let committed = committed(&client, "close-advance-conflicts", "close-conflicts");

    assert!(matches!(
        advance(&client, &committed, CloseCommitStatus::CleanupCommitted),
        Err(CloseStoreError::InvalidTransition)
    ));
    assert_eq!(
        observer
            .load_close_intent("close-advance-conflicts")
            .unwrap(),
        Some(committed.clone())
    );

    let mut stale_status = committed.clone();
    stale_status.commit_status = CloseCommitStatus::PtyStopped;
    assert!(matches!(
        advance(&client, &stale_status, CloseCommitStatus::CleanupCommitted),
        Err(CloseStoreError::CloseConflict)
    ));
    assert_eq!(
        observer
            .load_close_intent("close-advance-conflicts")
            .unwrap(),
        Some(committed.clone())
    );

    let mut stale_fingerprint = committed.clone();
    stale_fingerprint.intent = CloseIntent::new(Vec::new(), [8; 32]).unwrap();
    assert!(matches!(
        advance(&client, &stale_fingerprint, CloseCommitStatus::PtyStopped),
        Err(CloseStoreError::CloseConflict)
    ));
    assert_eq!(
        observer
            .load_close_intent("close-advance-conflicts")
            .unwrap(),
        Some(committed.clone())
    );

    observer
        .rotate_store_epoch(
            "run-writer-close",
            "epoch-writer-close",
            "epoch-writer-close-rotated",
        )
        .unwrap();
    assert!(matches!(
        advance(&client, &committed, CloseCommitStatus::PtyStopped),
        Err(CloseStoreError::StoreEpochConflict)
    ));
    assert_eq!(
        observer
            .load_close_intent("close-advance-conflicts")
            .unwrap(),
        Some(committed)
    );
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_advance_preserves_pending_project_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let committed_snapshot = project_snapshot("close-before-advance");
    let response = client
        .commit_close_intent(
            &committed_snapshot,
            &close_registration("close-advance-pending"),
        )
        .unwrap();
    let committed = response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    let pending = project_snapshot("pending-during-close-advance");
    client.save_project_snapshot(&pending).unwrap();

    advance(&client, &committed, CloseCommitStatus::PtyStopped).unwrap();

    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        committed_snapshot,
        "关闭头推进不得提前冲刷未到期的普通项目待写快照"
    );
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport {
            written: 1,
            failed: 0,
        },
        "关闭头推进不得清除普通项目待写快照"
    );
    assert_eq!(observer.load_project_snapshot().unwrap(), pending);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_close_advance_queue_full_channel_is_rejected() {
    let (commands, _command_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (final_states, _final_state_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (final_sidebar_state, _final_sidebar_receiver) = mpsc::sync_channel(1);
    let (final_project_snapshot, _final_project_receiver) = mpsc::sync_channel(1);
    let client = WorkspaceWriterClient {
        commands,
        final_states,
        final_sidebar_state,
        final_project_snapshot,
    };
    for position in 0..COMMAND_CAPACITY as u64 {
        client.save_view_state(&key(), &state(position)).unwrap();
    }
    let current = close_registration("close-advance-queue-full-channel");

    assert!(matches!(
        client.advance_close_intent(&current, CloseCommitStatus::StateCommitted),
        Err(WorkspaceWriterError::QueueFull)
    ));
}

#[test]
fn workspace_writer_close_advance_disconnect_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let committed = committed(
        &client,
        "close-advance-disconnected",
        "close-disconnected-advance",
    );
    writer.shutdown().unwrap();

    assert!(matches!(
        client.advance_close_intent(&committed, CloseCommitStatus::PtyStopped),
        Err(WorkspaceWriterError::Stopped)
    ));
}

#[test]
fn workspace_writer_close_advance_dropped_receipt_keeps_writer_alive() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    begin_close_run(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let committed = committed(
        &client,
        "close-advance-dropped-receipt",
        "close-dropped-receipt",
    );

    let response = client
        .advance_close_intent(&committed, CloseCommitStatus::PtyStopped)
        .unwrap();
    drop(response);
    writer.flush().unwrap();
    let stopped = observer
        .load_close_intent("close-advance-dropped-receipt")
        .unwrap()
        .unwrap();
    assert_eq!(stopped.commit_status, CloseCommitStatus::PtyStopped);

    assert_eq!(
        advance(&client, &stopped, CloseCommitStatus::CleanupCommitted)
            .unwrap()
            .commit_status,
        CloseCommitStatus::CleanupCommitted
    );
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_project_close_reconciles_all_pending_writes_with_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let original = seed_project_close(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();

    client.save_project_snapshot(&original).unwrap();
    client.save_final_project_snapshot(&original).unwrap();
    let response = client
        .commit_project_close_intent(
            &candidate,
            &project_close_registration("close-project-writer"),
        )
        .unwrap();
    let committed = response
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    writer.flush().unwrap();
    assert_eq!(project_rows(&observer, "projects", PROJECT_A), 1);
    assert_eq!(project_rows(&observer, "terminals", PROJECT_A), 1);

    let stopped = advance(&client, &committed, CloseCommitStatus::PtyStopped).unwrap();
    client.save_project_snapshot(&original).unwrap();
    client.save_final_project_snapshot(&original).unwrap();
    client
        .save_view_state(&settings_key(PROJECT_A), &settings_state(PROJECT_A, 9))
        .unwrap();
    client
        .save_view_state(&side_view_key(SideViewId::Nav), &nav_state(99.0))
        .unwrap();
    client
        .save_final_view_state(&side_view_key(SideViewId::Explorer), &explorer_state())
        .unwrap();
    client
        .save_view_state(&side_view_key(SideViewId::Commit), &commit_state(88.0))
        .unwrap();

    let cleaned = cleanup_project(&client, &stopped).unwrap();
    assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
    assert_eq!(project_rows(&observer, "projects", PROJECT_A), 0);
    writer.flush().unwrap();

    assert_eq!(project_rows(&observer, "projects", PROJECT_A), 0);
    assert_eq!(project_rows(&observer, "project_layouts", PROJECT_A), 0);
    assert_eq!(project_rows(&observer, "panel_groups", PROJECT_A), 0);
    assert_eq!(project_rows(&observer, "terminals", PROJECT_A), 0);
    assert_eq!(project_rows(&observer, "projects", PROJECT_B), 1);
    assert_eq!(project_rows(&observer, "terminals", PROJECT_B), 1);
    let registry = project_cleanup_registry();
    assert_eq!(
        observer
            .view_state(&settings_key(PROJECT_A), &registry)
            .unwrap(),
        None
    );
    assert!(
        observer
            .view_state(&settings_key(PROJECT_B), &registry)
            .unwrap()
            .is_some()
    );
    let nav = observer
        .view_state(&side_view_key(SideViewId::Nav), &registry)
        .unwrap()
        .unwrap();
    assert_eq!(nav.state.state()["projects"].as_array().unwrap().len(), 1);
    assert_eq!(
        nav.state.state()["projects"][0]["project_id"],
        json!(PROJECT_B)
    );
    assert_eq!(nav.state.state()["scroll"], json!(99.0));
    let explorer = observer
        .view_state(&side_view_key(SideViewId::Explorer), &registry)
        .unwrap()
        .unwrap();
    assert_eq!(
        explorer.state.state()["projects"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        explorer.state.state()["projects"][0]["project_id"],
        json!(PROJECT_B)
    );
    let commit = observer
        .view_state(&side_view_key(SideViewId::Commit), &registry)
        .unwrap()
        .unwrap();
    assert_eq!(commit.state.state()["scroll"], json!(88.0));

    let completed = advance(&client, &cleaned, CloseCommitStatus::Completed).unwrap();
    assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
    writer.shutdown().unwrap();
}

#[test]
fn workspace_writer_project_close_queue_full_is_rejected() {
    let (commands, _command_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (final_states, _final_state_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
    let (final_sidebar_state, _final_sidebar_receiver) = mpsc::sync_channel(1);
    let (final_project_snapshot, _final_project_receiver) = mpsc::sync_channel(1);
    let client = WorkspaceWriterClient {
        commands,
        final_states,
        final_sidebar_state,
        final_project_snapshot,
    };
    for position in 0..COMMAND_CAPACITY as u64 {
        client.save_view_state(&key(), &state(position)).unwrap();
    }

    assert!(matches!(
        client.commit_project_close_intent(
            &ProjectSnapshot::default(),
            &project_close_registration("close-project-queue-full"),
        ),
        Err(WorkspaceWriterError::QueueFull)
    ));
    assert!(matches!(
        client.cleanup_project_close(
            &stopped_project_registration("close-project-cleanup-queue-full"),
            Arc::new(project_cleanup_registry()),
        ),
        Err(WorkspaceWriterError::QueueFull)
    ));
}

#[test]
fn workspace_writer_project_close_disconnect_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let writer = WorkspaceWriter::start(fixture(&temp)).unwrap();
    let client = writer.client();
    writer.shutdown().unwrap();

    assert!(matches!(
        client.commit_project_close_intent(
            &ProjectSnapshot::default(),
            &project_close_registration("close-project-disconnected"),
        ),
        Err(WorkspaceWriterError::Stopped)
    ));
    assert!(matches!(
        client.cleanup_project_close(
            &stopped_project_registration("close-project-cleanup-disconnected"),
            Arc::new(project_cleanup_registry()),
        ),
        Err(WorkspaceWriterError::Stopped)
    ));
}

#[test]
fn workspace_writer_project_close_dropped_receipts_keep_writer_alive() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let original = seed_project_close(&mut observer);
    let writer = WorkspaceWriter::start(paths).unwrap();
    let client = writer.client();
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();
    let close_id = "close-project-dropped-receipts";

    let response = client
        .commit_project_close_intent(&candidate, &project_close_registration(close_id))
        .unwrap();
    drop(response);
    writer.flush().unwrap();
    let committed = observer.load_close_intent(close_id).unwrap().unwrap();
    assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
    assert_eq!(project_rows(&observer, "projects", PROJECT_A), 1);

    let stopped = advance(&client, &committed, CloseCommitStatus::PtyStopped).unwrap();
    let response = client
        .cleanup_project_close(&stopped, Arc::new(project_cleanup_registry()))
        .unwrap();
    drop(response);
    writer.flush().unwrap();
    let cleaned = observer.load_close_intent(close_id).unwrap().unwrap();
    assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
    assert_eq!(project_rows(&observer, "projects", PROJECT_A), 0);
    assert_eq!(
        advance(&client, &cleaned, CloseCommitStatus::Completed)
            .unwrap()
            .commit_status,
        CloseCommitStatus::Completed
    );
    writer.shutdown().unwrap();
}
