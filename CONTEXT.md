# slTerminal

面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。单窗口、暗色模式、GPU 加速渲染。

## 术语

### 核心容器

**项目**（Project）：
文件系统目录的顶层组织单元。一个项目包含多个操作页面，每个项目绑定一个根目录路径。
_Avoid_: workspace, repo

**操作页面**（OperationPage）：
项目内一个独立的工作页，拥有自己的面板布局。页面间显隐切换且终端跨页面存活。
_Avoid_: 操作页, tab, 标签页

**工作区**（Workspace）：
多页面 Dockview 布局的根 UI 组件，管理页面切换、布局持久化和侧栏布局。

**布局**（Layout）：
一个操作页面内面板排列的 JSON 序列化数据。布局存取只经集中的序列化/反序列化模块（workspace/layoutSerde.ts）。

**应用运行期**：
应用进程的一次运行——「单运行期内唯一」等语义的准确表述。

### 面板系统

**面板**（Panel）：
Dockview 布局中可托管的最小 UI 单元。每个面板属于一种面板类型、对应一个 React 组件。
_Avoid_: 窗格, 视图

**面板类型**（Panel Type）：
面板的分类，注册 id 与 panelRegistry.ts 一致。新增类型须显式注册；布局恢复时过滤未注册类型。

**面板实例**（Panel Instance）：
具体的一个面板，有唯一标识符，可被创建、关闭、拖拽分屏。

**面板注册表**（Panel Registry）：
面板类型到 React 组件的映射表，也是布局恢复时的面板类型白名单。

**终端面板**：
xterm.js 驱动的终端面板，每个对应一个 PTY 会话。

**编辑器面板**：
CodeMirror 6 驱动的文件编辑器面板。

**页签标题**（Tab Title）：
Dockview 标签页上显示的文字。终端为 `terminal-N`（每页独立编号），编辑器为文件名（冲突时用相对路径）。
_Avoid_: 标签页

**文件查看器注册表**（FileViewerRegistry）：
策略模式单例，按文件扩展名决定用哪种面板类型打开文件；未命中回退编辑器面板。
_Avoid_: 文件类型映射

### 文档预览

**docViewer 预览家族**：
共享 docViewer 基础设施的文档型预览面板（htmlviewer/markdownviewer）——预览渲染于跨源沙箱宿主 iframe，经注入桥与 postMessage 和主窗通信。

**查看形态**（View Mode）：
文档面板的显示形态。markdownviewer 三态：edit / split / preview；htmlviewer 二态：render / edit。形态随面板 params 持久化。
_Avoid_: 模式（与四态/编辑模式语义撞车）

**形态切换条**（ModeSwitcher）：
docViewer 共享的查看形态切换条组件，单选高亮当前形态，恒由右上悬浮区承载。
_Avoid_: 悬浮窗

**右上悬浮区**（FloatingArea）：
docViewer 面板工具条带内悬浮 UI 的单点承载区，横向侧排形态切换条与缩放 HUD。

**文档真值源**（docRef）：
文档面板的草稿优先文档模型——磁盘读入与编辑内容统一存放于面板级 docRef，预览渲染永以 docRef 为准而非磁盘；形态切换草稿保留。

**文档预览渲染管线**（mdPipeline）：
markdown-it 组合的同步纯函数主体 + 异步编排（mdRenderAsync），产物为完整 HTML 文档注入预览帧。

### 终端与 PTY

**前端会话**（SessionInfo）：
前端 Zustand 中记录的会话元数据——会话标识、面板标识、当前工作目录、是否活跃。面板只订阅自己的会话切片，不自存会话数据。
_Avoid_: 会话（无歧义上下文下可使用，但需与 PTY 会话区分）

**PTY 会话**（PtySession）：
后端 Rust 中管理的伪终端进程实例，包含主端描述符、子进程句柄、标准输入写入器、输出读取线程与 IPC 通道引用。
_Avoid_: 终端会话, shell 进程

**PTY 事件**（PtyEvent）：
PTY 输出流的带标签枚举——`Output`（终端输出字节）或 `Exit`（子进程退出码）。

**Shell 解析**：
默认 shell 的探测链——PowerShell 7 → Windows PowerShell 5.1 → cmd.exe。用户可在设置中覆盖。

**Shell 集成脚本**：
编译时嵌入的 PowerShell 脚本，覆盖 `prompt` 函数注入 OSC 序列（cwd 跟踪 + 提示符边界/退出码），供宿主跟踪工作目录与命令边界。

### 侧栏

**活动栏**（Activity Bar）：
应用最左侧的窄条，容纳侧栏视图按钮；按钮可拖拽归属上区/下区。底部固定「配置」钮——设置中心唯一入口，不入视图注册表。

**侧栏视图**（Side View）：
活动栏按钮对应的可开关内容视图（导航树、文件浏览器、Commit、Agent 全局文件）。
_Avoid_: 页面, 面板

**侧栏区**（Side Bar）：
侧栏视图共享的展示区域，位于活动栏与主区之间，垂直划分为上区和下区两个半区。

**上区 / 下区**（Top Zone / Bottom Zone）：
侧栏区的两个半区。每个活动栏按钮经拖拽归属于其一；同一半区同时只展示一个侧栏视图，不同半区可各展示一个。

**统一导航树**：
侧栏导航视图的信息架构——树层级恰为 项目 → 页面 → 会话，活跃会话挂页面下、历史会话折叠为计数节点挂项目下。管理项目/操作页面 CRUD 与页面切换导航；新建项目或页面时布局为空，不自动创建终端面板。
_Avoid_: 项目列表

**文件浏览器**：
以侧栏视图形式展示的文件树——展示活跃项目根目录结构，支持创建/删除/重命名/打开，git 状态着色，文件系统变更增量刷新；双击文件经文件查看器注册表决定打开面板类型。

**Commit 视图**：
以侧栏视图形式展示当前项目的 Git 变更概览，含变更列表与未跟踪文件列表两个可折叠分组；双击文件条目按 git 状态分派到对应面板类型。

**Agent 全局文件视图**：
侧栏视图（id `agentFiles`）——agent 节点按 CLI 展开为该 CLI 全局配置目录（如 `~/.claude`）的文件浏览器，功能与文件浏览器相同；展示内容可在设置中心配置。

### 版本控制

**Git 状态**：
工作区文件相对于 HEAD 的状态——已修改、已添加、已删除、已重命名、未跟踪、冲突、已忽略。

**差异块**（DiffHunk）：
文件中连续变更行的范围，记录旧文件与新文件中的起止行号（1-based），驱动编辑器行号 gutter 的颜色标记。

**Git 仓库缓存**：
工作目录到 git2 仓库对象的 LRU 映射缓存，避免重复的 `Repository::discover` 遍历。

**变更列表**（Changes）：
Commit 视图中已跟踪变更文件的分组（added / modified / deleted / renamed / conflict），按相对路径字母序排列。
_Avoid_: 暂存区列表

**未跟踪文件列表**（Unversioned Files）：
Commit 视图中未跟踪文件（untracked）的分组，按相对路径字母序排列。

**状态分派**（Status Dispatch）：
Commit 视图双击文件条目时 git 状态到面板类型的映射——modified/renamed/conflict → diff 面板，deleted → gitshow 面板，added/untracked → editor 面板。

### Agent 集成

**编码 CLI**（Coding CLI）：
以命令行形态运行的 AI 编码代理程序（claude、codex、aider 等）。slTerminal 对其提供专门优化，经 CLI profile 抽象实现可插拔支持。

**CLI profile**（编码 CLI Profile）：
一个编码 CLI 的完整能力描述与注册单元——身份识别（内置命令集 + 品牌 logo）+ 分域能力声明（hooks 注入/事件状态映射/历史 provider/用量策略/配置编辑器等），能力可选，未声明即该域不可用。内置命令集为静态声明，用户别名独立存 `cliAliases` 段、不并入。

**CLI 别名**（CLI Alias）：
用户为某编码 CLI 追加配置的启动命令名（如 `cc` → claude），别名命中与内置命令命中完全等价。存应用 settings.json `cliAliases` 段，全命名空间唯一、大小写敏感；与套餐「URL 别名」不同域。

**历史会话**（Agent Session History）：
编码 CLI 在某项目目录运行产生的持久化会话记录（claude 为 `~/.claude/projects/` 下 `<uuid>.jsonl` transcript），由各 CLI 的 history provider 扫描/删除/恢复。与前端会话、PTY 会话是不同概念。

**会话行**：
统一导航树活跃会话区中的一行，对应一个运行中的编码 CLI 会话（纯 shell 终端无行）；上下文用量由 ContextUsage 信号驱动更新。
_Avoid_: 终端行

**四态**：
页签图标的四种编码 CLI 会话状态——工作、注意、完成、错误；跨 CLI 归一状态模型，各 CLI profile 把自有事件映射进四态。专有术语，其他概念避免裸用（侧栏布局/Commit 状态机等用「四布局态」「四渲染态」）。

**CC hooks**（Claude Code Hooks）：
Claude Code CLI 内置的 hooks 机制——settings.json `hooks` 字段的三层嵌套配置（事件 → matcher 组 → handler 数组），经 stdin/stdout JSON 与 exit code 语义通信。
_Avoid_: 终端 hooks, slTerminal hooks

**宿主侧增强**：
slTerminal 不改动 Claude Code 本身，为其 hooks 提供状态可视化与配置管理外围能力的功能方向。

**信号文件通道**：
slTerminal 感知 CC hook 事件与 context 用量信号的主通道——注入的 hook 脚本把事件写为 JSON 信号文件到约定路径，后端监听目录并经 IPC 推送前端。

**statusline 桥接**：
context 官方用量百分比的数据通道——settings.json `statusLine` 指向注入的桥接脚本，claude 渲染状态行经 stdin 传入用量 JSON，桥接脚本节流后写 ContextUsage 信号文件并透传用户原 statusline 命令。

**ContextUsage 信号**：
statusline 桥接脚本产出的信号事件（event = `ContextUsage`），payload 携带 `usedPercentage`（0–100 float）。

**SLTERM_PANEL_ID**：
pty_spawn 注入子进程环境的环境变量，经 shell → CLI → hook 脚本继承链传递并写入信号文件，实现会话→页签的精确路由。

**三层配置**：
CC settings.json 的三个编辑层级——user（`~/.claude/settings.json`）、project（`.claude/settings.json`）、local（`.claude/settings.local.json`），优先级 local > project > user。

**注入 / 卸载**：
把 slTerminal 状态上报 hook 配置 merge 写入 user 层 settings.json（注入），或干净移除配置段与脚本文件（卸载）。仅手动触发。

**双模式面板**：
设置中心 Agent 组「Hooks 配置」页内 hooks 编辑器的两种编辑模式——GUI 表单与 JSON 编辑器，实时同步编辑同一份配置。

### 套餐余量

**编码套餐**（Coding Plan）：
编码 CLI 背后的计费订阅方（deepseek、kimi），由 user 层 settings.json 的 `env.ANTHROPIC_BASE_URL` 命中套餐 URL 匹配集判定；一个套餐可有多个 URL 别名。

**套餐余量**（Plan Balance）：
套餐当前剩余可用量，两种形态：金额余额 / 时间窗用量。展示于导航树视图底部固定区。

**用量窗口**（Usage Window）：
时间窗计费套餐的限流窗口（5 小时滚动窗 / 7 天窗），含剩余百分比与重置时间。

**余量来源**（Plan Source）：
判定套餐的配置文件来源（当前为 claude user 层 settings.json 的 env 段），可扩展。

### 后台任务

**后台定时任务**（Background Task）：
应用级周期性执行的任务单元——元数据注册于后端任务注册表，执行体在后端或前端，配置持久化于应用 settings.json `backgroundTasks` 段。新增任务 = 注册一条元数据 + 一端写执行体，框架零改动。

**扫描执行体**（Scan Executor）：
历史会话扫描的唯一执行路径——遍历全部已注册 history provider 逐个扫描并聚合；手动刷新与定时刷新同为它的触发器。

**触发来源**（Trigger Source）：
任务执行体的触发渠道二分——`manual`（手动）与 `tick`（定时器）。仅影响失败处理策略（tick 失败静默、manual 失败置 error 态），执行体本身无差别。

### 设置中心

**设置中心**（Settings Center）：
本应用统一配置入口的 Dockview 面板（面板类型 `settings`）——左导航（全局/Agent 两组）+ 右侧配置页槽位，经 SettingsPageRegistry 分派渲染。
_Avoid_: 配置面板, 设置面板

**配置页**（Settings Page）：
设置中心内的注册单元，一个配置域一个页。新增配置页 = 实现组件 + 注册一条，框架零改动；Agent 组页 id 形态 = `agent.<cliId>.<page>`。

**全局组**：
应用级单例配置组，无需项目上下文即可编辑（快捷键、后台定时任务）。

**Agent 组**：
按编码 CLI 分节的配置组——每 CLI 一个分节，子页为该 CLI 的配置（基础配置 / Hooks 配置）。
_Avoid_: 项目组

**前端消费型配置**：
消费侧在前端（store/注册表）的配置域，后端纯透传存储；写通道为通用 `save_settings` 段写。

**后端消费型配置**：
消费侧在后端的配置域；写通道为域模块专用命令（校验 + 内存态 + 落盘一体）。

### 外观

**配色 token**（Color Token）：
UI 颜色的语义命名槽位（如 panelBg/focusBorder）。组件只引用 token、禁止硬编码颜色；token 定义在配色方案的 ui 段，经 colors.ts facade 导出。

**配色方案**（Color Scheme）：
一套完整配色定义的注册单元——ui token 取值 + 终端调色板 + 编辑器主题 + 三方库变量覆盖四段。仅暗色系（定位约束）。

**方案注册表**（SchemeRegistry）：
配色方案的模块级单例注册表——方案经 register 注册、setActive 激活；激活方案在 React 挂载前解析，切换方案需重载窗口生效。

**明度阶梯**（Lightness Ramp）：
UI 背景色的 6 档离散取值（l0 → l5），任何组件背景只能取其一。l0 永远留给内容区（「内容区最暗」原则），层级靠相邻档差表达，不叠加阴影/边框。

**发丝线**（Hairline）：
暗色界面分隔线的唯一形态——半透明白 1px 线，分默认与加强两档；禁止实色粗边框。

**双轨配色**：
UI 壳层与内容区各自独立的用色体系——壳层走明度阶梯 + 单强调色，终端 ANSI 16 色与编辑器语法色自成色板，两轨不混用 token。

**状态圆点**：
会话运行状态的可视化——小圆点，四态映射：工作→绿、注意→黄、完成→灰、错误→红；出现于会话行与终端页签。

**启动链 fail-safe 色**：
React 挂载前防白闪的硬编码色（index.html 底色、窗口底色、启动错误页），不在配色方案系统内，与方案色值手动同步。

### 基础设施

**IPC 层**：
前端唯一允许调用 Tauri `invoke` 的通信层；所有其他前端代码必须经此层的领域函数访问后端能力。

**文件监听器**（FileWatcher）：
基于操作系统文件变更通知的监听器，递归监听目录树，去抖后广播变更事件到前端。

**监听器池**（LruWatcherPool）：
LRU 缓存的文件监听器池，切换项目时经暂停/恢复切换活跃监听器，而非销毁重建。

**E2E 全局注入**：
应用在 E2E 构建下注入到 `window` 的测试辅助对象（就绪标志、项目创建、剪贴板写入、终端读写等），测试代码经 WebDriver `execute` 调用。

**硬约束**（Hard Constraints）：
不可违背的架构规则集，约束 IPC 边界、模块隔离、面板注册、配色、布局、会话、平台代码、权限、测试覆盖等方面；新增功能必须遵守。完整清单见 `.claude/CLAUDE.md`。

## 历史改名

代码标识符的历史改名映射，读旧 commit/旧代码时消歧用：

| 旧标识符 | 现行标识符 | 原因 |
|------|------|------|
| `hook-event` / `onHookEvent` | `agent-event` / `onAgentEvent` | 信号广播按 agent 域泛化 |
| `claude_history_*` / `claudeHistory` / `claude-history-*` | `agent_history_*` / `agentHistory` / `agent-history-*` | 历史会话模块泛化为 CLI 无关 |
| `claudeSession` / `setClaudeSession` | `agentSession` / `setAgentSession` | 会话模型泛化 |
