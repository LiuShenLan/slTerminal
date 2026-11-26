//! Runtime Git 方法路由测试：状态、diff 分页、破坏写与打开。

use super::runtime::{hello_scoped, start_dual_scope_server_with_mutation};
use super::*;

fn start_dual_scope_server_with_git_diff<D>(
    on_git_diff: D,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    D: Fn(
            Option<slterm_workspace::ProjectId>,
            String,
        ) -> Result<
            (
                slterm_workspace::ProjectId,
                crate::local_control::git::GitDiffSnapshot,
            ),
            JsonRpcError,
        > + Send
        + Sync
        + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        crate::local_control::server::RuntimeCallbacks {
            event_revision: Arc::new(|| 7),
            snapshot: Arc::new(|| Ok(SnapshotResources::default())),
            operation: Arc::new(|_| Ok(None)),
            events: Arc::new(|_| {
                Err(JsonRpcError::new(
                    -32001,
                    "event subscription not ready",
                    Some(JsonRpcErrorData {
                        code: slterm_runtime_protocol::DomainErrorCode::NotReady,
                    }),
                ))
            }),
            mutation: Arc::new(unavailable_mutation),
            read: Arc::new(unavailable_read),
            wait_status: Arc::new(unavailable_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(on_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

#[test]
fn runtime_git_diff_paginates_and_rejects_stale_cursor() {
    let text = format!(
        "{}\ntail",
        "a".repeat(crate::local_control::reads::PAGE_BUDGET_BYTES)
    );
    let revision = Arc::new(Mutex::new(5u64));
    let revision_by_callback = Arc::clone(&revision);
    let (mut server, _calls) = start_dual_scope_server_with_git_diff(move |project_id, path| {
        assert!(project_id.is_none(), "缺省项目由前台解析");
        assert_eq!(path, "src/main.rs");
        Ok((
            "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"
                .parse()
                .expect("测试项目身份必须合法"),
            crate::local_control::git::GitDiffSnapshot {
                revision: *revision_by_callback.lock().expect("revision"),
                text: text.clone(),
            },
        ))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(50),
            RuntimeMethod::GitDiff,
            slterm_runtime_protocol::GitDiffRequest {
                project_id: None,
                path: "src/main.rs".to_owned(),
                cursor: None,
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(
        reply["result"]["data"].as_str().unwrap().len(),
        crate::local_control::reads::PAGE_BUDGET_BYTES
    );
    assert_eq!(reply["result"]["revision"], 5);
    let cursor = reply["result"]["next_cursor"]
        .as_str()
        .expect("尾部仍有内容必须返回续读游标")
        .to_owned();

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(51),
            RuntimeMethod::GitDiff,
            slterm_runtime_protocol::GitDiffRequest {
                project_id: None,
                path: "src/main.rs".to_owned(),
                cursor: Some(cursor.clone()),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["data"], "\ntail");
    assert!(reply["result"]["next_cursor"].is_null());

    *revision.lock().expect("revision") = 6;
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(52),
            RuntimeMethod::GitDiff,
            slterm_runtime_protocol::GitDiffRequest {
                project_id: None,
                path: "src/main.rs".to_owned(),
                cursor: Some(cursor),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "stale_cursor");
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_git_status<G>(
    on_git_status: G,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    G: Fn(
            Option<slterm_workspace::ProjectId>,
        ) -> Result<slterm_runtime_protocol::GitStatusResponse, JsonRpcError>
        + Send
        + Sync
        + 'static,
{
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    let server = LocalControlServer::start_with_runtime(
        test_secret("sk-test"),
        test_secret("sk-test-runtime"),
        hello_response(),
        move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(HandoverResponse::accepted())
        },
        crate::local_control::server::RuntimeCallbacks {
            event_revision: Arc::new(|| 7),
            snapshot: Arc::new(|| Ok(SnapshotResources::default())),
            operation: Arc::new(|_| Ok(None)),
            events: Arc::new(|_| {
                Err(JsonRpcError::new(
                    -32001,
                    "event subscription not ready",
                    Some(JsonRpcErrorData {
                        code: slterm_runtime_protocol::DomainErrorCode::NotReady,
                    }),
                ))
            }),
            mutation: Arc::new(unavailable_mutation),
            read: Arc::new(unavailable_read),
            wait_status: Arc::new(unavailable_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(on_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

#[test]
fn runtime_git_status_routes_project_selection() {
    let selected = Arc::new(Mutex::new(Vec::new()));
    let selected_by_callback = Arc::clone(&selected);
    let (mut server, _calls) = start_dual_scope_server_with_git_status(move |project_id| {
        selected_by_callback
            .lock()
            .expect("项目选择记录")
            .push(project_id);
        Ok(slterm_runtime_protocol::GitStatusResponse {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            root: "D:/work/a".to_owned(),
            branch: "main".to_owned(),
            head: "abc".to_owned(),
            upstream: None,
            ahead: 0,
            behind: 0,
            entries: vec![slterm_runtime_protocol::GitStatusEntry {
                path: "src/main.rs".to_owned(),
                original: None,
                index: slterm_runtime_protocol::GitStatusCode::Modified,
                worktree: slterm_runtime_protocol::GitStatusCode::Unmodified,
                conflict: false,
                ignored: false,
                staged: true,
                unstaged: false,
            }],
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(40),
            RuntimeMethod::GitStatus,
            slterm_runtime_protocol::GitStatusRequest { project_id: None },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["branch"], "main");
    assert_eq!(reply["result"]["entries"][0]["path"], "src/main.rs");
    assert_eq!(reply["result"]["entries"][0]["staged"], true);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(41),
            RuntimeMethod::GitStatus,
            slterm_runtime_protocol::GitStatusRequest {
                project_id: Some("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned()),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["branch"], "main");
    assert_eq!(selected.lock().expect("项目选择记录").len(), 2);
    assert!(
        selected.lock().expect("项目选择记录")[1].is_some(),
        "显式项目必须透传给回调"
    );

    write_json(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","id":42,"method":"git.status","params":{"project_id":"not-a-project"}}),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(selected.lock().expect("项目选择记录").len(), 2);
    server.shutdown().unwrap();
}

#[test]
fn runtime_git_writes_route_expected_state_and_project() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::GitRollback {
                project_id,
                path,
                expected,
            } => {
                assert!(project_id.is_none(), "缺省项目由前台解析");
                assert_eq!(path, "src/main.rs");
                assert_eq!(
                    *expected,
                    slterm_runtime_protocol::GitRollbackExpected::Deleted
                );
            }
            crate::local_control::mutations::RuntimeMutation::GitDelete {
                project_id,
                path,
                expected,
                permanent,
            } => {
                assert_eq!(
                    project_id.map(|id| id.to_string()).as_deref(),
                    Some("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b")
                );
                assert_eq!(path, "notes.txt");
                assert_eq!(
                    *expected,
                    slterm_runtime_protocol::GitDeleteExpected::StagedAdded
                );
                assert!(*permanent);
            }
            crate::local_control::mutations::RuntimeMutation::GitOpenDiff { project_id, path } => {
                assert!(project_id.is_none());
                assert_eq!(path, "src/main.rs");
            }
            crate::local_control::mutations::RuntimeMutation::GitOpenDeleted {
                project_id,
                path,
            } => {
                assert!(project_id.is_none());
                assert_eq!(path, "gone.txt");
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-git".to_owned(),
            status: OperationStatus::Succeeded,
            result: Some(serde_json::json!({})),
            error: None,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(60),
            RuntimeMethod::GitRollback,
            serde_json::json!({"client_id":"c","mutation_id":"m-rollback","expected_revision":null,"request":{"project_id":null,"path":"src/main.rs","expected":"deleted"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(61),
            RuntimeMethod::GitDelete,
            serde_json::json!({"client_id":"c","mutation_id":"m-delete","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","path":"notes.txt","expected":"staged_added","permanent":true}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","id":62,"method":"git.delete","params":{"client_id":"c","mutation_id":"m-invalid","expected_revision":null,"request":{"project_id":"not-a-project","path":"notes.txt","expected":"untracked"}}}),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(63),
            RuntimeMethod::GitOpenDiff,
            serde_json::json!({"client_id":"c","mutation_id":"m-open-diff","expected_revision":null,"request":{"project_id":null,"path":"src/main.rs"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(64),
            RuntimeMethod::GitOpenDeleted,
            serde_json::json!({"client_id":"c","mutation_id":"m-open-deleted","expected_revision":null,"request":{"project_id":null,"path":"gone.txt"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","id":65,"method":"git.open_diff","params":{"client_id":"c","mutation_id":"m-invalid-open","expected_revision":null,"request":{"project_id":"not-a-project","path":"src/main.rs"}}}),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 4, "非法项目不得进入回调");
    server.shutdown().unwrap();
}
