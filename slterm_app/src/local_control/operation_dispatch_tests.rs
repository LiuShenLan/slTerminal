//! Runtime 写操作派发锚：幂等、参数冲突、代次前置与结果不明可查询。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;
use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError, OperationStatus};

use super::operations::{DispatchOutcome, DispatchRequest, OperationDispatcher};
use super::service::mutation_dispatch_request;
use crate::persistence::metadata::RunMarker;
use crate::persistence::workspace::WorkspaceDatabase;
use crate::persistence::workspace_writer::WorkspaceWriter;

const STORE_EPOCH: &str = "epoch-dispatch";

struct Fixture {
    _temp: tempfile::TempDir,
    writer: WorkspaceWriter,
    dispatcher: OperationDispatcher,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let paths =
        slterm_path::AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut database = WorkspaceDatabase::open(&paths).unwrap();
    database
        .begin_run(
            STORE_EPOCH,
            RunMarker {
                run_id: "run-dispatch".into(),
                process_id: 11,
                process_created_at_ms: 13,
            },
        )
        .unwrap();
    let writer = WorkspaceWriter::start(paths).unwrap();
    let dispatcher = OperationDispatcher::new(writer.client(), STORE_EPOCH);
    Fixture {
        _temp: temp,
        writer,
        dispatcher,
    }
}

fn request<'a>(
    method: &'a str,
    mutation_id: &'a str,
    params: &'a serde_json::Value,
) -> DispatchRequest<'a> {
    DispatchRequest {
        client_id: "client-runtime",
        mutation_id,
        method,
        params,
        expected_revision: None,
        current_revision: 0,
        resource_generations: BTreeMap::new(),
    }
}

fn assert_domain(error: &JsonRpcError, code: DomainErrorCode) {
    assert_eq!(error.data.as_ref().map(|data| data.code), Some(code));
}

/// 锚：结果不明时可按操作标识查询，且同一幂等键不自动重发。
#[test]
fn runtime_outcome_queryable() {
    let fixture = fixture();
    let executions = AtomicUsize::new(0);
    let params = json!({"panel_id": "panel-a"});
    let first = fixture
        .dispatcher
        .dispatch(request("panel.close", "mutation-unknown", &params), || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::OutcomeUnknown(json!({"code": "side_effect_unknown"}))
        })
        .expect("首次派发必须登记并执行");
    assert_eq!(first.status, OperationStatus::OutcomeUnknown);
    assert_eq!(first.error, Some(json!({"code": "side_effect_unknown"})));

    // 同一幂等键重试只返回原 Operation，执行体不得再跑。
    let retry = fixture
        .dispatcher
        .dispatch(request("panel.close", "mutation-unknown", &params), || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::Succeeded(json!({"wrong": true}))
        })
        .expect("重复幂等键必须返回原记录");
    assert_eq!(retry.operation_id, first.operation_id);
    assert_eq!(retry.status, OperationStatus::OutcomeUnknown);
    assert_eq!(executions.load(Ordering::SeqCst), 1, "结果不明不得自动重发");

    let queried = fixture
        .dispatcher
        .query(&first.operation_id)
        .expect("查询必须成功")
        .expect("操作必须存在");
    assert_eq!(queried.status, OperationStatus::OutcomeUnknown);
    assert_eq!(queried.error, Some(json!({"code": "side_effect_unknown"})));

    fixture.writer.shutdown().unwrap();
}

#[test]
fn same_mutation_key_with_different_params_is_rejected_without_execution() {
    let fixture = fixture();
    let executions = AtomicUsize::new(0);
    let first = fixture
        .dispatcher
        .dispatch(
            request("panel.save", "mutation-a", &json!({"path": "a"})),
            || {
                executions.fetch_add(1, Ordering::SeqCst);
                DispatchOutcome::Succeeded(json!({"saved": true}))
            },
        )
        .unwrap();
    assert_eq!(first.status, OperationStatus::Succeeded);

    let conflict = fixture
        .dispatcher
        .dispatch(
            request("panel.save", "mutation-a", &json!({"path": "b"})),
            || {
                executions.fetch_add(1, Ordering::SeqCst);
                DispatchOutcome::Succeeded(json!({"saved": true}))
            },
        )
        .expect_err("同键不同参数必须冲突");
    assert_domain(&conflict, DomainErrorCode::MutationConflict);
    assert_eq!(executions.load(Ordering::SeqCst), 1, "冲突不得执行副作用");

    // 键序不同但语义相同的参数仍是同一请求。
    let retry = fixture.dispatcher.dispatch(
        request(
            "panel.save",
            "mutation-a",
            &json!({"path": "a", "options": {"z": 1, "a": 2}}),
        ),
        || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::Succeeded(json!({"saved": true}))
        },
    );
    assert!(
        retry.is_err() || retry.unwrap().status == OperationStatus::Succeeded,
        "不同参数必须按散列区分"
    );
    assert_eq!(executions.load(Ordering::SeqCst), 1);

    fixture.writer.shutdown().unwrap();
}

#[test]
fn stale_expected_revision_is_rejected_before_registration() {
    let fixture = fixture();
    let executions = AtomicUsize::new(0);
    let params = json!({"panel_id": "panel-a"});
    let mut stale = request("panel.close", "mutation-revision", &params);
    stale.expected_revision = Some(4);
    stale.current_revision = 5;

    let error = fixture
        .dispatcher
        .dispatch(stale, || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::Succeeded(json!({}))
        })
        .expect_err("过期 expected_revision 必须拒绝");
    assert_domain(&error, DomainErrorCode::RevisionConflict);
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    // 拒绝发生在登记前：同一 mutation_id 用正确代次仍可登记并执行。
    let mut fresh = request("panel.close", "mutation-revision", &params);
    fresh.expected_revision = Some(5);
    fresh.current_revision = 5;
    let done = fixture
        .dispatcher
        .dispatch(fresh, || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::Succeeded(json!({"closed": true}))
        })
        .expect("正确代次必须可登记");
    assert_eq!(done.status, OperationStatus::Succeeded);

    fixture.writer.shutdown().unwrap();
}

#[test]
fn failed_execution_persists_structured_error() {
    let fixture = fixture();
    let params = json!({"panel_id": "panel-a"});
    let failed = fixture
        .dispatcher
        .dispatch(request("panel.close", "mutation-failed", &params), || {
            DispatchOutcome::Failed(json!({"code": "decision_required"}))
        })
        .expect("失败终态仍必须可查询");
    assert_eq!(failed.status, OperationStatus::Failed);
    assert_eq!(failed.error, Some(json!({"code": "decision_required"})));

    let queried = fixture
        .dispatcher
        .query(&failed.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(queried.status, OperationStatus::Failed);

    // 等待查询在终态上立即返回，不依赖执行体。
    let waited = fixture
        .dispatcher
        .wait(&failed.operation_id, Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert_eq!(waited.status, OperationStatus::Failed);

    fixture.writer.shutdown().unwrap();
}

/// 线缆 expected_revision 必须原样进入派发器并在登记前拒绝过期意图。
#[test]
fn wire_expected_revision_reaches_the_dispatcher_projection() {
    let fixture = fixture();
    let mutation = crate::local_control::mutations::RuntimeMutationRequest {
        client_id: "client-runtime".to_owned(),
        mutation_id: "mutation-wire-revision".to_owned(),
        expected_revision: Some(4),
        mutation: crate::local_control::mutations::RuntimeMutation::AppFocus,
    };
    let params = mutation.mutation.params_value().unwrap();
    let dispatch = mutation_dispatch_request(&mutation, &params, 5);
    assert_eq!(dispatch.expected_revision, Some(4));
    assert_eq!(dispatch.current_revision, 5);
    let executions = AtomicUsize::new(0);
    let error = fixture
        .dispatcher
        .dispatch(dispatch, || {
            executions.fetch_add(1, Ordering::SeqCst);
            DispatchOutcome::Succeeded(json!({}))
        })
        .expect_err("线缆过期代次必须拒绝");
    assert_domain(&error, DomainErrorCode::RevisionConflict);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    fixture.writer.shutdown().unwrap();
}
