# 03 AI CLI 集成详细设计

> pebrel-design 分片 03/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/03-ai-cli-integration.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **`AgentKind`、hook 事件类型(`AiHookEvent` 族)、`AgentActivity` 状态** 的锚点归属篇(01 篇类型锚点表已登记),他篇(02/04/05/06/08/09 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

AI CLI 集成在单进程 GPUI 世界重建为「**一进程 + 一小桥**」形态:

- **`slterm_hook` crate**(bin `slterm-hook`):纯 std 零依赖小进程,照抄 pebrel `nebula_hook`(spec 采纳点 1)。它是事件链第一推动力,行为合同(exit 0、冷启动 <15ms、无声)一字不动。
- **`slterm_app::ai_hook` 模块树**:命名管道服务器 + 信封解析 + 有界载荷 + 事件门重排 + 每 pane 仲裁 + 安装权威 + 9 家一等安装器,整体照抄 pebrel `nebula_app/src/ai_hook/` 改名落地(裁剪:remote 整支、unix 分支、legacy-shell feature 通道、双名兼容层)。
- **`slterm_app::ai_agents` / `ai_sessions` / `assistant_answer` / `agent_env` 模块**:27 家识别、声明式屏幕规则、会话发现有界扫描、回答投影、per-pane 环境契约,照抄改名。
- 事件通路:**命名管道为 hook 事件唯一权威通道**;slTerminal 旧信号文件通道整面退役(spec 采纳点 41)。statusline 用量桥接与 hook 事件**分通道**保留(信号文件介质是否保留见待沉淀 D03-3)。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_hook/src/main.rs` | pebrel 照抄改名 | hook 小进程:`main`/`run`/`read_payload`/`envelope`/`send_local` |
| `slterm_app/src/ai_hook.rs` | pebrel 照抄改名 | 模块面 + `PIPE_ENV`/`PANE_ENV`/`HOOK_EXE_ENV` 常量(→ `SLTERM_*`)+ `contains_helper`/`is_helper_*` 归属判据 |
| `slterm_app/src/ai_hook/windows.rs` | pebrel 照抄改名 | `spawn_gpui_server`(mpsc 形态)/`spawn_pipe_server`/`serve` 命名管道服务器 + 内核进程身份核验 |
| `slterm_app/src/ai_hook/protocol.rs` | pebrel 照抄改名 | `parse_envelope` 信封解析 + provider 归一化(线协议头 `slterm-hook/1`) |
| `slterm_app/src/ai_hook/payload.rs` | pebrel 照抄改名 | 有界提取、脱敏词表、`attention_is_actionable`、后台任务终态词表 |
| `slterm_app/src/ai_hook/ordering.rs` | pebrel 照抄改名 | `AiHookEventGate` 有界重排七判、`GateVerdict` |
| `slterm_app/src/ai_hook/lifecycle.rs` | pebrel 照抄改名 | `AgentActivity` 每 pane 仲裁器、`HookCoverage` 四级 |
| `slterm_app/src/ai_hook/event.rs` | pebrel 照抄改名 | `AiHookKind`/`AiTurnOutcome`/`AiHookCapabilities`/`AiPermissionMode`/`AiBackgroundTasks`/`AttentionContext`/`AiHookEvent`(本篇锚点) |
| `slterm_app/src/ai_hook/installation.rs` | pebrel 照抄改名 | 共享安装权威:`codex_mode`/`codex_groups`/`merge_groups`/`enable_codex_feature` |
| `slterm_app/src/ai_hook/installation/notify.rs` | pebrel 照抄改名 | notify 槽包装 + #38 指数膨胀防线 |
| `slterm_app/src/ai_hook/local.rs` + `local/*` | pebrel 照抄改名 | 9 家安装器(claude/codex_notify/codex_hooks/cursor/kimi/extended/native_events/managed_files/config_guard/settings/runtime_skills 挂点) |
| `slterm_app/src/ai_hook/bridges.rs` + `res/hooks/` | pebrel 照抄改名 | opencode.js / pi.ts 插件落盘桥 |
| `slterm_app/src/ai_hook/integrations.rs` | pebrel 照抄改名 | `AGENTS` 9 家一等数组 + `HookInspection` + `executable_directories` |
| `slterm_app/src/ai_agents.rs` + `ai_agents/screen_context.rs` | pebrel 照抄改名 | `AgentKind` 27 家(本篇锚点)+ TOML 规则引擎 + 结构判据 |
| `slterm_agent_detection/*.toml` | pebrel 照抄改名 | 20 厂商规则 + `_shared.toml` 兜底(资源文件随仓) |
| `slterm_app/src/ai_sessions.rs` | pebrel 照抄改名 | 有界扫描 + hook 注册表合并 + resume/fork 命令 |
| `slterm_app/src/assistant_answer.rs` + `assistant_answer/conversation.rs` | pebrel 照抄改名 | 回答三态 + `AnswerInbox` + 有界 transcript 投影 |
| `slterm_app/src/agent_env.rs` | pebrel 照抄改名 | per-pane 环境契约(`apply`/`executable`/幂等三件套) |

裁剪不迁:`ai_hook/remote/` 整支、`ai_hook/unix.rs` + `send_local` unix 分支、`main.rs` 远端 OSC 777 分支与 `REMOTE_HOOK_TOKEN`/`base64_encode`、`windows.rs::spawn_server`(winit EventLoopProxy 通道)、`ai_hook.rs` 的 `LEGACY_*_ENV` 常量族、managed_files legacy hashes、`agent_env.rs` 的 `LEGACY_CLI_ENV`/`LEGACY_BIN_DIR_ENV` 与 SSH 臂、`PEBREL_PANE_REMOTE`/`NEBULA_PANE_REMOTE` 环境变量(01 篇 C 节「SSH 语义消亡后字段重估」的裁定:远程 pane 语义无对象,`SLTERM_PANE_REMOTE` 不立)(spec 分片 03 不采纳点 1–4)。

### 类型锚点归属(本篇登记,01 篇表已引)

`AgentKind`、`AgentStatus`、`AgentStatusSource`、`Detection`、`AiHookEvent`、`AiHookKind`、`AiTurnOutcome`、`AiHookCapabilities`、`AiPermissionMode`、`AiBackgroundTasks`、`AttentionContext`、`GateVerdict`、`AgentActivity`、`HookCoverage`、`Owner`、`AiSession`、`AssistantAnswer`、`AnswerInbox`、`HookInspection`、`AgentIntegration` 唯一定义在本篇;他篇只许 `use` / facade 传参。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **27 家 AgentKind 枚举不裁剪**(定位约束 7),`ALL` 数组 27 项一项不少;9 家一等集成数组不裁剪。
3. **hook 小进程行为合同不可变**:任何路径 exit 0(含 panic)、1MB 载荷上限、2s 一次性转发超时、stdin 无条件抽干、NotHosted 恒无声。这是第三方 CLI(claude/kimi)的调用方合同,改动即破坏已安装的 hook 接线。
4. **环境变量单名 `SLTERM_*`**:pebrel 双名并写兼容层整体不迁(spec 不采纳点 3);`TERM_PROGRAM=slterm`、管道名 `\\.\pipe\slterm-notify-<pid>`、managed marker 后缀 `.slterm-managed`、备份后缀 `.slterm-bak`、信封头 `slterm-hook/1`(01 篇单点表 P-1/P-2/C 节)。
5. **hook 事件与用量信号分通道**:statusline 的 context 用量不混入 hook 管道载荷(spec 采纳点 42,介质裁定见 D03-3)。
6. **pane 生命周期内 pane id 不复用**(05 篇不变式 2 的承接):事件门全进程共一扇、按稳定 pane id 路由,这一前提由 05 篇的 PaneId 单调分配器保障,本篇不自行做 id 回收。
7. **事件静默丢弃必带原因**:`GateVerdict` 非 Accepted 的每条丢弃路径必须落应用日志;模块约定写进 `ai_hook` 子路径 CLAUDE.md(spec 优化方向「可解释性纪律内化」)。
8. **platform 分支收敛**:命名管道/进程身份核验等 `#[cfg(windows)]` 只允许出现在 `ai_hook/windows.rs`;协议解析、事件门、仲裁器平台无关。
9. **凭据纪律**:hook 载荷的脱敏词表(`sensitive_context_key`)照抄;`AttentionContext::raw_context` 绝不进 Debug 日志(payload.rs 注释合同随迁);`SLTERM_HOOK_LOG` 侧信道日志绝不记载荷正文。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致,改名点已标;pebrel 侧 pane id 为裸 `u64` 处,本篇以 05 篇 `PaneId` newtype 承接(改动理由同 05 篇:防「比例/id 混算」笔误,不改语义)。

### AgentKind 27 家(`slterm_app::ai_agents`,本篇锚点,照抄)

```rust
// 27 家一项不裁(定位约束 7)。slug 是跨篇身份事实单源:事件 source、
// 设置页、恢复命令、目录表全部消费 slug,禁第二份枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    Claude, Codex, Gemini, Aider, Amp, OpenCode, Copilot, Cursor, Goose,
    Droid, Pi, Auggie, Hermes, Vibe, Antigravity, Grok, Qwen, OhMyPi,
    Cline, Devin, Kimi, Kiro, Kilo, Qoder, Maki, Trae, CodeBuddy,
}

impl AgentKind {
    pub const ALL: [Self; 27] = [ /* 上列顺序,照抄 pebrel */ ];
    pub fn slug(self) -> &'static str;        // claude/codex/.../omp(OhMyPi)/qodercli(Trae 为 "trae-cli")
    pub fn display_name(self) -> &'static str; // 展示名里的 pebrel 产品名同步替换为 slTerminal
    pub(crate) fn aliases(self) -> &'static [&'static str]; // 进程识别与设置页发现共用同组官方/兼容名
    pub fn parse(raw: &str) -> Option<Self>;              // slug/alias 归一
    pub fn parse_command(command: &str) -> Option<Self>;  // argv 首 token → AgentKind
    pub fn resume_command(&self, session_id: &str) -> Option<String>; // claude --resume <id> 等
    pub fn fork_command(&self, session_id: &str) -> Option<String>;   // --fork-session 等
}
```

`aliases` 的 cursor 历史标签(`agent`/`cursor`/`cursor-agent`)保留以归一旧进程快照;resume/fork 语法分散在各 AgentKind 实现(参考 spec 采纳点 35,不做统一抽象——命令生成是各 CLI 合法领地)。

### AgentStatus 观察三值(`slterm_app::ai_agents`,照抄)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentStatus { Unknown, Idle, Working, Blocked, Done }
impl AgentStatus { pub fn is_decided(self) -> bool; }  // 无判定时才轮到响铃兜底

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatusSource { Unknown, Screen, Process, Hook }  // 谁报的——hook 高于屏幕

#[derive(Debug, Clone)]
pub struct Detection {
    pub agent: AgentKind,
    pub status: AgentStatus,
    pub rule_id: String,      // TOML 规则 id,仲裁与可解释性共用
}
pub fn detect(program: &str, screen: &str) -> Option<Detection>;  // TOML 规则引擎入口
```

### hook 事件类型族(`slterm_app::ai_hook::event`,本篇锚点,照抄)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiHookKind {
    SessionStart, PromptSubmit, ToolComplete, TurnDone, NeedsAttention, SessionEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTurnOutcome { Unspecified, Succeeded, Failed, Cancelled, Incomplete, Unknown }
// 只信 provider 结果元数据,绝不扫 assistant 散文判成败。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiHookCapabilities {       // 六字段按 source 显式不对称(spec 采纳点 10)
    pub lifecycle: bool,              //   capabilities_for(source) → 逐 source 表
    pub attention_events: bool,
    pub attention_context: bool,
    pub background_tasks: bool,
    pub bridge_sequence: bool,        // bridge 序号是自家 plugin 注入,非 provider 原生
    pub serialized_delivery: bool,    // Bun 串行交付——Done 后无序 ToolComplete 翻案判据
}
pub fn capabilities_for(source: &str) -> AiHookCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiPermissionMode { Default, AcceptEdits, BypassPermissions, Plan }
impl AiPermissionMode { pub fn can_ask_for_permission(self) -> bool; }  // Bypass 恒不问

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AiBackgroundTasks { pub active: u32, pub total: u32 }  // Stop 携带;active>0 → 流保持 Working

#[derive(Clone)]
pub struct AttentionContext {        // 全字段有界:消息 300 字符、id 512、原文 16KB
    pub source: String,
    pub pane_id: Option<u64>,        // → PaneId(05 篇 newtype 承接)
    pub session_id: Option<String>,
    pub event_kind: AiHookKind,
    pub event_id: Option<String>,
    pub bridge_sequence: Option<u64>,
    pub occurred_at_ms: Option<u64>,     // provider 声明,不可靠时 None
    pub received_at_ms: u64,             // 宿主解析完成时间
    pub cwd: Option<String>, pub project: Option<String>, pub git_branch: Option<String>,
    pub permission_or_tool: Option<String>,
    pub permission_mode: Option<AiPermissionMode>,
    pub message: Option<String>, pub selection: Option<String>,
    pub raw_context: Option<String>,     // 脱敏+有界副本;Debug impl 只打字节数(手动实现)
}
impl AttentionContext { pub fn summary_for_pane(&self, pane_id: u64) -> String; }  // toast 摘要

#[derive(Debug, Clone)]
pub struct AiHookEvent {             // 一条管道消息的类型化结果(protocol.rs 产出)
    pub(super) legacy_attention: bool,
    pub(crate) codex_hooks: Option<CodexHookMode>,     // native codex 版本化合同
    pub(crate) turn_id: Option<String>,
    pub(crate) session_compacted: bool,
    pub answer: Option<AssistantAnswer>,               // eg assistant_answer
    pub answer_cwd: Option<PathBuf>,
    pub pane: Option<u64>,                             // SLTERM_PANE_ID;None → 焦点 pane 兜底
    pub source: String,                                // = AgentKind::slug
    pub kind: AiHookKind,
    pub turn_outcome: AiTurnOutcome,
    pub session_id: Option<String>, pub event_id: Option<String>,
    pub bridge_instance: Option<String>,               // pi 以它代 session_id 分流
    pub bridge_sequence: Option<u64>,
    pub occurred_at_ms: Option<u64>, pub received_at_ms: u64,
    pub received_sequence: u64,                        // 宿主侧单调序号,LRU 驱逐序
    pub agent_pid: Option<u32>,                        // 内核身份核验产出,沿祖先链查
    pub client_pid: Option<u32>,                       // 管道客户端内核 pid(载荷自报不可信)
    pub permission_mode: Option<AiPermissionMode>,
    pub background_tasks: Option<AiBackgroundTasks>,
    pub attention: Option<AttentionContext>,
}
```

`remote_process` 字段不迁(SSH 整支砍);native codex 有 `agent_id` 的子代理事件在 protocol.rs 直接丢弃(与主会话共享 session_id,不能替主回合收尾)。

### 事件门(`slterm_app::ai_hook::ordering`,照抄锚点)

```rust
pub const MAX_TRACKED_STREAMS: usize = 512;          // 流上限,LRU 驱逐
pub const MAX_EVENT_IDS_PER_STREAM: usize = 64;
pub const DUPLICATE_WINDOW_MS: u64 = 1_500;          // 无身份元数据终态事件的指纹窗口

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {                 // 七判全带原因——「通知没出现」唯一取证面
    Accepted,
    DuplicateEventId,                  // 同 event_id 已处理
    StaleSequence,                     // bridge 序号不比上次大
    StaleTime,                         // 无序号但 provider 时间戳更早
    DuplicateFingerprint,              // 无身份终态事件 1.5s 窗口重复
    AfterSessionEnd,                   // Ended 流只有 SessionStart 能复活
    UnorderedAfterDone,                // Done 后无序 ToolComplete,无翻案证据
}

#[derive(Debug, Default)]
pub(super) struct AiHookEventGate { streams: HashMap<AiHookStreamKey, AiHookStreamState> }
impl AiHookEventGate {
    pub(super) fn verdict(&mut self, event: &AiHookEvent, pane_id: u64) -> GateVerdict;
    pub(super) fn accept_for_pane(&mut self, event: &AiHookEvent, pane_id: u64) -> bool;
    pub(super) fn reorder_batch(&mut self, events: &mut Vec<(PaneId, AiHookEvent)>);  // 同 pump 批次组内重排
}
```

流键 = source + session_id(pi 以 bridge_instance 代)+ pane + agent_pid + remote_process(字段砍后键随之收窄,见开放问题 4)。全进程共一扇(模块级 `LazyLock<Mutex<AiHookEventGate>>`),序元数据优先级 bridge_sequence > occurred_at_ms > 指纹 + 1.5s 窗口;`reorder_batch` 只在同 pump 批次且全带序号时组内重排,跨批次旧序号由门拒绝。

### 每 pane 仲裁器(`slterm_app::ai_hook::lifecycle`,本篇锚点,照抄)

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HookCoverage { #[default] None, Completion, Turns, Lifecycle }  // 当前 hook 组合的实际能力声明

#[derive(Debug, Clone)]
struct Owner {                        // 嵌套子代理区分:source/session/pid/bridge_instance
    source: String, session: Option<String>, pid: Option<u32>,
    bridge_instance: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentActivity {     // hook 事件是事实,屏幕匹配是有限权威观察
    status: AgentStatus, source: AgentStatusSource, rule: Option<String>,
    coverage: HookCoverage, owner: Option<Owner>,
    idle_samples: u8, turn_observed: bool, pending_submit: bool,
    screen_armed: bool,                // Completion-only hook 的结果锁存
    command_ended: bool,               // 显式 shell 边界;同时拒绝旧 CLI 迟到事件
    native_codex: bool, turn_id: Option<String>,
}

impl AgentActivity {
    pub fn status(&self) -> AgentStatus;
    pub fn source(&self) -> AgentStatusSource;
    pub fn rule(&self) -> Option<&str>;
    pub fn hook_seen(&self) -> bool;
    pub fn primary_pid(&self) -> Option<u32>;
    pub fn allows_screen(&self) -> bool;   // Lifecycle 覆盖时剥夺屏幕发言权
    pub fn reset(&mut self);
    pub fn command_finished(&mut self);    // shell 边界 = reset + command_ended 锁存
    pub fn begin_command(&mut self, agent: bool);
    pub fn identify(&mut self, source: AgentStatusSource);
    pub fn submitted(&mut self);
    pub fn submitted_text(&mut self, program: &str, text: &str);  // codex /plan 特例
    pub fn input_sent(&mut self);
    pub fn accepts_hook(&self, event: &AiHookEvent) -> bool;      // 门记录前的所有权预检
    pub fn apply_hook(&mut self, event: &AiHookEvent);            // 事件进状态机
    pub fn observe_screen(&mut self, program: &str, screen: &str) -> Option<Detection>;
    pub fn note_bell(&mut self);                                  // 无判定状态的响铃兜底
}
```

每 pane 一个,按 `PaneId` 键挂在 pane 宿主结构(归 05 篇 Workspace/TerminalPane,本篇只定仲裁器语义);TurnDone 时有在飞后台任务(`background_tasks.active > 0`)则流保持 Working 不置 Done。

### hook 小进程(`slterm_hook/src/main.rs`,照抄锚点)

```rust
const MAX_PAYLOAD_BYTES: usize = 1 << 20;                    // 1MB 载荷上限
const FORWARD_TIMEOUT: Duration = Duration::from_secs(2);    // 一次性等待,不链式

// 行为合同(一个不能动):
// - 任何路径含 panic 都 exit 0:catch_unwind + 所有出口显式 process::exit(0)
// - 超限抽干 stdin 但不转发截断 JSON(防 CLI 侧 hook write error)
// - 非白名单首参数 = 误调用,恒无声 no-op(known_source / payload_on_stdin 路由)
// - Outcome 各终态写 SLTERM_HOOK_LOG 侧信道日志,绝不记载荷
enum Outcome { PayloadTooLarge, Sent, NotHosted, PipeUnavailable, ForeignRunner /* RemoteOsc 不迁 */ }

const FOREIGN_HOOK_RUNNERS: &[&str] = &["GROK_HOOK_NAME", "GROK_HOOK_EVENT"];  // 串台门:空值不算命中
fn foreign_hook_runner() -> Option<&'static str>;
fn run() -> Outcome;
fn read_payload(source: Source) -> Vec<u8>;          // stdin 模式:有界抽干
fn envelope(source: &str, pane: Option<&str>, payload: &[u8]) -> Vec<u8>;  // slterm-hook/1 头 + 原样 JSON 体
fn send_local(bytes: &[u8]) -> Result<(), Outcome>; // 命名管道直写,20×5ms 竞态重试
fn chain_notifier(previous: &[String], payload: &str);  // codex --chain:不驱逐原 notify,转发后带同载荷 spawn
fn helper_env(name: &str) -> Option<OsString>;       // 单名 SLTERM_*,别名层不迁
```

usage 形态照旧:`slterm-hook claude|kimi`(stdin)、`slterm-hook codex <json>`、`codex --hooks=full`(native,stdin)、`codex --chain <exe> <fixed…> <json>`、`opencode <json>`、`pi <json>`、`cursor --event prompt`(无条件回写 `{"continue":true}`——提交前 hook 响应合同,传输失败也不能阻止用户提交)。

### 命名管道服务器(`slterm_app::ai_hook::windows`,照抄锚点)

```rust
pub const PIPE_ENV: &str = "SLTERM_NOTIFY_PIPE";     // 作用域闸门:进程级 set 早于首个 PTY spawn
pub const PANE_ENV: &str = "SLTERM_PANE_ID";
pub const HOOK_EXE_ENV: &str = "SLTERM_HOOK_EXE";    // opencode 插件 shell out 的 helper 路径

pub fn spawn_gpui_server() -> mpsc::Receiver<AiHookEvent>;  // GPUI 唯一形态(legacy-shell 通道不迁)
fn spawn_pipe_server(sink: impl Fn(AiHookEvent) -> bool + Send + 'static);
fn serve(name: &str, sink: impl Fn(AiHookEvent) -> bool);
// serve 内合同(照抄):
// - CreateNamedPipeW + PIPE_ACCESS_INBOUND;每次连接一个全新管道实例
// - GetNamedPipeClientProcessId 必须在断开连接前读——内核对「谁在写」的回答,
//   载荷自报 pid 可伪造;client_pid → process_tree::nearest_agent_ancestor 得 agent_pid
// - 线程名 slterm-ai-pipe(01 篇 P-7)
// - 管道名 \\.\pipe\slterm-notify-<pid>(01 篇 P-2)
// - spawn_pipe_server 同时 set PIPE_ENV / HOOK_EXE_ENV 进程级环境(早于首个 PTY spawn 即全覆盖)
```

### 信封解析(`slterm_app::ai_hook::protocol`,照抄锚点)

```rust
pub(super) fn parse_envelope(bytes: &[u8]) -> Option<AiHookEvent>;
// 线协议:首行 `slterm-hook/1 source=<s> pane=<n>`,体 = 原样 JSON(helper 不重编码)
// 第二道串台门在此独立拦(双防线之宿主侧,与 helper 侧 FOREIGN_HOOK_RUNNERS 互补):
// - claude 分支拒收 camelCase hookEventName
// - session_id 键名按 source 收紧(claude 绝不读 camelCase——否则把不存在的
//   会话交给 claude --resume)
// codex 双形态并存:legacy notify(type=agent-turn-complete)与 native hooks
// (hook_event_name + codex_hooks=turns|full 头);native 会话身份以 transcript 文件名
// 换 rollout id(fork/换载后 thread 名不可信)
const ID_MAX_CHARS: usize = 512;
const TURN_RESULT_MAX_CHARS: usize = 4_000;
// parse_remote_envelope / REMOTE_HOOK_TOKEN / OSC 777 分支整删(不采纳点 1)
```

### 有界载荷(`slterm_app::ai_hook::payload`,照抄锚点)

```rust
// 三层容器回退取值(payload / context / payload 嵌套 / properties)+ 多拼写字段名兼容
fn sensitive_context_key(key: &str) -> bool;   // token/secret/password/authorization/cookie/credential/apikey/environment…
pub(super) fn bounded_context(raw: &Value) -> Option<String>;  // 词表命中即 [redacted];JSON 深度 6/数组 24/对象 48 有界清洗
pub(super) fn attention_is_actionable(context: &AttentionContext) -> bool;
// 显式阻塞类型词表:permission/permission_request/tool_permission/approval_request/
//   input_request/awaiting_input/elicitation_dialog…;不可操作 Notification(idle/auth/result)直接丢,
//   无类型 legacy Notification 保留可解析但生命周期里降权
pub(super) fn background_task_summary(tasks: &Value) -> Option<AiBackgroundTasks>;
// 只看 status 终态词表 TERMINAL_TASK_STATUSES(本机 claude 二进制实测字面量,注释即知识载体,迁移逐字保留);
// in_process_teammate 的 isIdle 特判只对 teammate 生效
pub const MESSAGE_MAX_CHARS: usize = 300;
```

### 会话发现(`slterm_app::ai_sessions`,照抄锚点)

```rust
pub const HEAD_BYTES: usize = 64 * 1024;            // 只读文件头部;标题几乎总在最前
pub const TITLE_STORAGE_CHARS: usize = 512;

pub struct AiSession {
    pub source: AgentKind,      // 本篇锚点类型
    pub id: String,             // claude:jsonl 文件名主干;codex:rollout 文件名 uuid 尾
    pub title: String,
    pub project: String,        // claude=项目目录(编码过);codex=cwd
    pub modified: SystemTime,
    path: Option<PathBuf>,
}
impl AiSession {
    pub fn resume_command(&self) -> Option<String>;  // 消费 AgentKind::resume_command
    pub fn fork_command(&self) -> Option<String>;
    pub fn place_label(&self) -> String;             // 位置词取末段尾缀
}

pub fn scan(limit: usize) -> Vec<AiSession>;         // CLI 原生档案扫描 ∪ hook 注册表合并去重
pub fn record_hook_session(source: &str, session_id: &str, file: Option<PathBuf>, title: Option<&str>);
// upsert 到 ai_sessions.json:原子写 + 文件锁 + 容量上限截断 + 版本字段;
// 只记「如何重新找到会话」,不复制对话正文
pub fn relative_label(modified: SystemTime) -> String;  // 刚刚/N 分钟前/N 小时前/N 天前
// scan_claude(~/.claude/projects/<编码目录>/<uuid>.jsonl)+ scan_codex(~/.codex/sessions 年/月/日
//   手写栈递归,深度封顶)私有;标题提取 claude_title(summary 行优先,否则首条真人 user 消息)
//   + codex_details(session_meta cwd + 首条 input_text);looks_injected 形态筛(< 开头 / Caveat: /
//   # AGENTS.md / # CLAUDE.md / [Request interrupted)+ isMeta——注入块挂 user 名,防「AGENTS.md
//   instructions」标题事故;id 校验复用 resume/fork 命令构造器做不可信 id 过滤器
```

### 回答投影(`slterm_app::assistant_answer`,照抄锚点)

```rust
pub const MAX_ANSWER_BYTES: usize = 128 * 1024;
pub enum AssistantAnswer { Complete(Arc<str>), Missing, TooLarge { bytes: usize } }
impl AssistantAnswer {
    pub fn from_hook(source: &str, payload: &Value) -> Option<Self>;  // 按 source+事件名取字段
    pub fn source(&self) -> Option<&Arc<str>>;
    pub fn notice(&self) -> Option<String>;   // 「不从屏幕猜测、请在终端查看」
}

pub struct AnswerSnapshot { /* 展示用快照字段 */ }
pub struct AnswerInbox { /* 按 (provider, session) 归属 + received_sequence 单调去旧 */ }
impl AnswerInbox {
    pub fn observe(&mut self, event: &AiHookEvent, pane_id: u64) -> bool;
    pub fn close(&mut self);
    pub fn begin_command(&mut self);
}

// assistant_answer/conversation.rs:
pub(super) fn project(path: &Path) -> Option<Conversation>;  // 头部 256KB capture + 分页预算
// 预算契约(照抄):页 1MB / 单消息 48KB / 输出 128KB / 消息数 160;
// validate_source 校验身份与路径一致后才投影;truncated/complete 显式携带;
// 原则:无历史目录扫描、无屏幕到消息的猜测
```

### per-pane 环境契约(`slterm_app::agent_env`,照抄锚点)

```rust
pub const TERM_PROGRAM: &str = "slterm";             // 生态事实标准入口,第三方识别逻辑零改动
pub const CLI_ENV: &str = "SLTERM_CLI";
pub const BIN_DIR_ENV: &str = "SLTERM_BIN_DIR";
pub const PROCESS_ENV: &str = "SLTERM_PROCESS_ID";   // 宿主 PID + pane id 双因子防同机两实例 pane 混淆
// PANE_ENV 复用 ai_hook::PANE_ENV(SLTERM_PANE_ID);LEGACY_* 全删

pub fn apply(env: &mut HashMap<String, String>, pane_id: impl Display);
// 注入组:TERM_PROGRAM/TERM_PROGRAM_VERSION(=VERSION)/PANE_ID/PROCESS_ENV/
//   CLI/BIN_DIR(绝对路径,current_exe() 优先——嵌套启动继承值指向外层旧副本)+
//   BIN_DIR 前置 PATH;变量组只注本地 PTY(WSLENV 臂砍,WSL 全家不采纳)
pub fn executable() -> Option<PathBuf>;              // 优先 current_exe()
// 幂等两件(照抄,WSLENV 第三件砍):
// - PATH 前置按值判重(先移除等值项再前置;Windows 大小写不敏感归一)
// ENDPOINT_ENV 注入与否归 04 篇 runtime API 设计(spec 采纳点 38,方向性)
```

### 安装权威面(`slterm_app::ai_hook::installation` + `local`,照抄锚点)

```rust
// installation.rs —— 共享权威:平台适配器供文件与命令,归属/事件/feature 兼容只有这一处
pub(crate) enum CodexHookMode { Turns, Full }        // 版本化合同;0.154.0 是本机实测验证点
pub(crate) fn codex_mode(version: &str, features: &str) -> Option<CodexHookMode>;
pub(crate) fn codex_groups(mode: CodexHookMode) -> …;
pub(crate) fn merge_groups(existing: &Value, desired: &Value, marker: &str) -> Value;
// marker 语义:只按 marker 认领;用户编辑过的组报「edited … hook preserved」错而不覆盖;
// 卸载恢复 marker 之前的样子
pub(crate) fn enable_codex_feature(raw: &str) -> Result<Option<(String, bool)>, String>;
// toml_edit 保注释;显式 hooks=false opt-out 优先;返回是否本次引入 features 键(卸载精准还原)

// installation/notify.rs —— desired_codex_notify 是 #38 指数膨胀防线的承重件:
// helper 标记出现在任何位置(含别家 JSON 序列化的 --previous-notify 参数内部)都算已接线;
// 只有最外层是自己时才自愈路径;嵌套愈合深度 8 上限;序列化字节预算 8KB 超限拒写;
// 首次改动留 *.slterm-bak

// local.rs —— ensure_claude_hooks 安装纪律:
pub fn ensure_claude_hooks() -> bool;
// 目录不存在不 scaffold;helper 路径拿不到不动手;原子文件锁;非法 JSON 绝不「修复」式覆盖;
// 幂等合并只认领完整自有命令(is_helper_executable / is_helper_shell_command——
// echo/管道/追加命令仍属于用户);CLAUDE_EVENTS 全事件注入;legacy shell 形态原位自愈
fn announce();                                       // 一次性 toast:ai-hooks-announced 哨兵 create_new 原子占位
fn claim_setup_announcement(directory: &Path) -> io::Result<bool>;

// local/managed_files.rs —— 归属指纹:SHA-256 + .slterm-managed marker + 三通道判归属;
//   安装/检视/卸载共用同一判断;被用户编辑过的同名文件报 Conflict 保留不动
// local/config_guard.rs —— 安装/修复调度线程(notify 监听 + 唤醒 channel + Drop 停止);
//   heal_all 拿锁后重读授权;ai_hooks=false 全局开关短路;runtime skills 挂点在此(heal_all)
// local/settings.rs —— inspect 字段面(下详)

// integrations.rs —— 设置页数据源
pub(crate) const AGENTS: [AgentKind; 9] = [Claude, Codex, OpenCode, Cursor, Kimi, Pi, OhMyPi, Copilot, Grok];
pub(crate) struct HookInspection {                   // installed ≠ current 两个事实分离
    pub config_path: Option<PathBuf>, pub available: bool,
    pub installed: bool, pub needs_repair: bool, pub enabled: bool,
    pub helper_missing: bool, pub error: Option<String>,
}
pub(crate) struct AgentIntegration { pub agent: AgentKind, pub executable: Option<PathBuf>,
                                     pub hook: Option<AgentHook>, pub inspection: HookInspection }
pub(crate) fn inspect() -> Vec<AgentIntegration>;
pub(crate) fn hook_for(agent: AgentKind) -> Option<AgentHook>;   // AgentHook 归 06 篇 settings 键域
pub(crate) fn set_enabled(hook: AgentHook, enabled: bool) -> Result<(), String>;
fn executable_directories() -> Vec<PathBuf>;   // PATH + ~/.local/bin .cargo/bin .bun/bin .grok/bin + %APPDATA%\npm + %LOCALAPPDATA%\cursor-agent(macOS 分支砍)
fn find_executable(agent: AgentKind, dirs: &[PathBuf]) -> Option<PathBuf>;
// 通用文件名按安装路径归属:canonicalize 后查路径分量(.grok/bin/agent 不误报 Cursor,反之亦然);
// 扩展名序 .exe/.cmd/.ps1/无;桌面编辑器 cursor 启动器不证明 Agent CLI 已装(别名+路径双判据)
```

`AgentHook` 枚举(claude/codex/opencode/pi/copilot/grok/ohMyPi/cursor/kimi 九变体)住在 `slterm_settings`(设置键域,eg 06 篇),本篇只消费——ai_hooks 全局开关的读口在此,06 篇定键。

## 数据流与状态机

### hook 事件全链(管道 → 仲裁 → UI)

```
AI CLI(claude/codex/…)全局配置里的 hook 命令
  → slterm-hook 小进程(串台门 FOREIGN_HOOK_RUNNERS → SLTERM_NOTIFY_PIPE 闸门
    → 信封 slterm-hook/1 + 原样 JSON → 管道写,2s 超时,exit 0 恒成立)
  → slterm-ai-pipe 线程 serve:CreateNamedPipeW 全新实例等待连接
    → GetNamedPipeClientProcessId(断开前读)→ client_pid
    → nearest_agent_ancestor(client_pid) → event.agent_pid
    → protocol::parse_envelope(第二道串台门 + provider 归一 + 有界提取)
  → mpsc channel(spawn_gpui_server)→ GPUI foreground executor 排水
  → 按 event.pane(SLTERM_PANE_ID 环境值)路由到 pane 的 AgentActivity
    ├─ AgentActivity::accepts_hook(所有权预检:command_ended / legacy 降权 /
    │   native_codex 双桥 / turn_id / owner 匹配)
    ├─ AiHookEventGate::accept_for_pane(七判,非 Accepted 落日志)
    ├─ AgentActivity::apply_hook(状态机)→ AgentStatus 边
    │   ├─ TurnDone(带 AssistantAnswer + AiTurnOutcome)→ 通知漏斗(eg 09)
    │   └─ NeedsAttention(带 AttentionContext)→ 通知漏斗 + 页签 attention
    └─ AnswerInbox::observe(回答按 (provider, session) 收编)
  → UI:页签徽标/attention 态归 05/09 消费;回答展示归 07
```

辅助通路(与 hook 事件并行、不混载):statusline 桥接脚本 → context 用量信号(分通道,eg D03-3);屏幕观察采样 → `ai_agents::detect` → `AgentActivity::observe_screen`(仅在 `allows_screen()` 时有发言权)。

### 作用域闸门数据流(照抄)

```
slterm_app 启动早期:spawn_pipe_server
  ├─ set_var(SLTERM_NOTIFY_PIPE, \\.\pipe\slterm-notify-<pid>)   进程级
  ├─ set_var(SLTERM_HOOK_EXE, <exe 旁 slterm-hook.exe 路径>)      进程级
  └─ spawn 管道服务线程
ConPTY spawn:ConPTY 合并当前进程环境 → 全部 pane 的子进程树天然携带闸门
  → 在 slTerminal 外启动的同名 CLI:无闸门 → slterm-hook NotHosted → 无声退出
```

### AgentActivity 仲裁状态机(照抄语义)

```
初始 Unknown/Unknown;begin_command(agent=true) → Working(Process)
                       command_finished → reset + command_ended 锁存(拒绝迟到事件)
屏幕观察:observe_screen → detect(program, screen) → Detection
  ├─ coverage == Lifecycle → 屏幕结果被忽略(allows_screen = false)
  ├─ Blocked 命中 → status=Blocked(Screen),rule_id 记录
  └─ idle 样本累计 → screen_armed 且 idle 帧 → Idle
hook 事件:apply_hook 按 kind:
  SessionStart → 记录 owner(source/session/pid/bridge_instance),coverage 升级
  PromptSubmit/submitted → Working(Process),turn_observed=true
  NeedsAttention → Blocked(Hook),AttentionContext 携带权限档位
  ToolComplete → Blocked 拉回 Working(权限已批)
  TurnDone → 按 AiTurnOutcome 分类;active 后台任务 > 0 → 保持 Working;
             否则 Done;answer 进 AnswerInbox
  SessionEnd → owner 清空,coverage 回落
嵌套子代理:turn_id/owner 不一致的事件被 accepts_hook 拒收,不污染主回合
```

### 安装/修复守护状态机(config_guard,照抄)

```
安装调度线程启动 → notify watcher 监听各家配置文件目录 + 唤醒 channel + Drop 停止
  → heal_all:拿原子文件锁 → 重读授权(不能把菜单/卸载刚移除的 hook 按旧快照装回)
    → ai_hooks=false(归 06 设置键)→ 全局短路
    → 逐家 ensure_*(claude settings.json / codex config.toml / kimi config.toml /
      cursor hooks.json / managed_files 指纹对账 / runtime_skills 挂点归 08)
卸载路径:只移除带 marker 的自有组;恢复 marker 之前的样子;用户编辑过的组保留
检视路径:inspect() → Vec<AgentIntegration>(installed/current 双事实 + helper_missing)
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;每项一行:pebrel 源 · 符号 + 缝合点 + 因果链一句)

| pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- |
| `nebula_hook/src/main.rs` `main`/`run`/`read_payload`/`envelope`/`send_local`/`chain_notifier` + `Cargo.toml` pure-std 零依赖 | `slterm_hook` 全量改名 | hook 是事件链第一推动力,exit 0/1MB/2s/stdin 抽干是对 CLI 的行为合同 |
| `nebula_hook/src/main.rs` `FOREIGN_HOOK_RUNNERS`/`foreign_hook_runner` | 全量 | 串台门第一道:别家 runner 独有环境变量是唯一可靠判据,空值不算命中 |
| `nebula_app/src/ai_hook.rs` `PIPE_ENV`/`PANE_ENV`/`HOOK_EXE_ENV` | 全量改 `SLTERM_*` | 环境作用域闸门:hook 配置全局、效果必须 slTerminal-only |
| `nebula_app/src/ai_hook/windows.rs` `spawn_gpui_server`/`spawn_pipe_server`/`serve` | 全量改名(线程名/管道名依 01 篇表) | 内核进程身份核验是「谁在写管道」的唯一权威答案 |
| `nebula_app/src/ai_hook/protocol.rs` `parse_envelope` | 全量改协议头 `slterm-hook/1` | 载荷形状二次校验 = 串台双门的宿主侧防线;claude 绝不读 camelCase |
| `nebula_app/src/ai_hook/payload.rs` 有界提取/脱敏/可操作性/终态词表 | 全量,注释逐字随迁 | 「后台 bash 还在跑就先弹完成」的修复核心是终态词表,注释即知识载体 |
| `nebula_app/src/ai_hook/ordering.rs` `AiHookEventGate`/`GateVerdict`/`accept_for_pane`/`reorder_batch` | 全量 | 512 流 LRU + 七判带原因——事件静默丢弃必须有可取证面 |
| `nebula_app/src/ai_hook/lifecycle.rs` `AgentActivity`/`HookCoverage`/`Owner` | 全量 | hook 事件是事实、屏幕是有限权威观察;命令所有权止于 shell 边界 |
| `nebula_app/src/ai_hook/event.rs` `AiHookKind`/`AiTurnOutcome`/`AiHookCapabilities`/`capabilities_for`/`AiPermissionMode`/`AiBackgroundTasks`/`AttentionContext`/`AiHookEvent` | 本篇锚点 | 能力不对称显式声明,防上层把「有生命周期 hook」误当「有顺序保证」 |
| `nebula_app/src/ai_hook/installation.rs` `codex_mode`/`merge_groups`/`enable_codex_feature` | 全量 | 安装权威单点:codex 版本分流/marker 认领/编辑保留 |
| `nebula_app/src/ai_hook/installation/notify.rs` + `local/codex_notify.rs` | 全量改 `.slterm-bak` | desired_codex_notify 防 #38 指数包装膨胀(130MB 实证) |
| `nebula_app/src/ai_hook/local/codex_hooks.rs` | 全量改 `hooks.slterm-managed.json` | 能力探针真跑 codex(3s 限时),不认广告信口;探针失败退回 legacy notify |
| `nebula_app/src/ai_hook/local.rs` `ensure_claude_hooks`/`announce`/`claim_setup_announcement` | 全量 | 不 scaffold/不修复式覆盖/原子锁/幂等认领完整自有命令 |
| `nebula_app/src/ai_hook/local/kimi.rs` | 全量 | kimi 四字段硬约束(#80 空格路径教训),三键写入 + 双引号包裹 |
| `nebula_app/src/ai_hook/local/cursor.rs`/`native_events.rs`/`local/extended.rs` | 全量 | 五事件映射 + EncodedCommand 形态识别;copilot/grok/omp 同形态 |
| `nebula_app/src/ai_hook/bridges.rs` + `res/hooks/opencode.js`/`pi.ts` | 全量改 `SLTERM_HOOK_EXE` | 插件落盘桥与 opencode SDK 演进解耦;Bun 天然串行 + 3s 看门狗 |
| `nebula_app/src/ai_hook/local/managed_files.rs` | 全量改 `.slterm-managed`,legacy hashes 砍 | SHA-256 指纹三通道判归属,安装/检视/卸载共用 |
| `nebula_app/src/ai_hook/local/config_guard.rs` | 全量 | heal_all 拿锁重读授权,防旧快照回装 |
| `nebula_app/src/ai_hook/integrations.rs` `AGENTS`/`HookInspection`/`inspect`/`executable_directories`/`find_executable` | 全量,macOS 目录分支砍 | 9 家一等清单;installed ≠ current 双事实;「agent」通用名按路径归属 |
| `nebula_app/src/ai_agents.rs` `AgentKind`/`ALL`/`slug`/`display_name`/`aliases`/`parse`/`AgentStatus`/`AgentStatusSource`/`Detection`/`detect` | 本篇锚点 | 27 家身份事实单源;aliases 进程识别与设置页发现共用 |
| `agent_detection/*.toml` 20 厂商 + `_shared.toml` | 资源文件随迁 | 声明式规则改不动代码;claude.toml 实测知识注释逐字保留(esc 唯一可靠证据/动画符号不可当证据/❯ ≠ 空闲) |
| `nebula_app/src/ai_agents.rs` 用户覆盖机制 + mtime 节流(≤2s) | 全量 | 运行中调参但文件系统不上每一帧;覆盖按 slug 一对一 |
| `nebula_app/src/ai_agents/screen_context.rs` `has_live_input_controls`/`CONTROLS`/`BINARY`/`attention_region` | 全量 | 结构判据公共闸:assistant 正文里的字不是事件,只有最近控制行说了算 |
| `nebula_app/src/ai_sessions.rs` `HEAD_BYTES`/`AiSession`/`scan`/`record_hook_session`/`relative_label`/`looks_injected` | 全量 | 64KB 头部有界扫描 + 注册表合并;注入形态筛防标题事故 |
| `nebula_app/src/assistant_answer.rs` `AssistantAnswer`/`AnswerInbox` + `conversation.rs` 投影 | 全量 | 回答三态 + 256KB 有界投影;不从屏幕猜测 |
| `nebula_app/src/agent_env.rs` `apply`/`executable`/PATH 幂等两件（WSLENV 臂砍） | 全量改 `SLTERM_*`,`PROCESS_ENV` 单名 | per-pane 身份变量组;current_exe 优先防嵌套套娃 |
| `nebula_app/src/ai_hook/local/settings.rs` `inspect` | 全量 | 检视字段面(归设置页数据源,eg 06) |
| `nebula_app/src/process_tree.rs` `nearest_agent_ancestor` | 全量归 app 平台件 | 内核 pid → agent pid 的祖先链解析,身份核验的另一半 |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威;本篇直接消费点:信封头 `slterm-hook/1`(P-1)、管道 `\\.\pipe\slterm-notify-<pid>`(P-2)、`SLTERM_NOTIFY_PIPE`/`SLTERM_PANE_ID`/`SLTERM_HOOK_EXE`/`SLTERM_CLI`/`SLTERM_BIN_DIR`(C 节)、managed marker `*.slterm-managed` 与备份 `*.slterm-bak`(D 节)、线程名 `slterm-ai-pipe`(P-7)、`TERM_PROGRAM=slterm`。本篇域内无新增映射项。

### 缝合点

1. **02 篇(终端事件)**:OSC 133;C/D(`Event::CommandStart`/`CommandDone`)与 `Event::CwdReport` 是仲裁器的 shell 边界数据源——壳在收到 `CommandDone` 后调 `AgentActivity::command_finished`;OSC 133;C 首 token 命中 AgentKind 走 `parse_command`(静态识别),与屏幕规则(运行态观察)经同一仲裁器汇合,互不推导。
2. **04 篇(runtime API)**:`AgentActivity::status`/`source`/`primary_pid` 暴露给 runtime 的 pane 方法族;`PROCESS_ENV` 双因子与 runtime 端点注入形态归 04;`AgentKind::slug` 是 runtime 信封里的 source 值。
3. **05 篇(pane 归属)**:`PaneId` newtype 本篇以 `u64` 形参承接 05 篇类型(事件门/仲裁器/AnswerInbox 签名处);pane 的 `AgentActivity` 实例挂 `TerminalPane`,由 05 篇 Workspace 生命周期创建/销毁;`SLTERM_PANE_ID` 环境值与 PaneId 分配器同源(05 篇单调计数器)。
4. **06 篇(设置双轨)**:`AgentHook` 枚举与 `ai_hooks` 全局开关键归 06 键域;设置页 hooks 组数据源 = `integrations::inspect()` 字段面(installed/needs_repair/helper_missing 三态 + config_path 直达),替换 slTerminal 旧注入三态展示。
5. **08 篇(AI assistant)**:config_guard `heal_all` 的 runtime skills 挂点;`AiTurnOutcome::Failed` 等失败边归 08 的建议条触发源之一。
6. **09 篇(通知)**:TurnDone/NeedsAttention 两个状态边归 09 的 `Notification` 漏斗(枚举形态归 09 锚点,本篇只保证边干净、带上下文);attention 闪烁由生命周期状态驱动。
7. **11 篇(测试)**：本篇全部测试点的基础设施形态(虚拟窗口/夹具/门禁登记)归 11。

## 改造 / 移植 / 新建设计

### 1. slTerminal hooks 信号文件通道退役消亡清单

| 消亡项(旧栈位置) | 消亡语境 | 新世界对应物 |
| --- | --- | --- |
| `src-tauri/src/hooks/watcher.rs` notify + 3s 轮询双通道 watcher | 管道直连后文件中介失去存在理由 | 命名管道 `serve` 线程;失败语义显式(PipeUnavailable/NotHosted),无静默失效面 |
| `src-tauri/src/hooks/signal.rs` 信号文件读写 | 同上 | hook 载荷走管道,零中间文件 |
| `~/.slterminal/hooks-events/` 信号目录 + `slterm-hook-reporter.js` 落盘 | 同上 | `slterm-hook.exe`(安装进各家 CLI 配置);reporter 的 C10 合同(exit 0/不写 stderr)精神由 helper 的 catch_unwind + 超时等待继承 |
| `agent-event` Tauri 事件广播 | 跨进程边界消失 | 单进程 mpsc 直送 foreground executor 排水 |
| 注入三态 `AgentInjectionStatus`(Injected/Outdated/NotInjected) | 数据源换代 | `HookInspection` 字段面;installed/current 双事实取代版本/哈希三态 |
| settings.json 自研 merge 逻辑(`hooks/claude/inject.rs` 等) | pebrel 安装纪律为终态权威 | `ensure_claude_hooks` + marker 认领/编辑保留/不修复式覆盖/原子锁 |

保留物(不随通道退役):SEC-12 审查门(改造节 2)、statusline 用量桥接(改造节 3)、关闭恢复 statusline 语义(归 09 窗口收尾挂点;备份-还原形态随安装器纪律重设计)。

### 2. SEC-12 statusline 审查并入新安装器(签名级)

slTerminal 独有、pebrel 无对应物,保留并合流进 `ensure_claude_hooks` 的安装纪律:

```rust
// local.rs 域内新增(改造,非照抄):
pub(crate) enum InstallGate {
    Proceed,
    /// 命中可疑原命令,返回原文供 UI 二次确认(零写盘)
    Suspended { suspicious_command: String },
}

pub(crate) fn review_statusline_command(raw: &str) -> InstallGate;
// 审查词表(curl/wget/Invoke-Expression/iex 等,源自旧 hooks 域审计三件套 SEC-12)
// 仅作用于「我们即将接管的 statusLine 原命令」;审计进 target: "audit" 通道归 10 篇

pub fn ensure_claude_hooks(gate: impl Fn(&str) -> InstallGate) -> InstallOutcome;
// 安装入口携带审查门:Proceed 正常;Suspended 时 settings.json 零写盘,
// 由设置页展示原文,用户确认后经 confirm 路径(跳过审查)完成注入。
// 启动自愈/重注入路径(reconcile)命中 → 跳过 + 审计,不改写用户配置(无交互可用)——
// 语义继承自旧「启动对账只补缺失、不覆盖」纪律
pub enum InstallOutcome { Installed, Current, Suspended { suspicious_command: String }, Conflict }
```

合同位置:审查发生在「写盘前」,与 pebrel「非法 JSON 绝不修复式覆盖」同一层——都是**安装权威不得擅自处置用户配置**的分支。UI 消费归 06(设置页 hooks 组),审计落盘归 10。

### 3. statusline 用量桥接分通道保留形态

statusline 的 `context_window.used_percentage` 官方口径只存在于 statusline stdin JSON,hook 事件链路不含此信息(spec 采纳点 42)——互补通道保留:

```rust
// slterm_app::statusline(新建,改造):
pub const STATUSLINE_BRIDGE_SCRIPT: &str = include_str!("res/slterm-statusline.js");  // 随安装器落盘归 local/*
pub const USAGE_SIGNAL_DIR: &str = …;   // 介质裁定见 D03-3(信号文件 or settings 写通道归 06)
// 旧形态:stdin → 提取 used_percentage → 节流(取整无变化不写 + ≥1s)→ 原子写信号文件
//         → 透传用户原 statusline 命令
// 新世界形态三件套:
// - 桥接脚本注入/卸载并入 ensure_claude_hooks 的 statusLine 键接管(备份-还原纪律同 hook 安装)
// - 原命令审查走改造节 2 的门(SEC-12)
// - 用量信号与 hook 管道分通道,不混入 AiHookEvent 载荷
```

### 4. cliProfiles 退化为 UI 表现层

身份事实单源收敛到 Rust `AgentKind`(27 家枚举,本篇锚点);旧 `src/features/cliProfiles/cliProfileRegistry.ts` 的前端枚举消亡,GPUI 侧对应物:

```rust
// slterm_app::gpui_shell::cli_profiles(新建,UI 表现注册表):
pub struct CliProfileView {          // 纯表现,无身份裁决权
    pub kind: AgentKind,             // 身份消费 Rust 枚举,禁本地重列清单
    pub icon: IconHandle,            // eg 06 主题/资产归 06
    pub tab_title_hint: Option<&'static str>,   // OSC 133 首 token 命中的展示hint归 02 消费链归壳
    pub editor_hooks: Vec<ConfigEditorHook>,    // claude 配置编辑器挂点归 06/07
}
pub fn profile_view(kind: AgentKind) -> Option<CliProfileView>;  // 注册表单点,归壳
```

旧前端侧的 alias 校验(`aliasValidation.ts`)/globalFiles 能力声明(`AgentGlobalDir` DTO)随之消亡:别名归 04 CLI 动词、globalFiles eg 改造节 5。OSC 133 首 token 命中(页签标题/logo)由壳「命令 profile 匹配器」消费 `AgentKind::parse_command`,与屏幕规则并存不互导(spec 优化方向「身份事实单源」)。

### 5. agent_dirs 扩至 27 家(沙箱放行真值源)

旧 `agent_dirs.rs` 静态表 1 家(claude)扩到 27 家;身份域分工:目录表管沙箱放行、AgentKind 管身份、TOML 管屏幕规则——三份清单各管一域,不合并(合并 = 单点失效面扩大):

```rust
// slterm_app::agent_dirs(改造,提取自 git 历史归 01 篇):
/// cliId → home 相对配置目录。27 行静态表,与 AgentKind::ALL 对齐(编译测试钉对)。
/// 新增 agent = AgentKind 加变体 + 本表加行 + 屏幕 TOML 加文件,三处同步登记 11 篇门禁。
const AGENT_CONFIG_DIRS: [(&str, &str); 27] = [
    ("claude", ".claude"),
    ("codex", ".codex"),
    ("gemini", ".gemini"),
    /* …23 行,值以各 CLI 官方文档/实测为准(eg opencode=.config/opencode, cursor=.cursor) */
];
pub(crate) fn agent_dir_paths() -> Vec<PathBuf>;   // 路径沙箱放行集归 07 文件域消费
pub fn agent_global_dirs() -> Vec<AgentGlobalDir>; // 设置页/侧栏归 05/06
```

旧「两侧各自枚举同一 agent 集属刻意冗余」的冗余收敛:身份枚举(AgentKind)单源在 Rust;放行目录表(安全边界)留在后端静态表;前端 profile 的能力声明层消亡。

### 6. agent_history 被 ai_sessions 吸收

旧 `agent_history/`(全量扫描 + 指纹缓存 + SEC-05 校验前置 + restoreSession 注入)形态替换:

- **数据源换代**:`ai_sessions::scan` 头部有界读 + hook 注册表合并取代全量扫描缓存;旧指纹失效机制无存在前提(每次扫描只读 64KB 头,代价已天然有界)。
- **resume/fork 统一**:`AgentHistorySession` DTO 消亡,恢复注入消费 `AiSession::resume_command`/`fork_command` 产物(05 篇恢复注入归 `restore_agent` 挂点);旧「133;A 恢复注入闸门」由 02 篇语义 prompt 模型替代。
- **标题回退链同源**:`claude_title`/`codex_details` 两侧同源,旧链路的第二份回退逻辑删除。
- **SEC-05 校验前置**:不可信 id 过滤复用 resume/fork 命令构造器做过滤器(spec 采纳点 32),等效保留。

```rust
// 05 篇缝合接口(本篇给出):
pub(crate) fn restore_agent(leaf: &AgentSession) -> Option<String>;
// LayoutSession::Pane.agent(source/session_id/session_file)→ 启动命令行归 05 恢复注入消费;
// 只构造命令,执行归 02 spawn 链
```

### 7. 与 02/04/05/06 的缝合契约(改造侧)

- **02 篇**:shell 边界事件序由 `StreamProcessor` 偏移切分保序(02 篇锚点);壳在 `Event::CommandStart`/`CommandDone` 上调 `AgentActivity::begin_command`/`command_finished`;`SLTERM_PANE_ID` 注入点在 `Options.env` 组装(02 篇「固定注入清单」末位叠加,core 只负责时机)。OSC 133 首 token → `AgentKind::parse_command` 在壳侧完成,core 不感知 AgentKind。
- **04 篇**:runtime 暴露 `AgentActivity::status`/`source`/`primary_pid` 只读快照;pane 方法族的 agent 字段值集 = `AgentKind::slug` 全集(版本化信封归 04);`SLTERM_CLI`/`SLTERM_BIN_DIR` 是 `slterm` CLI 资源动词的离线发现面(eg 04)。
- **05 篇**:事件路由表 pane 键 = `PaneId`;`TerminalPane` 持有 `AgentActivity` 与 `AnswerInbox`;关 pane 时随 05 篇 shutdown 纪律一并 drop;恢复注入消费改造节 6 的 `restore_agent`。
- **06 篇**:设置页 hooks 组渲染 `Vec<AgentIntegration>`;`ai_hooks` 全局开关与 `AgentHook` 键域归 06;`HookInspection.config_path` 直达打开(eg 07 文件域)。

## 测试点清单

> 引测试一律例名;测试基础设施形态归 11 篇。pebrel 侧承重用例全量随迁改名(用例名不变),替换 slTerminal 旧信号通道测试;bugfix 防复发纪律按本仓测试金字塔归位。

| 测试 | 层级 | 来源 |
| --- | --- | --- |
| hook 进程合同族:panic 路径 exit 0 / 1MB 上限抽干不转发 / 2s 超时一次性等待 / 非白名单参数无声 no-op / `--chain` 不驱逐原 notifier / NotHosted 恒无声 | L1 slterm_hook(注入式 env+管道 mock) | 迁移 pebrel `nebula_hook` 用例 |
| 串台双门:`foreign_runner_payload_is_rejected_even_if_the_env_gate_fails`(钉双防线独立性)+ 空值变量不算命中 | L1(helper 侧 + protocol 侧各一) | 迁移 |
| 管道身份核验:`GetNamedPipeClientProcessId` 断开前读、载荷自报 pid 与内核 pid 不一致时拒收、竞态重试 20×5ms | L1(管道 mock)/L1 集成(真管道,--test-threads=1) | 迁移 |
| 信封解析:首行协议头 `slterm-hook/1`、camelCase 拒收、session_id 键名按 source 收紧、codex 双形态(legacy/native)、native agent_id 子代理丢弃、有界字段(id 512/消息 300/结果 4000) | L1 protocol | 迁移 |
| 事件门七判全族:DuplicateEventId / StaleSequence / StaleTime / DuplicateFingerprint(1.5s 窗口)/ AfterSessionEnd(SessionStart 复活)/ UnorderedAfterDone(serialized_delivery 翻案)/ Accepted;512 流 LRU 驱逐序(received_sequence 最旧) | L1 ordering | 迁移 |
| `reorder_batch`:同 pump 全序号组内重排、跨批次旧序号拒绝 | L1 | 迁移 |
| AgentActivity 仲裁族:command_finished 拒迟到事件 / Lifecycle 覆盖剥夺屏幕发言权(`allows_screen`)/ Completion-only `screen_armed` 锁存 / Blocked→ToolComplete 拉回 / TurnDone 有在飞后台任务保持 Working / 嵌套子代理 turn_id 拒收 / native_codex 双桥不降级 | L1 lifecycle | 迁移 |
| 能力表 `capabilities_for` 27 source 字面量契约(不对称显式声明) | L1 event | 迁移 |
| 安装权威族:`version_and_advertised_feature_select_the_installed_contract` / `merge_upgrade_and_remove_preserve_foreign_hooks_and_metadata` / `edited_and_malformed_hook_configs_are_not_overwritten` / `feature_installation_respects_user_opt_out_and_comments` | L1 installation | 迁移 |
| #38 防线:desired_codex_notify 标记任意位置算已接线 / 只自愈最外层 / 嵌套愈合深度 8 / 8KB 序列化预算拒写 | L1 installation/notify | 迁移 |
| claude 安装:不 scaffold / 原子锁 / 非法 JSON 不修复式覆盖 / 幂等认领完整自有命令(echo/管道/追加归用户)/ legacy shell 形态原位自愈 / 自愈保留 timeout 字段 | L1 local.rs | 迁移 |
| kimi 安装器:三键写入(event/matcher/command 中省略 matcher 四字段硬约束)/ 双引号包裹空格路径(#80 回归)/ 八事件订阅面 | L1 local/kimi | 迁移 |
| cursor/native_events:五事件映射 / EncodedCommand base64 解码回 UTF-16 校验 | L1 | 迁移 |
| 归属指纹:三通道判归属 / 用户编辑报 Conflict 保留 / 设置页不误显示同名用户文件为已接入 | L1 managed_files | 迁移 |
| `claim_setup_announcement`:create_new 原子占位只报一次 / 并发只占一位 / 非目录报错 | L1 local.rs | 迁移 |
| 屏幕规则:20 厂商 + `_shared.toml` 全例随迁(含 claude「esc to interrupt 唯一证据」「动画符号不当证据」「❯ ≠ 空闲」三组实测注释对应用例)/ 用户覆盖一对一 / mtime 节流 ≤2s | L1 ai_agents | 迁移 |
| screen_context:live-control 边界(quoted 旧表单压不过新 footer)/ codex composer attention_region 特判 | L1 | 迁移 |
| 27 家身份:`aliases_resolve_to_canonical_agents` 全别名归一 + `AgentKind::ALL` 27 项钉死(数量进测试不进文档) | L1 ai_agents | 迁移 + 新建 |
| 可执行发现:`.grok/bin/agent` 不误报 Cursor 反例双例 / 桌面 cursor 启动器不算 Agent CLI / 扩展名序 | L1 integrations | 迁移 |
| ai_sessions:64KB 头部读(大文件不整读)/ claude_title 回退链 / codex rollout uuid 尾提取 / `looks_injected` 五形态筛 / 注册表合并取较新 modified / id 复用 resume 构造器过滤 / `relative_label` 档 | L1 ai_sessions | 迁移 |
| assistant_answer:三态(Complete/Missing/TooLarge)/ notice 文案 / `AnswerInbox` (provider,session) 归属 + 单调去旧 | L1 | 迁移 |
| conversation 投影:256KB capture / 分页预算(1MB/48KB/128KB/160)/ validate_source 身份校验 / truncated 显式 | L1 | 迁移 |
| agent_env：注入组字面量全等 / PATH 值判重幂等（嵌套不涨份）/ `executable()` current_exe 优先（WSLENV 件砍） | L1 agent_env | 迁移 |
| **SEC-12 审查并入**:`review_statusline_command` 词表正反例 / Suspended 零写盘 / confirm 路径跳过审查 / 启动自愈命中跳过+审计 | L1(改造节 2 新建) | 新建(防复发:旧 CP-043 用例语义改写) |
| **27 家 agent_dirs 表对齐**:表长 == `AgentKind::ALL.len()` 编译守卫 + 每行目录值字面量钉 | L1 agent_dirs | 新建 |
| **信号通道退役守卫**:全树 grep 无 `hooks-events`/`slterm-hook-reporter.js`/`AgentInjectionStatus` 残留归 01 篇禁名门禁扩展词表 | 门禁归 01/11 | 新建 |
| 管道端到端(真机、串行):claude 真 spawn → settings.json 已接线 → hook 事件 → 仲裁 → 状态边;codex legacy/native 双形态 | L1 集成(eg 11 篇豁免口径) | 迁移范式 |

## 阶段归属与出口标准

本片主体归 **M4 AI CLI 集成**(00-roadmap),细分五步,出口全为可机验项:

**M4.1 `slterm_hook` crate 迁入**(依赖序中紧随 slterm_split,eg 00-roadmap M1):pebrel `nebula_hook/` → `slterm_hook/`,砍 unix 分支与远端 OSC 臂,改名 `SLTERM_*`。出口 = `cargo check` 过;hook 合同用例族全绿(panic exit 0/1MB/2s/stdin 抽干);零生产依赖契约校验过。

**M4.2 `ai_hook` 协议与仲裁层**:`ai_hook.rs`/`windows.rs`/`protocol.rs`/`payload.rs`/`ordering.rs`/`lifecycle.rs`/`event.rs` 落 `slterm_app`。出口 = 信封解析/串台双门/七判全族/仲裁族用例全绿;协议头 `slterm-hook/1` 两侧同步(helper + app 字面量测试钉);身份核验用例过(管道 mock 层)。

**M4.3 安装权威 + 9 家安装器 + SEC-12 合流**:`installation*`/`local/*`/`bridges.rs`/`integrations.rs`/`res/hooks/` + 改造节 2 审查门。出口 = 安装权威四用例/#38 防线/claude/kimi/cursor 安装器/指纹/哨兵用例全绿;`review_statusline_command` 正反例绿;禁名门禁过(marker/备份后缀等新品牌位零旧名)。

**M4.4 识别/屏幕/会话/环境**:`ai_agents`/`agent_detection/*.toml`/`screen_context`/`ai_sessions`/`assistant_answer`/`agent_env`/`agent_dirs`(27 家)。出口 = 27 家身份用例 + 屏幕规则全例 + 会话扫描/注入筛/回答三态/环境幂等两件全绿（WSLENV 件砍）;agent_dirs 对齐守卫绿。

**M4.5 壳侧点亮(依赖 M3)**:管道服务器接线 GPUI foreground executor;`AgentActivity`/`AnswerInbox` 挂 M3 单 pane 宿主(宿主形态归 M3)。三件前倾工作改登记为后阶段件的缝合点,不在 M4.5 落地:`TerminalPane` 三件套挂载归 05 篇 M5.3(`TerminalPane` 锚定 05 篇)、`restore_agent` 恢复注入缝合归 05 篇 M5.5、通知漏斗接通归 09 篇 M10.1——M4.5 仅做事件通道。出口 = 管道端到端集成用例(真机串行)绿;`cargo test` 全绿;AI 检测/事件链路点亮对齐 00-roadmap M4 出口(hook 链路契约测试过 + 禁名门禁过)。

M4 全程关窗即退出无 mux;statusline 桥接介质随 D03-3 落地,不阻塞 M4.5 主链。

## 待沉淀决策

> [待沉淀] **D03-1 · `AgentSession.source` 字段值域:自由字符串 vs `AgentKind::slug` 封闭集**。真实权衡:05 篇 schema 锚的 `AgentSession.source: String`(hook 直报的 CLI 名,serde 层不约束)允许未登记 CLI 的会话也能落盘恢复;封闭集则把身份事实单源贯彻到持久化层,代价是新增 CLI 须同步改 schema 判据。意外因素:27 家之外的 CLI 经 hook 上报(用户自接)在单源纪律下会被静默丢弃,与「面向所有 AI CLI 调优」的定位存在张力面。难逆点:session 文件值域一旦放开再收紧 = 数据判读变更。截止:M4.4 session schema 落地时定(默认:String 保留 hook 原报,eg 恢复注入时经 `AgentKind::parse` 归一,不归一即不可恢复——宽松落盘、严格消费)。

> [待沉淀] **D03-2 · SEC-12 审查词表的演化归属**。真实权衡:词表(curl/wget/iex 等)是安全审计件,误判 = 每次注入多一步人工确认(摩擦),漏判 = 恶意原命令被桥接接管;词表随新攻击面演化。意外因素:审查门只覆盖 statusLine 键,各家 hooks 数组里的 command 字段(我们写的部分除外)不在审查面——pebrel 的 marker 认领已覆盖归属,但「用户先装了我们、再手动改坏」的路径两形态都未审。难逆点:词表收紧方向可逆、放松方向不可逆(一旦漏过的命令形态被合法化,再加回审查 = 兼容破坏)。截止:M4.3 审查门落地时定初版(eg 10 篇审计域登记,演化走 note 链归 01 篇)。

> [待沉淀] **D03-3 · statusline 用量信号介质:信号文件保留 vs 并入 settings 写通道(eg 06)vs 进程内直传**。真实权衡:旧形态原子写信号文件 + 前端轮询/监听,跨进程解耦、CLI 不可用时零耦合,代价是文件系统税与第二通道复杂度;并入 settings 写通道则全量偏好态一处持久化,代价是每秒级用量 churn 进 1Hz 快照冲突面(05 篇快照纪律的写盘时机是布局不是用量);进程内直传(桥接脚本写管道第二端口)最省介质但要求 slterm-hook 常在线、桥接脚本复杂度上移。难逆点:桥接脚本随安装器固化在用户机器,介质变更 = 已安装基地的兼容义务。意外因素:用量信号消费方归壳(HUD/页签),纯进程内态若选直传则历史曲线无来源。截止:M4.3 安装器落地前定(默认:信号文件介质保留归 app 数据目录归 01 篇,演化面最小——与 hook 管道分通道原则一致)。

> [待沉淀] **D03-4 · `Owner`/`AiHookStreamKey` 的 `remote_process` 字段随 SSH 砍除后的键收窄**。真实权衡:字段砍除后流键少一维,同名本地进程身份合并概率上升(两个不同远程会话串流的前提已不存在,但同 pid 复用窗口内的歧义仍在);保留字段则死代码常驻。意外因素:单窗口单实例下 pid 复用窗口 ≈ pane 生命周期,pane id 维度已兜住。难逆点:键结构进测试字面量后再加回 = 全键用例重写。截止:M4.2 仲裁层落地时定(默认:砍除,流键 = source + session + pane + agent_pid + bridge_instance)。

## 开放问题

1. **native codex `agent_id` 子代理事件的丢弃边界**:spec 采纳点 6 定「直接丢」;若未来 codex 子代理以独立 rollout id 上报(当前与主会话共享 session_id),丢弃规则需按 turn_id/agent_id 两维重评——M4.2 落地时以当时 codex 版本实测为准,规则注释留锚。
2. **`AgentKind::parse_command` 与 OSC 133;C 首 token 的形态差异**:前者解析裸命令(argv 首 token),后者拿到的是 shell 整行(含参数/管道/路径前缀);壳侧匹配器(eg 改造节 4)的归一规则(路径剥离/引号/npx 包装)归 M4.5 实现期,本篇不定算法。
3. **agent_history 旧数据的去向**:旧栈 `agent_history` 的扫描缓存与历史索引随 M0 删除;用户已有的 CLI 原生会话档案(claude/codex 目录)天然被 `ai_sessions::scan` 重新发现,无迁移义务——但旧「最近会话」列表的排序/展示偏好不可自动化恢复,属可接受损失,登记确认归 M4.4。
4. **流键中 `bridge_instance` 仅 pi 消费的特判位置**:ordering.rs 的 `stream_key` 在 source == "pi" 时以 bridge_instance 代 session_id;该特判留在流键构造处(照抄)还是上移到 protocol 归一层(归一时填 session_id),M4.2 实现期按改动面最小定,两处测试锚等价。
5. **设置页「Agent 组」旧 ADR-0023 的面貌**:hooks 页数据源换代(注入三态 → HookInspection 字段面)后,旧页的「注入/卸载」按钮语义直接映射 set_enabled/install/uninstall,eg 06 篇键域;页面其余区块(statusline/全局文件视图)归 06/05,本篇不 preempt。
