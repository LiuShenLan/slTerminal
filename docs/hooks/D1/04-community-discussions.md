# 社区讨论汇总：Claude Code Hooks 视觉反馈

> 调研日期：2026-07-25
> 数据来源：GitHub Issues、NPM 生态、Hacker News、社区博客、Reddit（无直接讨论）

---

## 1. 核心 GitHub Issues（anthropics/claude-code 官方仓库）

### 1.1 Issue #44093 — ModeChanged hook 事件 + 程序化 /color API（最重要）

- **标题**：`[FEATURE] Add ModeChanged hook event and programmatic /color API for mode-aware visual feedback`
- **状态**：已关闭（重复于 #42880）。canonical tracking issue #42880 仍为 Open。
- **日期**：约 2026 年 4 月
- **核心诉求**：
  1. 新增 `ModeChanged` hook 事件，在用户切换模式（Default/Plan/Edit/BypassPermissions）时触发
  2. 允许 hooks 通过 JSON 返回值、环境变量或 settings 键来设置输入框边框颜色
- **UX 依据**：边框颜色比状态栏文字更强——利用周边视觉和注意力前处理，用户无需阅读文字即可感知当前模式（如红色=旁路权限危险模式）
- **示例用例**：Shell 脚本根据模式自动设置边框颜色——绿色(Plan)、蓝色(Edit)、红色(Bypass)

### 1.2 Issue #17139 — 区分阻塞 vs 非阻塞 hook 状态的视觉指示

- **标题**：`[FEATURE] Clarify blocking vs non-blocking hook status in hook messages`
- **现状**：所有 hook 消息显示一致（如 `PreToolUse:Write hook success: Success`），无法区分哪些 hook 会阻塞操作
- **提议**：添加图标区分——阻塞型显示锁图标，非阻塞型显示信息图标
- **意义**：多自定义 hook 场景下，用户需快速理解哪些 hook 可能导致操作卡住

### 1.3 Issue #27412 — Task subagent 自定义进度渲染

- **标题**：`Feature: Custom progress rendering for background Task subagents`
- **现状**：长时间运行的 Task 子代理仅显示默认旋转动画（spinner），20-40 分钟多波操作无进度反馈
- **提出三个方案**：
  - 方案 A：新增 `onTaskProgress` hook 类型，定期触发，输出替换 spinner 区域
  - 方案 B：Task 工具的 `progress_renderer` 参数，指定 shell 命令定期调用
  - 方案 C：Claude Code 原生监听状态目录，自动渲染结构化 JSON 进度文件
- **状态**：开放，未实现。用户已实现"写入侧"（子代理写进度到 JSON），但"读取侧"（渲染）需原生支持

### 1.4 Issue #6454 — 提示完成后的桌面通知

- **标题**：`Add a way to display a notification when a prompt has finished`
- **核心诉求**：Stop hook 驱动桌面通知，用户不必一直盯终端
- **状态**：已有大量社区方案（见第 2 节），但官方未提供一键开关

### 1.5 Issue #45619 — notify-on-complete 插件

- **标题**：`[FEATURE] Add notify-on-complete plugin — ready-to-use Stop hook for completion notifications`
- **诉求**：将社区反复实现的 Stop-hook 通知方案内置为官方插件

### 1.6 Issue #32610 — 终端响铃

- **标题**：`[FEATURE] Terminal Bell`
- **诉求**：任务完成时触发终端响铃（`\a`），零依赖简单提示

### 1.7 Issue #7590 — 跨设备通知

- **标题**：`[Feature Request] Add Cross-Device Notifications for CLI Project Completion`
- **诉求**：任务完成时推送到手机（社区方案：curl ntfy.sh）

### 1.8 通知相关 Hook 事件缺陷

| Issue | 问题 | 状态 |
|-------|------|------|
| #13024 | 需要"等待用户输入"hook 事件 | 功能请求 |
| #13830 | Notification hook 需支持 AskUserQuestion 事件 | 功能请求 |
| #15872 | 需为 AskUserQuestion 工具添加 hook 支持 | 功能请求 |
| #59908 | AskUserQuestion 不触发 Notification hook（v2.1.140~2.1.143 回归） | 已在 v2.1.146 修复 |
| #51882 | idle_prompt 应同样在 AskUserQuestion 等待选择时触发 | 开放中 |
| #19627 | hook 触发存在高延迟（数秒延迟） | 已知问题 |

### 1.9 终端标题相关 Issues

| Issue | 描述 | 状态 |
|-------|------|------|
| #18326 | 将 session 名传播到终端标题（OSC 转义序列） | 功能请求 |
| #55397 | 终端标题包含 session 名——tmux -CC + iTerm2 关键需求 | 功能请求 |
| #31641 | 退出时清除终端标题为空——即使设了 CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1 | Bug |
| #16353 | /clear 不清除终端标题——多 tab 场景困惑 | 已知问题 |

### 1.10 其他视觉/UX 相关 Issues

| Issue | 描述 |
|-------|------|
| #10936 | Hook 状态标签在成功执行时错误显示"Hook Error" |
| #31344 | 斜杠命令展开无视觉指示——用户不知道 skill 是否被加载 |
| #41285 | SessionStart hook 的 systemMessage 在最小化 UI 模式下不再渲染 |
| #39099 | 需 PreCompact/PostCompact hook 事件（上下文压缩前后） |
| #3986 | Hooks UI 应允许编辑 hook |
| #36707 | 需交互式 MCQ/选择提示的 hook 事件 |
| #39499 | 允许 hooks 隐藏用户可见输出（静默重新提示） |
| #32551 | 抑制异步 hook 完成消息 |
| #41006 | PostThinking hook 事件——推理内容检查 |
| #53242 | 桌面模式不支持 webhook 通知（如 Slack） |
| #64018 | 无启动/session-init hook 事件——插件无法在首个提示前初始化 |
| #69380 | MessageDisplay hook 不通过 plugin hooks.json 注册（仅 settings.json 生效，Windows 限定） |
| #59429 | Recap 文本 hook 事件 |

---

## 2. 社区第三方工具生态全景

### 2.1 终端页签标题 + 状态指示

| 工具 | 平台 | 核心机制 | 安装方式 |
|------|------|---------|---------|
| **claude-code-tab-title** (franzvill) | macOS/Linux | OSC 转义序列设置终端页签标题：`* Topic`=忙碌，`· Topic`=空闲。遍历进程树找父 Claude 进程 pty 写入 OSC 序列（hooks 无控制 tty） | `/plugin marketplace add` |
| **claude-wsl** (fullstacktard) | Windows Terminal + WSL | 橙色圆点=运行中，橙色旋转动画=处理中，铃铛 emoji=完成/需权限，Windows Toast 通知 | `npm install -g claude-wsl` |
| **tabby-claude-status** | Tabby terminal（Windows 侧重） | 页签底部彩色边框、emoji 前缀、不确定进度条、活动标记点、任务栏闪烁、图标叠加、TTS 播报。映射 9 个 hook 事件到 5 种状态 | npm |
| **claude-tab-watcher** (dgr8akki) | macOS + iTerm2 + zsh | 琥珀色页签=等输入，蓝色=工作中，macOS 可点击通知跳转到正确页签 | 需 `jq` + `terminal-notifier` |
| **@ttigger/claude-status** | 跨平台 | HUD 状态栏：模型、项目、git 分支、上下文%、5h/7d 用量；桌面通知（仅 >30s 的轮次） | `npx @ttigger/claude-status install` |
| **claude-code-warp** (Warp 官方) | Warp 终端 | 原生 Warp 通知：任务完成、空闲提示、权限请求；内联 session 状态指示 | `/plugin marketplace add` |

### 2.2 桌面通知

| 工具 | 平台 | 核心机制 | 特点 |
|------|------|---------|------|
| **claude-notifications** (dimokol) | VS Code（跨平台） | 声音 + OS 通知横幅 + 点击跳到正确 VS Code 终端页签。状态栏显示 `🔔 Claude: Notify` / `🔕 Claude: Muted` | 多实例支持，自动安装 hooks |
| **claude-buzz** (ethanplusai) | macOS（iTerm2/Terminal/Warp/Kitty/Alacritty） | 原生 macOS 通知 + 点击跳到正确终端页签。5s 去抖，跳过当前已聚焦终端 | `npm install -g claude-buzz` |
| **CCNotify** (dazuiba) | macOS | terminal-notifier 桌面通知 + 任务耗时显示 + 点击跳回 VS Code 项目 | 本地日志 |
| **claude-nudge** | macOS | 一键安装 + 权限提示和完成通知 + 提示音（osascript）。可禁用完成通知 | `npx claude-nudge` |
| **job-finish** | Windows | 原生 Toast + 任务栏闪烁 + 通知音 + 焦点感知（VS Code 活跃时跳过）+ 点击聚焦 VS Code | npm |
| **@erica_s/claude-code-notify** | Windows | Toast + 任务栏闪烁 + "Open"按钮激活终端窗口 | npm |
| **claude-code-sound-notification** | macOS | 循环"进行中"声音（权限等待时）+ 庆祝声（完成时） | afplay |
| **claude-hook-notify** | 跨平台 | osascript/notify-send/PowerShell Toast + token 消耗统计 + API 错误独立告警 | npm |
| **claude-done** | 跨平台 | 原生 OS 通知 + 检测项目类型（Node/Rust/Python/Go 等）+ 点击聚焦终端 + 8 种语言 | npm |

### 2.3 终端包装器 / GUI / TUI（集成视觉状态）

| 工具 | 类型 | 平台 | 视觉状态特性 |
|------|------|------|------------|
| **claude-code-session-manager** | Electron GUI | Linux/macOS | 17 个内置页签（总览/终端/记忆/设置/权限/技能/插件/MCP/Hooks/调度器等），Cmd-K 命令面板 41+ 命令 |
| **Claude Code IDE** (Powellga) | Web IDE | Windows/macOS/Linux | 最多 8 个并发 session 页签 + 橙色脉冲点通知 + OS Toast + Monaco 编辑器面板 + session 跨刷新存活 |
| **Deckard** (gi11es) | 原生 macOS | macOS 14+ | Agent 页签（Claude/Codex/终端并存）+ 项目侧栏 + session 浏览器 + 状态徽章 + 上下文/配额/token 面板 |
| **Shelf** (Harukaon) | Tauri GUI | macOS/Linux | 工作区管理 + 终端页签（真实 PTY）+ 拖拽重排 + session 浏览器 + AI session 自动分类 |
| **CLUI CC** | Electron 覆盖层 | macOS 13+ | 聊天式页签 + 工具权限 UI + `⌥+Space` 切换 |
| **aimux** | TUI 多路复用器 | 跨平台（需 Bun） | 多 AI 页签（Claude/Codex/OpenCode/Grok）+ 分割窗格 + Git 模式 + 67 种主题 |
| **maestro-tui** | Rust TUI | 跨平台 | 双面板（shell + Claude Code）+ worktree-per-tab + Ratatui + Alacritty |
| **Multicode** | TUI | 跨平台（需 Bun） | 多项目页签 + 活动指示器 + Git worktree + session 持久化 |

---

## 3. Hacker News 讨论

### 3.1 "Claude Hooks: 6 hooks to make Claude Code cleaner, safer, and saner"

- **URL**：`https://news.ycombinator.com/item?id=44477756`
- **日期**：2025-07-06（中期，HN API 确认）
- **核心共识**：
  - Hook 系统弥合了重大功能缺口——Claude Code 的 commit 生成绕过常规 Git hooks
  - 确定性（deterministic）特性被广泛认为是相对 CLAUDE.md 指令的重大改进
  - 社区用例广泛：代码质量强制（自动格式化、lint 阻断）、上下文管理（CLAUDE.md 自动注入）、TDD 强制（测试不过不编辑）、自主循环（Stop hook 注入新 prompt）
- **相关 Show HN 帖子**：
  - "Recall – Persistent Memory for Claude Code via MCP Hooks" (`item?id=47189906`)
  - "Approve Claude Code permission requests from your phone via ntfy" (`item?id=47111171`)
  - "Claude Code Kit: Reliable Coding Using Claude Skills, Hooks and Command" (`item?id=45789960`)

### 3.2 "Claude Code now supports hooks"

- **URL**：`https://news.ycombinator.com/item?id=44429225`
- **日期**：2025-07-01（中期，hooks 功能首次发布，HN API 确认）
- **社区情绪**：总体积极，讨论集中在 hooks 相比 MCP 工具调用的优势（确定性 > 概率性）

---

## 4. 社区技术博客与教程

### 4.1 色码 Claude Code session 状态（びるへる/Biruheru）

- **URL**：`https://note.com/villhell_note/n/n3ca645dd6088`
- **标题**：`Color-coding Claude Code session states using Hooks [Part 2] Local Express server implementation`
- **方案概要**：
  - 使用本地 Express 服务器接收 hook 事件
  - 通过进程树向上查找终端 pty 写入 OSC 颜色序列
  - 事件→状态映射表（`mapEventToStatus`）：
    - `UserPromptSubmit` → **active**（绿色）
    - `Notification` → **waiting**（黄色 + 闪烁点）
    - `Stop` → **stopped**（蓝色）
    - `SessionEnd` → **idle**（浅灰色）
  - **关键教训**：Claude Code 只发送事件名——状态映射完全由用户定义
- **相关项目**：hiboard 终端网格视图，多 session 同时可视化

### 4.2 Hook 执行顺序可视化

- **URL**：`https://github.com/philoserf/claude-code-config/issues/264`
- **状态**：低优先级建议
- **内容**：建议在文档中用图表展示 hook 执行顺序

---

## 5. 技术机制总结

### 5.1 Hook 事件的视觉反馈能力矩阵

| Hook 事件 | 可驱动的视觉反馈 | 社区采用度 |
|----------|----------------|----------|
| `UserPromptSubmit` | 页签标题→忙碌态、边框颜色→工作中 | 高（tabby-claude-status、claude-wsl、hiboard） |
| `Stop` | 桌面通知、终端响铃、页签标题→空闲态、任务栏闪烁 | 极高（几乎所有通知工具） |
| `Notification` | 桌面通知、页签颜色→需输入态、声音循环 | 高（pidle_prompt / permission_prompt 子事件） |
| `SessionStart` | 页签初始化、HUD 更新 | 中 |
| `SessionEnd` | 页签重置、状态清理 | 中 |
| `PreToolUse` / `PostToolUse` | 页签状态→工作中、进度动画 | 中（tabby-claude-status） |
| `PermissionRequest` | 桌面通知、页签颜色警示 | 高 |
| `ModeChanged`（未实现） | 边框颜色随模式切换 | 高需求（#44093） |
| Task progress（未实现） | 自定义进度条替代 spinner | 高需求（#27412） |

### 5.2 实现路径分类

| 路径 | 代表工具 | 优点 | 限制 |
|------|---------|------|------|
| **OSC 转义序列**（写终端标题/颜色） | claude-code-tab-title、hiboard、claude-wsl | 终端原生、零依赖 | hooks 无控制 tty，需遍历进程树找父 pty；Claude Code 持续覆写标题 |
| **桌面通知 API** | claude-buzz、CCNotify、job-finish | 跨应用、可点击跳转 | 平台特定 API（osascript/notify-send/Toast） |
| **终端插件 API** | tabby-claude-status、claude-code-warp | 最丰富视觉（进度条、颜色、叠加层） | 绑定特定终端 |
| **监听文件 + 外部进程** | tabby-claude-status（JSON 文件）、hiboard（Express 服务器） | 解耦 Claude Code 进程 | 需额外守护进程 |
| **包装器/IDE** | claude-code-session-manager、Claude Code IDE、Shelf | 完整 UI 控制 | 引入整个替代前端 |

### 5.3 OSC 终端标题操作要点

- **OSC 0**（`\033]0;title\007`）= 图标名 + 窗口标题
- **OSC 2**（`\033]2;title\007`）= 窗口标题
- **OSC 1**（`\033]1;title\007`）= 仅页签标题（推荐，避免与 Claude Code OSC 0 冲突）
- **hooks 无控制 tty**：`/dev/tty` 从 hook 脚本静默失败，必须遍历进程树找到父 Claude 进程的 pty
- **Claude Code 持续覆写标题**：一次性写入会被下一渲染帧覆盖，需设 `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1`
- **环境变量**：`CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` 阻止 Claude Code 修改标题（在 `~/.claude/settings.json` 的 `"env"` 段设置）

---

## 6. slaTerminal 相关竞品分析结论

### 6.1 与 slTerminal 直接竞争的工具

以下工具与 slTerminal 定位最接近（终端模拟器/包装器 + Claude Code 集成）：

| 工具 | 视觉反馈亮点 | slTerminal 可借鉴 |
|------|------------|-----------------|
| **Claude Code IDE** | 橙色脉冲点通知、浏览器内多 session 页签、Monaco 编辑器面板 | 页签状态指示（颜色/图标动态切换） |
| **Shelf** | 终端页签（真实 PTY）+ session 浏览器 + 拖拽重排 | 页签内 session 管理 |
| **Deckard** | Agent 页签并存（Claude/Codex/终端）+ 状态徽章 | 状态徽章/图标指示 |
| **aimux** | 多 AI 页签 + Git 模式 + 67 主题 | 多 session 管理 |

### 6.2 slTerminal 差异化机会

slTerminal 是**唯一面向 Windows 10/11、专为 Claude Code CLI 调优的原生终端模拟器**——Tauri 2 + WebView2 + xterm.js + ConPTY 架构。竞品主要缺失：

1. **Windows 原生体验**：claude-wsl 需 WSL、tabby-claude-status 需 Tabby、Claude Code IDE 用浏览器。没有竞品在 Windows 上以原生终端形式深度集成 Claude Code 视觉状态
2. **渲染灵活性**：WebView2/CSS 可实现比 ANSI/OSC 转义序列更丰富的视觉反馈（渐变边框、动画指示器、自定义 overlay）
3. **架构优势**：两进程模型（Rust 后端拥有 OS 访问）可以直接监控 PTY 输出和进程状态，不依赖 hooks 文件轮询——实时性更高

### 6.3 视觉反馈实现建议

基于社区调研，slTerminal 的 hooks 视觉反馈可实现以下层级（按复杂度递增）：

1. **L1 - 页签标题动态更新**（低复杂度）：`UserPromptSubmit` → 页签标题前缀 `*`（工作中），`Stop` → `·`（空闲）。直接采纳 claude-code-tab-title 的模式
2. **L2 - 页签图标/颜色切换**（中复杂度）：`Notification` → 页签变黄/铃铛图标，`Stop` → 绿色/勾选图标。利用现有 `DefaultTab` 的 `api.updateParameters({ tabIcon })` 机制
3. **L3 - 进度指示器**（中高复杂度）：`PostToolUse` 或桌面通知触发不确定进度条。利用 Dockview 页签组件注入进度条 DOM
4. **L4 - 桌面通知**（高复杂度）：Windows Toast / 任务栏闪烁。可利用 Rust 后端的 `windows` crate 调用原生 Toast API
5. **L5 - 全窗口边框颜色模式指示**（最高复杂度）：类似 #44093 的提案，通过 CSS 主题变量切换窗口/输入框边框颜色

---

## 7. 信息来源索引

### GitHub Issues（anthropics/claude-code 官方仓库）
- https://github.com/anthropics/claude-code/issues/44093 — ModeChanged hook + /color API
- https://github.com/anthropics/claude-code/issues/17139 — 阻塞 vs 非阻塞 hook 视觉区分
- https://github.com/anthropics/claude-code/issues/27412 — Task 自定义进度渲染
- https://github.com/anthropics/claude-code/issues/20526 — Plan 生命周期 Hooks
- https://github.com/anthropics/claude-code/issues/6454 — 提示完成通知
- https://github.com/anthropics/claude-code/issues/45619 — notify-on-complete 插件
- https://github.com/anthropics/claude-code/issues/32610 — 终端响铃
- https://github.com/anthropics/claude-code/issues/7590 — 跨设备通知
- https://github.com/anthropics/claude-code/issues/13024 — 等待输入 hook
- https://github.com/anthropics/claude-code/issues/13830 — AskUserQuestion 通知 hook
- https://github.com/anthropics/claude-code/issues/15872 — AskUserQuestion hook 支持
- https://github.com/anthropics/claude-code/issues/59908 — AskUserQuestion 不触发 Notification hook
- https://github.com/anthropics/claude-code/issues/51882 — idle_prompt + AskUserQuestion
- https://github.com/anthropics/claude-code/issues/19627 — hook 触发高延迟
- https://github.com/anthropics/claude-code/issues/18326 — session 名→终端标题
- https://github.com/anthropics/claude-code/issues/55397 — 终端标题含 session 名
- https://github.com/anthropics/claude-code/issues/31641 — 退出清除标题 bug
- https://github.com/anthropics/claude-code/issues/16353 — /clear 不清标题
- https://github.com/anthropics/claude-code/issues/10936 — Hook 状态标签误报错误
- https://github.com/anthropics/claude-code/issues/31344 — 斜杠命令无视觉指示
- https://github.com/anthropics/claude-code/issues/41285 — SessionStart systemMessage 不渲染
- https://github.com/anthropics/claude-code/issues/39099 — PreCompact/PostCompact hooks
- https://github.com/anthropics/claude-code/issues/3986 — Hooks UI 编辑
- https://github.com/anthropics/claude-code/issues/36707 — MCQ 选择 hook 事件
- https://github.com/anthropics/claude-code/issues/39499 — 静默 hook 输出
- https://github.com/anthropics/claude-code/issues/32551 — 抑制异步 hook 消息
- https://github.com/anthropics/claude-code/issues/41006 — PostThinking hook
- https://github.com/anthropics/claude-code/issues/53242 — 桌面模式 webhook
- https://github.com/anthropics/claude-code/issues/64018 — 启动 hook 事件
- https://github.com/anthropics/claude-code/issues/69380 — MessageDisplay hook 不注册
- https://github.com/anthropics/claude-code/issues/59429 — Recap hook 事件

### 社区项目仓库
- https://github.com/franzvill/claude-code-tab-title — OSC 页签标题
- https://github.com/fullstacktard/claude-wsl — Windows Terminal + WSL 通知
- https://github.com/dimokol/claude-notifications — VS Code 扩展
- https://github.com/ethanplusai/claude-buzz — macOS 通知
- https://github.com/dgr8akki/claude-tab-watcher — macOS iTerm2 页签颜色
- https://github.com/dazuiba/CCNotify — 桌面通知
- https://github.com/EryouHao/claude-code-sound-notification — 声音提醒
- https://github.com/StanislavBG/claude-code-session-manager — Electron GUI
- https://github.com/Powellga/Claude-Code-IDE — Web IDE
- https://github.com/gi11es/deckard — macOS 原生 app
- https://github.com/Harukaon/shelf — Tauri 工作区管理器
- https://github.com/warpdotdev/claude-code-warp — Warp 终端集成

### NPM 包
- https://www.npmjs.com/package/@ttigger/claude-status — 状态栏 HUD
- https://www.npmjs.com/package/tabby-claude-status — Tabby 终端插件
- https://www.npmjs.com/package/claude-nudge — macOS 通知
- https://www.npmjs.com/package/job-finish — Windows 通知
- https://www.npmjs.com/package/claude-hook-notify — 跨平台通知
- https://www.npmjs.com/package/claude-done — 跨平台通知
- https://www.npmjs.com/package/@erica_s/claude-code-notify — Windows 通知
- https://www.npmjs.com/package/@brimveyn/aimux — TUI 多路复用器

### Hacker News 讨论
- https://news.ycombinator.com/item?id=44477756 — "6 hooks to make Claude Code cleaner, safer, and saner"
- https://news.ycombinator.com/item?id=44429225 — "Claude Code now supports hooks"

### 社区博客
- https://note.com/villhell_note/n/n3ca645dd6088 — 色码 session 状态（日文）
- https://github.com/philoserf/claude-code-config/issues/264 — Hook 执行顺序可视化

### 官方文档
- https://code.claude.com/docs/en/hooks-guide — Claude Code Hooks 指南

---

> 注：Reddit 搜索（`site:reddit.com "Claude Code" hooks`）未找到直接讨论结果——相关讨论集中在 GitHub Issues 和 Hacker News。此搜索受限于工具能力，可能存在假阴性（r/ClaudeCode 或 r/ClaudeAI 子版块可能存在未被抓取的讨论）。
