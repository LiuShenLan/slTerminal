//! Runtime v2 请求、响应与 JSON Lines 校验。

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

const REQUEST_BUDGET: usize = 128 * 1024;

/// JSON-RPC 请求标识；通知与空标识不属于本协议。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
}

/// 带封闭方法类型的 JSON-RPC 请求信封。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest<M, P> {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["2.0"])))]
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: M,
    pub params: P,
}

impl<M, P> JsonRpcRequest<M, P> {
    pub fn new(id: JsonRpcId, method: M, params: P) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            method,
            params,
        }
    }

    fn has_valid_envelope(&self) -> bool {
        self.jsonrpc == "2.0"
    }
}

/// 首包只接受 Runtime hello。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum HelloMethod {
    #[serde(rename = "runtime.hello")]
    RuntimeHello,
}

/// 单实例移交只接受 instance.handover。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum HandoverMethod {
    #[serde(rename = "instance.handover")]
    InstanceHandover,
}

/// Runtime 与 handover 使用独立授权的同名作用域。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ControlScope {
    Runtime,
    Handover,
}

/// 线缆凭据只参与序列化与认证比较，调试输出永远脱敏。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Credential(String);

impl Credential {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Credential([redacted])")
    }
}

/// runtime.hello 的认证与协议协商参数。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HelloParams {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = [2])))]
    pub protocol_version: u32,
    pub client_id: String,
    pub credential: Credential,
    pub scope: ControlScope,
}

/// 第二实例结构化意图；路径授权由应用层 PathPolicy 复核。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HandoverParams {
    Focus,
    OpenProject {
        project_root: String,
    },
    OpenFile {
        path: String,
    },
    NewTerminal {
        project_root: Option<String>,
        cwd: Option<String>,
    },
}

impl HandoverParams {
    fn has_valid_paths(&self) -> bool {
        fn present(value: &str) -> bool {
            !value.is_empty()
        }
        fn optional_present(value: &Option<String>) -> bool {
            value.as_deref().is_none_or(present)
        }

        match self {
            Self::Focus => true,
            Self::OpenProject { project_root } => present(project_root),
            Self::OpenFile { path } => present(path),
            Self::NewTerminal { project_root, cwd } => {
                optional_present(project_root) && optional_present(cwd)
            }
        }
    }
}

pub type HelloRequest = JsonRpcRequest<HelloMethod, HelloParams>;
pub type HandoverRequest = JsonRpcRequest<HandoverMethod, HandoverParams>;

/// 当前已实现的客户端请求；新增方法必须显式扩展封闭集合。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ClientRequest {
    Hello(HelloRequest),
    Handover(HandoverRequest),
}

impl ClientRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let valid = match self {
            Self::Hello(request) => {
                request.has_valid_envelope()
                    && request.params.protocol_version == 2
                    && !request.params.client_id.trim().is_empty()
                    && !request.params.credential.is_empty()
            }
            Self::Handover(request) => {
                request.has_valid_envelope() && request.params.has_valid_paths()
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::InvalidRequest)
        }
    }
}

/// 当前方法注册表同时用于 hello 能力摘要与 schema 覆盖检查。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlMethod {
    RuntimeHello,
    InstanceHandover,
}

impl ControlMethod {
    pub const ALL: [Self; 2] = [Self::RuntimeHello, Self::InstanceHandover];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeHello => "runtime.hello",
            Self::InstanceHandover => "instance.handover",
        }
    }
}

/// 当前客户端可调用的方法集合。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub methods: BTreeSet<String>,
}

/// hello 握手返回的世代、事件起点与合同散列。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HelloResponse {
    pub store_epoch: String,
    pub stream_epoch: String,
    pub revision: u64,
    pub protocol_sha256: String,
    pub schema_sha256: String,
    pub capabilities: Capabilities,
}

impl HelloResponse {
    pub fn new(
        store_epoch: impl Into<String>,
        stream_epoch: impl Into<String>,
        revision: u64,
        protocol_sha256: impl Into<String>,
        schema_sha256: impl Into<String>,
    ) -> Self {
        Self {
            store_epoch: store_epoch.into(),
            stream_epoch: stream_epoch.into(),
            revision,
            protocol_sha256: protocol_sha256.into(),
            schema_sha256: schema_sha256.into(),
            capabilities: Capabilities {
                methods: ControlMethod::ALL
                    .into_iter()
                    .map(|method| method.as_str().to_owned())
                    .collect(),
            },
        }
    }
}

/// 结构化领域错误码，不与 JSON-RPC 数值码混用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DomainErrorCode {
    NotReady,
    Unauthorized,
    QueueFull,
    Closing,
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonRpcErrorData {
    pub code: DomainErrorCode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<JsonRpcErrorData>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>, data: Option<JsonRpcErrorData>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }
}

/// result 与 error 在类型层互斥的 JSON-RPC 响应。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged, deny_unknown_fields)]
pub enum JsonRpcResponse<T> {
    Result {
        jsonrpc: String,
        id: JsonRpcId,
        result: T,
    },
    Error {
        jsonrpc: String,
        id: JsonRpcId,
        error: JsonRpcError,
    },
}

impl<T> JsonRpcResponse<T> {
    pub fn result(id: JsonRpcId, value: T) -> Self {
        Self::Result {
            jsonrpc: "2.0".to_owned(),
            id,
            result: value,
        }
    }

    pub fn error(id: JsonRpcId, error: JsonRpcError) -> Self {
        Self::Error {
            jsonrpc: "2.0".to_owned(),
            id,
            error,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        let jsonrpc = match self {
            Self::Result { jsonrpc, .. } | Self::Error { jsonrpc, .. } => jsonrpc,
        };
        if jsonrpc == "2.0" {
            Ok(())
        } else {
            Err(ProtocolError::InvalidRequest)
        }
    }
}

/// hello 成功响应的静态形状，与通用错误响应严格分离。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonRpcResult<T> {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["2.0"])))]
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub result: T,
}

/// 通用错误响应的静态形状，不包含任何 result 字段。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonRpcErrorResponse {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["2.0"])))]
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub error: JsonRpcError,
}

/// 当前请求与响应的根消息集合，schema 与线缆 DTO 共用此边界。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ControlMessage {
    Hello(HelloRequest),
    Handover(HandoverRequest),
    HelloResponse(JsonRpcResult<HelloResponse>),
    Error(JsonRpcErrorResponse),
}

/// 解析失败只返回封闭分类，不回显请求行、参数或凭据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    RequestTooLarge,
    InvalidUtf8,
    InvalidJson,
    BatchUnsupported,
    InvalidRequest,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RequestTooLarge => "Runtime 请求超过大小上限",
            Self::InvalidUtf8 => "Runtime 请求不是有效 UTF-8",
            Self::InvalidJson => "Runtime 请求不是有效 JSON-RPC 对象",
            Self::BatchUnsupported => "Runtime 不支持 JSON-RPC batch",
            Self::InvalidRequest => "Runtime 请求参数或版本无效",
        })
    }
}

impl std::error::Error for ProtocolError {}

/// 解析单个 JSON Lines 请求并执行结构、版本与参数校验。
pub fn parse_client_request(input: &[u8]) -> Result<ClientRequest, ProtocolError> {
    if input.len() > REQUEST_BUDGET {
        return Err(ProtocolError::RequestTooLarge);
    }
    let text = std::str::from_utf8(input).map_err(|_| ProtocolError::InvalidUtf8)?;
    if text.trim_start().starts_with('[') {
        return Err(ProtocolError::BatchUnsupported);
    }
    let request: ClientRequest =
        serde_json::from_str(text).map_err(|_| ProtocolError::InvalidJson)?;
    request.validate()?;
    Ok(request)
}
