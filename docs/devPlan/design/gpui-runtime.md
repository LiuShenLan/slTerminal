# GPUI 运行时

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

运行期采用“根实体 + 项目实体 + 面板组实体 + 面板宿主实体 + 面板 body 实体”的树形所有权。状态权威在领域对象，GPUI 实体只持运行实例与展示态。

### Entity 拓扑与所有权

```
Workspace (Entity, 窗口根)
├─ ProjectRegistry（领域权威：项目条目与活跃项目）
├─ IdAllocator
├─ HashMap<ProjectId, Entity<ProjectView>>     全部项目常驻
├─ HashMap<ProjectId, FocusMemory>             运行期焦点记忆
├─ Option<FocusIntent>                         一次性焦点意图
├─ CloseState
├─ Entity<SidebarHost>                         应用级侧栏
├─ Entity<NotificationLayer>
├─ Entity<DialogLayer>
└─ Entity<ShutdownChain>（仅关闭期间存在）

ProjectView (Entity, 每项目一个)
├─ ProjectLayout（领域权威对象：树/组/面板记录/聚焦组）
├─ TerminalBook（编号/标题/cwd）
├─ HashMap<PanelGroupId, Entity<PanelGroupView>>
├─ HashMap<PanelId, Entity<PanelHost>>         句柄索引，非成员权威
├─ DedupeIndex
├─ HashMap<PanelId, Subscription>
└─ FocusMemory（运行期）

PanelGroupView (Entity)
├─ PanelGroupId
├─ Vec<PanelId>（成员与顺序的展示副本，权威在 ProjectLayout；变更由 ProjectView 推送）
├─ active: PanelId
└─ 页签栏滚动/溢出下拉状态（运行期）

PanelHost (Entity)
├─ PanelId / PanelKind
├─ PanelParams 运行副本（只读模型；权威在 PanelRecord）
├─ PanelBody（封闭枚举，持具体 Entity）
├─ poisoned: Option<PanelPoison>
├─ TaskSlots（generation + 任务句柄）
└─ Vec<Subscription>

PanelBody = TerminalPanel | EditorPanel | MarkdownPanel | DiffPanel | GitShowPanel | SettingsPanel
```

所有权规则：

- `Workspace` 拥有项目生命周期；项目创建/移除只经 `Workspace`。
- `ProjectView` 拥有面板组与面板实例生命周期；关闭面板时先更新 `ProjectLayout`，再销毁 `PanelHost`。
- `PanelGroupView` 拥有页签顺序与激活的展示副本；权威仍是 `ProjectLayout.groups`，每次结构变化由 `ProjectView` 推送并覆盖。
- `PanelHost` 拥有 body 实例与订阅；body 拥有缓冲、脏、光标、滚动等运行期事实。
- `PanelRecord.params` 是持久参数唯一权威；body 内的同值字段是创建时快照，变更必须经 `PanelHost → ProjectView` 回写。
- `RuntimeHub` 只读 `Workspace` 派生投影，不持有第二份可变状态。

### 订阅图

- `Workspace` 订阅每个 `ProjectView`：
  - `ProjectEvent::ActivateProject`
  - `ProjectEvent::LayoutChanged { debounce: LayoutFacet }`
  - `ProjectEvent::FocusRequested { group, panel, target }`
  - `ProjectEvent::PanelOpened/Closed`
  - `ProjectEvent::ClosePanelRequested`
  - `ProjectEvent::DirtyChanged`（仅用于刷新标题栏/关闭守卫缓存，不改变脏权威）
- `ProjectView` 订阅每个 `PanelHost`：
  - `PanelEvent::TitleChanged`
  - `PanelEvent::DirtyChanged`
  - `PanelEvent::ParamsChanged(PanelParamsPatch)`
  - `PanelEvent::FocusRequested`
  - `PanelEvent::CloseRequested`
  - `PanelEvent::Exited`（终端）/ `LoadFailed` / `Ready`
- `ProjectView` 订阅每个 `PanelGroupView`：
  - `TabActivated`
  - `TabCloseRequested`
  - `TabReorderRequested`
  - `TabDragStarted/Finished`
- `PanelBody` 订阅全局服务：`SettingsService`、`NotificationService`、项目 `GitService`、`FsWatch`。订阅句柄随 body 释放。
- 订阅回调只做三件事：更新领域对象、标记持久化脏、`cx.notify`。禁止在回调里做阻塞 I/O。

### 渲染路径与隐藏项目

`Workspace::render` 结构：

```
title bar
[activity bar | sidebar | main]
  main = active ProjectView 或项目空态/无项目空白
overlays: dialogs, notifications, tab menus, drag previews
```

- 只渲染活跃项目的 `ProjectView`；隐藏项目实体不在元素树中。
- `ProjectView::render`：
  - 用提交比例调用 `SplitTree::<PanelGroupId>::layout`；分隔条拖拽只走 overlay 与 `preview_ratio`，拖拽期间不重排终端网格；松手提交后才让面板观测新几何。
  - 每个叶渲染 `PanelGroupView`；`PanelGroupView` 只渲染激活 `PanelHost`，非激活面板保留实体但不进树。
  - 无面板组时渲染项目空态（不是面板）。
- 隐藏项目：
  - `ProjectView.set_visible(false)` 后不计算布局、不渲染；`PanelHost` 收到 `PanelActivity::Hidden`。
  - 终端仍解析输出；前台不安排帧回调节流，模型保持最新网格/回滚。
  - 切回时 `set_visible(true)`，`TerminalView` 以当前快照重绘一次并恢复滚动锚点。
- 侧栏始终渲染；其内容按活跃项目刷新，不随项目隐藏。
- 窗口级通知/确认框挂在 `Workspace` 根部，不随项目切换清空。
- 面板页签菜单、重命名弹窗、拖拽预览为覆盖层；项目切换时关闭。

### 焦点与一次性焦点意图

```rust
pub struct FocusIntent {
    token: u64,
    project: ProjectId,
    group: PanelGroupId,
    panel: PanelId,
    target: FocusTarget,
}
```

- 新建/显式打开面板：`Workspace` 生成 `FocusIntent`，等待 `PanelEvent::Ready` 或下一帧消费。
- 消费条件：项目仍是活跃项目、组仍是聚焦组、面板仍是激活面板、意图 token 未失效。任一不满足即丢弃。
- 面板关闭、项目切换、用户点击其他面板、显式聚焦其他控件都会使当前意图失效（递增 token）。
- 命中去重键的打开：直接调用 `focus_now`，不生成 `FocusIntent`。
- 点击面板内容/页签：`Workspace` 先设置活跃项目（如需要）、聚焦组、激活面板，再 `window.focus(handle)`。
- 切换项目：若 `focus_memory` 存在且面板存活，聚焦其落点；否则焦点停留在工作区层级。
- 启动/布局恢复：不调用 `window.focus`；`focused_group` 只决定布局层级。
- `Ctrl+Tab`/`Ctrl+Shift+Tab`：仅在存在聚焦面板时消费，按 `focus_cycle` 结果激活并聚焦；终端与编辑器不拦截。
- 焦点落点由面板 body 提供：

```rust
pub trait PanelFocus {
    fn focus_target(&self, cx: &App) -> FocusHandle;
}
```

  - 终端 → 输入区；编辑器 → 编辑区；Markdown → 渲染容器或源码编辑区；Diff → 工作区可编辑侧或 HEAD 只读侧；GitShow → 只读内容区；设置中心 → 面板容器。
  - 只读/降级态由 body 自行返回对应容器；布局恢复不调用。

### 异步任务与取消

- 每个 `PanelHost` 持 `TaskSlots`：

```rust
pub struct TaskSlots {
    generation: u64,
    tasks: Vec<Task<()>>,
}

pub struct TaskToken { generation: u64 }

impl TaskSlots {
    pub fn begin(&mut self) -> TaskToken;      // 递增 generation，旧结果自动失效
    pub fn is_current(&self, token: TaskToken) -> bool;
    pub fn cancel_all(&mut self);
}
```

- 文件读取、Git 读取、Markdown 渲染、Diff 计算、外部修改复核、快照写入都携带 `TaskToken`；结果应用前检查 `is_current` 与实体存活。
- 面板关闭：`cancel_all`，调用 body `shutdown`（终端终止 PTY、取消 watcher、删除崩溃快照），释放订阅，移除 `PanelHost`。
- 项目移除：按面板顺序关闭全部面板；取消项目级任务；删除项目目录与恢复快照；从 `Workspace` 移除索引。
- 旧结果（先发后至）不得覆盖新结果；Diff/Markdown 渲染必须保留上一版内容直到新结果可用。
- 前台禁止 `block_on` 子进程/网络；PTY spawn 在终端 core 的专用路径，结果异步回传。

### 窗口关闭钩子

- `Workspace::request_close(window, cx)` 是唯一关闭入口；GPUI 关闭事件、标题栏按钮、系统命令、托盘退出都路由到它。
- 状态机：

```rust
pub enum CloseState {
    Idle,
    Guarding,
    SavingAll { failed: Vec<PanelId> },
    ShuttingDown,
}
```

- 进入 `Guarding` 后收集全部项目（含隐藏项目）的脏面板与前台进程，弹出一次聚合确认。
- 取消 → 回 `Idle`，窗口保持；保存全部失败 → 回 `Guarding` 并列出失败项，窗口保持。
- 确认 → `ShuttingDown`，启动 `ShutdownChain` 异步任务；完成后 `window.remove_window()`。
- 非 `Idle` 时再次请求关闭直接忽略；异步保存回调不得重入 `remove_window`。
- 关闭期间禁止新建面板/终端；命令返回 `invalid_state`。

### 面板错误边界

- `PanelHost::render` 对 body 渲染执行：

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body.render(window, cx)));
```

- panic 时：
  - 捕获 payload 转成用户可读摘要与诊断摘要（不含文件内容、终端输出、凭据）。
  - 设置 `poisoned = Some(PanelPoison { message, at })`，记录 `tracing` 日志。
  - 返回面板错误占位：`面板渲染出错`、说明、`重试`、可折叠详情。
- `重试`：丢弃旧 body 实体，按 `PanelRecord.params` 重新创建；清空 poison。重建失败则显示不可恢复错误与关闭入口。
- 其他面板、项目与窗口不受影响。
- 区域错误边界：`TitleBarRegion`、`MainRegion`、`NotificationsRegion`、`SidebarRegion` 各自包裹 `RegionBoundary`；区域 panic 显示区域错误占位，顶层兜底全屏 `应用渲染错误`。
- 发布 profile 必须保持 `panic = "unwind"`；若改为 abort，面板级隔离失效，视为架构回归。
- 后台线程 panic 不跨线程捕获为渲染错误；任务包装器转成 `PanelEvent::TaskFailed`。

### 终端与后台输出

- `TerminalPanel` 持 `TerminalSession` 句柄与 `TerminalView`；PTY 读取/解析在终端 core 线程，网格快照供前台渲染。
- 输出合帧：可见终端最长 16ms 提交一次；同帧网格变更原子呈现。
- 隐藏项目/不可见面板：解析继续，`PanelActivity::Hidden` 下不安排前台帧；切回时读取最新快照。
- PTY 退出：`PanelEvent::Exited { code }` → 面板底部操作栏；重连创建新 `TerminalSession`，旧回滚保留并插入“新会话”分隔。
- `TerminalRegistry` 是 PTY 映射唯一权威；`TerminalPanel` 只持 `PanelId` 与视图句柄，不另存进程表。
- 关闭面板与项目移除必须显式 `shutdown`；禁止依赖 Drop 终止进程。

### 测试接缝

- 纯领域测试：`slterm_workspace` 内完成 ID、布局归一化、恢复对账、去重、焦点、标题、注册表不变量；不链接 GPUI。
- 虚拟窗口测试：`#[cfg(all(test, feature = "gpui-test-support"))]` 双门控；使用虚拟窗口与测试执行器，只断言实体状态与命令结果，不断言像素。
- 可注入点：

```rust
pub trait TerminalLauncher { fn spawn(&self, spec: LaunchSpec) -> Result<TerminalSession, LaunchError>; }
pub trait PanelFactory { fn create(&self, ctx: PanelCreateContext<'_>) -> Result<PanelInstance, PanelOpenError>; }
pub struct WorkspaceHarness {
    pub workspace: Entity<Workspace>,
    pub clock: Arc<ManualClock>,
    pub store: Arc<InMemoryStore>,
    pub launcher: Arc<FakeTerminalLauncher>,
    pub handoff: Arc<RecordingInstanceCoordinator>,
}
```

- 测试数据目录必须隔离：持久化路径注入临时目录；生产路径 `%APPDATA%` 永不被测试触碰。
- 真实 GPU/DPI/输入法/拖拽手感按豁免登记人工核验；自动化只覆盖状态机与命令。
