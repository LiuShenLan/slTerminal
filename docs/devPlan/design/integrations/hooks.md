# Hooks 引擎

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

### 职责与边界

本域负责：Hook 事件与 handler 能力目录、schema 校验、User/Project/Local 三层配置文档、GUI/JSON 双模式编辑模型、保存事务、注入与卸载、托管条目归属、外部修改检测、文件锁与上一代备份。

不负责：Hook 事件运行时接收与状态投影（Agent CLI 域）、设置中心页面布局与导航（设置中心）、终端状态四态（终端面板）、安装偏好持久化（设置存储）。

### 决策与红线

| 规则 | 原因 | 边界 |
|---|---|---|
| 事件目录、handler 目录、GUI 可选项、schema 与保存校验共用同一份 `HookCatalog` | 规格禁止“仅 JSON 可写/仅保存可接受”的隐藏子集 | 新增事件/字段只改目录；schema 由目录派生并有等价性测试 |
| 目录外事件不能通过 GUI 添加，也不能通过 JSON 保存 | 目录即合同 | 未知事件键校验失败；未知字段按对象策略保留或拒绝 |
| 保存只替换目标文件的 hooks 子树，其他字段语义原样保留 | 用户配置文件还承载权限、env、`$schema` 等 | 不做整文件模板重写；不改变顶层其他键 |
| 写前备份、锁、外部修改检测、原子替换四者缺一不可 | 外部编辑器可能同时改同一文件 | 任一失败中止，原文件保持完整，未保存状态保留 |
| User 层保存必须显式确认；Project/Local 直接保存 | hooks 可执行任意命令，用户级影响全部项目 | 引擎校验 `user_confirmed`，不只依赖 UI |
| 注入以用户级设置文件写入为提交点 | 避免“配置已写、脚本未就绪”或反之 | 提交前失败回滚已准备资产；提交后清理失败可见并可重试 |
| 托管条目按精确命令/归属指纹认领，绝不按产品名子串 | 用户可能写包含 slterm 字样的自定义命令 | 被用户编辑过的托管文件报 Conflict 并保留；GUI 托管 handler 只读 |
| 卸载遇到损坏 JSON 整体失败 | 删除脚本但配置残留会造成悬空状态 | 保留脚本与信号目录；提供打开文件位置与备份恢复 |
| 外部修改只提示不自动合并；保存冲突始终独立检测 | 静默覆盖会丢用户修改 | 监听提示与保存前检测并存 |

### 能力目录

```rust
pub struct HookCatalog {
    pub id: CatalogId,
    pub events: &'static [HookEventSpec],
    pub handler_types: &'static [HookHandlerSpec],
}

pub struct HookEventSpec {
    pub name: &'static str,
    pub group: HookGroup,
    pub supports_matcher: bool,
    pub handler_types: HandlerTypeMask,
}

pub struct HookHandlerSpec {
    pub kind: HookHandlerType,
    pub required_fields: &'static [&'static str],
    pub optional_fields: &'static [&'static str],
    pub common_fields: &'static [&'static str],
    pub extras: ExtraFields, // Preserve | Reject
}

pub enum HookHandlerType { Command, Http, McpTool, Prompt, Agent }
```

Claude 目录（首个内置实现）：

| 分组 | 事件 |
|---|---|
| 会话生命周期 | SessionStart、SessionEnd、Setup |
| 用户交互 | UserPromptSubmit、UserPromptExpansion |
| 工具调用 | PreToolUse、PermissionRequest、PermissionDenied、PostToolUse、PostToolUseFailure、PostToolBatch |
| 通知与消息 | Notification、MessageDisplay |
| 子代理与任务 | SubagentStart、SubagentStop、TaskCreated、TaskCompleted、TeammateIdle |
| 上下文管理 | PreCompact、PostCompact |
| 停止与错误 | Stop、StopFailure |
| 配置与文件变更 | ConfigChange、CwdChanged、FileChanged、InstructionsLoaded、DirectoryAdded |
| 工作树 | WorktreeCreate、WorktreeRemove |
| 启发式交互 | Elicitation、ElicitationResult |

handler 字段：

| 类型 | 必填 | 可选 |
|---|---|---|
| command | command | args、async、asyncRewake、shell |
| http | url | headers、allowedEnvVars |
| mcp_tool | server、tool | input |
| prompt | prompt | model、continueOnBlock |
| agent | prompt | model |
| 通用 | — | if、timeout、statusMessage |

- 每个事件的 handler 类型集合由目录声明，GUI 下拉按集合过滤；支持集合可为全部类型、command/http/mcp_tool 或 command/mcp_tool。
- matcher：仅目录声明支持 matcher 的事件显示输入框；不支持时显示 `全匹配（该事件不支持 matcher）` 并省略输入框。
- `ExtraFields::Preserve` 用于 handler 详情对象：未知字段原样保留并写回；事件名与 handler 类型本身必须命中目录，否则 schema 失败。未知顶层 `hooks` 之外的字段不参与校验，保存时原样保留。
- 其他 CLI：在描述表声明自己的 `CatalogId`、层级路径、配置形态（JSON hooks 子树、TOML、插件/扩展文件）与是否提供配置编辑器；没有目录的 CLI 不提供编辑面，显示 `该 CLI 未提供配置编辑器`。

### Schema 校验

```rust
pub struct Diagnostic {
    pub path: String,          // JSON Pointer 风格路径，供 UI 定位
    pub code: DiagnosticCode,  // Syntax | UnknownEvent | UnknownHandlerType | MissingField | WrongType | ExtraField
    pub message: String,
}

pub fn validate_hooks(value: &serde_json::Value, catalog: &HookCatalog) -> Result<HooksTree, Vec<Diagnostic>>;
pub fn json_schema(catalog: &HookCatalog) -> serde_json::Value;
```

- `validate_hooks` 先校验根为对象、`hooks` 为对象；再逐事件匹配目录；再逐 matcher 组与 handler 校验类型、必填字段、字段类型与额外属性策略。
- `json_schema` 从同一目录派生，供 JSON 模式诊断与文档/测试使用；测试断言「schema 可接受的配置 == `validate_hooks` 可接受」。
- 诊断按源顺序返回；UI 显示首条并保留完整列表供悬停。保存只接受零诊断结果。JSON 诊断在停止输入 200ms 后异步更新；超过 1 MiB 的配置显示性能提示，诊断不阻塞前台。
- JSON 语法错误在解析时产生 `Syntax`，不进入目录校验；保留最后一次合法模型，禁用 GUI 切换与保存。

### 三层文档模型

```rust
pub enum HookLayer { User, Project, Local }

pub struct HookLayerPaths {
    pub user: PathBuf,
    pub project: Option<PathBuf>, // 项目根不可访问时为 None
    pub local: Option<PathBuf>,
}

pub struct HookDocument {
    raw: serde_json::Value,
    hooks: HooksTree,          // 当前层规范化模型（含托管 handler）
    editable: HooksTree,       // GUI 可编辑视图（托管 handler 只读）
    snapshot: FileSnapshot,
    layer: HookLayer,
}

pub struct FileSnapshot {
    pub stamp: FileStamp,
    pub content_hash: [u8; 32],
}

impl HookDocument {
    pub fn load(path: &Path, layer: HookLayer, catalog: &HookCatalog) -> Result<Self, HookLoadError>;
    pub fn render_json(&self) -> String;
    pub fn apply_json(&mut self, text: &str) -> Result<(), Vec<Diagnostic>>;
    pub fn apply_gui(&mut self, tree: HooksTree) -> Result<(), Vec<Diagnostic>>;
    pub fn save(&mut self, req: SaveRequest) -> Result<SaveReceipt, SaveError>;
}
```

- 路径：Claude 的 User = `~/.claude/settings.json`；Project = `<项目根>/.claude/settings.json`；Local = `<项目根>/.claude/settings.local.json`。Project/Local 使用设置中心所属项目的根，不使用活跃项目。
- 初始层级为 User；工具栏固定显示 `优先级：Local > Project > User`。层级优先级只作展示提示；编辑始终只读写当前层。注入始终写 User 层。
- 所属项目根不可访问时 Project/Local 禁用并提示 `项目根不可访问`；正在查看不可访问层时回退 User 并重新读取。
- 读取：文件不存在或没有 hooks 内容按空配置处理；JSON 非法进入 `Corrupt`，不进入编辑界面；同目录存在上一代备份时提供恢复动作。
- 文件访问：User 层位于 Agent 全局目录，Project/Local 位于项目根，均经 app 服务层的允许根、原子写与外部修改基线；Hooks 引擎不自建第二套读写通道。
- 切换层级：重新读取目标层；当前层有未保存修改时先按未保存守卫确认；读取期间编辑区显示加载态（轻量重读保留原内容）。
- 项目根不可访问：Project/Local 禁用并提示；正在查看不可访问层时回退 User。

### GUI/JSON 双模式状态机

```rust
pub enum EditorMode { Json, Gui }

pub enum EditorState {
    Loading,
    Ready { mode: EditorMode, document: HookDocument, dirty: bool },
    Invalid { mode: EditorMode, document: HookDocument, diagnostics: Vec<Diagnostic> },
    Corrupt { message: String, backup_available: bool },
    ReadOnly { message: String },
}
```

状态迁移：

| 当前 | 触发 | 结果 |
|---|---|---|
| Ready(Gui) | GUI 增删事件/matcher/handler 或改字段 | 更新模型，重新渲染 JSON 文本；dirty=true |
| Ready(Gui) | 切换 JSON | 直接用当前模型渲染文本；不丢改动 |
| Ready(Json) | 文本解析 + 目录校验成功 | 更新模型与编辑视图；dirty 由内容比较决定 |
| Ready(Json) | 解析或校验失败 | 保留最后一次合法模型；GUI 切换与保存禁用；诊断可见 |
| Ready(Json) | 校验成功但含未知保留字段 | 模型保留未知字段；GUI 容错展示；保存原样写回 |
| 任意 Ready | 层级切换/外部重载且 dirty | 确认丢弃或取消；取消保持当前状态 |
| 任意 Ready | 保存成功 | dirty=false；刷新 snapshot；显示重启提示 |
| Corrupt | 从备份恢复并二次确认 | 写回备份后重新加载 |
| ReadOnly | 目录/文件权限恢复 | 重新加载 |

- GUI 事件树只展示目录中事件；添加下拉列出尚未配置的事件；handler 类型下拉按事件支持集合过滤；删除事件/组/handler 立即更新 JSON 文本。
- 托管 handler 显示 `slTerminal 托管` 徽标，全部字段只读、删除按钮禁用；所在 matcher 组与事件的删除按钮同样禁用。
- 必填缺失显示 `此字段为必填`；数组/对象类字段非法草稿不向上更新模型；切换 handler 类型保留通用字段与适用字段，清除不适用字段。
- 配置页重新挂载时模式重置为 JSON；未保存编辑按守卫处理。

### 保存事务

```rust
pub struct SaveRequest<'a> {
    pub layer: HookLayer,
    pub document: &'a HookDocument,
    pub expected: &'a FileSnapshot,
    pub user_confirmed: bool,
}

pub enum SaveError {
    Validation(Vec<Diagnostic>),
    ConfirmationRequired,
    Corrupt { backup_available: bool },
    ExternalModified { current: FileSnapshot },
    Busy,
    BackupFailed(String),
    WriteFailed(String),
    ReadOnly,
}
```

保存顺序：

1. 校验目录与 JSON 对象；零诊断才继续。
2. User 层要求 `user_confirmed == true`，否则返回 `ConfirmationRequired`。
3. 取得目标文件专属跨进程锁（锁键 = 规范化绝对路径的哈希；锁位于应用数据目录，句柄型，进程崩溃自动释放）。取不到返回 `Busy`，不等待 UI。
4. 重读目标文件并解析；解析失败返回 `Corrupt`；计算当前 `FileSnapshot`。
5. 与 `expected` 比较（stamp + 内容哈希）；不一致返回 `ExternalModified`，提供当前快照供“重新加载”。
6. 文件存在时创建上一代备份（同目录 `<文件名>.slterm-bak`，只保留一代）；备份失败中止，零写入。
7. 只替换 `hooks` 子树：保留顶层其他键与值，保留托管 handler 与未知可保留字段；JSON 重新序列化，不保留原始空白格式。
8. 原子替换目标文件；失败时原文件不变，临时文件清理。
9. 更新 snapshot，返回 `SaveReceipt { snapshot, restart_required: true }`。

- 保存成功后工具栏显示 `hooks 改动需重启 claude 会话生效`；下一次编辑或重新加载后消失。
- 保存失败保留 dirty 与完整诊断；不得只写日志。
- 注入/卸载与保存共用同一目标锁：同一文件同一时刻最多一个写操作。

### 注入与卸载

```rust
pub enum InjectionStatus { Injected, NotInjected, Outdated { reason: OutdatedReason } }

pub struct InjectionPlan {
    pub settings_path: PathBuf,
    pub managed_groups: serde_json::Value,
    pub statusline: Option<StatuslineBridge>,
    pub staged_files: Vec<StagedFile>,
    pub backup: StatuslineBackup,
}

pub enum InstallOutcome {
    Installed,
    Current,
    Suspended { suspicious_command: String },
    Outdated { reason: OutdatedReason },
    Conflict { path: PathBuf },
}
```

Claude 注入：

- 在 User 层 `hooks` 中写入本应用订阅事件的 command handler，命令精确指向 `slterm-hook.exe`，并携带 source 判别参数；同一事件保留用户其他 handler 与 matcher 组。
- statusline：读取原 `statusLine.command` 并备份到应用数据目录；若命中可疑模式（下载器或 PowerShell 任意执行类命令），暂停注入、零写盘，设置页显示命令原文与 `确认注入`/`取消`；确认后第二次注入恒放行。
- 桥接命令透传原 statusline 输出，同时把上下文窗口用量写入信号目录；信号目录、脚本目录与备份都限制在应用托管数据目录。
- 托管识别：handler 命令必须精确解析为 `slterm-hook.exe` 的 argv 形态；同时维护归属 marker 文件（记录已安装条目与内容指纹）。不因命令包含 `slterm` 子串而认领。
- 注入状态查询只返回 `Injected`、`NotInjected`、`Outdated`。`Outdated` 来源：脚本内容/版本与内置模板不一致、hooks 存在但 statusline 桥接缺失、marker 指纹不匹配。
- 状态查询失败返回 `Err(StatusQueryFailed)`：UI 显示 `注入状态查询失败` 与重试；若已有上次已知值，显示该值并标注状态未知，不仅记录日志。
- 提交点 = 用户级设置文件原子写成功。提交前失败：设置文件不变，已准备的脚本与备份回滚；提交后清理失败：设置文件保持已提交，显示可见警告与 `重试清理`，不得伪装成注入失败。
- 注入/卸载完成后刷新状态并自动重读 User 层；当前在其他层则切换到 User 层（未保存守卫照常生效）。

卸载：

- 只移除本应用注入的 handler 组与 statusline 桥接；用户 handler 保留。
- 删除注入脚本、信号目录与状态行备份；若 statusline 仍是本应用桥接，则还原备份；备份缺失时移除 statusline 配置。
- 目标设置文件不是合法 JSON：整体失败，脚本/信号目录全部保留，错误提示提供 `打开文件位置`；存在上一代备份时提供 `从备份恢复`。
- 卸载同样以设置文件写入为提交点。

其他 CLI：

- 配置形态由 `HookProviderSpec` 声明：JSON hooks 子树、TOML `hooks`/`notify` 键、插件文件（JS）、扩展文件（TS）或通知器串联。
- 插件/扩展与 runtime skill 使用“内容全等或 marker 指纹”判定归属；用户编辑过的同名文件报 Conflict，不覆盖。
- 未声明配置编辑器的 CLI 只提供注入/卸载与状态，不提供 GUI/JSON 编辑面。

### 外部修改检测

- 触发条件：设置中心面板是其面板组当前面板、所属项目为活跃项目、面板与项目布局可见。
- 可见期间监听当前层目标文件；连续事件合并为一次提示，显示 `配置文件已在外部修改` 横幅与 `重新加载` 动作。
- 窗口从后台/最小化恢复、切回所属项目、面板页签激活：轻量重读目标 hooks 子树，不整页遮罩；有未保存修改时先确认，确认后覆盖编辑模型，取消保持现状。
- 面板隐藏、非激活或项目不活跃时停止监听，不重读、不弹提示。
- 监听提示不自动覆盖编辑模型；保存前的冲突检测独立执行，冲突时中止并提示重新加载。
- 外部修改导致 JSON 损坏：进入 `Corrupt`；存在备份时提供恢复。

### 文件锁与备份

```rust
pub struct TargetLock { /* 句柄型 lifetime lock */ }

pub fn lock_target(path: &Path) -> Result<TargetLock, LockError>;
pub fn backup_path(path: &Path) -> PathBuf; // <file>.slterm-bak
pub fn restore_backup(path: &Path) -> Result<(), RestoreError>;
```

- 锁：跨进程独占、无超时回收窗口；锁文件位于应用数据目录，键为真实路径哈希；进程崩溃由内核释放。锁冲突显示 `文件被其他程序占用，请稍后重试`。
- 备份：仅保留上一代，写前创建，备份失败中止保存；备份文件不进入 git 追踪目录以外的特殊处理，不记录内容到日志。
- 原子写：与文件服务相同的临时文件 + 原子替换；不使用普通覆盖写。
- 备份恢复：用户确认后以备份内容替换目标；恢复同样走锁与原子写。

### 错误映射

| 错误 | 用户可见 | 恢复 |
|---|---|---|
| `Syntax` / `Diagnostic` | JSON 存在错误 + 首条诊断 | 修正或保留最后合法快照 |
| `Corrupt` | 配置文件损坏，请先修复 | 从备份恢复；打开文件位置 |
| `ExternalModified` | 文件已被外部修改，请先重新加载 | 重新加载；取消保留编辑 |
| `Busy` | 文件被其他程序占用，请稍后重试 | 重试 |
| `BackupFailed` | 保存失败：无法创建备份 | 保留 dirty；重试 |
| `WriteFailed` | 保存失败：原因 | 保留 dirty；重试 |
| `ConfirmationRequired` | 确认写入用户级 hooks 配置 | 用户确认后重试 |
| `Suspended` | 检测到可疑 statusline 命令，注入已暂停 | 确认注入/取消；零写盘 |
| `UninstallBlocked` | 卸载失败，请检查设置文件 | 打开文件位置/备份恢复 |
| `CleanupPending` | 已注入，清理失败 | 重试清理；配置保持已提交 |
| `StatusQueryFailed` | 注入状态查询失败 | 重试；有上次已知值时标注状态未知 |
| `Outdated` | 注入状态：版本过旧 | 重新注入 |

### 复用、改造与新建

| 现有模块 | 处置 | 说明 |
|---|---|---|
| `slterm_app/src/ai_hook/editor.rs` | 改造并迁移 | handler 类型/字段矩阵、事件白名单、加载/保存通道复用；扩展为目录驱动、三层、schema 校验与托管只读 |
| `slterm_app/src/ai_hook/local.rs` | 改造并迁移 | statusline 审查门、claude/codex/plugin 安装器、原子写、备份、marker 归属直接复用；补注入提交点与清理重试 |
| `slterm_app/src/ai_hook/local/managed_files.rs` | 直接复用 | 内容指纹 + marker 双通道归属 |
| `slterm_app/src/ai_hook/local/runtime_skills.rs` | 直接复用 | skill 资产投放/卸载与冲突保留 |
| `slterm_app/src/ai_hook/local/config_guard.rs` | 改造 | 自愈 watcher 保留；改为使用统一目录/保存引擎与失败分类 |
| `slterm_app/src/ai_hook/local/settings.rs` | 改造 | 检视/应用入口保留；三层路径与状态查询扩展 |
| `slterm_app/src/atomic_file.rs` | 直接复用 | 原子写、lifetime lock、临时文件语义 |
| `slterm_app/src/app_shell/settings_pane/agents.rs` | 改造 | 页面控件结构可复用；数据源改为目录驱动的 GUI/JSON 模型，补三层、schema 诊断、外部修改提示、注入状态 |
| 新 `HookCatalog` / schema 派生 | 新建 | 事件与 handler 单一事实源 |
| 新三层文档/保存事务 | 新建 | snapshot 冲突、备份、锁、只替换 hooks 子树 |
| 新注入状态与 statusline 桥接 | 新建 | 提交点、清理重试、用量信号 |
| 新外部修改监听接入 | 新建 | 当前层文件监听与重读守卫 |

### 验收与测试接缝

- 目录：事件/分组/matcher/handler 类型/必填字段矩阵；GUI 与 schema 与保存校验同源；DirectoryAdded 在 GUI 可见且可保存。
- 文档：三层路径解析、缺文件空配置、损坏 JSON、其他顶层字段保留、未知可保留字段往返、JSON↔GUI 同步、非法 JSON 保留最后合法模型。
- 保存：User 确认、Project/Local 直写、锁冲突、备份失败、外部修改、写失败原子性、保存成功后仅 hooks 子树变化。
- 注入/卸载：可疑 statusline 零写盘与确认路径、提交前回滚、提交后清理重试、状态三值、卸载保留用户 handler、损坏文件卸载失败保留脚本。
- 托管：marker 指纹、内容全等、用户编辑 Conflict、GUI 只读与删除禁用、JSON 修改托管标识后按普通条目处理。
- 外部修改：虚拟时钟下去抖、可见性门控、脏/干净守卫、损坏提示、保存冲突独立检测。
- 夹具：tempdir 假 home、fixture settings JSON、故障注入（只读/占用/磁盘满/备份失败）、schema 快照；真实 CLI 安装/卸载由构建产物人工验收并登记豁免。
