//! Runtime hello 与单实例移交的线缆合同测试。

use crate::protocol::{
    ClientRequest, ControlMessage, ControlMethod, ControlScope, Credential, DiscoveryRecord,
    DomainErrorCode, HandoverMethod, HandoverParams, HandoverRequest, HelloMethod, HelloParams,
    HelloRequest, HelloResponse, JsonRpcError, JsonRpcErrorData, JsonRpcId, JsonRpcRequest,
    JsonRpcResponse, ProtocolError, parse_client_request,
};

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
