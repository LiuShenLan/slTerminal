# 04 Runtime 控制 API + 单实例移交详细设计

> pebrel-design 分片 04/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/04-runtime-api.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **Runtime API 信封 / 方法类型** 的锚点归属篇(01 篇类型锚点表已登记):版本化信封(`ApiRequest`/`ApiResponse`/`ApiEvent`/`ApiError`)、协议常量、snapshot/pane/agent/run/orchestrate 全族协议类型、`RuntimeCommand`/`RuntimeDispatch`/`RuntimeCallback`/`RuntimeHub`、移交与端点发现类型在此签名级定义,他篇(03/05/06/09/11 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

Runtime 控制面在单进程 GPUI 世界重建为「**一个状态权威 + 两类消费端**」:

- **状态权威 = `slterm_app::runtime_api::RuntimeHub`**(照抄 pebrel `nebula_app/src/runtime_api.rs` 的 `RuntimeHub`):持有唯一 `RuntimeSnapshot` + 单调 `revision` + 每 pane `state_change_seq` + managed agent 注册表(generation 绑定)+ run 等待表 + 委派在途表。壳与它消费**同一份**投影:GPUI 壳把 `Workspace`(05 篇)投影成 snapshot 经 `RuntimeHub::publish` 入账——壳内侧栏/托盘/等待语义与外部客户端读的是同一Reducer 产物,不存在第二份私有状态拷贝。
- **消费端一 = GPUI 壳(UI owner 线程)**:transport 线程不做写操作;凡窗口/Tab/PTY 写一律打包成 `RuntimeCommand` 经 `RuntimeDispatch` 派发到壳事件队列,在 GPUI foreground executor 上由壳执行并 `respond`。事件循环不可用返回 `runtime_unavailable`,不伪造成功。
- **消费端二 = loopback 控制面**:外部客户端(AI CLI、脚本、未来插件)经 `127.0.0.1` JSON Lines + `runtime.port`(port + 128bit token)发现与调用;读方法(wait/subscribe)在 transport 线程直接对 hub 等待,写方法派发到 UI owner。
- **单实例 = 移交不驻留**:关窗即退出(00-roadmap 不变量 5);二次启动只把「还原/聚焦窗口 + 新开 tab(可带 cwd/shell)」经同一 loopback 交给首实例,自身退出。移交与控制面共用端口与首行分流位,不维护第二套 socket。

「壳即前端」在本篇的具体含义:旧 Tauri 命令面里凡是「pane 里的 AI CLI 也需要」的能力(pty 写族、git 读族),一律以本篇控制面方法形态重暴露;前端专属能力随 Tauri 消亡,不留存。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_app/src/runtime_api.rs` | pebrel 照抄裁剪改名 | 模块面 + 协议常量 + 信封四件 + snapshot/pane/agent/run 类型族(本篇锚点)+ `RuntimeHub` |
| `slterm_app/src/runtime_api/server.rs` | pebrel 照抄裁剪 | `RuntimeServer`/`spawn_callback`/`spawn_at`/`Drop`、`Endpoint`/`parse_endpoint`、`fresh_token`、`read_endpoint` 三级发现、`try_open_default_tab_existing`/`try_open_directory_existing`/`legacy_request_to`、`ENDPOINT_ENV`/`CHILD_ENDPOINT`/`apply_child_endpoint` |
| `slterm_app/src/runtime_api/transport.rs` | pebrel 照抄裁剪 | `serve`/`handle_connection`/`handle_legacy`、`runtime_description`、`subscribe_connection`、`wait_connection`、`dispatch_connection`、`dispatch_runtime_command`、`write_json_line` |
| `slterm_app/src/runtime_api/command.rs` | pebrel 照抄 | 参数结构族(`WaitParams`/`SubscribeParams`/`*Params` 全族)、`RuntimeCommand::from_request`、`validate_prompt`/`validate_paste_text`/`validate_command_line`/`validate_chat_message`、`wait_matches`/`wait_state_matches`、`capture_process_tree` |
| `slterm_app/src/runtime_api/agent_api.rs` | pebrel 照抄 | `agents_connection`/`agent_get_connection`/`agent_delegate_connection`/`agent_wait_connection`、`prepare_dispatch_command`(worktree 事务)、`rollback_prepared_worktree`/`attach_worktree_result`、`verified_agent_launch` |
| `slterm_app/src/runtime_api/orchestrate.rs` | pebrel 照抄 | `OrchestrateParams`/`OrchestrateStep`/`PaneTarget`/`StepReference`、`validate_params`、`orchestrate_connection`、`wait_agent_ready`、回执族(`WorkflowReceipt`/`StepReceipt`) |
| `slterm_app/src/runtime_api/terminal_read.rs` | pebrel 照抄裁剪 | `RuntimePaneRead`、`capture_terminal_tail`(screen v1 不迁) |
| `slterm_app/src/runtime_api/tabs.rs` | pebrel 照抄裁剪 | `TabId` 消费(锚点归 05 篇 `TabId(u64)`,本篇禁重定义)、tab 定位参数族 |
| `slterm_app/src/runtime_api/git.rs` | pebrel 参考收口 | `Request`(expected_cwd + revision 防陈旧)、status/diff/rollback/unstage 四方法(裁剪映射,见改造节) |
| `slterm_app/src/runtime_exec.rs` | pebrel 照抄裁剪 | `PaneExecContext`(仅 Host 形态)、`spawn` 非 TTY 子进程 + 并行排水 + 超时整树回收 |
| `slterm_app/src/runtime_api/cli.rs` | pebrel 照抄 | `request_once`/`request_once_bounded`/`client_stream`/`decode_plugin_response`/`bounded_exchange`/`require_submission_baseline`、`run_cli` |
| `slterm_app/src/runtime_api/shortcuts.rs` | pebrel 照抄改名 | 资源+动词薄别名(`env`/`window`/`tab`/`pane`/`agent`)、`command_catalog`、`probe_runtime` |
| `slterm_app/src/cli.rs`(runtime 子集) | pebrel 照抄裁剪 | clap 子命令面(`Subcommands` 的 `Ctl`/`Env`/`Window`/`Tab`/`Pane`/`Agent`;其余子命令归 03/06/07/09 篇)+ `ControlCommand` 全 verb 面 |
| `slterm_app/src/agent_env.rs` | 归 03 篇锚点 | 环境契约 `apply`/`executable`/`prepended_path`(本篇消费 + ENDPOINT 注入缝合,见改造节) |
| `slterm_app/src/atomic_file.rs` | 归 05/06 篇共有件 | `write`/`try_lock`/`try_lifetime_lock`/`replace`(本篇消费 owner 锁与原子写) |
| `docs/skills/slterm-runtime/SKILL.md` + `agents/openai.yaml` | pebrel 照抄改名 | Skill 自发现蓝本(命名/内容改造归改造节) |
| `docs/runtime-api-v1.schema.json`(或协议单文件落位) | pebrel 照抄改名 | 协议 Schema 蓝本(protocol 常量改 `slterm.runtime`) |

裁剪不迁:`runtime_api/conversation.rs` 整组、`mobile_bridge.rs`/`mobile_screen.rs`/`terminal_screen_v1`、`EventSink::Winit` 与 `server::dispatch_prompt`(legacy-shell feature)、`RuntimeServer::spawn_with_sink` 的提权隔离分支、`try_open_window_existing`/`window.create` 链、`mux.rs` 模块本体(其 ATTACH/PING 分流形态并入 transport 分流位,常量与 `request_at` 语义并入 server 移交路径)、`agent_env.rs` 的 WSL 臂与双名别名层、`git.rs` 的 stage/commit/fetch/pull/push 五写方法(收口形态见改造节)、`tabs.rs` 的 `tab.open`/`tab.read`(文件 tab 通道,eg 07 篇如未来需要再登记)。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **spec 分片 04 的 12 条不采纳点逐一点名**:mux 驻留保活语义(关窗即退出,仅留移交机制与降级语义);SSH pane 全套(`ssh_not_ready` 门禁、`PEBREL_PANE_REMOTE` 远端标记、remote_* 错误码、远端进程树);mobile_bridge 整支(含 bridge-policy 白名单作为独立机制);mobile screen v1 手机重排帧(`pane.read` 首发只抄文本尾部);`conversation.*` 整组;WSL 透传(`WSLENV` 路径翻译、`ExecLocation::Wsl`、`for_wsl_distribution`、git WSLENV 凭据转发——env 幂等判重与 PATH 前置仍归 03 篇照抄);多窗口方法面(`window.create`、`detached_windows` 计数、`session_exempt`、进程级窗口分发器);legacy-shell winit 双壳 `EventSink`;`NEBULA_*`/`PEBREL_*` 双名兼容层(runtime.port 与 mux.port 双发现文件、legacy 兼容回落位仅作扩展位保留、首发不启用);提权进程隔离发布逻辑(私有实例 env-only 发现形态仍抄);非终端 tab 类型枚举(snapshot `tab.kind` 按 05 篇 `WorkspaceTab` 注册表重定);超大方法面整族照搬(git 九方法、window.create 等按本篇收口裁剪)。
3. **协议边界硬约束**:信封与全部 params 结构 `deny_unknown_fields`;协议名 `slterm.runtime`、版本不匹配返回 `protocol_version_mismatch` 且 `details.supported_versions` 列出可用版本;token 鉴权先于详细参数校验(未认证本机进程不得把解析错误当协议预言机),token 不符静默丢弃连接;只监听 `127.0.0.1`,token 只挡其他本机用户,不承诺抵御同用户攻击者。
4. **等待语义为协议一等公民**:`state_change_seq`/`after_seq`/`--wait` 提交后基线、generation 绑定不做成客户端约定;`settled ≠ 成功`、超时只代表未确认结果(`submission_outcome_unknown`)的错误语义成文。
5. **单窗口收编**:snapshot 恒为单窗口(`windows` 一元素,`window_id = 1` 常量);`window.*` 只留 close(忙碌 pane 显式确认错误)+ focus 语义收编;`pane_target` 的 `ambiguous_target` 分支保留(协议形态不删,多窗口未来兼容位),但单窗口下不可达。
6. **类型锚点纪律**:本篇唯一定义信封/方法/snapshot/run/orchestrate/移交协议类型;`PaneId`/`TabId` 锚 05 篇、`AgentKind`/`AgentActivity` 锚 03 篇、`Grid`/`Term` 锚 02 篇——本篇只 `use` 或经 facade 传参序列化,禁重定义、禁别名漂移。
7. **pane id 语义承接 05 篇**:`PaneId` 全 workspace 唯一、终生不复用(05 篇单调分配器),protocol 序列化直接暴露 `PaneId.0`;pebrel 的「pane id 窗口内局部唯一」语义被 05 篇 newtype 取代,`state_change_seq` 记账键随之收窄为 pane id 单维(05 篇唯一性已兜住,无需 (window, pane) 复合键)。
8. **关窗即退出**:无驻留、无 detached tab、无隐藏宿主窗口;`RuntimeServer` 随壳生命周期持有,Drop 删端口文件(仅当文件仍等于自己的 endpoint)。
9. **环境变量单名 `SLTERM_*`**:`SLTERM_RUNTIME_ENDPOINT`(01 篇 C 节)、注入幂等判重、显式 env 端点非法时不得静默回落端口文件(语义照抄,防「选错实例」)。
10. **凭据纪律**:token 属能力边界而非凭据,但日志纪律照抄——transport 日志记 request_id/目标/字节数,不记 prompt/paste 正文;委派回传 `worker_output` 按不可信数据处理(消费归 03 篇,本篇只保证边界与上限)。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致,改名/裁剪点已标。契约数值(上限/超时/容量)首发取 pebrel 实测值照抄、标定机制归待沉淀 D04-2。pane id 形参处本篇以 05 篇 `PaneId`、tab 身份以 05 篇 `TabId` 承接(序列化形态见各条),与 pebrel 裸 `u64`/字符串 `TabId` 的偏差只是类型承载,不改语义。

### 协议常量与信封族(`slterm_app::runtime_api`,本篇锚点,照抄)

```rust
pub const PROTOCOL_NAME: &str = "slterm.runtime";      // 01 篇禁名门禁的「协议兼容名」语境
pub const PROTOCOL_VERSION: u16 = 1;
pub const SUPPORTED_VERSIONS: &[u16] = &[PROTOCOL_VERSION];

// 请求 {protocol, version, id, token, method, params}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApiRequest {
    pub protocol: String,
    pub version: u16,
    pub id: String,          // "<pid>-<全局单调序>"——响应身份校验的依据
    pub token: String,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: serde_json::Value,
}

// 响应 {protocol, version, id, ok, result|error};result 显式 null 是有效结果,
// 与「缺 result 字段」区分(present_json 反序列化器照抄)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiResponse {
    pub protocol: String,
    pub version: u16,
    pub id: String,
    pub ok: bool,
    #[serde(default, deserialize_with = "present_json", skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

// 事件 {protocol, version, event: "runtime.snapshot", revision, data}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiEvent {
    pub protocol: String,
    pub version: u16,
    pub event: String,
    pub revision: u64,
    pub data: RuntimeSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
// 构造辅件照抄:new(code, message)/details(v)/invalid_params(msg)
```

### 契约常量(首发值照抄 pebrel `runtime_api.rs` 常量块,重新标定归 D04-2)

```rust
// 传输与执行
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);   // 控制面客户端 connect
const IO_TIMEOUT: Duration = Duration::from_secs(5);            // 控制面客户端读写
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);      // UI 派发等待
const HANDOVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(400); // 移交链 connect(原 mux.rs CONNECT_TIMEOUT)
const HANDOVER_IO_TIMEOUT: Duration = Duration::from_millis(700);      // 移交链读写(原 mux.rs IO_TIMEOUT)
const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_CLIENTS: usize = 64;
// 输入校验
const MAX_PROMPT_BYTES: usize = 32 * 1024;      // prompt/paste/command/argv 共用上限
const MAX_TAB_NAME_BYTES: usize = 256;
pub(crate) const MAX_KEY_REPEAT: u16 = 64;
pub(crate) const MIN_PANE_RATIO: f32 = 0.05;
pub(crate) const MAX_PANE_RATIO: f32 = 0.95;
// 读取
pub(crate) const DEFAULT_READ_LINES: usize = 120;
pub(crate) const MAX_READ_LINES: usize = 2_000;
const MAX_READ_BYTES: usize = 1024 * 1024;
// 非 TTY 执行
pub(crate) const DEFAULT_EXEC_OUTPUT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_EXEC_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
// 等待
const MAX_WAIT: Duration = Duration::from_secs(24 * 60 * 60);
// run 分段限时(OSC 133 真实 exit code 的观察窗)
const RUN_START_GRACE: Duration = Duration::from_secs(3);
const RUN_PROGRESS_PROBE_LINES: usize = 8;
const RUN_PROGRESS_PROBE_INTERVAL: Duration = Duration::from_millis(500);
// 委派
const MAX_PENDING_DELEGATIONS: usize = 128;
const MAX_DELEGATION_RESULT_CHARS: usize = 4_000;
// 编排
const MAX_ORCHESTRATE_STEPS: usize = 32;
const MAX_ORCHESTRATE_BYTES: usize = 64 * 1024;
const DEFAULT_READY_TIMEOUT_MS: u64 = 10_000;
const EVIDENCE_TAIL_LINES: usize = 40;
const MAX_RECEIPT_TAIL_BYTES: usize = 4 * 1024;
// 终端尾部读取(与屏幕证据规则同一判据:底部不足一屏时多扫再回退)
const TAIL_SCAN_EXTRA_LINES: usize = 200;
```

### 任务状态投影族(本篇锚点,照抄)

```rust
/// 唯一任务状态投影:侧栏、托盘、snapshot、agents.list、pane.wait 共用,
/// 优先级 失败 > attention > 等待输入 > 运行 > 完成 > 空闲。
/// 外部客户端禁解析标题猜生命周期。投影来源 = 03 篇 AgentActivity 仲裁器(缝合见改造节)。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskState { Idle, Running, WaitingInput, Attention, Finished, Failed }

/// 等待请求的期望状态;Settled = 非 Running(≠ 成功,语义成文)
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RuntimeWaitState { Idle, Running, WaitingInput, Attention, Finished, Failed, Settled }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAgentStateSource { Hook, Screen, Process }   // 值集与 03 篇 AgentStatusSource 对齐

/// 挂在「被识别为 AI Agent 的 pane」上的身份与证据;生命周期状态仍在 task_state,
/// 托盘/侧栏/等待/外部客户端消费同一 reducer 输出。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAgent {
    #[serde(skip_serializing_if = "Option::is_none")] pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub worktree: Option<slterm_app::git_worktree::WorktreeProvenance>,
    pub kind: String,                 // = 03 篇 AgentKind::slug,身份事实单源
    pub display_name: String,
    pub session_id: Option<String>,
    pub state_source: RuntimeAgentStateSource,
    #[serde(skip_serializing_if = "Option::is_none")] pub state_rule: Option<String>, // TOML 规则 id
    pub hook_seen: bool,
}

/// managed agent 注册表项(hub 私有表投影出);generation 单调 +1,名冲突即 agent_name_conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeManagedAgent {
    pub agent_id: String,             // "agent-<pid>-<全局序>"
    pub generation: u64,
    pub name: String,
    pub kind: String,
    pub window_id: u64,
    pub pane_id: u64,                 // PaneId.0(05 篇)
    #[serde(skip_serializing_if = "Option::is_none")] pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub worktree: Option<slterm_app::git_worktree::WorktreeProvenance>,
    pub active: bool,
    pub observed: bool,
    #[serde(skip_serializing_if = "Option::is_none")] pub closed_reason: Option<String>,
    // closed_reason ∈ agent_exited | agent_replaced | pane_closed | pane_exited | agent_closed
}

/// 委派端点:pane + Agent 身份是路由护栏(window/tab 坐标只是观察值——tab 会移动、索引不是稳定身份)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeDelegationEndpoint {
    pub window_id: u64,
    pub pane_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")] pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub generation: Option<u64>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeDelegationReceipt {
    pub task_id: String,              // "delegation-<pid>-<单调序>"
    pub origin: RuntimeDelegationEndpoint,
    pub target: RuntimeDelegationEndpoint,
    pub status: String,
}
```

### snapshot 族(本篇锚点,照抄 + 单窗/pane 类型收编)

```rust
/// 唯一状态权威的快照。revision 单调,语义内容不变不加 revision、不重发事件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub process_id: u32,
    pub app_version: String,          // env!("VERSION")
    pub protocol_version: u16,
    // pebrel 的 detached_windows 随驻留语义砍;单窗口恒一元素、window_id = 1 常量
    pub windows: Vec<RuntimeWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pane_lifecycles: Vec<RuntimePaneLifecycle>,
}
// 定位辅助照抄:pane(window_id, pane_id) / pane_target(...) -> Result<(window_id, &RuntimePane)>
//   [] => target_not_found;多命中 => ambiguous_target(单窗口下不可达,协议分支保留)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeWindow {
    pub id: u64,                      // 单窗口 = 1(壳常量 SINGLE_WINDOW_ID)
    pub focused: bool,
    // session_exempt(提权隔离)不迁
    pub active_tab: usize,            // 位置索引(活跃定位)
    pub focused_pane_id: Option<u64>,
    pub tabs: Vec<RuntimeTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeTab {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<TabId>,        // 05 篇锚点,serde 透明序列化为 u64;跨重排稳定身份
    pub index: usize,                 // 窗口内零基索引(与 tab_id 双轨定位)
    pub active: bool,
    pub label: String,
    pub kind: String,                 // WorkspaceTab 变体名 snake_case(terminal/settings/…归 05 注册表)
    pub bell: bool,
    pub focused_pane_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoomed_pane_id: Option<u64>,
    pub layout: Option<RuntimeLayout>,
    pub panes: Vec<RuntimePane>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeLayout {
    Pane { pane_id: u64 },
    Split { direction: RuntimeSplitDirection, ratio: f32,
            first: Box<RuntimeLayout>, second: Box<RuntimeLayout> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSplitDirection { LeftRight, TopBottom }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePane {
    pub id: u64,                      // PaneId.0(05 篇)
    pub active: bool,
    pub title: String,
    pub cwd: String,
    pub branch: String,
    // ssh_destination 随 SSH 整支砍
    pub running_program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<RuntimeAgent>,
    pub task_state: RuntimeTaskState,
    /// 单调跃迁计数:state 值本身回答不了「我提交之后有没有动过」,
    /// 发送方取提交后基线、等待方只承认 seq > after_seq。
    /// publish 时盖戳(见 RuntimeHub);投影调用方留 0。首见 pane 从 1 起,0 非法基线。
    #[serde(default)]
    pub state_change_seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_run: Option<RuntimePaneRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<RuntimeRunOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePaneLifecycle {
    pub sequence: u64,
    pub window_id: u64,
    pub pane_id: u64,
    pub event: RuntimePaneLifecycleKind,   // Closed | Exited → pane_closed / pane_exited
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub executable: String,
    pub display_name: String,
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_kind: Option<String>,   // 03 篇 AgentKind::parse(display_name) → slug
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePaneProcesses {
    pub window_id: u64, pub pane_id: u64, pub root_pid: u32,
    pub processes: Vec<RuntimeProcess>,
}
```

### run 结果族(本篇锚点,照抄;OSC 133 缝合归 02 篇)

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRunPhase { Submitted, Started }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePaneRun { pub run_id: u64, pub phase: RuntimeRunPhase }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRunState { Finished, Failed, Unavailable }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExitCodeCapability { Supported, Unavailable }

/// 只认 OSC 133 CommandDone 携带的真实 exit code;无集成时
/// exit_code_unavailable / run_start_timeout / run_aborted,绝不把未知结果伪造成 0。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRunOutcome {
    pub run_id: u64,
    pub state: RuntimeRunState,
    pub exit_code: Option<i32>,
    pub exit_code_capability: ExitCodeCapability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,  // command_start_not_observed / exit_code_not_reported
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRunResult {
    pub window_id: u64,
    pub pane_id: u64,
    #[serde(flatten)]
    pub outcome: RuntimeRunOutcome,
}
```

### 控制键(本篇锚点,照抄)

```rust
/// pane.send_key 命名键白名单;可打印文本有意缺席(文本输入走 pane.prompt),
/// 字母必须配 control=true,repeat ≤ MAX_KEY_REPEAT,API 不接受任意字节/ANSI 串。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKey {
    Escape, Enter, Tab, Backspace, Up, Down, Left, Right, Home, End, Insert, Delete,
    PageUp, PageDown, F1, /* … */ F12, A, /* … */ Z,
}
impl RuntimeKey { pub fn as_str(self) -> &'static str; pub(crate) fn letter(self) -> Option<char>; }

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeKeyModifiers {
    #[serde(default)] pub shift: bool,
    #[serde(default)] pub alt: bool,
    #[serde(default)] pub control: bool,
}
```

### 派发原语(本篇锚点,照抄 + 单壳收编)

```rust
/// UI 写操作命令全集(单窗收编:NewWindow 砍,CloseWindow 语义 = 唯一当前窗口)。
/// 普通请求与编排步骤共享同一枚举,经同一派发原语执行——
/// worktree 事务、run 等待、UI 超时不许在两条路径上产生语义分叉。
#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    Snapshot,
    Tab { window_id: Option<u64>, request: tabs::Request },
    // NewWindow(原 window.create)不迁
    CloseWindow { window_id: Option<u64> },
    Focus { window_id: Option<u64>, pane_id: Option<u64> },
    NewTab { window_id: Option<u64>, cwd: Option<PathBuf>, shell_id: Option<String> },
    CloseTab { window_id: Option<u64>, tab_index: usize, tab_id: Option<TabId> },  // 双轨定位,eg 改造节
    RenameTab { window_id: Option<u64>, tab_index: usize, tab_id: Option<TabId>, name: String },
    MoveTab { window_id: Option<u64>, tab_index: usize, tab_id: Option<TabId>, to_index: usize },
    Split { window_id: Option<u64>, pane_id: Option<u64>, direction: RuntimeSplitDirection },
    ClosePane { window_id: Option<u64>, pane_id: u64 },
    ZoomPane { window_id: Option<u64>, pane_id: u64, zoomed: bool },     // 幂等显式值,不用 toggle
    ResizePane { window_id: Option<u64>, pane_id: u64, ratio: f32 },     // 直接父分屏占比,钳 0.05–0.95
    Prompt { window_id: Option<u64>, pane_id: u64, text: String, submit: bool },
    Paste { window_id: Option<u64>, pane_id: u64, text: String, submit: bool },
    ReadPane { window_id: Option<u64>, pane_id: u64, lines: usize },
    Procs { window_id: Option<u64>, pane_id: u64 },
    SendKey { window_id: Option<u64>, pane_id: u64, key: RuntimeKey,
              modifiers: RuntimeKeyModifiers, repeat: u16 },
    Run { window_id: Option<u64>, pane_id: u64, command: String, wait: bool, timeout_ms: u64 },
    Exec { window_id: Option<u64>, pane_id: u64, argv: Vec<String>,
           timeout_ms: u64, max_output_bytes: usize },
    Git { window_id: Option<u64>, pane_id: u64, request: git::Request },
    AgentStart { window_id: Option<u64>, pane_id: Option<u64>, name: String,
                 kind: slterm_app::ai_agents::AgentKind,      // 03 篇锚点
                 cwd: Option<PathBuf>, session_id: Option<String>, command: String,
                 worktree: Option<slterm_app::git_worktree::WorktreeProvenance> },
    AgentFork { window_id: Option<u64>, source_pane_id: Option<u64>, source_cwd: Option<PathBuf>,
                name: String, kind: slterm_app::ai_agents::AgentKind, session_id: Option<String>,
                command: String, branch: Option<String>, base: Option<String>,
                path: Option<PathBuf>, allow_dirty_source: bool },
    AgentPrompt { agent: String, generation: Option<u64>, text: String, submit: bool },
    AgentPaste { agent: String, generation: Option<u64>, text: String, submit: bool },
    AgentRead { agent: String, generation: Option<u64>, lines: usize },
}

/// 写操作跨线程回执:transport 线程持 receiver,UI owner 执行后 respond。
#[derive(Debug)]
pub struct RuntimeDispatch {
    pub command: RuntimeCommand,
    reply: SyncSender<Result<serde_json::Value, ApiError>>,
}
impl RuntimeDispatch {
    fn new(command: RuntimeCommand) -> (Arc<Self>, Receiver<Result<serde_json::Value, ApiError>>);
    pub(crate) fn respond(&self, response: Result<serde_json::Value, ApiError>);
}

/// 单壳唯一形态(原 EventSink::Winit 双形态不迁)。
/// ATTACH 为 fire-and-forget(唤醒/还原窗口);Control 等待 respond。
#[derive(Clone)]
pub enum RuntimeCallback { Attach, Control(Arc<RuntimeDispatch>) }

/// 单形态事件汇(原双壳 EventSink 收敛):闭包桥进壳事件队列,驱动方为壳归 M3。
pub(crate) type EventSink = Arc<dyn Fn(RuntimeCallback) + Send + Sync>;
```

### RuntimeHub(本篇锚点,照抄 + 单窗收窄)

```rust
#[derive(Clone, Default)]
pub struct RuntimeHub { inner: Arc<Mutex<HubState>> }

#[derive(Default)]
struct HubState {
    current: Option<RuntimeSnapshot>,
    next_subscription: u64,
    subscribers: Vec<(u64, SyncSender<RuntimeSnapshot>)>,     // 有界 channel(cap 16)
    next_pane_lifecycle: u64,
    pane_lifecycles: VecDeque<RuntimePaneLifecycle>,
    next_run_waiter: u64,
    run_waiters: HashMap<(u64, u64, u64), Vec<(u64, SyncSender<RuntimeRunResult>)>>, // (win, pane, run_id)
    completed_runs: VecDeque<RuntimeRunResult>,
    agent_generations: HashMap<String, u64>,                 // name → 单调 generation
    managed_agents: HashMap<String, RuntimeManagedAgent>,
    next_delegation: u64,
    pending_delegations: HashMap<String, PendingDelegation>,
    pending_delegation_callbacks: VecDeque<RuntimeDelegationCallback>,
    // pebrel 的 pending_pane_relocations / move_panes_to_window 随多窗口砍(不变更项 2)
}

impl RuntimeHub {
    pub fn new() -> Self;
    /// 只发布语义变化:重复事件循环唤醒不烧 revision、不刷订阅方;
    /// 先盖 state_change_seq 再判等——状态未变的 pane 保留旧计数,否则相等比较必假。
    pub(crate) fn publish(&self, snapshot: RuntimeSnapshot) -> RuntimeSnapshot;
    fn subscribe(&self) -> (u64, Option<RuntimeSnapshot>, Receiver<RuntimeSnapshot>);
    fn current(&self) -> Option<RuntimeSnapshot>;
    pub(crate) fn record_pane_closed(&self, window_id: u64, pane_id: u64);
    pub(crate) fn record_pane_exited(&self, window_id: u64, pane_id: u64);

    // managed agent 注册表:register 时查重(active 同名 → agent_name_conflict)、
    // generation = 旧值+1(同名跨会话单调);closed_reason 分 agent_exited/agent_replaced/pane_*
    pub(crate) fn register_agent(&self, name: String, kind: AgentKind, window_id: u64,
        pane_id: u64, session_id: Option<String>,
        worktree: Option<WorktreeProvenance>) -> Result<RuntimeManagedAgent, ApiError>;
    pub(crate) fn ensure_agent_name_available(&self, name: &str) -> Result<(), ApiError>;
    pub(crate) fn close_agent(&self, agent_id: &str, reason: &str);   // 注册表纯变化也重发快照唤醒等待
    pub(crate) fn active_agent(&self, selector: &str, generation: Option<u64>)
        -> Result<RuntimeManagedAgent, ApiError>;
    //   选择序:id 精确 → (name, 精确 generation) → (name, 最新 active);
    //   generation 不符 → agent_identity_mismatch(details 带 expected/actual);
    //   非 active → closed_reason 即错误码(agent_exited/agent_replaced/…),不允许静默换目标

    // 委派:每目标单在途(delegation_in_progress)、容量 MAX_PENDING_DELEGATIONS、
    // 不可自委派;origin 身份 = 同 pane 的 RuntimeAgent(generation/session 二选一必须有)
    pub(crate) fn begin_delegation(&self, origin_pane_id: u64, target: &RuntimeManagedAgent)
        -> Result<RuntimeDelegationReceipt, ApiError>;
    pub(crate) fn cancel_delegation(&self, task_id: &str);
    pub(crate) fn take_delegation_callback(&self, agent_id: &str, generation: u64)
        -> Option<RuntimeDelegationCallback>;   // 消费归 03 篇回合完成链
}

/// state_change_seq 盖戳(照抄语义,记账键随 05 篇 PaneId 全 workspace 唯一而收窄为 pane id 单维):
/// 上一快照同 pane 状态未变 → 保留旧计数;变了 → +1;首见 → 1。
fn stamp_state_change_seq(previous: Option<&RuntimeSnapshot>, next: &mut RuntimeSnapshot);
```

### 等待参数与匹配(`runtime_api/command.rs`,本篇锚点,照抄)

```rust
#[derive(Debug, Deserialize)] #[serde(deny_unknown_fields)]
pub(super) struct WaitParams {
    #[serde(default)] pub(super) window_id: Option<u64>,
    pub(super) pane_id: u64,
    pub(super) state: RuntimeWaitState,
    pub(super) timeout_ms: u64,                       // 1..=86_400_000,越界 invalid_params
    #[serde(default)] pub(super) after_seq: Option<u64>,  // 提交时捕获的基线
}

#[derive(Debug, Deserialize)] #[serde(deny_unknown_fields)]
pub(super) struct SubscribeParams { #[serde(default)] pub(super) since_revision: Option<u64> }

/// 命中 = 状态匹配 且(带基线时)seq > after_seq——否则对本来就 idle 的 pane
/// 等 idle 会立即返回,调用方误以为提交的工作已完成(shell 都还没看到输入)。
pub(super) fn wait_matches(pane: &RuntimePane, expected: RuntimeWaitState,
                           after_seq: Option<u64>) -> bool;
pub(super) fn wait_state_matches(actual: RuntimeTaskState, expected: RuntimeWaitState) -> bool;

// 输入校验(语义照抄,常量首发值见上):
pub(crate) fn validate_prompt(text: &str) -> Result<(), ApiError>;      // 非空 / ≤32KiB / 禁一切控制字符(单行)
pub(crate) fn validate_paste_text(text: &str) -> Result<(), ApiError>;  // 保留 CR/LF/TAB,拒 ESC/NUL 等
pub(crate) fn validate_command_line(command: &str) -> Result<(), ApiError>; // pane.run 单行
pub(crate) fn validate_chat_message(text: &str) -> Result<(), ApiError>;   // 多行但同样拒控制字符(内部用,不放宽 prompt)
fn validate_tab_name(name: &str) -> Result<(), ApiError>;                 // ≤256B / 禁控制字符
fn validate_exec_argv(argv: &[String]) -> Result<(), ApiError>;           // 非空 program / ≤256 元 / 总 ≤32KiB / 禁 NUL
pub(super) fn validate_agent_name(name: &str) -> Result<(), ApiError>;    // 1..=64B,禁首尾空白/控制字符
pub(super) fn validate_agent_selector(agent: &str) -> Result<(), ApiError>; // ≤128
pub(super) fn parse_params<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError>; // deny_unknown_fields 由 T 承担
pub(crate) fn capture_process_tree(window_id: u64, pane_id: u64, root_pid: u32)
    -> Result<RuntimePaneProcesses, ApiError>;   // process_tree::descendants + AgentKind 标注,eg 03 篇
```

### 终端读取与非 TTY 执行(本篇锚点,照抄 + 裁剪)

```rust
// runtime_api/terminal_read.rs —— pane.read 直读真实 Grid 尾部:
// 锁 Term 经 bounds_to_string 读 scrollback,不截图、不动用户滚动位置/选区/光标;
// 范围锚定 buffer 底部,不足一屏时多扫 TAIL_SCAN_EXTRA_LINES 再回退最后 N 个非空行;
// 字节上限 MAX_READ_BYTES,超限 byte 截断 + truncated=true。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePaneRead {
    pub window_id: u64,
    pub pane_id: u64,
    pub text: String,
    pub requested_lines: usize,
    pub returned_lines: usize,
    pub history_available: usize,    // = total_lines - screen_lines
    pub truncated: bool,
    pub task_state: RuntimeTaskState,
    pub exited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
    // screen: Option<RuntimeTerminalScreen> 随 mobile screen v1 不迁(首发只抄文本尾部)
}
pub(crate) fn capture_terminal_tail<T: EventListener>(term: &Term<T>, lines: usize,
    task_state: RuntimeTaskState, exited: bool, exit_reason: Option<String>)
    -> Result<RuntimePaneRead, ApiError>;   // Term/Grid 归 02 篇,本篇只消费

// runtime_exec.rs —— pane.exec 独立非 TTY 子进程:
// 直接收 argv 无 shell 展开;环境语义 = 「继承宿主进程 + PTY 创建时冻结的 overrides」,
// 运行中 shell 的 export 不会反向流回,故不假装能继承;不进 Grid/history、不改交互 shell 状态。
#[derive(Clone, Debug)]
pub(crate) struct PaneExecContext {
    shell_program: Option<String>,
    env: HashMap<String, String>,
    fallback_cwd: Option<PathBuf>,
    // ExecLocation 仅留 Host(Wsl 分支不迁);凭据护栏 for_git():GIT_TERMINAL_PROMPT=0 /
    // GCM_INTERACTIVE=never / SSH_ASKPASS_REQUIRE=never(纯 env,无 WSLENV 臂)
}
impl PaneExecContext {
    pub(crate) fn from_pty_options(options: &slterm_terminal::tty::Options) -> Self;
    pub(crate) fn shell_program(&self) -> Option<&str>;
    pub(crate) fn process_instance(&self) -> Option<&str>;  // SLTERM_PROCESS_ID 双因子归 03 篇
    pub(crate) fn for_git(self) -> Self;
}
pub(crate) fn spawn(dispatch: Arc<RuntimeDispatch>, context: PaneExecContext, cwd: String);
// stdout/stderr 并行排水各上限 max_output_bytes;超时回收整个进程树(归 02 篇 Job Object
// 形态的平台件),保留已捕获输出;响应 {success, timed_out, stdout, stderr, exit_code…}
```

### 编排类型(`runtime_api/orchestrate.rs`,本篇锚点,照抄)

```rust
#[derive(Debug, Deserialize)] #[serde(deny_unknown_fields)]
struct OrchestrateParams { steps: Vec<OrchestrateStep>, #[serde(default)] on_error: OnError }
// on_error: stop(默认)|continue;中途失败保留已完成动作,回执带 failed_step,调用方从失败步续作

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum OrchestrateStep {
    NewTab { id: String, #[serde(default)] window_id: Option<u64>, #[serde(default)] cwd: Option<PathBuf> },
    Focus { id: String, target: PaneTarget },
    Split { id: String, #[serde(default)] window_id: Option<u64>,
            #[serde(default)] target: Option<PaneTarget>, direction: RuntimeSplitDirection },
    Prompt { id: String, target: PaneTarget, text: String, #[serde(default = "default_true")] submit: bool },
    Run { id: String, target: PaneTarget, command: String, #[serde(default = "default_true")] wait: bool,
          #[serde(default = "default_command_timeout_ms")] timeout_ms: u64,
          #[serde(default)] tail_lines: Option<usize> },
    AgentLaunch { id: String, target: PaneTarget, name: String, kind: String,
                  #[serde(default)] resume_session_id: Option<String>,
                  initial_prompt: String,
                  #[serde(default = "default_ready_timeout_ms")] ready_timeout_ms: u64 },
    Wait { id: String, target: PaneTarget, state: RuntimeWaitState,
           #[serde(default = "default_command_timeout_ms")] timeout_ms: u64,
           #[serde(default)] tail_lines: Option<usize> },
    // wait 的 after_seq 基线自动取自被引用步骤的回执——「发 prompt 再等 settled」
    // 不会命中提交前那个旧空闲态
}

/// 步骤引用只允许 {step, field: "pane_id"} 的结构化回指;未来引用/重复 id/未知字段
/// 在任何 UI 动作之前被拒(invalid_reference / invalid_params)。
#[derive(Debug, Deserialize)] #[serde(untagged)]
enum PaneTarget { Reference(StepReference), Existing(ExistingPane) }
#[derive(Debug, Deserialize)] #[serde(deny_unknown_fields)]
struct StepReference { step: String, field: ReferenceField }
#[derive(Debug, Clone, Copy, Deserialize)] #[serde(rename_all = "snake_case")]
enum ReferenceField { PaneId }
#[derive(Debug, Deserialize)] #[serde(deny_unknown_fields)]
struct ExistingPane { pane_id: u64, #[serde(default)] window_id: Option<u64> }

#[derive(Debug, Serialize)] struct WorkflowReceipt {
    workflow_id: String, ok: bool, duration_ms: u64, partial: bool,
    completed: usize, failed_step: Option<String>, steps: Vec<StepReceipt>,
}
#[derive(Debug, Serialize)] struct StepReceipt {
    id: String, op: &'static str, ok: bool, duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")] action: Option<Value>,   // 只带必要字段
    #[serde(skip_serializing_if = "Option::is_none")] error: Option<ApiError>,
}
pub(super) fn validate_params(value: &Value) -> Result<(), ApiError>; // 全量预检:步骤数/字节预算/引用图
pub(super) fn orchestrate_connection(stream: &mut TcpStream, request: ApiRequest,
    sink: &EventSink, hub: &RuntimeHub) -> Result<(), IoError>;
pub(super) fn wait_agent_ready(hub: &RuntimeHub, agent_id: &str, generation: u64, deadline: Instant)
    -> Result<(), ApiError>;   // 先并行启动全部 Agent,再等 ready 后投首任务;超时 agent_ready_timeout
```

### 端点、服务器与移交(`runtime_api/server.rs`,本篇锚点,照抄 + 单实例改造)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Endpoint { pub(super) port: u16, pub(super) token: String }
// 端口文件 runtime.port(文件名 = 不变,归 01 篇 D 节;目录 = settings data_dir):
// 内容 "<port> <128bit-token> <protocol_version>\n"
// token = 两个 OS 种子 RandomState 哈希拼 32 hex;只挡其他本机用户,不承诺抵御同用户攻击者

pub(crate) const ENDPOINT_ENV: &str = "SLTERM_RUNTIME_ENDPOINT";   // 01 篇 C 节
static CHILD_ENDPOINT: Mutex<Option<Endpoint>> = Mutex::new(None); // 本进程作为子进程时的宿主端点

/// 私有实例(不写端口文件)的 pane 照样能发现控制面:
/// 注入前按名清旧值(Windows 大小写不敏感),有 CHILD_ENDPOINT 才写 "<port> <token>"。
pub(crate) fn apply_child_endpoint(env: &mut HashMap<String, String>);

fn parse_endpoint(data: &str) -> Option<Endpoint>; // port 非 0、token 恰 32 位 hex
pub(super) fn read_endpoint() -> Option<Endpoint>;
// 三级发现优先级:本进程 CHILD_ENDPOINT > 显式 env 端点 > 端口文件。
// 显式 env 端点存在但非法 → 返回 None,绝不回落端口文件选别的实例(防选错)。
// 兼容回落位(read_endpoint_from(legacy 端口文件))仅作扩展位保留,首发不启用(eg 不变更项 9)。

/// 二次启动移交(机制照抄,语义单窗化;成功 = 对方回 OK 且本进程应退出):
pub fn try_open_default_tab_existing(shell: Option<&str>) -> bool;
pub fn try_open_directory_existing(dir: &std::path::Path, shell: Option<&str>) -> bool;
// try_open_window_existing / window.create 随多窗口不迁
fn try_open_tab_existing(dir: Option<&std::path::Path>, shell: Option<&str>) -> bool;
//   先发 legacy "ATTACH <token>\n"(唤醒/还原窗口,fire-and-forget 语义),
//   再发版本化 JSON tab.new;两者落同一事件队列,窗口先恢复、新标签随后创建。
//   handover_params 只带有值键(cwd/shell),空 shell 当没给——跨构建交接时对方 serde(default) 走原行为。
fn legacy_request_to(verb: &str, endpoint: &Endpoint) -> Option<()>;
//   connect HANDOVER_CONNECT_TIMEOUT(400ms),IO HANDOVER_IO_TIMEOUT(700ms);
//   陈旧端口文件(被杀进程残留)由 connect 超时兜底 → 本进程接管,移交降级从不挂起。

pub struct RuntimeServer {
    endpoint: Endpoint,
    port_file: Option<PathBuf>,           // None = 私有实例(env-only 发现,测试/嵌套用)
    _owner_lock: Option<LifetimeFileLock>, // OS 句柄锁:进程死即释放,健康进程永不被超时回收
}
impl RuntimeServer {
    pub fn spawn_callback(on_event: impl Fn(RuntimeCallback) + Send + Sync + 'static,
                          hub: RuntimeHub) -> Option<Self>;
    fn spawn_at(sink: EventSink, hub: RuntimeHub, path: Option<PathBuf>) -> Option<Self>;
    // 启动序:LifetimeFileLock(拿不到 = 有主,stay client-only)→ PING 既有 endpoint
    //   (通 = 有活实例,stay client-only)→ bind 127.0.0.1:0 → fresh_token → 原子写端口文件
    //   → spawn serve 线程(线程名 slterm-runtime-api归 01 篇 P 节表)→ 登记 CHILD_ENDPOINT。
    // 单实例定位下无 requires_isolation 分支:端口文件恒写(私有实例由调用方显式传 None归测试)。
}
impl Drop for RuntimeServer {
    fn drop(&mut self); // 仅当端口文件现内容仍等于自己的 endpoint 才删除(不删别进程的新记录)
}
```

### transport 主链(`runtime_api/transport.rs`,本篇锚点,照抄)

```rust
pub(super) fn serve(listener: TcpListener, token: String, sink: &EventSink, hub: RuntimeHub);
// accept 循环按 MAX_CLIENTS 计数、每连接一线程(线程名可辨)、ActiveClient Drop 归还配额
pub(super) fn handle_connection(stream: TcpStream, token: &str, sink: &EventSink, hub: &RuntimeHub)
    -> Result<(), IoError>;
// 读超时 IO_TIMEOUT;请求体 take(MAX_REQUEST_BYTES+1),超限 request_too_large;
// 首行 trim_start 非 '{' → handle_legacy;JSON 解析 → token 校验(不符静默 Ok(()),先于详细校验)
//   → deny_unknown_fields 反序列化 → protocol/version 校验(protocol_version_mismatch +
//   supported_versions)→ 方法分派:
//   runtime.describe / events.subscribe / agents.list / agent.get / agent.delegate /
//   agent.wait / pane.wait / runtime.orchestrate 专链,其余 dispatch_connection
pub(super) fn handle_legacy(stream: &mut TcpStream, line: &str, token: &str, sink: &EventSink)
    -> Result<(), IoError>;   // "ATTACH <token>" → sink Attach + OK;"PING <token>" → OK;token 不符静默丢弃
pub(super) fn runtime_description() -> Value;      // 见下
pub(super) fn subscribe_connection(...);  // 首行确认(subscription_id/current_revision),随后每行一快照;
                                          // since_revision 断点续传;语义不变不重发
pub(super) fn wait_connection(...);       // wait 专链:超时 details 带 after_seq 与 observed_state_change_seq
                                          // (区分「一直没动」与「跃迁了但没到目标态」),生命周期错误优先于超时
pub(super) fn dispatch_connection(...);   // RuntimeCommand::from_request + 关键方法审计日志(不记正文)
pub(super) fn dispatch_runtime_command(command: RuntimeCommand, sink: &EventSink, hub: &RuntimeHub)
    -> Result<Value, ApiError>;
//   prepare_dispatch_command(worktree 事务)→ 计算派发超时(Exec = timeout+2s,余 COMMAND_TIMEOUT)
//   → emit_control 失败 = runtime_unavailable + worktree 回滚
//   → recv_timeout:Ok → 事务 commit + 附 provenance;Run{wait} → wait_run_phased 分段限时
//   → 超时 = runtime_timeout + worktree cleanup_deferred=true 保留 checkout
//     (晚到 PTY 可能已在使用刚被删的 cwd,绝不在未知态回收)
pub(super) fn write_json_line<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), IoError>;
```

`runtime_description()` 的 slTerminal 能力面(裁剪后成文,全部枚举归 Schema 单文件归改造节):

- `capabilities`(首发集合):`runtime.describe` / `runtime.snapshot` / `runtime.orchestrate` / `events.subscribe` / `agents.list` / `agent.start` / `agent.fork` / `agent.get` / `agent.delegate` / `agent.prompt` / `agent.paste` / `agent.read` / `agent.wait` / `window.close` / `window.focus` / `tab.new` / `tab.close` / `tab.rename` / `tab.move` / `pane.split` / `pane.close` / `pane.zoom` / `pane.resize` / `pane.prompt` / `pane.paste` / `pane.read` / `pane.procs` / `pane.send_key` / `pane.run` / `pane.exec` / `pane.wait` / `git.status` / `git.diff` / `git.rollback` / `git.unstage`。pebrel 的 `window.create`、gpui-shell 追加的 `tab.focus`/`tab.open`/`tab.read`、`conversation.*`、git 五写方法均不在首发面(扩展位归 07 篇如未来需要)。
- `features` 探测串:`pane.wait.after_seq` / `pane.wait.lifecycle` / `agent.wait.identity` / `agent.delegate.callback` / `agent.fork.transactional_worktree` / `agent.worktree.provenance` / `events.pane_lifecycle` / `runtime.orchestrate.typed_steps` / `runtime.orchestrate.agent_ready` / `env.pane_identity` / `cli.resource_verbs` / `cli.paste_sources` / `pane.exec.non_tty`。`layout.typed_mutations` 随 mobile bridge 不迁;`pane.read.screen.v1` 首发不开放。
- `env` 段:term_program / pane / cli / bin_dir / process / `bin_dir_on_path: true`;remote_marker 不迁(SSH 砍)。
- `commands` 段(env/window/tab/pane/agent 资源动词清单,与 capabilities 同语义不同入口,eg 改造节 CLI)。
- 附加参数能力(如 `pane.wait.after_seq`)不可从 capabilities 探测——旧版静默忽略未知参数仍带竞态,严格客户端必须检查 feature 串;`env` 段让客户端探测契约而不硬编码变量名。

### CLI 客户端(`runtime_api/cli.rs`,本篇锚点,照抄)

```rust
fn client_stream(endpoint: &Endpoint, request: &ApiRequest, timeout: Option<Duration>)
    -> Result<TcpStream, IoError>;
// connect/read/write 全部带超时;写完后 shutdown(Shutdown::Write),再读完整响应行
pub(super) fn request_once(method: &str, params: Value, timeout: Duration)
    -> Result<ApiResponse, Box<dyn Error>>;        // 无端点 = runtime_unavailable
pub(crate) fn request_once_bounded(method: &str, params: Value, timeout: Duration)
    -> Result<ApiResponse, ApiError>;              // 插件入口:请求 ≤128KiB、响应 ≤256KiB
fn decode_plugin_response(request: &ApiRequest, bytes: &[u8]) -> Result<ApiResponse, Box<dyn Error>>;
// 响应身份逐字段校验:protocol/version/id 一致;ok ⇒ result 在且 error 无;!ok ⇒ 反之,否则 invalid_response
fn bounded_exchange(endpoint: &Endpoint, request: &[u8], timeout: Duration)
    -> Result<Vec<u8>, Box<dyn Error>>;            // 整次 I/O 共用一个 deadline;对端先关读侧容错
fn input_transport_error(method: &str, error: Box<dyn Error>) -> Box<dyn Error>;
// pane.prompt/paste、agent.prompt/paste、agent.delegate 的传输/解析失败改写为
// submission_outcome_unknown:输入可能已送达,先读目标再决定是否重试,不盲目重复投递
pub(super) fn require_submission_baseline(baseline: Option<u64>) -> Result<u64, CliError>;
// --wait 基线取自提交后快照;缺有效非零基线 → 明确报错,不降级为无基线等待
```

## 数据流与状态机

### 控制面请求全链(外部客户端 → UI owner → hub → 订阅方)

```
AI CLI / 脚本 / 未来插件(pane 内进程,经 SLTERM_CLI / SLTERM_BIN_DIR/PATH 发现入口)
  → 三级端点发现(CHILD_ENDPOINT > SLTERM_RUNTIME_ENDPOINT > runtime.port)
  → TcpStream 127.0.0.1(connect 500ms,read/write 5s)
  → transport 线程 handle_connection:
      首行分流:非 '{' 开头 → legacy ATTACH/PING(token 不符静默丢弃)
      JSON → token 校验(先认证后解析,防协议预言机)→ 信封 deny_unknown_fields
      → protocol/version 校验 → 方法分派
  ├─ 读方法(wait/subscribe/agents.list/agent.get):transport 线程直接对 hub
  │   订阅/查表,不碰 UI owner;命中即返回
  ├─ 写方法:RuntimeCommand::from_request(参数校验全在 transport 侧完成)
  │   → dispatch_runtime_command:
  │       prepare_dispatch_command(agent.fork 的 worktree 事务在 transport 线程预建)
  │     → RuntimeDispatch::new → sink.emit_control → 壳事件队列(mpsc)
  │     → GPUI foreground executor 取出,在 UI owner 上由壳执行(归 M3/05 篇 Workspace 变更)
  │     → dispatch.respond(Ok/Err)→ transport recv_timeout(COMMAND_TIMEOUT / Exec 超时 +2s)
  │     → Run{wait} → wait_run_phased(hub 等待 OSC 133 真实 exit code)
  │   → 响应信封写回(显式 null 有效;identity 逐字段)
  └─ 事件订阅:subscribe_connection 首行确认 → hub.subscribe 有界 channel(cap 16)
      → publish 语义变化时逐行推送 ApiEvent(runtime.snapshot)
```

### 壳与外部客户端消费同一权威(「壳即前端」的机质)

```
Workspace(05 篇,UI owner 属主)
  → 投影函数(归 05 篇壳层,本篇给契约:Workspace → RuntimeSnapshot 草稿)
      · pane 集合 == 树叶集合 / PaneId 单调 / TabId 稳定身份归 05 篇不变式
      · task_state/agent 字段 = 每 pane AgentActivity(03 篇)的 RuntimeTaskState 投影
      · active_run/last_run = 02 篇 OSC 133 事件链在壳上记账
  → RuntimeHub::publish
      · stamp_state_change_seq(语义跃迁才 +1)→ 与上一快照判等
      · 相等 → 不烧 revision、不重发(事件循环空转不刷屏)
      · 不等 → revision+1、换 current、notify 全部订阅方(含 wait 专链与外部 subscribe)
  → 消费端:
      · 壳内侧栏/托盘归 09/05:直接读 Workspace/最新快照投影,不解析标题
      · 外部客户端:loopback subscribe / snapshot / wait
      · CLI 薄别名:与完整协议共用 request 函数、响应信封、generation/after_seq 保护
```

单一Reducer 承诺:`RuntimeTaskState` 六态是侧栏、托盘、snapshot、`agents.list`、`pane.wait` 共用的唯一投影(03 篇仲裁器为语义源);任何消费方不得另起状态判读。

### 等待状态机(after_seq 竞态堵死链)

```
提交方:
  1. pane.prompt/agent.prompt 成功 → 响应内取该 pane/agent 的 state_change_seq(提交后快照)
  2. 作为 after_seq 传给 pane.wait/agent.wait;CLI --wait 内部已串好这两步
  3. 缺有效非零基线 → 明确报错(require_submission_baseline),不降级无基线等待
     --no-submit 与 --wait 由参数解析直接拒绝(逻辑矛盾,不执行一半静默停)
等待方(wait_connection):
  · 先验当前快照:已匹配 → 立即成功
  · 未匹配 → 记录 observed(task_state, seq),进入 recv_timeout 循环
  · 命中条件 = wait_state_matches ∧ (无基线 ∨ seq > after_seq)
  · 超时 → timeout + details{after_seq, observed_state_change_seq}
    ——区分「一直没动」与「跃迁了但没到目标态」
  · 等待期间 pane_closed/pane_exited → 对应生命周期错误优先返回(不等到超时)
agent.wait 追加 identity 重验:每次快照到达先重查 active_agent(generation 不变),
  agent_exited/agent_replaced/agent_identity_mismatch 立即失败,不静默换目标
```

### 提交时同步建立 running 边沿(照抄语义,落点归壳)

壳在 `pane.prompt` 成功提交 Enter 的**同步动作**里先建立 `running` 边沿(`state_change_seq` 随之 +1),再由真实 shell/hook 结束事件归位。因此即使命令在 runtime pump 两拍之间完成,`--wait` 观察到的也是提交后的新计数,不会把提交前的 `idle` 当成「已完成」。此机制是协议等待语义成立的壳侧前提,eg 02 篇 prompt 提交通路与 03 篇仲裁器的汇合点。

### 单实例移交状态机(启动期)

```
进程启动(早于窗口创建):
  RuntimeServer::spawn_callback:
    ├─ 拿 LifetimeFileLock 失败(端口文件被锁)→ stay client-only
    ├─ PING 既有 endpoint 通 → stay client-only
    └─ 成功 → 写端口文件 → serve 线程 → 本进程 = 首实例
  spawn 返回 None(已有实例)且本次启动无显式命令:
    → 移交重试环(40 × 25ms,防与对端抢锁竞态):
        ATTACH(唤醒/还原窗口)→ tab.new(handover_params: 仅带值键 cwd/shell)
        成功 → 托盘/服务未启动即退出(本进程消亡,零窗口残留)
  端口文件陈旧(被杀进程残留):
    → connect 400ms 超时 → 判无活实例 → 本进程接管(重写端口文件),移交降级从不挂起
运行期:
  关窗 = 退出(无驻留);RuntimeServer Drop 删端口文件(仅当自己仍是最新记录)
  被杀 → 端口文件残留 → 由下一份启动的 connect 超时兜底接管
```

### 端点注入与发现数据流(私有实例形态照抄)

```
首实例 serve 成功 → CHILD_ENDPOINT = Some(endpoint)(进程级,不发盘)
每次 PTY spawn(02 链归 03 篇 agent_env::apply 末位):
  apply_child_endpoint(env):
    · 按名(Windows 大小写不敏感)清掉继承来的旧 ENDPOINT_ENV
    · CHILD_ENDPOINT 有值 → 注入 "<port> <token>"
    → 本实例 pane 的子进程树天然携带端点,私有实例 pane 照样可回控
二次启动移交请求同走三级发现链;兼容回落位(旧版端口文件)扩展位保留、首发不启用
显式 SLTERM_RUNTIME_ENDPOINT 存在但解析失败 → None,绝不回落端口文件(防选错实例)
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(33 条采纳点逐条覆盖;薄写:pebrel 源 · 符号 + 缝合点 + 因果链一句)

**传输与发现(采纳点 1–5)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 1 | `nebula_app/src/runtime_api/server.rs` `RuntimeServer::spawn_at`/`fresh_token`/`port_file`/`Endpoint`/`parse_endpoint` | `slterm_app/src/runtime_api/server.rs` 全量改 `slterm.runtime`/数据目录归 01 篇 | loopback + 128bit token 只挡本机其他用户,是最小可用发现面 |
| 2 | 同上 `spawn_at` 锁序与 `Drop`;`nebula_app/src/atomic_file.rs` `write`/`try_lifetime_lock` | 原子写归 05/06 共有件;owner 锁归本篇 | LifetimeFileLock 句柄锁进程死即释放;原子写 + Drop 仅删自己记录,两进程不抢文件 |
| 3 | `nebula_app/src/runtime_api.rs` `ApiRequest`/`ApiResponse`/`ApiEvent`/`ApiError`/`PROTOCOL_NAME`/`PROTOCOL_VERSION`/`SUPPORTED_VERSIONS`;`docs/runtime-api-v1.schema.json` | 本篇锚点;protocol 常量改 `slterm.runtime`,Schema 单文件随迁归改造节 | JSON Lines + deny_unknown_fields + 版本协商把「旧客户端撞新服务端」变成显式错误 |
| 4 | `nebula_app/src/runtime_api/transport.rs` `handle_connection`;`nebula_app/src/runtime_api/cli.rs` `client_stream`/`decode_plugin_response` | transport 与 cli 全量 | 单连接单请求 + 读超时不限大小拒(`request_too_large`)+ 客户端 `shutdown(Write)` 后读响应 + 身份逐字段校验 |
| 5 | `nebula_app/src/runtime_api/transport.rs` `serve`/`ActiveClient` | 全量(线程名归 01 篇 P 节) | accept 按 MAX_CLIENTS 计数、每连接一线程、配额 Drop 归还,拖死连接不饿死新客户端 |

**状态权威与等待语义(采纳点 6–11)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 6 | `nebula_app/src/runtime_api.rs` `RuntimeHub`/`RuntimeSnapshot`/`RuntimeWindow`/`RuntimeTab`/`RuntimePane`/`RuntimeLayout` | 本篇锚点 + 05 篇投影(单窗/pane 类型收编) | GUI/CLI/外部读同一份快照,语义不变不加 revision 不重发,状态问题可复现可审计 |
| 7 | 同上 `RuntimePane::state_change_seq`/`stamp_state_change_seq`;`runtime_api/command.rs` `WaitParams`/`wait_matches`;`runtime_api/transport.rs` `wait_connection` | 本篇锚点;记账键收窄归 05 篇 PaneId 唯一性 | seq 只在真实跃迁 +1、服务端只认 `seq > after_seq`,堵死「提交前的 idle 被判成已完成」 |
| 8 | `docs/runtime-control-api.md`「等待语义与 state_change_seq」节;落点 = 壳 prompt 提交通路 | 壳归 M3/02 篇提交通路 + 03 篇仲裁器 | 提交 Enter 同步建立 running 边沿,`--wait` 即使落在 pump 两拍之间也观察得到新 seq |
| 9 | `nebula_app/src/runtime_api.rs` `RuntimeAgent`/`RuntimeManagedAgent`/`RuntimePaneLifecycleKind`;`runtime_api/agent_api.rs` 全族 | 本篇锚点;identity 事实归 03 篇(slug/session/进程身份) | agent.* 全路径 generation 绑定,退出重开不会把任务投给新会话;两个资源名把边界摆在命令行上 |
| 10 | `docs/runtime-control-api.md` CLI 节;`runtime_api/cli.rs` `request_once`/`require_submission_baseline` | CLI 归本篇 | `--wait` 基线取提交后快照、缺基线明确报错、`--no-submit`+`--wait` 解析期拒、超时语义走 `submission_outcome_unknown` |
| 11 | `nebula_app/src/runtime_api.rs` `RuntimeTaskState`;`docs/runtime-control-api.md`「Agent 状态与终端读取」节 | 投影源归 03 篇仲裁器,消费归 05/09 | 六态单投影优先级失败>attention>等待输入>运行>完成>空闲,外部客户端不解析标题猜生命周期 |

**执行模型(采纳点 12–13)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 12 | `nebula_app/src/runtime_api.rs` `RuntimeDispatch`/`RuntimeCallback`/`EventSink`;`runtime_api/transport.rs` `dispatch_runtime_command` | 本篇锚点;EventSink 单形态归改造节;壳事件队列归 M3 | transport 只校验等待序列化,写全进 UI owner;事件循环不可用返回 `runtime_unavailable` 不伪造成功 |
| 13 | `runtime_api/transport.rs` `dispatch_runtime_command` 超时/未知态分支;`agent_api.rs` `rollback_prepared_worktree` | 本篇;worktree 事务归 git_worktree 域归 07 篇边界 | UI 超时 `runtime_timeout`;worktree 已建但结果未知 → `cleanup_deferred=true` 保留 checkout,晚到 PTY 不会用刚被删的 cwd |

**能力面(采纳点 14–24)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 14 | `runtime_api/transport.rs` `runtime_description`/`subscribe_connection` | 本篇;capabilities 裁剪归改造节 | describe 回报版本/能力/env 契约/命令清单;subscribe 首行确认 + since_revision 断点续传 |
| 15 | 同上 `runtime_description` 的 features/env 段 | 本篇 | 附加参数能力只能经 feature 串探测(旧版静默忽略未知参数仍带竞态);env 段免客户端硬编码变量名 |
| 16 | `runtime_api/command.rs` `validate_prompt`/`validate_paste_text`/`validate_command_line`;`runtime_api.rs` `RuntimeKey`/`MAX_KEY_REPEAT`/`MIN_PANE_RATIO`/`MAX_PANE_RATIO` | 本篇锚点 | prompt 单行拒控制字符、paste 留 CRLF 拒 ESC/NUL、send_key 白名单字母必配 control、zoom 幂等、resize 钳 0.05–0.95 |
| 17 | `runtime_api/terminal_read.rs` `RuntimePaneRead`/`capture_terminal_tail` | Term/Grid 归 02 篇 | 直读真实 Grid 尾部(scrollback 经 bounds_to_string),不截图不动用户滚动/选区/光标,范围锚定 buffer 底部 |
| 18 | `nebula_app/src/runtime_api.rs` `RuntimeRunOutcome`/`ExitCodeCapability`/`RUN_START_GRACE`/`wait_run_phased` | OSC 133 事件归 02 篇,记账归壳 | pane.run 只认 CommandDone 真实 exit code;无集成返回 exit_code_unavailable/run_start_timeout/run_aborted,绝不伪造 0 |
| 19 | `nebula_app/src/runtime_exec.rs` `PaneExecContext`/`spawn`(Host 分支) | 02 篇 Job Object 平台件归超时整树回收 | argv 无 shell 展开、冻结 cwd/环境、不进 Grid/history、双流并行排水有界、超时回收整树保留输出 + timed_out |
| 20 | `runtime_api/orchestrate.rs` 全族;`docs/runtime-api-v1.schema.json` `orchestrate_*` 定义族 | 本篇锚点 | 强类型 op 面 + 结构化回指 `{step, field:"pane_id"}` + 全量预检(未来引用/重复 id/未知字段在任何 UI 动作前被拒)+ 并行冷启 + 回执续作 |
| 21 | `runtime_api/agent_api.rs` 方法族;`runtime_api.rs` `RuntimeDelegationEndpoint`/`RuntimeDelegationReceipt` | 03 篇身份链(slug/session/pid);worktree eg 07 | 冷启动 kind 白名单只放官方文档核实 CLI;fork 事务序 解析→校验→建 branch+worktree→交 Tab/PTY;delegate 回传只认同 pane 同 generation、每目标单在途、worker_output 按不可信数据 |
| 22 | `runtime_api/git.rs` 校验形态;`docs/runtime-api-v1.schema.json` `git_params` | 本篇收口归改造节(四方法裁剪映射) | 写操作 expected_cwd + 服务端重查 repo root + 写前 revision 校验(outdated → git_stale);路径参数永不解释为 shell/glob |
| 23 | `runtime_api/tabs.rs`;`runtime-api-v1.schema.json` `tab_*` 定义族 | TabId 锚 05 篇;tab_id 双轨归改造节 | tab.new/close/rename/move;rename 空值恢复生成标题;窗口内零基索引 + 稳定 tab_id 双轨定位 |
| 24 | `docs/runtime-control-api.md` 方法表 `window.close`;`runtime_api/command.rs` 收编形态 | 单窗收编归改造节 | 关空闲窗口、忙碌 pane 返回显式确认错误,不静默强杀;单窗下 = 关唯一当前窗口 |

**CLI 与发现层(采纳点 25–28)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 25 | `nebula_app/src/cli.rs` verb 解析与 `ControlCommand`;`runtime_api/cli.rs`;`runtime_api/shortcuts.rs` | 本篇;clap 面归改造节(同 exe 子命令) | 资源+动词薄别名与完整协议共用请求函数/信封/竞态保护;pane 用数字 id、agent 用名字/稳定 id |
| 26 | `nebula_app/src/runtime_api/shortcuts.rs` `env`/`env_inner`/`probe_runtime`/`command_catalog` | 本篇 | 离线可用的发现命令:身份/CLI 路径/可达性/自身 pane/完整命令清单带可直接复制样例;缺什么如实报告,不在「没连上」时整体失败 |
| 27 | `docs/skills/pebrel-runtime/SKILL.md` 全文;`ai_hook/local/runtime_skills.rs` 安装器 | 本篇归改造节(SKILL 内容边界归 08) | 打包 SKILL.md 教 AI CLI 用 `slterm env` 定向后直接调 Runtime API,禁扫进程/读端口文件/grep 源码/GUI 自动化;含委派规则与安全边界 |
| 28 | `nebula_app/src/agent_env.rs` `apply`/`insert_env`/`prepended_path`/`TERM_PROGRAM`/`CLI_ENV`/`BIN_DIR_ENV`/`PROCESS_ENV` | 03 篇锚点(agent_env eg 03 篇);ENDPOINT 注入归本篇改造节 | env 契约扩展叠加既有注入,幂等判重(嵌套 shell 不增长 PATH);双名兼容层一律不抄归 01 篇 |

**与 ai_hook 的交界与单实例(采纳点 29–33)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 29 | `runtime_api/server.rs` `ENDPOINT_ENV`/`CHILD_ENDPOINT`/`apply_child_endpoint`/`read_endpoint` | env 注入点归 03 篇 agent_env::apply 末位 | `SLTERM_RUNTIME_ENDPOINT` 直接注入本实例 pane 子进程;私有实例 pane 照样可发现;显式 env 非法不静默回落 |
| 30 | 同上 `read_endpoint` 三级优先级 + `legacy_request` | 兼容回落位归改造节(扩展位保留首发不启用) | 本进程 CHILD_ENDPOINT > 显式 env > 端口文件;二次启动移交同走该链 |
| 31 | `runtime_api/server.rs` `try_open_default_tab_existing`/`try_open_directory_existing`/`try_open_tab_existing`/`handover_params` | 移交改造归改造节(驻留语义砍、单窗化) | 二次启动经 loopback 先发 ATTACH 唤醒/还原窗口再发 tab.new;「还原文档 tab 的 PTY 常驻」不搬 |
| 32 | `nebula_app/src/mux.rs` `CONNECT_TIMEOUT`/`IO_TIMEOUT`/`spawn_callback_at`/`request_at` | 模块不迁,常量与语义并入 server 移交路径(改名 HANDOVER_*) | 陈旧端口文件 connect 400ms 超时即接管;IO 700ms;移交降级从不挂起 |
| 33 | `runtime_api/transport.rs` `handle_connection`/`handle_legacy`;`mux.rs` `serve_callback` | 本篇 | 首行非 `{` 走 legacy ATTACH/PING、token 不符静默丢弃;JSON 主面首发即实现,分流位作移交探测 + 未来演进扩展位 |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威。本篇直接消费点:bin `slterm`(B-1,归改造节 CLI 方案)、`SLTERM_RUNTIME_ENDPOINT`(C 节)、`runtime.port`(= 不变,D 节)、协议名 `slterm.runtime`(本篇锚点)、线程名归 01 篇 P 节、`docs/skills/slterm-runtime/`(B-4)。pebrel 侧 `NEBULA_PANE_REMOTE`/`PEBREL_PANE_REMOTE` 相关 env 描述不迁(SSH 砍,归 03 篇);`mux.port` 兼容回落位不启用归不变更项 9。本篇域内无新增映射项。

### 缝合点

1. **03 篇(agent 状态暴露)**:runtime 的 `RuntimeTaskState` 投影源 = 每 pane `AgentActivity`(03 篇仲裁器)——本篇需要 03 篇给出的只读面:`AgentActivity::status`/`source`/`rule`/`hook_seen`/`primary_pid`;`RuntimeAgent` 字段装配:`kind = AgentKind::slug`、`session_id` 取 hooks 会话身份链、`state_source` 值集与 `AgentStatusSource` 对齐(Hook/Screen/Process 同名同义)。generation 事实来源 = hooks 的会话身份(session/panel 路由归 03 篇),`register_agent`/`close_agent` 由壳在 agent 启动与 SessionEnd/pane 关闭时调用;委派回传消费 `RuntimeHub::take_delegation_callback`,身份匹配归 03 篇回合完成链。`PROCESS_ENV` 双因子归 03 篇,`PaneExecContext::process_instance` 只消费。
2. **05 篇(pane/tab 与布局)**:`PaneId`/`TabId` 锚点直接进协议序列化(禁重定义);snapshot 投影函数(Workspace → RuntimeSnapshot 草稿,eg 数据流节)归 05 篇壳层,本篇只定 RuntimeSnapshot 形状与 publish 语义;全部写命令在 UI owner 上落为 Workspace 变更(`SplitTree` 树函数裁定结局归 05 篇,壳不自行散判);`TerminalRegistry` 对应物归 05 篇(会话元数据单点硬约束 #8 的 Rust 落点),`pane.read`/`pane.exec` 经它定位 pane 宿主。
3. **02 篇(终端核心)**:`capture_terminal_tail` 消费 `Term::bounds_to_string`(02 篇锚点);OSC 133 `CommandStart`/`CommandDone` 是 `RuntimePaneRun`/`last_run` 记账的数据源(eg 02 篇 Event 枚举);`pane.exec` 环境冻结自 PTY `Options`(02 篇 spawn 链),超时整树回收归 02 篇 Job Object 平台件;提交 running 边沿归壳的 prompt 提交通路(02 篇 `Msg` 写入)。
4. **06 篇(设置读取面)**:`tab.new`/`agent.start` 的 shell id 解析归 06 篇设置键域(默认 shell 回退链 pwsh→powershell→cmd 归 02 篇,不进协议);`windowing_behavior` 键已裁删除(B.10,单窗行为硬编码归 09 篇);describe 的 env 段变量名与 settings 常量同源归 01/06。
5. **09 篇(通知/托盘/系统面)**:ATTACH 的「还原窗口」落地 = 窗口聚焦/还原归 09 篇窗口管理;关窗退出链(托盘 Quit/最后窗口关闭)归 09 篇,RuntimeServer Drop 挂同一退出路径;托盘 agent 菜单直达归 09 消费同一 snapshot。**回登(开放问题 4 闭环)**:09 篇已答——单窗下「还原窗口」无独立语义,移交链 = 聚焦现有唯一窗 + tab.new。
6. **07 篇(文件域,扩展位)**:`tab.open`/`tab.read`/`tab.focus` 首发不开放,eg 文件 tab 点亮后如需控制面读归 07 篇登记;git 四方法的子进程封装归 07 篇 git 域(`--no-optional-locks` 纪律归 07)。
7. **08 篇(runtime skills 边界)**:SKILL.md 投放机制归本篇改造节;config_guard `heal_all` 的挂点归 03/08 篇,本篇只管 runtime skill 这一件资产的安装/保留用户编辑/卸载;其他 skill 归 08。
8. **11 篇(测试基础设施)**:conformance 测试驱动形态(虚拟窗口内 pane 中真 CLI 回控归 M7 出口归 11 篇;L4 资产重生的「Runtime API 驱动」类别 eg 11 篇)。

## 改造 / 移植 / 新建设计

### 1. 单实例移交改造(驻留语义全砍、仅移交,机制照抄)

pebrel `nebula_app/src/mux.rs` 的驻留价值——「关窗后 claude 会话不退出、二次启动重新挂载 detached tab」——整体不迁;本篇只取其移交机制与降级语义。改造点:

- **启动分支单窗化**(照抄 pebrel `nebula_app/src/gpui_shell/mod.rs` 启动序骨架,删多窗臂):

```rust
// slterm_app 启动早期(壳骨架归 M3,本篇给契约):
let runtime_hub = runtime_api::RuntimeHub::new();
let runtime_server = runtime_api::RuntimeServer::spawn_callback(
    |callback| { let _ = shell_tx.send(match callback {
        runtime_api::RuntimeCallback::Attach => ShellEvent::Attach,   // 归 09 窗口聚焦/还原
        runtime_api::RuntimeCallback::Control(dispatch) =>
            ShellEvent::RuntimeControl(dispatch),
    }); },
    runtime_hub.clone(),
);
if runtime_server.is_none() && initial_command.is_none() {
    // 与对端抢锁竞态:对方已拿锁但 endpoint 尚未落盘,短暂重试再决定自启还是移交
    let handover_cwd = initial_cwd.clone().or_else(|| std::env::current_dir().ok());
    for _ in 0..40 {
        let handed_over = match handover_cwd.as_deref() {
            Some(dir) => runtime_api::try_open_directory_existing(dir, shell_id.as_deref()),
            None => runtime_api::try_open_default_tab_existing(shell_id.as_deref()),
        };
        if handed_over { return; }   // 移交成功:本进程零窗口退出
        std::thread::sleep(Duration::from_millis(25));
    }
}
let _runtime_server = runtime_server;  // 随壳生命周期持有,Drop 删端口归 09 退出链
```

- **砍掉的部分**:`try_open_window_existing`/`window.create` 链(多窗口);驻留窗口/隐藏宿主/文档 tab PTY 常驻不归本篇;「下次启动恢复布局」归 05 篇 session v4。移交覆盖面 = 「还原/聚焦窗口(ATTACH)+ 新开 tab(tab.new,可带 cwd/shell)」两个动作。
- **保留的降级语义**:陈旧端口文件(被杀进程残留)由 connect 400ms 超时兜底,下一份启动接管;Drop 仅删自己仍是最新的记录;两进程同时启动由 owner 锁 + 40×25ms 重试环裁决,不存在双 serve。
- **关窗即退出的接线**:单窗口下无「隐藏宿主窗口」概念,ATTACH 的「还原窗口」= 把已存在窗口从最小化/后台聚焦归 09 篇;最后窗口关闭 = 进程退出归 09 篇退出链,RuntimeServer 随之 Drop。

### 2. SLTERM_ 环境变量注入与端点发现(改名服从 01 篇单点表)

- 注入组归 03 篇 `agent_env::apply` 锚点(`TERM_PROGRAM=slterm`/`TERM_PROGRAM_VERSION`/`SLTERM_PANE_ID`/`SLTERM_PROCESS_ID`/`SLTERM_CLI`/`SLTERM_BIN_DIR`/PATH 前置,eg 03 篇);本篇新增的唯一点是端点注入挂在 `apply` 的末位(照抄 pebrel `agent_env.rs::apply` 末尾的 `runtime_api::apply_child_endpoint(env)` 调用序):

```rust
// slterm_app::runtime_api::apply_child_endpoint(签名照抄 pebrel server.rs):
// 1. env.retain 按名清旧值(Windows 大小写不敏感)——继承来的端点一律作废,
//    子进程必须指向「正在为自己提供控制面的那个实例」
// 2. CHILD_ENDPOINT 有值才写 "<port> <token>";无值则不注入
//    (嵌套/宿主场景由宿主自己的 CHILD_ENDPOINT 链保证,不留死值)
pub(crate) fn apply_child_endpoint(env: &mut HashMap<String, String>);
```

- **三级发现优先级**(照抄 `read_endpoint`):本进程 `CHILD_ENDPOINT` > 显式 `SLTERM_RUNTIME_ENDPOINT` env > `runtime.port` 端口文件;显式 env 存在但非法(port 0/token 非 32hex)→ 返回 `None`,**绝不回落**端口文件(语义红线:不允许「指了 A 却悄悄连 B」)。
- **兼容回落位**(照抄 pebrel `legacy_request` 的 `.or_else(read_endpoint_from(legacy_port_file))` 形态):slTerminal 无历史协议包袱,该位仅作「未来迁移/降级协议面」扩展位保留,首发不启用、不读第二端口文件。
- **注入时机纪律**:`CHILD_ENDPOINT` 在 serve 线程 spawn 成功后立即登记(照抄 `spawn_at` 尾序);早于首个 PTY spawn 即全覆盖——spawn 失败的清理路径(删端口文件)同步清空,防半初始化态。

### 3. CLI 二进制定位:同 exe 子命令(推荐)vs 独立 CLI exe(取舍)

**方案 A(推荐,pebrel 形态照抄)**:`slterm` 单 exe 双人格——带 argv 子命令 = CLI 客户端,无子命令 = 启 GUI。

```rust
// slterm_app/src/cli.rs(签名级):
#[derive(Parser)]
#[command(name = "slterm", bin_name = "slterm", version = env!("VERSION"))]
pub struct Cli { #[command(subcommand)] pub command: Option<Subcommands> }

pub enum Subcommands {
    Ctl(ControlOptions),          // 完整协议 verb 面归本篇
    Env(EnvOptions),              // 离线发现归本篇
    Window(WindowResourceOptions), Tab(TabResourceOptions),
    Pane(PaneOptions), Agent(AgentOptions),
    // SetupAi归 03 / 设置 eg 06 / 其余归各篇归位归 01 篇 B 节
}

// main 分流(eg M3):
fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(sub) => std::process::exit(run_cli(sub)),  // CLI 人格:请求-响应后退出
        None => gpui_shell::run(),                       // GUI 人格归 M3/09
    }
}
pub fn run_cli(command: Subcommands) -> i32;  // 响应一律 JSON(--pretty 仅排版归 01 篇约定)
// 关键辅助签名归本篇:
const SHORT_TIMEOUT_MS: u64 = 30_000;        // 短命令查询超时(负载标定归 D04-2)
const SHORT_WAIT_TIMEOUT_MS: u64 = 600_000;  // wait 默认上限:编码 Agent 以分钟计
pub(super) struct ShortOutput { pub pretty: bool }   // 全资源命令共享的薄输出面
```

取舍:A 的优点 = 发现零成本(`SLTERM_CLI` 指向的就是它,pebrel 既有纪律)、双人格永不版本错位、打包单文件归 12 篇体积工程;代价 = Windows 子系统选择——D04-1 已裁 **console 子系统**(GUI 启动闪控制台窗接受,标准流正确性优先;Runtime 控制面保留,AI CLI agent 是其主要消费者),`AttachConsole` + stdout 句柄重开的 GUI 子系统臂不取。

**方案 B(否决,记录理由)**:独立 `slterm-cli.exe`(仅客户端代码,~MB 级)。优点 = 免子系统麻烦、冷启动毫秒级;否决理由 = `SLTERM_CLI`/`SLTERM_BIN_DIR` 必须改指第二 exe(发现契约分叉)、双 exe 打包/签名/更新一致性成本归 12 篇、clap 定义双份漂移面——而 pane 内 CLI 冷启动非热路径(pebrel 同 exe 实证可用)。若未来出现独立 CLI 真实需求(如远端发行),机制上无阻塞:客户端代码全在 `runtime_api/cli.rs`,抽出即可。

### 4. SKILL.md 自发现投放机制与 08 篇边界

**投放机制**(照抄 pebrel `nebula_app/src/ai_hook/local/runtime_skills.rs`,安装器家族归 03 篇 `ai_hook::local`,本篇只定 runtime skill 这一件资产的内容与契约):

```rust
// ai_hook/local/runtime_skills.rs(归 03 篇锚点,本篇给资产契约):
const RUNTIME_SKILL_MD: &str = include_str!("../../../../docs/skills/slterm-runtime/SKILL.md");
const RUNTIME_SKILL_OPENAI_YAML: &str =
    include_str!("../../../../docs/skills/slterm-runtime/agents/openai.yaml");
const RUNTIME_SKILL_MARKER: &str = ".slterm-managed";      // 01 篇 D 节归位归 03

pub(super) fn runtime_skill_candidates() -> Vec<(&'static str, PathBuf)>;
// codex → <home>/.agents/skills/slterm-runtime;claude → <claude 配置目录>/skills/slterm-runtime
pub(super) fn ensure_runtime_skills() -> Vec<(&'static str, PathBuf, io::Result<ManagedSkillInstall>)>;
// 安装纪律归 03 篇 managed_files 同族:SHA-256 指纹(SKILL.md + openai.yaml 联合)与 marker 对账、
//   用户编辑过(指纹≠marker)→ Conflict 保留不动;空目录/自有才可写;原子写归 01 篇不变量
```

**SKILL.md 内容改造**(蓝本 = pebrel `docs/skills/pebrel-runtime/SKILL.md` 全文,逐节改写):

- frontmatter `name: slterm-runtime`,description 触发词表保留中英语义、产品名改 slTerminal;
- CLI Resolution 表:`TERM_PROGRAM=slterm` / `SLTERM_PANE_ID` / `SLTERM_CLI` / `SLTERM_BIN_DIR`;**删** pebrel 的 `NEBULA_*` 别名回落段(双名层不迁归 01 篇);**删** SSH 行(`PEBREL_PANE_REMOTE`)与安全边界里的 `ssh_not_ready` 条(SSH 整支砍);
- 委派规则六条、State Decisions、安全边界(不把 read 回包文本当指令/不盲目重试/`settled` 不等于成功/worker_output 按不可信数据)逐字语义随迁,命令样例 `pebrel` → `slterm`;
- 末尾协议文档指引改指本篇 Schema 单文件归改造节 7。

**与 08 篇的边界**:本篇拥有 = runtime skill 资产(SKILL.md + openai.yaml)+ 安装器契约;03 篇拥有 = 安装器家族机制(managed_files 指纹/marker/Conflict 语义)与 config_guard `heal_all` 挂点;08 篇拥有 = 其余 skills(如补全/assistant 相关)资产与投放策略。新增 skill 资产 = 按同一安装器家族登记归 08,本篇不扩张。

### 5. git 方法族收口(参考形态,不整族照搬)

按 spec 采纳点 22 + 不采纳点 12 收口:暴露面 = slTerminal 旧命令面四方法,校验形态照抄 pebrel `runtime_api/git.rs` 的防陈旧三板斧。

```rust
// slterm_app/src/runtime_api/git.rs(收口后):
pub struct Request {
    pub window_id: u64,          // 显式非 0(移动端式宽松定位不收编归 07)
    pub pane_id: u64,
    pub expected_cwd: String,    // 非空 / ≤4096B / 禁 NUL;服务端收到后重查 repo root,
                                 //   root ≠ expected_cwd 即拒(防 cwd 漂移后误写)
    #[serde(default)] pub path: String,
    #[serde(default)] pub revision: String,   // 写操作必带:64 位 hex(服务端 status 摘要)
    #[serde(default)] pub operation: String,  // status | diff | rollback | unstage
    // pebrel 的 stage/commit/fetch/pull/push/history 不迁归 07 篇扩展位
}
// 写前校验:重跑 status 得新 revision,≠ 请求 revision → git_stale「repository state
//   changed; refresh before writing」,绝不基于陈旧视图改写;
// 路径参数永不解释为 shell 命令或 glob(纯 argv 子进程归 07 git 域,--no-optional-locks);
// 读操作(status/diff)同样带 expected_cwd 归一入口。
```

### 6. 协议 Schema 单文件与错误码表成文

- **Schema 单文件**:`docs/runtime-api-v1.schema.json` 蓝本随迁(protocol 常量改 `slterm.runtime`),落位归 12 篇(M11 前 `docs/` 临时稿清空;协议文件正式归置已裁 = 根级 `assets/`,D12-3,开放问题 1 闭环);`runtime.describe` 的 `schema` 字段指向相对路径(照抄)。
- **错误码表**成文归本篇测试点(机器可读错误码全集):envelope 层(`invalid_request`/`protocol_version_mismatch`/`request_too_large`/`method_not_found`/`invalid_params`/`target_not_found`/`ambiguous_target`/`invalid_state`/`action_failed`)、生命周期(`pane_closed`/`pane_exited`/`agent_exited`/`agent_replaced`/`agent_closed`/`agent_name_conflict`/`agent_identity_mismatch`)、执行(`runtime_unavailable`/`runtime_timeout`/`invalid_runtime_response`/`submission_outcome_unknown`/`timeout`)、run(`exit_code_unavailable`/`run_start_timeout`/`run_aborted`)、git(`git_stale`/`git_unavailable`/`invalid_base`/`invalid_branch`/`dirty_source`/`branch_conflict`/`worktree_path_conflict`)、委派(`origin_not_agent`/`origin_identity_unavailable`/`delegation_in_progress`/`delegation_capacity`/`agent_ready_timeout`)、编排(`invalid_reference`)。SSH/WSL/远端系错误码不迁归不变更项 2。

## 测试点清单

> 引测试一律例名;测试基础设施形态(虚拟窗口/夹具/conformance 驱动归 11 篇)。pebrel 侧承重用例全量随迁改名(用例名不变);bugfix 防复发纪律归本仓测试金字塔归位归 11 篇。

| 测试 | 层级 | 来源 |
| --- | --- | --- |
| 信封族:请求/响应/事件 round-trip;`deny_unknown_fields` 未知字段拒;显式 null result 与缺 result 字段区分(present_json);响应身份逐字段校验(id/protocol/version/ok-result 互斥) | L1 runtime_api | 迁移 pebrel `runtime_api/tests.rs` 用例 |
| 版本协商:protocol/version 不匹配 → `protocol_version_mismatch` + `supported_versions`;合法版本直通 | L1 | 迁移 |
| token 鉴权:错误/缺失 token 静默丢弃(先于详细参数校验——未认证进程不得拿解析错误当协议预言机);legacy 行 token 不符静默丢弃 | L1 | 迁移 |
| 请求体上限:首行超 MAX_REQUEST_BYTES → `request_too_large`;`request_once_bounded` 双侧上限(128KiB 请求/256KiB 响应) | L1 | 迁移 |
| `validate_prompt`/`validate_paste_text`/`validate_command_line`/`validate_chat_message`/`validate_exec_argv` 正反向全表(空/超 32KiB/控制字符/CRLF 保留/NUL) | L1 command | 迁移 |
| `wait_matches`:`after_seq` 缺席立即匹配旧语义;基线存在时 `seq > after_seq` 才命中;Settled = 非 Running | L1 command | 迁移 |
| `stamp_state_change_seq`:状态未变保留旧计数(publish 判等不刷 revision);跃迁 +1;首见 = 1;revision 单调与「语义不变不重发」 | L1 hub | 迁移 |
| 提交后基线链:CLI `--wait` 基线取自提交后快照;缺非零基线报错不降级;`--no-submit`+`--wait` 解析期拒绝 | L1 cli | 迁移 |
| 超时语义:`pane.wait` 超时 details 带 `after_seq`+`observed_state_change_seq` 区分「没动」与「动了没到」;等待中 pane 生命周期错误优先于超时 | L1 transport | 迁移 |
| generation 绑定:active_agent 选择序(id→name+generation→name 最新 active);`agent_identity_mismatch`(details 带 expected/actual);`agent_exited`/`agent_replaced` 错误码直传;close_agent 重发快照唤醒等待 | L1 hub | 迁移 |
| 事务回滚:`prepare_dispatch_command` 校验序(dirty/base/branch/path→建事务);派发失败回滚;UI 超时 `cleanup_deferred=true` 保留 checkout;成功 attach provenance | L1 agent_api | 迁移 |
| 委派:每目标单在途(`delegation_in_progress`);容量上限;不可自委派;origin 无身份(`origin_not_agent`/`origin_identity_unavailable`);回传只认同 pane 同 generation;worker_output 截断 4000 字符 | L1 agent_api/hub | 迁移 |
| orchestrate 预检:未来引用/重复 id/未知字段在任何 UI 动作前拒;步骤数与字节预算;`on_error=continue` 保留已完成动作;回执从失败步续作;agent 并行冷启后投首任务(ready 超时 `agent_ready_timeout`) | L1 orchestrate | 迁移 |
| `pane.exec`:argv 无 shell 展开;双流并行排水只留上限;超时整树回收保留已捕获输出 + `timed_out`;不改 shell 状态 | L1 exec(02 篇 Job Object 集成归 02 用例) | 迁移 |
| `pane.run`:只认 OSC 133 CommandDone;无集成 `exit_code_unavailable`;未观察 CommandStart `run_start_timeout`;run 消失 `run_aborted`;绝不伪造 0 | L1(02 篇事件链集成归 02) | 迁移 |
| `pane.read`:真实 Grid 尾部(bounds_to_string);锚定 buffer 底部;不足一屏多扫回退;字节截断 + truncated;不动用户滚动/选区 | L1(02 Term 夹具归 02/11) | 迁移 |
| 端点解析:`parse_endpoint` 正反向(port 0/65536/非 hex/长度);三级发现优先级;显式 env 非法不回落端口文件;`apply_child_endpoint` 清旧值 + 无 CHILD 不注入 | L1 server | 迁移 |
| 移交链:ATTACH→tab.new 同队列序;`handover_params` 只带值键;stale 端口文件 400ms connect 超时接管;Drop 仅删自己的记录;错 token PING 静默;40×25ms 重试环裁决双启动 | L1 server(双进程用例归 11 豁免口径) | 迁移 mux/runtime server 用例 |
| env 契约:身份变量组全等(归 03 篇 agent_env 用例)+ ENDPOINT 注入幂等(嵌套不涨份)+ `slterm env` 离线段在控制面不可达时仍成功且如实报告 | L1 agent_env/shortcuts | 迁移 + 新建 |
| describe 形态:capabilities/features/env/commands 四段键集合钉死(裁剪后集合归本篇成文);feature 串含 `pane.wait.after_seq`;env 段变量名与 03 篇常量同源(编译钉归 03) | L1 transport | 迁移 + 改造 |
| git 收口:`expected_cwd` 与重查 root 不一致拒;写前 revision 校验 `git_stale`;路径参数不解释 shell/glob;四方法操作名白名单 | L1 git(子进程夹具归 07) | 改造(旧 Tauri git 命令用例语义归 07 篇) |
| 协议 Schema 与实现同构:Schema 单文件对 `ApiRequest`/params 族抽样校验(结构兼容,eg 11 篇 conformance 夹具) | L1 | 改造 |
| Runtime API conformance:虚拟窗口内 pane 中真 CLI(或协议桩归 11 裁定)实际回控——env 定向 → pane send/read/wait → agent delegate 回传全链 | L4 形态归 11(gpui-test-support 虚拟窗口归 11 篇) | 新建(eg SPEC 总表「conformance(Runtime API 驱动)」) |
| 移交双进程:真起两份进程(embedded driver eg 11 篇口径)——二启零窗口退出、首实例窗口还原 + 新 tab、首实例被杀后残留文件被三启接管归 11 篇豁免登记 | L1 集成归 11 | 新建 |
| 禁名门禁:runtime_api 域零 `nebula`/`pebrel` 回流(协议常量 `slterm.runtime`、错误消息归 01 篇 EXEMPT 形态);线程名归 01 篇 P 节 | 归 01 篇门禁归 11 登记 | 新建归 01 |

## 阶段归属与出口标准

本片主体归 **M7 Runtime API + 单实例**(00-roadmap),细分五步,出口全为可机验项:

**M7.1 传输与信封落地**:`runtime_api.rs` 信封四件 + 协议常量、`transport.rs` 主链(serve/handle_connection/handle_legacy/读写行)、`atomic_file` 消费件(owner 锁/原子写)、Schema 单文件随迁归位归改造节 6。出口 = 信封/版本协商/token/上限用例全绿;`cargo check` 过;禁名门禁过(协议常量零旧名)。

**M7.2 RuntimeHub + 投影入账**:`RuntimeHub`(publish/订阅/盖戳/生命周期/managed agents/run 等待表)+ 05 篇投影函数接线(Workspace → RuntimeSnapshot,归 05 篇共建)+ describe/subscribe/wait 专链。出口 = hub 判等/盖戳/订阅有界用例全绿;describe 四段键集钉测试绿;`cargo test` 全绿。

**M7.3 方法族与派发原语**:`RuntimeCommand` 全族 + `dispatch_runtime_command`(worktree 事务/run 分段限时/超时未知态)+ pane/agent/orchestrate/git 方法族 + 参数校验面。出口 = 校验正反向全表、generation 族、委派族、orchestrate 预检族、git 收口用例全绿;派发超时与 `cleanup_deferred` 语义用例绿。

**M7.4 CLI + env 契约 + SKILL 投放**:方案 A 同 exe 分流(改造节 3,console 子系统 D04-1 已裁)+ 资源动词薄别名 + `slterm env` + `apply_child_endpoint`/三级发现 + runtime skill 安装器接线(eg 03 config_guard 挂点)。出口 = CLI --wait 基线链/解析拒绝用例绿;env 离线可用用例绿;端点三级发现用例绿;SKILL 指纹/marker/Conflict 用例绿(eg 03 族)。

**M7.5 单实例移交**:`RuntimeServer` 全序(owner 锁→PING→serve→CHILD_ENDPOINT)+ 移交链(ATTACH+tab.new+重试环)+ Drop 清理 + 壳启动分支接线(eg M3 壳归 05/09)。出口 = 移交链/stale 接管/Drop 只删己录用例绿;双进程移交集成用例归 11 篇豁免口径绿;对照 00-roadmap M7 出口:Runtime API conformance 测试过(eg 11 篇基础设施)+ 单实例移交测试过。

M7 全程依赖 M3(壳骨架)/M4(AgentActivity 投影源)/M5(Workspace 投影归 05)已点亮;未点亮部分按过渡期不可用清单缺席,不设功能兜底。

## 待沉淀决策

> [待沉淀] **D04-1 · CLI 双人格的 Windows 子系统取向**(归改造节 3 方案 A)。
> 难逆:子系统属性(`#![windows_subsystem]` / 链接器 `/SUBSYSTEM`)在发布物定型后返工成本高——已装用户的升级器、快捷方式、开机自启链全部按既有形态铺开,首版发出去再改 = 对既有安装面 breaking。
> 意外:同一 exe 走 GUI 子系统时,父进程是控制台(如 pwsh 管道)的句柄继承语义与 console 子系统相反,存在 `AttachConsole` 后标准流拿不到/管道断流的边缘缺陷;反之 console 子系统跑 GUI 启动链必然闪一个控制台窗,两个方向都有非直觉代价。
> 真实权衡:GUI 子系统 + `AttachConsole` + 标准句柄重开 = 无启动闪窗,但 PowerShell 管道重定向有已知边缘缺陷;console 子系统 = 标准流行为最正,代价是 GUI 启动闪控制台(eg 12 篇打包裁定)。

> [待沉淀] **D04-2 · 契约数值首版照搬 pebrel 的校准取向**(归改造节 6 + 关键类型节契约常量块)。
> 难逆:契约数值写进协议文档/SKILL.md 样例/conformance 夹具后即成为外部可依赖面——AI CLI 会按样例超时值写脚本,事后收紧会被既有自动化脚本撞红,比改内部实现难逆一个数量级。
> 意外:pebrel 的数值(等待上限 600s/短超时 30s/RUN_START_GRACE 3s/订阅有界 16/请求 128KiB/响应 256KiB/字节截断 4000/worker_output 截断 4000/40×25ms 重试环)是按其 PTY 冷启与网络遥测的实测拍的,并非普适常数;slTerminal 同 GPUI 链路下大概率同域,但未见实测证据。
> 真实权衡:首版零成本照搬求稳(不重测不争论),但一经发布即成事实契约;校准需拿真实冷启/委派/run 时延实测再拍,属 D04-2 重做项,代价是首版若偏差大,首版用户脚本即被锁进旧值。

> [待沉淀] **D04-3 · `windowing_behavior` 设置键与单窗收编的去向**(归缝合点 4 + 不变更项 10)。
> 难逆:设置键一旦进持久化文件即背负迁移义务——键改名/删键需写升级器,首版若带着「未来可能多窗」的形状发出去,删它就要动用户已有配置。
> 意外:pebrel 的 `windowing_behavior` 是新窗口开 tab/开目录/独立窗的策略开关,其存在前提是多窗;单窗收编后该键失去全部语义宿主,但 describe 的 capabilities/features 面又需要一个位置声明「单窗即终态」,二者不是同一概念却共用一处文面。
> 真实权衡:直接删键(最干净)vs 保留键但改语义为「单窗常驻确认策略」(给未来多窗留扩展缝,但违反「按理想终态设计、不做兼容过渡」取向)vs 首版不设键、单窗行为硬编码归壳(eg 09 篇窗口管理),归 12 篇打包时随 settings 面统一裁定。

## 开放问题

1. **Schema 单文件的正式归置**（已闭环：D12-3 裁永久资产归根级 `assets/`,skills/schema/协议文档同址）:首版 `docs/runtime-api-v1.schema.json` 落 `docs/` 临时稿归 12 篇（M11 前清空）,M11 随 12 篇改造节 2 改指 `assets/`;本篇只保证「单文件、可机器校验、describe.schema 相对路径可达」三不变量归改造节 6。
2. **conformance 驱动桩 vs 真 CLI**:L4 conformance 用例归 11 篇基础设施,本篇消费其驱动形态;「虚拟窗口内真起 CLI 子进程回控」与「协议桩替身」二选一归 11 篇按可重复性裁定,本篇不预设归测试点清单末行。
3. **legacy ATTACH 回落的首发开关位**:机制照抄归改造节(33 号采纳点),但首发禁用开关归 12 篇(打包开关 or 编译 feature eg 12 篇);若首版后发现 VS Code 系等旧客户端有接入诉求,启用决策归 12 篇随扩展位重开归改造节 2 三级发现。
4. **移交窗口还原的 09 篇落地细节**（已闭环）:09 篇改造节 1 已答——单窗下「还原窗口」无独立语义，移交链只剩「聚焦现有唯一窗 + tab.new」;已回登本篇缝合点 5。
