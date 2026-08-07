# slTerminal

面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。单窗口、暗色模式、GPU 加速渲染。

## 顶层组织

**项目**（Project）：
文件系统目录的顶层组织单元。一个项目包含多个操作页面，每个项目绑定一个根目录路径。
_Avoid_: workspace, repo

**操作页面**（OperationPage）：
项目内一个独立的工作页，拥有自己的面板布局和当前工作目录。页面间显隐切换且终端跨页面存活（详见 workspace 模块文档）。
_Avoid_: 操作页, tab, 标签页

**硬约束**（Hard Constraints）：
10 条不可违背的架构规则，编号 #1–#10，分别约束 IPC 边界、模块隔离、面板注册、配色、布局、会话、平台代码、权限等方面。任何新增功能必须遵守。

---

## 面板系统

**面板**（Panel）：
Dockview 布局中可托管的最小 UI 单元。每个面板属于一种面板类型、对应一个 React 组件，由布局引擎管理其位置和生命周期。
_Avoid_: 窗格, 视图

**面板类型**（Panel Type）：
面板的分类。当前有 6 种：terminal / editor / html / gitshow / diff / hooksConfig。新增面板类型需在面板注册表中显式注册。布局恢复时会过滤掉未注册的面板类型。

**面板实例**（Panel Instance）：
具体的一个面板，有唯一标识符，可被创建、关闭、拖拽分屏。

**终端面板**：
xterm.js 驱动的终端面板。每个终端面板对应一个 PTY 会话，提供命令行交互。渲染优先使用 WebGL，不可用时回退 DOM 渲染。每个终端面板在 React 挂载时创建新的 xterm.js 实例。

**编辑器面板**：
CodeMirror 6 驱动的文件编辑器面板。支持语法高亮热切换、Ctrl+S 保存、git 差异 gutter 标记、脏状态跟踪。

**页签标题**（Tab Title）：
Dockview 标签页上显示的文字。终端为 `terminal-N`（每页独立编号），编辑器为文件名（冲突时用相对路径）。标题由集中的标题管理器计算，布局持久化时不保存标题——从文件路径恢复。

**文件查看器注册表**（FileViewerRegistry）：
策略模式单例，根据文件扩展名决定用哪种面板类型打开文件。策略链式匹配、命中即返回，未命中回退编辑器面板；当前注册 `.html`/`.htm` → HTML 预览面板。由文件浏览器和 Commit 视图共用（详见 fileViewers 模块文档）。
_Avoid_: 文件类型映射

---

## 会话

**前端会话**（SessionInfo）：
前端 Zustand 中记录的会话元数据——会话标识、面板标识、当前工作目录、是否活跃。面板只订阅自己的会话切片，不自存会话数据。
_Avoid_: 会话（无歧义上下文下可使用，但需与 PTY 会话区分）

**PTY 会话**（PtySession）：
后端 Rust 中管理的伪终端进程实例。包含主端描述符、子进程句柄、标准输入写入器、输出读取线程、IPC 通道引用和环形缓冲区。
_Avoid_: 终端会话, shell 进程

**PTY 事件**（PtyEvent）：
PTY 输出流的带标签枚举——`Output`（终端输出字节）或 `Exit`（子进程退出码）。通过 IPC 通道从前端推送到后端。

**通道**（Channel）：
Tauri IPC 流式数据通道，用于 PTY 输出从后端连续推送到前端。可被替换（PTY 重连时换新，详见 pty 模块文档）。

**环形缓冲**（Output Ring Buffer）：
FIFO 字节队列。前端 Channel 断开时缓存 PTY 最新输出，重连时回放（容量与淘汰粒度见 pty 模块文档）。

**PTY 重连**：
前端页面切换后重新挂载终端面板时，替换已有 PTY 会话的 Channel 并回放环形缓冲的操作。避免了杀进程重建。

**历史会话**（Claude Session）：
claude CLI 在某项目目录运行产生的 transcript 会话（`~/.claude/projects/` 下 `<uuid>.jsonl`），由 claude_history 模块扫描/恢复。与前端会话、PTY 会话是不同概念（详见 claude_history 模块文档）。

**应用运行期**：
应用进程的一次运行——ID 生成等"单运行期内唯一"语义的准确表述。

---

## 工作区

**工作区**（Workspace）：
多页面 Dockview 布局的根 UI 组件。管理所有操作页面的 Dockview 实例、页面切换、布局持久化和侧边栏布局。

**布局**（Layout）：
一个操作页面内面板排列的 JSON 序列化数据。包含面板的位置、大小、分组方向等信息。布局操作只通过集中的序列化/反序列化模块进行。

**布局恢复守卫**：
防止程序化恢复布局时触发布局变更写回存储的机制（机制细节见 workspace 模块文档）。

**面板注册表**（Panel Registry）：
面板类型到 React 组件的映射表。工作区创建面板时从此表查找对应组件。也是布局恢复时的面板类型白名单。

---

## 侧栏

**活动栏**（Activity Bar）：
应用最左侧的窄条，容纳侧栏视图按钮。按钮可通过鼠标左键拖拽在上区/下区之间移动，决定对应视图的展示半区。

**侧栏视图**（Side View）：
活动栏按钮对应的可开关内容视图（如项目列表视图、文件浏览器视图）。点击按钮开关视图；视图在侧栏区中展示。
_Avoid_: 页面, 面板

**项目列表**：
项目/操作页面二级树，以侧栏视图形式展示。管理项目 CRUD、操作页面 CRUD、页面切换导航。新建项目或操作页面时布局为空——不自动创建终端面板，由用户手动添加。

**Commit 视图**：
以侧栏视图形式展示当前项目的 Git 变更概览。包含两个可折叠列表——变更列表（Changes）列出已跟踪文件的增/删/改/重命名/冲突，未跟踪文件列表（Unversioned Files）列出未纳入版本控制的新文件。双击文件条目按 git 状态分派到对应面板类型。

**侧栏区**（Side Bar）：
侧栏视图共享的展示区域，位于活动栏与主区之间。垂直划分为上区和下区两个半区。

**上区 / 下区**（Top Zone / Bottom Zone）：
侧栏区的两个半区。每个活动栏按钮经拖拽归属于其一；同一半区同时只展示一个侧栏视图（最后点击者），不同半区可各展示一个。

---

## 文件系统

**文件浏览器**：
以侧栏视图形式展示的文件树。展示活跃项目根目录的文件结构，支持创建/删除/重命名/打开文件，git 状态着色，并对文件系统变更事件做增量刷新。双击文件通过文件查看器注册表决定用哪种面板类型打开——`.html`→HTML 预览面板，其余→编辑器面板。

**文件监听器**（FileWatcher）：
基于操作系统文件变更通知的监听器。递归监听目录树，去抖后广播变更事件到前端。

**监听器池**（LruWatcherPool）：
LRU 缓存的文件监听器池。切换项目时通过暂停/恢复切换活跃监听器，而非销毁重建（Windows 上递归注册大目录树耗时，详见 notify 模块文档）。

**暂停 ≠ 停止**：
监听器暂停仅阻止事件上报前端，watcher 线程和 OS 监听句柄保持活跃；停止则销毁 watcher 线程、释放 OS 句柄。

---

## 版本控制

**Git 状态**：
工作区文件相对于 HEAD 的状态——已修改、已添加、已删除、已重命名、未跟踪、冲突、已忽略。

**差异块**（DiffHunk）：
文件中连续变更行的范围。每个差异块记录旧文件和新文件中的起止行号（1-based），驱动编辑器行号 gutter 的颜色标记。

**Git 仓库缓存**：
工作目录到 git2 仓库对象的映射缓存。避免重复的 `Repository::discover` 遍历。目录切换时清理。

**变更列表**（Changes）：
Commit 视图中展示已跟踪变更文件的分组——状态为 added、modified、deleted、renamed、conflict 的文件。按相对路径字母序排列。
_Avoid_: 暂存区列表

**未跟踪文件列表**（Unversioned Files）：
Commit 视图中展示未跟踪文件的分组——状态为 untracked 的文件。按相对路径字母序排列。

**状态分派**（Status Dispatch）：
Commit 视图中双击文件条目时，git 状态到面板类型的映射规则。modified/renamed/conflict → diff 面板（双栏对比），deleted → gitshow 面板（只读查看 HEAD 版本），added/untracked → editor 面板（普通编辑）。页签标题带后缀区分（如 `(git diff)`、`(git delete)`、`(git add)`）。

---

## IPC 通信

**IPC 层**：
前端唯一允许调用 Tauri `invoke` 的通信层。所有其他前端代码必须通过此层的领域函数访问后端能力（PTY、文件、git、剪贴板、对话框、设置、通知）。

**IPC 模块映射**：
前端 IPC 模块与后端功能模块一一对应——PTY、文件系统、Git、设置/项目、文件监听、hooks 注入与配置、历史会话、剪贴板、对话框、通知、窗口、外部链接（完整映射见 ipc 模块文档）。

---

## Shell 集成

**Shell 解析**：
默认 shell 的探测链——PowerShell 7 → Windows PowerShell 5.1 → cmd.exe。用户可在设置中覆盖。

**Shell 集成脚本**：
编译时嵌入的 PowerShell 脚本，覆盖 `prompt` 函数注入 OSC 序列（cwd 跟踪 + 提示符边界/退出码 + UTF-8 编码修复），供宿主跟踪工作目录与命令边界（详见 pty 模块文档）。

**OSC 7**：
ANSI 转义序列 `ESC ] 7 ; file://<路径> ESC \`，用于向终端报告当前工作目录。

**OSC 133**：
ANSI 转义序列族——A 为提示符前标记、B 为提示符后标记、D 为上一命令退出码。宿主据此跟踪提示符边界，无需解析提示符文本。

---

## 测试

**E2E 测试**：
端到端测试。经 WDIO + embedded driver 驱动真实 WebView2（零 msedgedriver 依赖，详见 e2e-tests 模块文档）。

**E2E 全局注入**：
应用在 E2E 构建下注入到 `window` 的测试辅助对象（就绪标志、项目创建、剪贴板写入、终端读写等），测试代码经 WebDriver `execute` 调用。

---

## Hooks 宿主侧增强

**CC hooks**（Claude Code Hooks）：
Claude Code CLI 内置的 hooks 机制。settings.json `hooks` 字段的三层嵌套配置（事件 → matcher 组 → handler 数组），30+ 生命周期事件、5 种 handler 类型（command/http/mcp_tool/prompt/agent），经 stdin/stdout JSON 与 exit code 0/2 语义通信。
_Avoid_: 终端 hooks, slTerminal hooks（后者指未采纳的方向 B）

**宿主侧增强**：
slTerminal 不改动 Claude Code 本身，为其 hooks 提供状态可视化与配置管理外围能力的功能方向（方向 A）。与"方向 B"（slTerminal 自有终端级 hook 系统，未采纳）相对。

**信号文件通道**：
slTerminal 感知 CC hook 事件的主通道。注入用户配置的 hook 脚本把事件写为 JSON 信号文件到约定路径，后端监听目录并经 IPC 推送前端。辅以 transcript JSONL 被动解析补充上下文用量。

**SLTERM_PANEL_ID**：
pty_spawn 时注入子进程环境块的环境变量。经 shell → claude → hook 脚本继承链传递并写入信号文件，实现会话→页签的精确路由（不用 cwd 猜测）。

**四态**：
页签图标的四种 claude 会话状态——工作⚡、注意🟡、完成✅、错误❌。F3 专有术语，触发源映射见 panels 模块文档。

**注入 / 卸载**：
把 slTerminal 状态上报 hook 配置 merge 写入 user 层 settings.json（注入），或干净移除配置段与脚本文件（卸载）。仅手动按钮触发，不做启动引导。

**三层配置**：
CC settings.json 的三个编辑层级——user（`~/.claude/settings.json`）、project（`.claude/settings.json`）、local（`.claude/settings.local.json`）。优先级 local > project > user。

**双模式面板**：
hooks 配置编辑面板的两种编辑模式——GUI 表单（Master-Detail）与 JSON 编辑器（CM6 + Schema 校验），顶部切换、实时同步编辑同一份配置。

**Agent Status 视图**：
侧栏视图（id `agent-status`，图标 🤖），一屏总览当前活跃项目所有运行中的 claude 会话——四态、上下文用量条、最后事件时间，点击行跳转聚焦对应终端页签。会话退出行即移除，非历史列表（详见 agentStatus 模块文档）。
_Avoid_: agent 面板, 会话列表

**会话行**：
Agent Status 视图中的一行，对应一个**运行中的 claude 会话**（非终端面板——纯 shell 终端无行）。经 OSC 133 C（命令检测，未注入 hooks 的会话也有行，四态 🟡、用量条不可用态）或 SessionStart（hook 事件）建立；上下文用量由信号文件 transcriptPath 定位 transcript JSONL 后端解析，事件驱动更新、不轮询。
_Avoid_: 终端行

---

## 配色

**配色 token**（Color Token）：
UI 颜色的语义命名槽位（如 panelBg/focusBorder）。组件只引用 token，禁止硬编码颜色（硬约束 #6）。token 定义在配色方案的 ui 段，经 colors.ts facade 导出供组件消费。

**配色方案**（Color Scheme）：
一套完整配色定义的注册单元——ui token 取值 + 终端调色板 + 编辑器主题引用与覆盖 + 三方库变量覆盖四段。仅暗色系（定位约束）。当前内置 darcula 一套。

**方案注册表**（SchemeRegistry）：
配色方案的模块级单例注册表。方案经 register 注册、setActive 激活；激活方案在启动时（React 挂载前）解析，切换方案需重载窗口生效。

**启动链 fail-safe 色**：
React 挂载前防白闪的硬编码色（index.html body 底色、tauri.conf.json 窗口底色、main.tsx 超时错误页）。不在配色方案系统内，与方案色值手动同步。

---

## 同义词/废弃术语

| 废弃 | 替代 | 原因 |
|------|------|------|
| 操作页 | 操作页面 | 省略"面"字不正式，统一全称 |
| 会话（当指 PTY 时） | PTY 会话 | 前端 SessionInfo 和后端 PtySession 是不同概念，不可混用 |
| 窗格 | 面板 | 统一使用 Dockview 的 "panel" 对译 |
| 标签页 | 页签标题 / 操作页面 | "标签页"既可能指 Dockview tab（标题），也可能指 OperationPage（页面切换），避免多义 |
| 四态（当指侧栏布局/Commit 状态机时） | 四布局态 / 四渲染态 | F3 页签四态为专有术语，其他概念避免裸用 |
| 双通道（当指 hooks 消费架构时） | notify+轮询双通道 | 与 F5 行建模"双通道建行"区分 |
