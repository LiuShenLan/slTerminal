//! Runtime 方法路由测试：快照、操作、事件、项目、面板、终端、Agent 与 Git。

use super::*;

pub(super) fn hello_scoped(credential: &str, scope: ControlScope) -> HelloRequest {
    JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: "loopback-test".to_owned(),
            credential: Credential::new(credential),
            scope,
        },
    )
}

fn start_dual_scope_server() -> (LocalControlServer, Arc<AtomicUsize>) {
    start_dual_scope_server_with_snapshot(|| Ok(SnapshotResources::default()))
}

fn start_dual_scope_server_with_snapshot<S>(
    on_snapshot: S,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    S: Fn() -> Result<SnapshotResources, JsonRpcError> + Send + Sync + 'static,
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
            snapshot: Arc::new(on_snapshot),
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
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

fn runtime_request(method: RuntimeMethod) -> RuntimeRequest {
    RuntimeRequest::new(JsonRpcId::Number(2), method, EmptyParams {})
}

fn operation_get(operation_id: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(3),
        RuntimeMethod::OperationGet,
        OperationQueryParams {
            operation_id: operation_id.to_owned(),
        },
    )
}

fn operation_wait(operation_id: &str, timeout_ms: u64) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(4),
        RuntimeMethod::OperationWait,
        OperationWaitParams {
            operation_id: operation_id.to_owned(),
            timeout_ms: Some(timeout_ms),
        },
    )
}

fn operation_resource(status: OperationStatus) -> OperationResource {
    OperationResource {
        operation_id: "operation-a".to_owned(),
        status,
        result: status
            .eq(&OperationStatus::Succeeded)
            .then(|| serde_json::json!({"revision": 3})),
        error: matches!(
            status,
            OperationStatus::Failed | OperationStatus::Cancelled | OperationStatus::OutcomeUnknown
        )
        .then(|| serde_json::json!({"code": "side_effect_unknown"})),
    }
}

fn start_dual_scope_server_with_operation<O>(
    on_operation: O,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    O: Fn(&str) -> Result<Option<OperationResource>, JsonRpcError> + Send + Sync + 'static,
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
            operation: Arc::new(on_operation),
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
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

fn subscribe_request() -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(5),
        RuntimeMethod::EventsSubscribe,
        EventsSubscribeParams {
            store_epoch: "store".to_owned(),
            stream_epoch: "stream".to_owned(),
            since_revision: 0,
        },
    )
}

fn unsubscribe_request() -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(6),
        RuntimeMethod::EventsUnsubscribe,
        EmptyParams {},
    )
}

fn project_activate_request(project_id: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(7),
        RuntimeMethod::ProjectActivate,
        ProjectActivateParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-activate".to_owned(),
            expected_revision: None,
            request: ProjectActivateRequest {
                project_id: project_id.to_owned(),
            },
        },
    )
}

pub(super) fn start_dual_scope_server_with_mutation<M>(
    on_mutation: M,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    M: Fn(
            crate::local_control::mutations::RuntimeMutationRequest,
        ) -> Result<OperationResource, JsonRpcError>
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
            snapshot: Arc::new(|| Ok(sample_resources())),
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
            mutation: Arc::new(on_mutation),
            read: Arc::new(unavailable_read),
            wait_status: Arc::new(unavailable_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

fn terminal_view_key() -> String {
    slterm_workspace::ViewKey::Terminal {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".parse().unwrap(),
    }
    .to_canonical_json()
    .unwrap()
}

fn panel_split_request(view: &str) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(8),
        RuntimeMethod::PanelGroupSplit,
        PanelGroupSplitParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-split".to_owned(),
            expected_revision: None,
            request: PanelGroupSplitRequest {
                view: view.to_owned(),
                target_group: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a".to_owned(),
                direction: SplitDirectionWire::LeftRight,
                ratio: 0.5,
                viewport: ViewportSize {
                    width: 800.0,
                    height: 600.0,
                },
            },
        },
    )
}

#[test]
fn runtime_panel_mutations_route_through_dispatcher_and_reject_bad_views() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "panel_group.split");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-split".to_owned(),
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

    write_json(&mut stream, &panel_split_request(&terminal_view_key()));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    write_json(&mut stream, &panel_split_request("not-a-view-key"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_panel_open_settings_routes_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "panel.open_settings");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-settings".to_owned(),
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

    let request = RuntimeRequest::new(
        JsonRpcId::Number(9),
        RuntimeMethod::PanelOpenSettings,
        PanelOpenSettingsParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-settings".to_owned(),
            expected_revision: None,
            request: EmptyParams {},
        },
    );
    write_json(&mut stream, &request);
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_panel_open_file_and_save_route_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::PanelOpenFile { path } => {
                assert_eq!(path, "D:/work/a/README.md");
            }
            crate::local_control::mutations::RuntimeMutation::PanelSave { view } => {
                assert!(matches!(view, slterm_workspace::ViewKey::Editor { .. }));
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-panel".to_owned(),
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
            JsonRpcId::Number(70),
            RuntimeMethod::PanelOpenFile,
            slterm_runtime_protocol::PanelOpenFileParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-open-file".to_owned(),
                expected_revision: None,
                request: slterm_runtime_protocol::PanelOpenFileRequest {
                    path: "D:/work/a/README.md".to_owned(),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    let view = slterm_workspace::ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".parse().unwrap(),
    };
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(71),
            RuntimeMethod::PanelSave,
            slterm_runtime_protocol::PanelSaveParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-save".to_owned(),
                expected_revision: None,
                request: slterm_runtime_protocol::PanelSaveRequest {
                    view: view.to_canonical_json().unwrap(),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 2);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(72),
            RuntimeMethod::PanelSave,
            serde_json::json!({"client_id":"c","mutation_id":"m-invalid","expected_revision":null,"request":{"view":"not-a-view-key"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 2, "非法视图不得进入回调");
    server.shutdown().unwrap();
}

#[test]
fn runtime_app_focus_and_close_route_explicit_dispositions() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::AppFocus => {}
            crate::local_control::mutations::RuntimeMutation::AppClose { documents, hooks } => {
                assert_eq!(
                    *documents,
                    Some(slterm_runtime_protocol::CloseDocumentsPolicy::Save)
                );
                assert_eq!(
                    *hooks,
                    Some(slterm_runtime_protocol::CloseHooksPolicy::Discard)
                );
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-app".to_owned(),
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
            JsonRpcId::Number(80),
            RuntimeMethod::AppFocus,
            slterm_runtime_protocol::AppFocusParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-focus".to_owned(),
                expected_revision: None,
                request: EmptyParams {},
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(81),
            RuntimeMethod::AppClose,
            serde_json::json!({"client_id":"c","mutation_id":"m-close","expected_revision":null,"request":{"documents":"save","hooks":"discard"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 2);

    assert_eq!(
        mutations.load(Ordering::SeqCst),
        2,
        "两条合法写都必须进入回调"
    );
    server.shutdown().unwrap();
}

#[test]
fn runtime_panel_close_routes_view_identity_and_disposition() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::PanelClose { view, documents } => {
                assert!(matches!(view, slterm_workspace::ViewKey::Editor { .. }));
                assert_eq!(
                    *documents,
                    Some(slterm_runtime_protocol::CloseDocumentsPolicy::Discard)
                );
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-panel-close".to_owned(),
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

    let view = slterm_workspace::ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".parse().unwrap(),
    };
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(73),
            RuntimeMethod::PanelClose,
            slterm_runtime_protocol::PanelCloseParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-panel-close".to_owned(),
                expected_revision: None,
                request: slterm_runtime_protocol::PanelCloseRequest {
                    view: view.to_canonical_json().unwrap(),
                    documents: Some(slterm_runtime_protocol::CloseDocumentsPolicy::Discard),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_project_remove_routes_explicit_close_dispositions() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::ProjectRemove {
                project_id,
                documents,
                hooks,
            } => {
                assert_eq!(
                    project_id.to_string(),
                    "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"
                );
                assert_eq!(
                    *documents,
                    Some(slterm_runtime_protocol::CloseDocumentsPolicy::Save)
                );
                assert_eq!(
                    *hooks,
                    Some(slterm_runtime_protocol::CloseHooksPolicy::Discard)
                );
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-project-remove".to_owned(),
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
            JsonRpcId::Number(74),
            RuntimeMethod::ProjectRemove,
            serde_json::json!({"client_id":"c","mutation_id":"m-remove","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","documents":"save","hooks":"discard"}}),
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

#[test]
fn runtime_terminal_input_and_execute_route_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        let method = request.mutation.method().to_owned();
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::TerminalInput {
                generation,
                data,
                ..
            } => {
                assert_eq!(method, "terminal.input");
                assert_eq!(*generation, 3);
                assert_eq!(data, "echo hi");
            }
            crate::local_control::mutations::RuntimeMutation::TerminalExecute {
                generation,
                command,
                ..
            } => {
                assert_eq!(method, "terminal.execute");
                assert_eq!(*generation, 4);
                assert_eq!(command, "cargo test");
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: format!("operation-{method}"),
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

    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(10),
            RuntimeMethod::TerminalInput,
            TerminalInputParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-input".to_owned(),
                expected_revision: None,
                request: TerminalInputRequest {
                    terminal_id: terminal.to_owned(),
                    generation: 3,
                    data: "echo hi".to_owned(),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(11),
            RuntimeMethod::TerminalExecute,
            TerminalExecuteParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-execute".to_owned(),
                expected_revision: None,
                request: TerminalExecuteRequest {
                    terminal_id: terminal.to_owned(),
                    generation: 4,
                    command: "cargo test".to_owned(),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 2);

    // 稳定身份非法必须在进入派发器前拒绝，且不断开连接。
    for invalid in [
        serde_json::json!({"jsonrpc":"2.0","id":12,"method":"terminal.input","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"not-a-terminal-id","generation":1,"data":"ls"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":13,"method":"terminal.execute","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"not-a-terminal-id","generation":1,"command":"ls"}}}),
    ] {
        write_json(&mut stream, &invalid);
        let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
        assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    }
    assert_eq!(mutations.load(Ordering::SeqCst), 2);
    server.shutdown().unwrap();
}

#[test]
fn runtime_agent_read_and_wait_route_agent_identity() {
    let (mut server, _calls) = start_dual_scope_server_with_read(|target| {
        assert_eq!(target.agent_id.as_deref(), Some("claude:session-a"));
        Ok(crate::local_control::reads::TerminalTextSnapshot {
            generation: target.generation,
            revision: 2,
            text: "agent output".to_owned(),
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
            JsonRpcId::Number(23),
            RuntimeMethod::AgentRead,
            AgentReadRequest {
                agent_id: "claude:session-a".to_owned(),
                terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                generation: 2,
                cursor: None,
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["data"], "agent output");
    assert_eq!(reply["result"]["revision"], 2);
    server.shutdown().unwrap();

    let (mut server, _calls) = start_dual_scope_server_with_wait_status(|target| {
        assert_eq!(target.agent_id.as_deref(), Some("claude:session-a"));
        Ok(6)
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
            JsonRpcId::Number(24),
            RuntimeMethod::AgentWait,
            AgentWaitRequest {
                agent_id: "claude:session-a".to_owned(),
                terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                generation: 2,
                since_revision: 5,
                timeout_ms: Some(1_000),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["revision"], 6);
    server.shutdown().unwrap();
}

#[test]
fn runtime_terminal_close_and_reconnect_route_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::TerminalClose {
                generation, ..
            } => {
                assert_eq!(*generation, 4);
            }
            crate::local_control::mutations::RuntimeMutation::TerminalReconnect {
                generation,
                ..
            } => {
                assert_eq!(*generation, 5);
            }
            crate::local_control::mutations::RuntimeMutation::TerminalRename { title, .. } => {
                assert_eq!(title.as_deref(), Some("build"));
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-close".to_owned(),
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
            JsonRpcId::Number(16),
            RuntimeMethod::TerminalClose,
            TerminalCloseParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-close".to_owned(),
                expected_revision: None,
                request: TerminalCloseRequest {
                    terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                    generation: 4,
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(17),
            RuntimeMethod::TerminalReconnect,
            TerminalReconnectParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-reconnect".to_owned(),
                expected_revision: None,
                request: TerminalReconnectRequest {
                    terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                    generation: 5,
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 2);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(18),
            RuntimeMethod::TerminalRename,
            TerminalRenameParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-rename".to_owned(),
                expected_revision: None,
                request: TerminalRenameRequest {
                    terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                    title: Some("build".to_owned()),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 3);
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_read<R>(on_read: R) -> (LocalControlServer, Arc<AtomicUsize>)
where
    R: Fn(
            crate::local_control::reads::TerminalReadTarget,
        ) -> Result<crate::local_control::reads::TerminalTextSnapshot, JsonRpcError>
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
            read: Arc::new(on_read),
            wait_status: Arc::new(unavailable_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

fn terminal_read_request(terminal: &str, generation: u64, cursor: Option<&str>) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(14),
        RuntimeMethod::TerminalRead,
        TerminalReadRequest {
            terminal_id: terminal.to_owned(),
            generation,
            cursor: cursor.map(str::to_owned),
        },
    )
}

#[test]
fn runtime_terminal_read_pages_snapshot_and_rejects_stale_cursor() {
    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    let text = format!(
        "{}\ntail",
        "a".repeat(crate::local_control::reads::PAGE_BUDGET_BYTES)
    );
    let snapshot_state = Arc::new(Mutex::new((3u64, 5u64, text)));
    let read_state = Arc::clone(&snapshot_state);
    let (mut server, _calls) = start_dual_scope_server_with_read(move |target| {
        assert_eq!(target.terminal_id.to_string(), terminal);
        assert_eq!(target.generation, 3);
        let state = read_state.lock().expect("读取快照");
        Ok(crate::local_control::reads::TerminalTextSnapshot {
            generation: state.0,
            revision: state.1,
            text: state.2.clone(),
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &terminal_read_request(terminal, 3, None));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(
        reply["result"]["data"].as_str().unwrap().len(),
        crate::local_control::reads::PAGE_BUDGET_BYTES
    );
    assert_eq!(reply["result"]["generation"], 3);
    assert_eq!(reply["result"]["revision"], 5);
    let cursor = reply["result"]["next_cursor"]
        .as_str()
        .expect("尾部仍有内容必须返回续读游标")
        .to_owned();

    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["data"], "\ntail");
    assert!(reply["result"]["next_cursor"].is_null());

    // 输出 revision 变化后旧游标必须失效。
    snapshot_state.lock().expect("读取快照").1 = 6;
    write_json(
        &mut stream,
        &terminal_read_request(terminal, 3, Some(&cursor)),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "stale_cursor");

    write_json(
        &mut stream,
        &terminal_read_request("not-a-terminal-id", 3, None),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_agents<A>(on_agents: A) -> (LocalControlServer, Arc<AtomicUsize>)
where
    A: Fn() -> Result<Vec<slterm_runtime_protocol::AgentResource>, JsonRpcError>
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
            agents: Arc::new(on_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

#[test]
fn runtime_agent_list_and_get_filter_by_identity() {
    let (mut server, _calls) = start_dual_scope_server_with_agents(|| {
        Ok(vec![
            slterm_runtime_protocol::AgentResource {
                agent_id: "claude:session-a".to_owned(),
                terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
                project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
                kind: "claude".to_owned(),
                status: "working".to_owned(),
                native_session_id: Some("session-a".to_owned()),
            },
            slterm_runtime_protocol::AgentResource {
                agent_id: "claude:session-a".to_owned(),
                terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".to_owned(),
                project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
                kind: "claude".to_owned(),
                status: "unknown".to_owned(),
                native_session_id: Some("session-a".to_owned()),
            },
        ])
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &runtime_request(RuntimeMethod::AgentList));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["agents"].as_array().unwrap().len(), 2);
    assert_eq!(reply["result"]["agents"][0]["status"], "working");

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(19),
            RuntimeMethod::AgentGet,
            slterm_runtime_protocol::AgentGetRequest {
                agent_id: "claude:session-a".to_owned(),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["agents"].as_array().unwrap().len(), 2);

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(20),
            RuntimeMethod::AgentGet,
            slterm_runtime_protocol::AgentGetRequest {
                agent_id: "codex:missing".to_owned(),
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "unknown_agent");
    server.shutdown().unwrap();
}

#[test]
fn runtime_agent_input_and_close_route_through_dispatcher() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        match &request.mutation {
            crate::local_control::mutations::RuntimeMutation::AgentInput {
                agent_id,
                generation,
                data,
                ..
            } => {
                assert_eq!(agent_id, "claude:session-a");
                assert_eq!(*generation, 2);
                assert_eq!(data, "ls");
            }
            crate::local_control::mutations::RuntimeMutation::AgentClose {
                agent_id,
                generation,
                ..
            } => {
                assert_eq!(agent_id, "claude:session-a");
                assert_eq!(*generation, 2);
            }
            crate::local_control::mutations::RuntimeMutation::AgentStart { kind, .. } => {
                assert_eq!(kind, "claude");
            }
            crate::local_control::mutations::RuntimeMutation::AgentResume {
                source,
                session_id,
                ..
            } => {
                assert_eq!(source, "claude");
                assert_eq!(session_id.as_deref(), Some("abc"));
            }
            crate::local_control::mutations::RuntimeMutation::AgentFork {
                source,
                session_id,
                ..
            } => {
                assert_eq!(source, "claude");
                assert_eq!(session_id, "abc");
            }
            other => panic!("意外写意图: {other:?}"),
        }
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-agent".to_owned(),
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

    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(21),
            RuntimeMethod::AgentInput,
            AgentInputParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-agent-input".to_owned(),
                expected_revision: None,
                request: AgentInputRequest {
                    agent_id: "claude:session-a".to_owned(),
                    terminal_id: terminal.to_owned(),
                    generation: 2,
                    data: "ls".to_owned(),
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    write_json(
        &mut stream,
        &RuntimeRequest::new(
            JsonRpcId::Number(22),
            RuntimeMethod::AgentClose,
            AgentCloseParams {
                client_id: "client-runtime".to_owned(),
                mutation_id: "mutation-agent-close".to_owned(),
                expected_revision: None,
                request: AgentCloseRequest {
                    agent_id: "claude:session-a".to_owned(),
                    terminal_id: terminal.to_owned(),
                    generation: 2,
                },
            },
        ),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");

    for (index, (method, params)) in [
        (
            RuntimeMethod::AgentStart,
            serde_json::json!({"client_id":"c","mutation_id":"m-start","expected_revision":null,"request":{"kind":"claude","project_root":null,"cwd":null}}),
        ),
        (
            RuntimeMethod::AgentResume,
            serde_json::json!({"client_id":"c","mutation_id":"m-resume","expected_revision":null,"request":{"source":"claude","session_id":"abc","session_file":null,"project_root":null,"cwd":null}}),
        ),
        (
            RuntimeMethod::AgentFork,
            serde_json::json!({"client_id":"c","mutation_id":"m-fork","expected_revision":null,"request":{"source":"claude","session_id":"abc","project_root":null,"cwd":null}}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let request = RuntimeRequest::new(JsonRpcId::Number(30 + index as i64), method, params);
        write_json(&mut stream, &request);
        let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
        assert_eq!(reply["result"]["status"], "succeeded");
    }
    assert_eq!(mutations.load(Ordering::SeqCst), 5);
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_wait_status<W>(
    on_wait_status: W,
) -> (LocalControlServer, Arc<AtomicUsize>)
where
    W: Fn(crate::local_control::server::TerminalStatusTarget) -> Result<u64, JsonRpcError>
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
            wait_status: Arc::new(on_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

fn terminal_wait_request(
    terminal: &str,
    generation: u64,
    since_revision: u64,
    timeout_ms: u64,
) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(15),
        RuntimeMethod::TerminalWait,
        TerminalWaitRequest {
            terminal_id: terminal.to_owned(),
            generation,
            since_revision,
            timeout_ms: Some(timeout_ms),
        },
    )
}

#[test]
fn runtime_terminal_wait_returns_newer_revision_and_times_out() {
    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    let (mut server, _calls) = start_dual_scope_server_with_wait_status(move |target| {
        assert_eq!(target.terminal_id.to_string(), terminal);
        assert_eq!(target.generation, 3);
        Ok(6)
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &terminal_wait_request(terminal, 3, 5, 1_000));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["revision"], 6);
    assert_eq!(reply["result"]["generation"], 3);

    write_json(&mut stream, &terminal_wait_request(terminal, 3, 100, 1));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "not_ready");
    server.shutdown().unwrap();
}

#[test]
fn runtime_project_list_and_mutations_route_through_callbacks() {
    let mutations = Arc::new(AtomicUsize::new(0));
    let mutations_by_callback = Arc::clone(&mutations);
    let (mut server, _calls) = start_dual_scope_server_with_mutation(move |request| {
        assert_eq!(request.mutation.method(), "project.activate");
        mutations_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(OperationResource {
            operation_id: "operation-project".to_owned(),
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

    write_json(&mut stream, &runtime_request(RuntimeMethod::ProjectList));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["projects"][0]["display_name"], "Repo");

    write_json(
        &mut stream,
        &project_activate_request("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"),
    );
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);

    // 非法稳定身份在调用派发器前拒绝。
    write_json(&mut stream, &project_activate_request("not-a-uuid"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "invalid_request");
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    server.shutdown().unwrap();
}

fn start_dual_scope_server_with_events<E>(on_events: E) -> (LocalControlServer, Arc<AtomicUsize>)
where
    E: Fn(
            EventsSubscribeParams,
        ) -> Result<crate::local_control::events::Subscription, JsonRpcError>
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
            events: Arc::new(on_events),
            mutation: Arc::new(unavailable_mutation),
            read: Arc::new(unavailable_read),
            wait_status: Arc::new(unavailable_wait_status),
            agents: Arc::new(unavailable_agents),
            git_status: Arc::new(unavailable_git_status),
            git_diff: Arc::new(unavailable_git_diff),
        },
    )
    .unwrap();
    (server, calls)
}

#[test]
fn runtime_subscription_streams_events_and_accepts_unsubscribe() {
    let (events_tx, events_rx) =
        mpsc::sync_channel(crate::local_control::events::CLIENT_QUEUE_CAPACITY);
    let receiver_slot = Arc::new(Mutex::new(Some(events_rx)));
    let slot = Arc::clone(&receiver_slot);
    let (mut server, _calls) = start_dual_scope_server_with_events(move |_| {
        let events = slot
            .lock()
            .expect("订阅槽")
            .take()
            .expect("每次连接只订阅一次");
        Ok(crate::local_control::events::Subscription {
            confirmed: EventsSubscribedResponse {
                stream_epoch: "stream".to_owned(),
                revision: 0,
            },
            events,
        })
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &subscribe_request());
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["stream_epoch"], "stream");
    assert_eq!(reply["result"]["revision"], 0);

    events_tx
        .send(RuntimeEvent {
            revision: 1,
            kind: "project.activated".to_owned(),
            resource: "project-a".to_owned(),
            generation: 1,
        })
        .unwrap();
    let notification: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(notification["method"], "events.event");
    assert_eq!(notification["params"]["revision"], 1);
    assert_eq!(notification["params"]["kind"], "project.activated");
    assert_eq!(notification["params"]["resource"], "project-a");

    write_json(&mut stream, &unsubscribe_request());
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"], serde_json::json!({}));
    assert_eq!(read_line(&mut stream), "", "取消订阅后连接必须结束");
    server.shutdown().unwrap();
}

#[test]
fn runtime_operation_get_returns_persisted_result_or_structured_unknown() {
    let (mut server, calls) = start_dual_scope_server_with_operation(|operation_id| {
        Ok((operation_id == "operation-a")
            .then(|| operation_resource(OperationStatus::OutcomeUnknown)))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &operation_get("operation-a"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["operation_id"], "operation-a");
    assert_eq!(reply["result"]["status"], "outcome_unknown");
    assert_eq!(reply["result"]["error"]["code"], "side_effect_unknown");

    write_json(&mut stream, &operation_get("operation-missing"));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["error"]["data"]["code"], "unknown_operation");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_operation_wait_polls_until_terminal_then_returns_result() {
    let polls = Arc::new(AtomicUsize::new(0));
    let polls_by_callback = Arc::clone(&polls);
    let (mut server, _calls) = start_dual_scope_server_with_operation(move |operation_id| {
        assert_eq!(operation_id, "operation-a");
        let round = polls_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(Some(if round == 0 {
            operation_resource(OperationStatus::Running)
        } else {
            operation_resource(OperationStatus::Succeeded)
        }))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));

    write_json(&mut stream, &operation_wait("operation-a", 2_000));
    let reply: serde_json::Value = serde_json::from_str(&read_line(&mut stream)).unwrap();
    assert_eq!(reply["result"]["status"], "succeeded");
    assert_eq!(reply["result"]["result"]["revision"], 3);
    assert!(polls.load(Ordering::SeqCst) >= 2);
    server.shutdown().unwrap();
}

/// 客户端断开后等待必须立即结束，不能继续占用连接直到超时。
#[test]
fn runtime_operation_wait_cancels_when_the_client_disconnects() {
    let polls = Arc::new(AtomicUsize::new(0));
    let polls_by_callback = Arc::clone(&polls);
    let (mut server, _calls) = start_dual_scope_server_with_operation(move |_| {
        polls_by_callback.fetch_add(1, Ordering::SeqCst);
        Ok(Some(operation_resource(OperationStatus::Running)))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &operation_wait("operation-a", 5_000));
    std::thread::sleep(Duration::from_millis(200));
    let started = std::time::Instant::now();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let _ = read_line(&mut stream);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "客户端断开后等待必须快速结束"
    );
    let before = polls.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        polls.load(Ordering::SeqCst),
        before,
        "等待结束后不得继续轮询"
    );
    server.shutdown().unwrap();
}

fn sample_resources() -> SnapshotResources {
    SnapshotResources {
        projects: vec![ProjectResource {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            display_name: "Repo".to_owned(),
            root: "D:/work/a".to_owned(),
            active: true,
        }],
        groups: vec![PanelGroupResource {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            group_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c".to_owned(),
            views: vec![ViewResource {
                kind: "terminal".to_owned(),
                key: r#"{"kind":"terminal","version":1}"#.to_owned(),
            }],
            active_view: r#"{"kind":"terminal","version":1}"#.to_owned(),
        }],
        terminals: vec![TerminalResource {
            terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d".to_owned(),
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".to_owned(),
            number: 1,
            cwd: "D:/work/a".to_owned(),
            shell: "pwsh.exe".to_owned(),
            title: None,
            run_intent: "launch".to_owned(),
            generation: 1,
        }],
        documents: vec![DocumentResource {
            document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".to_owned(),
            path: "D:/work/a/README.md".to_owned(),
        }],
    }
}

#[test]
fn runtime_describe_returns_registry_and_contract_hashes() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(value["result"]["protocol_sha256"], "protocol-hash");
    assert_eq!(value["result"]["schema_sha256"], "schema-hash");
    let methods: Vec<&str> = value["result"]["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|method| method["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "runtime.hello",
            "runtime.describe",
            "runtime.snapshot",
            "runtime.operation_get",
            "runtime.operation_wait",
            "events.subscribe",
            "events.unsubscribe",
            "project.list",
            "project.activate",
            "project.rename",
            "project.reorder",
            "project.remove",
            "panel_group.split",
            "panel_group.merge",
            "panel_group.resize",
            "panel.activate",
            "panel.move",
            "panel.open_settings",
            "panel.open_file",
            "panel.save",
            "panel.close",
            "terminal.create",
            "terminal.input",
            "terminal.execute",
            "terminal.read",
            "terminal.wait",
            "terminal.close",
            "terminal.reconnect",
            "terminal.rename",
            "agent.list",
            "agent.get",
            "agent.input",
            "agent.close",
            "agent.read",
            "agent.wait",
            "agent.start",
            "agent.resume",
            "agent.fork",
            "git.status",
            "git.diff",
            "git.rollback",
            "git.delete",
            "git.open_diff",
            "git.open_deleted",
            "app.focus",
            "app.close",
            "instance.handover"
        ]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_snapshot_returns_injected_resources_with_hello_generation() {
    let (mut server, calls) = start_dual_scope_server_with_snapshot(|| Ok(sample_resources()));
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Snapshot));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();

    assert_eq!(value["result"]["store_epoch"], "store");
    assert_eq!(value["result"]["stream_epoch"], "stream");
    assert_eq!(
        value["result"]["revision"], 7,
        "快照 revision 必须来自事件环回调而不是 hello 固定值"
    );
    let resources = &value["result"]["resources"];
    assert_eq!(resources["projects"][0]["display_name"], "Repo");
    assert_eq!(resources["terminals"][0]["run_intent"], "launch");
    assert_eq!(resources["documents"][0]["path"], "D:/work/a/README.md");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_snapshot_failure_returns_structured_error_and_keeps_connection() {
    let (mut server, _calls) = start_dual_scope_server_with_snapshot(|| {
        Err(JsonRpcError::new(
            -32001,
            "not ready",
            Some(JsonRpcErrorData {
                code: slterm_runtime_protocol::DomainErrorCode::NotReady,
            }),
        ))
    });
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Snapshot));
    let reply = read_line(&mut stream);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(value["error"]["data"]["code"], "not_ready");

    // 结构化错误不关闭连接；后续 describe 仍可用。
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));
    assert!(read_line(&mut stream).contains("protocol-hash"));
    server.shutdown().unwrap();
}

#[test]
fn handover_connection_rejects_runtime_methods_after_handshake() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test", ControlScope::Handover),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &runtime_request(RuntimeMethod::Describe));

    assert_eq!(
        read_line(&mut stream),
        "",
        "handover 连接不得执行 Runtime 方法"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_scope_handshake_uses_its_own_credential() {
    let (mut server, _calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    let reply = read_line(&mut stream);

    assert!(
        reply.contains("store"),
        "Runtime 作用域必须完成握手: {reply}"
    );
    server.shutdown().unwrap();
}

#[test]
fn runtime_and_handover_credentials_are_isolated() {
    let (mut server, calls) = start_dual_scope_server();

    // Runtime 作用域不接受 handover 凭据。
    let mut runtime_stream = connect(&server);
    runtime_stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut runtime_stream,
        &hello_scoped("sk-test", ControlScope::Runtime),
    );
    assert_eq!(read_line(&mut runtime_stream), "");

    // Handover 作用域不接受 runtime 凭据。
    let mut handover_stream = connect(&server);
    handover_stream
        .set_read_timeout(Some(TEST_TIMEOUT))
        .unwrap();
    write_json(
        &mut handover_stream,
        &hello_scoped("sk-test-runtime", ControlScope::Handover),
    );
    assert_eq!(read_line(&mut handover_stream), "");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}

#[test]
fn runtime_v1_rejected() {
    let (mut server, _calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    let v1 = JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 1,
            client_id: "v1-client".to_owned(),
            credential: Credential::new("sk-test-runtime"),
            scope: ControlScope::Runtime,
        },
    );
    write_json(&mut stream, &v1);

    assert_eq!(read_line(&mut stream), "", "v1 协议必须无响应断开");
    server.shutdown().unwrap();
}

#[test]
fn runtime_connection_rejects_handover_method_after_handshake() {
    let (mut server, calls) = start_dual_scope_server();
    let mut stream = connect(&server);
    stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
    write_json(
        &mut stream,
        &hello_scoped("sk-test-runtime", ControlScope::Runtime),
    );
    assert!(read_line(&mut stream).contains("store"));
    write_json(&mut stream, &focus());

    assert_eq!(read_line(&mut stream), "", "Runtime 连接不得执行 handover");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    server.shutdown().unwrap();
}
