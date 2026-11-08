use slterm_workspace::CloseIntent;

use super::workspace_writer_tests::{
    begin_close_run, close_registration, fixture, key, project_snapshot, state,
};
use super::*;

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
