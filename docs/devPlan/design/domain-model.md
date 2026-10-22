# 领域模型

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

领域模型以“稳定身份 + 封闭类型 + 纯转换 + 显式不变量”为核心。持久化 DTO 与运行期实例严格分离：DTO 只描述重建所需事实，实例持有进程、缓冲、订阅与临时状态。

### 身份类型与分配器

```rust
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ProjectId(u64);
pub struct PanelGroupId(u64);
pub struct PanelId(u64);
pub struct TerminalOrdinal(u32);
```

- 三类 ID 均为应用级全局唯一，单调递增、终生不复用；`PanelGroupId`/`PanelId` 不按项目分段，Runtime v2、CLI 与随包 skill 可在不带项目的情况下无歧义寻址。单实例移交载荷用根路径/cwd 定位，不传内部 ID。
- `ProjectId` 在根目录重定位、显示名重命名、项目排序后保持不变；根路径唯一是不变量，不是身份。
- `TerminalOrdinal` 是项目内终端编号，从 0 开始，关闭不复用；不同项目可出现相同编号。
- 分配器：

```rust
pub struct IdAllocator {
    next_project: u64,
    next_group: u64,
    next_panel: u64,
}

impl IdAllocator {
    pub fn allocate_project(&mut self) -> ProjectId;
    pub fn allocate_group(&mut self) -> PanelGroupId;
    pub fn allocate_panel(&mut self) -> PanelId;
    /// 恢复时把游标抬到已见最大 ID 之上；绝不回退。
    pub fn observe_existing(&mut self, ids: PersistedIdCeiling);
}
```

- 恢复时 `observe_existing` 必须同时考虑状态库 `workspace.next_*`、已加载项目内的全部 ID、`recovery/` 目录名；游标统一取 `max(next_*, maxSeen + 1, maxDir + 1)`。这样状态库丢失后以空状态继续也不会与孤儿恢复目录 ID 冲突。
- 字符串形式（Runtime v2、环境变量、CLI 与随包 skill）：`project:<u64>`、`group:<u64>`、`panel:<u64>`；解析失败即 `invalid_params`，未知资源即 `resource_not_found`。前缀集合封闭，未来新增资源类型只追加前缀，不复用旧前缀语义。
- 线缆类型：协议 crate 的 `resource_id.rs` 只定义不透明字符串 `ResourceId` 及其解析/格式化/错误，不重复定义 `ProjectId`/`PanelGroupId`/`PanelId`；领域 newtype 唯一定义于本文件（`slterm_workspace::ids`）。完整线缆合同见 `integrations/runtime-api-v2.md`。

### 项目与项目注册表

```rust
pub struct ProjectRegistry {
    entries: Vec<ProjectEntry>,
    active: Option<ProjectId>,
}

pub struct ProjectEntry {
    pub id: ProjectId,
    pub root: ProjectRoot,
    pub display_name: String,
}

/// 恢复/快照聚合体。运行期不整体驻留于 Workspace：
/// entry 归 ProjectRegistry，layout/terminals 移交给 ProjectView。
pub struct ProjectRecord {
    pub entry: ProjectEntry,
    pub layout: ProjectLayout,
    pub terminals: TerminalBook,
}

pub struct ProjectRoot {
    /// 归一化词法路径：绝对、`/` 分隔、大小写折叠、去尾分隔符（盘符根除外）。身份比较用它。
    lexical: String,
    /// 创建/重定位时解析的真实路径（符号链接/目录联接目标）；包含性校验用它。
    real: Option<String>,
}
```

注册表不变量与操作：

- 项目根词法路径唯一；添加已存在根路径返回 `AddProjectOutcome::Existing(id)`，由导航树激活并提示。
- `active` 非空当且仅当 `entries` 非空；移除活跃项目后，按列表顺序取下一个，没有则取上一个，没有其他项目则为 `None`。
- 显示名可重复；创建时与现有显示名冲突则追加递增数字后缀（`demo`、`demo 2`、`demo 3`）；重命名不再追加。
- 排序只改变 `Vec` 顺序；活跃项目、布局、ID 不变。
- 根重定位：

```rust
pub struct RelocationPlan {
    pub root: ProjectRoot,
    pub affected_panels: Vec<PanelPathChange>,
    pub terminals_keep_cwd: bool,          // 恒 true；既有终端不重启
    pub new_terminal_base: PathBuf,        // 新根
}

pub fn relocate(entry: &ProjectEntry, layout: &ProjectLayout,
                new_root: ProjectRoot) -> Result<RelocationPlan, WorkspaceError>;
```

  - 项目布局树、面板组、面板实例不重建。
  - `project_relative` 路径按新根重新解析；新根外文件面板转为根外只读或触发错误态，由面板规格决定。
  - `absolute`/`repo_relative` 路径不变；Diff/GitShow 的仓库根按新根重新解析。
  - 既有终端保持原 cwd；新终端与文件浏览器使用新根。
  - 重定位只提交 `projects.root`；布局 JSON 中的相对路径无需改写。

### 项目布局、面板组与面板

```rust
pub struct ProjectLayout {
    pub tree: Option<SplitTree<PanelGroupId>>,  // None = 空布局；Some 为 slterm_split 的几何树
    pub groups: BTreeMap<PanelGroupId, PanelGroup>,
    pub panels: BTreeMap<PanelId, PanelRecord>,
    pub focused_group: Option<PanelGroupId>,
}

pub struct PanelGroup {
    pub id: PanelGroupId,
    /// 页签顺序；唯一权威。
    pub panels: Vec<PanelId>,
    /// 非空组恰好一个激活面板。
    pub active: PanelId,
}

pub struct PanelRecord {
    pub id: PanelId,
    pub params: PanelParams,
}
```

归属关系：

- `ProjectLayout` 拥有全部 `PanelRecord`；`PanelGroup.panels` 只持 ID，不持有记录副本。
- 每个面板恰好属于一个面板组；每个面板组恰好是布局树的一个叶；布局树叶集合 = `groups` 键集合。
- 空面板组不存在：最后一个面板关闭或被拖出后，组从树与 `groups` 中移除；父节点按 `slterm_split::RemoveOutcome` 塌缩，比例按剩余空间归一。
- 每个含面板的项目布局恰好一个聚焦面板组；布局为空时 `focused_group = None`。
- 面板不跨项目移动。跨项目使用同一文件时各项目持有独立 `PanelRecord` 与实例。

布局操作纯函数：

```rust
impl ProjectLayout {
    pub fn normalize(&mut self) -> Vec<Repair>;
    pub fn validate(&self) -> Result<(), LayoutInvariantError>;
    pub fn split_group(&mut self, anchor: PanelGroupId, new: PanelGroupId,
                       direction: SplitDirection, ratio_permille: u16) -> Result<(), LayoutError>;
    pub fn remove_group(&mut self, id: PanelGroupId) -> RemoveOutcome<PanelGroupId>;
    pub fn move_panel(&mut self, panel: PanelId, target: PanelGroupId, index: usize) -> Result<(), LayoutError>;
    pub fn close_panel(&mut self, panel: PanelId) -> ClosePanelOutcome;
    pub fn cycle_focus(&self, from: PanelId, forward: bool) -> Option<PanelId>;
}
```

比例持久化用千分比整数（`1..=999`），运行时转 `f32`；转换是纯函数，避免 f32 序列化噪声。`slterm_split::SplitTree` 的 `preview_ratio`/`dragging` 是交互态，不进 DTO。

### 面板封闭枚举与参数 schema

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum PanelKind {
    Terminal,
    Editor,
    Markdown,
    Diff,
    GitShow,
    Settings,
}

#[derive(Clone, PartialEq)]
pub enum PanelParams {
    Terminal(TerminalParams),
    Editor(EditorParams),
    Markdown(MarkdownParams),
    Diff(DiffParams),
    GitShow(GitShowParams),
    Settings(SettingsParams),
}
```

每个参数结构带独立 `revision: u32`（首版为 1）。字段级 schema：

```rust
pub struct TerminalParams { pub revision: u32 }

pub struct EditorParams {
    pub revision: u32,
    pub path: PersistedPath,           // 只读判定只在 PersistedPath.read_only
    pub word_wrap: bool,               // 新面板取设置默认值
    pub language_override: Option<LanguageId>,
}

pub struct MarkdownParams {
    pub revision: u32,
    pub path: PersistedPath,           // 只读判定只在 PersistedPath.read_only
    pub view: MarkdownView,            // Rendered | Source
    pub outline_open: bool,
    pub zoom_percent: u16,             // 25..=400，默认 100
    pub scroll_anchor: Option<MarkdownAnchor>,
}

pub struct MarkdownAnchor { pub anchor: String, pub offset_px: f32 }

pub struct DiffParams {
    pub revision: u32,
    pub head_path: RepoPath,            // 仓库相对，大小写不敏感比较
    pub workspace_path: PersistedPath,
    pub view: DiffView,                // SideBySide | Inline
    pub word_wrap: bool,
    pub sync_scroll: bool,
    pub split_permille: u16,           // 100..=900，默认 500
    pub collapsed_side: Option<DiffSide>,
    pub swapped: bool,
}

pub struct GitShowParams {
    pub revision: u32,
    pub repo_path: RepoPath,
}

pub struct SettingsParams {
    pub revision: u32,
    pub selected_page: Option<SettingsPageId>, // None = 当前没有可用配置页；默认“快捷键”
    /// 页内参数；类型、清洗与保存归 `panels/settings.md`，随布局持久化。
    pub page_params: BTreeMap<SettingsPageId, PageParams>,
}

/// 页面自有的不透明参数；schema 由页面注册表声明，工作区只存取、不解释。
pub struct PageParams(pub serde_json::Value);
```

排除项（运行期状态，不持久化）：Diff 的“忽略行尾空白/忽略全部空白/折叠未变更区域”、编辑器代码折叠、手动只读、搜索状态、Markdown 查找栏、终端状态圆点/CLI 图标、GitShow 查找/换行、所有滚动位置（Markdown 滚动锚点例外，属于持久参数）。

版本规则：

- 状态库 `user_version` 与当前版本不等（含更高或更低）即不兼容；不提供升级链。
- 面板参数 `revision` 高于当前支持：丢弃该面板并聚合警告；低于当前：首版无旧版本，按不兼容丢弃。
- 追加可选字段必须 `serde(default)`，不升版本；字段改名、删除、语义变化必须升 `revision`。

路径描述：

```rust
pub enum PathScope { ProjectRelative, RepoRelative, Absolute }

pub struct PersistedPath {
    pub scope: PathScope,
    pub value: String,
    pub read_only: bool,       // 唯一只读标记；打开/恢复时按当前允许根重算
}

pub struct RepoPath { pub value: String }  // 相对仓库实际根
```

- `ProjectRelative` 在根重定位时按新根解析；`RepoRelative` 在仓库根变化时重新解析；`Absolute` 原样使用。
- 恢复时重新执行允许根校验：项目根 + 已登记 Agent 全局目录（`AgentRoot` 作用域）可写；仓库根内、项目根外只读；仓库根外一律只读。系统另存为授权只绑定当前运行期，不进入布局；Agent 根可写性是运行期判定，不新增持久化字段。
- `PersistedPath.read_only` 是只读判定的唯一持久化字段；`EditorParams`/`MarkdownParams` 不重复携带。
- 路径比较用 `PathKey`：真实路径规范化后的 Windows 大小写不敏感键；显示路径保留原始大小写。

### 终端编号与终端簿

```rust
pub struct TerminalBook {
    pub next_ordinal: u32,
    pub entries: BTreeMap<PanelId, TerminalEntry>,
}

pub struct TerminalEntry {
    pub ordinal: TerminalOrdinal,
    pub custom_title: Option<String>,   // <= 128 字符，去除首尾空白
    pub cwd: Option<String>,            // 最近已知目录；恢复时校验
}

impl TerminalBook {
    pub fn allocate(&mut self, panel: PanelId) -> TerminalOrdinal;
    pub fn remove(&mut self, panel: PanelId);
    pub fn reconcile(&mut self, layout: &ProjectLayout, root: &ProjectRoot) -> Vec<Repair>;
}
```

- 创建终端：先分配编号，再创建面板；关闭只删条目，不回退计数。
- `TerminalBook` 是终端自定义标题、编号与最近 cwd 的唯一持久化权威；项目布局 `layout_json` 不保存这些字段，也不保存编号计数。
- 恢复：布局中的终端面板按状态库 `project_terminals` 的 `TerminalBook` 取编号；缺失条目从 `next_ordinal` 递增分配；重复编号保留最小者，其余重分配；孤儿条目丢弃。恢复不以布局重算编号。
- 自定义标题立即持久化；编号与 cwd 去抖 300ms。
- 标题展示：自定义标题 > 运行中 CLI 会话标题 > 历史会话标题 > `terminal-N`。

### 去重键与标题

```rust
pub enum DiffMode { HeadToWorktree, ConflictSnapshot }

pub enum DedupeKey {
    Editor { project: ProjectId, path: PathKey },
    Markdown { project: ProjectId, path: PathKey },
    Diff { project: ProjectId, workspace: PathKey },
    GitShow { project: ProjectId, repo_path: CaseFoldKey },
    Settings { project: ProjectId },
}

impl PanelParams {
    pub fn dedupe_key(&self, project: ProjectId) -> Option<DedupeKey>;
}

pub struct DedupeIndex { /* key -> PanelId */ }
```

- 终端返回 `None`，每次新建独立面板。
- 编辑器与 Markdown 的键为“项目 + 规范化真实路径”；默认 Diff 为“项目 + 规范化工作区路径”，来源与模式不参与去重；`DiffMode` 只作运行期判别，不进入键。GitShow 为“项目 + 仓库相对路径”，大小写不敏感；设置中心为项目单例。
- 冲突快照是瞬态 Diff 面板：有独立 `PanelId`，不参与去重、不持久化、不入布局，重启消失；其去重按“项目 + 源面板 + 快照代次”在运行期完成，不进入 `DedupeIndex` 的持久键。
- 命中去重键：激活所属项目/面板组/面板并直接聚焦；不产生一次性焦点意图。
- 标题计算：

```rust
pub fn compute_titles(layout: &ProjectLayout, project_root: &ProjectRoot,
                      runtime: &TitleRuntime) -> BTreeMap<PanelId, String>;
```

  - 文件面板默认文件名；同项目同名（大小写不敏感）改用项目相对路径；项目根外使用规范化绝对路径。
  - 编辑器与 Commit 来源的文件打开路由（Markdown/编辑器）不加来源后缀；Diff 追加 `(git diff)`；GitShow 追加 `(git delete)`；冲突快照追加 `(冲突差异)`（瞬态面板，不入布局）。
  - 设置中心固定 `设置中心`。
  - 终端按标题优先级；终端编号与自定义标题来自 `TerminalBook`。

### 焦点模型

焦点分层：活跃项目 → 聚焦面板组 → 激活面板 → 面板内焦点落点。`FocusIntent`/`FocusMemory` 的类型与消费规则归 `ux-framework.md`；本文件只保留 `FocusTarget` 与关闭/循环纯函数。`ProjectView.focus_memory` 是 `FocusMemory` 的唯一属主，`Workspace` 不持有第二份。

```rust
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FocusTarget {
    TerminalInput,
    EditorText,
    MarkdownRendered,
    MarkdownSource,
    DiffWorkspace,
    DiffReadOnlySide,
    GitShowContent,
    SettingsContainer,
    PanelContainer,
    Workspace,
}

pub enum FocusTransfer {
    Panel { group: PanelGroupId, panel: PanelId, target: FocusTarget },
    Workspace,
}

pub fn focus_after_close(layout: &ProjectLayout, closed: PanelId) -> FocusTransfer;
pub fn focus_cycle(layout: &ProjectLayout, from: PanelId, forward: bool) -> Option<PanelId>;
```

规则：

- `ProjectLayout.focused_group` 是持久化的“上次聚焦组”，用于重启恢复布局层级；不产生键盘焦点。
- `ProjectView.focus_memory` 是运行期“上次聚焦组/面板”的唯一副本，用于切换项目；不持久化、不跨重启。
- 关闭聚焦面板：同组下一个 → 同组上一个 → 相邻面板组激活面板 → 工作区层级。相邻组按布局树深度优先叶序。
- `Ctrl+Tab` 循环按活跃项目的面板组树序与组内页签序；无聚焦面板时不消费。
- 单面板显式打开/激活产生一次性焦点意图；命中去重键的聚焦是立即聚焦。
- 布局恢复/启动恢复不产生任何焦点意图。

### 纯转换与恢复对账

`WorkspaceDto`、`ProjectLayoutDto`、`TerminalsDto` 作为本地状态库的 JSON 载荷持久化；状态库 schema 与事务归 `persistence.md`，本文件定义 DTO 字段语义、恢复对账与不变量。

```rust
pub struct ProjectViewState {
    pub layout: ProjectLayout,
    pub terminals: TerminalBook,
}

pub fn snapshot_project(entry: &ProjectEntry, state: &ProjectViewState)
    -> (ProjectLayoutDto, TerminalsDto);
pub fn restore_project(entry: ProjectEntry, layout: ProjectLayoutDto, terminals: TerminalsDto,
                       ctx: &RestoreContext) -> (ProjectViewState, RestoreReport);
pub fn snapshot_registry(reg: &ProjectRegistry, ids: &IdAllocator) -> WorkspaceDto;
pub fn restore_registry(dto: WorkspaceDto) -> (ProjectRegistry, IdAllocator, RestoreReport);
```

持久化 DTO 合同：

```rust
pub struct WorkspaceDto {
    pub schema_version: u32,
    pub revision: u64,
    pub active_project: Option<u64>,
    pub projects: Vec<ProjectEntryDto>,   // 数组顺序 = 导航树顺序
    pub next_project: u64,
    pub next_group: u64,
    pub next_panel: u64,
}

pub struct ProjectEntryDto {
    pub id: u64,
    pub root_lexical: String,
    pub display_name: String,
}

pub struct ProjectLayoutDto {
    pub schema_version: u32,
    pub revision: u64,
    pub tree: Option<SplitTreeDto>,       // slterm_split 的序列化树，叶为 group id
    pub groups: Vec<PanelGroupDto>,
    pub panels: Vec<PanelRecordDto>,
    pub focused_group: Option<u64>,
}

pub struct PanelGroupDto { pub id: u64, pub panels: Vec<u64>, pub active: u64 }
pub struct PanelRecordDto { pub id: u64, pub kind: PanelKindDto, pub params: PanelParamsDto }

pub struct TerminalsDto {
    pub schema_version: u32,
    pub revision: u64,
    pub next_ordinal: u32,
    pub entries: Vec<TerminalEntryDto>,
}

pub struct TerminalEntryDto {
    pub panel_id: u64,
    pub ordinal: u32,
    pub custom_title: Option<String>,
    pub cwd: Option<String>,
}
```

- 三套 DTO 的 `schema_version` 必须等于 `state.db.user_version`；不等即整库不兼容。`revision` 是载荷结构版本（首版为 1），恢复时高于当前支持即不兼容；它不是行级并发令牌，并发由单写线程与事务保证。
- `ProjectEntryDto` 只持久化词法根路径；真实路径在恢复时按当前磁盘重新解析，不进入 DTO。
- `PanelKindDto` 是 `PanelKind` 的 snake_case 字符串；`PanelParamsDto` 使用 `#[serde(tag = "kind", content = "params", rename_all = "snake_case")]`，变体字段与对应的 `TerminalParams`/`EditorParams`/`MarkdownParams`/`DiffParams`/`GitShowParams`/`SettingsParams` 完全一致，并保留各自 `revision`。
- 容器一律数组，不依赖 JSON map 键；数组顺序是语义：`projects` 是导航树顺序，`PanelGroupDto.panels` 是页签顺序。重复 ID 保留先出现者，后者丢弃并记入 `RestoreReport`；`TerminalsDto.entries` 按 `panel_id` 去重。
- `serde` 统一 `deny_unknown_fields`；新增可选字段用 `serde(default)`，不升 `revision`；字段改名、删除或语义变化必须升对应参数 `revision`。
- 运行期到 DTO 的映射是纯函数：`PanelRecord.params` 是唯一权威，`PanelGroup.panels` 只存 ID；`TerminalBook` 只进 `TerminalsDto`，不复制进布局。`WorkspaceDto` 不保存布局与终端簿，`ProjectLayoutDto`/`TerminalsDto` 按项目分表存储。

恢复对账顺序（幂等）：

- 状态库 `user_version` 与完整性由 `persistence.md` 校验；本层只接收已通过版本校验的载荷。
- ID 解析与全局唯一性校验；重复的 `PanelGroupId`/`PanelId` 保留先出现者，后者丢弃。
- 丢弃未知 `PanelKind`、超版本 `revision`、缺失必需字段的面板。
- 丢弃引用不存在面板的组条目；丢弃不在树上的组；删除未被任何 `PanelGroup.panels` 引用的 `PanelRecord`；丢弃空组；按 `slterm_split` 规则塌缩树。
- 修正 `active`（不在组内 → 首面板）、`focused_group`（不在树 → 树首叶；无组 → `None`）。
- 终端簿对账：补编号、去重复、丢孤儿、校验 cwd（不存在/越界 → `None`，运行时回退项目根并提示）。
- 去重冲突：同键多个面板保留布局顺序靠前者，其余丢弃并警告。
- 设置中心每项目至多一个；多余丢弃。
- 返回 `RestoreReport`，由启动聚合为一条警告。

快照方向（运行期 → DTO）只取持久参数，不读取 body 的缓冲、光标、滚动或临时态；因此恢复结果不会把运行期噪声写回磁盘。

### 不变量清单

- 项目集合非空 ⇒ 活跃项目恰好一个；项目集合为空 ⇒ 无活跃项目。
- 项目根词法路径唯一；`ProjectId` 全局唯一且不复用。
- 布局树叶集合 = 面板组集合；每个组非空；每个组 `active ∈ panels`。
- 每个面板记录恰好被一个组引用；每个面板组恰好被树引用一次。
- 含面板的项目布局恰好一个聚焦组；无面板则无聚焦组。
- 每个项目至多一个终端自定义标题相同的约束不存在（允许重名标题）；但终端编号在项目内唯一。
- 每个项目至多一个设置中心面板。
- 编辑器/Markdown 同项目同路径至多一个面板；Diff 默认模式同项目同工作区路径至多一个（冲突快照不计入去重约束）；GitShow 同项目同仓库路径至多一个。
- 面板不跨项目移动；跨项目拖拽被拒绝。
- DTO 只含路径、参数、标题与编号；不含凭据、文件内容、终端输出、剪贴板内容。

### Runtime v2 资源 ID 前向合同

- 资源模型：`project` / `panel-group` / `panel`；方法参数与事件字段一律使用字符串资源 ID，不再使用页码/下标寻址。
- ID 形式：

```json
{ "project": "project:7", "panel_group": "group:12", "panel": "panel:31" }
```

- 合同条款：
  - ID 不透明：客户端不得解析数值做排序、范围或归属推断。
  - ID 终生稳定：重命名、排序、面板组拆分/合并、项目根重定位均不改变。
  - 不复用：删除后的 ID 永不再分配。
  - 前缀封闭：新增资源类型只追加新前缀；旧前缀语义不变。
  - 未知前缀/未知 ID → `resource_not_found`；格式不合法 → `invalid_params`。
  - 事件与快照同时携带资源 ID 与必要父 ID（`project` 可选，因为 `panel` 全局唯一）。
- 环境变量：`SLTERM_PROJECT_ID`、`SLTERM_PANEL_GROUP_ID`、`SLTERM_PANEL_ID` 使用同一字符串；`SLTERM_PANE_ID` 随 v1 退役，不保留兼容别名。
- CLI `slterm ctl` 与随包 skill 使用同一 ID 形式；单实例移交载荷用根路径/cwd 定位，不传内部 ID。
- 资源 ID 不是授权凭据；权限仍由本机 token 与单实例边界决定。

### 面板类型注册表扩展规则

- `PanelKind` 与 `PanelParams` 是封闭枚举；新增类型必须同时修改：
  - `slterm_workspace::panel` 的枚举与参数结构；
  - DTO tag 与 `revision`；
  - `slterm_workspace::dedupe` 的去重键；
  - `slterm_app::app::panel_registry` 的单一注册项：`kind`、工厂、能力、标题策略；
  - 恢复过滤与 `RestoreReport` 测试；
  - `docs/featureSpec/workspace/shell.md` 的面板类型表与验收。
- 注册项类型（`PanelDescriptor`、`PanelCapabilities`）、工厂上下文与焦点接缝归 `workspace/shell.md` 与 app 注册点所有；本文件只定义领域枚举与参数。core 不引用 `PanelHost`/`PanelCreateContext` 等 app 类型。
- 未注册类型在恢复时丢弃并聚合警告；运行时不能创建。
- 禁止面板类型由 UI 侧私增；禁止用“通用文档面板 + 模式字段”绕过封闭集。
