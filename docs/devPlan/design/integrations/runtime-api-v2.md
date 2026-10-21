# Runtime API v2

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

### 职责与边界

本域负责：Runtime API v2 资源模型（project / panel-group / panel）、稳定 ID、协议信封、传输与鉴权、端点发现、命令语义、并发与幂等、`slterm ctl`/`slterm env` 客户端、随包 skill、审计与 conformance。

不负责：项目管理与布局的业务规则（工作区域）、终端 PTY（终端面板）、Git 执行体（文件系统 Git 域）、Agent CLI 能力（Agent 集成域）。Runtime 只暴露资源与已存在的领域能力，不复制领域语义。

### 决策与红线

| 规则 | 原因 | 边界 |
|---|---|---|
| v2 资源模型固定为 project → panel-group → panel；不保留 window/tab/pane 资源 | 目标态只有单窗口、项目布局、面板组、面板；兼容层会让两套身份长期并存 | 不提供 `window.*` 方法；快照只保留 `window.focused`，不暴露窗口 ID |
| 稳定 ID 与可复用编号分离 | 面板编号会重排/复用；资源身份必须跨重排、重定位、重启稳定 | 终端编号仍是项目级展示编号，不进入资源 ID |
| 协议版本固定 2；版本不匹配显式报错；无 v1 路由 | 工作区模型已破坏性重建，v1 语义无法映射 | 端口文件、schema、skill、env 变量同步升级；旧客户端得到 `protocol_version_mismatch` |
| 协议先行、方法增量 | 壳的移交与早期 AI CLI 集成需要控制面；完整方法依赖各域实现 | 协议 v2 版本冻结；project/panel/terminal 先交付，git/agent/orchestrate 随域追加；只追加方法，不改旧语义；交付节奏见 `plan.md` |
| 写命令统一经 UI owner 单队列派发；读命令直接读不可变快照 | 布局/面板操作必须在 UI 线程串行；读不得阻塞 UI | 长等待（run/wait/exec）不占 UI 队列 |
| 变更可选携带 `expected_revision`；不匹配 `revision_conflict` | 外部客户端可能基于旧快照执行破坏性操作 | 不强制所有读；破坏性/移动操作建议携带 |
| 写方法可选 `mutation_id`；服务端按 `(mutation_id, method, params_hash)` 缓存响应 | 网络重试不应产生重复面板/分组；请求 `id` 只作关联 | 缓存有界、短 TTL；同 `mutation_id` 不同参数返回 `idempotency_conflict`；跨进程不共享 |
| 关闭/移除走现有脏状态与关闭守卫；`force` 才丢弃 | Runtime 不得绕过用户数据保护 | 返回 `dirty_panels` 详情；`force` 明确记录审计 |
| 稳定 ID 永不复用；关闭后短期墓碑用于幂等关闭 | 复用 ID 会把旧请求作用到新对象 | 墓碑有界；超出后 `resource_not_found` |
| 审计只记操作/资源 ID/路径/结果/错误码 | 控制面可执行敏感操作，需可追溯 | 不记 token、提示词、终端内容、文件内容 |

### 资源模型与稳定 ID

```rust
// 线缆身份：不透明字符串，形如 "project:<n>" / "group:<n>" / "panel:<n>"。
// 协议 crate 只定义 ResourceId 的解析、格式化与错误；领域 newtype 与分配器归 slterm_workspace::ids。
pub struct ResourceId(String);

// 线缆枚举，与领域 PanelKind 一一映射；未知值拒绝。
pub enum WirePanelKind { Terminal, Editor, Markdown, Diff, GitShow, Settings }

pub struct ProjectResource {
    pub id: ResourceId,
    pub name: String,
    pub root: String,              // 词法归一化显示路径
    pub active: bool,
    pub visible: bool,
    pub layout: Option<PanelGroupLayout>,
    pub groups: Vec<PanelGroupResource>,
}

pub struct PanelGroupResource {
    pub id: ResourceId,
    pub focused: bool,
    pub active_panel_id: Option<ResourceId>,
    pub panels: Vec<PanelResource>,
}

pub struct PanelResource {
    pub id: ResourceId,
    pub kind: WirePanelKind,
    pub title: String,
    pub active: bool,
    pub focused: bool,
    pub dirty: bool,
    pub document: Option<DocumentResource>,
    pub terminal: Option<TerminalResource>,
}

pub struct DocumentResource {
    pub path: String,
    pub read_only: bool,
}

pub enum PanelGroupLayout {
    Group(ResourceId),
    Split { direction: SplitDirection, ratio: f32, first: Box<PanelGroupLayout>, second: Box<PanelGroupLayout> },
}
```

- 线缆字段一律使用 `ResourceId`；下文 `ProjectId`/`PanelGroupId`/`PanelId` 只指其语义，不是协议 crate 的类型。领域 newtype、分配器与持久化归 `slterm_workspace::ids`。
- 项目 ID 语义：项目创建时生成并随项目持久化；根目录重定位、重命名、排序、重启均保持不变。根路径唯一仍是不变量，但不再是身份。
- 面板组 ID 语义：首个面板组创建或拆分时生成；持久化于项目布局；面板组空置被移除后 ID 退役，不复用。
- 面板 ID 语义：面板创建时生成；同项目内移动、页签重排、面板组变化、重启均保持不变；关闭后退役。
- `WirePanelKind` 线缆枚举是封闭集，与工作区面板类型单点登记一一映射；未知值拒绝。
- 终端展示编号 `terminal_number` 是项目级编号，随状态库 `project_terminals`/`TerminalBook` 持久化、关闭不复用，但不是资源身份；Runtime 不提供按编号寻址。
- 资源定位只接受稳定 ID；不再接受 `tab_index`、`pane_id` 或索引路径。索引只作为快照里的顺序字段。
- 快照：

```json
{
  "revision": 42,
  "process_id": 1234,
  "app_version": "x.y.z",
  "protocol_version": 2,
  "active_project_id": "project:1",
  "window": { "focused": true },
  "projects": [
    {
      "id": "project:1",
      "name": "slTerminal",
      "root": "D:/code/slTerminal",
      "active": true,
      "visible": true,
      "layout": { "type": "split", "direction": "left_right", "ratio": 0.5,
                  "first": { "type": "group", "id": "group:2" },
                  "second": { "type": "group", "id": "group:3" } },
      "groups": [
        { "id": "group:2", "focused": true, "active_panel_id": "panel:4",
          "panels": [
            { "id": "panel:4", "kind": "terminal", "title": "terminal-0",
              "active": true, "focused": true, "dirty": false,
              "document": null,
              "terminal": { "cwd": "D:/code/slTerminal", "branch": "main",
                            "number": 0, "agent": null, "task_state": "idle" } }
          ] }
      ]
    }
  ],
  "lifecycles": [
    { "sequence": 7, "panel_id": "panel:4", "event": "closed" }
  ]
}
```

- `revision`：进程内单调；仅语义变化才递增；重复发布相同快照不递增、不推送。
- `lifecycles`：面板关闭/退出墓碑，有界；首个终态事件即原因，后续不覆盖。
- `window` 只含 `focused`，不暴露窗口 ID、句柄或平台对象。
- `PanelResource.document` 只含 `path`/`read_only`；不暴露原始面板 `params`。
- 冲突快照是瞬态 Diff 面板：有 `PanelId`、不持久化、不进入快照与布局，重启消失。
- 每资源 `state_change_seq`（终端/Agent 状态）用于等待基线，语义与 v1 的 pane 跃迁计数一致，但挂在 `panel` 上。

关键命令类型：

```rust
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub process_id: u32,
    pub app_version: String,
    pub protocol_version: u16,
    pub active_project_id: Option<ResourceId>,
    pub window: RuntimeWindowState,
    pub projects: Vec<ProjectResource>,
    pub lifecycles: Vec<PanelLifecycle>,
}

pub struct RuntimeWindowState {
    pub focused: bool,
}

pub struct CreateProjectParams {
    pub root: String,
    pub name: Option<String>,
    pub activate: bool,
    pub expected_revision: Option<u64>,
}

pub struct CreatePanelParams {
    pub project_id: ResourceId,
    pub group_id: Option<ResourceId>, // 缺省 = 项目聚焦组；无组则创建
    pub kind: WirePanelKind,
    pub activate: bool,
    pub terminal: Option<TerminalCreateParams>, // cwd/shell
    pub document: Option<DocumentOpenParams>,   // path/mode
    pub expected_revision: Option<u64>,
}

pub struct MutationResult {
    pub revision: u64,
    pub affected: Vec<ResourceRef>,
    pub warnings: Vec<RuntimeWarning>,
}

pub enum ResourceRef {
    Project(ResourceId),
    PanelGroup(ResourceId),
    Panel(ResourceId),
}
```

- 响应统一为 `MutationResult` + 方法专属结果；所有 ID 以字符串序列化，不暴露内部整数。
- `RuntimeWarning` 只携带类别与资源引用（如布局恢复丢弃未知面板），不携带文件内容。

### 传输、鉴权与发现

- 控制面：本机 `127.0.0.1` TCP，JSON Lines；首行一条请求，响应一行；`events.subscribe` 在响应后于同一连接持续推送事件。
- 信封：

```json
{"protocol":"slterm.runtime","version":2,"id":"<pid>-<seq>","token":"<token>","method":"project.list","params":{}}
{"protocol":"slterm.runtime","version":2,"id":"<pid>-<seq>","token":"<token>","mutation_id":"<optional>","method":"panel.create","params":{}}
{"protocol":"slterm.runtime","version":2,"id":"<pid>-<seq>","ok":true,"result":{}}
```

- 请求 `deny_unknown_fields`；响应 `ok/result/error` 互斥，`result:null` 与缺省 `result` 区分；响应按 `protocol/version/id` 逐字段校验。
- `mutation_id` 仅写方法可选；请求 `id` 只作关联，不参与幂等。
- token：128 位随机 hex；缺失/不符静默丢弃，不返回错误，避免未认证进程把错误当协议预言机。token 不进日志、审计、错误详情。
- 限额：请求体 128 KiB；单实例最多 64 个并发客户端；读写超时 5s；控制派发 30s（`terminal.exec` 按参数追加预算）；订阅通道有界 16；wait 上限 600s。
- 端点发现：进程内子端点（`SLTERM_RUNTIME_ENDPOINT` 注入）> 显式环境变量 > 本地根 `<local>/state/runtime.port`。端口文件内容 `slterm.runtime/2 <port> <token> <pid>`，原子写；显式端点非法时不回落端口文件。
- 端口文件版本前缀固定 `slterm.runtime/2`；读到其他版本视为不兼容实例，不接管、不静默降级。
- 单实例：启动先取本地根 lifetime 锁；已有实例时发送 v2 移交请求后退出，不创建第二窗口。移交载荷类型、校验与接收语义归 `platform-windows.md`；本协议只定义 `runtime.handover` 方法与信封。载荷用根路径定位，不传内部 ID；目标项目/路径不存在只提示，不自动创建。
- `runtime.describe` 返回：app_version、protocol、protocol_version、supported_versions、schema 相对路径、capabilities、features、env 约定；不返回 token。

### 命令语义

**读方法**

| 方法 | 参数 | 结果 |
|---|---|---|
| `runtime.describe` | 空 | 能力/版本/schema 信息 |
| `runtime.snapshot` | 空 | 完整快照 |
| `events.subscribe` | `since_revision?`、`types?` | 先响应，再按 revision 推送 `runtime.snapshot` |
| `project.list` | 空 | 项目摘要数组 |
| `project.get` | `project_id` | 项目资源 |
| `panel-group.get` | `group_id` | 面板组资源 |
| `panel.get` | `panel_id` | 面板资源 |
| `panel.read` | `panel_id`、`lines` | 终端尾部文本 |
| `panel.procs` | `panel_id` | 进程树 |
| `panel.wait` | `panel_id`、`after_seq?`、`timeout_ms` | 等待终端/Agent 状态稳定 |
| `agent.list` | `project_id?` | 仅 AI CLI 面板的身份与状态 |
| `agent.get` | `panel_id` 或 `agent_id` | Agent 身份/代次/会话 |
| `agent.wait` | `agent_id`、`generation?`、`after_seq?`、`timeout_ms` | 等待回合结束 |

**写方法**

| 方法 | 语义 | 幂等 |
|---|---|---|
| `project.create` | 按根路径创建项目；根已存在返回已有项目并激活 | 天然幂等（根路径唯一） |
| `project.activate` | 切换活跃项目 | 幂等 |
| `project.rename` | 改显示名，允许重名 | 同名幂等 |
| `project.move` | 调整项目顺序 | 同位置幂等 |
| `project.relocate` | 重定位根；保留项目 ID/布局/名称 | 同根幂等；冲突拒绝 |
| `project.remove` | 移除项目；默认受脏面板与运行中终端守卫，`force` 丢弃 | 关闭后墓碑期内幂等 |
| `panel-group.split` | 在项目布局中拆分；返回新组 ID | 非幂等，重试须复用 `mutation_id` |
| `panel-group.close` | 关闭组及其面板；受关闭守卫 | 墓碑期内幂等 |
| `panel-group.resize` | 设置相对父分屏占比 | 幂等（显式值） |
| `panel-group.focus` | 聚焦面板组/激活面板 | 幂等 |
| `panel.create` | 在指定组创建面板（终端/编辑器/Markdown/Diff/GitShow/设置中心） | 非幂等；项目根/路径去重规则由领域执行 |
| `panel.close` | 关闭面板；受关闭守卫 | 墓碑期内幂等 |
| `panel.move` | 同项目内移动面板/重排 | 同目标幂等 |
| `panel.activate` | 激活面板并聚焦 | 幂等 |
| `panel.rename` | 仅终端自定义标题；CLI 运行中拒绝 | 同名幂等 |
| `terminal.prompt` | 文本输入；`submit` 决定回车 | 非幂等（重复提交会重复输入） |
| `terminal.paste` | 多行粘贴；可选来源文件 | 非幂等 |
| `terminal.send_key` | 命名键白名单 | 非幂等 |
| `terminal.run` | 写入命令行并回车，可选等待 | 非幂等 |
| `terminal.exec` | 独立非 TTY argv 执行，不改 shell | 非幂等 |
| `terminal.wait` | `panel.wait` 的终端别名，仅接受终端面板 | 天然只读/幂等 |
| `git.status` / `git.diff` | 项目根对应仓库的只读快照/内容 | 幂等 |
| `git.rollback` / `git.unstage` | 需 `revision`；写前重跑 status 校验 | 非幂等（重复回滚同状态应返回已一致） |
| `agent.start` | 在面板启动 Agent；`kind` 必须可冷启动 | 非幂等 |
| `agent.fork` | 事务型 worktree + 新面板；返回 provenance | 非幂等；客户端重试须复用 `mutation_id` |
| `agent.delegate` | 将任务委派给目标 Agent，返回 request/task ID；不等待回合结束 | 非幂等；客户端重试须复用 `mutation_id` |
| `agent.prompt` / `agent.paste` / `agent.read` / `agent.wait` | 按 Agent 身份与 generation 绑定 | 读幂等；写非幂等 |
| `runtime.handover` | 第二实例移交：打开项目/文件、新建终端、聚焦 | 按载荷自然键幂等 |

- 参数约束：文本 32 KiB；名称 256 字节；路径 4096 字节且禁止 NUL；键重复上限 64；占比 0.05–0.95；行数 1–2000；exec 输出 1 MiB 默认、16 MiB 上限。
- `terminal.prompt`/`terminal.paste` 不接受 ANSI 控制注入；发送内容按终端输入路径编码。`terminal.send_key` 只接受命名键；可打印文本访问 `terminal.prompt`。
- `panel.create` 的终端工作目录遵循终端规格允许根；编辑器类面板按文件打开路由与允许根校验。
- `git.*` 参数从 v1 的 `pane_id + expected_cwd` 改为 `project_id + path`；服务端按项目根重查仓库根；写操作仍携带 status 摘要 `revision`，不一致返回 `git_stale`。
- 所有写命令的响应包含 `revision`（操作完成后的快照 revision）与受影响的资源 ID。

**错误码登记**

| 类别 | 错误码 |
|---|---|
| 信封 | `invalid_request`、`protocol_version_mismatch`、`request_too_large`、`method_not_found`、`invalid_params` |
| 资源 | `resource_not_found`、`ambiguous_target`、`revision_conflict`、`idempotency_conflict`、`invalid_state`、`action_failed` |
| 守卫 | `dirty_panels`、`running_terminals`、`close_blocked` |
| 项目 | `project_root_conflict`、`project_root_unavailable`、`project_relocate_failed` |
| 面板生命周期 | `panel_closed`、`panel_exited`、`subscriber_lagged` |
| Agent | `agent_exited`、`agent_replaced`、`agent_closed`、`agent_name_conflict`、`agent_identity_mismatch`、`agent_launch_unsupported`、`agent_resume_unsupported` |
| 执行 | `runtime_unavailable`、`runtime_timeout`、`invalid_runtime_response`、`submission_outcome_unknown`、`timeout`、`git_stale`、`git_target_changed` |

- 错误码集合与 schema、`runtime.describe`、文档登记集合三向一致；新增码必须在单一登记点添加并补 conformance。

### 并发与幂等

- UI owner 单队列：所有写命令按到达顺序串行执行；读命令在 transport 线程读 `RuntimeHub` 的不可变快照，不排队。
- 长操作：`terminal.run`/`terminal.exec`/`agent.*wait`/`panel.wait` 在写命令中只发起并观察，不持有 UI 队列超过派发时间；超时返回明确错误，不取消已启动的进程，除非方法显式声明。
- 乐观并发：写命令可选 `expected_revision`；服务端在派发前比较当前 revision，不一致返回 `revision_conflict` 并携带 `current_revision` 与 `current_snapshot` 摘要。`panel.close`、`project.remove`、`panel.move`、`panel-group.close` 建议强制携带。
- 请求重试：写方法可选 `mutation_id`；服务端按 `(mutation_id, method, params_hash)` 缓存最近响应（有界、短 TTL、按 token 作用域），命中直接返回原响应、不重复执行；同一 `mutation_id` 携带不同 method 或参数返回 `idempotency_conflict`。请求 `id` 只作关联，不参与幂等。自然幂等操作可省略 `mutation_id`；缓存溢出后旧 mutation 可能重新执行，客户端不得无限期重试同一 ID。
- 顺序保证：同一连接内请求顺序即执行顺序；跨连接按到达顺序进入单队列。订阅事件按 revision 单调；订阅方落后超过容量 16 时连接关闭并给出 `subscriber_lagged`，客户端重新订阅并带 `since_revision`。
- 墓碑：面板/分组/项目关闭后保留有界墓碑（含 `state_change_seq`），用于关闭幂等与等待方收尾；超出容量返回 `resource_not_found`。
- 关闭/移除的守卫：默认拒绝并返回 `dirty_panels`/`running_terminals` 列表；`force=true` 丢弃未保存修改并终止进程，审计记录 force 与影响资源。
- 单实例移交与 Runtime 请求并发：移交请求进入同一队列；先到先执行，项目创建按根路径幂等。

### CLI 与随包 skill

**`slterm env`**

- 离线可用；输出面板/项目身份、端点可达性、当前项目/组/面板 ID、cwd、branch、Agent 身份、能力与示例。
- 环境契约（v2，不再保留 v1 名称）：

| 变量 | 含义 |
|---|---|
| `TERM_PROGRAM=slterm` | 当前终端由 slTerminal 承载 |
| `SLTERM_PANEL_ID` | 当前终端面板稳定 ID |
| `SLTERM_PROJECT_ID` | 所属项目稳定 ID |
| `SLTERM_PANEL_GROUP_ID` | 所属面板组稳定 ID |
| `SLTERM_CLI` | 控制面可执行文件绝对路径 |
| `SLTERM_BIN_DIR` | 可执行文件目录，并加入 PATH |
| `SLTERM_RUNTIME_ENDPOINT` | 嵌套进程继承的 `<port> <token>` |

**`slterm ctl`**：完整协议面，所有输出 JSON，`--pretty` 只改排版。资源别名：

```text
slterm project list|add|remove|activate|rename|move|relocate
slterm group split|close|resize|focus
slterm panel list|create|close|move|activate|read|procs|wait
slterm terminal prompt|paste|send-key|run|exec|wait
slterm agent list|get|start|fork|send|paste|read|wait|delegate
slterm git status|diff|rollback|unstage
slterm ctl <method> --params <json>
slterm ctl orchestrate --spec <json> --timeout-ms <n>
```

- `orchestrate` 的步骤引用改为 `{step, field}` 且字段返回项目/组/面板 ID；步骤集合与 v2 方法语义一致，不再出现 tab/pane 术语。
- `tab.*`、`pane.*` 旧别名、`window.*` 方法、v1 schema、`legacy-attach` 编译 feature 全部删除，不做转发。
- 随包 skill `assets/skills/slterm-runtime/`：示例、身份表、工作流全部改为 project/group/panel；明确“先 `slterm env`，再按 capabilities 调用”，禁止进程/端口/源码探测。
- skill 投放：Claude 配置目录与 Codex `.agents/skills` 目录，使用内容指纹 + `.slterm-managed` marker；用户编辑过的同名 skill 保留不覆盖；卸载只删托管内容。

### 审计

- 所有写方法与 `runtime.handover` 经 `tracing` target `audit` 记录，字段遵守项目审计红线：仅 `operation`、`path`（项目根或涉及的文件路径）、`source`（`runtime` 等通道标识）、`result` 与必要的 `expected`/`actual` 指纹。
- 资源 ID 不单列为审计字段：项目根/文件路径进入 `path`；面板/分组操作以 `operation` 的受控枚举区分；`force` 与失败原因编码进 `result` 的受控枚举值。
- 不记录 token、prompt/paste 文本、终端输出、文件内容、Agent 会话正文；错误详情只记录分类码与路径。
- 认证失败只记录计数/原因类别，不记录 token 值；慢订阅断开记录 `subscriber_lagged`。
- 审计与业务日志分离；Runtime 不新建第二套日志通道。

### Conformance 与迁移

**删除的 v1 面**

- `assets/runtime-api-v1.schema.json` 与 v1 文档中的方法/错误码描述；`legacy-attach` feature；`SLTERM_PANE_ID`；`tab.*`、`pane.*` 方法；`window.tabs`/`tabs` 资源；旧移交“新建页签 + 聚焦窗口”组合。
- 端口文件版本 1 不接管；`version:1` 请求返回 `protocol_version_mismatch`，`details.supported_versions=[2]`。

**v1 → v2 映射**

| v1 | v2 |
|---|---|
| `pane_id` | `panel_id`（终端面板） |
| `tab_index`/`tab_id` | `panel_group_id` + `panel_id` |
| `window.tabs[].panes[]` | `projects[].groups[].panels[]` |
| `pane.prompt/paste/read/procs/send_key/run/exec/wait` | `terminal.*` 或 `panel.*`（按语义拆分） |
| `tab.new` | `panel.create {kind:"terminal"}` |
| `tab.close/rename/move` | `panel.close/rename/move` |
| `pane.split/close/zoom/resize` | `panel-group.split/close/resize` + `panel.close` |
| `git.*` 的 `pane_id + expected_cwd` | `git.*` 的 `project_id + path` |
| `agents.list`/`agent.*` | `agent.list/get/...`，目标改 `panel_id`/`agent_id` |

**conformance 测试面**

- 契约一致性：方法表 = `capabilities` = schema 方法枚举；错误码集合 = 文档登记集合；`runtime.describe` 与 schema 版本一致。
- 信封：未知字段拒绝、token 静默丢弃、版本不匹配、请求体上限、响应身份校验、`result:null` 区分。
- 资源：稳定 ID 跨重排/移动/重命名/重定位/重启不变；关闭后不复用；墓碑窗口；未知 ID 返回 `resource_not_found`。
- 命令：每类写命令的正常/非法参数/守卫拒绝/force/陈旧 revision；关闭守卫返回详情；项目创建按根幂等；重试同 `mutation_id` 不重复执行、同 `mutation_id` 不同参数返回 `idempotency_conflict`；`expected_revision` 冲突返回 `revision_conflict`。
- 并发：多连接写串行；订阅 revision 单调；慢订阅断开；长 wait 不阻塞写命令。
- 移交：第二实例只聚焦并打开载荷，不建第二窗；项目根幂等。
- 真实进程：条件跳过的双进程 smoke 覆盖端口文件/发现/移交；假 UI owner 覆盖全部派发语义。

### 模块树

```text
slterm_runtime_protocol/          // core，无 GPUI
  src/
    envelope.rs                   // ApiRequest/ApiResponse/ApiEvent
    resource_id.rs                // 线缆 ResourceId：解析/格式化/错误
    resources.rs                  // Project/Group/Panel/布局/快照
    methods/
      read.rs
      write.rs
      terminal.rs
      agent.rs
      git.rs
      handover.rs
    errors.rs                     // 错误码登记与 details 形状
    schema.rs                     // schema 派生与一致性测试
    describe.rs                   // capabilities/features
    limits.rs                     // 契约数值
slterm_app/src/runtime_api/
  server.rs                       // 端点发现、锁、端口文件、停止
  transport.rs                    // accept、信封校验、订阅、派发超时
  hub.rs                          // RuntimeHub v2：revision/墓碑/等待
  dispatch.rs                     // UI owner 单队列与命令执行
  mutation_cache.rs               // (mutation_id, method, params_hash) → response
  cli.rs                          // slterm ctl/env 客户端
  handover.rs                     // 第二实例载荷
  skill_install.rs                // 随包 skill 投放
  audit.rs                        // 审计事件
  conformance.rs                  // 条件跳过真实进程 smoke
```

- `slterm_runtime_protocol::resource_id` 只处理线缆字符串；`ProjectId`/`PanelGroupId`/`PanelId` 的领域 newtype、分配器与持久化归 `slterm_workspace::ids`。

### 复用、改造与新建

| 现有模块 | 处置 | 说明 |
|---|---|---|
| `slterm_app/src/runtime_api/server.rs` | 改造 | 端点发现、token、lifetime 锁、端口文件、serve 线程、停止语义复用；端口文件版本升 2，移交载荷改 v2 |
| `slterm_app/src/runtime_api/transport.rs` | 改造 | accept 配额、信封校验、单请求行、超时直接复用；方法路由与订阅事件改 v2，删 legacy ATTACH |
| `slterm_app/src/runtime_api.rs` 的 `RuntimeHub` | 改造 | revision/订阅/墓碑/等待/委派注册表语义复用；资源类型从 window/tab/pane 改为 project/group/panel |
| `slterm_app/src/runtime_api/command.rs` | 改造 | 参数校验与派发原语复用；`RuntimeCommand` 变体重命名并新增 project/group 方法 |
| `slterm_app/src/runtime_api/git.rs` | 改造 | 陈旧校验三板斧复用；目标从 pane 改项目，执行改经 `GitServices` 与仓库写门 |
| `slterm_app/src/runtime_api/orchestrate.rs` | 改造 | 预检/引用图/逐步回执/失败续作复用；步骤与引用改 v2 资源 |
| `slterm_app/src/runtime_api/agent_api.rs` | 改造 | Agent 身份/代次/等待语义复用；目标改 panel/agent；worktree 事务复用 |
| `slterm_app/src/runtime_api/cli.rs` | 改造 | 请求-响应双人格与端点发现复用；子命令面按 v2 重写；env 变量改名 |
| `slterm_app/src/runtime_api/shortcuts.rs` | 改造/删除 | 仅保留 v2 资源别名；tab/pane 快捷面删除 |
| `slterm_app/src/git_worktree.rs` | 改造 | 事务语义保留；接入 `GitServices` 写门与 v2 错误码 |
| `assets/skills/slterm-runtime/` | 重写 | 身份/示例/工作流改 v2；skill 安装器复用 |
| `assets/runtime-control-api.md`、`runtime-api-v1.schema.json` | 替换 | 新增 v2 文档与 schema；v1 文件删除 |
| 新 `slterm_runtime_protocol` | 新建 | 协议类型、线缆 `ResourceId`、资源、schema、限制值 |
| 新 UI owner 派发队列 | 新建 | 单队列、revision 校验、幂等缓存、墓碑 |
| 新 conformance 套件 | 新建 | 契约/资源/命令/并发/移交 |

### 验收与测试接缝

- 协议：信封逐字段、`deny_unknown_fields`、token 静默丢弃、版本不支持、结果/错误互斥、请求上限、响应身份。
- 资源：快照 schema、稳定 ID 生命周期、布局树重建、面板移动保 ID、组空置移除、项目重定位保 ID、墓碑幂等。
- 命令：每个方法的参数边界、错误码、守卫、force、expected revision、重试 `mutation_id` 缓存与冲突；`git.*` 的 revision 陈旧与项目作用域。
- 并发：多连接串行、订阅落后断开、长 wait 不阻塞写、关闭与等待竞态。
- CLI/skill：`slterm env` 离线/在线输出、资源别名与 ctl 等价、skill 内容与托管投放、卸载保留用户编辑。
- 审计：写操作字段集合、无 token/文本/内容、force 与失败记录。
- 迁移：v1 方法/版本/环境变量全部失败或不存在；端口文件版本 2；移交载荷 v2；schema 文档无 v1 残留。
