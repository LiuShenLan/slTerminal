# D5：其他优秀项目的 Hooks 优化案例 -- 汇总报告

> 研究日期：2026-07-25
> 子报告：D5a (AI 编程工具) | D5b (Claude Code 社区) | D5c (终端模拟器) | D5d (IDE Task 系统)

---

## 研究方法

4 个子代理并行检索，每个覆盖一个子领域。各子代理先多轮 WebSearch，再对有价值 URL 做 WebFetch 获取详细内容。总计执行 116 次 WebSearch/WebFetch 调用，产出 4 份独立的 md 报告。

| 子报告 | 文件 | 规模 | 覆盖项目 |
|--------|------|------|---------|
| D5a | [D5a-ai-tools-hooks.md](./D5a-ai-tools-hooks.md) | 628 行 | Cursor、Windsurf、Copilot CLI、Codex CLI、Gemini CLI、aider、Claude Code |
| D5b | [D5b-claude-code-community.md](./D5b-claude-code-community.md) | 490 行 | GitHub 15+ 仓库、Reddit/HN 20+ 讨论、官方文档、6 通知工具 |
| D5c | [D5c-terminal-hooks-visualization.md](./D5c-terminal-hooks-visualization.md) | 400 行 | Windows Terminal、iTerm2、Warp、WezTerm |
| D5d | [D5d-ide-task-system-ui.md](./D5d-ide-task-system-ui.md) | 约 500 行 | VS Code (Tasks/Extensions/Terminal)、JetBrains (External Tools/File Watchers/Run Configurations) |

---

## 一、核心发现：行业共识与趋势

### 1.1 经 stdio 传输 JSON 成为行业通行模式

Cursor、Windsurf、Gemini CLI、Claude Code 4 个工具使用原始 stdin→stdout JSON 协议，享有相同的传输契约：

- **stdin**：JSON 格式事件上下文（tool name、input、file path 等）
- **stdout**：JSON 格式决策（`{continue, permission, decision, systemMessage, reason}`）
- **exit 0**：成功/放行
- **exit 2**：阻断操作
- **其他非零**：非阻塞警告

> **注意**：Copilot CLI 和 Codex CLI 虽然也经 stdio 传输 JSON，但使用的是 **JSON-RPC over stdio**（结构化双向协议，含 request/response/notification），与上述 4 工具的原始 stdin→stdout 单次调用模型在协议层**不兼容**。Copilot CLI 的 SDK 通过 `joinSession()` 建立持久化 JSON-RPC 连接，Codex CLI 的 App Server 使用双向 JSON-RPC（JSONL 格式）。跨工具统一接口（`agent-hooks`、`polyhook`）的存在本身反证了各工具协议层的不兼容性。

slTerminal 的 hook 设计应遵循原始 stdin→stdout JSON + exit 0/2 语义，确保与主流 hook 脚本生态互操作。

**来源**：Cursor 官方文档 (https://cursor.com/docs/agent/hooks)、Claude Code 官方文档 (https://code.claude.com/docs/en/hooks)、Windsurf 文档 (https://docs.windsurf.com/windsurf/cascade/hooks)、Copilot CLI SDK (https://github.com/github/copilot-sdk)、Codex App Server (https://openai.com/index/unlocking-the-codex-harness/)

### 1.2 Hook 事件粒度呈三级分化

| 粒度 | 代表工具 | 事件数 | 特点 |
|------|---------|--------|------|
| 粗粒度 | aider | 0（无 hook 系统） | 仅 post-edit lint/test 自动反馈 |
| 中粒度 | Cursor、Windsurf、Claude Code | 12-18 | pre/post tool execution + MCP + shell + session 生命周期 |
| 细粒度 | Gemini CLI | 11 | BeforeModel/AfterModel/BeforeToolSelection 等可修改 LLM 请求/响应的事件 |

slTerminal 定位为终端级 hook（非应用层），应聚焦中粒度：PTY 输入/输出生命周期、命令执行前后、OSC 序列检测。

**来源**：各项目官方文档 URL 见 D5a 子报告

### 1.3 Plugin = Skills + MCP + Hooks 三位一体

Copilot CLI、Codex CLI、Gemini CLI、Claude Code 均采用此模型：
- **Skills**：Markdown 文件 + YAML frontmatter，定义 agent 行为
- **MCP**：标准化工具协议（`stdio` / `http` / `websocket`）
- **Hooks**：JSON 配置 + 外部脚本，生命周期回调

**来源**：Claude Code Plugin 参考 (https://code.claude.com/docs/en/plugins-reference)

### 1.4 三级配置合并是通用模式

所有工具均支持 User / Project / Local 三层（或 Managed / User / Project / Local 四层）配置合并：

| 层级 | 路径 | 版本控制 | 优先级 |
|------|------|---------|--------|
| Managed/System | 企业网关推送 | N/A | 最高 |
| Local | 项目级 `.local.json` | 不入 git | 高 |
| Project | 项目级 `settings.json` | 入 git（团队共享） | 中 |
| User | `~/` 全局 | 不跟踪 | 低 |

合并规则：数组跨层累加（如 `permissions.allow`），标量值高层覆盖低层。

**来源**：Claude Code Settings 文档 (https://code.claude.com/docs/en/settings)、Windsurf 文档、Codex CLI config 文档

---

## 二、终端模拟器的 Hook/Event 可视化（D5c 核心发现）

### 2.1 iTerm2 Triggers -- 最成熟的"条件->动作"模型

iTerm2 的 Triggers 系统是终端 hook 可视化的**最佳参考**：

- **条件**：regex 匹配终端输出
- **动作**：26 种内建动作（Bounce Dock Icon、Post Notification、Run Command、Send Text、Highlight Text 等）
- **管理**：GUI 表格编辑器，逐行显示 regex → action → parameters，支持拖拽排序
- **Instant 模式**：部分 trigger 在终端渲染前执行（零延迟）
- **可编程**：Python API 提供 Hooks、Variables、Daemons、Custom Control Sequences

对 slTerminal 的启示：提供一个类似表格的可视化 hook 配置面板，用户逐行配置触发条件 + 动作。

**来源**：https://iterm2.com/documentation-triggers.html (2026-07)

### 2.2 Warp DCS Hooks -- shell 生命周期跟踪的现代化方案

Warp 通过 DCS（Device Control String）转义序列自动注入 shell RC 文件，实现：

| DCS Hook | 对应 Shell 事件 |
|----------|----------------|
| Precmd | 提示符出现前（命令完成） |
| Preexec | 命令执行前 |
| CommandFinished | 命令完成 + 退出码 |

结合 **OSC 9**（桌面通知）和 **OSC 777**（自定义通知），形成完整的"shell 事件 → 通知/动作"管线。

Warp 的 **Blocks** 概念将每个命令的输出隔离为独立块，支持折叠/展开/复制/书签——这是终端输出结构化的另一个方向。

slTerminal 已有 OSC 133（提示符边界检测），可扩展为更完整的 shell 生命周期 hook 体系（OSC 133 C/D → hook 触发）。

**来源**：https://docs.warp.dev/features/sessions/dcs-hooks (2026-07)

### 2.3 WezTerm Lua Event System -- 代码配置的最高自由度

12 个 window 内置事件（`format-tab-title`、`update-status`、`bell`、`open-uri` 等）+ 2 个 GUI 生命周期事件（`gui-startup`、`gui-attached`）+ 自定义事件（`wezterm.emit`）+ `action_callback` 内联回调。

**优势**：Lua 代码配置自由度最高，支持运行时 `set_config_overrides()` 动态修改配置。
**劣势**：可视化程度为零——所有配置在代码中，用户需要编程能力。

**来源**：https://wezterm.org/config/lua/window-events/index.html (2026-07)

### 2.4 Windows Terminal -- 最简单的绑定模型

Actions + Keybindings + Fragment Extensions（JSON 文件放置即可注入 profile）。但 hook 能力极弱——没有终端输出匹配的条件触发，也没有通知机制。

**来源**：https://learn.microsoft.com/en-us/windows/terminal/json-fragment-extensions (2026-07)

### 2.5 四个终端的可视化对比

| 维度 | Windows Terminal | iTerm2 | Warp | WezTerm |
|------|-----------------|--------|------|---------|
| Event/Hook 类型 | Actions + Keybindings | Triggers (26 动作) + Python Hooks | DCS Hooks + OSC 通知 + Blocks | 12 内置 Events + 自定义 Events |
| 配置方式 | 纯 JSON | GUI 表格 + Python 脚本 | GUI + Shell 转义序列 | 纯 Lua 代码 |
| UI 可视化 | Settings UI + Command Palette | **Triggers 表格编辑器** + Script Console | Settings UI + Warp Drive + Agent 状态 | Command Palette + 状态栏 |
| 条件触发 | 无（仅按键驱动） | **强**（regex 匹配 + Instant 模式） | 强（DCS shell 生命周期） | 弱（仅事件驱动） |
| 可编程性 | 低 | 高 | 中 | 高 |
| 通知能力 | 无 | Post Notification + Bounce Dock | 内置桌面通知 + OSC 9/777 | 依赖外部工具 |
| 第三方扩展 | Fragment Extensions | Python API | Agent SDK | Lua 模块 |

---

## 三、Claude Code Hooks 社区实践（D5b 核心发现）

### 3.1 社区用例热度排名

| 排名 | 用例 | 出现频率 | 关键技术 |
|------|------|---------|---------|
| 1 | **危险命令拦截** (rm -rf / git push --force / DROP TABLE) | 极高（几乎所有仓库） | PreToolUse + exit 2 |
| 2 | **自动格式化** (Prettier/Ruff/gofmt) | 极高 | PostToolUse + Edit\|Write |
| 3 | **通知** (Desktop/ntfy/Slack/Discord/飞书/钉钉) | 高（6+ 独立工具） | Notification + Stop + webhook |
| 4 | **SessionStart 上下文注入** (git status/项目结构/TODO) | 高 | SessionStart + stdout |
| 5 | **敏感文件保护** (.env/*.key/*.pem/.git/) | 高 | PreToolUse + exit 2 |
| 6 | **Stop Hook 质量门禁** (lint/test 不过则不能停止) | 中高 | Stop + exit 2 |
| 7 | **持久化记忆** (跨 session 上下文) | 中（3 独立项目） | SessionStart+SessionEnd+PreCompact |
| 8 | **Token 优化** (anatomy 索引 + 选择性读取) | 中 | PreToolUse(Read) + PostToolUse |

**来源**：D5b 子报告，基于 GitHub 15+ 仓库、Reddit/HN 20+ 讨论、dev.to 博客等

### 3.2 社区共识的"最小可行 hook 链"

Reddit r/ClaudeAI 社区（2026.05）推荐的入门前 4 个 hook：

```
PreToolUse (Bash)       → 拦截 rm -rf / --force / DROP TABLE
PreToolUse (Edit|Write) → 保护 .env / lock files / credentials
PostToolUse (Edit|Write) → 自动 lint + 审计日志（非阻塞）
SessionEnd               → 状态快照 + 通知
```

**来源**：https://reddit.com/r/ClaudeAI (2026.05 讨论)

### 3.3 官方 10 大 Hook 模式

Anthropic 官方插件开发仓库总结了 10 种 hook 模式：

| # | 模式 | 事件 | 用途 |
|---|------|------|------|
| 1 | Security Validation | PreToolUse | 拦截敏感路径写入 |
| 2 | Test Enforcement | Stop | 未跑测试阻止停止 |
| 3 | Context Loading | SessionStart | 检测项目类型并注入上下文 |
| 4 | Notification Logging | Notification | 审计日志 |
| 5 | MCP Tool Monitoring | PreToolUse | regex matcher 拦截破坏性 MCP 操作 |
| 6 | Build Verification | Stop | 要求构建成功才允许 Stop |
| 7 | Permission Confirmation | PreToolUse | 危险 Bash 命令要求确认 |
| 8 | Code Quality Checks | PostToolUse | Write/Edit 后自动 linter/formatter |
| 9 | Temporarily Active Hooks | PreToolUse | flag 文件控制 hook 开关 |
| 10 | Configuration-Driven Hooks | SessionStart | JSON 配置文件驱动可配置行为 |

**来源**：https://github.com/anthropics/claude-plugins-official/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md (2026)

### 3.4 关键教训

1. **`exit 1` 不阻塞，必须用 `exit 2`**（社区 #1 踩坑点）
2. **PostToolUse 无法撤销已执行的操作**（只能反馈 stderr）
3. **Hook 变更需重启 Claude Code**（或执行 `/hooks`）
4. **同一事件多个 hook 并行执行**，相同命令自动去重
5. **Hooks 100% 执行 vs Skills 约 50-80% 触发率**——如果必须执行，用 hooks 而非 CLAUDE.md 提示词（Skills 触发是概率性的，hooks 是确定性的）

**来源**：lakshminp.com 博客 (https://lakshminp.com/2026/01/claude-code-hooks/)、Reddit 社区讨论

---

## 四、IDE Task/Extension 系统 UI 参考（D5d 核心发现）

### 4.1 VS Code Tasks 系统

**核心架构**：`tasks.json`（JSON Schema 2.0.0）声明式配置 + 三种执行类型：

| 执行类型 | 说明 | 适用场景 |
|---------|------|---------|
| ShellExecution | 通过 shell 执行命令字符串 | 通用命令 |
| ProcessExecution | 直接执行可执行文件 | 编译器等 |
| CustomExecution | 回调返回 `Pseudoterminal`，完全控制 I/O | 自定义终端集成 |

**Problem Matchers**：正则捕获组将任务输出解析为 Problems 面板的结构化条目（文件、行、列、严重级别、消息）。支持后台 watching matcher（持续监控）。

**输出面板**：任务输出直接集成到**终端面板**（非独立 Output 面板）。通过 `presentation` 配置控制：
- `reveal`: always / silent / never
- `focus`: 是否自动聚焦
- `panel`: shared / dedicated / new
- `clear`: 是否清空之前输出

**TaskProvider API**：扩展可注册自定义 task type，提供 `provideTasks` + `resolveTask`。

对 slTerminal 的启示：
- tasks.json 的 `presentation` 字段设计可直接借鉴（reveal/focus/panel/clear 控制 hook 输出面板行为）
- Problem Matchers 的 regex 捕获组模式可应用于 hook 输出的结构化解析

**来源**：VS Code 官方文档 (https://code.visualstudio.com/docs/editor/tasks)，通过 Context7 检索，2026-07-25

### 4.2 VS Code Terminal Profiles API

```
terminal.integrated.profiles.windows  →  JSON 配置 profiles
registerTerminalProfileProvider       →  Extension API 注册
Pseudoterminal 接口                   →  完全控制输入输出
```

`Pseudoterminal` 接口提供 `onDidWrite`（输出事件）、`handleInput`（输入事件）、`open`/`close` 生命周期方法——与 slTerminal 的 PTY 层概念高度对应。

**来源**：VS Code API 文档 (https://code.visualstudio.com/api/references/vscode-api#Terminal)

### 4.3 JetBrains Run Configurations

五层架构：`ConfigurationType` -> `ConfigurationFactory` -> `RunConfiguration` -> `SettingsEditor` -> `RunProfileState`。

RunConfiguration 是**"配置 + UI 编辑器 + 执行器"三位一体**——配置数据、GUI 设置面板、进程启动逻辑集中在同一个类中。

- **OSProcessHandler**：管理进程生命周期（start/stop/destroy）
- **ConsoleView**：输出显示在 Run tool window
- **Reworked Terminal**（2025.2+ 默认）：`TerminalView` / `TerminalOutputModel` / `TerminalSendTextBuilder` API

与 VS Code 的核心差异：
| 维度 | VS Code | JetBrains |
|------|---------|-----------|
| 配置方式 | JSON 声明式 | Java 类型化 API |
| 输出显示 | 终端面板 | 独立 Console/Run 视图 |
| 扩展方式 | Pseudoterminal 接口 + TaskProvider API | RunConfiguration 子类化 + SettingsEditor |
| UI 编辑器 | JSON 文件手动编辑 + 部分 GUI | 完整 GUI 表单编辑器 |

**来源**：JetBrains Platform SDK (https://plugins.jetbrains.com/docs/intellij/run-configurations.html)，2026-07-25

---

## 五、AI 编程工具 Hook 系统横向对比（D5a 核心发现）

| 维度 | Cursor | Windsurf | Copilot CLI | Codex CLI | Gemini CLI | aider | Claude Code |
|------|--------|----------|-------------|-----------|------------|-------|-------------|
| Hook 事件数 | 21（3.11+） | 11+1（含 transcript） | ~13（JSON+SDK 两套方式，事件重叠） | ~10（未独立验证） | 11 | 0 | 14+（12 核心 + 2+ 实验性） |
| Hook 类型 | command | command | command/script | command | command | N/A | command/prompt/http/mcp_tool/agent |
| 配置方式 | `.cursor/hooks.json`（Hooks 4 层）+ `.cursor/rules/`（Rules 4 层，独立维度） | `.windsurf/hooks.json` 三级合并 | `.github/hooks/*.json` + `extension.mjs` | `hooks.json` / `config.toml` | `.gemini/settings.json` 多级 JSON | `.aider.conf.yml` (YAML) | `.claude/settings.json` 四层合并 |
| Plugin 系统 | Rules+Hooks+MCP+Skills | Hooks+Rules+MCP+Memories | Plugin=agents+skills+hooks+MCP | Plugin=Skills+MCP+Hooks | Extension=清单+agents+skills+hooks | 无 | Plugin=Skills+Agents+Hooks+MCP+Themes |
| 终端 UI | IDE 内嵌面板 | IDE 内嵌 + Cascade | Ink TUI (React+Flexbox) | App Server JSON-RPC | Ink TUI | Python prompt_toolkit | Ink TUI (React-in-terminal) |
| 阻断机制 | `permission:deny` + `continue:false` | pre-hook exit 2 | `deny` + `agentStop` | 退出码非 0 | 非 0 退出码 + `systemMessage` | N/A (仅 lint/test) | exit 2 + `decision:"block"` |
| 细粒度事件 | 有 (afterAgentThought/afterAgentResponse) | 无 | 无 | 无 | 有 (BeforeModel/AfterModel) | 无 | 无 |

> **注意**：Copilot CLI 的 JSON 配置文件 13 个事件与 SDK 6 个事件（onSessionStart 等）高度重叠——它们是同一事件集在不同配置层的表达，不应简单相加为 19。Codex CLI 使用 JSON-RPC over stdio（JSONL），与 Cursor/Windsurf/Claude Code 的原始 stdin→stdout 协议层不兼容。Windsurf 收购金额（"约 2.5 亿美元"）未经公开来源独立核实。

**来源**：各项目官方文档，URL 详见 D5a 子报告

---

## 六、对 slTerminal 的关键启示（按 4 个方向汇总）

### 6.1 视觉反馈（来自 D5c + D5d）

1. **iTerm2 Triggers 表格编辑器**是最佳参考：逐行 regex → action → parameters，支持拖拽排序。slTerminal 可直接借鉴此交互模式做 hook 管理界面。
2. **Warp Blocks**将命令输出隔离为独立块（折叠/展开/复制/书签），提供了终端输出结构化的思路。
3. **VS Code `presentation` 配置**（reveal/focus/panel/clear）可直接用于 slTerminal hook 输出的面板行为控制。
4. **WezTerm 状态栏**（`update-status` + `update-right-status`）提供周期性渲染自定义信息的模式——slTerminal 可在终端底部注入 hook 状态指示器。

### 6.2 配置管理（来自 D5a + D5b + D5d）

1. **JSON Schema 驱动配置**（VS Code tasks.json 模式）：为 hook 配置定义 JSON Schema，提供自动补全和校验。
2. **三层配置合并**（User / Project / Local）是行业标准，slTerminal 已有 `settings.json` 可自然扩展。
3. **Problem Matchers 模式**（VS Code）：正则捕获组将 hook 输出解析为结构化条目——可用于"代码检查"类 hook 的结果可视化。
4. **Plugin 目录结构**（Claude Code）：`hooks/` + `hooks.json` + 脚本文件的标准化布局，允许多个 hook 文件组合分发。

### 6.3 日志可视化（来自 D5b + D5d）

1. **VS Code Problems 面板**：结构化输出（文件 + 行号 + 严重级别 + 消息），是 hook 执行结果最成熟的展示方式。
2. **Notification Hook 生态**（6+ 独立工具）：社区已形成标准化的通知输出模式——Desktop/ntfy/Slack/Discord/飞书/钉钉。
3. **DCS Hook + OSC 通知**（Warp）：转义序列驱动的通知是终端原生方案，不依赖外部 IPC。
4. **审计日志**（社区共识）：PostToolUse hook 的审计日志是第二热门的 hook 应用（仅次于安全拦截）。

### 6.4 PTY 集成（来自 D5c + D5a）

1. **OSC 133 C/D 扩展**：slTerminal 已有 OSC 133（提示符边界检测），可扩展为完整的 shell 生命周期 hook 体系（Precmd/Preexec/CommandFinished → hook 触发）。
2. **Warp DCS Hooks**：自动注入 RC 文件的 DCS 转义序列方案值得参考，但需权衡注入安全性和用户可接受度。
3. **VS Code Pseudoterminal 接口**：`onDidWrite` / `handleInput` / `open` / `close` ——与 slTerminal PTY 层概念高度对应，可作为前端 hook API 设计的参考模型。
4. **stdin/stdout JSON 协议**：slTerminal 的 hook 脚本协议应遵循行业标准（stdin JSON 上下文 + stdout JSON 决策 + exit 0/2 语义），确保与现有 hook 脚本生态系统互操作。

---

## 七、建议的 slTerminal Hook 优先级路线图

基于以上研究，按投入产出比排序：

| 优先级 | 功能 | 参考来源 | 理由 |
|--------|------|---------|------|
| P0 | stdin/stdout JSON 协议 + exit 0/2 语义 | 行业通行模式（Cursor/Windsurf/Gemini CLI/Claude Code 一致） | 零成本跟随主流，确保脚本可复用 |
| P0 | 命令执行前后 hook（基于 OSC 133 C/D → Precmd/Preexec/CommandFinished） | Cursor/Windsurf/Claude Code 共同模式 | 最高频社区用例：安全拦截 + 自动格式化。slTerminal 是终端级 hook，应使用 PTY/Shell 生命周期术语（非应用层 PreToolUse/PostToolUse） |
| P1 | 可视化 hook 配置面板（iTerm2 Triggers 表格风格） | iTerm2 Triggers GUI | 降低使用门槛 |
| P1 | Notification hook（Desktop/Slack/ntfy） | 6+ 社区通知工具 | 第二大热门用例 |
| P2 | Problem Matchers（regex → 结构化输出） | VS Code problemMatchers | 代码检查类 hook 的杀手功能 |
| P2 | Hook 输出面板（VS Code presentation 配置风格） | VS Code tasks.json | 控制输出可见性/聚焦/分组 |
| P3 | Plugin/hook 包分发（目录结构标准化） | Claude Code Plugin 系统 | 社区生态增长后自然需求 |
| P3 | 状态栏 hook 状态指示器 | WezTerm update-status | 辅助性功能 |

---

## 八、来源汇总

### 官方文档（第一手资料）

| 项目 | 文档 URL | 日期 |
|------|---------|------|
| Claude Code Hooks | https://code.claude.com/docs/en/hooks | 2026 |
| Claude Code Plugins | https://code.claude.com/docs/en/plugins-reference | 2026 |
| Claude Code Settings | https://code.claude.com/docs/en/settings | 2026 |
| Claude Code Hook Patterns | https://github.com/anthropics/claude-plugins-official/.../patterns.md | 2026 |
| Cursor Agent Hooks | https://cursor.com/docs/agent/hooks | 2025 |
| Windsurf Hooks | https://docs.windsurf.com/windsurf/cascade/hooks | 2025-2026 |
| Copilot CLI Extensions | GitHub CLI 官方文档 | 2025-2026 |
| Codex CLI | OpenAI 官方文档 | 2025-2026 |
| Gemini CLI | Google 官方文档 | 2025-2026 |
| aider | https://aider.chat/docs/config.html | 2025-2026 |
| Windows Terminal | https://learn.microsoft.com/en-us/windows/terminal/ | 2026 |
| iTerm2 Triggers | https://iterm2.com/documentation-triggers.html | 2026 |
| Warp DCS Hooks | https://docs.warp.dev/features/sessions/dcs-hooks | 2026 |
| WezTerm Events | https://wezterm.org/config/lua/window-events/index.html | 2026 |
| VS Code Tasks | https://code.visualstudio.com/docs/editor/tasks | 2026 |
| JetBrains Run Configurations | https://plugins.jetbrains.com/docs/intellij/run-configurations.html | 2026 |

> **注意**：Google 于 2026年6月18日弃用了 Gemini CLI 的免费/个人层级，推荐迁移到 Antigravity CLI (`agy`)。开源 `gemini` 二进制和付费 API key 不受影响。评估工具未来 hook 系统发展方向时需考虑此变化。

### 社区资源（第二手资料）

完整列表见各子报告。代表性来源：

- GitHub: disler/claude-code-hooks-mastery、karanb192/claude-code-hooks、joaoariedi/ai-assisted-development-framework 等 15+ 仓库
- Reddit: r/ClaudeAI 社区讨论（2026.05 共识——hooks 是唯一可靠的执法层）
- dev.to: cytostack token 优化教程、idapixl 多 agent 持久化记忆、rulestack 入门教程
- Hacker News: Recall、Pickle Rick、Draft、MCR、TDD Guard、Han 等 9 个展示项目
- 中文社区: 腾讯云、CSDN、w3cschool、GUVI

### 通知工具生态

ai-agent-notifier、claude-notify、agent-notify、claude-notifier、claude-notifications-go、ccnotify（共 6 个独立工具）

---

*汇总完成。各子方向的详细信息参见 D5a-D5d 子报告。*
