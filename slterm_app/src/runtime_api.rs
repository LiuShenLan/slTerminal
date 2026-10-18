//! Runtime 控制面(04 篇锚点):版本化信封 + loopback JSON Lines 传输。
//!
//! 单一状态权威(`RuntimeHub`)与投影入账归 M7.2,方法族/派发原语归 M7.3,
//! CLI 与 env 契约归 M7.4,单实例移交归 M7.5;本步只落协议信封与传输主链。

mod server;
mod transport;

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 协议名:01 篇禁名门禁的「协议兼容名」语境,旧品牌名不得回流。
pub const PROTOCOL_NAME: &str = "slterm.runtime";
/// 当前协议版本;不匹配即 `protocol_version_mismatch`,不做静默降级。
pub const PROTOCOL_VERSION: u16 = 1;
/// 本运行时支持的版本集合(版本协商时随错误详情返回)。
pub const SUPPORTED_VERSIONS: &[u16] = &[PROTOCOL_VERSION];

/// 控制面客户端 connect 超时。
#[allow(dead_code)] // 消费点 = M7.4 CLI 客户端 `client_stream`。
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
/// 控制面连接读写超时;服务端在 `handle_connection` 同时设置读/写。
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// 请求首行字节上限;超限返回 `request_too_large`,不静默截断。
const MAX_REQUEST_BYTES: usize = 128 * 1024;
/// 同时在线连接上限;拖死连接不饿死新客户端。
const MAX_CLIENTS: usize = 64;

/// 请求信封:JSON Lines 首行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApiRequest {
    pub protocol: String,
    pub version: u16,
    /// `"<pid>-<全局单调序>"`:响应身份校验的依据。
    pub id: String,
    pub token: String,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: Value,
}

impl ApiRequest {
    /// 构造一条本进程请求。
    #[allow(dead_code)] // 消费点 = M7.4 CLI 客户端。
    pub(crate) fn new(token: String, method: impl Into<String>, params: Value) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
        let sequence = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            protocol: PROTOCOL_NAME.to_owned(),
            version: PROTOCOL_VERSION,
            id: format!("{}-{sequence}", std::process::id()),
            token,
            method: method.into(),
            params,
        }
    }
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

/// 响应信封:`result` 显式 null 是有效结果,与「缺 result 字段」区分。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiResponse {
    pub protocol: String,
    pub version: u16,
    pub id: String,
    pub ok: bool,
    #[serde(
        default,
        deserialize_with = "present_json",
        skip_serializing_if = "Option::is_none"
    )]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

/// 保留显式 null 与「回复缺少 result 字段」的区别。
fn present_json<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(input).map(Some)
}

impl ApiResponse {
    #[allow(dead_code)] // 消费点 = M7.2 方法专链;本步仅有失败响应路径。
    pub(crate) fn success(id: impl Into<String>, result: Value) -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_owned(),
            version: PROTOCOL_VERSION,
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub(crate) fn failure(id: impl Into<String>, error: ApiError) -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_owned(),
            version: PROTOCOL_VERSION,
            id: id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }

    /// 客户端响应身份校验:protocol/version/id 逐字段一致,ok 与 result/error 互斥。
    #[allow(dead_code)] // 消费点 = M7.4 `decode_plugin_response`。
    pub(crate) fn verify_identity(&self, request: &ApiRequest) -> Result<(), ApiError> {
        let consistent = if self.ok {
            self.result.is_some() && self.error.is_none()
        } else {
            self.error.is_some() && self.result.is_none()
        };
        if self.protocol == PROTOCOL_NAME
            && self.version == PROTOCOL_VERSION
            && self.id == request.id
            && consistent
        {
            Ok(())
        } else {
            Err(ApiError::new(
                "invalid_runtime_response",
                "runtime response identity or envelope mismatch",
            ))
        }
    }
}

/// 事件信封:`runtime.snapshot` 逐行推送;载荷由订阅链注入,
/// M7.2 以 `RuntimeSnapshot` 实例化。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiEvent<T> {
    pub protocol: String,
    pub version: u16,
    pub event: String,
    pub revision: u64,
    pub data: T,
}

impl<T> ApiEvent<T> {
    #[allow(dead_code)] // 消费点 = M7.2 `subscribe_connection`。
    pub(crate) fn new(event: impl Into<String>, revision: u64, data: T) -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_owned(),
            version: PROTOCOL_VERSION,
            event: event.into(),
            revision,
            data,
        }
    }
}

/// 错误对象:错误码机器可读,message 供人读,details 携带结构化补充。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ApiError {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub(crate) fn details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    #[allow(dead_code)] // 消费点 = M7.2/M7.3 参数校验。
    pub(crate) fn invalid_params(message: impl Into<String>) -> Self {
        Self::new("invalid_params", message)
    }
}

/// 壳事件回调。ATTACH 为 fire-and-forget(单窗语义 = 聚焦现有窗口);
/// 写命令派发(`Control`)随 M7.3 扩展——同一闭包通道,不另起事件通道。
#[derive(Clone)]
pub enum RuntimeCallback {
    Attach,
}

/// 单形态事件汇:闭包桥进壳事件队列(壳侧驱动方归 M3/M7.5 接线)。
pub(crate) type EventSink = Arc<dyn Fn(RuntimeCallback) + Send + Sync>;

#[cfg(test)]
mod runtime_api_tests {
    use super::*;
    use serde_json::json;

    fn test_request() -> ApiRequest {
        ApiRequest::new("sk-test-token".into(), "pane.read", json!({ "pane_id": 1 }))
    }

    #[test]
    fn api_request_round_trip_defaults_params_to_empty_object() {
        let raw = json!({
            "protocol": PROTOCOL_NAME,
            "version": PROTOCOL_VERSION,
            "id": "4242-7",
            "token": "sk-test-token",
            "method": "runtime.describe",
        });
        let request: ApiRequest = serde_json::from_value(raw).expect("request without params");
        assert_eq!(request.params, json!({}));
        assert_eq!(request.protocol, PROTOCOL_NAME);
        assert_eq!(request.version, PROTOCOL_VERSION);

        let encoded = serde_json::to_string(&request).expect("serialize request");
        let decoded: ApiRequest = serde_json::from_str(&encoded).expect("deserialize request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn api_request_rejects_unknown_fields() {
        let raw = json!({
            "protocol": PROTOCOL_NAME,
            "version": PROTOCOL_VERSION,
            "id": "4242-8",
            "token": "sk-test-token",
            "method": "runtime.describe",
            "extra": true,
        });
        assert!(serde_json::from_value::<ApiRequest>(raw).is_err());
    }

    #[test]
    fn api_response_distinguishes_explicit_null_from_missing_result() {
        let explicit_null: ApiResponse = serde_json::from_value(json!({
            "protocol": PROTOCOL_NAME,
            "version": PROTOCOL_VERSION,
            "id": "4242-9",
            "ok": true,
            "result": null,
        }))
        .expect("explicit null result");
        assert_eq!(explicit_null.result, Some(Value::Null));
        assert_eq!(
            serde_json::to_value(&explicit_null).expect("serialize null result")["result"],
            Value::Null
        );

        let missing: ApiResponse = serde_json::from_value(json!({
            "protocol": PROTOCOL_NAME,
            "version": PROTOCOL_VERSION,
            "id": "4242-10",
            "ok": false,
            "error": { "code": "timeout", "message": "no answer" },
        }))
        .expect("missing result field");
        assert_eq!(missing.result, None);
        assert!(
            serde_json::to_value(&missing)
                .expect("serialize missing result")
                .get("result")
                .is_none()
        );
    }

    #[test]
    fn api_response_identity_requires_field_consistency() {
        let request = test_request();
        let response = ApiResponse::success(request.id.clone(), json!({ "ok": true }));
        assert!(response.verify_identity(&request).is_ok());

        let mut wrong_id = response.clone();
        wrong_id.id = "other-id".to_owned();
        assert_eq!(
            wrong_id
                .verify_identity(&request)
                .expect_err("id mismatch")
                .code,
            "invalid_runtime_response"
        );

        let mut wrong_protocol = response.clone();
        wrong_protocol.protocol = "other.runtime".to_owned();
        assert!(wrong_protocol.verify_identity(&request).is_err());

        let mut wrong_version = response.clone();
        wrong_version.version = PROTOCOL_VERSION + 1;
        assert!(wrong_version.verify_identity(&request).is_err());

        let mut failure_without_error = response.clone();
        failure_without_error.ok = false;
        assert!(failure_without_error.verify_identity(&request).is_err());

        let mut success_with_error = response.clone();
        success_with_error.error = Some(ApiError::new("timeout", "unexpected"));
        assert!(success_with_error.verify_identity(&request).is_err());

        let mut failure_with_result = response;
        failure_with_result.ok = false;
        failure_with_result.error = Some(ApiError::new("timeout", "no answer"));
        assert!(failure_with_result.verify_identity(&request).is_err());
    }

    #[test]
    fn api_event_round_trip_carries_revision_and_payload() {
        let event = ApiEvent::new("runtime.snapshot", 7, json!({ "revision": 7 }));
        let raw = serde_json::to_value(&event).expect("serialize event");
        assert_eq!(raw["protocol"], PROTOCOL_NAME);
        assert_eq!(raw["version"], PROTOCOL_VERSION);
        assert_eq!(raw["event"], "runtime.snapshot");
        assert_eq!(raw["revision"], 7);
        let decoded: ApiEvent<Value> = serde_json::from_value(raw).expect("deserialize event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn api_error_details_stay_optional_on_the_wire() {
        let plain = ApiError::new("invalid_params", "bad parameter");
        assert!(
            serde_json::to_value(&plain)
                .expect("serialize plain error")
                .get("details")
                .is_none()
        );
        let detailed = plain.details(json!({ "supported_versions": SUPPORTED_VERSIONS }));
        assert_eq!(
            serde_json::to_value(&detailed).expect("serialize detailed error")["details"]["supported_versions"],
            json!([PROTOCOL_VERSION])
        );
        assert_eq!(ApiError::invalid_params("bad").code, "invalid_params");
    }

    #[test]
    fn protocol_schema_matches_envelope_constants() {
        let schema: Value =
            serde_json::from_str(include_str!("../../docs/runtime-api-v1.schema.json"))
                .expect("protocol schema must stay valid JSON");
        assert_eq!(schema["properties"]["protocol"]["const"], PROTOCOL_NAME);
        assert_eq!(schema["properties"]["version"]["const"], PROTOCOL_VERSION);
        assert_eq!(
            schema["$defs"]["request"]["properties"]["protocol"]["const"],
            PROTOCOL_NAME
        );
        assert_eq!(
            schema["$defs"]["response"]["properties"]["version"]["const"],
            PROTOCOL_VERSION
        );
    }
}
