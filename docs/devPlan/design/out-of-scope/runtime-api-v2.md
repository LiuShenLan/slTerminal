# Runtime API v2 迁移

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 职责：定义 Runtime API v2 的资源模型、稳定身份、传输与鉴权、发现、命令语义、并发/幂等、CLI/env、随包 skill、审计与 conformance；不保留 v1 兼容层。

### 资源模型与快照

```rust
pub struct RuntimeSnapshotV2 {
    pub revision: u64,
    pub app_version: String,
    pub protocol_version: u16,          // 2
    pub active_project_id: Option<ProjectId>,
    pub projects: Vec<RuntimeProject>,
}

pub struct RuntimeProject {
    pub id: ProjectId,
    pub root: PathBuf,
    pub display_name: String,
    pub active: bool,
    pub focused_group_id: Option<PanelGroupId>,
    pub groups: Vec<RuntimeGroup>,
    pub layout: LayoutNode,             // Group(id) | Split(direction, ratio, first, second)
}

pub struct RuntimeGroup {
    pub id: PanelGroupId,
    pub focused: bool,
    pub active_panel_id: Option<PanelId>,
    pub panels: Vec<RuntimePanel>,
}

pub struct RuntimePanel {
    pub id: PanelId,
    pub kind: PanelKind,                // terminal/editor/markdown/diff/gitshow/settings
    pub title: String,
    pub dirty: bool,
    pub resource_path: Option<PathBuf>, // 文件型面板的项目内/根外只读路径
    pub params: serde_json::Value,      // 面板参数只读投影，不承载内容
    pub terminal: Option<RuntimeTerminal>,
}

pub struct RuntimeTerminal {
    pub cwd: String,
    pub branch: String,
    pub running_program: Option<String>,
    pub task_state: RuntimeTaskState,
    pub state_change_seq: u64,
    pub agent: Option<RuntimeAgent>,
    pub active_run: Option<RuntimePaneRun>,
    pub last_run: Option<RuntimeRunOutcome>,
}
```

- 快照包含全部项目的布局；非活跃项目 `active=false`、其面板组 `focused=false`，但仍在快照中。`layout` 与 `groups` 同时提供结构与索引，二者由同一模型投影，不允许独立构造。
- `revision` 单调递增；语义内容不变不增长、不重发事件。`state_change_seq` 只在终端任务状态跃迁时增长，等待方只承认 `seq > after_seq`。
- 面板类型是封闭枚举；未知类型在恢复时丢弃并聚合警告，不进入快照。

### 稳定 ID

- 内部使用 `ProjectId(u64)`、`PanelGroupId(u64)`、`PanelId(u64)`；应用全局单调分配并持久化计数器，删除项目/面板后永不复用。
- 外部序列化为带前缀的不透明字符串：`p-<u64>`、`g-<u64>`、`l-<u64>`。前缀防止跨命名空间误用；解析严格校验前缀与十进制数字。
- `ProjectId` 在根目录重定位、显示名重命名、顺序调整后保持不变；根路径是唯一性约束，不是身份。
- 面板组与面板 ID 在项目布局内稳定，拖拽、排序、拆分、合并不改变 ID；关闭后 ID 退役。
- Runtime 不暴露窗口 ID；单窗口下所有目标由项目/组/面板定位。

### 传输、鉴权与发现

- 协议名 `slterm.runtime`，版本 `2`，本机 `127.0.0.1` TCP，JSON Lines；单请求连接写一行读一行，订阅连接在首行响应后持续推送事件。
- 请求信封：
  ```rust
  struct ApiRequestV2 {
      protocol: String,        // "slterm.runtime"
      version: u16,            // 2
      id: String,              // "<pid>-<单调序>"
      token: String,           // 128 位 hex
      mutation_id: Option<String>,
      method: String,
      params: serde_json::Value,
  }
  ```
  响应信封含 `ok`、`replayed`、`result`、`error`；`result` 显式 null 与字段缺失区分。未知字段拒绝。
- 限额：请求首行 128 KiB；同时在线客户端 64；读写超时 5 秒；控制派发 30 秒；prompt/paste/argv 单次 32 KiB；读取默认 120 行、上限 2000 行；exec 输出每条流默认 1 MiB、上限 16 MiB；订阅通道有界 16，消费过慢即断开，由客户端按 `since_revision` 重订阅。
- 鉴权：token 缺失或不符静默丢弃，先于参数解析；日志不插值 token。端点记录 `slterm.runtime/2 <port> <token> <pid>`，原子写。
- 发现优先级：进程内子端点 > 显式 `SLTERM_RUNTIME_ENDPOINT`（只接受 v2 格式）> `state/runtime.port`。显式值非法即不可达，不回落。
- 单实例 owner 锁独立于端点；拿不到锁的进程不得 bind 或发布端点。旧 `runtime.port` 永不读取。

### 命令语义

| 方法 | 目标 | 语义 | 幂等/并发 |
|---|---|---|---|
| `runtime.describe` | 无 | 版本、能力、features、限额、schema 路径、环境变量契约 | 只读 |
| `runtime.snapshot` | 无 | 当前完整快照 | 只读 |
| `events.subscribe` | 无 | `since_revision` 断点续传；推送 `runtime.snapshot` 与 `panel.lifecycle` | 长连接 |
| `project.open` | 根路径 | 规范化根路径；已存在则激活，否则创建项目并激活 | 根路径自然幂等 |
| `project.activate` / `project.rename` | project_id | 激活或改名；改名允许重名 | 需要 mutation_id |
| `project.remove` | project_id | 需要 `force` 覆盖脏面板/运行中终端；否则 `confirmation_required` | 需要 mutation_id |
| `group.split` | panel_id/group_id + 方向 | 在目标面板组旁创建新组，并把指定面板移入；空间不足拒绝 | 需要 mutation_id |
| `group.resize` | group_id + ratio | 调整直接父分划比例，夹取 0.05–0.95 | 需要 mutation_id |
| `group.focus` / `group.close` | group_id | 聚焦；关闭空组或 `force` 关闭含面板组 | 需要 mutation_id |
| `panel.open` | project_id、可选 group_id、kind、params | 按落点规则打开；终端不去重，编辑器/Markdown/Diff/GitShow/设置按自然键去重 | 自然幂等；并发单飞 |
| `panel.close` | panel_id、force | 脏面板或前台进程默认 `confirmation_required`；`force` 丢弃/终止 | 需要 mutation_id |
| `panel.focus` / `panel.move` | panel_id、目标组/索引 | 聚焦；同项目内移动；跨项目拒绝 | 需要 mutation_id |
| `panel.set_title` | panel_id + title | 仅终端；空串恢复自动标题；运行中 CLI 禁止改名 | 需要 mutation_id |
| `terminal.prompt` / `terminal.paste` | panel_id + text + submit | 单行文本注入或括号粘贴；返回提交后快照与状态基线 | 必须 mutation_id |
| `terminal.send_key` | panel_id + key/modifiers/repeat | 仅受支持的命名控制键；字母键必须带 control | 必须 mutation_id |
| `terminal.read` | panel_id + lines | 读取回滚尾部；UTF-8 边界；超限整行递减 | 只读 |
| `terminal.run` | panel_id + command + wait/timeout | 注入命令，等待 OSC 133 完成；退出码不可用不伪造 0 | 必须 mutation_id |
| `terminal.exec` | panel_id + argv + timeout | 独立非 TTY 执行，不进入 shell 历史或网格 | 必须 mutation_id |
| `terminal.wait` | panel_id + after_seq + state | 等待状态跃迁；缺有效基线报错 | 只读长请求 |
| `terminal.procs` | panel_id | 本地进程树 | 只读 |
| `agents.list` / `agent.get` | 可选过滤 | agent 身份、generation、任务状态 | 只读 |
| `agent.prompt` / `agent.paste` / `agent.read` / `agent.wait` | agent + generation | 绑定 generation，防会话被替换后误投 | 提交类要求 mutation_id |
| `agent.delegate` | agent + generation + task | 目标完成后回传有界结果给来源面板 | 必须 mutation_id |
| `agent.fork` | 来源面板/cwd + 名称 | 隔离 worktree 事务；脏源默认拒绝 | 必须 mutation_id |
| `runtime.orchestrate` | 步骤数组 | 封闭 typed 步骤：open_project/focus/open_panel/split/prompt/run/agent_launch；步骤引用只能向后引用 | 必须 mutation_id |
| `app.handover` | 类型化载荷 | 第二启动移交：聚焦、打开项目、打开文件、新建终端、激活项目 | 必须 mutation_id |

- 文件型 `panel.open` 只接受项目根、项目根所在仓库实际根或已登记 Agent 目录内的路径；越界返回 `path_not_allowed`。不弹出系统对话框。
- `terminal.run` 的完成判定只认 OSC 133 CommandDone 退出码；不可用返回 `unavailable` 与原因，不伪造成功。
- 关闭/移除操作的目标集合与影响对象在错误 `details` 中列出（面板 ID、标题、是否有前台进程），供客户端展示确认。

错误码固定为封闭集，客户端按 `error.code` 分支：

| 错误码 | 触发 |
|---|---|
| `invalid_request` | 信封 JSON/字段非法 |
| `protocol_version_mismatch` | 协议名或版本不符；details 返回支持版本 |
| `method_not_found` | 方法不在路由表 |
| `invalid_params` | 参数缺失、类型不符、超限 |
| `invalid_id` | 资源 ID 前缀或十进制格式非法 |
| `path_not_allowed` | 文件路径不在允许根内 |
| `target_not_found` | 项目/组/面板/agent 不存在或已关闭 |
| `confirmation_required` | 脏面板、运行中终端、项目移除需要显式 force |
| `dirty_panels` | 批量关闭中列出未保存面板 |
| `pty_limit_reached` | 达到会话上限 |
| `idempotency_conflict` | 同一 mutation_id 携带不同参数 |
| `payload_too_large` | 请求或读取载荷超限 |
| `runtime_timeout` | UI 派发逾期；details 标记 `outcome_unknown` |
| `submission_outcome_unknown` | 提交类请求传输失败，输入可能已送达 |
| `runtime_unavailable` | 无端点或连接失败 |
| `shutting_down` | 关闭流程已开始，拒绝新写命令 |
| `internal` | 未分类内部失败；不回显敏感内容 |

### 并发与幂等

- 所有写命令经壳事件队列在 GPUI 前台串行执行；读命令在 transport 线程直接读 hub 快照。同一面板的写命令按到达顺序执行。
- 变更类命令必须携带 `mutation_id`；服务端以 `(mutation_id, method, 规范化 params 摘要)` 为键保留有界结果缓存。完全重复的请求返回原结果并置 `replayed=true`，不重复执行；同 `mutation_id` 不同参数返回 `idempotency_conflict`。缓存溢出后旧 ID 可能重新执行，客户端不得无限期重试同一 ID。
- 提交类命令（prompt/paste/run、agent.prompt/paste/delegate）响应携带提交后快照与 `state_change_seq`；客户端 `wait` 必须使用该基线。传输失败改写为 `submission_outcome_unknown`，提示先读状态再决定是否重试。
- 关闭流程开始后不再接受写命令，返回 `shutting_down`；进行中的命令按 30 秒上限回执，超时返回 `runtime_timeout` 并在 `details` 标记 `outcome_unknown`。
- 项目/面板移除、窗口关闭、脏面板确认共用壳关闭守卫；Runtime 不绕过守卫，`force` 是显式覆盖。

### CLI、env 与随包 skill

- 同 exe 双人格保留：无子命令 = GUI；有子命令 = CLI 客户端，走同一协议、同一信封、同一身份校验，不进程内直调处理器。
- 环境契约：`TERM_PROGRAM=slterm`、`SLTERM_CLI`（exe 绝对路径）、`SLTERM_BIN_DIR`（并前置 `PATH`）、`SLTERM_PANEL_ID`、`SLTERM_PROJECT_ID`、`SLTERM_PANEL_GROUP_ID`、`SLTERM_RUNTIME_ENDPOINT`（v2 格式）。不再注入 `SLTERM_PANE_ID`。
- `slterm env`：离线可答；输出所在面板/项目/组、CLI 路径、端点可达性、自身终端状态与完整命令示例。
- `slterm ctl`：完整方法面，支持 `--params`/`--file`、`--timeout-ms`、`--pretty`、`--mutation-id`、`subscribe` 流式输出。
- 资源别名：`slterm project ...`、`slterm group ...`、`slterm panel ...`、`slterm agent ...`；不保留 `window`/`tab`/`pane` 别名。
- 退出码：成功 0；运行期拒绝 1；用法错误 2；运行时不可达/超时 3；输出仍为 JSON 信封，脚本可同时按退出码与 `error.code` 判定。
- 随包 skill `assets/skills/slterm-runtime/` 重写：先 `slterm env`，按项目/组/面板寻址；一次 `runtime.orchestrate` 完成确定性布局与派发；不扫进程、不猜端口、不读源码。`SKILL.md` 与 `agents/openai.yaml` 同步更新，投放指纹与 `.slterm-managed` 归属机制保留；用户改过的目录冲突时不覆盖。

### 审计与 conformance

- 审计只记录写命令：`operation=runtime.<method>`、`request_id`、`mutation_id`、项目/组/面板 ID、结果；prompt/paste 只记字节数，exec 只记 argv 长度，不记正文、argv 内容或 token。
- `runtime.describe` 的 capabilities、路由表、schema 方法枚举三向一致，由启动自检与测试锁定。
- 随包契约文件改为 `assets/runtime-api-v2.schema.json` 与 `assets/runtime-control-api-v2.md`；describe 返回其路径。
- conformance 覆盖：信封与未知字段、鉴权静默丢弃、版本不匹配、发现优先级、稳定 ID、全部方法参数校验、幂等重放与冲突、等待基线、订阅断点、脏关闭、项目移除、handover。真实 GUI 进程用例条件跳过并登记豁免。

### 不保留 v1

- 删除 `legacy-attach` feature、ATTACH/PING 首行协议、`window.*`/`tab.*`/`pane.*` 方法、v1 schema/docs/skill 与 v1 `RuntimeSnapshot` 投影。
- 版本不匹配一律 `protocol_version_mismatch`，不做参数降级或方法翻译；旧 CLI 无法驱动新实例，新 CLI 不会连接旧端点。
- 旧 `runtime.port` 与旧 `SLTERM_RUNTIME_ENDPOINT` 格式不读；旧 schema 资产从包清单移除。

### 依赖里程碑

- 项目与布局模型、工作区壳、面板宿主注册点。
- 单实例与窗口服务、持久化协调器。
- 终端面板（`terminal.*` 与 `agent.*` 的执行落点）。
- 设置中心应用页（展示 Runtime 状态与文档入口）。

### 迁移顺序

1. 在新 core 领域 crate 定义 ID、快照与布局投影；提供稳定 ID 分配与持久化。
2. 实现 v2 hub 投影、事件 revision、订阅与只读方法。
3. 实现传输、端点发现、owner 锁、鉴权与写命令派发；先接通 `project/panel/group` 生命周期。
4. 接通终端与 agent 方法、orchestrate 与 handover。
5. 重写 CLI、env 契约、skill、schema、文档与审计。
6. 跑 conformance 与真实进程用例；通过后删除 v1 代码、旧 schema、旧 skill 与旧别名。

### 测试与验收

- 单元：信封校验、未知字段拒绝、token 静默丢弃、版本不匹配、限额、端点格式与发现优先级。
- 模型：ID 在重命名/重定位/排序/重启后不变；关闭后不复用；布局投影与显式 groups 一致。
- 命令：资源生命周期、落点规则、去重键、路径白名单、脏关闭、`force` 覆盖、wait 基线、orchestrate 向后引用。
- 幂等：重复 mutation 返回相同结果并 `replayed=true`；参数不同 `idempotency_conflict`；缓存溢出语义。
- 事件：revision 单调、since_revision 续传、慢订阅断开、生命周期事件。
- 外部：conformance 对真实 GUI 进程、双进程 handover、skill 投放冲突与重装。
- 验收以客户端可观察响应、快照与事件为准，不以内部日志文本为准。

### 不可自动化面

- 真实 GUI 双进程 handover、窗口还原与焦点；以条件跳过用例与构建产物人工 smoke 兜底。
- 外部 AI CLI 的 hook 事件、agent 会话身份与 delegate 回传。
- 本机防火墙、安全软件对 loopback 监听与命名管道的干预。
- 其他本机用户对 token 的访问边界无法在单用户测试中完整模拟。
