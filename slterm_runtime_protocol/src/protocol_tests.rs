//! Runtime hello 与单实例移交的线缆合同测试。

use crate::protocol::{
    ClientRequest, ControlMessage, ControlMethod, ControlScope, Credential, DescribeResponse,
    DiscoveryRecord, DocumentResource, DomainErrorCode, EmptyParams, HandoverMethod,
    HandoverParams, HandoverRequest, HelloMethod, HelloParams, HelloRequest, HelloResponse,
    JsonRpcError, JsonRpcErrorData, JsonRpcId, JsonRpcRequest, JsonRpcResponse, MutationParams,
    OperationQueryParams, OperationResource, OperationWaitParams, PanelActivateRequest,
    PanelGroupMergeRequest, PanelGroupResizeRequest, PanelGroupResource, PanelGroupSplitRequest,
    PanelMoveRequest, ProjectActivateRequest, ProjectRemoveRequest, ProjectRenameRequest,
    ProjectReorderRequest, ProjectResource, ProtocolError, RuntimeMethod, RuntimeRequest,
    SnapshotResources, SnapshotResponse, SplitDirectionWire, TerminalCreateRequest,
    TerminalExecuteRequest, TerminalInputRequest, TerminalResource, ViewResource, ViewportSize,
    params_sha256, parse_client_request,
};
use crate::{MethodKind, OperationStatus};

#[cfg(feature = "schema")]
use crate::protocol::HandoverResponse;

fn hello_request() -> HelloRequest {
    JsonRpcRequest::new(
        JsonRpcId::String("client-request-1".to_owned()),
        HelloMethod::RuntimeHello,
        HelloParams {
            protocol_version: 2,
            client_id: "test-client".to_owned(),
            credential: Credential::new("sk-test"),
            scope: ControlScope::Handover,
        },
    )
}

fn discovery_record() -> DiscoveryRecord {
    DiscoveryRecord {
        protocol_version: 2,
        endpoint: "127.0.0.1:4242".parse().unwrap(),
        run_id: "run-test".to_owned(),
        process_id: 123,
        process_created_at_ms: 456,
    }
}

#[test]
fn discovery_round_trip_contains_only_non_sensitive_metadata() {
    let record = discovery_record();
    record.validate().unwrap();
    let encoded = serde_json::to_value(&record).unwrap();
    let decoded: DiscoveryRecord = serde_json::from_value(encoded.clone()).unwrap();

    assert_eq!(decoded, record);
    let mut fields: Vec<&str> = encoded
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "endpoint",
            "process_created_at_ms",
            "process_id",
            "protocol_version",
            "run_id"
        ]
    );
}

#[test]
fn discovery_rejects_unknown_and_sensitive_fields() {
    for field in ["extra", "credential", "token", "secret"] {
        let mut encoded = serde_json::to_value(discovery_record()).unwrap();
        encoded[field] = serde_json::Value::String("sk-test".to_owned());
        assert!(
            serde_json::from_value::<DiscoveryRecord>(encoded).is_err(),
            "发现 DTO 必须拒绝字段 {field}"
        );
    }
}

#[test]
fn discovery_rejects_invalid_version_endpoint_and_identity() {
    let invalid_values = [
        serde_json::json!({"protocol_version": 1}),
        serde_json::json!({"protocol_version": 3}),
        serde_json::json!({"endpoint": "0.0.0.0:4242"}),
        serde_json::json!({"endpoint": "192.168.1.2:4242"}),
        serde_json::json!({"endpoint": "[::]:4242"}),
        serde_json::json!({"endpoint": "127.0.0.1:0"}),
        serde_json::json!({"endpoint": "[::1]:0"}),
        serde_json::json!({"run_id": ""}),
        serde_json::json!({"run_id": "  "}),
        serde_json::json!({"process_id": 0}),
        serde_json::json!({"process_created_at_ms": 0}),
    ];
    for patch in invalid_values {
        let mut encoded = serde_json::to_value(discovery_record()).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let record: DiscoveryRecord = serde_json::from_value(encoded).unwrap();
        assert!(
            record.validate().is_err(),
            "必须拒绝无效发现记录 {record:?}"
        );
    }
}

#[test]
fn hello_round_trip_redacts_credential_debug() {
    let request = hello_request();
    let encoded = serde_json::to_string(&request).unwrap();
    let decoded: HelloRequest = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, request);
    assert!(encoded.contains("runtime.hello"));
    assert!(encoded.contains("sk-test"));
    assert!(!format!("{request:?}").contains("sk-test"));
}

#[test]
fn credential_clear_overwrites_secret_bytes_in_place() {
    let mut credential = Credential::new("sk-test-clear-in-place");
    let secret_len = credential.as_str().len();

    credential.clear();

    assert_eq!(credential.as_str().as_bytes(), vec![0; secret_len]);
}

#[test]
fn handover_supports_all_closed_intent_shapes() {
    let cases = [
        HandoverParams::Focus,
        HandoverParams::OpenProject {
            project_root: "D:/work/a".to_owned(),
        },
        HandoverParams::OpenFile {
            path: "D:/work/a/README.md".to_owned(),
        },
        HandoverParams::NewTerminal {
            project_root: Some("D:/work/a".to_owned()),
            cwd: Some("D:/work/a/src".to_owned()),
        },
    ];

    for params in cases {
        let request: HandoverRequest = JsonRpcRequest::new(
            JsonRpcId::Number(7),
            HandoverMethod::InstanceHandover,
            params,
        );
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: HandoverRequest = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, request);
    }
}

#[test]
fn parser_accepts_hello_and_handover() {
    let hello = serde_json::to_vec(&hello_request()).unwrap();
    assert!(matches!(
        parse_client_request(&hello),
        Ok(ClientRequest::Hello(_))
    ));

    let handover = serde_json::to_vec(&JsonRpcRequest::new(
        JsonRpcId::Number(1),
        HandoverMethod::InstanceHandover,
        HandoverParams::Focus,
    ))
    .unwrap();
    assert!(matches!(
        parse_client_request(&handover),
        Ok(ClientRequest::Handover(_))
    ));
}

#[test]
fn parser_rejects_batch_notifications_and_bad_envelopes() {
    assert_eq!(
        parse_client_request(br#"[{"jsonrpc":"2.0"}]"#),
        Err(ProtocolError::BatchUnsupported)
    );
    assert_eq!(
        parse_client_request(
            br#"{"jsonrpc":"2.0","id":null,"method":"runtime.hello","params":{}}"#
        ),
        Err(ProtocolError::InvalidJson)
    );
    assert_eq!(
        parse_client_request(br#"{"jsonrpc":"1.0","id":1,"method":"runtime.hello","params":{"protocol_version":2,"client_id":"c","credential":"sk-test","scope":"handover"}}"#),
        Err(ProtocolError::InvalidRequest)
    );
    assert_eq!(
        parse_client_request(br#"{"jsonrpc":"2.0","id":1,"method":"runtime.hello","params":{"protocol_version":2,"client_id":"c","credential":"sk-test","scope":"handover"},"extra":true}"#),
        Err(ProtocolError::InvalidJson)
    );
}

#[test]
fn parser_rejects_invalid_hello_parameters() {
    for params in [
        HelloParams {
            protocol_version: 1,
            client_id: "test-client".to_owned(),
            credential: Credential::new("sk-test"),
            scope: ControlScope::Runtime,
        },
        HelloParams {
            protocol_version: 2,
            client_id: String::new(),
            credential: Credential::new("sk-test"),
            scope: ControlScope::Runtime,
        },
        HelloParams {
            protocol_version: 2,
            client_id: "test-client".to_owned(),
            credential: Credential::new(""),
            scope: ControlScope::Runtime,
        },
    ] {
        let request = JsonRpcRequest::new(JsonRpcId::Number(1), HelloMethod::RuntimeHello, params);
        let encoded = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            parse_client_request(&encoded),
            Err(ProtocolError::InvalidRequest)
        );
    }
}

#[test]
fn parser_rejects_empty_handover_paths() {
    for params in [
        HandoverParams::OpenProject {
            project_root: String::new(),
        },
        HandoverParams::OpenFile {
            path: String::new(),
        },
        HandoverParams::NewTerminal {
            project_root: Some(String::new()),
            cwd: None,
        },
        HandoverParams::NewTerminal {
            project_root: None,
            cwd: Some(String::new()),
        },
    ] {
        let request = JsonRpcRequest::new(
            JsonRpcId::Number(1),
            HandoverMethod::InstanceHandover,
            params,
        );
        let encoded = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            parse_client_request(&encoded),
            Err(ProtocolError::InvalidRequest)
        );
    }
}

#[test]
fn parser_enforces_utf8_and_request_budget() {
    assert_eq!(
        parse_client_request(&[0xff, 0xfe]),
        Err(ProtocolError::InvalidUtf8)
    );
    let oversized = vec![b' '; 128 * 1024 + 1];
    assert_eq!(
        parse_client_request(&oversized),
        Err(ProtocolError::RequestTooLarge)
    );
}

#[test]
fn parser_rejects_duplicate_fields_and_unknown_methods() {
    assert_eq!(
        parse_client_request(
            br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"runtime.hello","params":{"protocol_version":2,"client_id":"c","credential":"sk-test","scope":"handover"}}"#
        ),
        Err(ProtocolError::InvalidJson)
    );
    assert_eq!(
        parse_client_request(
            br#"{"jsonrpc":"2.0","id":1,"method":"runtime.unknown","params":{"protocol_version":2,"client_id":"c","credential":"sk-test","scope":"handover"}}"#
        ),
        Err(ProtocolError::InvalidJson)
    );
}

#[test]
fn response_shapes_are_closed_and_versioned() {
    let result = JsonRpcResponse::result(
        JsonRpcId::Number(1),
        HelloResponse::new("store", "stream", 3, "protocol-hash", "schema-hash"),
    );
    assert!(result.validate().is_ok());
    let encoded_result = serde_json::to_string(&result).unwrap();
    assert!(encoded_result.contains("result"));
    assert!(!encoded_result.contains("error"));

    let error = JsonRpcResponse::<HelloResponse>::error(
        JsonRpcId::Number(1),
        JsonRpcError::new(
            -32001,
            "not ready",
            Some(JsonRpcErrorData {
                code: DomainErrorCode::NotReady,
            }),
        ),
    );
    assert!(error.validate().is_ok());
    let encoded_error = serde_json::to_string(&error).unwrap();
    assert!(encoded_error.contains("error"));
    assert!(!encoded_error.contains("result"));

    let invalid = JsonRpcResponse::Result {
        jsonrpc: "1.0".to_owned(),
        id: JsonRpcId::Number(1),
        result: HelloResponse::new("store", "stream", 3, "protocol-hash", "schema-hash"),
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn control_message_distinguishes_error_from_hello_result() {
    let error = JsonRpcResponse::<HelloResponse>::error(
        JsonRpcId::Number(1),
        JsonRpcError::new(-32001, "not ready", None),
    );
    let encoded = serde_json::to_string(&error).unwrap();
    let decoded: ControlMessage = serde_json::from_str(&encoded).unwrap();
    assert!(matches!(decoded, ControlMessage::Error(_)));
}

#[test]
fn schema_generator_does_not_embed_its_output_asset() {
    for source in [
        include_str!("lib.rs"),
        include_str!("schema.rs"),
        include_str!("bin/runtime_schema.rs"),
    ] {
        assert!(!source.contains("include_str!"));
    }
}

#[test]
fn method_registry_drives_hello_capabilities() {
    let response = HelloResponse::new("store", "stream", 3, "protocol", "schema");
    for method in ControlMethod::ALL {
        assert!(response.capabilities.methods.contains(method.as_str()));
    }
}

fn runtime_request(method: RuntimeMethod) -> RuntimeRequest {
    RuntimeRequest::new(JsonRpcId::Number(7), method, EmptyParams {})
}

fn operation_get_request() -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(9),
        RuntimeMethod::OperationGet,
        OperationQueryParams {
            operation_id: "operation-a".to_owned(),
        },
    )
}

fn operation_wait_request(timeout_ms: u64) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(10),
        RuntimeMethod::OperationWait,
        OperationWaitParams {
            operation_id: "operation-a".to_owned(),
            timeout_ms: Some(timeout_ms),
        },
    )
}

#[test]
fn runtime_requests_are_closed_and_require_empty_params() {
    for (method, wire) in [
        (RuntimeMethod::Describe, "runtime.describe"),
        (RuntimeMethod::Snapshot, "runtime.snapshot"),
    ] {
        let encoded = serde_json::to_value(runtime_request(method)).unwrap();
        let parsed = parse_client_request(encoded.to_string().as_bytes()).unwrap();
        match parsed {
            ClientRequest::Runtime(request) => assert_eq!(request.method, method),
            other => panic!("{wire} 必须解析为 Runtime 请求: {other:?}"),
        }
    }
    for invalid in [
        serde_json::json!({"jsonrpc":"2.0","id":7,"method":"runtime.unknown","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":7,"method":"runtime.describe","params":{"extra":1}}),
        serde_json::json!({"jsonrpc":"2.0","id":7,"method":"runtime.describe","params":null}),
        serde_json::json!({"jsonrpc":"2.0","id":7,"method":"runtime.snapshot"}),
        serde_json::json!({"jsonrpc":"1.0","id":7,"method":"runtime.describe","params":{}}),
    ] {
        assert!(
            parse_client_request(invalid.to_string().as_bytes()).is_err(),
            "非法 Runtime 请求必须拒绝: {invalid}"
        );
    }
}

#[test]
fn operation_queries_parse_with_method_specific_params() {
    for (request, expected) in [
        (operation_get_request(), RuntimeMethod::OperationGet),
        (operation_wait_request(1_000), RuntimeMethod::OperationWait),
    ] {
        let encoded = serde_json::to_value(request).unwrap();
        let parsed = parse_client_request(encoded.to_string().as_bytes()).unwrap();
        match parsed {
            ClientRequest::Runtime(request) => assert_eq!(request.method, expected),
            other => panic!("操作查询必须解析为 Runtime 请求: {other:?}"),
        }
    }
    let wait_without_timeout = RuntimeRequest::new(
        JsonRpcId::Number(11),
        RuntimeMethod::OperationWait,
        OperationWaitParams {
            operation_id: "operation-a".to_owned(),
            timeout_ms: None,
        },
    );
    let encoded = serde_json::to_value(wait_without_timeout).unwrap();
    assert!(parse_client_request(encoded.to_string().as_bytes()).is_ok());
    for invalid in [
        // get 不接受 timeouts
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"runtime.operation_get","params":{"operation_id":"a","timeout_ms":10}}),
        // 空身份
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"runtime.operation_get","params":{"operation_id":""}}),
        // wait 超时上限 600 秒
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"runtime.operation_wait","params":{"operation_id":"a","timeout_ms":600001}}),
        // describe 不接受操作参数
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"runtime.describe","params":{"operation_id":"a"}}),
        // 方法未登记
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"runtime.operation_cancel","params":{"operation_id":"a"}}),
    ] {
        assert!(
            parse_client_request(invalid.to_string().as_bytes()).is_err(),
            "非法操作查询必须拒绝: {invalid}"
        );
    }
}

fn project_mutation<P: serde::Serialize>(method: RuntimeMethod, request: P) -> RuntimeRequest {
    RuntimeRequest::new(
        JsonRpcId::Number(20),
        method,
        MutationParams {
            client_id: "client-runtime".to_owned(),
            mutation_id: "mutation-1".to_owned(),
            expected_revision: Some(3),
            request,
        },
    )
}

#[test]
fn project_methods_require_idempotency_envelope_and_project_identity() {
    let project_id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    for request in [
        project_mutation(
            RuntimeMethod::ProjectActivate,
            ProjectActivateRequest {
                project_id: project_id.to_owned(),
            },
        ),
        project_mutation(
            RuntimeMethod::ProjectRename,
            ProjectRenameRequest {
                project_id: project_id.to_owned(),
                name: "renamed".to_owned(),
            },
        ),
        project_mutation(
            RuntimeMethod::ProjectReorder,
            ProjectReorderRequest {
                project_id: project_id.to_owned(),
                new_index: 2,
            },
        ),
        project_mutation(
            RuntimeMethod::ProjectRemove,
            ProjectRemoveRequest {
                project_id: project_id.to_owned(),
            },
        ),
    ] {
        let encoded = serde_json::to_value(&request).unwrap();
        let parsed = parse_client_request(encoded.to_string().as_bytes()).unwrap();
        match parsed {
            ClientRequest::Runtime(parsed) => assert_eq!(parsed.method, request.method),
            other => panic!("项目写请求必须解析为 Runtime 请求: {other:?}"),
        }
    }
    let list = RuntimeRequest::new(
        JsonRpcId::Number(21),
        RuntimeMethod::ProjectList,
        EmptyParams {},
    );
    assert!(parse_client_request(&serde_json::to_vec(&list).unwrap()).is_ok());

    for invalid in [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.activate","params":{"client_id":"","mutation_id":"m","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.activate","params":{"client_id":"c","mutation_id":"","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.activate","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"project_id":""}}}),
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.rename","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","name":" "}}}),
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.reorder","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","new_index":0,"extra":1}}}),
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"project.list","params":{"extra":1}}),
    ] {
        assert!(
            parse_client_request(invalid.to_string().as_bytes()).is_err(),
            "非法项目请求必须拒绝: {invalid}"
        );
    }
}

#[test]
fn panel_methods_validate_layout_identities_and_bounds() {
    let view = r#"{"kind":"terminal","project_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b","terminal_id":"018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d","version":1}"#;
    let group = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    for request in [
        project_mutation(
            RuntimeMethod::PanelGroupSplit,
            PanelGroupSplitRequest {
                view: view.to_owned(),
                target_group: group.to_owned(),
                direction: SplitDirectionWire::LeftRight,
                ratio: 0.5,
                viewport: ViewportSize {
                    width: 800.0,
                    height: 600.0,
                },
            },
        ),
        project_mutation(
            RuntimeMethod::PanelGroupMerge,
            PanelGroupMergeRequest {
                source_group: group.to_owned(),
                target_group: group.to_owned(),
            },
        ),
        project_mutation(
            RuntimeMethod::PanelGroupResize,
            PanelGroupResizeRequest {
                path: vec![true, false],
                ratio: 0.25,
                viewport: ViewportSize {
                    width: 800.0,
                    height: 600.0,
                },
            },
        ),
        project_mutation(
            RuntimeMethod::PanelActivate,
            PanelActivateRequest {
                view: view.to_owned(),
            },
        ),
        project_mutation(
            RuntimeMethod::PanelMove,
            PanelMoveRequest {
                view: view.to_owned(),
                target_group: group.to_owned(),
                target_index: 1,
            },
        ),
    ] {
        let encoded = serde_json::to_value(&request).unwrap();
        let parsed = parse_client_request(encoded.to_string().as_bytes()).unwrap();
        match parsed {
            ClientRequest::Runtime(parsed) => assert_eq!(parsed.method, request.method),
            other => panic!("面板写请求必须解析为 Runtime 请求: {other:?}"),
        }
    }

    for invalid in [
        // 空视图
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"panel.activate","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"view":""}}}),
        // ratio 越界
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"panel_group.split","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"view":"v","target_group":"g","direction":"left_right","ratio":1.5,"viewport":{"width":800.0,"height":600.0}}}}),
        // viewport 非正
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"panel_group.resize","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"path":[true],"ratio":0.5,"viewport":{"width":0.0,"height":600.0}}}}),
        // path 为空
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"panel_group.resize","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"path":[],"ratio":0.5,"viewport":{"width":800.0,"height":600.0}}}}),
        // 未知方向
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"panel_group.split","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"view":"v","target_group":"g","direction":"diagonal","ratio":0.5,"viewport":{"width":800.0,"height":600.0}}}}),
    ] {
        assert!(
            parse_client_request(invalid.to_string().as_bytes()).is_err(),
            "非法面板请求必须拒绝: {invalid}"
        );
    }
}

#[test]
fn terminal_methods_validate_generation_and_payloads() {
    let terminal = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    for request in [
        project_mutation(
            RuntimeMethod::TerminalCreate,
            TerminalCreateRequest {
                project_root: None,
                cwd: Some("D:/work/a".to_owned()),
            },
        ),
        project_mutation(
            RuntimeMethod::TerminalInput,
            TerminalInputRequest {
                terminal_id: terminal.to_owned(),
                generation: 1,
                data: "echo hi".to_owned(),
            },
        ),
        project_mutation(
            RuntimeMethod::TerminalExecute,
            TerminalExecuteRequest {
                terminal_id: terminal.to_owned(),
                generation: 2,
                command: "cargo test".to_owned(),
            },
        ),
    ] {
        let encoded = serde_json::to_value(&request).unwrap();
        let parsed = parse_client_request(encoded.to_string().as_bytes()).unwrap();
        match parsed {
            ClientRequest::Runtime(parsed) => assert_eq!(parsed.method, request.method),
            other => panic!("终端写请求必须解析为 Runtime 请求: {other:?}"),
        }
    }

    for invalid in [
        // 空白终端身份
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.input","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"","generation":1,"data":"ls"}}}),
        // 代次为零
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.input","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":0,"data":"ls"}}}),
        // 空原始输入
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.input","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":1,"data":""}}}),
        // 未知字段
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.input","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":1,"data":"ls","bytes":"ls"}}}),
        // 空命令
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.execute","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":1,"command":" "}}}),
        // 多行命令
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.execute","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":1,"command":"ls\nwhoami"}}}),
        // 回车与控制字符
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.execute","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"terminal_id":"t","generation":1,"command":"ls\r"}}}),
        // 空路径不能伪装成显式目标
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"terminal.create","params":{"client_id":"c","mutation_id":"m","expected_revision":null,"request":{"project_root":"  "}}}),
    ] {
        assert!(
            parse_client_request(invalid.to_string().as_bytes()).is_err(),
            "非法终端请求必须拒绝: {invalid}"
        );
    }
}

#[test]
fn params_hash_is_stable_and_json_serialization_dependent() {
    let a = serde_json::json!({"z": 1, "a": {"n": [1, 2]}});
    let b = serde_json::json!({"a": {"n": [1, 2]}, "z": 1});
    assert_eq!(params_sha256(&a), params_sha256(&b), "对象键序不得影响散列");
    assert_ne!(
        params_sha256(&a),
        params_sha256(&serde_json::json!({"z": 2, "a": {"n": [1, 2]}}))
    );
}

#[test]
fn operation_resource_round_trips_terminal_states() {
    for status in OperationStatus::ALL {
        let resource = OperationResource {
            operation_id: "operation-a".to_owned(),
            status,
            result: status
                .eq(&OperationStatus::Succeeded)
                .then(|| serde_json::json!({"ok": true})),
            error: matches!(
                status,
                OperationStatus::Failed
                    | OperationStatus::Cancelled
                    | OperationStatus::OutcomeUnknown
            )
            .then(|| serde_json::json!({"code": "decision_required"})),
        };
        let encoded = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            serde_json::from_value::<OperationResource>(encoded.clone()).unwrap(),
            resource
        );
    }
    let encoded = serde_json::to_value(OperationResource {
        operation_id: "operation-a".to_owned(),
        status: OperationStatus::OutcomeUnknown,
        result: None,
        error: Some(serde_json::json!({"code": "side_effect_unknown"})),
    })
    .unwrap();
    assert_eq!(encoded["status"], "outcome_unknown");
}

#[test]
fn describe_lists_the_method_registry_with_contract_hashes() {
    let describe = DescribeResponse::current("protocol-hash", "schema-hash");
    assert_eq!(describe.protocol_sha256, "protocol-hash");
    assert_eq!(describe.schema_sha256, "schema-hash");
    let names: Vec<&str> = describe
        .methods
        .iter()
        .map(|method| method.method.as_str())
        .collect();
    assert_eq!(
        names,
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
            "terminal.create",
            "terminal.input",
            "terminal.execute",
            "instance.handover"
        ]
    );
    let runtime_describe = describe
        .methods
        .iter()
        .find(|method| method.method == "runtime.describe")
        .unwrap();
    assert_eq!(runtime_describe.scopes, [ControlScope::Runtime]);
    assert_eq!(runtime_describe.kind, MethodKind::Query);
    assert!(runtime_describe.idempotent);
    let encoded = serde_json::to_value(&describe).unwrap();
    assert_eq!(encoded["methods"][1]["kind"], "query");
    assert_eq!(encoded["methods"][1]["scopes"][0], "runtime");
    assert_eq!(encoded["methods"][1]["idempotent"], true);
}

fn sample_snapshot() -> SnapshotResponse {
    SnapshotResponse {
        store_epoch: "store".to_owned(),
        stream_epoch: "stream".to_owned(),
        revision: 3,
        resources: SnapshotResources {
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
                shell: "C:/Program Files/PowerShell/7/pwsh.exe".to_owned(),
                title: Some("build".to_owned()),
                run_intent: "launch".to_owned(),
            }],
            documents: vec![DocumentResource {
                document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e".to_owned(),
                path: "D:/work/a/README.md".to_owned(),
            }],
        },
    }
}

#[test]
fn snapshot_response_round_trips_and_rejects_unknown_fields() {
    let snapshot = sample_snapshot();
    let encoded = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_value::<SnapshotResponse>(encoded.clone()).unwrap(),
        snapshot
    );
    let mut with_extra = encoded;
    with_extra["extra"] = serde_json::Value::from(1);
    assert!(serde_json::from_value::<SnapshotResponse>(with_extra).is_err());
}

#[cfg(feature = "schema")]
fn packaged_schema() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../assets/runtime-api-v2.schema.json"
    ))
    .unwrap()
}

#[cfg(feature = "schema")]
#[test]
fn generated_schema_matches_the_packaged_asset() {
    assert_eq!(crate::schema::runtime_api_schema_json(), packaged_schema());
}

#[cfg(feature = "schema")]
#[test]
fn schema_generator_recreates_missing_output_file() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("fresh-schema.json");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_runtime-schema"))
        .arg(&destination)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(destination).unwrap(),
        crate::schema::runtime_api_schema_json()
    );
}

#[cfg(feature = "schema")]
#[test]
fn schema_generator_replaces_existing_output_file() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("existing-schema.json");
    std::fs::write(&destination, "old schema").unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_runtime-schema"))
        .arg(&destination)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(destination).unwrap(),
        crate::schema::runtime_api_schema_json()
    );
}

#[cfg(feature = "schema")]
#[test]
fn schema_generator_failure_preserves_existing_output_file() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("existing-schema.json");
    std::fs::write(&destination, "old schema").unwrap();
    std::fs::create_dir(destination.with_extension("schema.tmp")).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_runtime-schema"))
        .arg(&destination)
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(std::fs::read_to_string(destination).unwrap(), "old schema");
}

#[cfg(feature = "schema")]
#[test]
fn schema_accepts_discovery_and_rejects_sensitive_fields() {
    let schema: serde_json::Value = serde_json::from_str(&packaged_schema()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let discovery = serde_json::to_value(discovery_record()).unwrap();
    validator.validate(&discovery).unwrap();

    let mut with_token = discovery;
    with_token["token"] = serde_json::Value::String("sk-test".to_owned());
    assert!(validator.validate(&with_token).is_err());
}

#[cfg(feature = "schema")]
#[test]
fn schema_accepts_real_requests_and_rejects_unknown_methods() {
    let schema: serde_json::Value = serde_json::from_str(&packaged_schema()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let hello: serde_json::Value = serde_json::to_value(hello_request()).unwrap();
    validator.validate(&hello).unwrap();
    for params in [
        HandoverParams::Focus,
        HandoverParams::OpenProject {
            project_root: "D:/work/a".to_owned(),
        },
        HandoverParams::OpenFile {
            path: "D:/work/a/README.md".to_owned(),
        },
        HandoverParams::NewTerminal {
            project_root: Some("D:/work/a".to_owned()),
            cwd: Some("D:/work/a/src".to_owned()),
        },
    ] {
        let handover: serde_json::Value = serde_json::to_value(JsonRpcRequest::new(
            JsonRpcId::Number(1),
            HandoverMethod::InstanceHandover,
            params,
        ))
        .unwrap();
        validator.validate(&handover).unwrap();
    }
    let hello_response = JsonRpcResponse::result(
        JsonRpcId::Number(1),
        HelloResponse::new("store", "stream", 3, "protocol", "schema"),
    );
    validator
        .validate(&serde_json::to_value(hello_response).unwrap())
        .unwrap();
    let describe_request = serde_json::to_value(runtime_request(RuntimeMethod::Describe)).unwrap();
    validator.validate(&describe_request).unwrap();
    let snapshot_request = serde_json::to_value(runtime_request(RuntimeMethod::Snapshot)).unwrap();
    validator.validate(&snapshot_request).unwrap();
    let describe_response = JsonRpcResponse::result(
        JsonRpcId::Number(1),
        DescribeResponse::current("protocol", "schema"),
    );
    validator
        .validate(&serde_json::to_value(describe_response).unwrap())
        .unwrap();
    let snapshot_response = JsonRpcResponse::result(JsonRpcId::Number(1), sample_snapshot());
    validator
        .validate(&serde_json::to_value(snapshot_response).unwrap())
        .unwrap();
    let operation_get = serde_json::to_value(operation_get_request()).unwrap();
    validator.validate(&operation_get).unwrap();
    let operation_wait = serde_json::to_value(operation_wait_request(1_000)).unwrap();
    validator.validate(&operation_wait).unwrap();
    let operation_response = JsonRpcResponse::result(
        JsonRpcId::Number(1),
        OperationResource {
            operation_id: "operation-a".to_owned(),
            status: OperationStatus::OutcomeUnknown,
            result: None,
            error: Some(serde_json::json!({"code": "side_effect_unknown"})),
        },
    );
    validator
        .validate(&serde_json::to_value(operation_response).unwrap())
        .unwrap();
    let handover_response =
        JsonRpcResponse::result(JsonRpcId::Number(1), HandoverResponse::accepted());
    validator
        .validate(&serde_json::to_value(handover_response).unwrap())
        .unwrap();
    let rejected_handover = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {"accepted": false}
    });
    assert!(validator.validate(&rejected_handover).is_err());
    let error_response = JsonRpcResponse::<HelloResponse>::error(
        JsonRpcId::Number(1),
        JsonRpcError::new(-32001, "not ready", None),
    );
    validator
        .validate(&serde_json::to_value(error_response).unwrap())
        .unwrap();

    let mut wrong_jsonrpc = hello.clone();
    wrong_jsonrpc["jsonrpc"] = serde_json::Value::String("1.0".to_owned());
    assert!(validator.validate(&wrong_jsonrpc).is_err());
    let mut wrong_protocol = hello.clone();
    wrong_protocol["params"]["protocol_version"] = serde_json::Value::from(1);
    assert!(validator.validate(&wrong_protocol).is_err());
    let mut unknown = hello;
    unknown["method"] = serde_json::Value::String("runtime.unknown".to_owned());
    assert!(validator.validate(&unknown).is_err());
}
