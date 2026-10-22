# 安全

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有路径允许根与真实路径校验、URL/OSC 白名单、Markdown 清洗、进程启动、凭据域、威胁模型与测试矩阵；所有领域的业务写入与应用托管写入经同一安全边界的两类作用域读写外部资源。

### 决策与红线

- **默认拒绝**：路径越界、设备路径、盘符相对路径、未登记协议、脚本内容、后台剪贴板写入、未验证更新包一律拒绝；拒绝理由可见，不提供“仍然执行”的绕过入口。原因：终端输出、仓库内容与 Markdown 都是不可信输入。边界：用户显式点击的远程图片单次加载、聚焦终端 OSC 52 写入、系统另存为路径是受控例外。
- **词法身份与真实路径分离**：项目身份按词法根路径与稳定 `ProjectId` 管理；包含性校验按解析符号链接/目录联接后的真实路径。原因：项目根内的链接可以指向根外，只做字符串前缀判断会被绕过。边界：项目根自身是 UNC 时，只允许其真实子路径。
- **业务写入只允许项目根与已登记 Agent 根**：`AgentRoot` 是显式可写作用域，编辑器打开该作用域可保存；仓库实际根只提供读取；项目根外、仓库根内以只读打开。原因：Git 仓库可能包含项目根外的共享文件，写入会越过项目边界。边界：系统“另存为”对话框选定路径后，该文档在当前运行期成为可写文档，授权不持久化；Markdown 显式本地链接只读打开单个根外文件。
- **应用托管写入限定在 AppManaged 作用域**：设置、状态库、快照、备份/隔离、日志、审计、更新状态、Hooks 资产与 CLI 配置只写入登记的托管根；托管根由 `slterm_settings::paths` 在启动时登记，用户选择或项目内容路径不得声明为托管根。原因：托管写入若绕过 `PathGuard` 会形成第二套写通道。边界：托管写入与业务写入共用 `PathGuard` 判定，但使用独立的 `AuditedWrite` 入口、原子写与审计事件。
- **设置导入先校验版本**：导入文件先校验 `schema_version` 与 schema，再预览、备份、原子写；版本或 schema 不兼容时零写入。原因：版本不兼容的载荷可能被错误解释。边界：导出只含应用设置，不含 Hooks、CLI 配置、凭据或崩溃快照。
- **结构化启动，不拼 shell**：所有外部程序通过 `Command::new(program).args(args)` 或 `ShellExecuteW` 的结构化参数启动；自定义 Shell 只接受绝对路径的可执行文件，不提供参数注入。原因：字符串拼接会被文件名中的引号、空格与 `&` 注入。边界：CLI 自身的命令行解析在 CLI 进程内独立完成。
- **凭据不出域**：真实凭据只存 Windows 凭据管理器；类型系统用 `SecretRef`/`SecretValue` 隔离，DTO、布局、设置、日志、错误、崩溃快照与诊断包不得出现凭据值。原因：凭据一旦进入可序列化类型，迟早随日志或导出泄漏。边界：凭据管理器中的 target 名称可记录，用于诊断。
- **审计只记指纹**：安全写事件只经 tracing target `audit`，字段限 `operation/path/source/result` 与 `expected/actual` 指纹；不记文件内容、handler 命令文本或凭据。原因：审计日志本身是持久化面，必须按最小信息原则设计。
- **文档内容按不可信 HTML 处理**：Markdown 原始 HTML 只解析安全子集；脚本、事件属性、表单、内嵌框架、对象与嵌入元素永不执行。原因：本地文件也可能来自不受信任的仓库。边界：代码高亮使用主题 token，不执行语言插件。

### 路径允许根与真实路径校验

```rust
pub struct AllowedRoots {
    pub project: ProjectId,
    pub project_root: PathBuf,          // 词法根，可能含大小写差异
    pub repo_root: Option<PathBuf>,     // 项目根所在仓库实际根
    pub agent_roots: Vec<AgentRoot>,    // 内置与用户登记
    pub app_managed: Vec<AppManagedRoot>, // 应用托管写入；由 paths 登记
}

pub struct AppManagedRoot {
    pub kind: AppManagedKind,
    pub root: PathBuf,
    pub operations: Vec<PathAccess>,    // 该托管根的允许操作
}

pub enum AppManagedKind {
    RoamingRoot,     // <data>：设置 JSON、设置备份与隔离
    LocalRoot,       // <local>：状态库、快照、补全、用量、更新状态、恢复点
    SystemTemp,      // 原子写临时文件、诊断包与下载暂存
    Backup,          // settings.json.bak、state.db.bak、加密备份恢复点
    Quarantine,      // 设置/状态库/快照隔离件
    Logs,            // logs/
    Audit,           // audit/
    HooksAssets,     // hooks/ 注入脚本、锁与上一代备份
    CliConfig,       // 用户级 CLI 配置文件（仅 hooks 子树）
}

pub struct AgentRoot {
    pub id: AgentRootId,
    pub kind: AgentRootKind,            // Builtin / Custom
    pub display_name: String,
    pub path: PathBuf,                  // 绝对路径
    pub writable: bool,                 // 自定义节点可写；内置按登记
}

pub enum PathAccess { Read, Write, Create, Delete, Rename }

pub enum RootScope {
    Project,
    Repo,
    Agent,
    UserSelected,
    SingleFileLink,
    AppManaged(AppManagedKind),
}

pub enum PathDecision {
    Allowed { real: PathBuf, scope: RootScope, writable: bool },
    ReadOnly { real: PathBuf, scope: RootScope, reason: ReadOnlyReason },
    Denied(PathDeny),
}

pub enum PathDeny {
    NotAbsolute,
    DriveRelative,
    DevicePath,
    UncOutsideRoot,
    OutsideAllowedRoots,
    SymlinkEscape,
    GitInternal,
    InvalidName,
    ReparsePointEscape,     // 仅真实目标越界或无法解析的链接
    ManagedRootMisuse,      // 用户内容路径被声明为 AppManaged，或托管路径被当作业务内容
}

pub enum PathIntent {
    Business,                        // 项目/仓库/Agent/用户选择/单文件链接
    AppManaged(AppManagedKind),      // 应用托管写入
}

pub struct PathGuard { /* roots + platform resolver */ }

impl PathGuard {
    pub fn classify(&self, path: &Path, access: PathAccess, intent: PathIntent) -> PathDecision;
    pub fn open(&self, path: &Path, access: PathAccess, intent: PathIntent) -> Result<ValidatedPath, PathError>;
}
```

校验顺序：

1. 词法归一：拒绝空路径、相对路径、盘符相对路径（`C:foo`）、设备路径（`\\.\`、`\\?\GLOBALROOT`、命名设备）、含控制字符的路径。
2. 名称校验：单段名称拒绝空、`.`、`..`、路径分隔符、`<>:"|?*`、尾随空格/点与 Windows 保留设备名（`CON`、`NUL`、`COM1` 等）。
3. 真实路径解析：对已存在路径 `canonicalize`；对创建/重命名目标解析最近存在祖先，再拼接剩余段。符号链接与目录联接按最终目标判定。
4. 包含性比较：真实路径按 Windows 大小写不敏感规则逐组件比较；不得用字符串前缀判断（`C:\a` 不得匹配 `C:\ab`）。
5. 写路径再校验：保存链（编辑器与 Diff）在原子替换前必须重新解析真实路径；只拒绝真实目标越界的重解析点（reparse point）与符号链接（`ReparsePointEscape`），根内目标的链接允许读写。写入/重命名/删除前同样复核目标链；打开后可用句柄最终路径复核，缩小检查与使用之间的竞态窗口。
6. `.git` 内部：文件树、搜索、编辑器与 Markdown 资源不得显示或写入 `.git` 目录或文件；Git 服务自身的写操作走独立执行体，不经 UI 路径通道。

允许根规则：

| 操作 | 项目根 | 仓库根内、项目根外 | 已登记 `AgentRoot` | 根外显式链接 | 系统另存为 |
|---|---|---|---|---|---|
| 读取 | 允许 | 允许 | 允许 | 单个文件只读 | 允许 |
| 写入/创建/删除/重命名 | 允许 | 拒绝 | 允许（可写节点） | 拒绝 | 仅该文档 |
| 资源加载（Markdown） | 允许 | 拒绝 | 允许 | 拒绝 | 不适用 |
| 自动打开（文件树/Commit） | 允许 | 只读打开 | 允许 | 不适用 | 不适用 |

- UNC：仅当真实目标位于允许根内时允许；允许根本身是 UNC 时，根内子路径允许。盘符根与设备命名空间永不作为隐式允许根。
- 项目根重定位后，`PathGuard` 以新根重建；已有文件型面板按新根重新计算相对路径，根外旧路径按只读或错误态处理。
- 已登记 `AgentRoot` 是显式可写作用域：编辑器打开该作用域内的文件可保存，保存链走同一 `PathGuard` 复核；未登记目录、项目根外与仓库根内仍只读。
- 自定义 Agent 节点是显式信任决定：目录必须是绝对路径；目录不可读写时节点显示错误态与重试。不得通过显示名或路径分隔符制造第二个根。拒绝盘符根、`%WINDIR%` 与 `System32`；节点注册校验执行拒绝名单，已登记节点在下次加载时复核。
- 系统“另存为”返回的路径由用户显式选择，生成一次性 `PathGrant::UserSelected`，只绑定当前文档与当前运行期；文档关闭、另存到别处或应用重启后失效，不写入布局。
- Markdown 显式本地链接可只读打开允许根外的单个文件，生成 `PathGrant::SingleFileLink`；该文件不得加载其根外资源，不适用于文件树、Commit、GitShow 或编辑器自动打开。

应用托管写入作用域：

| 托管根 | 实际位置 | 允许写入 |
|---|---|---|
| 漫游根 | `<data>/state/` | 设置 JSON、设置备份与隔离件 |
| 本地根 | `<local>/state/` | 状态库与 WAL/SHM、恢复快照、补全、用量、更新状态、恢复点、实例锁、Runtime 端点 |
| 系统临时目录 | 系统临时目录 | 原子写临时文件、诊断包与下载暂存；完成后清理 |
| 备份 | 漫游/本地根内的 `.bak` 与恢复点 | 备份生成、恢复、删除 |
| 隔离 | 漫游/本地根内的 `quarantine/` | 损坏件移动与保留 |
| 日志/审计 | `<local>/logs/`、`<local>/audit/` | 轮转、追加、清理 |
| Hooks 资产 | `<local>/state/hooks/` | 注入脚本、锁、statusline 备份与信号目录 |
| CLI 配置 | 用户级 CLI 配置文件 | 只经 Hooks 引擎写 hooks 子树，保留其他字段 |

- 托管写入统一经 `AuditedWrite` 入口：`PathGuard` 判定 + 临时文件 + 原子替换 + `audit` 事件；不提供跳过判定的私有写通道。同一路径匹配多个托管根时取最具体的真实路径前缀，不允许跨 kind 升级操作权限。
- 托管根与业务根互斥：分类同时校验 `PathIntent` 与实际根；`Business` 意图命中托管根、`AppManaged` 意图命中业务根或错误的托管 kind 均返回 `ManagedRootMisuse`。用户选择路径、项目文件、Agent 文件与 Markdown 资源永不落入 `AppManaged`。
- 审计字段限 `operation/path/source/result` 与 expected/actual 指纹；不记录文件内容、handler 命令文本或凭据。CLI 配置只记录路径与 hooks 子树指纹。
- 备份、隔离、日志与审计目录只接受各自格式的生成、移动、轮转与删除；不得把任意项目文件复制进这些目录。崩溃快照与加密备份的用户内容按各自 owner 契约处理，不扩展为通用托管写入口。

### URL、终端 OSC 与剪贴板

```rust
pub enum LinkDecision {
    External { scheme: ExternalScheme, url: String },
    LocalProject { path: PathBuf },
    LocalOutsideRoot { path: PathBuf },   // 仅 Markdown 显式链接
    Denied(LinkDeny),
}

pub enum ExternalScheme { Http, Https, Mailto, Tel }

pub enum LinkDeny { DisallowedScheme, Malformed, DevicePath, OutsideRoot, TooLong }
```

- 外部链接只分派 `http`、`https`、`mailto`、`tel`；`javascript:`、`data:`、`blob:`、`file:` 等协议忽略或无动作。外部 URL 交系统默认程序，不在应用内嵌浏览器。
- 本地链接先走 `PathGuard`；`file:` URL 不做隐式本地路径转换。项目内相对路径按当前文档所在目录解析；越界按规则只读或拒绝。
- 终端链接只接受程序声明的 OSC 8，协议限 `http`、`https`、`mailto`；裸文本自动识别不产生可点击链接。
- OSC 52：
  - 只写不读；应用从不因 OSC 52 读取请求把剪贴板内容写入终端。
  - 默认仅聚焦终端可写；设置开启“允许非聚焦终端写入剪贴板”后，只有活跃项目内的非聚焦终端可写。隐藏项目中的终端与后台非聚焦终端始终不可写。
  - 单次写入上限 1MB；超限且终端聚焦时提示已忽略；写入成功显示非阻塞提示“已复制到剪贴板（终端）”。
- 粘贴：含控制字符或超过 50 行需确认；超过 10MB 拒绝。确认框展示行数与控制字符类别，不展示完整内容。
- 剪贴板内容不进入日志、错误、诊断包或崩溃快照。

### Markdown 清洗与资源加载

处理流水线：

```text
原始 Markdown
  → 解析为 AST（不执行内联 HTML）
  → 链接/图片 URL 分类（LinkDecision）
  → 原始 HTML 安全子集过滤
  → 资源加载策略（本地根内 / 远程按需 / 拒绝）
  → 渲染为静态文本、代码高亮、公式；Mermaid 按源码文本显示
```

- 禁止元素：`script`、`style`（用户内容）、`iframe`、`frame`、`object`、`embed`、`form`、`input`、`button`、`base`、`link`、`meta`。
- 禁止属性：所有 `on*` 事件属性、`srcdoc`、`formaction`、`xlink:href` 中的脚本协议、`style` 中的外部 `url()`。
- 允许的 HTML 子集只保留结构化标签（段落、标题、列表、表格、强调、代码、引用、分隔线等）与安全属性；未知标签按文本处理或丢弃，不报脚本错误。
- 远程图片默认不加载；点击后单次加载且不带 Cookie/凭据/Referer；加载失败显示占位与重试。远程字体、样式、脚本永不加载。
- 本地图片/资源只允许项目根或已登记 AgentRoot 内真实路径；根外、设备路径、盘符相对路径显示占位或被拒绝；UNC 在任一允许根内允许；符号链接按真实路径校验。
- Mermaid 首版按源码文本显示，不生成 SVG、不执行内容；未来接入渲染器时沿用同一安全过滤。
- 公式渲染只产出绘制指令，不求值、不执行宏、不访问文件系统。
- Markdown 渲染失败时显示错误态与源码切换入口；不以“渲染失败”为由丢弃源码缓冲。

### 进程启动与终端边界

```rust
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
    pub stdio: StdioMode,
    pub window: WindowMode,
}

pub enum StdioMode { Inherit, Piped, Null }
pub enum WindowMode { Hidden, ConsoleHost }
```

- 所有外部程序启动使用结构化参数；禁止把用户输入拼进命令行字符串，禁止经 `cmd /c` 转发。
- 非交互子进程（探针、Git 读取、更新 helper）使用隐藏窗口与空/管道标准流，不继承宿主控制台；终端 PTY 使用 ConPTY，独占控制台会话。
- 默认 Shell 解析链：`pwsh.exe` → `powershell.exe` → `cmd.exe`；设置可覆盖为显式 Shell 或绝对路径可执行文件。自定义路径校验：空、非绝对、不存在、非可执行文件分别给出行内错误，不提交。
- 终端工作目录与应用发起的路径限项目根与已登记 Agent 目录；越界失败，不继承应用当前目录；UNC 仅在允许根内允许。
- `cmd` 不提供完整 Shell 集成：无命令边界、自动标题、四态与目录跟踪；历史恢复使用固定等待。根因是第三方 Shell 能力限制，不伪造。
- Windows 10 早期版本缺少 ConPTY 时回退系统控制台并一次性提示；基本输入输出保留，增强能力不可用。
- 终端进程树终止：PTY 子进程放入 Job Object（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`），关闭时先 `TerminateJobObject`，再按进程快照兜底终止后代；关闭链最多等待 3 秒，不留孤儿进程。
- 外部程序启动失败必须提示原因，不静默无响应；`ShellExecuteW` 返回值 ≤32 视为失败。

### 凭据域

```rust
pub struct SecretRef { pub scope: SecretScope, pub id: SecretId }
pub enum SecretScope { AiProvider, Runtime, Update, User }

pub struct SecretValue(Zeroizing<Vec<u8>>);    // Debug 输出 <redacted>，不实现 Serialize/Clone 公开面

pub trait SecretStore {
    fn get(&self, reference: &SecretRef) -> Result<Option<SecretValue>, SecretError>;
    fn put(&self, reference: &SecretRef, value: SecretValue) -> Result<(), SecretError>;
    fn delete(&self, reference: &SecretRef) -> Result<(), SecretError>;
    fn exists(&self, reference: &SecretRef) -> Result<bool, SecretError>;
}
```

- 唯一后端为 Windows 凭据管理器，`CRED_TYPE_GENERIC`、`CRED_PERSIST_LOCAL_MACHINE`；target 名称统一小写 `slterm/<scope>/<id>`，ID 由稳定业务标识派生（AI Provider 使用 provider ID），`SecretId` 不接受用户任意字符串。
- `SecretValue` 的 `Debug` 固定输出 `<redacted>`，不输出长度、前缀、哈希或其他可观察元数据；需要长度时走显式诊断方法，不改变 `Debug` 契约。
- 业务类型只持有 `SecretRef` 或 `has_secret: bool`；AI Provider、Runtime API、更新链不得把明文 token 放进 DTO、配置、布局或错误。
- 写入缓冲区在系统调用返回后清零；读取结果包在 `Zeroizing` 中，出域即擦除。错误消息只含操作名与 target 名称，不拼接值。
- 真实凭据不得写入任何 git 追踪文件；测试与文档只用 `sk-test` 形态假值。
- 凭据管理器不可用或读取失败时，功能进入不可用态并提示；不得回退到明文文件存储。
- 套餐余量查询读取 `~/.claude/settings.json` 的 `env` token 是只读外部凭据的受控例外：不写入凭据库、不进入 DTO/日志/诊断/快照，只在查询执行器内使用。
- 加密备份的敏感键表 `SENSITIVE_KEYS` 由本文件维护：导出按表剥除、导入按表复验；手改文档塞入的凭据字段一律剥除/拒绝；错误与日志不输出键值。

### 威胁模型

| 资产 | 对手/来源 | 攻击面 | 缓解 | 验证 |
|---|---|---|---|---|
| 项目文件 | 恶意仓库 | 路径穿越、符号链接、`.git` 写入 | 真实路径校验、写根限制、越界重解析点拒绝 | 路径决策表驱动单测 |
| 应用托管数据 | 路径混淆/用户内容混入 | 托管根误用、越界写 | `AppManaged` 分类、`PathIntent` 校验、原子写、审计 | 托管写入矩阵 |
| 用户凭据 | 本地恶意内容 | 终端 OSC 52、Markdown 链接、日志 | 凭据域隔离、OSC 白名单、脱敏 | DTO 与日志快照断言 |
| 终端会话 | 终端输出 | OSC 8/52、控制字符粘贴 | 协议白名单、写入上限、粘贴确认 | OSC 解析与粘贴单元测试 |
| Markdown 渲染 | 恶意文档 | 脚本、事件、远程资源 | 安全子集、资源策略、源码文本 | 清洗器快照与安全断言 |
| 更新包 | 网络中间人/伪造 release | 资产替换、降级、无摘要 | 名称/架构/版本/URL 前缀四重匹配、SHA-256、PE 头、尺寸上限、事务授权 | 更新链单元测试与发布核验 |
| 外部程序 | 文件名/参数注入 | shell 拼接、危险扩展名 | 结构化参数、可执行文件校验、系统处理器 | 启动参数断言 |
| Hooks 配置 | 外部编辑/恶意配置 | 任意命令执行、部分写入 | 显式保存、User 层确认、备份、锁、外部修改检测 | Hooks 引擎测试 |
| 诊断包 | 误导出 | 路径、日志中的敏感片段 | 白名单内容、脱敏、导出前自检 | 诊断包排除测试 |

残余风险与边界：

- 用户显式选择的系统另存为路径可以写入允许根外；这是产品明确例外，权限由用户与操作系统决定。
- 自定义 Agent 节点可以指向用户可写的普通绝对目录；盘符根、`%WINDIR%` 与 `System32` 拒绝；节点注册即写入授权，必须由用户显式添加。
- 本机其他进程若以同一用户身份运行，可读取用户可读文件；应用不提供跨用户隔离。
- 第三方默认程序、回收站与 Git 可执行文件的行为由操作系统或第三方决定；应用只负责参数传递与错误呈现。

### 测试矩阵

| 面 | 自动化层 | 用例要点 |
|---|---|---|
| 路径分类 | 单元 | 绝对/相对/盘符相对/设备路径/UNC/大小写/尾随点/保留名/符号链接逃逸/联接逃逸/根内链接允许/根外链接拒绝/AppManaged 分类/ManagedRootMisuse/根外只读/另存为授权/单文件链接 |
| 真实路径竞态 | 单元 + 集成 | 创建目标父目录解析、真实目标越界拒绝、根内链接目标允许、句柄最终路径复核、编辑器/Diff 保存链原子替换前复解析 |
| 应用托管写入 | 单元 + 集成 | 漫游/本地/临时/备份/隔离/日志/审计/Hooks 资产/CLI 配置分类；原子写；审计字段；用户内容路径混用拒绝；CLI 配置只替换 hooks 子树 |
| URL/OSC | 单元 | 允许与拒绝协议、超长 URL、OSC 8 参数、OSC 52 聚焦/后台/超限/非活跃项目 |
| 粘贴 | 单元 + 虚拟窗口 | 控制字符、50 行边界、10MB 拒绝、确认与取消 |
| Markdown 清洗 | 单元 | 脚本/事件/iframe/object/form、远程图片、远程字体 |
| 进程启动 | 单元 | 参数数组不被重新分词、隐藏窗口、Shell 路径校验、进程树终止 |
| 凭据 | 单元 + 人工 | `Debug` 脱敏、无序列化、错误不含值、真实凭据管理器读写删 |
| Hooks | 单元 + 集成 | 只替换 hooks 子树、备份失败中止、锁冲突、外部修改中止 |
| 诊断包 | 单元 | 排除凭据/内容/输出/剪贴板/草稿、脱敏规则、导出前自检 |
| 系统集成 | 人工/条件跳过 | 系统另存为、回收站、默认程序、真实符号链接与 UNC |
