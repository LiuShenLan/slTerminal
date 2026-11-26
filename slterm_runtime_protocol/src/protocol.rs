//! Runtime v2 请求、响应与 JSON Lines 校验。

use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::handshake::{DescribeResponse, HelloResponse};
use crate::snapshot::{ProjectResource, SnapshotResponse};

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

/// `runtime discover --json` 的唯一输出合同：端点记录加上 Runtime 凭据。
///
/// 只用于 CLI 输出，不进入端点文件；Debug 永远脱敏，JSON 是唯一泄露 token 的通道。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscoverOutput {
    pub protocol_version: u32,
    pub endpoint: SocketAddr,
    pub run_id: String,
    pub process_id: u32,
    pub process_created_at_ms: u64,
    pub runtime_token: Credential,
}

impl DiscoverOutput {
    pub fn new(record: DiscoveryRecord, runtime_token: Credential) -> Self {
        Self {
            protocol_version: record.protocol_version,
            endpoint: record.endpoint,
            run_id: record.run_id,
            process_id: record.process_id,
            process_created_at_ms: record.process_created_at_ms,
            runtime_token,
        }
    }
}

impl fmt::Debug for DiscoverOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscoverOutput")
            .field("protocol_version", &self.protocol_version)
            .field("endpoint", &self.endpoint)
            .field("run_id", &self.run_id)
            .field("process_id", &self.process_id)
            .field("process_created_at_ms", &self.process_created_at_ms)
            .field("runtime_token", &"[redacted]")
            .finish()
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
    #[serde(rename = "panel.open_file")]
    PanelOpenFile,
    #[serde(rename = "panel.save")]
    PanelSave,
    #[serde(rename = "panel.close")]
    PanelClose,
    #[serde(rename = "terminal.create")]
    TerminalCreate,
    #[serde(rename = "terminal.input")]
    TerminalInput,
    #[serde(rename = "terminal.execute")]
    TerminalExecute,
    #[serde(rename = "terminal.read")]
    TerminalRead,
    #[serde(rename = "terminal.wait")]
    TerminalWait,
    #[serde(rename = "terminal.close")]
    TerminalClose,
    #[serde(rename = "terminal.reconnect")]
    TerminalReconnect,
    #[serde(rename = "terminal.rename")]
    TerminalRename,
    #[serde(rename = "agent.list")]
    AgentList,
    #[serde(rename = "agent.get")]
    AgentGet,
    #[serde(rename = "agent.input")]
    AgentInput,
    #[serde(rename = "agent.close")]
    AgentClose,
    #[serde(rename = "agent.read")]
    AgentRead,
    #[serde(rename = "agent.wait")]
    AgentWait,
    #[serde(rename = "agent.start")]
    AgentStart,
    #[serde(rename = "agent.resume")]
    AgentResume,
    #[serde(rename = "agent.fork")]
    AgentFork,
    #[serde(rename = "git.status")]
    GitStatus,
    #[serde(rename = "git.diff")]
    GitDiff,
    #[serde(rename = "git.rollback")]
    GitRollback,
    #[serde(rename = "git.delete")]
    GitDelete,
    #[serde(rename = "git.open_diff")]
    GitOpenDiff,
    #[serde(rename = "git.open_deleted")]
    GitOpenDeleted,
    #[serde(rename = "app.focus")]
    AppFocus,
    #[serde(rename = "app.close")]
    AppClose,
}

/// 原始输入：显式写入 PTY，不追加回车，也不模拟键盘事件。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalInputRequest {
    pub terminal_id: String,
    pub generation: u64,
    pub data: String,
}

/// 可信 Shell 行执行：只接受单行命令，服务端追加一次回车。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalExecuteRequest {
    pub terminal_id: String,
    pub generation: u64,
    pub command: String,
}

pub type TerminalInputParams = MutationParams<TerminalInputRequest>;
pub type TerminalExecuteParams = MutationParams<TerminalExecuteRequest>;

/// 终端输出分页读取；游标只由服务端生成，客户端不得构造。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalReadRequest {
    pub terminal_id: String,
    pub generation: u64,
    pub cursor: Option<String>,
}

/// 终端输出页面；`next_cursor` 存在即表示尾部仍有内容。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalReadResponse {
    pub terminal_id: String,
    pub generation: u64,
    pub revision: u64,
    pub data: String,
    pub next_cursor: Option<String>,
}

/// 等待终端输出推进到 `since_revision` 之后；超时映射 NotReady。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalWaitRequest {
    pub terminal_id: String,
    pub generation: u64,
    pub since_revision: u64,
    pub timeout_ms: Option<u64>,
}

/// 等待结果：`revision` 一定大于请求的 `since_revision`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalWaitResponse {
    pub terminal_id: String,
    pub generation: u64,
    pub revision: u64,
}

/// 关闭终端：代次必须与快照中的当前 PTY 代次一致（无 worker 时为零）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalCloseRequest {
    pub terminal_id: String,
    pub generation: u64,
}

pub type TerminalCloseParams = MutationParams<TerminalCloseRequest>;

/// 重连已退出终端：代次必须与快照中的当前 PTY 代次一致（无 worker 时为零）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalReconnectRequest {
    pub terminal_id: String,
    pub generation: u64,
}

pub type TerminalReconnectParams = MutationParams<TerminalReconnectRequest>;

/// 更新终端自定义标题；`None` 表示清除，空白与超过 128 字符拒绝。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TerminalRenameRequest {
    pub terminal_id: String,
    pub title: Option<String>,
}

pub type TerminalRenameParams = MutationParams<TerminalRenameRequest>;

/// Agent 会话状态的封闭线缆枚举。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    #[default]
    Unknown,
}

/// 活跃 Agent 关联：一个对外身份可关联多个终端。
///
/// 有原生会话 ID 时 `agent_id = kind:native_session_id`，否则按
/// `kind@terminal_id`；写方法必须再提供 TerminalId 与 PTY 代次唯一定位。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentResource {
    pub agent_id: String,
    pub terminal_id: String,
    pub project_id: String,
    /// AI CLI slug；持久来源可能先于目录登记，保持开放文本。
    pub kind: String,
    /// 封闭运行状态；新增值必须先登记协议。
    pub status: AgentState,
    pub native_session_id: Option<String>,
}

/// agent.list 与 agent.get 共用的关联集合；空集合表示没有匹配。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentListResponse {
    pub agents: Vec<AgentResource>,
}

/// 按对外 Agent 身份查询关联。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentGetRequest {
    pub agent_id: String,
}

/// Agent 写方法的目标：对外身份 + TerminalId + PTY 代次唯一定位。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentInputRequest {
    pub agent_id: String,
    pub terminal_id: String,
    pub generation: u64,
    pub data: String,
}

pub type AgentInputParams = MutationParams<AgentInputRequest>;

/// 关闭 Agent 关联的终端；身份与代次必须同时匹配。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentCloseRequest {
    pub agent_id: String,
    pub terminal_id: String,
    pub generation: u64,
}

pub type AgentCloseParams = MutationParams<AgentCloseRequest>;

/// Agent 关联终端的分页读取；目标身份与代次必须同时匹配。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentReadRequest {
    pub agent_id: String,
    pub terminal_id: String,
    pub generation: u64,
    pub cursor: Option<String>,
}

/// 等待 Agent 关联终端的输出 revision 超过 `since_revision`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentWaitRequest {
    pub agent_id: String,
    pub terminal_id: String,
    pub generation: u64,
    pub since_revision: u64,
    pub timeout_ms: Option<u64>,
}

/// 在目标项目新建终端并启动指定 CLI；不隐式创建项目。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentStartRequest {
    pub kind: String,
    pub project_root: Option<String>,
    pub cwd: Option<String>,
}

/// 恢复已有原生会话：会话身份必须由 AgentCatalog 校验后构造命令。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentResumeRequest {
    pub source: String,
    pub session_id: Option<String>,
    pub session_file: Option<String>,
    pub project_root: Option<String>,
    pub cwd: Option<String>,
}

/// 从已有原生会话创建独立分支，原会话不变。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AgentForkRequest {
    pub source: String,
    pub session_id: String,
    pub project_root: Option<String>,
    pub cwd: Option<String>,
}

pub type AgentStartParams = MutationParams<AgentStartRequest>;
pub type AgentResumeParams = MutationParams<AgentResumeRequest>;
pub type AgentForkParams = MutationParams<AgentForkRequest>;

/// 项目仓库状态查询；`project_id` 缺省时使用活跃项目。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitStatusRequest {
    pub project_id: Option<String>,
}

/// 单个工作区条目的 porcelain v2 状态。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitStatusEntry {
    /// 仓库相对路径（`/` 分隔）。
    pub path: String,
    pub original: Option<String>,
    pub index: GitStatusCode,
    pub worktree: GitStatusCode,
    pub conflict: bool,
    pub ignored: bool,
    pub staged: bool,
    pub unstaged: bool,
}

/// Git porcelain v2 的 XY 状态字符封闭域。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GitStatusCode {
    #[serde(rename = ".")]
    Unmodified,
    #[serde(rename = "M")]
    Modified,
    #[serde(rename = "T")]
    TypeChanged,
    #[serde(rename = "A")]
    Added,
    #[serde(rename = "D")]
    Deleted,
    #[serde(rename = "R")]
    Renamed,
    #[serde(rename = "C")]
    Copied,
    #[serde(rename = "U")]
    Unmerged,
    #[serde(rename = "?")]
    Untracked,
    #[serde(rename = "!")]
    Ignored,
}

impl GitStatusCode {
    pub const fn from_char(value: char) -> Option<Self> {
        match value {
            '.' => Some(Self::Unmodified),
            'M' => Some(Self::Modified),
            'T' => Some(Self::TypeChanged),
            'A' => Some(Self::Added),
            'D' => Some(Self::Deleted),
            'R' => Some(Self::Renamed),
            'C' => Some(Self::Copied),
            'U' => Some(Self::Unmerged),
            '?' => Some(Self::Untracked),
            '!' => Some(Self::Ignored),
            _ => None,
        }
    }

    pub const fn as_char(self) -> char {
        match self {
            Self::Unmodified => '.',
            Self::Modified => 'M',
            Self::TypeChanged => 'T',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::Unmerged => 'U',
            Self::Untracked => '?',
            Self::Ignored => '!',
        }
    }
}

/// 项目仓库状态：分支、上游与完整条目集，不含正文。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitStatusResponse {
    pub project_id: String,
    pub root: String,
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<GitStatusEntry>,
}

/// 读取单个仓库相对路径的 HEAD 与工作区差异；游标只由服务端生成。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitDiffRequest {
    pub project_id: Option<String>,
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
    pub cursor: Option<String>,
}

/// Git diff 页面；`next_cursor` 存在即表示尾部仍有内容。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitDiffResponse {
    pub project_id: String,
    pub path: String,
    pub revision: u64,
    pub data: String,
    pub next_cursor: Option<String>,
}

/// 关闭集合中 Document 的处置策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CloseDocumentsPolicy {
    Save,
    Discard,
}

/// 非交互关闭可用的 Hooks 草稿处置；保存需要交互确认，不在线缆内。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CloseHooksPolicy {
    Discard,
}

/// 应用关闭：缺省策略不写入任何决定，必要时返回 `decision_required`。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppCloseRequest {
    pub documents: Option<CloseDocumentsPolicy>,
    pub hooks: Option<CloseHooksPolicy>,
}

/// 应用聚焦：把主窗口带到前台，不改工作区状态。
pub type AppFocusParams = MutationParams<EmptyParams>;
pub type AppCloseParams = MutationParams<AppCloseRequest>;

/// 回滚前置形态：只允许放弃精确的未暂存修改或删除。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GitRollbackExpected {
    Modified,
    Deleted,
}

/// 放弃未暂存更改并恢复索引版本；`expected` 是专用前置条件。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitRollbackRequest {
    pub project_id: Option<String>,
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
    /// 调用方观察到的未暂存形态；执行前必须复核一致。
    pub expected: GitRollbackExpected,
}

pub type GitRollbackParams = MutationParams<GitRollbackRequest>;

/// 删除前置形态：未跟踪文件或已暂存新增。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GitDeleteExpected {
    Untracked,
    StagedAdded,
}

/// 受限删除：`expected` 是专用前置条件，`permanent` 是显式处置策略。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitDeleteRequest {
    pub project_id: Option<String>,
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
    /// 调用方观察到的条目形态；执行前必须复核一致。
    pub expected: GitDeleteExpected,
    /// 跳过回收站直接永久删除；缺省走回收站。
    #[serde(default)]
    pub permanent: bool,
}

pub type GitDeleteParams = MutationParams<GitDeleteRequest>;

/// 打开共享 Diff：目标必须是当前映射为差异的条目（modified／renamed／conflict）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitOpenDiffRequest {
    pub project_id: Option<String>,
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
}

pub type GitOpenDiffParams = MutationParams<GitOpenDiffRequest>;

/// 打开固定版本：目标必须是当前映射为删除的条目。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GitOpenDeletedRequest {
    pub project_id: Option<String>,
    /// 仓库相对路径，使用 `/` 分隔。
    pub path: String,
}

pub type GitOpenDeletedParams = MutationParams<GitOpenDeletedRequest>;

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

/// 打开文件到文本面板；`path` 是绝对 OS 路径，类型与授权由 application 裁定。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelOpenFileRequest {
    pub path: String,
}

pub type PanelOpenFileParams = MutationParams<PanelOpenFileRequest>;

/// 保存视图绑定的 Document；`view` 是规范 ViewKey JSON 文本。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelSaveRequest {
    pub view: String,
}

pub type PanelSaveParams = MutationParams<PanelSaveRequest>;

/// 关闭单个面板；`documents` 缺省时脏 Document 会返回 `decision_required`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelCloseRequest {
    /// 规范 ViewKey JSON 文本。
    pub view: String,
    pub documents: Option<CloseDocumentsPolicy>,
}

pub type PanelCloseParams = MutationParams<PanelCloseRequest>;

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
    /// 项目范围内 Document 的处置策略；缺省时脏 Document 触发 `decision_required`。
    #[serde(default)]
    pub documents: Option<CloseDocumentsPolicy>,
    /// 项目 Hooks 草稿的处置策略；缺省时草稿触发 `decision_required`。
    #[serde(default)]
    pub hooks: Option<CloseHooksPolicy>,
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
    /// 封闭事件类型；新增值必须先登记协议与发布点。
    pub kind: RuntimeEventKind,
    /// 资源身份文本（稳定 ID 或规范 ViewKey JSON）。
    pub resource: String,
    /// 资源代次；旧代次事件不覆盖新代次状态。
    pub generation: u64,
}

/// Runtime 事件类型的封闭线缆枚举。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum RuntimeEventKind {
    /// 项目激活 [resource=project id]。
    #[serde(rename = "project.activated")]
    ProjectActivated,
    /// 面板激活 [resource=规范 ViewKey JSON]。
    #[serde(rename = "panel.activated")]
    PanelActivated,
    /// 面板关闭 [resource=规范 ViewKey JSON]。
    #[serde(rename = "panel.closed")]
    PanelClosed,
    /// 终端输出代次推进 [resource=terminal id]。
    #[serde(rename = "terminal.output")]
    TerminalOutput,
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
    TerminalInput(TerminalInputParams),
    TerminalExecute(TerminalExecuteParams),
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
        RuntimeMethod::PanelOpenFile => {
            request
                .parse::<PanelOpenFileParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && !request.path.trim().is_empty()
                        && !request.path.contains('\0')
                })
        }
        RuntimeMethod::PanelSave => request.parse::<PanelSaveParams>().is_some_and(|params| {
            params.has_valid_envelope() && !params.request.view.trim().is_empty()
        }),
        RuntimeMethod::PanelClose => request.parse::<PanelCloseParams>().is_some_and(|params| {
            params.has_valid_envelope() && !params.request.view.trim().is_empty()
        }),
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
        RuntimeMethod::TerminalInput => {
            request
                .parse::<TerminalInputParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && present(&request.terminal_id)
                        && request.generation > 0
                        && !request.data.is_empty()
                })
        }
        RuntimeMethod::TerminalExecute => {
            request
                .parse::<TerminalExecuteParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && present(&request.terminal_id)
                        && request.generation > 0
                        && present(&request.command)
                        && !request.command.contains(['\0', '\r', '\n'])
                })
        }
        RuntimeMethod::TerminalRead => {
            request
                .parse::<TerminalReadRequest>()
                .is_some_and(|params| {
                    present(&params.terminal_id)
                        && params.generation > 0
                        && params
                            .cursor
                            .as_deref()
                            .is_none_or(|cursor| !cursor.is_empty())
                })
        }
        RuntimeMethod::TerminalWait => {
            request
                .parse::<TerminalWaitRequest>()
                .is_some_and(|params| {
                    present(&params.terminal_id)
                        && params.generation > 0
                        && params
                            .timeout_ms
                            .is_none_or(|timeout| (1..=OPERATION_WAIT_LIMIT_MS).contains(&timeout))
                })
        }
        RuntimeMethod::TerminalClose => {
            request
                .parse::<TerminalCloseParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope() && present(&params.request.terminal_id)
                })
        }
        RuntimeMethod::TerminalReconnect => {
            request
                .parse::<TerminalReconnectParams>()
                .is_some_and(|params| {
                    params.has_valid_envelope() && present(&params.request.terminal_id)
                })
        }
        RuntimeMethod::TerminalRename => {
            request
                .parse::<TerminalRenameParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && present(&request.terminal_id)
                        && request.title.as_deref().is_none_or(|title| {
                            !title.trim().is_empty() && title.chars().count() <= 128
                        })
                })
        }
        RuntimeMethod::AgentList => request.parse::<EmptyParams>().is_some(),
        RuntimeMethod::AgentGet => request
            .parse::<AgentGetRequest>()
            .is_some_and(|params| present(&params.agent_id)),
        RuntimeMethod::AgentInput => request.parse::<AgentInputParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && present(&request.agent_id)
                && present(&request.terminal_id)
                && request.generation > 0
                && !request.data.is_empty()
        }),
        RuntimeMethod::AgentClose => request.parse::<AgentCloseParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && present(&request.agent_id)
                && present(&request.terminal_id)
        }),
        RuntimeMethod::AgentRead => request.parse::<AgentReadRequest>().is_some_and(|params| {
            present(&params.agent_id)
                && present(&params.terminal_id)
                && params.generation > 0
                && params
                    .cursor
                    .as_deref()
                    .is_none_or(|cursor| !cursor.is_empty())
        }),
        RuntimeMethod::AgentWait => request.parse::<AgentWaitRequest>().is_some_and(|params| {
            present(&params.agent_id)
                && present(&params.terminal_id)
                && params.generation > 0
                && params
                    .timeout_ms
                    .is_none_or(|timeout| (1..=OPERATION_WAIT_LIMIT_MS).contains(&timeout))
        }),
        RuntimeMethod::AgentStart => request.parse::<AgentStartParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && present(&request.kind)
                && valid_optional_path(request.project_root.as_deref())
                && valid_optional_path(request.cwd.as_deref())
        }),
        RuntimeMethod::AgentResume => request.parse::<AgentResumeParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && present(&request.source)
                && valid_optional_path(request.project_root.as_deref())
                && valid_optional_path(request.cwd.as_deref())
                && request
                    .session_id
                    .as_deref()
                    .is_none_or(|id| !id.trim().is_empty())
                && request
                    .session_file
                    .as_deref()
                    .is_none_or(|path| !path.trim().is_empty())
        }),
        RuntimeMethod::AgentFork => request.parse::<AgentForkParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && present(&request.source)
                && present(&request.session_id)
                && valid_optional_path(request.project_root.as_deref())
                && valid_optional_path(request.cwd.as_deref())
        }),
        RuntimeMethod::GitStatus => request.parse::<GitStatusRequest>().is_some_and(|params| {
            params
                .project_id
                .as_deref()
                .is_none_or(|id| !id.trim().is_empty())
        }),
        RuntimeMethod::GitDiff => request.parse::<GitDiffRequest>().is_some_and(|params| {
            params
                .project_id
                .as_deref()
                .is_none_or(|id| !id.trim().is_empty())
                && !params.path.trim().is_empty()
                && valid_repo_relative_path(&params.path)
                && params
                    .cursor
                    .as_deref()
                    .is_none_or(|cursor| !cursor.is_empty())
        }),
        RuntimeMethod::GitRollback => request.parse::<GitRollbackParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && request
                    .project_id
                    .as_deref()
                    .is_none_or(|id| !id.trim().is_empty())
                && !request.path.trim().is_empty()
                && valid_repo_relative_path(&request.path)
        }),
        RuntimeMethod::GitDelete => request.parse::<GitDeleteParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && request
                    .project_id
                    .as_deref()
                    .is_none_or(|id| !id.trim().is_empty())
                && !request.path.trim().is_empty()
                && valid_repo_relative_path(&request.path)
        }),
        RuntimeMethod::GitOpenDiff => request.parse::<GitOpenDiffParams>().is_some_and(|params| {
            let request = &params.request;
            params.has_valid_envelope()
                && request
                    .project_id
                    .as_deref()
                    .is_none_or(|id| !id.trim().is_empty())
                && !request.path.trim().is_empty()
                && valid_repo_relative_path(&request.path)
        }),
        RuntimeMethod::GitOpenDeleted => {
            request
                .parse::<GitOpenDeletedParams>()
                .is_some_and(|params| {
                    let request = &params.request;
                    params.has_valid_envelope()
                        && request
                            .project_id
                            .as_deref()
                            .is_none_or(|id| !id.trim().is_empty())
                        && !request.path.trim().is_empty()
                        && valid_repo_relative_path(&request.path)
                })
        }
        RuntimeMethod::AppFocus => request
            .parse::<AppFocusParams>()
            .is_some_and(|params| params.has_valid_envelope()),
        RuntimeMethod::AppClose => request
            .parse::<AppCloseParams>()
            .is_some_and(|params| params.has_valid_envelope()),
    }
}

/// 仓库相对路径：非空、无 NUL／控制字符／冒号、不以分隔符开头，且每段都是
/// 普通组件（非空、非 `.`／`..`）。
///
/// 拒绝冒号同时排除盘符绝对路径（`C:/...`，`Path::join` 在 Windows 上会整体
/// 替换仓库根）、盘符相对路径（`C:file`）与备用数据流（`file:stream`）。
fn valid_repo_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\0', '\\', ':'])
        || path.chars().any(char::is_control)
    {
        return false;
    }
    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

/// 可选路径要么缺省，要么非空白。
fn valid_optional_path(path: Option<&str>) -> bool {
    path.is_none_or(|path| !path.trim().is_empty())
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
    RuntimePanelOpenFile,
    RuntimePanelSave,
    RuntimePanelClose,
    RuntimeTerminalCreate,
    RuntimeTerminalInput,
    RuntimeTerminalExecute,
    RuntimeTerminalRead,
    RuntimeTerminalWait,
    RuntimeTerminalClose,
    RuntimeTerminalReconnect,
    RuntimeTerminalRename,
    RuntimeAgentList,
    RuntimeAgentGet,
    RuntimeAgentInput,
    RuntimeAgentClose,
    RuntimeAgentRead,
    RuntimeAgentWait,
    RuntimeAgentStart,
    RuntimeAgentResume,
    RuntimeAgentFork,
    RuntimeGitStatus,
    RuntimeGitDiff,
    RuntimeGitRollback,
    RuntimeGitDelete,
    RuntimeGitOpenDiff,
    RuntimeGitOpenDeleted,
    RuntimeAppFocus,
    RuntimeAppClose,
    InstanceHandover,
}

impl ControlMethod {
    pub const ALL: [Self; 47] = [
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
        Self::RuntimePanelOpenFile,
        Self::RuntimePanelSave,
        Self::RuntimePanelClose,
        Self::RuntimeTerminalCreate,
        Self::RuntimeTerminalInput,
        Self::RuntimeTerminalExecute,
        Self::RuntimeTerminalRead,
        Self::RuntimeTerminalWait,
        Self::RuntimeTerminalClose,
        Self::RuntimeTerminalReconnect,
        Self::RuntimeTerminalRename,
        Self::RuntimeAgentList,
        Self::RuntimeAgentGet,
        Self::RuntimeAgentInput,
        Self::RuntimeAgentClose,
        Self::RuntimeAgentRead,
        Self::RuntimeAgentWait,
        Self::RuntimeAgentStart,
        Self::RuntimeAgentResume,
        Self::RuntimeAgentFork,
        Self::RuntimeGitStatus,
        Self::RuntimeGitDiff,
        Self::RuntimeGitRollback,
        Self::RuntimeGitDelete,
        Self::RuntimeGitOpenDiff,
        Self::RuntimeGitOpenDeleted,
        Self::RuntimeAppFocus,
        Self::RuntimeAppClose,
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
            Self::RuntimePanelOpenFile => "panel.open_file",
            Self::RuntimePanelSave => "panel.save",
            Self::RuntimePanelClose => "panel.close",
            Self::RuntimeTerminalCreate => "terminal.create",
            Self::RuntimeTerminalInput => "terminal.input",
            Self::RuntimeTerminalExecute => "terminal.execute",
            Self::RuntimeTerminalRead => "terminal.read",
            Self::RuntimeTerminalWait => "terminal.wait",
            Self::RuntimeTerminalClose => "terminal.close",
            Self::RuntimeTerminalReconnect => "terminal.reconnect",
            Self::RuntimeTerminalRename => "terminal.rename",
            Self::RuntimeAgentList => "agent.list",
            Self::RuntimeAgentGet => "agent.get",
            Self::RuntimeAgentInput => "agent.input",
            Self::RuntimeAgentClose => "agent.close",
            Self::RuntimeAgentRead => "agent.read",
            Self::RuntimeAgentWait => "agent.wait",
            Self::RuntimeAgentStart => "agent.start",
            Self::RuntimeAgentResume => "agent.resume",
            Self::RuntimeAgentFork => "agent.fork",
            Self::RuntimeGitStatus => "git.status",
            Self::RuntimeGitDiff => "git.diff",
            Self::RuntimeGitRollback => "git.rollback",
            Self::RuntimeGitDelete => "git.delete",
            Self::RuntimeGitOpenDiff => "git.open_diff",
            Self::RuntimeGitOpenDeleted => "git.open_deleted",
            Self::RuntimeAppFocus => "app.focus",
            Self::RuntimeAppClose => "app.close",
            Self::InstanceHandover => "instance.handover",
        }
    }
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
    TerminalRead(TerminalReadResponse),
    TerminalWait(TerminalWaitResponse),
    Agents(AgentListResponse),
    GitStatus(GitStatusResponse),
    GitDiff(GitDiffResponse),
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
    /// 分页游标非法、越界或资源代次／revision 已变化；客户端必须重新读取。
    StaleCursor,
    /// 对外 Agent 身份没有匹配关联。
    UnknownAgent,
    /// 编码后的响应超过单帧预算；请求必须缩小范围或改用分页读取。
    ResponseTooLarge,
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
    DiscoverOutput(DiscoverOutput),
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
/// 对象键按字典序递归规范化后再散列，不依赖 `serde_json` 的映射实现；
/// 工作区特性统一可能启用 `preserve_order`，键序不得因此改变幂等身份。
pub fn params_sha256(params: &serde_json::Value) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let canonical = canonical_json(params);
    let encoded = serde_json::to_vec(&canonical).expect("JSON Value 序列化不会失败");
    Sha256::digest(encoded).into()
}

/// 递归规范化 JSON：对象键排序，数组保持元素顺序。
fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key.clone(), canonical_json(value));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonical_json).collect())
        }
        other => other.clone(),
    }
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
