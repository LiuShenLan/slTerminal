# slTerminal

面向 Windows 10/11、面向所有 AI CLI 调优的通用终端模拟器。单进程 GPUI 应用，单窗口单实例、仅暗色模式、GPU 加速渲染。

## 术语

### 产品与工作区

**工作区**（Workspace）：
单窗口内的根 UI 与状态权威（`SltermWorkspace`），直持标签页、窗格与展示态。应用进程的一次运行即一个工作区。
_Avoid_: 多工作区, 项目工作区

**标签页**（Tab / WorkspaceTab）：
工作区中的顶层容器，封闭类型集为终端 / 设置 / 文档 / 源码 / 图片。标签页有稳定 `TabId`，跨排序与活跃切换不变。
_Avoid_: 页签, 页面

**窗格**（Pane）：
终端标签页内的分屏单元，对应一个 `TerminalView` 与一个 PTY 会话。`PaneId` 全工作区唯一、终生不复用。
_Avoid_: 面板, 视图

**分屏树**（SplitTree）：
描述窗格布局的二叉树，叶 = `PaneId`；切割数学、拖拽比例与方向导航由 `slterm_split` 纯函数裁定。
_Avoid_: 布局树

**会话快照**（Session Snapshot）：
落盘于 `session.json` 的工作区恢复数据；只收终端标签页（含分屏树、启动身份、WindowState、侧栏现场），不收设置与纯视图标签页。

**WindowState**：
窗口大小/位置/最大化状态；语义是启动回放的行为输入，不是诊断字段。

**活动栏**（ActivityBar）：
最左侧常驻窄条，容纳侧栏视图按钮与设置入口。

**侧栏视图**（Side View）：
活动栏按钮对应的可开关内容视图，封闭集 `SideViewId`（导航树 / 文件浏览器 / Commit / Agent 全局文件）。未知 id 恢复时静默丢弃。

**双槽**（Top Zone / Bottom Zone）：
侧栏内容的上下两个半区；同一半区最多一个打开的视图，展示由状态纯推导。

### 终端与 PTY

**终端视图**（TerminalView）：
GPUI 中的终端呈现实体：接收 PTY 输出、驱动网格渲染、处理输入与选择。每个视图对应一个 pane。

**PTY 会话**：
`slterm_terminal` 管理的伪终端进程实例（ConPTY、读写线程、子进程与生命周期）。与「标签页」「窗格」是不同概念。

**Shell 解析**：
默认 shell 的探测链——PowerShell 7 → Windows PowerShell 5.1 → `cmd.exe`；用户可在设置中覆盖。

**Shell 集成脚本**：
随终端注入的 PowerShell 脚本，覆盖 `prompt` 函数发射 OSC 序列（cwd 跟踪、提示符边界与退出码），供宿主跟踪命令边界。

**ConPTY 捆绑**：
Win10（build < 21376）下嵌入并提取的新版 `conpty.dll` + `OpenConsole.exe`；Win11 走系统 ConPTY。提取失败静默回退系统路径。

**DA1 / DSR 接管**：
终端核心对设备属性查询（DA1）与光标位置查询（DSR）的按需代答/剥离策略；统一身份 `?64;22c`，禁止盲注。详见 ADR-0022。

**滚动回看**（Scrollback）：
终端历史行缓冲；容量是设置键与契约数值，默认值见代码。

**复制语义**：
`Ctrl+Shift+C` 有选区复制并反馈、无选区静默吞；`Ctrl+C` 恒中断并透传 PTY。

### 设置与外观

**设置中心**（SettingsPane）：
统一配置入口的标签页，左侧导航 + 右侧配置页；配置页经注册表分派。新增配置页 = 组件 + 注册一条。

**设置段**（Settings Segment）：
`settings.json` 的语义分组（appearance/scrolling/agentHooks/…）；「键 → 段」映射单源在 `slterm_settings::keys`，未知键原位保留。

**设置写通道**（settings_store）：
设置落盘的唯一硬机制：白名单校验、段级浅合并、保存锁、原子写、`.bak` 与损坏三态。写核心私有，所有写入经审计包装。

**前端/后端消费型配置**：
消费侧在 UI 的配置走通用段写；消费侧在后端的配置走域专用命令（校验 + 内存态 + 落盘一体）。禁止第二写通道。

**配色 token**：
UI 颜色的语义命名槽位。组件只引用 token，禁止硬编码颜色；仅暗色。

**内置主题**（ThemeName）：
八套暗色主题，`Linear` 为默认；主题色经 WCAG 语义槽校验后才可入目录。

**明度阶梯**（Lightness Ramp）：
UI 背景色的离散档位；层级靠相邻档差表达，不叠加阴影/边框。

**发丝线**（Hairline）：
暗色界面分隔线的唯一形态——半透明 1px 线，禁止实色粗边框。

**窗口材质**（BlurMode）：
五档枚举 `None/Mica/MicaAlt/Aero/Acrylic`，默认 `None`；`opacity` 为标量。WCA/DWMSBT 通道互斥，按 Windows build 门控。

**i18n 语言偏好**（LanguagePref）：
`System/English/Chinese`；System 按系统 locale 协商。翻译键集是构建期硬合同。

### Agent 集成

**编码 CLI / AI CLI**：
以命令行运行的 AI 编码代理（claude、codex、gemini 等）。slTerminal 面向所有 AI CLI 调优，内置 27 家 `AgentKind`。

**AgentKind**：
27 家 AI CLI 的封闭身份枚举；承载 slug、显示名、启动/恢复/分叉命令与屏幕规则匹配。持久化 `source` 宽松落盘，消费时经 `AgentKind::parse` 严格归一。

**Agent 状态**（AgentStatus）：
pane 的 agent 语义状态：Idle / Working / Blocked / Done / Unknown；来源分为 Hook / Screen / Process / Unknown。`Unknown` 表示尚无权威判定（普通 shell 或未识别）。

**AgentHub**：
agent 事件归属与 pane 状态投影的宿主：消费 hook 事件与屏幕看门狗结果，维护 AgentActivity/AnswerInbox。

**AI hook 事件**（AiHookEvent）：
归一化的生命周期事实：SessionStart / PromptSubmit / ToolComplete / TurnDone / NeedsAttention / SessionEnd，附回合结果与能力位。

**hook 小进程**（`slterm-hook`）：
每个 AI 回合事件调用一次的独立小进程；经命名管道把原始载荷转发给宿主后退出。必须 exit 0、快速、隐形。

**命名管道通道**：
hook 事件唯一权威通道；宿主侧服务器解析 JSON，helper 不重编码。

**statusline 桥接**：
context 用量百分比的数据通道；桥接脚本节流后写 ContextUsage 信号文件，并透传用户原 statusline。与 hook 管道分通道。

**信号文件通道**：
statusline 用量等低频信号的介质；app 数据目录，与命名管道分通道。

**ContextUsage 信号**：
statusline 桥接产出的用量事件，payload 携带 `usedPercentage`。

**SLTERM_PANE_ID**：
PTY spawn 注入子进程的环境变量，经 shell → CLI → hook 继承链传递，用于会话→pane 路由。

**历史会话**（AiSession）：
编码 CLI 在某目录运行产生的持久化会话记录（claude 为 `~/.claude/projects/` 下 `<uuid>.jsonl`），由各 CLI 的扫描/恢复命令处理。

**hook 失败边**：
hook 执行失败时的合成命令与证据；默认关（复用 `AssistantConfig.enabled`），只进闸门与 log，不直接弹 UI。

### Runtime 控制面

**Runtime API**：
本机 loopback 上的 JSON Lines 控制面，供 AI CLI 与脚本查询/驱动终端；协议名 `slterm.runtime`，首发版本 1。

**信封**（Envelope）：
请求/响应的统一外层：`protocol`/`version`/`id`/`token`/`method`/`params`；响应以 `ok` + `result` 或 `error` 互斥表达。

**端点发现**：
进程内子端点 > `SLTERM_RUNTIME_ENDPOINT` 环境变量（`<port> <token>`）> 数据目录 `runtime.port`。

**CLI 双人格**：
同一 `slterm.exe`：带子命令 = CLI 客户端，无子命令 = GUI。`slterm ctl` 暴露完整协议，资源动词（`window`/`tab`/`pane`/`agent`）为同一信封上的薄别名，`slterm env` 离线报告身份与端点。

**RuntimeHub / RuntimeDispatch**：
Runtime API 的宿主侧枢纽：接收写命令、派发到工作区执行、经回执通道返回结果。

**单实例移交**：
第二个实例启动时不建窗，改为唤起既有实例；移交走版本化 JSON（`window.focus` + `tab.new`）。旧式 `ATTACH/PING` 回落由 `legacy-attach` feature 控制，首发关闭。

**Runtime skill**：
随包分发的 `slterm-runtime` Skill（`assets/skills/`），提供给 AI CLI 自发现控制面；安装器按受管文件指纹投放。

### 文件与编辑器

**文件编辑器**（file_editor）：
内置源码/文本编辑内核与视图：文档模型、撤销、选择、输入规则、gutter、outline、预览/并排形态。

**源码标签页 / 文档标签页 / 图片标签页**：
`WorkspaceTab::Code` / `Document` / `Image`；文件打开路由由 `openable_in_app` 单判定。

**Diff 标签页**：
只读双栏对照（HEAD ↔ 工作区），复用 `WorkspaceTab::Code` 的 Diff 模式；gutter 点击与重开命中走 `open_diff_tab` 单入口。

**Gutter**：
编辑器行号区，按 git 状态显示增删改标记；点击打开对应 diff。

**文件浏览器**：
侧栏视图 `Explorer`：项目目录树、git 状态着色、创建/删除/重命名/打开，回收站删除走 `SHFileOperationW`。

**文件树模型**（side_panel）：
只持数据与状态转换的模型层；渲染按行号取行，模型内禁止出现 GPUI 类型。

### 后台任务与套餐余量

**后台定时任务**（Background Task）：
应用级任务单元；元数据注册于 `background_tasks`，执行体在后端或订阅方，配置落 `backgroundTasks.{taskId}` 段。

**TaskSchedule**：
任务触发形态三态——`Interval`（周期）/ `Once`（启动一次性，`interval_sec` 为启动延迟）/ `OnDemand`（事件驱动）。共用订阅生命周期与失败策略。

**扫描执行体**：
历史会话扫描的唯一执行路径；手动刷新与定时刷新共用。

**套餐余量**（Plan Balance）：
编码套餐当前剩余可用量（金额余额 / 时间窗用量）；查询走 user 层 settings 的 `env` 段，token 不出凭据域。

**余量推送**（Plan Balance Watch）：
模块级 `SNAPSHOT` + watch 语义订阅：只报「有新值」，订阅者读最新快照；有界 1 槽合并连续推送。

### 安全

**凭据域**：
真实凭据只存 Windows 凭据管理器；业务经 `secrets` 门面访问。`SecretBytes` 出域即用即焚，token/key 不出域。

**审计通道**（audit）：
`tracing` target `audit` 的安全事件通道；事件字段只含 operation/path/source/result 与 expected/actual 指纹，禁插值凭据值与文件内容。

**加密备份**：
口令保护的本地备份，格式 `TRMBK001 | salt | nonce | ciphertext`（AES-256-GCM，魔数作 AAD；Argon2id 派生）。类别六类，敏感键导出剥除、导入复验。

**恢复点**（RestorePoint）：
一次备份/恢复操作的可撤回句柄；会话内撤回，跨重启清单归设置页安全分区。

**敏感键表**（SENSITIVE_KEYS）：
备份导出剥除与导入复验共用的第二道防线；手改文档塞入的凭据字段一律剥除/拒绝。

### 基础设施与治理

**GPUI 前台/后台**：
前台 = UI 线程（渲染与状态更新，禁阻塞）；后台 = background executor 或专用线程（阻塞 I/O、网络、子进程）。

**AppError**：
通用错误信封四变体 `Io`/`Pty`/`Gpui`/`Unknown`；`Unknown` 承载领域内部失败，消息用户可见且禁带凭据。

**会话元数据索引**（terminal_registry）：
模块级单例，按 `PaneId` 提供会话元数据查询；Workspace 是实体唯一属主，索引不裁决生命周期。

**原子写**：
持久化通用纪律——临时文件 + rename；损坏三态；设置写经审计包装。

**门禁三件套**：
行数棘轮（`architecture/file-budgets.txt`）、依赖方向（`architecture/dependencies.toml`）、禁名回流（`scripts/check_brand_regress.py`）；git hooks 常挂。

**因果 note**（architecture/notes）：
决策成因档案（九段结构、Supersedes 链、Revisit when）；与 adr 分工：adr 记当前规则，note 记成因与废弃路径。

**ADR**：
`.claude/adr.md` 中当前生效决策的当前形态；条目原位收敛，沿革行记修订日期与要点。

**豁免登记**（test-exemptions）：
不可自动化测试面的单表真值源；未登记豁免即硬约束违规。

## 历史改名

读旧 commit 或旧栈代码时的消歧表：

| 旧标识符 / 概念 | 现行形态 | 说明 |
|---|---|---|
| 项目 / 操作页面 | 已删除 | 单窗口模型改为工作区 + 标签页 + 窗格 |
| Dockview 面板 / panelRegistry | `WorkspaceTab` 封闭枚举 | 面板类型白名单改为 Rust 枚举 + 单一注册点 |
| 前端会话 `SessionInfo` / Zustand store | 已删除 | 单进程内状态直持；会话元数据经 `terminal_registry` |
| IPC 层 / `invoke` / Tauri 命令 | 已删除 | 单进程直接调用；跨进程只剩 Runtime API 与 hook 管道 |
| xterm.js 终端 | `slterm_terminal` + `TerminalView` | VT/网格/PTY 归 core，呈现归 GPUI |
| CodeMirror 编辑器 | `file_editor` | 自绘编辑内核 |
| `SideViewRegistry` / `panelRegistry` | `SideViewId` 封闭枚举 | 注册表改为封闭集 + 单点分派 |
| `plan_balance` emit + 前端订阅 | 模块级 watch channel | 多消费者读最新快照 |
| `hook-event` / `onHookEvent` | `AiHookEvent` / agent 事件 | 按 agent 域泛化 |
| `claude_history_*` / `claudeHistory` | `AiSession` / 历史会话扫描 | 泛化为 CLI 无关 |
| `claudeSession` / `setClaudeSession` | `AgentKind` + pane 会话归属 | 会话模型泛化 |
