# D3：Claude Code Hooks 的输出/日志可视化

> 研究方向：hooks 执行历史、stdout/stderr 日志如何存储与展示？
> 检索日期：2025-07-25
> 时间范围：2025-2026 年，越新权重越高

---

## 目录

1. [Claude Code Hooks 输出机制](#1-claude-code-hooks-输出机制)
2. [Claude Code 日志/调试基础设施](#2-claude-code-日志调试基础设施)
3. [终端日志面板设计模式（同类工具参考）](#3-终端日志面板设计模式同类工具参考)
4. [结构化日志与过滤](#4-结构化日志与过滤)
5. [社区讨论与已知问题](#5-社区讨论与已知问题)
6. [对 slTerminal 的启示与设计建议](#6-对-slterminal-的启示与设计建议)

---

## 1. Claude Code Hooks 输出机制

### 1.1 通信模型

Claude Code hooks 通过三个通道与 Claude 通信：

| 通道 | 方向 | 内容 |
|------|------|------|
| **stdin** | Claude → Hook | JSON 事件载荷（session_id、transcript_path、tool_name、tool_input 等） |
| **stdout** | Hook → Claude | 结构化 JSON 决策输出 或 纯文本（注入 context） |
| **stderr** | Hook → Claude/用户 | 错误信息、阻塞原因 |

**来源**：[Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks)、[Claude Code Hooks: 8 Production Patterns (2026)](https://dev.to/claudeguide/claude-code-hooks-8-production-patterns-2026-3e8f)

### 1.2 退出码语义（核心契约）

| 退出码 | 含义 | stdout 处理 | stderr 处理 |
|--------|------|-------------|-------------|
| **0** | 成功/继续 | 解析为 JSON 决策输出；或注入为 Claude 上下文 | 忽略 |
| **2** | 硬阻塞 | 忽略 | 发送给 Claude 和用户作为阻塞原因反馈（双方可见） |
| **1 或其他** | 非阻塞错误 | 忽略 | 显示给用户/记录到日志（verbose 模式下可见） |

**关键误区**：`exit 1` 不是阻塞——操作仍会继续。只有 `exit 2` 才是硬阻塞。这是 hooks 调试中最常见的错误来源。

**来源**：[Claude Code Hook Control Flow - Steve Kinney](https://stevekinney.com/courses/ai-development/claude-code-hook-control-flow)、[Claude Code Hooks (2026): 30 Hook Events](https://www.morphllm.com/claude-code-hooks)

### 1.3 PostToolUse 三种可见性模式（实测）

GitHub Issue [#11224](https://github.com/anthropics/claude-code/issues/11224) 记录了 PostToolUse hook 的三种输出可见性模式：

| 模式 | 触发条件 | 用户看到 | Claude 看到 |
|------|---------|---------|------------|
| **用户独占** | stdout + exit 0 | `PostToolUse:ToolName hook succeeded: message` | 否 |
| **用户独占（带错误标签）** | 任意输出 + exit 1/3+ | `PostToolUse:ToolName hook error: message` | 否 |
| **双方可见** | stderr + exit 2 | `PostToolUse:ToolName hook blocking error: message` | 是 |

**关键陷阱**：exit 2 必须输出到 stderr——stdout 会被忽略，导致消息对用户和 Claude 均不可见。

### 1.4 结构化 JSON 输出格式

exit 0 时，hooks 可通过 stdout 输出 JSON 进行精确控制。通用输出信封：

```json
{
  "continue": true,
  "stopReason": "",
  "suppressOutput": false,
  "systemMessage": "显示给用户的警告",
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "原因",
    "updatedInput": {},
    "additionalContext": "注入给 Claude 的上下文"
  }
}
```

**permissionDecision 取值**：`"allow"`（跳过权限提示）、`"deny"`（取消工具调用）、`"ask"`（显示权限提示）、`"defer"`（延迟到后续逻辑）

**来源**：[Claude Code Docs - Hooks Reference](https://code.claude.com/docs/en/hooks)

### 1.5 各事件的输出行为差异

| Hook 事件 | stdout 处理 | 是否可阻塞 |
|-----------|-------------|-----------|
| **SessionStart** | 注入为 Claude 上下文 | 否 |
| **UserPromptSubmit** | 应注入为上下文（v2.0.69 有 bug——stdout 报错而非注入） | 是 |
| **PreToolUse** | 解析为 permissionDecision JSON | 是 |
| **PostToolUse** | 选择性可见（取决于退出码+流，见 1.3） | 否（`decision:"block"` 仅外观标记，不阻塞） |
| **Stop** | 解析为 decision JSON | 是 |
| **StopFailure** | 输出和退出码均被忽略（API 错误终结回合，无意义） | 否 |
| **PreCompact** | 解析为 decision JSON | 是 |
| **PostToolBatch** | 解析；exit 2 停止循环 | 是 |

**来源**：[Claude Code Docs - 30 Hook Events Full Reference](https://code.claude.com/docs/en/hooks)、[Claude Code Hooks: Complete Guide](https://github.com/ThamJiaHe/claude-code-handbook/blob/main/docs/hooks-guide.md)

### 1.6 大输出溢出到磁盘（50K 阈值）

v2.1.89+ 起，超过 **50,000 字符** 的 hook 输出不再直接注入 Claude 上下文，而是保存到磁盘并以文件路径+预览形式呈现。影响 `additionalContext`、`systemMessage` 和异步 hook 载荷。

> 注意：更新日志宣称 50,000 字符，但实际代码使用 **10,000 字符** 阈值（issue #51537、#50571）。

**来源**：[DOCS: Hooks docs omit >50K output file-path preview behavior - Issue #41799](https://github.com/anthropics/claude-code/issues/41799)

---

## 2. Claude Code 日志/调试基础设施

### 2.1 CLI 调试标志

| 标志 | 作用 |
|------|------|
| `--debug` | 通用 debug 模式。`claude --debug hooks` 专门显示 hook 匹配/执行/退出码 |
| `--debug mcp` | 显示 MCP server stderr 输出 |
| `--output-format stream-json` | 流式 JSON 输出，用于编程式调试 |
| `--bare` | 绕过 hooks、LSP、OAuth 等自定义配置，用于隔离问题。也可在 settings.json 中设 `"disableAllHooks": true` |

**verbose 模式**：主要通过 `claude config set -g verbose true` 持久化启用（或 `Ctrl+O` 在会话中切换）。启用后显示逐回合完整输出，暴露 subagent 的思考/工具调用/输出（正常情况下 subagent 静默运行）。**推荐用于 hooks 调试**。

**来源**：[Claude Code Cheatsheet - Collabnix](https://collabnix.com/claude-code-cheatsheet/)、[Debug Your Configuration - Claude Code Docs](https://code.claude.com/docs/en/debug-your-config)

### 2.2 调试日志文件位置

| 路径 | 内容 |
|------|------|
| `~/.claude/debug/latest` | 最新 debug 会话日志 |
| `~/.claude/debug/<sessionId>.txt` | 按 session ID 的详细调试日志 |

debug 日志中的 hook 相关条目格式：
```
[DEBUG] Getting matching hook commands for PreToolUse with query: Write
[DEBUG] Found 1 hook matchers in settings
[DEBUG] Matched 1 unique hooks for query "Write" (1 before deduplication)
[DEBUG] PreToolUse:Write [/path/to/hook.sh] completed with status 0
```

**来源**：[Debug Your Configuration - Claude Code Docs](https://code.claude.com/docs/en/debug-your-config)

### 2.3 Session Transcript JSONL 格式

Claude Code 将完整会话记录为 **JSONL** (JSON Lines) 文件，存储在 `~/.claude/projects/<encoded-cwd>/<sessionId>.jsonl`。

**主要条目类型**（v2.1.216 实测，7+ 种）：

| 类型 | 关键字段 | 内容 |
|------|---------|------|
| `custom-title` | `title` | 第一条：用户自定义的会话标题 |
| `mode` | `mode` | 第二条：会话模式（如 `"default"`） |
| `user` | `parentUuid`, `message.content`, `toolUseResult`, `cwd`, `sessionId`, `timestamp` | 用户消息或工具执行结果 |
| `assistant` | `parentUuid`, `message.content[]`, `message.usage`, `message.model` | AI 响应（含 tool_use 块、thinking 块） |
| `system` | `subtype`, `content`, `level` | 系统事件（如 `/resume`、hook 执行；subtype 可为 `"local_command"` 等） |
| `file-history-snapshot` | `snapshot.trackedFileBackups` | `/undo` 功能的文件备份快照 |
| `attachment` | `attachment` | 附件内容（如通过 `/add-dir` 添加） |

此外还可能包含 `last-prompt`、`ai-title`、`tag`、`permission-mode` 等元数据类型。`summary` 类型在 v2.1.216 中**不存在**（已被 `custom-title`/`ai-title` 替代）。

**消息树结构**：通过 `parentUuid` 构成树形关系，支持分支对话（`isSidechain: true`）。

**来源**：[JSONL Format - simonw/claude-code-transcripts (DeepWiki)](https://deepwiki.com/simonw/claude-code-transcripts/5.1-jsonl-format)、[Session File Formats - DeepWiki](https://deepwiki.com/simonw/claude-code-transcripts/5-session-file-formats)

### 2.4 Subagent 日志

Subagent 会话存储在：
```
~/.claude/projects/<encoded-cwd>/<session-uuid>/subagents/agent-<id>.jsonl
```

每个 agent JSONL 有配套的 `.meta.json` 文件，包含 agent 类型、名称、描述和 toolUseId。

**常见错误**：许多工具错误地在项目根目录查找 `agent-*.jsonl`——subagent 日志嵌套在 `<session-uuid>/subagents/` 下。

**来源**：[claude-powerline Issue #53](https://github.com/Owloops/claude-powerline/issues/53)、[agentyard research](https://github.com/wannabefro/agentyard/blob/main/docs/research/claude-code.md)

### 2.5 会话注册表

`~/.claude/session-registry/*.json` 存储会话元数据（sessionId、开销、transcript_path）。注意已知 bug——transcript_path 可能指向不存在的文件（issue #20612）或 git worktree 中路径不匹配（issue #44450）。

**来源**：[Session transcripts never written - Issue #20612](https://github.com/anthropics/claude-code/issues/20612)

### 2.6 内置诊断命令

| 命令 | 作用 |
|------|------|
| `/hooks` | 列出当前会话所有注册的 hook（按事件分组） |
| `/context` | 列出上下文窗口内容（系统提示、memory 文件、skills、MCP 工具、sub-agent 来源） |
| `/doctor` | 安装健康检查（无效 settings、重复 sub-agent 名） |
| `/status` | 活跃配置来源 |
| `/debug [issue]` | 启用会话 debug 日志并提示 Claude 诊断 |

**来源**：[Debug Your Configuration - Claude Code Docs](https://code.claude.com/docs/en/debug-your-config)

### 2.7 自定义日志编写模式

社区常用的 hooks 日志模式（以下编号为 D3 自行归纳分类，源文档未使用 "mode 1-4" 编号体系）：

```bash
# 归纳模式 1：专用日志文件
echo "=== $(date -Iseconds) | $EVENT ===" >> ~/.claude/hooks.log

# 归纳模式 2：带 stdin 捕获的日志包装器
INPUT=$(cat)
echo "=== $(date) ===" >> "$LOG"
echo "$INPUT" | "$1" ; CODE=$?
echo "Exit: $CODE" >> "$LOG"
exit $CODE

# 归纳模式 3：SessionStart 记录 transcript_path
jq -r '"Session: " + .transcript_path' >> ~/.claude/sessions.log

# 归纳模式 4：审计日志
echo "$(date -uIseconds) | WRITE | $FILE_PATH | $SESSION_ID" >> ~/.claude/audit.log
```

**来源**：[Claude Code Handbook - Hooks Guide](https://github.com/ThamJiaHe/claude-code-handbook/blob/main/docs/hooks-guide.md)

---

## 3. 终端日志面板设计模式（同类工具参考）

### 3.1 VS Code 三面板架构

VS Code 采用**三个独立输出面板**的分离式设计：

| 面板 | 定位 | 交互性 | 输入源 | 输出源 |
|------|------|--------|--------|--------|
| **OUTPUT** | 只读日志查看器 | 只读 | 扩展/构建任务 | 扩展诊断和状态信息 |
| **TERMINAL** | 交互式 Shell 模拟器 | 完全交互 | 用户 stdin | 程序 stdout/stderr |
| **Debug Console** | 调试 REPL | 表达式求值 | 用户调试表达式 | 调试器输出 |

设计原则：
- **OUTPUT = 扩展日志通道**：每个扩展有自己的命名通道，通过 `vscode.window.createOutputChannel("name")` 写入
- **TERMINAL = 程序 I/O**：基于 PTY，流式双向字节传输，支持 ANSI 转义码
- **Debug Console = 调试协议**：仅调试会话期间激活

**对 slTerminal 的启示**：hooks 日志应参考 OUTPUT 面板模式——只读、按来源分类、结构化过滤。

**来源**：[VS Code 三种输出位置切换技巧](https://wenku.csdn.net/answer/gp1i5sj4sx)

### 3.2 Warp 终端 Block 模型

Warp 将终端视口组织为**类型化 Block 的有序列表 (BlockList)**，而非传统终端的 2D 字符网格：

**Block 结构**：
- 每个 Block 包含**命令侧网格**（提示符 + 输入命令）和**输出侧网格**（命令输出）
- Block 类型决定存储和渲染方式
- 通过 Shell 的 `precmd`/`preexec` hook 检测命令边界

**存储架构**：
| 存储类型 | 用途 | 特性 |
|----------|------|------|
| **GridStorage** | 活跃区域 + 交替屏幕 | 可变、随机访问 |
| **FlatStorage** | 滚动缓冲区 | 字节偏移索引，节省内存，写入后不可变 |

**性能优化——SumTree 索引**：
- BlockList 使用 SumTree（平衡树，内部节点聚合高度）替代 `Vec`
- 视口交叉查询从 O(n) 降至 **O(log n)**
- 渲染双层虚拟化：BlockList 层（仅可见 Block 渲染）和终端 Block 层（仅视口内行渲染）

**Agent/终端混合**：
- BlockList **不关心 Block 内容**——仅需高度信息
- Agent 对话 Block 与终端 Block 在同一流中混合排列
- Block 可通过赋零高度实现隐藏——过滤/折叠无需特殊渲染代码

**对 slTerminal 的启示**：Block 模型非常适合 hooks 日志——每个 hook 执行作为一个 Block，与终端输出交替排列，支持折叠/过滤。

**来源**：[Warp Block Model](https://www.warp.dev/blog/block-model-behind-warps-agentic-development-environment)、[How Warp Works](https://www.warp.dev/blog/how-warp-works)

### 3.3 Gonzo TUI 日志分析器

2025 年 TUI 日志查看器的主流设计模式：

**四面板 2x2 仪表盘** (k9s/Gonzo 模式)：
```
┌───────────┬───────────┐
│ 严重度图表 │  词频统计  │
├───────────┼───────────┤
│ 实时日志流  │ 时间线/体积 │
└───────────┴───────────┘
```

**交互模式**：
- Vim 式键位（j/k 导航，`/` 搜索，gg/G 首尾）
- 智能自动滚动：跟踪新日志，用户上滚时暂停，`End` 恢复
- Space 全局暂停：冻结仪表盘，缓冲新日志
- 渐进详情级别：`+`/`-` 切换 0-4 级详细度

**架构模式**：
| 模式 | 描述 |
|------|------|
| **Provider/Display 分离** | `LogProvider` trait 获取数据，`LogParser` trait 格式化，框架负责渲染 |
| **Ring Buffer** | 无锁环形缓冲区，容量可配（如 16K 项），旧条目自动淘汰 |
| **非阻塞后台线程** | Provider 在独立线程轮询，UI 线程负责渲染/输入 |
| **格式自动检测** | 自动识别 JSON、logfmt、纯文本格式 |

**2025 年技术栈**：
- Go：Bubble Tea + Lipgloss + Bubbles（Gonzo，2K+ GitHub stars）
- Rust：ratatui + crossterm + ringbuf + rayon（lazylog-framework）
- Python：npyscreen（OrKa TUI）

**对 slTerminal 的启示**：hooks 日志面板可采用 Provider/Display 分离 + Ring Buffer 架构，支持 vim 键位搜索和过滤。

**来源**：[Gonzo TUI Log Analyzer](https://dev.to/discoposse/gonzo-an-open-source-terminal-ui-thats-changing-how-i-analyze-logs-3h40)、[Why We Made Gonzo](https://www.controltheory.com/blog/why-we-made-gonzo-a-terminal-log-viewer-for-observability-next/)

### 3.4 现有 Claude Code 日志可视化工具

| 工具 | 类型 | Hooks 支持 |
|------|------|-----------|
| **claude-devtools** (matt1398) | Electron 桌面应用 | 通知触发器（regex 告警），无独立 Hooks 面板 |
| **@lukehungngo/claude-devtools** | Web 仪表盘 (port 3142) | **独立 Hooks 标签页**，多种事件类型，双向高亮，compaction 归因（具体事件类型数量未在 npm 文档中公开） |
| **claude-session-dashboard** | Web 仪表盘 (port 3000) | 无独立 hooks 视图 |
| **cc-history-viewer** | Web 仪表盘 (port 3080) | Hook 输出出现在 Attachments 区域 |

`@lukehungngo/claude-devtools` 的 Hooks 标签页功能：
- 每行一个 hook 执行记录（历史 + 实时进行中行带 spinner + 耗时计数）
- 列：event type、hook name、tool-use id、duration、exit code、stdout/stderr 预览
- Source 列区分 hook vs Monitor/TaskCreate 队列命令
- 会话总计：count、failed、cancelled、平均耗时、总 hook 耗时
- 双向高亮：hover hook 行高亮对应的 tool_use，反之亦然

**对 slTerminal 的启示**：hooks 日志面板应包含运行历史表格（带 exit code、duration）+ 实时进行中指示 + 与终端 tool_use 的双向关联。

**来源**：[claude-devtools](https://github.com/matt1398/claude-devtools)、[@lukehungngo/claude-devtools](https://www.npmjs.com/package/@lukehungngo/claude-devtools)、[cc-history-viewer](https://www.npmjs.com/package/cc-history-viewer)

---

## 4. 结构化日志与过滤

### 4.1 JSONL 格式——事实标准

Claude Code 生态中 JSONL 是日志存储的事实标准：
- **转录文件**：`~/.claude/projects/<slug>/<sessionId>.jsonl`
- **历史文件**：`~/.claude/history.jsonl`
- **自定义日志**：`~/.claude/skills/logs/<plugin>/<skill>/YYYY-MM-DD.jsonl`

每条 JSONL 行是一个自包含 JSON 对象，结构因类型而异。

### 4.2 社区日志模式

`athola/claude-night-market` 项目的 hook 日志格式：
```json
{
  "timestamp": "2025-07-25T10:30:00.000Z",
  "invocation_id": "uuid",
  "skill": "plugin-name",
  "duration_ms": 150,
  "outcome": "success|failure|partial",
  "context": {
    "session_id": "...",
    "tool_input": {},
    "output_preview": "..."
  },
  "error": "前500字符（失败时）"
}
```

`johnlindquist/claude-hooks` 项目的会话数据格式：
```json
{
  "timestamp": "2025-07-25T10:30:00.000Z",
  "hookType": "PreToolUse",
  "payload": {
    "session_id": "abc123",
    "transcript_path": "...",
    "hook_event_name": "PreToolUse",
    "tool_name": "Edit",
    "tool_input": {}
  }
}
```

### 4.3 处理器链模式（structlog 参考）

Python structlog 的生产级结构化 JSON 日志采用**处理器链**（processor chain）模式：

1. 事件进入 → 逐个处理器丰富字段 → 最终渲染器输出 JSON
2. 处理器按序执行：`merge_contextvars` → `add_log_level` → `TimeStamper` → `EventRenamer` → `JSONRenderer`
3. 自定义处理器可在任意位置插入

**对 slTerminal 的启示**：hooks 日志管道可采用类似处理器链——原始 hook 事件 → 添加时间戳 → 分类标签 → 输出预览截断 → JSON 序列化。

**来源**：[structlog production config](https://www.dash0.com/guides/python-logging-with-structlog)

### 4.4 三维过滤设计

| 维度 | 机制 | 参考实现 |
|------|------|---------|
| **事件类型** | 自定义 `event_type` 字段 + 内容过滤 | structlog `ensure_event_type` 处理器 + `DropEvent` |
| **时间** | ISO 8601 时间戳 + 按小时分区文件 + 时间范围查询 | structlog `TimeStamper(fmt="iso")` + timequerylog 小时分区 |
| **Hook 名称** | `CallsiteParameterAdder` 添加 `func_name` 字段 + 基于函数名的过滤 | structlog `CallsiteParameterAdder` + 自定义过滤器 |

**对 slTerminal 的启示**：hooks 日志面板应支持三种过滤维度：按事件类型（PreToolUse/PostToolUse 等）、时间范围、hook 名称/ID，并提供全文搜索。

**来源**：[structlog filtering examples](https://www.structlog.org/en/20.2.0/examples.html)、[timequerylog](https://www.skypack.dev/view/timequerylog)

---

## 5. 社区讨论与已知问题

### 5.1 Hook 输出可见性相关 Issues

| Issue | 标题 | 状态 |
|-------|------|------|
| [#11224](https://github.com/anthropics/claude-code/issues/11224) | PostToolUse hook output visibility depends on exit code and stream | 社区记录了三种模式（见 1.3） |
| [#4084](https://github.com/anthropics/claude-code/issues/4084) | Hook Output Visibility Blocked in Claude Code UI | — |
| [#13650](https://github.com/anthropics/claude-code/issues/13650) | SessionStart hook stdout silently dropped despite valid JSON and exit 0 | v2.0.76 已修复 |
| [#13912](https://github.com/anthropics/claude-code/issues/13912) | UserPromptSubmit hooks: stdout causes error despite docs saying it's added to context | v2.0.69 已知回归 |
| [#10875](https://github.com/anthropics/claude-code/issues/10875) | Plugin hooks JSON output not captured | — |
| [#65120](https://github.com/anthropics/claude-code/issues/65120) | PostToolUse hook stdout/stderr never surfaces in agent context | Workaround: 使用 PreToolUse + exit 2 |
| [#27886](https://github.com/anthropics/claude-code/issues/27886) | PostToolUseFailure hook always shows 'hook error' even with exit 0 | — |
| [#64119](https://github.com/anthropics/claude-code/issues/64119) | Hook stdout shown + suppressOutput:true ignored in 2.1.158 | v2.1.158 回归 |
| [#28305](https://github.com/anthropics/claude-code/issues/28305) | SessionStart 'compact' matcher: additionalContext not injected | 2026年3月确认修复 |

### 5.2 Hook 执行与调试相关 Issues

| Issue | 标题 | 关键发现 |
|-------|------|---------|
| [#16047](https://github.com/anthropics/claude-code/issues/16047) | Hooks stop executing after ~2.5 hours | 根因：`~/.claude/hooks.log` 增长到 48GB 导致静默失败。建议日志轮转 |
| [#33606](https://github.com/anthropics/claude-code/issues/33606) | Plugin init failures reported as 'hook error' | 错误消息无法区分用户 hook 失败和插件初始化失败 |
| [#18900](https://github.com/anthropics/claude-code/issues/18900) | Plugin hooks loaded but not in registry lookups | 插件 hook 加载到独立注册表，查询未合并 |
| [#55644](https://github.com/anthropics/claude-code/issues/55644) | PostToolUse hooks not firing (v2.1.119+) | 跨 9 天 42 个 session 全部失效 |
| [#24115](https://github.com/anthropics/claude-code/issues/24115) | Plugin hooks fire twice | marketplace source + cache 双重加载 |
| [#50287](https://github.com/anthropics/claude-code/issues/50287) | **Feature Request**: Hook runtime execution telemetry | **最关键的需求**：请求暴露 hook 的 fire、exit code、duration、stderr/stdout。建议通过 `/hooks` 命令 + transcript JSONL 事件实现 |

### 5.3 Hook 运行时遥测需求（Issue #50287）

社区的核心需求——**目前不存在的功能**：
- 无法查看哪个 hook 触发
- 无法查看 hook 退出码
- 无法查看 hook 执行耗时
- 无法查看 hook 的 stderr/stdout
- 必须自行构建日志系统

此 feature request 建议：
1. `/hooks` slash command 展示每个 hook 最近一次执行的统计数据
2. Transcript JSONL 中新增 hook 执行事件类型
3. 与静态 hook 可见性需求（#49778）互补

---

## 6. 对 slTerminal 的启示与设计建议

### 6.1 核心洞察

1. **Claude Code 目前没有内置的 hook 运行时遥测/可视化。** 用户在调试 hook 时完全靠自己构建日志系统。slTerminal 有机会填补这一空白。

2. **Hook 输出可见性极其复杂且不稳定**——退出码 + stdout/stderr + 事件类型的三维矩阵，不同版本行为不一致。可视化面板需要清晰区分"用户可见"、"Claude 可见"、"仅日志"三种可见性层级。

3. **现有社区工具多基于 JSONL 文件后处理**——claude-devtools、cc-history-viewer 等需扫描完整的 `~/.claude/projects/` 目录。slTerminal 如果实现实时 hook 日志面板，将优于这些后处理方案。

4. **Warp 的 Block 模型非常适合 hook 日志组织**——每次 hook 执行是一个自包含 Block，可以折叠、过滤、与终端输出交替排列。

### 6.2 推荐架构

```
slTerminal Hook 日志面板
├── 数据采集层
│   ├── IPC 命令：监听 hook 执行事件（Tauri event "hook-event"）
│   ├── 实时 JSONL 文件 tail：~/.claude/projects/<slug>/hooks.jsonl
│   └── /hooks 命令输出解析（静态注册表快照）
├── 存储层
│   ├── Ring Buffer（环形缓冲区）：内存中保留最近 N 条 hook 执行
│   ├── 结构化 JSON 格式：timestamp, event_type, hook_name, exit_code, duration_ms, stderr_preview
│   └── 持久化：按 session 写入 hooks.jsonl
├── 过滤器
│   ├── 按事件类型（PreToolUse, PostToolUse, SessionStart, Stop...）
│   ├── 按时间范围
│   ├── 按退出码（成功/阻塞/错误）
│   └── 全文搜索
└── UI 层
    ├── Hook 执行历史表格（事件类型、hook 名称、退出码、耗时、输出预览）
    ├── 实时进行中指示（spinner + 已运行时长）
    ├── 展开查看完整 stdout/stderr
    ├── 与终端输出关联（hover hook → 高亮对应 tool_use）
    └── 配置：可见性过滤、日志级别
```

### 6.3 关键设计决策

| 决策点 | 建议 | 理由 |
|--------|------|------|
| 存储格式 | JSONL | 与 Claude Code 生态一致，行级读写高效 |
| 缓冲策略 | Ring Buffer (256KB) | 同 PTY output_ring 模式，避免大日志 OOM |
| UI 模式 | 专用 Hook 面板（Dockview tab） | 符合项目架构约束 #5（面板封闭） |
| 数据源 | Tauri event + JSONL tail 双路径 | 实时性 + 可靠性互补 |
| 可见性标注 | 三种标签：user-only / claude-only / both | 映射 exit code + stream 三维矩阵 |
| 过滤器 | 事件类型 + 退出码 + 时间 + 搜索 | 覆盖最常用调试场景 |

### 6.4 优先实现路径

**Phase 1（MVP）**：读取 `~/.claude/debug/latest` 中的 hook 执行日志，提取 `[DEBUG]` 行中 hook 触发/匹配/退出码信息，展示为表格。

**Phase 2**：新增自定义 `hooks.jsonl` 日志文件，通过 SessionStart hook 自动初始化，PostToolUse/Stop 等 hook 自动写入结构化日志条目。

**Phase 3**：实时面板 + 过滤器 + 与终端输出关联。

---

## 信息来源汇总

| 来源 | URL | 类型 | 日期 |
|------|-----|------|------|
| Claude Code Docs - Hooks Reference | https://code.claude.com/docs/en/hooks | 官方文档 | 2026 |
| Claude Code Docs - Debug Your Config | https://code.claude.com/docs/en/debug-your-config | 官方文档 | 2026 |
| Claude Code Docs - Automate Actions with Hooks | https://code.claude.com/docs/en/hooks-guide | 官方文档 | 2026 |
| Claude Code Blog - How to Configure Hooks | https://claude.com/blog/how-to-configure-hooks | 官方博客 | 2025 |
| Steve Kinney - Hook Control Flow | https://stevekinney.com/courses/ai-development/claude-code-hook-control-flow | 教程 | 2025 |
| MorphLLM - 30 Hook Events Reference | https://www.morphllm.com/claude-code-hooks | 参考 | 2026 |
| dev.to - 8 Production Patterns | https://dev.to/claudeguide/claude-code-hooks-8-production-patterns-2026-3e8f | 博客 | 2026 |
| Collabnix - Mastering Hooks | https://collabnix.com/mastering-claude-code-hooks-automate-your-dev-workflow-in-2026/ | 教程 | 2026 |
| GitHub Issue #11224 - PostToolUse Visibility | https://github.com/anthropics/claude-code/issues/11224 | Issue | 2025 |
| GitHub Issue #50287 - Runtime Telemetry FR | https://github.com/anthropics/claude-code/issues/50287 | Feature Request | 2026 |
| GitHub Issue #16047 - Hooks Stop After 2.5h | https://github.com/anthropics/claude-code/issues/16047 | Issue | 2025 |
| GitHub Issue #20612 - Transcripts Not Written | https://github.com/anthropics/claude-code/issues/20612 | Issue | 2025 |
| GitHub Issue #33606 - Plugin Init Misreported | https://github.com/anthropics/claude-code/issues/33606 | Issue | 2026 |
| GitHub Issue #18900 - Plugin Hook Registry Gap | https://github.com/anthropics/claude-code/issues/18900 | Issue | 2025 |
| GitHub Issue #13650 - SessionStart stdout Drop | https://github.com/anthropics/claude-code/issues/13650 | Issue | 2025 |
| GitHub Issue #10875 - Plugin Hook JSON Not Captured | https://github.com/anthropics/claude-code/issues/10875 | Issue | 2025 |
| GitHub Issue #65120 - PostToolUse Never in Context | https://github.com/anthropics/claude-code/issues/65120 | Issue | 2026 |
| GitHub Issue #55644 - PostToolUse Batch Regression | https://github.com/anthropics/claude-code/issues/55644 | Issue | 2025 |
| GitHub Issue #24115 - Plugin Hooks Fire Twice | https://github.com/anthropics/claude-code/issues/24115 | Issue | 2025 |
| Simon Willison - Transcript JSONL Format | https://deepwiki.com/simonw/claude-code-transcripts/5.1-jsonl-format | 分析 | 2025 |
| ccrider - Session Schema Research | https://github.com/neilberkman/ccrider/blob/main/research/schema.md | 分析 | 2025 |
| ThamJiaHe - Claude Code Handbook | https://github.com/ThamJiaHe/claude-code-handbook/blob/main/docs/hooks-guide.md | 社区指南 | 2025 |
| claude-devtools | https://github.com/matt1398/claude-devtools | 工具 | 2026 |
| @lukehungngo/claude-devtools | https://www.npmjs.com/package/@lukehungngo/claude-devtools | 工具 | 2026 |
| claude-session-dashboard | https://github.com/dlupiak/claude-session-dashboard | 工具 | 2025 |
| cc-history-viewer | https://www.npmjs.com/package/cc-history-viewer | 工具 | 2026 |
| DazzleML - claude-session-logger | https://github.com/DazzleML/claude-session-logger | 工具 | 2025 |
| Warp - Block Model | https://www.warp.dev/blog/block-model-behind-warps-agentic-development-environment | 参考 | 2025 |
| Warp - How Warp Works | https://www.warp.dev/blog/how-warp-works | 参考 | 2025 |
| Gonzo - TUI Log Analyzer | https://dev.to/discoposse/gonzo-an-open-source-terminal-ui-thats-changing-how-i-analyze-logs-3h40 | 参考 | 2025 |
| structlog - Structured Logging | https://www.dash0.com/guides/python-logging-with-structlog | 参考 | 2025 |
| VS Code 三面板架构 | https://wenku.csdn.net/answer/gp1i5sj4sx | 参考 | 2025 |
| AgentWorkforce - Hooks SKILL.md | https://github.com/AgentWorkforce/relay/blob/main/.claude/skills/creating-claude-hooks-skill/SKILL.md | 社区 | 2025 |
| dev.to - Log Agent Invocations | https://dev.to/bokuwalily/log-every-agent-invocation-building-usage-analytics-with-claude-codes-stop-hook-and-6oc | 博客 | 2025 |
| Collabnix - Claude Code Cheatsheet | https://collabnix.com/claude-code-cheatsheet/ | 参考 | 2026 |
