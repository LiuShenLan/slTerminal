//! Runtime v2 请求、响应与 JSON Lines 校验。

use std::collections::BTreeSet;
use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

const REQUEST_BUDGET: usize = 128 * 1024;

/// 本机控制面发现记录；认证材料只经凭据门面独立获取。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscoveryRecord {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = [2])))]
    pub protocol_version: u32,
    pub endpoint: SocketAddr,
    pub run_id: String,
    pub process_id: u32,
    pub process_created_at_ms: u64,
}

impl DiscoveryRecord {
    /// 拒绝非 loopback、不可连接端口与无法核对进程身份的记录。
    pub fn validate(&self) -> Result<(), InvalidDiscovery> {
        let valid = self.protocol_version == 2
            && self.endpoint.ip().is_loopback()
            && self.endpoint.port() != 0
            && !self.run_id.trim().is_empty()
            && self.process_id != 0
            && self.process_created_at_ms != 0;
        if valid { Ok(()) } else { Err(InvalidDiscovery) }
    }
}

/// 发现记录不符合当前版本或本机运行身份合同。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDiscovery;

impl fmt::Display for InvalidDiscovery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Runtime 发现记录无效")
    }
}

impl std::error::Error for InvalidDiscovery {}

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

/// 认证后的 Runtime 方法；写方法族由后续切片逐个接入。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum RuntimeMethod {
    #[serde(rename = "runtime.describe")]
    Describe,
    #[serde(rename = "runtime.snapshot")]
    Snapshot,
    #[serde(rename = "runtime.operation_get")]
    OperationGet,
    #[serde(rename = "runtime.operation_wait")]
    OperationWait,
    #[serde(rename = "events.subscribe")]
    EventsSubscribe,
    #[serde(rename = "events.unsubscribe")]
    EventsUnsubscribe,
    #[serde(rename = "project.list")]
    ProjectList,
    #[serde(rename = "project.activate")]
    ProjectActivate,
    #[serde(rename = "project.rename")]
    ProjectRename,
    #[serde(rename = "project.reorder")]
    ProjectReorder,
    #[serde(rename = "project.remove")]
    ProjectRemove,
    #[serde(rename = "panel_group.split")]
    PanelGroupSplit,
    #[serde(rename = "panel_group.merge")]
    PanelGroupMerge,
    #[serde(rename = "panel_group.resize")]
    PanelGroupResize,
    #[serde(rename = "panel.activate")]
    PanelActivate,
    #[serde(rename = "panel.move")]
    PanelMove,
    #[serde(rename = "panel.open_settings")]
    PanelOpenSettings,
    #[serde(rename = "terminal.create")]
    TerminalCreate,
}

/// 新建终端：Shell 固定为应用默认，只开放目标项目与工作目录选择。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalCreateRequest {
    pub project_root: Option<String>,
    pub cwd: Option<String>,
}

pub type TerminalCreateParams = MutationParams<TerminalCreateRequest>;

/// 封闭拆分方向；与布局树的轴向一一对应。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SplitDirectionWire {
    LeftRight,
    TopBottom,
}

/// 布局操作使用的逻辑像素尺寸。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ViewportSize {
    pub width: f32,
    pub height: f32,
}

/// 从源视图拆分新组；`view` 是规范 ViewKey JSON 文本。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelGroupSplitRequest {
    pub view: String,
    pub target_group: String,
    pub direction: SplitDirectionWire,
    pub ratio: f32,
    pub viewport: ViewportSize,
}

/// 合并两个稳定组；顺序由布局转换裁定。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelGroupMergeRequest {
    pub source_group: String,
    pub target_group: String,
}

/// 调整分割比例；`path` 是从根到分割节点的布尔路径。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelGroupResizeRequest {
    pub path: Vec<bool>,
    pub ratio: f32,
    pub viewport: ViewportSize,
}

/// 激活视图；`view` 是规范 ViewKey JSON 文本。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelActivateRequest {
    pub view: String,
}

/// 将视图移动到目标组指定位置。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelMoveRequest {
    pub view: String,
    pub target_group: String,
    pub target_index: u64,
}

pub type PanelGroupSplitParams = MutationParams<PanelGroupSplitRequest>;
pub type PanelGroupMergeParams = MutationParams<PanelGroupMergeRequest>;
pub type PanelGroupResizeParams = MutationParams<PanelGroupResizeRequest>;
pub type PanelActivateParams = MutationParams<PanelActivateRequest>;
pub type PanelMoveParams = MutationParams<PanelMoveRequest>;
/// 设置入口不接收参数；有项目时打开项目设置，无项目时打开应用浮层。
pub type PanelOpenSettingsParams = MutationParams<EmptyParams>;

/// 写请求信封：幂等键与客户端身份是必填，期望代次可选。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MutationParams<P> {
    pub client_id: String,
    pub mutation_id: String,
    pub expected_revision: Option<u64>,
    pub request: P,
}

impl<P> MutationParams<P> {
    fn has_valid_envelope(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.mutation_id.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectActivateRequest {
    pub project_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectRenameRequest {
    pub project_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectReorderRequest {
    pub project_id: String,
    pub new_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectRemoveRequest {
    pub project_id: String,
}

pub type ProjectActivateParams = MutationParams<ProjectActivateRequest>;
pub type ProjectRenameParams = MutationParams<ProjectRenameRequest>;
pub type ProjectReorderParams = MutationParams<ProjectReorderRequest>;
pub type ProjectRemoveParams = MutationParams<ProjectRemoveRequest>;

/// `project.list`：按导航顺序返回项目摘要；与 snapshot 的项目资源同构。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResource>,
}

/// 无参数的 Runtime 方法；未知字段一律拒绝。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

/// 按操作标识查询持久结果；operation.get 与 operation.wait 共用身份。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OperationQueryParams {
    pub operation_id: String,
}

/// 有界等待：超时缺省为服务端上限，显式值不得超过 600 秒。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OperationWaitParams {
    pub operation_id: String,
    pub timeout_ms: Option<u64>,
}

/// 事件续传三元组：世代与起点必须同时匹配事件环，否则要求重新快照。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventsSubscribeParams {
    pub store_epoch: String,
    pub stream_epoch: String,
    pub since_revision: u64,
}

/// 订阅确认：返回事件流身份与该订阅看到的当前末尾 revision。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventsSubscribedResponse {
    pub stream_epoch: String,
    pub revision: u64,
}

/// 取消订阅的确认；订阅身份绑定连接，不另行分配。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventsUnsubscribedResponse {}

/// 轻量事件：资源身份、代次与必要元数据；终端正文只经分页读取。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RuntimeEvent {
    /// 单调 revision；缺口只能以重新快照补偿。
    pub revision: u64,
    /// 封闭事件类型名。
    pub kind: String,
    /// 资源身份文本（稳定 ID 或规范 ViewKey JSON）。
    pub resource: String,
    /// 资源代次；旧代次事件不覆盖新代次状态。
    pub generation: u64,
}

/// 服务器到客户端的事件通知；客户端不得回复，连接只承载订阅事件。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventNotification {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["2.0"])))]
    pub jsonrpc: String,
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["events.event"])))]
    pub method: String,
    pub params: RuntimeEvent,
}

impl EventNotification {
    pub fn new(event: RuntimeEvent) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            method: "events.event".to_owned(),
            params: event,
        }
    }
}

/// Runtime 方法参数形状的 schema 汇总；运行时不参与分派。
#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
pub enum RuntimeParams {
    Empty(EmptyParams),
    OperationQuery(OperationQueryParams),
    OperationWait(OperationWaitParams),
    EventsSubscribe(EventsSubscribeParams),
    ProjectActivate(ProjectActivateParams),
    ProjectRename(ProjectRenameParams),
    ProjectReorder(ProjectReorderParams),
    ProjectRemove(ProjectRemoveParams),
    PanelGroupSplit(PanelGroupSplitParams),
    PanelGroupMerge(PanelGroupMergeParams),
    PanelGroupResize(PanelGroupResizeParams),
    PanelActivate(PanelActivateParams),
    PanelMove(PanelMoveParams),
    PanelOpenSettings(PanelOpenSettingsParams),
    TerminalCreate(TerminalCreateParams),
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

    /// 覆盖当前分配中的凭据字节；Drop 与 crate 内测试共用此入口。
    pub(crate) fn clear(&mut self) {
        // SAFETY: 只把既有字节改写为 NUL；NUL 是有效 UTF-8，且长度与容量不变。
        let bytes = unsafe { self.0.as_mut_vec() };
        for byte in bytes {
            // SAFETY: byte 来自存活 Vec 的可变迭代，指针对齐且对单字节写入有效。
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for Credential {
    fn drop(&mut self) {
        self.clear();
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

/// 认证后的 Runtime 请求：信封先携带方法名，参数再按方法二次解析。
///
/// 这样同构参数（例如同为 `{project_id}` 的两个写方法）不会在 untagged
/// 反序列化中互相吞并；业务语义只由 `method` 决定。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RuntimeRequest {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["2.0"])))]
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: RuntimeMethod,
    #[cfg_attr(feature = "schema", schemars(with = "RuntimeParams"))]
    pub params: serde_json::Value,
}

impl RuntimeRequest {
    pub fn new(id: JsonRpcId, method: RuntimeMethod, params: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            method,
            params: serde_json::to_value(params).expect("Runtime 参数必须可序列化"),
        }
    }

    fn has_valid_envelope(&self) -> bool {
        self.jsonrpc == "2.0"
    }

    /// 按方法把参数解析为封闭结构；未知字段由目标类型的 `deny_unknown_fields` 拒绝。
    pub fn parse<P: serde::de::DeserializeOwned>(&self) -> Option<P> {
        serde_json::from_value(self.params.clone()).ok()
    }
}

/// operation.wait 的等待上限与 transport 的请求等待一致。
pub const OPERATION_WAIT_LIMIT_MS: u64 = 600_000;

/// 方法名与参数形状必须同属一个封闭组合。
fn runtime_request_is_valid(request: &RuntimeRequest) -> bool {
    fn present(value: &str) -> bool {
        !value.trim().is_empty()
    }
    match request.method {
        RuntimeMethod::Describe | RuntimeMethod::Snapshot | RuntimeMethod::EventsUnsubscribe => {
            request.parse::<EmptyParams>().is_some()
        }
        RuntimeMethod::OperationGet => request
            .parse::<OperationQueryParams>()
            .is_some_and(|params| present(&params.operation_id)),
        RuntimeMethod::OperationWait => {
            request
                .parse::<OperationWaitParams>()
                .is_some_and(|params| {
                    present(&params.operation_id)
                        && params
                            .timeout_ms
                            .is_none_or(|timeout| (1..=OPERATION_WAIT_LIMIT_MS).contains(&timeout))
                })
        }
        RuntimeMethod::EventsSubscribe => request
            .parse::<EventsSubscribeParams>()
            .is_some_and(|params| present(&params.store_epoch) && present(&params.stream_epoch)),
        RuntimeMethod::ProjectList => request.parse::<EmptyParams>().is_some(),
        RuntimeMethod::ProjectActivate => {
            request
                .parse::<ProjectActivateParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope() && present(&params.request.project_id)
                })
        }
        RuntimeMethod::ProjectRename => {
            request
                .parse::<ProjectRenameParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope()
                        && present(&params.request.project_id)
                        && present(&params.request.name)
                })
        }
        RuntimeMethod::ProjectReorder => {
            request
                .parse::<ProjectReorderParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope() && present(&params.request.project_id)
                })
        }
        RuntimeMethod::ProjectRemove => {
            request
                .parse::<ProjectRemoveParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope() && present(&params.request.project_id)
                })
        }
        RuntimeMethod::PanelGroupSplit => {
            request
                .parse::<PanelGroupSplitParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && present(&request.view)
                        && present(&request.target_group)
                        && valid_ratio(request.ratio)
                        && valid_viewport(request.viewport)
                })
        }
        RuntimeMethod::PanelGroupMerge => {
            request
                .parse::<PanelGroupMergeParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope()
                        && present(&params.request.source_group)
                        && present(&params.request.target_group)
                })
        }
        RuntimeMethod::PanelGroupResize => {
            request
                .parse::<PanelGroupResizeParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && !request.path.is_empty()
                        && valid_ratio(request.ratio)
                        && valid_viewport(request.viewport)
                })
        }
        RuntimeMethod::PanelActivate => request
            .parse::<PanelActivateParams>()
            .is_some_and(|params| params.has_valid_envelope() && present(&params.request.view)),
        RuntimeMethod::PanelMove => request.parse::<PanelMoveParams>().is_some_and(|params| {
            params.has_valid_envelope()
                && present(&params.request.view)
                && present(&params.request.target_group)
        }),
        RuntimeMethod::PanelOpenSettings => request
            .parse::<PanelOpenSettingsParams>()
            .is_some_and(|params| params.has_valid_envelope()),
        RuntimeMethod::TerminalCreate => {
            request
                .parse::<TerminalCreateParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && request
                            .project_root
                            .as_deref()
                            .is_none_or(|path| !path.trim().is_empty())
                        && request
                            .cwd
                            .as_deref()
                            .is_none_or(|path| !path.trim().is_empty())
                })
        }
    }
}

fn valid_ratio(ratio: f32) -> bool {
    ratio.is_finite() && (0.0..=1.0).contains(&ratio)
}

fn valid_viewport(viewport: ViewportSize) -> bool {
    viewport.width.is_finite()
        && viewport.height.is_finite()
        && viewport.width > 0.0
        && viewport.height > 0.0
}

/// 当前已实现的客户端请求；新增方法必须显式扩展封闭集合。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ClientRequest {
    Hello(HelloRequest),
    Handover(HandoverRequest),
    Runtime(RuntimeRequest),
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
            Self::Runtime(request) => {
                request.has_valid_envelope() && runtime_request_is_valid(request)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::InvalidRequest)
        }
    }
}

/// 当前方法注册表同时用于 hello 能力摘要、describe 与 schema 覆盖检查。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlMethod {
    RuntimeHello,
    RuntimeDescribe,
    RuntimeSnapshot,
    RuntimeOperationGet,
    RuntimeOperationWait,
    RuntimeEventsSubscribe,
    RuntimeEventsUnsubscribe,
    RuntimeProjectList,
    RuntimeProjectActivate,
    RuntimeProjectRename,
    RuntimeProjectReorder,
    RuntimeProjectRemove,
    RuntimePanelGroupSplit,
    RuntimePanelGroupMerge,
    RuntimePanelGroupResize,
    RuntimePanelActivate,
    RuntimePanelMove,
    RuntimePanelOpenSettings,
    RuntimeTerminalCreate,
    InstanceHandover,
}

impl ControlMethod {
    pub const ALL: [Self; 20] = [
        Self::RuntimeHello,
        Self::RuntimeDescribe,
        Self::RuntimeSnapshot,
        Self::RuntimeOperationGet,
        Self::RuntimeOperationWait,
        Self::RuntimeEventsSubscribe,
        Self::RuntimeEventsUnsubscribe,
        Self::RuntimeProjectList,
        Self::RuntimeProjectActivate,
        Self::RuntimeProjectRename,
        Self::RuntimeProjectReorder,
        Self::RuntimeProjectRemove,
        Self::RuntimePanelGroupSplit,
        Self::RuntimePanelGroupMerge,
        Self::RuntimePanelGroupResize,
        Self::RuntimePanelActivate,
        Self::RuntimePanelMove,
        Self::RuntimePanelOpenSettings,
        Self::RuntimeTerminalCreate,
        Self::InstanceHandover,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeHello => "runtime.hello",
            Self::RuntimeDescribe => "runtime.describe",
            Self::RuntimeSnapshot => "runtime.snapshot",
            Self::RuntimeOperationGet => "runtime.operation_get",
            Self::RuntimeOperationWait => "runtime.operation_wait",
            Self::RuntimeEventsSubscribe => "events.subscribe",
            Self::RuntimeEventsUnsubscribe => "events.unsubscribe",
            Self::RuntimeProjectList => "project.list",
            Self::RuntimeProjectActivate => "project.activate",
            Self::RuntimeProjectRename => "project.rename",
            Self::RuntimeProjectReorder => "project.reorder",
            Self::RuntimeProjectRemove => "project.remove",
            Self::RuntimePanelGroupSplit => "panel_group.split",
            Self::RuntimePanelGroupMerge => "panel_group.merge",
            Self::RuntimePanelGroupResize => "panel_group.resize",
            Self::RuntimePanelActivate => "panel.activate",
            Self::RuntimePanelMove => "panel.move",
            Self::RuntimePanelOpenSettings => "panel.open_settings",
            Self::RuntimeTerminalCreate => "terminal.create",
            Self::InstanceHandover => "instance.handover",
        }
    }
}

/// `runtime.describe` 的单个方法合同：与注册表共用同一事实。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MethodDescriptor {
    pub method: String,
    pub scopes: Vec<ControlScope>,
    pub kind: crate::MethodKind,
    pub idempotent: bool,
}

/// `runtime.describe` 返回方法注册与当前合同散列。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DescribeResponse {
    pub protocol_sha256: String,
    pub schema_sha256: String,
    pub methods: Vec<MethodDescriptor>,
}

impl DescribeResponse {
    /// 只从方法注册表派生；散列由握手同一来源传入。
    pub fn current(protocol_sha256: impl Into<String>, schema_sha256: impl Into<String>) -> Self {
        let methods = crate::METHODS
            .iter()
            .map(|spec| MethodDescriptor {
                method: spec.method.as_str().to_owned(),
                scopes: spec.scopes.to_vec(),
                kind: spec.kind,
                idempotent: spec.idempotent,
            })
            .collect();
        Self {
            protocol_sha256: protocol_sha256.into(),
            schema_sha256: schema_sha256.into(),
            methods,
        }
    }
}

/// 快照中的项目资源身份与元数据。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectResource {
    pub project_id: String,
    pub display_name: String,
    pub root: String,
    pub active: bool,
}

/// 快照中的面板组；`views` 按持久顺序，`active_view` 为组内激活视图。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelGroupResource {
    pub project_id: String,
    pub group_id: String,
    pub views: Vec<ViewResource>,
    pub active_view: String,
}

/// 视图身份：封闭类型名与规范 ViewKey JSON 文本。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ViewResource {
    pub kind: String,
    pub key: String,
}

/// 快照中的逻辑终端；不含 PTY、输出或回滚正文。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalResource {
    pub terminal_id: String,
    pub project_id: String,
    pub number: u64,
    pub cwd: String,
    pub shell: String,
    pub title: Option<String>,
    /// 封闭运行意图：`launch` 或 `exited`。
    pub run_intent: String,
}

/// 快照中的 Document 位置元数据；不含正文。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DocumentResource {
    pub document_id: String,
    pub path: String,
}

/// 同一代次资源模型：项目、面板组、终端与 Document。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SnapshotResources {
    pub projects: Vec<ProjectResource>,
    pub groups: Vec<PanelGroupResource>,
    pub terminals: Vec<TerminalResource>,
    pub documents: Vec<DocumentResource>,
}

/// `runtime.snapshot` 的不可变资源快照与世代。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SnapshotResponse {
    pub store_epoch: String,
    pub stream_epoch: String,
    pub revision: u64,
    pub resources: SnapshotResources,
}

/// 持久 Operation 的查询视图；结果不明同样可查询且不自动重发。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OperationResource {
    pub operation_id: String,
    pub status: crate::OperationStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

/// Runtime 方法的封闭结果集合；服务端按方法构造。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum RuntimeResult {
    Describe(DescribeResponse),
    Snapshot(SnapshotResponse),
    Operation(OperationResource),
    Subscribed(EventsSubscribedResponse),
    Unsubscribed(EventsUnsubscribedResponse),
    Projects(ProjectListResponse),
}

/// 当前客户端可调用的方法集合。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub methods: BTreeSet<String>,
}

impl Capabilities {
    /// 能力摘要只从方法注册表派生，禁止第二份方法名单。
    pub fn current() -> Self {
        Self {
            methods: crate::method_names().map(str::to_owned).collect(),
        }
    }
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
            capabilities: Capabilities::current(),
        }
    }
}

/// 单实例移交已接受的确认；拒绝必须使用结构化错误而非 accepted=false。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HandoverResponse {
    #[cfg_attr(feature = "schema", schemars(extend("enum" = [true])))]
    accepted: bool,
}

impl HandoverResponse {
    pub const fn accepted() -> Self {
        Self { accepted: true }
    }

    pub const fn is_accepted(&self) -> bool {
        self.accepted
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
    /// 幂等键已绑定其他方法、参数版本或参数散列。
    MutationConflict,
    /// `expected_revision` 与执行前复核的资源代次不一致。
    RevisionConflict,
    /// 破坏操作缺少显式处置策略；调用方须使用新 mutation_id 重试。
    DecisionRequired,
    /// 副作用无法证明；Operation 为可查询终态，禁止自动重发。
    OutcomeUnknown,
    /// 查询的操作标识不存在。
    UnknownOperation,
    /// 事件续传三元组不匹配或历史缺口；客户端必须重新快照后订阅。
    ResyncRequired,
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

/// 当前请求、响应与发现记录的根集合，schema 与各领域 DTO 共用此边界。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ControlMessage {
    Hello(HelloRequest),
    Handover(HandoverRequest),
    Runtime(RuntimeRequest),
    HelloResponse(JsonRpcResult<HelloResponse>),
    HandoverResponse(JsonRpcResult<HandoverResponse>),
    RuntimeResponse(JsonRpcResult<RuntimeResult>),
    EventNotification(EventNotification),
    Error(JsonRpcErrorResponse),
    Discovery(DiscoveryRecord),
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

/// 写请求参数的规范散列：客户端与服务端必须对同一 JSON 得到同一值。
///
/// 对象键序由 `serde_json` 的映射实现决定；本 crate 不启用 `preserve_order`，
/// 因此同一逻辑参数的不同键序得到相同散列。
pub fn params_sha256(params: &serde_json::Value) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let encoded = serde_json::to_vec(params).expect("JSON Value 序列化不会失败");
    Sha256::digest(encoded).into()
}

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
