//! 视图可见性任务与显式操作结果的所有权测试。

use super::super::task_service::{TaskService, ViewRefreshOutcome, ViewTaskOwner};
use super::*;
use crate::persistence::operation::OperationRegistration;
use slterm_runtime_protocol::OperationStatus;

#[test]
fn visibility_coalesces_refresh_and_drops_stale() {
    let owner = ViewTaskOwner::SideView(SideViewId::Nav);
    let mut tasks = TaskService::new();
    let first = tasks.request_view_refresh(owner).unwrap();
    assert!(
        tasks.request_view_refresh(owner).is_none(),
        "在途刷新只记合并位"
    );

    assert!(tasks.set_view_visible(owner, false).is_none());
    assert!(tasks.request_view_refresh(owner).is_none());
    assert!(tasks.request_view_refresh(owner).is_none());
    let second = tasks.set_view_visible(owner, true).unwrap();
    assert_ne!(first.generation(), second.generation());

    let stale = tasks.finish_view_refresh(first);
    assert_eq!(stale.outcome, ViewRefreshOutcome::Stale);
    assert!(stale.next.is_none());
    let applied = tasks.finish_view_refresh(second);
    assert_eq!(applied.outcome, ViewRefreshOutcome::Applied);
    assert!(applied.next.is_none());

    let current = tasks.request_view_refresh(owner).unwrap();
    assert!(tasks.request_view_refresh(owner).is_none());
    let next = tasks.finish_view_refresh(current).next.unwrap();
    assert_ne!(current.generation(), next.generation());
    assert_eq!(
        tasks.finish_view_refresh(next).outcome,
        ViewRefreshOutcome::Applied
    );

    let orphaned = tasks.request_view_refresh(owner).unwrap();
    tasks.shutdown();
    assert_eq!(
        tasks.finish_view_refresh(orphaned).outcome,
        ViewRefreshOutcome::Stale,
        "所有者关闭后迟到结果不得复活任务"
    );
}

#[test]
fn operation_result_survives_view_hidden() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .begin_run(
            "epoch-operation-hidden",
            RunMarker {
                run_id: "run-operation-hidden".to_owned(),
                process_id: 23,
                process_created_at_ms: 789,
            },
        )
        .unwrap();
    store
        .register_operation(OperationRegistration {
            operation_id: "operation-hidden".to_owned(),
            store_epoch: "epoch-operation-hidden".to_owned(),
            client_id: "client-test".to_owned(),
            mutation_id: "mutation-hidden".to_owned(),
            method: "document.save".to_owned(),
            params_dto_version: 1,
            params_hash: [3; 32],
            resource_generations: std::collections::BTreeMap::new(),
        })
        .unwrap();
    store
        .transition_operation("operation-hidden", OperationStatus::Running, None, None)
        .unwrap();
    let expected = serde_json::json!({"saved": true});
    store
        .transition_operation(
            "operation-hidden",
            OperationStatus::Succeeded,
            Some(expected.clone()),
            None,
        )
        .unwrap();
    drop(store);

    let writer = WorkspaceWriter::start(paths).unwrap();
    let mut tasks = TaskService::new();
    let owner = ViewTaskOwner::SideView(SideViewId::Nav);
    assert!(tasks.set_view_visible(owner, false).is_none());
    let loaded = writer
        .client()
        .query_operation("operation-hidden")
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(loaded.status, OperationStatus::Succeeded);
    assert_eq!(loaded.result, Some(expected));
    writer.shutdown().unwrap();
}
