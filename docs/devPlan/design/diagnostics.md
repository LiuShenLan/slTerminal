# 诊断

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有日志、审计 target、panic 处理、诊断包导出与脱敏，以及应用页诊断入口的动作与规则；诊断面只收集可复现问题所需的最小信息，永不包含凭据、文件内容、终端输出或剪贴板。

### 决策与红线

- **单一日志栈**：应用只初始化一个 `tracing` 日志栈；`audit` 是保留 target，安全写事件无条件进入，不随常规级别过滤。原因：双日志栈会重复落盘并让取证面丢失。边界：第三方库日志按 target 过滤，常态只放行自家 crate 与 `audit`。
- **审计只记最小字段**：事件字段固定为 `operation/path/source/result` 与 `expected/actual` 指纹；不记内容、不记 handler 命令文本、不记凭据值。原因：审计日志是长期持久化面。边界：路径本身可记录；凭据 target 名称可记录。
- **panic 可见且可复制**：panic 写 stderr 与持久日志，并在 GUI 无控制台时弹出可复制的 Windows 错误框；对话框在专用线程运行，避免重入 GPUI 造成 double panic。原因：崩溃现场是唯一诊断材料。边界：panic 文本不主动拼入文件内容或凭据；代码审查保证格式化参数安全。
- **诊断包白名单**：只导出登记类别；导出前预览、生成后自检。原因：诊断包是用户主动外发的文件，默认应安全。边界：用户可显式勾选包含日志与路径，但不得包含凭据、文件内容、终端输出、剪贴板与草稿。
- **错误详情同源脱敏**：错误横幅、错误对话框与诊断包使用同一 `Redactor`；不得各自实现裁剪。原因：脱敏规则分叉会让某一路径泄漏。边界：用户可见的短错误文案可保留文件名与项目名。

### 日志

```rust
pub struct LogConfig {
    pub level: LogLevel,                 // 默认 Warn；环境变量可覆盖
    pub extra_targets: Vec<String>,
    pub directory: PathBuf,              // slterm_settings::paths 本地日志根
    pub max_file_bytes: u64,             // 10 MiB
    pub max_files: usize,                // 5
}

pub enum LogLevel { Error, Warn, Info, Debug, Trace }
```

- 输出：控制台/标准错误 + 按需文件双写。文件按大小轮转，超过上限滚动并删除最旧文件；目录创建失败时退回标准错误并继续运行。
- 格式：相对启动时间、级别、target、payload；多行 payload 续行对齐。每条记录包含进程 id 与线程名（可用时），便于对照崩溃与后台任务。
- 默认级别 `Warn`；诊断构建或环境变量可提升到 `Info`/`Debug`。`audit` target 不受级别过滤。
- 禁止记录：凭据、token、请求头、文件内容、终端输出、剪贴板、Markdown 草稿、Hooks handler 命令文本、用户输入正文。允许记录：操作名、路径、错误类别、稳定诊断码、耗时、容量、指纹。
- 日志写入不在 GPUI 前台线程执行；使用有界队列与非阻塞文件写入，队列满时丢弃低级别记录并递增丢弃计数，不阻塞 UI。
- 数据根与本地根由 `slterm_settings::paths` 单点解析：应用设置使用漫游根访问器；状态库、布局/项目数据、窗口几何、崩溃快照、隔离区、补全、用量信号、Hooks 资产、更新状态、备份恢复点、日志、审计与崩溃现场使用本地根访问器；调用点不拼接品牌目录，不硬编码系统配置路径。

### 审计 target

```rust
pub struct AuditEvent {
    pub operation: AuditOperation,
    pub path: Option<PathBuf>,
    pub source: AuditSource,
    pub result: AuditResult,
    pub expected: Option<Fingerprint>,
    pub actual: Option<Fingerprint>,
}

pub enum AuditOperation {
    SettingsWrite,
    LayoutWrite,
    ProjectListWrite,
    HooksInject,
    HooksUninject,
    SettingsImport,
    SettingsExport,
    CredentialWrite,
    CredentialDelete,
    UpdateCommit,
    RecoveryRestore,
    FileDelete,
}

pub enum AuditSource { User, Migration, Recovery, UpdateHelper, Diagnostic }
pub enum AuditResult { Success, Failed, Refused }
pub struct Fingerprint([u8; 32]);        // SHA-256，十六进制输出
```

- 事件经 `tracing` target `audit` 发出；字段名固定，禁止附加任意键值。
- 指纹用于对账：设置/布局写入记录旧内容与新内容哈希；Hooks 注入记录 hooks 子树哈希；导入导出记录文件哈希。哈希不泄露内容，但可验证是否被篡改。
- 凭据操作只记录 target 名称与结果，不记录 secret、用户名或长度以外的信息。
- 审计事件同时写入独立 `audit.log` 轮转文件；不依赖常规日志级别开关。
- 审计失败（磁盘满、权限不足）不阻断业务操作，但必须在常规日志与通知中可见；不得静默丢失取证面。

### panic 与错误边界

```rust
pub struct PanicReport {
    pub message: String,
    pub location: Option<String>,
    pub thread: String,
    pub backtrace: Option<String>,       // 仅符号化帧，不含局部变量
}
```

- `PanicHook` 在启动早期安装一次：写标准错误、追加 `panic.log`、写入崩溃标记、启动专用线程显示 `MessageBoxW`（标题为产品名 + Runtime Error，正文可 Ctrl+C 复制）。GUI 无控制台时才弹框；有控制台时标准错误已足够。
- 启动期错误在 logger 初始化前发生：有控制台写标准错误；GUI 无控制台弹出启动错误框。启动错误框不得触发数据目录之外的副作用。
- 后台任务 panic：任务边界 `catch_unwind`，转换为 `ErrorClass::Internal` 错误事件，记录 target 与任务名，不终止进程；同一任务连续 panic 时熔断并停止重试。
- 面板渲染错误：面板区域显示 `面板渲染出错`、说明、`重试` 与可折叠详情；同项目其他面板不受影响。应用整体渲染错误显示全屏 `应用渲染错误` 与可折叠诊断详情。
- 错误详情不得包含文件内容、凭据或终端输出；详情入口与诊断包共用脱敏。
- 崩溃恢复标记用于下次启动提示恢复；崩溃快照只含编辑器与 Markdown 源码态文本，不含终端输出、Diff 工作区侧或设置草稿。

### 诊断包

```rust
pub struct DiagnosticBundleRequest {
    pub destination: PathBuf,
    pub include_logs: bool,          // 默认 true
    pub include_audit: bool,         // 默认 true
    pub include_paths: bool,         // 默认 false
    pub include_environment: bool,   // 默认 true
}

pub struct BundleManifest {
    pub schema: u16,
    pub generated_at: String,
    pub app_version: String,
    pub build: BuildInfo,
    pub os: OsInfo,
    pub locale: String,
    pub files: Vec<BundleEntry>,     // path, bytes, sha256
}
```

包内容：

| 类别 | 默认 | 内容 |
|---|---|---|
| 版本与构建 | 是 | 应用版本、提交标识、构建配置、安装渠道、更新通道状态 |
| 运行环境 | 是 | Windows 版本/构建号、架构、DPI/文本缩放、显示器工作区数量与布局（不含设备序列号）、界面语言与解析结果 |
| 功能状态 | 是 | 主题、密度、透明度、壁纸启用、材质、托盘/开机启动/通知权限、ConPTY 与渲染能力 |
| 配置摘要 | 是 | 设置 schema 版本、各设置域是否损坏/只读、状态库版本/完整性/大小/备份状态、项目与面板类型摘要（路径默认脱敏） |
| 日志 | 是 | 最近日志轮转文件，按大小截断，脱敏后打包 |
| 审计 | 是 | 审计事件尾部，已是结构化最小字段 |
| 自检结果 | 是 | 诊断页最近一次环境自检的通过/失败/不可用与原因 |
| 崩溃现场 | 否 | panic 报告、崩溃标记与诊断现场；不含恢复快照或恢复文本 |
| 路径 | 否 | 勾选后保留绝对路径；默认替换为 `%USERPROFILE%`、`%PROJECT_ROOT%`、`%AGENT_ROOT%` |

明确排除：凭据与 token、AI Provider 密钥、文件内容、终端输出与回滚、剪贴板、Markdown 草稿与崩溃恢复文本、Diff 工作区侧、Hooks handler 命令文本、Runtime API token、完整 Hooks 配置文件。

导出流程：

1. 应用页点击“导出诊断包…” → 系统保存对话框选择目标 `.zip`。
2. 预览对话框列出将包含/排除的类别；路径与日志开关可调整。
3. 收集到临时目录，逐文件脱敏；生成 `diagnostics-manifest.json`（含每文件 SHA-256 与大小）。
4. 对生成文件做模式自检：扫描凭据键名、bearer/basic 头、常见 token 前缀、私钥块；命中则中止并报告“自检失败”，不输出半成品。
5. 压缩为临时文件 → 原子重命名到目标；失败清理临时文件。
6. 成功通知包含目标路径；失败通知包含原因，不包含包内容。

脱敏规则：

- 键名匹配（不区分大小写）：`token`、`secret`、`password`、`passwd`、`api_key`、`apikey`、`authorization`、`cookie`、`credential`、`private_key` → 值替换为 `<redacted>`。
- 值模式：`Bearer `、`Basic `、`sk-`、`ghp_`、`github_pat_`、`xoxb-`、PEM 私钥块 → 整体替换。
- 路径：默认将用户主目录替换为 `%USERPROFILE%`，项目根替换为 `%PROJECT_ROOT%`，Agent 目录替换为 `%AGENT_ROOT%`；保留文件名与相对结构。
- 日志中的自由文本同样过规则；无法确认安全的行整行替换为 `<redacted line>`，并在清单记录替换计数。

### 应用页入口与动作

- `settings/app-page.md` 只放诊断入口；动作集、二次确认、清理范围与脱敏规则由本篇独占，其他文档只引用。
- 规格内动作集：

| 动作 | 行为 |
|---|---|
| 版本/构建 | 只读显示应用版本、提交标识与构建配置；进入“复制诊断信息”内容 |
| 数据目录 | 只读显示数据根路径 |
| 日志目录 | 只读显示本地日志根路径 |
| 配置健康 | 显示设置加载状态（正常/损坏回退/只读）、项目数据加载状态与 Runtime API 状态 |
| 复制诊断信息 | 复制版本、路径、状态与最近错误类别；不含凭据、文件内容、终端输出、剪贴板或项目路径以外的用户数据 |
| 打开数据目录 | 交给系统文件管理器；失败时通知 |
| 打开日志文件 | 交给默认程序；失败时通知 |
| 通知自检 | 仅在用户显式触发时投递一条测试通知，绕过前台门控与节流 |
| 一键清理日志/审计/更新下载/崩溃现场 | 见下；二次确认并列出范围 |
| 导出诊断包… | 可选；打开预览与保存流程，导出前脱敏与自检；无活跃项目时同样可用 |
| 运行环境自检 | 可选；异步执行下列检查，显示通过/失败/不可用与原因，可重跑 |

- 一键清理规则：
  - 二次确认，确认框列出将删除的目录/文件与保留项；清理失败逐项可见。
  - 删除范围：日志轮转文件、`audit.log`、更新下载缓存、崩溃现场（panic 日志、崩溃标记与诊断现场）。
  - 不删除设置、状态库、状态库备份、项目数据、布局、恢复快照、加密备份恢复点；“崩溃现场”不含恢复快照。
  - 清理不触碰凭据、Hooks 配置或 Runtime token。
- 诊断命令在后台执行；失败显示原因，不阻塞页面。
- 不提供任意命令执行、环境变量查看或注册表编辑入口。

自检项：

| 检查 | 成功判据 | 失败表现 |
|---|---|---|
| 配置目录与状态库 | 设置临时文件写入/同步/删除成功；状态库可打开且 `quick_check` 通过 | 只读/权限/完整性错误，附路径 |
| 凭据管理器 | 写入/读取/删除假值 `sk-test` 成功 | 不可用或拒绝，不显示假值以外内容 |
| 通知能力 | AUMID 注册成功、通知权限已授予（不实际弹窗） | 未授予/不支持 |
| 托盘 | 图标创建/移除成功 | 失败原因；回窗入口受限 |
| ConPTY | 能力探测成功 | 回退系统控制台 |
| 更新渠道 | 分发渠道可识别；外部托管给出更新方式 | 无法识别，拒绝自更新 |
| 渲染 | GPUI 报告渲染器可用 | 软件渲染降级 |
| Runtime 控制端点 | 端点可用且单实例锁持有 | 端口占用/端点不可达 |

- 自检不得写入真实用户数据目录之外的文件；凭据自检使用独立 target 并在结束时删除。
- 诊断入口不显示 Runtime token、Provider 密钥或 Hooks 命令文本。

### 验收与测试

- 单元：日志级别/target 过滤、轮转与上限、审计字段集合、指纹确定性、脱敏规则表、诊断包清单与排除项、自检结果映射。
- 集成：诊断包导出到临时目录并解压校验；在日志与设置中注入假 token，确认包内无明文；备份失败/磁盘满时不产生半成品。
- 子进程：panic hook 在独立测试进程中触发，断言 stderr、panic.log、崩溃标记与对话框分流（无控制台人工核验）。
- 人工/条件跳过：真实 Windows 事件查看器、真实凭据管理器、系统通知与托盘、GPU 信息；登记豁免与兜底。
