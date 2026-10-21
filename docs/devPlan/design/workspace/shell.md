# 工作区壳

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 项目、项目布局、面板组与面板的领域模型与生命周期，以及窗口壳、脏状态与关闭、通知与确认、单实例、设置入口和持久化协调；焦点/命令、通知类型与窗口几何引用所有者文档。

### 分层与模块树

core crate 拓扑与 `architecture.md` 一致。`slterm_workspace`：

```text
slterm_workspace/
  src/lib.rs
  src/ids.rs                 ProjectId / PanelGroupId / PanelId / IdAllocator（领域 ID 唯一定义）
  src/project.rs             ProjectRegistry / ProjectRecord / ProjectRoot
  src/layout.rs              ProjectLayout / PanelGroup / 归一化 / 不变量
  src/panel.rs               PanelKind / PanelParams / 各面板参数
  src/terminal.rs            TerminalBook / TerminalEntry / 编号分配
  src/dedupe.rs              DedupeKey / DedupeIndex / 标题冲突
  src/focus.rs               焦点循环顺序与关闭焦点转移纯函数
  src/paths.rs               PathKey / PersistedPath / 根重定位计划
  src/sessions/              SessionKey / 归属 / 相对时间 / 删除账本
  src/persist/               DTO / 信封 / 恢复对账 / 版本判定
```

`slterm_sidebar`（依赖 `slterm_workspace` 的 ID/路径类型）：

```text
slterm_sidebar/
  src/registry.rs            SideViewId / ViewDef / 单一注册表
  src/layout.rs              SidebarLayout / 按钮组织 / 纯状态迁移
  src/geometry.rs            宽度、比例、指针换算
  src/view_state.rs          视图状态声明与清洗
  src/tree/                  TreeModel / Selection / Filter / Clipboard / FileOp
  src/commit.rs              Commit 四态、分组、排序、过滤投影
  src/agent_files.rs         Agent 节点、过滤匹配、标题消歧
```

app 侧重建：

```text
slterm_app/src/
  app/
    workspace.rs             Workspace 实体：项目集合、活跃项目、关闭状态
    project_view.rs          ProjectView 实体：布局、面板体、焦点记忆
    panel_group_view.rs      PanelGroupView 实体：页签栏、溢出、拖放
    panel_host.rs            PanelHost 实体：面板生命周期、能力查询、渲染隔离
    panel_registry.rs        面板类型唯一注册点与工厂
    panels/                  terminal / editor / markdown / diff / gitshow / settings
    sidebar/                 活动栏、双槽、四个视图
    focus.rs                 焦点意图与焦点协调
    close.rs                 关闭守卫与关闭链
    instance.rs              单实例与移交载荷
    notifications.rs         通知与确认框服务
  services/                  文件系统、Git、Agent 适配、Hooks 文件读写
  persistence/               PersistenceCoordinator、文件键、恢复
  runtime_api/               Runtime v2 服务器与投影
```

归属边界：`slterm_workspace` 只含可单测的领域状态与纯转换，不引用 GPUI、文件系统或进程；app 负责实体、I/O、渲染与持久化。领域 ID 唯一定义于 `slterm_workspace::ids`；Runtime v2 线缆使用 `slterm_runtime_protocol` 的不透明 `ResourceId` 字符串，不重复领域 newtype。

### 关键类型

领域类型 `ProjectId`/`PanelGroupId`/`PanelId`/`ProjectRoot`/`ProjectLayout`/`PanelGroup`/`PanelRecord`/`PanelParams`/`PanelKind`/`TerminalBook` 均以 `design/domain-model.md` 为单点；壳只持有 `ProjectView`，不另建 `Project` 聚合。`PanelParams` 字段由各面板篇定义，工作区只负责持久化与重建分发，不解释面板内部字段。`FocusTarget` 同属领域单点。

关键接口：

```rust
pub trait PanelBody {
    fn panel_id(&self) -> PanelId;
    fn kind(&self) -> PanelKind;
    fn capabilities(&self, cx: &App) -> PanelCapabilities;
    fn focus_target(&self, cx: &App) -> Option<FocusTarget>;
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> PanelRender;
    fn save(&mut self, cx: &mut Context<Self>) -> Task<SaveResult>;
    fn on_project_root_relocated(&mut self, old: &ProjectRoot, new: &ProjectRoot, cx: &mut Context<Self>);
    fn on_path_renamed(&mut self, old: &Path, new: &Path, cx: &mut Context<Self>) -> PathFollow;
    fn shutdown(&mut self, cx: &mut App);
}

pub struct PanelCapabilities {
    pub dirty: bool,
    pub can_save: bool,
    pub pending_autosave: bool, // 设置中心待写/失败自动保存
    pub foreground: Option<ForegroundProcess>,
}
```

`PanelHost` 持有 `PanelBody` 并负责生命周期、能力查询与渲染隔离。`PanelRender` 是 `Result<AnyElement, PanelError>`；错误边界在 `PanelHost` 统一处理，面板体不需要自建崩溃兜底。

### 不变量

- 项目集合非空时恰好一个活跃项目；无项目时无活跃项目。
- 每个项目恰好一个 `ProjectLayout`；布局为空时 `tree = None` 且无面板组。
- `tree.leaves()` 与 `groups` 键集全等；每个面板组非空；每个非空布局的 `focused_group` 必须是树叶。
- 每个面板组恰好一个激活面板；面板只属于一个项目。
- 面板体在关闭或项目移除前存活；切换项目、面板组、页签只改变可见性、激活态与焦点。
- 终端面板与 PTY 一一对应；面板组内不再拆分。
- 任何布局操作返回新的完整布局或 `LayoutError`，不产生半提交状态。

### 项目生命周期

- 创建：导航树触发文件夹选择；只接受目录。根路径按 Windows 规则归一后必须在目录中唯一；已存在则激活并通知 `该项目已存在`。显示名取文件夹名，与现有显示名重复时追加递增数字后缀（`demo 2`）。新项目分配新的稳定 `ProjectId`，布局为空，追加到项目顺序末尾，成为活跃项目并显示项目空态；不自动创建终端。失败通知 `添加项目失败` 与原因，项目列表不变。
- 激活与切换：点击项目行或打开属于该项目的会话/文件/面板。切换只切换可见性与焦点；目标项目的布局、面板实例、PTY 与脏状态保持。无加载态。
- 活跃回退：移除活跃项目后按项目顺序取下一个；没有下一个取上一个；没有其他项目进入无项目态，主区空白且标题栏中段为空。
- 移除：导航树只发移除请求，确认框由 shell 拥有。正文包含项目名、将终止的终端数量、需保存面板标题与前台运行终端；声明磁盘文件与历史会话记录不受影响。确认后先成功写 `workspace.json`（移除条目并更新活跃项目）；写失败中止移除、项目保持。成功后再删除项目目录与恢复快照，并终止 PTY、销毁面板与布局；取消不变；不可撤销。移除不删除磁盘文件和 CLI 会话记录。
- 根重定位：入口在导航树；新根必须存在、唯一且不是其他项目根。成功保留 `ProjectId`、显示名、顺序、布局、面板实例与展开状态；失败通知 `重新定位根目录失败` 或 `该根目录已属于其他项目`。
- 根不可访问：项目仍可激活、重命名、排序、重定位与移除；根警示由导航树显示；不阻断加载与切换。
- 项目身份以 `ProjectId` 为准，根路径只是唯一性不变量；featureSpec 已同步该表述。

### 根重定位的面板迁移

重定位成功先更新 `ProjectRoot`，再向该项目全部面板逐一分发 `on_project_root_relocated(old, new)`；任一迁移失败不回滚项目根，但必须留下可见状态。

- 终端：PTY 不重启，现有 cwd 保持不变；仅更新项目默认 cwd，新终端使用新根。已有终端继续可在旧目录运行，但应用发起的文件写入仍受新根边界约束。
- 编辑器/Markdown：重算文件相对路径与标题；路径仍位于新根内则保持可编辑；落到新根外则保留面板、缓冲与撤销历史，切换为根外只读并显示提示，脏缓冲允许“另存为”；不得静默关闭或丢弃草稿。
- Diff：按新根所在仓库重算数据根与相对路径；仓库或路径不再可用时进入面板级错误态并提供重试；工作区侧脏缓冲保留，不静默丢弃。
- GitShow：按新仓库根与仓库相对路径重新读取 HEAD；路径不在新仓库时显示不可用错误态，不关闭面板。
- 设置中心：面板实例与选中页保持；Hooks 的 Project/Local 层仍绑定 `ProjectId`，项目上下文显示新根。
- 文件浏览器：按新根重载树；持久化展开/当前行按新根重写相对路径，无法重写的路径丢弃。Commit 按新仓库根重载；Agent 全局文件不受项目根影响。
- 标题全部重算；项目行副行、根警示与历史归属立即更新；布局与面板实例不重建。迁移结果在一个保存周期内持久化。

### 面板组与布局操作

布局树复用 `slterm_split::{SplitTree, SplitDirection, RemoveOutcome, hit_divider, drag_ratio, preview_ratio, commit_ratio}`，叶类型改为 `PanelGroupId`。`slterm_split` 的节点比例、删除收编、分隔线命中与方向导航纯函数直接复用；新增 `LayoutOp` 校验层。

```rust
pub enum LayoutOp {
    SplitPanelOut { panel: PanelId, edge: Edge },
    MovePanel { panel: PanelId, target: PanelGroupId, index: usize },
    ReorderPanel { group: PanelGroupId, panel: PanelId, index: usize },
    ResizeSplit { path: Vec<bool>, ratio_permille: u16 },
    ClosePanel { panel: PanelId },
}

pub enum LayoutError {
    PanelNotFound,
    GroupNotFound,
    InsufficientSpace { group: PanelGroupId },
    InvalidTarget,
}
```

操作语义：

- 页签拖到面板组左/右/上/下边缘：显示落点预览；释放后按方向拆分新组，原面板迁入新组并成为激活面板；源组变空则移除。新组与源组各占 50%。
- 拖入另一组页签栏：按插入位置移动；目标组成为聚焦组，被移动面板成为激活面板。
- 同页签栏拖动：只调整顺序，不改变激活面板。
- 分隔线拖动：只改比例预览，松手才提交；终端网格不随预览重排。键盘聚焦后方向键每次调整 10px，换算为比例并夹取。
- 面板组可用尺寸下限 120×80px；拆分前按上一帧布局矩形校验，空间不足返回 `InsufficientSpace`，不执行拆分、不显示有效落点。
- 空组自动移除；兄弟子树收编后按剩余空间归一。残留的空组不持久化、不渲染。
- 新建终端落点：当前聚焦组；没有面板组时创建第一个组。组头 `+` 视觉尺寸 22×22px、点击区不小于 24×24px，悬停提示 `新建终端`，在该组创建终端；页签菜单 `新建终端` 同理；项目空态按钮创建第一个组与终端。
- 不提供浮动组、弹出窗口、跨项目拖拽或将面板移出主区。拖到窗口外、隐藏项目布局或无效区域只取消手势。
- 页签菜单 `拆分到右侧`、`拆分到下方`、`移动到下一个面板组` 与拖拽走同一 `LayoutOp`。

页签拖拽状态机：`Idle → Pending(阈值 4px) → Dragging(target: DropTarget) → Commit/Cancel`。落点 `TabBar { group, index } | Edge { group, edge } | Invalid`；预览只画覆盖层，布局树不动，提交一次性应用。拖拽中切换项目、关闭源面板或目标组消失时取消并保持原布局。

### 面板类型、标题与去重

面板类型封闭集为终端、编辑器、Markdown 文档、Diff、GitShow、设置中心。恢复时只接受该集合，未知类型丢弃；若项目全部面板被丢弃，显示项目空态并并入启动聚合警告。

去重键：

| 面板 | 去重键 | 命中行为 |
|---|---|---|
| 终端 | 不去重 | 每次新建独立 PTY |
| 编辑器 | 项目 + 规范化真实路径 | 聚焦已有面板，不新建 |
| Markdown | 项目 + 规范化真实路径 | 聚焦已有面板 |
| Diff | 项目 + 规范化工作区路径 + 面板模式/后缀 | 聚焦已有面板 |
| GitShow | 项目 + 仓库相对路径（大小写不敏感）+ 后缀 | 聚焦已有面板 |
| 设置中心 | 项目 | 聚焦已有面板，每项目最多一个 |

`PanelOpenRequest` 携带 `kind / params / dedupe_key / target_group / focus_policy / source`；`source` 用于区分 Explorer、Commit、Agent、历史恢复、Runtime 等入口，但不改变去重身份。跨项目不聚焦；同一文件在不同项目各自有实例。

标题规则由 `title::resolve_titles(project)` 统一计算：

- 终端默认 `terminal-N`。`N` 来自项目级持久计数，从 0 开始；关闭不复用，重启不重算，新建继续递增。自定义标题优先且随布局持久化。
- 文件型面板默认文件名；同一项目出现同名文件时，相关页签改用相对项目根的路径；项目根外文件使用规范化绝对路径。路径比较按 Windows 大小写不敏感规则。关闭同名面板之一后重算，可恢复纯文件名。
- Diff 默认模式追加 `(git diff)`；GitShow 追加 `(git delete)`；设置中心固定 `设置中心`。
- Agent 全局文件面板使用 `sidebar/agent-files.md` 的例外规则（文件名 + 最短可区分路径 + Agent 显示名），不套用项目相对路径回退。
- 标题过长单行省略，悬停显示完整标题。标题变化后同步页签、窗口标题（若涉及活跃项目）与名称。

页签呈现：

- 终端状态圆点语义：工作绿色、注意黄色、完成灰色、错误红色；同时提供悬停文本与名称。绑定 CLI 后叠加 16px 品牌图标，按 `AgentKind` slug 取资产；无资产不显示。
- 文件型面板显示 14px 类型图标，不显示终端状态点或 CLI 图标。
- 聚焦面板组的激活页签底部显示 2px 强调色指示条；非聚焦组不显示。
- 脏面板显示未保存标记，并提供名称“未保存”。
- 关闭按钮默认隐藏，悬停或键盘聚焦页签时显示；页签宽度不跳动。
- 页签最小宽度 80px；溢出时横向滚动并提供溢出下拉列表，列出该组全部面板及状态；选择激活对应面板。页签栏键盘：`←/→` 移焦点，`Enter/Space` 激活，关闭按钮可单独聚焦。
- 页签右键菜单：`新建终端`、`关闭`、`关闭其他`、`关闭全部`；文件型面板多 `复制相对路径`；终端多 `重命名`（运行编码 CLI 时禁用）。重命名弹窗校验空名与 128 字符上限。

### 焦点模型

焦点类型、意图、记忆、循环与关闭后继规则由 `ux-framework.md` 单点定义；壳只负责接线：

- `ProjectView` 持有本项目运行期 `FocusMemory`；`Workspace` 与面板不另存第二份。
- 面板/组/项目激活变化交给 `FocusCoordinator`；显式打开/新建产生的 `FocusIntent` 由协调器消费，布局恢复不产生意图。
- 关闭聚焦面板时按 `ux-framework.md` 的后继规则迁移焦点；`Ctrl+Tab` 与 `Ctrl+W` 的消费规则同源。

### 脏状态与关闭守卫

脏状态来源：编辑器、Markdown 源码态、Diff 默认模式工作区侧、设置中心手动 Hooks 保存；终端与 GitShow 永不脏。设置中心待写或失败自动保存单独计入“需保存集合”，不据此显示页签脏标记。

```rust
pub enum CloseScope {
    Panel(PanelId),
    OthersInGroup(PanelGroupId),
    Group(PanelGroupId),
    Project(ProjectId),
    Window,
}

pub struct CloseCandidate {
    pub panel: PanelId,
    pub project: ProjectId,
    pub title: String,
    pub dirty: bool,
    pub can_save: bool,
    pub foreground: Option<String>,
}

pub enum CloseDecision { SaveAll, DiscardAll, Cancel }
pub enum SaveBatchResult { AllSaved, Failed(Vec<PanelSaveFailure>) }
```

关闭算法：

- 收集范围内 `CloseCandidate`。
- 单面板：无脏、无前台进程直接关闭；脏弹出标题 `未保存的修改`，按钮 `取消 / 保存并关闭 / 丢弃并关闭`，不能保存时不显示保存。终端有前台进程弹出 `关闭运行中的终端`，按钮 `取消 / 终止并关闭`。设置中心先尝试冲刷待写自动保存，成功直接关闭；失败进入三键确认。
- 批量（关闭其他/关闭全部）：一次聚合确认。存在需保存面板时按钮 `保存全部并关闭 / 放弃全部并关闭 / 取消`；只有运行中终端时按钮 `关闭 / 取消`。
- 保存全部：按项目顺序与布局 DFS/页签顺序逐个保存；任一失败立即中止，保留全部面板，已成功面板清除脏状态并保持打开，终端不终止，错误通知列出失败项。
- 放弃全部：清除脏状态、关闭目标面板、终止范围内终端。
- 项目移除：确认框始终出现；列出终端数量、需保存面板与运行中终端；有脏时同样提供保存全部/放弃全部/取消，没有脏时提供 `取消 / 移除项目`。保存失败中止移除。
- 窗口关闭：聚合全部项目的需保存面板与前台运行终端，包含隐藏项目；有脏标题 `未保存的修改`，只有运行中终端标题 `关闭窗口`；保存全部失败中止关闭并保留窗口。无脏无前台进程不弹确认。
- 单面板保存失败通知 `保存失败，面板未关闭`，面板保持脏；批量失败通知列出失败项。
- 关闭聚焦面板后的焦点迁移按 `ux-framework.md` 的后继规则；中键关闭命中面板但不改变其他面板激活状态。
- 无激活面板时 `Ctrl+W` 不消费、不关窗。

### `Ctrl+,`、配置钮与无项目设置浮层

`SettingsSurface` 只由 `panels/settings.md` 定义，壳只引用，不另建设置形态类型。

- 有活跃项目：`Ctrl+,` / 配置钮确保活跃项目存在设置中心面板。不存在时在当前聚焦组创建（无组则建首个组）；存在时激活其所在组与面板并聚焦；不新建第二个，不切换项目。创建失败通知 `设置中心打开失败：项目尚未就绪，请重试`。
- 无活跃项目：`Ctrl+,` / 配置钮打开仅含“应用”页的全局设置浮层；遮罩、关闭、导入导出、恢复默认与保存行为见 `settings/app-page.md`。浮层不创建项目、项目布局、面板组或面板，不切换活跃项目，不持久化打开状态。
- 项目激活后浮层自动关闭，配置钮/`Ctrl+,` 转入项目设置中心；浮层内临时草稿按设置页失焦规则处理。
- 配置钮激活态：活跃项目的项目布局中存在设置中心面板，且该面板是其所在组的当前面板；面板组是否聚焦不影响。无项目时配置钮无激活态。

### 窗口、标题栏与单实例

窗口与标题栏：

- 单窗口、单实例、仅暗色、无系统标题栏与边框。窗口尺寸、最小几何、DPI 与多显示器规则由 `platform-windows.md` 拥有。
- 自绘标题栏高 34px，左右内边距 12px，字号 12px；左段品牌标识 + `slTerminal`，中段活跃项目显示名（无项目为空），右段最小化/最大化/还原/关闭。
- 左段与中段为拖拽区（`WindowControlArea::Drag`），右段不参与拖拽；中段单行省略，无项目仍保留全高拖拽区。
- 窗口控制钮 38×26px、图标 12px、间距 2px、圆角 4px；最大化/还原文案与图标按 `window.is_maximized()` 每帧回查，不本地翻转；关闭悬停危险色。复用旧 `title_bar.rs` 的原生控制区与虚拟窗口测试分派。
- 关闭钮、Alt+F4、任务栏关闭、托盘退出统一走窗口关闭请求 → 聚合确认 → 关闭链；不直接 `remove_window`。
- 主区无项目级页签栏；项目切换只在导航树与会话入口。确认框、通知、页签菜单、重命名弹窗为覆盖层，不参与横向分区。

单实例：

- 单实例锁为 `<state>/instance.lock` 句柄锁，由 `platform-windows.md` 拥有；进程崩溃由内核释放，锁文件可残留，所有权只看句柄。
- 已有实例时不创建窗口。本次启动经 Runtime v2 本地控制端点调用 `runtime.handover` 移交载荷：`Focus`、`OpenProject { root }`、`OpenFile { project_root?, path }`、`NewTerminal { project_root?, cwd }`。载荷用根路径定位，不传内部 ID；端点发现、超时与重试见 `platform-windows.md`。
- 已有实例按载荷顺序处理：`OpenProject` 归一后命中则激活、否则创建项目；`OpenFile` 在现有项目根中取最深命中，经统一文件打开路由分派并聚焦，根外路径只提示并聚焦窗口；`NewTerminal` 取 cwd 所属项目或活跃项目，无项目时只聚焦；`Focus` 最后执行前置。
- 单实例保证同一项目数据只有一个写进程；不提供多实例协同。

窗口几何：

- 窗口几何（迁移后落点 `state/window.json`、恢复算法、DPI 夹取与多显示器规则）由 `platform-windows.md` 拥有，属范围外迁移能力。本规格不创建该文件，关闭链不冲刷窗口几何。

### 启动与关闭

启动链：

1. 单实例检查。
2. 加载设置：单域失败使用默认值并记录警告；整体损坏通知 `配置已损坏，已回退默认值`。
3. 加载项目数据：失败显示项目数据错误界面（错误信息、`重试`、`以空状态继续`，保留自绘标题栏）；`以空状态继续` 后允许保存新数据，原损坏文件先隔离副本，不覆盖。
4. 急切恢复全部项目布局与全部面板参数；单个项目布局解析失败按空布局处理，其他项目不受影响；未知面板类型丢弃；结果合并为一条启动警告。
5. 恢复上次活跃项目；无法定位取项目顺序第一个；无项目进入无项目态。
6. 按上限并发创建 PTY；超限或失败的面板显示可重试错误态；隐藏项目只暂停绘制，PTY 继续运行。
7. 检测异常退出快照，以一条聚合提示询问恢复或丢弃；恢复不产生焦点意图。
8. 挂载标题栏、工作区、侧栏与通知；启动期通知队列按剩余时长显示。

关闭链：

1. 窗口关闭请求，若存在需保存面板或前台运行终端，弹聚合确认；取消即中止。
2. 冲刷 `Guarded` 集合：设置、侧栏、工作区索引、项目布局、终端簿。任一失败中止关闭，提供重试/取消；阶段时限与跨文件顺序见 `persistence.md` 关闭链落盘。
3. 并发终止全部已注册 PTY；失败汇总记录日志，由系统进程树兜底。
4. 以最终内存状态保存项目数据（完整布局与活跃项目标识）；失败只记录，不阻止关闭。
5. 销毁窗口与进程。

关闭链不冲刷窗口几何。移除项目不复用窗口关闭链：确认后先成功写 `workspace.json`，写失败中止移除；成功后再删除项目目录与恢复快照，并终止 PTY、销毁面板与布局。PTY 终止失败只记录。

### 通知与确认框

通知类型、时长、覆盖、去重与缓存由 `cross-cutting.md` 单点定义；本节只保留通知服务的实现行为。通知服务为唯一实现，所有视图、面板、shell 共用。

- 右上角纵向堆叠，单条最大宽度 320px；类型与默认时长/视图覆盖引用 `cross-cutting.md` 的 `NotificationSpec`。
- 不阻塞交互、不抢焦；正文纯文本、不可点击；关闭按钮可聚焦，名称 `关闭通知`。
- 悬停或聚焦暂停倒计时；可操作通知提供右侧按钮（如 `撤销`），`Enter/Space` 触发；关闭按钮或倒计时结束放弃操作。
- 启动期通知先入队列，挂载后按剩余时长显示；项目切换、面板切换、布局恢复不清空。
- 错误通知不得静默丢弃。
- 系统通知与任务栏闪烁的类别、门控、事件键、去重与缓存见 `platform-windows.md`。

确认框服务：

```rust
pub struct ConfirmRequest {
    pub title: String,
    pub body: String,
    pub buttons: Vec<ConfirmButton>,
    pub default_button: usize,
    pub danger_button: Option<usize>,
}
```

- 同一时刻只有一个确认框；新请求使先前请求按取消处理。
- 全屏遮罩模态；`Tab/Shift+Tab` 在按钮间循环，`Enter` 提交焦点按钮，`Esc`/遮罩/取消返回取消。
- 危险操作默认焦点在安全按钮；危险按钮错误红底，普通按钮强调色底；关闭后焦点回发起控件。
- 确认框在项目切换时保持挂起，目标对象由发起时确定；面板页签菜单与重命名弹窗在项目切换时关闭。

### 错误边界

边界区域：应用顶层、标题栏、主区、侧栏、通知区、每个面板。面板渲染走 `PanelRender::Err(PanelError)`；`PanelHost` 同时以 `catch_unwind(AssertUnwindSafe(...))` 捕获未预期 panic，首次失败写入 `PanelHealth::Crashed` 并渲染：

- `面板渲染出错`、说明文字、`重试` 按钮、可折叠错误详情。`重试` 清除健康状态并重新渲染；同项目其他面板不受影响。
- 应用顶层：全屏 `应用渲染错误`、用户可读错误、可折叠诊断详情。诊断不得包含凭据、文件内容或终端输出。
- 区域失败只替换该区域；标题栏与通知区保持可用。错误边界本身不得无限递归渲染。

### 持久化与协调

逻辑记录与字段级 schema 由 `design/persistence.md` 单点定义；本节只保留所有者与写入时机：

- 项目索引、项目布局、终端簿由工作区壳写入；侧栏布局与视图状态由侧栏框架写入；应用设置由设置域写入；崩溃快照由编辑器/Markdown 写入。
- 布局变化立即写内存，去抖按 `persistence.md` 的项目布局域；终端编号/cwd 与自定义标题按终端簿域规则；关闭前强制冲刷，不等待计时器。
- 写入经 `PersistenceCoordinator` 串行队列与 generation；原子写、备份、损坏隔离与跨文件提交顺序见 `design/persistence.md`。
- 启动恢复只读，不写回；恢复产生的程序化变化不触发保存。

### 复用与重建

| 旧实现 | 处置 |
|---|---|
| `slterm_split` 分屏树、删除收编、分隔线命中、比例提交、方向导航 | 直接复用为面板组布局树；叶类型换 `PanelGroupId` |
| `terminal_registry`、`AgentHub`、终端视图/PTY 会话 | 保留引擎；键改为 `ProjectId + PanelId`，所有权挂到 `PanelHost` |
| `title_bar.rs` 的原生控制区、34px/38×26 尺寸、虚拟窗口命令分派 | 复用；标题数据源改活跃项目名，关闭接统一关闭链 |
| `session_persistence` 的 latest/saved、无变化跳过、终局冻结、失败重试 | 复用状态机思想；按多项目/多域重写为 flush coordinator |
| `pumps.rs` 的 hook 泵、屏幕看门狗、壳事件泵、托盘快照 | 复用；事件路由改项目/面板，自动保存泵替换为去抖协调器 |
| `session_recovery.rs` 的断路器、隔离、启动计划、启动身份映射 | 复用恢复策略与 `LaunchSession` 安全转换；多项目重写 |
| `notify.rs` 的应用内通知队列 | 重建为统一通知服务；系统通知、闪烁与托盘归 `platform-windows.md` |
| `runtime_api` 端点发现、单实例移交重试、控制面服务 | 复用传输与生命周期；移交方法统一 `runtime.handover`，资源模型升级为 project/panel-group/panel |
| 旧 `SltermWorkspace` tabs/active/next_pane、`WorkspaceTab`、`TabEntry`、`TabMeta` | 整体废弃；由项目目录 + 项目布局 + `PanelHost` 取代 |
| 旧单工作区脏关闭 `guard_file_tab_close`/`guard_file_window_close` | 语义参考，重建为统一 close guard 与 batch save |
| 旧 `SidebarState`/`Session.sidebar` | 侧栏框架重建，见 sidebar/framework |
