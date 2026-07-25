# D5a：同类 AI 编程工具的 Hook/Plugin 系统

> 研究日期：2025-07-25
> 研究方法：跨 7 个竞品的并行 WebSearch + WebFetch + Context7 文档检索

## 1. Cursor

### Hook 机制

Cursor v1.7 (2025) 引入正式的 Agent Hook 系统。通过 `hooks.json` 配置（项目级 `.cursor/hooks.json` 或全局 `~/.cursor/hooks.json`），在 Agent 生命周期关键点触发外部脚本。

**协议**：stdin JSON 输入、stdout JSON 输出。exit 0 = 成功/放行，exit 2 = 拒绝/阻止。

**完整事件列表（21 个，Cursor 3.11+ 含新增事件）**：

| 事件 | 用途 | 交互模式 |
|------|------|----------|
| `beforeShellExecution` | shell 命令执行前 | 交互 -- 可返回 `{continue, permission, userMessage, agentMessage}`。`permission` 三态（allow/deny/ask）为 beforeShellExecution/beforeMCPExecution 专属 |
| `afterShellExecution` | shell 命令执行后 | 通知 |
| `preToolUse` / `postToolUse` / `postToolUseFailure` | 工具调用生命周期（支持 matcher 过滤如 `"Shell\|Write\|Edit"`） | 交互/通知 |
| `beforeReadFile` / `afterFileEdit` / `beforeTabFileRead` / `afterTabFileEdit` | 文件读写操作 | 交互/通知 |
| `beforeMCPExecution` / `afterMCPExecution` | MCP 工具调用 | 交互/通知 |
| `sessionStart` / `sessionEnd` | 会话边界 | 通知 |
| `beforeSubmitPrompt` | prompt 提交前 | 交互 |
| `subagentStart` / `subagentStop` | 子 Agent 生命周期 | 通知 |
| `preCompact` | 上下文压缩前 | 通知 |
| `stop` | Agent 尝试停止时 | 通知 |
| `afterAgentResponse` / `afterAgentThought` | Agent 输出生命周期 | 通知 |
| `workspaceOpen` | 工作区打开时 | 通知 |

`permission` 字段支持三态：`"allow"` / `"deny"` / `"ask"`。`continue: false` 终止整个任务。

TypeScript 类型定义通过 npm 包 `cursor-hooks` 提供。

### 终端 UI 集成方式

Cursor 基于 VS Code fork，终端是内建面板。Agent 模式下：
- 命令在隔离沙箱中执行（默认禁网络，文件系统限工作区和 `/tmp/`）
- 支持本地、云端 VM、远程 SSH 三种执行环境
- 输出回流到 Agent 循环，用于错误修正
- Chat 面板三种子模式：Ask（只读）/ Edit（写文件）/ Agent（自主多步）
- **没有通用插件 API 让第三方注册终端面板**

### 配置/自定义方式

Cursor 有两套**独立的配置维度**——Rules 系统和 Hooks 系统不是同一层级的子项：

**Rules 系统（4 层）**：

1. **Team Rules**（企业版）-- 管理中心下发
2. **Project Rules**（`.cursor/rules/*.mdc`）-- YAML frontmatter + Markdown，四种激活模式（Always Apply / Apply Intelligently / Apply to Specific Files [globs] / Apply Manually）
3. **User Rules**（Cursor Settings > Rules）
4. **AGENTS.md**（项目根目录）-- 跨工具可移植

**Hooks 系统（4 层配置优先级）**：

| 层级 | 路径 | 说明 |
|------|------|------|
| Enterprise | `/etc/cursor/hooks.json` | 企业级强制 |
| Team | 云端下发 | 团队共享 |
| Project | `.cursor/hooks.json` | 项目级，入 git |
| User | `~/.cursor/hooks.json` | 用户全局 |

其他配置维度：MCP（`~/.cursor/mcp.json` + `.cursor/mcp.json`）、Custom Agent Modes、Subagents（`.cursor/agents/`）、Skills（`.cursor/skills/<name>/SKILL.md`）。

**旧 `.cursorrules` 单文件已废弃**，迁移到 `.cursor/rules/` 目录结构。

### 关键来源

- https://cursor.com/docs/hooks -- 官方 Hooks 文档（事件列表、JSON 协议、exit codes）
- https://cursor.com/docs/rules -- 官方 Rules 文档（.mdc 格式、四种激活模式）
- https://blog.gitbutler.com/cursor-hooks-deep-dive -- Cursor Hooks 深度解析
- https://www.npmjs.com/package/cursor-hooks -- npm 类型定义包
- https://github.com/PatrickJS/awesome-cursorrules -- 社区配置集合

## 2. Windsurf (Codeium / Devin Desktop)

> 注：Windsurf 于 2025 年 12 月被 Cognition AI（Devin 开发商）收购，2026 年 6 月已更名为 **Devin Desktop**。收购金额未经公开来源独立核实（"约 2.5 亿美元"在公开报道中未出现）。以下信息基于当前公开文档。

### Hook 机制

Windsurf 有正式的 **Cascade Hooks** 系统（自 v1.12.41），是目前 AI 编码 IDE 中事件覆盖最全的 hook 架构。

**11 个 Hook 事件**（`post_cascade_response_with_transcript` 是 `post_cascade_response` 的子事件/附带完整 JSONL 转录的变体，`post_setup_worktree` 不在主要事件表中）：

| 事件 | 触发时机 | 可否阻断 |
|------|---------|---------|
| `pre_user_prompt` | 用户提示词被处理前 | 可以（exit 2） |
| `pre_read_code` | 读取代码文件前 | 可以（exit 2） |
| `post_read_code` | 成功读取代码文件后 | 否 |
| `pre_write_code` | 写入/修改代码文件前 | 可以（exit 2） |
| `post_write_code` | 写入/修改代码文件后 | 否 |
| `pre_run_command` | 终端命令执行前 | 可以（exit 2） |
| `post_run_command` | 终端命令执行后 | 否 |
| `pre_mcp_tool_use` | 调用 MCP 工具前 | 可以（exit 2） |
| `post_mcp_tool_use` | MCP 工具调用后 | 否 |
| `post_cascade_response` | Cascade 完成响应后 | 否 |
| `post_cascade_response_with_transcript` | 同上，附带完整 JSONL 转录 | 否 |
| `post_setup_worktree` | 新 git worktree 创建后 | 否 |

**配置三级合并**（执行顺序：system -> user -> workspace）：

| 级别 | 路径 |
|------|------|
| System | `/etc/windsurf/hooks.json` |
| User | `~/.codeium/windsurf/hooks.json` |
| Workspace | `.windsurf/hooks.json` |

同一事件在多个位置的配置**全部执行**（非覆盖）。

**stdin JSON 公共字段**：`agent_action_name`、`trajectory_id`、`execution_id`、`timestamp`、`model_name`、`tool_info`。

### 终端 UI 集成方式

Windsurf 的终端是 IDE 内嵌面板。Cascade AI 代理直接驱动终端命令执行，走 `pre_run_command` / `post_run_command` hook。

**终端命令自动执行四级控制**（Cascade 自身配置，非 hook 层）：
1. Disabled -- 所有命令需手动批准
2. Allowlist Only -- 仅白名单命令自动执行
3. Auto -- Cascade 判断安全性
4. Turbo -- 全部自动执行（除 deny-list）

三种终端入口：`Cmd/Ctrl+I`（自然语言→CLI）、`Cmd/Ctrl+L`（选中输出→分析）、`@` 提及（引用活跃终端会话）。

### 配置/自定义方式

Rules 系统旧格式为 `.windsurfrules`（纯 markdown，无 frontmatter）。Wave 8 新格式为 `.windsurf/rules/` 目录，每个规则独立 `.md` 文件 + YAML frontmatter，四种 trigger 模式：`always_on`、`model_decision`、`glob`、`manual`。

其他：Global Rules（`~/.codeium/windsurf/memories/global_rules.md`）、AGENTS.md、MCP 配置（`~/.codeium/windsurf/mcp_config.json`）、Memories（跨会话持久化上下文记忆，最多 100 个 transcripts）。

### 关键来源

- https://docs.windsurf.com/zh/windsurf/cascade/hooks -- 官方 Cascade Hooks 文档
- https://dev.to/digitalapplied/windsurf-swe-15-cascade-hooks-complete-developer-guide-20fh -- 企业合规 use case 分析
- https://design.dev/guides/windsurf-rules/ -- Rules 配置指南
- https://mer.vin/2025/12/windsurf-memory-rules-deep-dive/ -- Memory 深度解析
- https://www.tembo.io/blog/cursor-vs-windsurf -- Cursor vs Windsurf 对比

## 3. GitHub Copilot CLI

### Hook 机制

Copilot CLI 有两套互补的 hook 机制（**注意**：两套的事件集高度重叠——JSON 配置文件的 13 个事件与 SDK 的 6 个事件名对应同一生命周期点，如 `sessionStart` ↔ `onSessionStart`。它们是同一事件集在不同配置层的表达，不应相加为 19 个不同事件）：

**A. JSON 配置文件 Hook（13 个事件）**

通过 `.github/hooks/*.json` 或 `~/.copilot/hooks/*.json` 配置。三种执行类型：Command（shell 脚本）、HTTP（JSON POST）、Prompt（仅 CLI，sessionStart 时自动提交）。

事件列表：`sessionStart`、`sessionEnd`、`userPromptSubmitted`、`preToolUse`（可 allow/deny/modify）、`postToolUse`（可修改结果）、`postToolUseFailure`、`permissionRequest`、`preCompact`、`agentStop`（可阻止并强制继续）、`subagentStart`、`subagentStop`（可阻止并强制继续）、`errorOccurred`、`notification`。

**B. SDK 扩展 Hook（仅 CLI，与 A 的事件集高度重叠）**

通过 `@github/copilot-sdk` 的 `joinSession()` API（JSON-RPC over stdio）注册 6 个核心 hook：
- `onSessionStart` -- 注入基线上下文（对应 JSON `sessionStart`）
- `onUserPromptSubmitted` -- 改写/增强用户提示词（对应 JSON `userPromptSubmitted`）
- `onPreToolUse` -- 阻止危险命令（`deny`/`allow`）、修改工具参数（对应 JSON `preToolUse`）
- `onPostToolUse` -- 运行 linter/校验、修改结果（对应 JSON `postToolUse`）
- `onErrorOccurred` -- 自动恢复
- `onSessionEnd` -- 生成摘要/日志

### 扩展系统

CLI 扩展运行在**独立 Node.js 子进程**中，通过 **JSON-RPC over stdio** 与 CLI 主进程通信。

**发现机制**：`.github/extensions/NAME/extension.mjs`（项目级）或 `~/.copilot/extensions/NAME/extension.mjs`（用户级）。

**Plugin** 是更上层的打包单位，捆绑 agents、skills、hooks、MCP/LSP 配置，通过 `plugin.json` manifest 分发。

### 终端 UI 集成方式

Copilot CLI 使用 **Ink**（React 的终端渲染器）构建 TUI：
- React 虚拟组件树 -> Yoga WASM Flexbox 布局引擎 -> ANSI 转义序列输出
- 三种交互模式（`Shift+Tab` 切换）：Standard（每步审批）、Plan（先分析再动手）、Autopilot（自主执行）
- 支持 Headless 模式：`--prompt` + `--no-interactive` + `--output-format json`

### 配置/自定义方式

目录结构：`.github/copilot-instructions.md`（仓库级指令）、`.github/instructions/*.instructions.md`（路径级指令，通过 `applyTo` glob 匹配）、`.github/agents/*.agent.md`（自定义 Agent）、`.github/skills/<name>/SKILL.md`（Skill 渐进式加载：discovery ~100 tokens -> activation <5000 tokens -> resources）、`~/.copilot/settings.json`。

Agent Skill 三段式加载节省 token：Discovery（~100t）-> Activation（<5000t）-> Resources（脚本/模板）。

### 关键来源

- https://docs.github.com/en/copilot/reference/hooks-reference -- 13 个 hook 事件定义
- https://docs.github.com/en/copilot/concepts/agents/copilot-cli/about-cli-extensions -- CLI 扩展架构
- https://github.com/github/copilot-sdk/blob/main/nodejs/docs/extensions.md -- SDK API 文档
- https://www.devleader.ca/2026/07/09/github-copilot-cli-the-complete-guide-to-the-agentic-terminal-agent -- 架构全貌

## 4. Codex CLI (OpenAI)

### Hook 机制

Codex CLI 拥有完整的 hook 扩展系统。通信协议为 **JSON-RPC over stdio（JSONL 格式）**——与 Claude Code 的原始 stdin→stdout 单次调用模型有显著区别：Codex 的 App Server 使用**双向** JSON-RPC（服务器可主动发起请求），而 Claude Code hooks 是单向 stdin→stdout 模型。两者协议层不兼容。

**10 个生命周期事件**：`SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PostToolUse`、`PermissionRequest`、`PreCompact`、`PostCompact`、`SubagentStart`、`SubagentStop`、`Stop`。

**加载机制**：从多位置合并 hooks（非覆盖）-- `~/.codex/hooks.json`、`~/.codex/config.toml` 内联 `[hooks]` 段、`<repo>/.codex/hooks.json`、`<repo>/.codex/config.toml`、插件内置 `hooks/hooks.json`。

**运行时行为**：
- 同一事件匹配的多个 hook 并发执行
- 非托管 hook 需用户审查信任（基于内容 hash 记录信任状态）
- Hook 失败不阻塞代理循环（静默降级，返回 `{}`）

### Plugin 架构

Plugin = Skills + MCP Server + Hooks 三层组合。插件清单 `.codex-plugin/plugin.json`。

### 终端 UI 集成方式

Codex CLI 基于 **Codex App Server 协议**（双向 JSON-RPC over stdio，stream 为 JSONL），同一协议驱动所有交互界面（CLI、VS Code、Web App、macOS 桌面、JetBrains、Xcode）。

三个核心原语：**Item**（原子输入/输出单元，started->streaming->completed）、**Turn**（单次代理工作产生的 Item 序列）、**Thread**（持久化会话容器）。

程序化 CLI 模式：`codex exec --json` 输出 newline-delimited JSON 事件流。

### 配置/自定义方式

支持 YAML/JSON/TOML 三种格式。优先级：CLI 参数 > 环境变量（`CODEX_*`） > 配置文件（`~/.codex/config.*`） > 硬编码默认值。

指令文件多级体系：`~/.codex/instructions.md`（全局） -> `codex.md` / `AGENTS.md`（项目级）。Profiles 通过 `~/.codex/<name>.config.toml` 命名配置切换。

Skills 三级优先级：`.codex/skills/`（项目本地） > `~/.codex/skills/` > `/usr/local/share/codex/skills/`。

### 关键来源

- https://developers.openai.com/codex/hooks -- 官方 Hook 文档
- https://github.com/mturac/everything-openai-codex/blob/main/docs/architecture/cross-harness.md -- 跨 harness 架构
- https://www.infoq.com/news/2026/02/opanai-codex-app-server/ -- App Server 架构概述
- https://github.com/RoggeOhta/awesome-codex-cli -- 社区资源汇总

## 5. Gemini CLI (Google)

### Hook 机制

Gemini CLI 有一级 hooks 系统（first-class hook system），v1 版本于 2025-2026 年逐步落地。设计上追求与 Claude Code hooks 的契约兼容（相同的 JSON-over-stdin 协议、退出码语义、matcher 语法）。

> **重要**：Google 于 2026年6月18日弃用了 Gemini CLI 的免费/个人层级，推荐迁移到新的 **Antigravity CLI** (`agy`)。付费 API key 和开源 `gemini` 二进制不受影响。此变化影响工具未来 hook 系统的发展方向评估。

**两种 Hook 类型**：Command Hooks（执行任意 shell 脚本，当前主流方案）+ Plugin Hooks（加载标注为 `geminicli-plugin` 的 npm 包，设计阶段）。

**11 个生命周期事件**：

| 事件 | 特殊性 |
|------|--------|
| `SessionStart` / `SessionEnd` | 会话初始化/清理 |
| `BeforeAgent` / `AfterAgent` | AfterAgent 可**阻止会话退出**并注入下一轮迭代 prompt |
| `BeforeModel` / `AfterModel` | 可**修改/替换 LLM 请求或响应**（可 mock 模型返回值） |
| `BeforeToolSelection` | 调整候选工具列表（按代理模式沙箱化工具） |
| `BeforeTool` / `AfterTool` | 工具执行前后校验/拦截 |
| `PreCompress` | 上下文压缩前保存状态 |
| `Notification` | 自动审批、日志决策 |

**管理命令**：
```bash
gemini hooks install <package>
gemini hooks migrate --from-claude    # 一键转换 Claude Code hooks
/hooks panel   # 对话内查看活跃 hooks
/hooks reload  # 重载配置
```

### 扩展系统

扩展目录结构：`gemini-extension.json`（清单）+ `GEMINI.md`（上下文）+ `agents/`（子代理）+ `skills/`（SKILL.md）+ `hooks/hooks.json` + `scripts/`。

扩展设置通过两条路径注入：Hooks 作为环境变量注入子进程；Skills/Agents 通过文本变量替换（`${VAR_NAME}` 在 SKILL.md/agent system prompt 中替换）。

### 终端 UI 集成方式

与 Claude Code 采用相同基础架构：**Ink**（React-in-terminal）+ 全屏 TUI + alt screen buffer + Chat 式交互界面。

### 配置/自定义方式

配置文件层级（JSON 格式）：`.gemini/settings.json`（项目级，最高） > `~/.gemini/settings.json`（用户级） > 系统路径 > 扩展 `gemini-extension.json`。

Hook 环境变量：`GEMINI_PROJECT_DIR`、`GEMINI_SESSION_ID`、`GEMINI_API_KEY`。

### 关键来源

- https://github.com/google-gemini/gemini-cli/issues/9070 -- v1 hooks 系统设计综述
- https://github.com/google-gemini/gemini-cli/pull/16073 -- hooks + extensions 官方文档 PR
- https://geminicli.com/docs/hooks/ -- 官方 hooks 文档
- https://developers.googleblog.com/en/tailor-gemini-cli-to-your-workflow-with-hooks/ -- 官方博客

## 6. aider

### Hook 机制

aider **没有正式的 hook 系统**，通过两个正交机制提供类似的自动化反馈闭环：

| 机制 | 触发点 | 说明 |
|------|--------|------|
| `--lint-cmd` | AI 编辑文件后自动执行 | 按语言指定 lint 命令，非零退出码 -> 输出反馈给 LLM 自动修复 |
| `--test-cmd` | AI 编辑文件后自动执行（需 `--auto-test` 开启） | 运行测试套件，失败 -> 输出反馈给 LLM 修复 |

此外：`--git-commit-verify`（控制是否绕过 git pre-commit hooks）、`--notifications-command`（完成后执行自定义通知命令）。

### 命令系统（斜杠命令）

内置约 43 个斜杠命令（v0.86.x），包括 `/add`、`/drop`、`/ls`、`/code`、`/ask`、`/architect`、`/commit`、`/undo`、`/diff`、`/run`、`/test`、`/clear`、`/reset`、`/map`、`/map-refresh`、`/tokens`、`/voice`、`/web`、`/settings`、`/model`、`/weak-model`、`/chat-mode`、`/read-only`、`/editor-model`、`/think-tokens`、`/reasoning-effort`、`/cost`、`/save`、`/load`、`/paste`、`/copy`、`/lint`、`/help` 等。

`/run` 和 `/test` 可在聊天中执行任意 shell 命令并将输出注入对话上下文。

**没有自定义斜杠命令的扩展机制**——命令列表是硬编码的，不像 Claude Code / Codex CLI / Gemini CLI 那样支持用户定义 Markdown 格式的斜杠命令。不过第三方桌面包装器 **AiderDesk** 提供了自定义斜杠命令（`.aider-desk/commands/`）和 30+ 生命周期 hook 事件。

### 终端 UI 集成方式

aider 是纯 CLI 工具，六种交互路径：

1. 终端 TUI（默认）-- 基于 Python `prompt_toolkit`
2. `--watch-files` 模式 -- aider 后台运行，通过文件中的 `# AI! <指令>` 注释触发
3. `--browser` 模式 -- 本地 HTTP 服务器的 Web UI
4. 脚本模式 -- `--message`/`--yes`/`--commit` 无交互自动化
5. Copy/Paste 模式 -- 剪贴板中转
6. 第三方编辑器插件 -- NeoVim、VS Code 社区插件

### 关键来源

- https://aider.chat/docs/usage/lint-test.html -- lint/test 自动反馈机制
- https://aider.chat/docs/config/options.html -- 所有配置选项
- https://aider.chat/docs/scripting.html -- 脚本化/自动化模式
- https://aider.chat/docs/usage/commands.html -- 内嵌 slash 命令完整列表
- https://aider.chat/docs/usage/watch.html -- watch-files 模式

## 7. Claude Code (Anthropic)

> slTerminal 的核心目标平台。数据来源：Context7 `/anthropics/claude-code` 库检索 + 官方文档 `code.claude.com/docs` 深度抓取。

### Hook 机制

Claude Code 有成熟的 **14+ 事件 hook 系统**（12 核心 + 2+ 实验性），通过 `.claude/settings.json`（`hooks` 顶层字段）或插件的 `hooks/hooks.json` 配置。

**协议**：stdin JSON 输入、stdout JSON 输出。关键退 出码语义：

| 退出码 | 含义 | 行为 |
|--------|------|------|
| **0** | 成功 | 正常执行；stdout JSON 做精细控制（`decision:"block"`、`additionalContext` 等） |
| **2** | 阻断错误 | 阻止动作（PreToolUse/UserPromptSubmit）；stderr 反馈给 Claude |
| **其他** | 非阻断错误 | stderr 第一行记入 transcript，执行继续 |

exit 2 时忽略 stdout。推荐用 exit 0 + `decision:"block"` 结构化方式。

**完整生命周期事件**：

| 事件 | 触发时机 | 可阻断 | 说明 |
|------|---------|--------|------|
| `SessionStart` | 会话启动 | 否 | 接收 `source`(startup/resume/clear/compact)、`model`、`agent_type` |
| `Setup` | 一次性准备阶段 | 否 | 仅支持 command/mcp_tool 类型 |
| `UserPromptSubmit` | 用户提交提示词后、Claude 处理前 | **是** | 可注入 `additionalContext`；超时 30s |
| `PreToolUse` | 工具调用前 | **是** | 校验/阻止工具调用（matcher 正则过滤，如 `"Write\|Edit"`） |
| `PostToolUse` | 工具成功执行后 | 可标记 block | 修改工具输出(`updatedToolOutput`)、触发格式化 |
| `PostToolUseFailure` | 工具调用失败后 | 否 | 记录/上报失败 |
| `Stop` | Claude 完成响应后 | **可阻止停止** | `decision:"block"` 让 Claude 继续工作 |
| `SubagentStop` | 子代理停止时 | — | `agent_id`、`agent_transcript_path` |
| `PreCompact` | 上下文压缩前 | **是** | `trigger`(manual/auto)；exit 2 或 `{"decision":"block"}` 阻止 |
| `Notification` | 需要用户注意时 | 否 | `message` + `notification_type` |
| `PermissionDenied` | 权限被拒绝时 | — | 仅通知 |
| `FileChanged` | 外部文件变更 | — | 仅通知 |

另有 `ElicitationResult`（结果收集）、`WorktreeCreate`（退出码非 0 中止创建）等实验性事件。

**5 种 hook 类型**：
- `type: "command"` -- 执行 bash 脚本，支持 `timeout`（默认 600s）
- `type: "prompt"` -- 注入额外 prompt 文本到 LLM 上下文（超时默认 30s）
- `type: "http"` -- HTTP POST 发送 JSON payload 到外部服务
- `type: "mcp_tool"` -- 调用 MCP 工具作为 hook 回调
- `type: "agent"` -- 多轮 agent 验证（实验性）

**stdin JSON 通用字段**（所有事件接收）：
```json
{
  "session_id": "abc123",
  "prompt_id": "...",
  "transcript_path": "/path/to/.claude/projects/.../xxx.jsonl",
  "cwd": "/Users/sarah/myproject",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": { "command": "npm test" },
  "tool_use_id": "toolu_01ABC123..."
}
```

**stdout JSON 输出格式**（exit 0 时）：
```json
{
  "decision": "block",
  "reason": "不允许执行 rm 命令",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "格式化完成，自动修正 3 处缩进",
    "updatedToolOutput": { "...": "..." }
  }
}
```

关键输出字段：`decision`("block")、`reason`、`additionalContext`（上限 10K 字符）、`systemMessage`、`updatedToolOutput`、`updatedMCPToolOutput`、`sessionTitle`。

**运行时变量**：`${CLAUDE_PLUGIN_ROOT}`、`${tool_input.xxx}`（如 `${tool_input.file_path}`）、`$ARGUMENTS`（prompt 类型的完整 JSON）。

**Hook 加载时机**：hooks 在会话启动时加载。编辑配置不影响当前会话，需重启 Claude Code。

**配置示例**（来自官方文档）：
```json
{
  "PreToolUse": [
    {
      "matcher": "Write|Edit",
      "hooks": [
        {
          "type": "command",
          "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/scripts/security/scan-secrets.sh",
          "timeout": 30
        }
      ]
    },
    {
      "matcher": "Bash",
      "hooks": [
        {
          "type": "prompt",
          "prompt": "Evaluate if this bash command is safe for production environment.",
          "timeout": 20
        }
      ]
    }
  ],
  "PostToolUse": [
    {
      "matcher": "Bash",
      "hooks": [
        {
          "type": "command",
          "command": "bash ${CLAUDE_PLUGIN_ROOT}/hooks/scripts/workflow/update-status.sh",
          "timeout": 15
        }
      ]
    }
  ],
  "Stop": [
    {
      "matcher": "",
      "hooks": [
        {
          "type": "prompt",
          "prompt": "Verify tests run and build succeeded"
        }
      ]
    }
  ],
  "SessionStart": [
    {
      "matcher": "",
      "hooks": [
        {
          "type": "command",
          "command": "bash ${CLAUDE_PLUGIN_ROOT}/scripts/load-context.sh"
        }
      ]
    }
  ]
}
```

**Hook 加载时机**：hooks 在会话启动时加载。编辑 `hooks.json` 不影响当前会话，需重启 Claude Code。

### Plugin 系统

Plugin = **Skills + Agents + Hooks + MCP servers + Themes + Commands** 组合包。

**Plugin 目录结构**：
```
my-plugin/
├── .claude-plugin/
│   └── plugin.json              # 清单文件
├── skills/                      # 技能（推荐，替代旧 commands/）
│   └── code-reviewer/
│       └── SKILL.md
├── commands/                    # 传统 slash 命令（向后兼容）
│   └── status.md
├── agents/                      # 子 agent 定义
│   ├── security-reviewer.md
│   └── performance-tester.md
├── hooks/
│   ├── hooks.json               # hook 配置
│   └── run-hook.cmd             # hook 执行脚本
├── themes/                      # 颜色主题
├── output-styles/               # 输出风格
├── bin/                         # 可执行文件（加入 PATH）
├── settings.json                # plugin 默认设置
├── .mcp.json                    # MCP server 定义
└── .lsp.json                    # LSP server 配置
```

`${CLAUDE_PLUGIN_ROOT}` 占位符在 plugin 安装后指向插件根目录。

**plugin.json 格式**：
```json
{
  "name": "superpowers",
  "version": "5.0.7",
  "description": "Core skills library",
  "author": { "name": "...", "email": "..." },
  "repository": "https://github.com/...",
  "license": "MIT",
  "dependencies": ["other-plugin@marketplace"]
}
```

**Plugin 管理**：`claude plugin install <id>`、`/plugin` 命令浏览/安装/启用/禁用、`enabledPlugins: {"name@marketplace": true/false}` 在 settings.json 中控制。

**Slash Commands**：Markdown 文件 + YAML frontmatter：
```markdown
---
allowed-tools: Read, Grep, Glob
description: 运行安全漏洞扫描
model: claude-opus-4-8
argument-hint: [target-dir]
---
分析代码库的安全漏洞，包括 SQL 注入、XSS 漏洞...
```

命令位置：项目级 `.claude/commands/`、用户级 `~/.claude/commands/`。

**Agents 定义**（`agents/` 目录下 `.md` 文件）：
```markdown
---
name: code-reviewer
description: Use when a major project step has been completed
model: inherit
---
You are a Senior Code Reviewer...
```

**MCP 配置**：项目 `.mcp.json`、用户 `~/.claude.json`、Plugin 内 `.mcp.json`：
```json
{
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "${CLAUDE_PROJECT_DIR}"]
    },
    "remote-api": {
      "type": "http",
      "url": "https://api.example.com/mcp"
    }
  }
}
```

### 终端 UI 集成方式

Claude Code 是基于 **Ink**（React-in-terminal）的全屏 TUI CLI 工具：
- Ink 虚拟组件树 -> Yoga Flexbox 布局 -> ANSI 转义序列
- alt screen buffer 全屏模式
- Chat 式交互界面，支持流式输出
- 多接口统一：终端、桌面 App、VS Code、JetBrains、claude.ai/code、Slack、CI/CD -- 内核 agentic loop 完全相同

**slTerminal 的角色**：slTerminal 是专为 Claude Code CLI 设计的终端模拟器。Claude Code 运行在 slTerminal 的 xterm.js + ConPTY 终端中。slTerminal 不嵌入 Claude Code 的 hook 系统，而是作为独立的终端宿主提供**终端级 hook 能力**（如 pre-command safety check、post-command audit log）-- 这些 hook 在终端层面触发，独立于 Claude Code 的应用层 hook。

### 配置/自定义方式

**四层优先级**（从高到低）：

| 层级 | 路径 | 场景 |
|------|------|------|
| **Managed** | 企业网关推送 | 策略强制执行，最高优先级 |
| **CLI 参数** | 命令行 | 运行时覆盖 |
| **Local** | `<cwd>/.claude/settings.local.json` | 个人本地覆盖，不入 git |
| **Project** | `<cwd>/.claude/settings.json` | 团队共享，入 git |
| **User** | `~/.claude/settings.json` | 全局默认 |

**合并规则**：数组跨层累加（如 `permissions.allow`），标量值高层覆盖低层。

**CLAUDE.md 多级指令体系**：

| 层级 | 路径 | 加载时机 |
|------|------|---------|
| User 全局 | `~/.claude/CLAUDE.md` | 所有会话始终加载 |
| User 规则 | `~/.claude/rules/*.md` | 按路径 glob 匹配 |
| Project | `<cwd>/.claude/CLAUDE.md` + 父目录祖先链 | 项目上下文中加载 |
| Project 规则 | `<cwd>/.claude/rules/*.md` | 按路径 glob 匹配 |
| 子目录 | `<cwd>/<subdir>/CLAUDE.md` | agent 访问该目录时按需加载 |
| Local | `<cwd>/CLAUDE.local.md` | 个人本地指令，不入 git |

**Permissions 字符串格式**：`ToolName(pattern)`，支持 `*`/`**` glob。MCP 权限为 `mcp__<server>`（全部工具）或 `mcp__<server>__<tool>`（单个工具）。`defaultMode` 支持 `"auto"`(白名单自动)、`"acceptEdits"`、`"plan"`(只读)、`"bypassPermissions"`(全放行)。

**`~/.claude/` 目录结构**：
```
~/.claude/
├── settings.json        # 用户全局设置
├── CLAUDE.md            # 用户全局指令
├── CLAUDE.local.md      # 用户本地覆盖
├── rules/*.md           # 用户级规则
├── plugins/cache/       # 已安装插件缓存
├── keybindings.json     # 自定义快捷键
└── projects/<hash>/     # 按项目存储 transcript
```

### 关键来源

- https://code.claude.com/docs/en/hooks -- 官方 Hooks 文档（事件列表、配置 schema、stdin/stdout JSON 协议、退出码）
- https://code.claude.com/docs/en/hooks-guide -- 多 hook 配置、结构化 JSON 输出
- https://code.claude.com/docs/en/plugins-reference -- Plugin 目录结构、plugin.json 格式
- https://code.claude.com/docs/en/settings -- 四层 settings 层次、permissions、enabledPlugins
- https://code.claude.com/docs/en/claude-directory -- `~/.claude/` 完整结构
- https://code.claude.com/docs/en/agent-sdk/slash-commands -- Slash command Markdown + YAML frontmatter
- https://code.claude.com/docs/en/mcp-quickstart -- `.mcp.json` 配置格式
- https://code.claude.com/docs/en/plugin-marketplaces -- Plugin 高级入口配置
- https://code.claude.com/docs/en/how-claude-code-works -- 多接口统一 agentic loop
- https://code.claude.com/docs/en/permissions -- Permissions 层次优先级

## 对比总结表格

| 项目 | Hook 事件数 | Hook 协议 | 配置方式 | Slash 命令 | Plugin 系统 | 终端集成 | 阻断机制 |
|------|-----------|----------|---------|-----------|------------|---------|---------|
| **Cursor** | 21（3.11+） | stdin JSON / stdout JSON, exit 2 阻断 | `.cursor/hooks.json`（Hooks 4 层）+ `.cursor/rules/`（Rules 4 层，独立维度） | 无（Chat 内 `@规则名`） | Rules+Hooks+MCP+Skills | IDE 内嵌终端面板 | `permission: deny` + `continue: false` |
| **Windsurf** | 11+1（`post_cascade_response_with_transcript` 为子事件） | stdin JSON / stdout JSON, exit 2 阻断 | `.windsurf/hooks.json` 三级合并 | 无（`@规则名` 激活） | Hooks+Rules+MCP+Memories | IDE 内嵌终端 + Cascade 代理驱动 | pre-hook exit 2 阻断全部 |
| **Copilot CLI** | ~13（JSON+SDK 两套方式，事件高度重叠） | JSON 文件 / JSON-RPC SDK（`joinSession()`） | `.github/hooks/*.json` + `extension.mjs` | `/extensions` 内建 | Plugin（agents+skills+hooks+MCP） | Ink TUI（React + Flexbox） | `deny` + `agentStop` 可强制继续 |
| **Codex CLI** | ~10（未独立验证） | JSON-RPC over stdio（JSONL 格式，双向协议） | `hooks.json` / `config.toml` 多位置合并 | 无用户自定义命令（待定） | Plugin=Skills+MCP+Hooks | App Server JSON-RPC 驱动全界面 | 退出码非 0 可阻断 |
| **Gemini CLI** | 11 | stdin JSON / stdout JSON（2026.6 免费版已弃用，推荐迁移到 Antigravity CLI） | `.gemini/settings.json` 多级 JSON | `/hooks` `/skills` 内建 | Extension=清单+agents+skills+hooks | Ink TUI | 非 0 退出码阻断 + `systemMessage` |
| **aider** | 0（无 hook 系统，AiderDesk 第三方有 30+ 事件） | N/A | `.aider.conf.yml`（YAML） | ~43 个内置命令，不可扩展 | 无 | Python prompt_toolkit TUI | N/A（仅 lint/test 自动反馈） |
| **Claude Code** | 14+（12 核心 + 2+ 实验性） | stdin JSON / stdout JSON（5 种 hook 类型：command/prompt/http/mcp_tool/agent） | `.claude/settings.json` 四层合并（Managed > CLI > Local > Project > User） | Markdown+YAML frontmatter 自定义 slash commands | Plugin=Skills+Agents+Hooks+MCP+Themes | Ink TUI（React-in-terminal） | exit 2 + `decision:"block"` 阻断（PreToolUse/UserPromptSubmit/Stop/PreCompact）；`updatedToolOutput` 修改结果 |

### 行业趋势总结

1. **经 stdio 传输 JSON 是行业通行模式，但协议层不统一**：Cursor、Windsurf、Gemini CLI、Claude Code 使用原始 stdin→stdout JSON（exit 0/2 语义）。Copilot CLI 和 Codex CLI 使用 JSON-RPC over stdio（双向结构化协议），与前者协议层不兼容。`agent-hooks`（GitHub）和 `polyhook` 等第三方工具尝试提供统一接口，但跨工具 hook 脚本复用仍受协议差异限制。

2. **三级配置合并**（system/user/project）是通用模式：Windsurf 和 Codex 的 system/user/workspace 三级配置均有去重逻辑，可直接借鉴。

3. **Hook 事件粒度呈三级分化**：
   - 粗粒度（aider）：仅 post-edit lint/test
   - 中粒度（Cursor/Windsurf）：pre/post tool execution + MCP + shell
   - 细粒度（Gemini CLI）：BeforeModel/AfterModel/BeforeToolSelection 等可修改 LLM 请求/响应的事件

4. **Plugin = Skills + MCP + Hooks 三位一体**是当前主流架构：Copilot CLI、Codex CLI、Gemini CLI、Claude Code 均采用此模型。

5. **终端 UI 趋向 Ink（React TUI）**：Copilot CLI、Gemini CLI、Claude Code 均使用 Ink + Yoga Flexbox + ANSI 渲染——React 组件化思想延伸到终端 UI 已是大势所趋。

6. **AGENTS.md / CLAUDE.md / GEMINI.md / codex.md** 跨工具指令文件已成为 AI 编程工具的"通用配置文件"，可在不同 CLI 之间移植项目指令。
