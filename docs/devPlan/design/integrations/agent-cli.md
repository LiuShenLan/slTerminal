# CLI/Agent 集成

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

### 职责与边界

本域负责：AgentKind 身份与能力注册表、命令识别与别名、启动/恢复/分叉命令、Hook 能力声明与事件摄取、statusline 上下文用量、历史会话扫描与恢复计划、套餐余量、进程快照与身份解析、AI 状态系统通知事件。

不负责：PTY 创建与终端渲染（终端面板）、Hook 配置文件编辑（Hooks 域）、导航树视图状态与恢复编排（导航域）、设置页控件（设置中心）、Runtime API 传输（Runtime v2）。

### 决策与红线

| 规则 | 原因 | 边界 |
|---|---|---|
| `AgentKind` 是封闭身份集；能力用单张描述表声明，禁止散落 `match` | slug 跨 Hook、设置、恢复、目录、品牌资产共用；双表必然漂移 | 新增 CLI = 枚举变体 + 描述表一行 + 必要解析器/安装器登记；编译期守卫表长与 `ALL` 等长 |
| 能力按已验证事实声明；未验证的 CLI 不猜测恢复/分叉/扫描 | 错误命令会注入终端；伪造能力比不支持更危险 | 不支持时返回 `None`/禁用态并说明原因 |
| 会话 ID 先做形状校验再拼命令；模板只允许一个 `{session_id}` 占位 | session id 来自不可信 hook/文件输入 | 非法 ID 返回 `None`，不引号硬拼、不加额外文本 |
| 钩子管道是生命周期事件唯一权威通道；statusline 走独立用量通道 | 轮询丢失窗口；statusline 高频且非生命周期 | 两个通道各自有界；不解析正文判成败 |
| 历史扫描只读文件头部；不复制对话正文 | 会话文件可达数十 MB；隐私边界 | 头部预算 64 KiB；标题只在最终展示集合补齐；注册表不存 transcript |
| 归属按词法路径最深前缀；扫描结果按来源独立健康 | 目录暂不可访问不应改变归属；单来源失败不应清空其他来源 | 空/相对/无匹配进入未归属；失败保留旧数据并标记过期 |
| 套餐余量只读 user 层 settings 的 env 段，不触凭据库；DTO 无 token 字段 | 凭据域单点与类型层防线 | 失败保留旧值；错误不冒泡为 UI 阻塞 |
| 系统通知只在窗口失焦时发送；类别由能力声明；按事件键去重 | 前台不打扰；避免同类风暴 | 去重缓存上限 200，裁剪为最近 100；任务栏闪烁由 `taskbar_flash` 门控 |
| 进程身份使用 `pid + 创建时间` | PID 会复用；仅 pid 会把旧 agent 身份继承给新进程 | 创建时间不可得时保留边并标记不确定 |

### 模块树

```text
slterm_agents/                    // core，无 GPUI
  src/
    kind.rs                       // AgentKind 封闭枚举与 slug/显示名
    registry.rs                   // AgentDescriptor 单一能力表
    commands.rs                   // CommandTemplate / SessionId / resume/fork/start
    aliases.rs                    // 内置命令与用户别名合并、冲突校验
    brand.rs                      // 内置品牌资产索引
    capabilities.rs               // Hook/会话/进程/通知/用量能力位
    sessions/
      mod.rs                      // SessionCatalog 门面与健康模型
      record.rs                   // SessionRecord / SessionKey / RunState
      provider.rs                 // SessionProvider trait
      claude.rs                   // ~/.claude/projects JSONL
      codex.rs                    // ~/.codex/sessions rollout JSONL
      registry.rs                 // hook 会话注册表合并
      attribution.rs              // 词法最深前缀归属/未归属
      delete.rs                   // 隔离删除、撤销与永久化
    usage.rs                      // ContextUsage 模型与状态槽
    hooks_caps.rs                 // Hook 能力与安装器标识（配置编辑归 Hooks 域）
    test_support/                 // FakeAgentCli / 会话夹具 / 通知记录器
slterm_app/src/integrations/agent_cli/
  mod.rs                          // 组合根
  scan_service.rs                 // 扫描调度与来源健康聚合
  hook_ingress.rs                 // 命名管道接管、排序与事件分发
  process_identity.rs             // 进程快照解析与 hook 路由佐证
  usage_watch.rs                  // statusline 用量信号消费
  notifications.rs                // 四类系统通知投影
  plan_balance/                   // 现有 plan_balance 迁入组合层
```

### AgentKind 能力注册表

```rust
pub struct AgentDescriptor {
    pub kind: AgentKind,
    pub slug: &'static str,
    pub display_name: &'static str,
    pub brand: BrandAsset,                  // None = 无品牌资产，只显示名称
    pub commands: &'static [&'static str],  // 内置可执行名（含兼容名）
    pub start: Option<CommandTemplate>,
    pub resume: Option<CommandTemplate>,
    pub fork: Option<CommandTemplate>,
    pub hooks: HookIntegration,
    pub sessions: Option<SessionIntegration>,
    pub global_dirs: &'static [GlobalDir],
    pub process: ProcessMatch,
    pub notifications: NotificationCaps,
    pub usage: UsageKind,
}

pub enum BrandAsset {
    Svg { path: &'static str }, // assets/cli-svg/<slug>.svg，构建期索引
    None,
}

pub enum HookIntegration {
    None,
    Notify { installer: InstallerId },
    Native { installer: InstallerId, catalog: Option<CatalogId> },
    Plugin { installer: InstallerId },
}

pub struct SessionIntegration {
    pub roots: &'static [SessionRoot],
    pub format: SessionFormat,     // ClaudeJsonl | CodexRollout | ...
    pub parser: SessionParserId,
}

pub enum SessionRoot {
    Home(&'static str),                  // 相对 home
    Env { var: &'static str, suffix: &'static str },
}

pub struct GlobalDir { pub relative_to_home: &'static str, pub writable: bool }

pub struct NotificationCaps {
    pub turn_done: bool,
    pub waiting_input: bool,
    pub permission_request: bool,
    pub error: bool,
}
```

- 描述表按 `AgentKind::ALL` 顺序静态声明；测试校验 slug 唯一、显示名非空、命令名不跨 CLI 冲突、`hooks` 与安装器登记一致。
- 品牌资产由构建脚本扫描 `assets/cli-svg/` 生成长度为 `AgentKind::ALL.len()` 的索引；缺失即 `BrandAsset::None`，消费方不按 slug 猜测路径。
- 全局目录只声明静态部分；设置中心的自定义 Agent 节点由 Agent 基础配置持久化，并在构造允许根时与本表合并。目录职责分工保持：目录表管沙箱放行，AgentKind 管身份，屏幕规则管识别。
- 通知能力与 Hook 能力分离：CLI 可以没有 Hook 但仍有 OSC/BEL 通知来源；是否产生系统通知由能力位决定。

### 命令识别、启动、恢复与分叉

```rust
pub struct SessionId(String); // 1..=64 字节，[A-Za-z0-9._-]

pub struct CommandTemplate {
    pub program: &'static str,
    pub template: &'static str, // 仅含一个 {session_id}
}

pub struct CommandRegistry { /* descriptor commands + 用户别名，大小写不敏感 */ }

impl CommandRegistry {
    pub fn build(settings: &AliasSettings) -> Result<Self, AliasConflict>;
    pub fn parse_command_line(&self, command: &str) -> Option<AgentKind>;
    pub fn builtin_commands(&self) -> impl Iterator<Item = (&'static str, AgentKind)>;
    pub fn aliases_for(&self, kind: AgentKind) -> &[String];
}

impl AgentDescriptor {
    pub fn resume_command(&self, id: &SessionId) -> Option<String>;
    pub fn fork_command(&self, id: &SessionId) -> Option<String>;
    pub fn start_command(&self) -> Option<String>;
}
```

- 命令识别：取首 token，剥掉 `&`、环境变量赋值、`env`、常见包运行器（node/bun/npx/pnpm/yarn/python/uv 等）与 `.exe/.cmd/.bat/.ps1` 后缀后匹配；用户别名参与同一命名空间。
- 别名冲突在任何写入生效前返回结构化冲突：本 CLI 内置命令、其他 CLI 内置命令、其他 CLI 别名；Windows 不区分大小写。别名添加成功后内存即刻生效，不等待保存。
- 恢复/分叉命令只覆盖实测语法；Codex 的 session id 取 rollout 文件名末尾 UUID，不接受任意文件名片段。Claude 无 session id 时允许 `claude --continue` 仅用于终端恢复路径，不用于历史会话行。
- 恢复注入前调用方再次校验 `SessionId`；命令是单行文本，注入通道由终端提供，不在本域执行。

### Hook 摄取与 statusline 用量

**生命周期事件摄取**

- `slterm_hook` 保持零依赖小进程；宿主侧 `HookIngress` 接管命名管道，解析信封与载荷，输出 `AiHookEvent`。
- 事件身份与排序：流键 = `source + session_id + panel_id + agent_pid + bridge_instance`；同一流内按 provider 事件 id/桥序号/本地接收序去重排序；跨流不比较顺序。
- 路由：`SLTERM_PANEL_ID` 为主；管道客户端 pid 的祖先链为佐证。祖先链来自进程快照，达到最近的 CLI 可执行名匹配即认定 `agent_pid`；只把 pid 一致的事件归入该面板。
- 能力表沿用并按注册表扩展：lifecycle、attention_events、attention_context、background_tasks、bridge_sequence、serialized_delivery。未知来源事件可宽松落盘会话注册表，但恢复动作必须经 `AgentKind` 归一。
- Hook 事件到状态：SessionStart/PromptSubmit/ToolComplete/TurnDone/NeedsAttention/SessionEnd 映射到终端四态；回合结果只认 provider 显式元数据；后台任务未清时回合结束不置完成。

**statusline 用量**

```rust
pub struct ContextUsage {
    pub source: AgentKind,
    pub panel_id: PanelId,
    pub session_id: Option<String>,
    pub used_tokens: u64,
    pub window_tokens: u64,
    pub percent: Option<u8>,     // 缺失时由 used/window 计算
    pub updated_at_ms: u64,
}

pub enum UsageState { Unavailable, Fresh(ContextUsage), Stale(ContextUsage) }
```

- 注入的 statusline 桥接脚本在透传用户原 statusline 输出的同时，把宿主 stdin 中的上下文窗口信息节流写入信号目录的每面板 JSON 文件（原子写，上限 4 KiB）。
- 宿主 `usage_watch` 监听信号目录（不可用时 1 Hz 轮询兜底），解析失败静默丢弃并保留上一份有效值；会话结束或面板关闭清除该面板状态。
- 导航活跃会话行的迷你用量条消费 `UsageState`；无数据或过期显示 `--`/降级态。用量不是套餐余量，两者模型与刷新节奏独立。

### 历史会话扫描、恢复与删除

```rust
pub struct SessionKey { pub kind: AgentKind, pub id: String }

pub struct SessionRecord {
    pub key: SessionKey,
    pub title: Option<String>,
    pub cwd: Option<PathBuf>,       // 空/相对/不可解析 = 未归属
    pub file: Option<PathBuf>,
    pub modified: Option<SystemTime>,
    pub run_state: SessionRunState, // Running | Completed | Unknown
}

pub enum SessionRunState { Running, Completed, Unknown }

pub struct SessionSourceHealth {
    pub source: AgentKind,
    pub last_success: Option<SystemTime>,
    pub stale: bool,
    pub error: Option<SessionError>,
}

pub trait SessionProvider: Send + Sync {
    fn scan(&self, ctx: &ScanContext) -> Result<Vec<SessionRecord>, SessionError>;
    fn delete(&self, record: &SessionRecord) -> Result<DeletionRecord, SessionError>;
    fn restore(&self, record: &DeletionRecord) -> Result<(), SessionError>;
    fn finalize(&self, record: DeletionRecord);
}
```

- 扫描：每个来源独立后台执行；头部预算 64 KiB；Claude 解析 `summary` 与首条真人 user 消息，过滤注入块与 `isMeta`；Codex 从 session_meta 取 cwd、从首条 user 输入取标题；其他来源只有在描述表声明解析器时接入。扫描器排除应用会话隔离区路径。
- 会话文件的读取、移动与删除经 app 服务层的路径权威与原子操作执行（来源目录属于已登记 Agent 全局目录），不自行裸 `std::fs`；读取只取头部，删除/撤销使用应用会话隔离区移动；同卷优先原子改名，跨卷复制后删除。
- 合并：原生扫描结果与 hook 会话注册表按 `(kind, id)` 合并，较新 modified 胜出，空字段互补；注册表容量有界，不复制正文。
- 归属：按项目根词法归一化后取最深前缀；工作目录为空、相对路径或不可解析进入未归属；目录不可访问不改变词法归属。归属计算是纯函数，输入为项目根集合与记录，可在导航域单测。
- 运行状态：hook 注册表或活动会话能可靠关联时 `Running`；CLI 记录明确结束时 `Completed`；无法确认时 `Unknown`，不得显示为已完成。
- 扫描调度：后台任务注册表新增 `sessionHistoryScan`（默认 3s，合法 2–300s，默认启用）；只在导航树视图打开且任务启用时运行，视图关闭停止计时器；手动刷新始终可用。来源健康独立，部分失败保留旧记录并标记过期。
- 删除与撤销：删除事务把来源文件原子移入应用会话隔离区（同卷优先 rename，跨卷复制后删除），保存 hook 索引条目快照与原顺序键；行立即消失。撤销时按记录移回原路径并恢复索引与顺序。5 秒窗口结束、提示关闭、窗口关闭或应用退出后清除隔离文件，删除永久化；启动时清扫遗留隔离区（视为永久删除）。隔离区不可写或移动失败则删除失败，行保留并报错。运行中记录禁止删除；批量删除先过滤运行中记录。
- 恢复计划：`plan_resume(record) -> Result<ResumePlan, SessionError>` 返回命令行、工作目录、初始标题与目标项目根候选；项目创建、面板创建与注入等待归导航/终端域。

### 套餐余量

现有 `plan_balance` 语义整体保留，迁入组合层并显式化接口：

```rust
pub struct PlanSourceBalance {
    pub source_id: String,
    pub plan_id: String,
    pub frozen: bool,
    pub amount: Option<AmountInfo>,
    pub windows: Option<WindowsInfo>,
    pub updated_at: u64,
}

pub trait PlanBalanceQuery: Send + Sync {
    fn fetch(&self, source: &dyn PlanSource, token: &SecretBytes)
        -> Result<FetchOutcome, PlanError>;
}
```

- 来源解析只读 user 层 `~/.claude/settings.json` 的 `env` 段（Anthropic 兼容端点与 token）；不读 Windows 凭据管理器；token 不出凭据域，快照/DTO/推送载荷无 token 字段。
- 轮询由后台任务 `planBalance` 驱动（默认 10s，合法 10–3600s）；阻塞 HTTP 全程在 GPUI background executor，UI 线程零阻塞。快照经模块级 watch channel 推送；失败保留旧值、不推送、不冒泡为 UI 错误。
- 导航页脚刷新节流 5s；结果保留 30s；任务禁用/来源为空时页脚隐藏但保留最后快照；重新启用立即显示。
- 查询语义（URL 归一化、DeepSeek/Kimi 解析、冻结判定、5h/7d 窗口缺失规则）沿用现有实现与真实响应快照锚点，不在本次重建中改变。

### 进程快照与身份

```rust
pub struct ProcessIdentity { pub pid: u32, pub created: u64 }

pub struct ProcessRow {
    pub identity: ProcessIdentity,
    pub parent: Option<ProcessIdentity>,
    pub executable: PathBuf,
}

pub struct ProcessIndex { /* 单次快照 + 祖先链索引 */ }

impl ProcessIndex {
    pub fn snapshot() -> Result<Self, ProcessSnapshotError>;
    pub fn nearest_agent_ancestor(&self, from: u32) -> Option<(ProcessIdentity, AgentKind)>;
    pub fn tree(&self, root: u32) -> Vec<ProcessNode>;
}
```

- 快照 TTL 1s，按键为 `pid + 创建时间`；PID 复用不会命中旧身份。创建时间不可得时保留边并标记 `identity_uncertain`，只作为佐证不作唯一路由依据。
- 用途：Hook 管道客户端路由佐证、活动会话关联、`panel.procs` 的进程树、终端关闭守卫的前台进程信息。
- 匹配规则来自描述表 `ProcessMatch`：可执行名（含兼容名）与解释器启动形态；不读他人命令行，不做 WMI/PEB 扫描。

### 系统通知事件

```rust
pub enum AgentNotificationKind { TaskComplete, WaitingInput, PermissionRequest, Error }

pub struct AgentNotificationEvent {
    pub kind: AgentNotificationKind,
    pub agent: AgentKind,
    pub panel_id: PanelId,
    pub project_name: Option<String>,
    pub session_id: Option<String>,
    pub turn_key: Option<String>,   // 事件键：turn_id/event_id
    pub at: SystemTime,
}

pub trait NotificationSink { fn deliver(&self, event: AgentNotificationEvent); }
```

- 映射：成功的 TurnDone → 任务完成；等待输入的 NeedsAttention → 等待输入；权限请求/可确认的 attention → 权限请求；失败回合或显式错误 → 错误。CLI 能力位缺失的类别不产生通知。
- 门控：仅窗口失焦时发送系统通知与任务栏闪烁；窗口聚焦全部丢弃。所有类别在 `taskbar_flash` 开启时触发闪烁，闪烁是唯一回窗引导。
- 去重：事件键 = `(panel_id, kind, session_id, turn_key)`；缓存上限 200，超出裁剪为最近 100；同类同键在缓存内只通知一次。
- 正文 `<项目名> · <类别> · <时间>`；不含会话正文、选区、工具参数或凭据。系统通知无点击跳转能力，所有类别在 `taskbar_flash` 开启时以任务栏闪烁作为回窗引导。恢复失败等终端自有通知不走本通道。
- 通知权限首次需要时请求一次；未注册或没有状态事件能力的 CLI 不产生系统通知。

### 错误映射

| 错误 | 用户可见 | 恢复 |
|---|---|---|
| `UnsupportedResume` | 该 CLI 不支持恢复会话 | 禁用菜单项/双击提示；不创建面板 |
| `UnsupportedFork` | 不支持分支恢复 | 隐藏或禁用分支项 |
| `SessionDirectoryMissing` | 会话目录不存在 | 打开对话框；允许复制/删除 |
| `SessionFileUnreadable` | 无法读取会话记录 | 来源标记过期；保留旧数据 |
| `SessionDeleteFailed` | 删除会话失败 + 原因 | 行保留/恢复；可重试 |
| `AliasConflict` | 结构化冲突文案 | 输入保留；不写入 |
| `CommandInvalid` | 无法生成恢复命令 | 不写剪贴板、不注入 |
| `HookIngestInvalid` | 无用户提示（日志/诊断） | 事件丢弃；不影响终端 |
| `UsageInvalid` | 用量显示 `--` | 保留上一份有效值 |
| `PlanFetch` | 页脚警示 + 旧值 | 下次轮询/点击重试 |
| `ProcessSnapshot` | `panel.procs` 显示不可用 | 重试；hook 路由退回面板环境 |

### 复用、改造与新建

| 现有模块 | 处置 | 说明 |
|---|---|---|
| `slterm_app/src/ai_agents.rs` | 改造并迁移 | AgentKind、slug/显示名、aliases、resume/fork/start、屏幕规则直接复用；能力表从散落 match 收敛到描述表 |
| `slterm_settings::AgentHook` | 删除独立身份 | Hook 偏好键改由 `AgentKind` + 描述表派生；设置键名保持不变，避免第二套身份枚举与表长漂移 |
| `slterm_app/src/agent_dirs.rs` | 改造 | 静态目录表成为描述表字段；沙箱放行集合并入 `AccessPolicy` |
| `slterm_app/src/ai_sessions.rs` | 改造 | Claude/Codex 扫描、标题解析、注入块过滤、注册表合并复用；改为 provider trait + 来源健康 + 隔离删除 |
| `slterm_app/src/session.rs` | 部分复用 | `AgentSession` 安全恢复命令与 Codex rollout id 解析保留；旧 session schema 随旧布局废弃 |
| `slterm_app/src/ai_hook/event.rs`、`payload.rs`、`protocol.rs`、`ordering.rs` | 直接复用 | 事件类型、能力位、流键、排序与门控逻辑保留 |
| `slterm_hook/` | 直接复用 | 零依赖桥、作用域闸门、有界转发与退出码契约不变；仅同步终端身份环境变量名 |
| `slterm_app/src/ai_hook` 安装器与 statusline 审查门 | 改造 | 安装/检视归 Hooks 域；本域只声明能力与消耗事件 |
| `slterm_app/src/plan_balance/` | 直接复用 | 查询、快照、watch channel、执行体整体迁入组合层 |
| `slterm_app/src/background_tasks.rs` | 改造 | 增加 `sessionHistoryScan` 任务；任务元数据增加“用户可配置/内部”可见性 |
| `slterm_app/src/platform/process_snapshot.rs`、`process_tree.rs` | 直接复用 | 快照与祖先链索引复用；补创建时间身份与 TTL |
| `slterm_app/src/notify.rs` | 改造 | 保留漏斗/节流/投递；增加四类 AI 通知映射与事件键去重，正文按工作区规格 |
| 新能力描述表/命令注册表/品牌索引 | 新建 | 单一事实源 |
| 新会话目录服务与 provider | 新建 | 多来源扫描、健康、隔离删除与恢复计划 |
| 新 statusline 用量通道 | 新建 | 桥接脚本 + 信号目录 + 状态槽（脚本与注入归 Hooks 域） |
| 新假 CLI 适配器与确定性夹具 | 新建 | 测试不依赖真实 CLI |

### 验收与测试接缝

- 能力表：slug 唯一、表与枚举等长、品牌索引覆盖、命令名不冲突、不支持能力返回 `None`。
- 命令：别名大小写与跨 CLI 冲突矩阵；包运行器/环境变量/后缀识别；非法 session id 拒绝；恢复/分叉模板逐 CLI 钉值。
- 会话：头部预算、截断行容错、注入块过滤、Codex cwd/标题、注册表合并、最深前缀归属、未归属、来源部分失败、隔离删除/撤销/永久化、遗留隔离区清扫。
- Hook：管道载荷解析、流键、去重排序、客户端 pid 祖先链路由、未知来源宽松落盘/严格恢复。
- 用量与余量：状态文件原子写/损坏丢弃/过期；余量成功/失败/plan 切换/节流/保留旧值；DTO 键集合无 token。
- 通知：四类映射、失焦门控、去重上限、正文格式、类别能力缺失不发送。
- 假 CLI：`FakeAgentCli` 提供脚本化会话、hook 事件、用量、恢复命令与进程树；`AgentHarness` 注入假时钟、临时会话目录与通知记录器，测试全确定性且不触真实 home。
