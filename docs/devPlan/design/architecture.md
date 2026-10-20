# 架构总览

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

目标态架构：单进程、单窗口、GPUI 前台直持实体；领域层分为 `slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol` 五个 core crate；app 壳按 `Workspace → ProjectView → PanelGroupView → PanelHost` 重建；持久化由单一 `PersistenceCoordinator` 串行落盘。保留 `slterm_terminal`、`slterm_split`、`slterm_settings`、`slterm_completions`、`slterm_hook` 与既有 `atomic_file`/`settings_store` 机制。

### 新 core crate 边界

| crate | 职责 | 不承担 |
|---|---|---|
| `slterm_workspace` | 项目注册表、布局树、面板组、面板参数 DTO、去重、标题、焦点、会话归属纯函数、持久化 DTO | GPUI、I/O、PTY、编辑器缓冲 |
| `slterm_sidebar` | 侧栏布局状态机、文件树模型、选择/剪贴板/文件操作计划、Commit 投影、Agent 节点过滤与标题消歧纯函数 | GPUI、文件系统执行、Git 进程 |
| `slterm_agents` | `AgentKind` 能力注册表、会话元数据、hook 事件归一化、恢复/分叉命令描述 | 管道/进程/网络 I/O |
| `slterm_hooks` | Hooks 事件与 handler 目录、schema 校验、托管条目规则 | 文件读写、CLI 启动 |
| `slterm_runtime_protocol` | Runtime v2 信封、资源 ID、请求/响应 schema、错误码 | 服务器、传输、应用状态 |

- `slterm_workspace` 生产依赖仅 `slterm_split`；`slterm_sidebar` 依赖 `slterm_workspace` 与 `slterm_split`；`slterm_hooks` 依赖 `slterm_agents`；`slterm_runtime_protocol` 零生产依赖。
- 序列化使用 `serde`/`serde_json`；不使用 path 依赖绕开分类。
- Git/文件系统执行体、Hooks 文件读写与 Runtime 服务器留在 app。

### crate 与模块拓扑

```
slterm_workspace/                 项目/布局/面板领域
  src/lib.rs
  src/ids.rs                      ProjectId / PanelGroupId / PanelId / IdAllocator
  src/project.rs                  ProjectRegistry / ProjectRecord / ProjectRoot
  src/layout.rs                   ProjectLayout / PanelGroup / 归一化 / 不变量
  src/panel.rs                    PanelKind / PanelParams / 各面板参数
  src/terminal.rs                 TerminalBook / TerminalEntry / 编号分配
  src/dedupe.rs                   DedupeKey / DedupeIndex / 标题冲突
  src/focus.rs                    FocusMemory / 关闭焦点转移 / 循环
  src/paths.rs                    PathKey / PersistedPath / 根重定位计划
  src/sessions/                   SessionKey / 归属 / 相对时间 / 删除账本
  src/persist/                    DTO / 信封 / 恢复对账 / 版本判定

slterm_sidebar/                   侧栏与文件树模型
  src/registry.rs                 SideViewId / ViewDef / 单一注册表
  src/layout.rs                   SidebarLayout / 按钮组织 / 纯状态迁移
  src/geometry.rs                 宽度、比例、指针换算
  src/view_state.rs               视图状态声明与清洗
  src/tree/                       TreeModel / Selection / Filter / Clipboard / FileOp
  src/commit.rs                   Commit 四态、分组、排序、过滤投影
  src/agent_files.rs              Agent 节点、过滤匹配、标题消歧

slterm_agents/                    Agent 能力与会话元数据
  src/kind.rs                     AgentKind / 能力表 / 显示名 / 品牌 slug
  src/session.rs                  会话元数据、恢复/分叉命令描述
  src/events.rs                   hook 事件归一化

slterm_hooks/                     Hooks 目录与校验
  src/catalog.rs                  事件与 handler 目录
  src/schema.rs                   schema 校验
  src/managed.rs                  托管条目规则

slterm_runtime_protocol/          Runtime v2 协议类型
  src/envelope.rs                 信封 / 错误码
  src/resources.rs                资源 ID / 请求 / 响应

slterm_split/                     保留：SplitTree<T> / Rect / RemoveOutcome / 比例数学
slterm_settings/                  保留：设置值模型、键注册表、数据目录路径权威
slterm_terminal/                  保留：网格、VT、PTY、输入编码、渲染快照
slterm_completions/               保留：补全匹配引擎
slterm_hook/                      保留：hook 小进程桥

slterm_app/
  src/lib.rs                      启动链唯一入口 run()
  src/app/                        新壳
    workspace.rs                  Workspace 实体
    project_view.rs               ProjectView 实体
    panel_group_view.rs           PanelGroupView 实体
    panel_host.rs                 PanelHost 实体、错误边界
    panel_registry.rs             面板类型唯一注册点与工厂
    panels/                       terminal / editor / markdown / diff / gitshow / settings
    sidebar/                      活动栏、双槽、四个视图
    focus.rs                      焦点意图与焦点协调
    close.rs                      关闭守卫与关闭链
    instance.rs                   单实例与移交载荷
    notifications.rs              通知与确认框服务
  src/services/                   文件系统、Git、Agent 适配、Hooks 文件读写
  src/persistence/                新持久化层
    coordinator.rs                PersistenceCoordinator、去抖、串行队列、generation
    files.rs                      文件键、路径、信封、读写
    store.rs                      原子写、备份、损坏隔离、配额
    recovery.rs                   崩溃快照
    clock.rs                      Clock trait 与系统实现
    fault.rs                      故障注入
  src/runtime_api/                Runtime v2 服务器与投影
```

旧 `src/app_shell/` 在迁移期保留、只做编译修复；新代码不得 `use crate::app_shell::*`。

### 依赖方向与 architecture/dependencies.toml 登记

依赖方向：

```
slterm_app (application)
  → slterm_workspace → slterm_split
  → slterm_sidebar → slterm_workspace / slterm_split
  → slterm_agents
  → slterm_hooks → slterm_agents
  → slterm_runtime_protocol
  → slterm_terminal / slterm_settings / slterm_split / slterm_completions

slterm_hook 不被 app 依赖；renderer_packages 禁入 core/hook。
各 core 不得反向依赖 app。
```

`architecture/dependencies.toml` 增补：

```toml
[crates.slterm_workspace]
layer = "core"
dependencies = ["slterm_split"]

[crates.slterm_sidebar]
layer = "core"
dependencies = ["slterm_workspace", "slterm_split"]

[crates.slterm_agents]
layer = "core"

[crates.slterm_hooks]
layer = "core"
dependencies = ["slterm_agents"]

[crates.slterm_runtime_protocol]
layer = "core"

[crates.slterm_app]
layer = "application"
dependencies = ["slterm_terminal", "slterm_settings", "slterm_split", "slterm_completions", "slterm_workspace", "slterm_sidebar", "slterm_agents", "slterm_hooks", "slterm_runtime_protocol"]
build-dependencies = ["slterm_settings"]
dev-dependencies = []
```

同步动作：

- 根 `Cargo.toml` 的 `workspace.members` 增加五个新 core crate。
- `architecture/file-budgets.txt` 增加五个新 `root` 条目（沿用统一行数上限）。
- `scripts/check_architecture.py` 机械校验 member 与策略键集全等；新增 crate 未登记即红。
- 每个新 crate 使用显式 `[lib] test = false` + `[[test]] name = "lib_tests"`。
- `slterm_workspace` 的 `Cargo.toml` 显式声明 `serde`/`serde_json`；不使用 path 依赖绕开分类。

### 状态所有权矩阵

每个事实只有一个属主；其他实体只持句柄、索引或派生的只读快照。

| 事实 | 唯一属主 | 持久化落点 |
|---|---|---|
| 项目集合、顺序、显示名、根路径、活跃项目 | `slterm_workspace::ProjectRegistry`；运行期由 `Workspace` 实体持有 | `state/workspace.json` |
| ID 分配器（项目/面板组/面板） | `Workspace` 持有 `IdAllocator` | `workspace.json` 的 `next_*` |
| 项目布局树、面板组划分与比例 | `ProjectLayout.tree`（`SplitTree<PanelGroupId>`）；`ProjectView` 持有 | `state/projects/<id>/layout.json` |
| 面板组成员、顺序、激活面板 | `PanelGroup`；`PanelGroupView` 只持页签展示态 | `layout.json` |
| 面板持久参数（路径、形态、换行、Diff 参数、语言覆盖等） | `PanelRecord.params`；面板 body 的同值字段是只读运行副本 | `layout.json` |
| 聚焦面板组（重启恢复用） | `ProjectLayout.focused_group` | `layout.json` |
| 终端编号计数、自定义标题、最近 cwd | `TerminalBook`（项目内，`ProjectView` 持有） | `state/projects/<id>/terminals.json` |
| PTY 进程、网格、回滚、选区、输入状态 | `slterm_terminal::TerminalSession`；`TerminalRegistry` 只持进程映射索引 | 不持久化 |
| 编辑器/Markdown/Diff 缓冲、撤销、光标、滚动、脏状态 | 各面板 body 实体/引擎 | 崩溃快照（仅未保存文本） |
| 面板标题 | 派生值：`PanelRecord` 的持久字段 + body 运行期信息 + 项目标题冲突表 | 仅终端自定义标题持久化 |
| 键盘焦点 | GPUI `Window`/`FocusHandle` | 不持久化 |
| 运行期焦点记忆（切项目恢复） | `ProjectView.focus_memory` | 不持久化 |
| 应用设置值 | `SettingsService`（全局唯一） | `state/settings.json` |
| 侧栏布局 | `SidebarHost` | `state/sidebar.json` |
| 侧栏各视图界面状态 | 各视图实体 | `state/views/<view-id>.json` |
| 通知、确认框队列 | `NotificationService` / `DialogService` | 不持久化 |
| 关闭流程状态 | `Workspace.close_state` | 不持久化 |
| 持久化调度、去抖、写队列、generation | `PersistenceCoordinator` | — |
| 崩溃快照磁盘件 | `RecoveryStore` | `state/recovery/<project-id>/<panel-id>.json` |
| Runtime v2 资源投影 | `RuntimeHub` 从 `Workspace` 派生；不复制状态 | 不持久化 |
| 单实例句柄与移交载荷 | `InstanceCoordinator` | 不持久化 |

禁止派生第二份权威：`RuntimeHub`、侧栏、导航树、面板页签都只读同一属主；禁止在面板内另存项目布局或终端编号。

### 线程与执行模型

- **GPUI 前台线程**：唯一允许修改实体状态、派发命令、推进焦点、渲染的线程。禁止阻塞 I/O、网络、子进程等待、长计算。
- **PTY 解析线程**：每个 `TerminalSession` 的读取/解析在其专用线程；输出经有界通道交给前台；前台按帧合并，隐藏项目不提交绘制。
- **持久化写线程**：单线程串行消费 `WriteJob`；所有文件写入在此线程执行，前台只产生不可变字节快照。单实例保证同进程唯一写者。
- **后台执行器**：文件读取、Git 读取、Markdown 渲染、Diff 计算、目录扫描、历史会话扫描走 GPUI background executor 或专用线程；结果带 `(PanelId, generation)` 回前台，过期结果丢弃。
- **文件监听线程**：每个项目根与根外打开文件父目录的 watcher；事件合并窗口由域定义（编辑器约 300ms，Diff 外部修改 200ms），只向前台发事件。
- **GPU 渲染**：GPUI 内部线程；业务不接触。

跨线程规则：

- 只经有界通道传递消息或不可变快照；禁止 `Arc<Mutex<WorkspaceState>>` 式共享可变状态。
- 前台产生的持久化快照必须是 owned `Vec<u8>`，后台不得回读实体。
- 所有异步结果应用前检查实体存活与 `generation`；关闭/移除后到达的结果直接丢弃。

### 启动链

单入口 `slterm_app::lib::run()`；`main.rs` 只做 Windows 子系统选择。

1. **实例检查与移交**：获取单实例所有权；已有实例则解析移交载荷、发送、退出。载荷使用 Runtime v2 资源 ID（见 `design/domain-model.md`）。
2. **进程准备**：解析 CLI 参数；安装 panic hook（保留 unwind）；创建默认隐藏窗口；初始化 `Clock`、`PersistenceIo`、`NotificationService`、`RuntimeHub`。
3. **配置域并行加载**：`state/settings.json`、`state/sidebar.json`、各 `state/views/*.json` 并行读取。单域失败回退该域默认值并记录警告；设置损坏提示 `配置已损坏，已回退默认值`。配置加载完成后才开始项目数据。
4. **项目索引加载**：读取 `state/workspace.json`。
   - 成功：进入项目布局加载。
   - 失败：显示项目数据错误界面（自绘标题栏保留窗口控制），提供 `重试` 与 `以空状态继续`；阻断进入工作区。选择空状态后，首次写入前先隔离损坏索引。
5. **项目布局并行加载**：按项目并行读取 `state/projects/<id>/layout.json` 与 `terminals.json`，执行恢复对账。单项目失败或版本不兼容：该项目按空布局处理、隔离原文件、并入启动聚合警告；其他项目不受影响。
6. **构建运行期**：创建全部 `ProjectView`（急切恢复）；按 `PanelRecord` 创建 `PanelHost` 与 body；终端以并发上限 8 启动 PTY，总数受 64 上限约束；超限或启动失败的终端面板进入可见失败态，可重试。
7. **崩溃快照扫描**：扫描 `recovery/`，校验、清理过期/损坏项；存在可恢复快照时排队一次聚合提示。
8. **恢复活跃项目**：上次活跃项目存在则激活；否则取列表第一个；无项目则无活跃项目。布局恢复不产生焦点意图。
9. **界面挂载**：数据就绪后以标题栏 + 工作区 + 通知区替换加载界面；启动期间通知在挂载后按剩余时长显示。托盘静默启动路径在数据就绪前不显示窗口。

失败隔离原则：配置失败不阻断启动；项目索引失败阻断工作区；单项目布局失败只影响该项目；单面板构建失败只影响该面板。

### 关闭链

所有关闭入口（标题栏关闭钮、系统关闭、任务栏关闭、托盘退出）统一进入 `Workspace::request_close`，不直接 `remove_window`。

| 环节 | 内容 | 时限 | 失败行为 |
|---|---|---|---|
| 关闭守卫 | 聚合全部项目的需保存面板与前台运行进程；单次确认；保存全部按项目顺序与布局顺序逐个执行 | 用户交互 | 任一保存失败中止关闭，保留全部面板，错误通知列出失败项 |
| 冲刷 | `PersistenceCoordinator::flush` 强制到期设置、侧栏、项目索引、项目布局、终端状态 | 3 秒 | 受守卫文件失败则中止关闭，提供重试/取消；只读设置不进入重试循环，只提供放弃/取消 |
| PTY 终止 | 并发终止全部已注册 PTY；优雅等待 2 秒后强制终止 | 3 秒 | 记录日志并继续；系统进程树兜底 |
| 项目数据保存 | 以最终内存状态写 `workspace.json`、全部项目 `layout.json` 与 `terminals.json` | 3 秒 | 记录日志并继续 |
| 清理 | 删除崩溃快照；关闭监听、watcher、后台任务；关闭持久化写线程；销毁实体与窗口 | 尽力 | 记录日志并继续 |

规则：

- 关闭守卫的“需保存”集合 = 脏面板 + 待写/失败自动保存的设置中心面板；终端前台进程单独列出。
- 冲刷是“强制到期”而非取消计时器；冲刷期间新变更合并到最终快照。
- 关闭链只执行一次；`CloseState::{Idle, Guarding, Saving, ShuttingDown}` 防重入，第二次关闭请求在非 `Idle` 时忽略。
- 设置只读（版本过高或 1 MiB 超限）时无法保存，关闭守卫只提供放弃/取消，不反复进入失败循环。
- 系统关机/注销路径走同一链；若操作系统强制终止，已落盘内容以最近一次成功写入为准。

### 错误边界与错误映射

用户可见错误信封仍是 `slterm_app::error::AppError`；core 只定义结构化领域错误，不另立用户信封。

```rust
// slterm_workspace
pub enum WorkspaceError {
    RootNotFound,
    RootNotDirectory,
    RootDuplicate,
    ProjectNotFound,
    InvalidDisplayName,
    LayoutInvariant(LayoutInvariantError),
    RelocationConflict,
}

pub enum RestoreError {
    UnsupportedSchema(u32),
    Corrupt(String),
    Oversized,
}

pub struct RestoreReport {
    pub dropped_panels: Vec<DroppedPanel>,
    pub dropped_groups: Vec<PanelGroupId>,
    pub repairs: Vec<Repair>,
}
```

映射：

| 失败 | 边界 | 行为 |
|---|---|---|
| `workspace.json` 缺失 | 项目索引 | 视作空工作区；不创建文件，首次写入时创建 |
| `workspace.json` 解析/版本失败 | 项目索引 | 阻断工作区，错误界面；隔离前不覆盖 |
| 单项目 `layout.json` 失败 | 项目 | 该项目空布局 + 聚合警告；隔离原文件 |
| 未知面板类型/参数版本 | 面板 | 丢弃该面板 + 聚合警告；项目其余面板正常 |
| 面板工厂失败 | 面板 | 面板区域错误态，提供重试；不拖垮项目 |
| 终端启动失败 | 面板 | 终端面板失败态与重试；不自动切换 shell |
| 设置写失败 | 设置 | 内存值继续生效，失败徽标与重试；关闭时阻断 |
| 布局/侧栏写失败 | 对应域 | 可见通知；关闭冲刷时阻断并重试 |
| 崩溃快照写失败 | 快照 | 一次可见提示，后续周期重试；不阻塞编辑 |
| 面板渲染 panic | 面板 | `catch_unwind` 捕获，显示 `面板渲染出错` + 重试；其他面板不受影响 |
| 区域渲染 panic | 标题栏/主区/通知区 | 区域级错误占位；顶层兜底全屏 `应用渲染错误` |
| 后台任务 panic | 任务 | 转成任务失败事件；不得跨线程传播 |

错误消息不插值凭据、文件内容、终端输出或剪贴板内容。

### 隐藏项目的渲染与布局

- `Workspace::render` 只把活跃项目的 `ProjectView` 放进元素树；隐藏项目的实体、面板、PTY 全部存活。
- `ProjectView` 持 `visible: bool`；`set_visible(false)` 时：
  - 不再调用 `SplitTree::layout` 计算几何；保留上一次提交比例。
  - 向每个 `PanelHost` 传播 `PanelActivity::Hidden`。
  - 终端输出继续解析进网格/回滚；前台只更新模型，不安排重绘计时器。
- `TerminalView` 在隐藏态合并输出通知；切回时以当前网格快照重绘一次，不补帧、不重放。
- 侧栏是应用级外壳，不随项目隐藏；其视图内容按活跃项目刷新。
- 窗口最小化/遮挡不等于项目隐藏：活跃项目仍可布局；由 GPUI 自身节流。

### 旧壳隔离与删除顺序

隔离：

- 新壳代码位于 `src/app/`，旧壳位于 `src/app_shell/`；新模块禁止导入旧 `workspace`/`tab`/`pane`/`dock`/`session`。
- 保留引擎通过公开 API 被新面板适配器消费：`slterm_terminal`、`file_editor`/Markdown 渲染、Git、平台能力；适配层只做类型桥接，不复制引擎语义。
- 迁移期允许两套壳同时编译，但只有一套进入 `run()`；不存在“新旧状态互相同步”的桥。

删除顺序（命名动作，不设过程编号）：

- **核心先行**：新建 `slterm_workspace` 与新持久化层，以纯测试与临时目录验证；旧壳不改。
- **新壳骨架**：`Workspace`/`ProjectView`/`PanelGroupView`/`PanelHost` 与命令分发可运行，暂不接旧面板。
- **面板适配**：终端、编辑器、Markdown、Diff、GitShow、设置中心迁入新 `PanelHost`；旧 `WorkspaceTab` 变体逐个失去消费者。
- **侧栏重建**：活动栏、双槽、导航树/文件浏览器/Commit/Agent 文件迁入新侧栏；旧 `SidebarState` 停止写入。
- **启动切换**：`run()` 指向新 `src/app/`；旧壳仅保留编译。
- **旧壳删除**：删除 `app_shell::workspace`（tab/pane/split 拖拽）、`session`/`session_persistence`/`session_recovery`、旧 `settings_pane`、旧 `doc_tabs`/`code_tab` 中的旧状态壳；保留的引擎下沉为独立模块。旧测试按“旧测试 → 规格条目/新测试”映射后随属主删除。
- **依赖与门禁收尾**：清理不再使用的 Cargo 依赖；确认 `architecture/dependencies.toml`、`file-budgets.txt`、禁名与文档门禁全绿。

删除顺序的硬约束：旧持久化（`session.json` 与根 `settings.json` 读取）必须先于旧壳删除而停止被调用；新数据目录写入路径必须已全量接管。

### 新数据目录与文件名

数据根仍由 `slterm_settings::paths` 唯一权威：`SLTERM_CONFIG_DIR` 优先，默认 `%APPDATA%\slterm`。新状态根为 `<data>/state/`：

```
<data>/
  state/
    settings.json
    sidebar.json
    workspace.json
    instance.lock
    runtime.port
    views/
      nav.json
      explorer.json
      commit.json
      agent-files.json
    projects/
      <project-id>/
        layout.json
        terminals.json
    recovery/
      <project-id>/
        <panel-id>.json
    quarantine/
```

隔离规则：

- 旧 `session.json`、根 `settings.json`、`settings.json.bak`、`settings.corrupted-*` 一律不读、不删、不视为损坏；新代码不提供迁移入口。
- `slterm_settings::settings_path()` 改为返回 `<data>/state/settings.json`；新增 `workspace_path()`、`sidebar_path()`、`view_state_path(ViewId)`、`project_dir(ProjectId)`、`recovery_dir(ProjectId)`。app 侧不得自行拼接路径。
- 文件名去品牌；项目目录用十进制数值 ID，避免路径字符与大小写问题。
- 目录创建在首次写入前完成；读取缺失文件不等于损坏。

### 单实例与移交载荷

- 单实例所有权在创建窗口前获取；已有实例时只发送移交载荷并退出。
- 载荷结构：

```rust
pub enum HandoffRequest {
    Focus,
    OpenProject { root: PathBuf },
    OpenFile { project: Option<ProjectId>, path: PathBuf },
    NewTerminal { project: Option<ProjectId>, cwd: Option<PathBuf> },
}
```

- 资源 ID 字段使用 Runtime v2 字符串形式；未知/失效项目回退到活跃项目或提示，不崩溃。
- 移交接收端把请求排队到前台，在启动完成后顺序执行；同一请求不重复执行。
- 移交载荷不携带凭据、终端内容或文件内容。
