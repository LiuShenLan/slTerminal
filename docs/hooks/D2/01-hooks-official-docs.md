# Claude Code Hooks 官方文档汇总

> 信息收集日期：2026-07-25
> 主要来源：Claude Code 官方文档 (code.claude.com/docs/en/hooks)、Anthropic 官方博客、GitHub Issues

---

## 1. 配置文件层级（settings.json）

Hooks 在 JSON 设置文件的四层中配置，优先级为: 托管策略 > local > project > user。本地设置(.local.json)覆盖项目设置(.settings.json)，项目设置覆盖用户设置(~/.claude/settings.json)：

| 文件 | 作用域 | 是否可共享 |
|------|--------|-----------|
| `~/.claude/settings.json` | 所有项目（用户全局） | 否 |
| `.claude/settings.json` | 单个项目 | 是（可提交到仓库） |
| `.claude/settings.local.json` | 单个项目（本地） | 否（应 gitignore） |
| 托管策略设置 | 组织级 | 是 |

### 基本配置结构

```json
{
  "hooks": {
    "EventName": [
      {
        "matcher": "ToolPattern",
        "hooks": [
          {
            "type": "command",
            "command": "your-shell-command-here",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 2. 完整 Schema 字段说明

### 2.1 `matcher` 组级别的字段

| 字段 | 必填 | 说明 |
|------|:--:|------|
| `matcher` | 是 | 匹配工具名的模式（区分大小写） |
| `hooks` | 是 | 要执行的 hook handler 数组 |

`matcher` 仅在 `PreToolUse`、`PostToolUse`、`PermissionRequest` 等工具事件中有效。生命周期事件（如 `UserPromptSubmit`）不需要 matcher。

### 2.2 Hook Handler 级别字段

| 字段 | 必填 | 说明 |
|------|:--:|------|
| `type` | 是 | `"command"`（shell 脚本）、`"prompt"`（LLM 评估）、`"agent"`（agent 验证）、`"http"`（POST 到 URL）或 `"mcp_tool"`（调用 MCP 工具） |
| `command` | type=`"command"` 时 | Shell 命令，使用 `$CLAUDE_PROJECT_DIR` 引用项目路径 |
| `prompt` | type=`"prompt"` 时 | 发送给 LLM 的评估 prompt |
| `url` | type=`"http"` 时 | HTTP POST 目标 URL |
| `timeout` | 否 | 超时秒数（超时后 hook 被取消），默认值因事件类型而异 |
| `if` | 否 | **细粒度条件过滤**（v2.1.85+），按工具名+参数匹配，使用权限规则语法 |

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 3. Hook 类型

有 5 种 hook handler 类型：

| 类型 | 说明 | 示例 |
|------|------|------|
| `command` | 执行 shell 命令，输入通过 stdin JSON 传入 | `"command": "node lint-check.js"` |
| `prompt` | 将 prompt 发送给 LLM 评估，结果决定后续动作 | `"prompt": "检查代码质量..."` |
| `agent` | 启动 agent 执行验证任务 | 用于复杂多步骤验证 |
| `http` | POST JSON 到指定 URL | 用于集成外部系统/Webhook |
| `mcp_tool` | 调用 MCP 工具 | 用于调用 MCP 服务器提供的能力 |

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 4. 退出码语义

| 退出码 | 含义 | 行为 |
|:------:|------|------|
| **0** | 成功 | 允许操作继续；stdout 中的 JSON 被解析处理 |
| **1** | 非阻塞错误 | stderr 作为警告显示，操作继续执行 |
| **2** | 阻塞错误 | **阻止操作**；stderr 反馈给 Claude/model |
| 其他 | 同退出码 1 | 非阻塞错误 |

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 5. 环境变量

Hook 执行时可用的环境变量：

| 变量 | 说明 | 可用范围 |
|------|------|---------|
| `CLAUDE_PROJECT_DIR` | 项目根目录绝对路径 | 所有 hooks |
| `CLAUDE_CODE_REMOTE` | 远程/Web 环境时为 `"true"` | 所有 hooks |
| `CLAUDE_ENV_FILE` | 写入 `export VAR=value` 以持久化环境变量的文件路径 | SessionStart, Setup, CwdChanged, FileChanged |
| `CLAUDE_PLUGIN_ROOT` | 插件目录绝对路径 | 仅插件 hooks |
| `CLAUDE_CODE_BRIDGE_SESSION_ID` | Remote Control 会话 ID（v2.1.199+） | Remote Control 时 |
| `HOME` | 用户主目录 | 所有 hooks |
| `PATH`、`USER` 等 | 标准 shell 环境变量 | 所有 hooks |

**重要说明**：`CLAUDE_TOOL_INPUT` 和 `CLAUDE_TOOL_RESPONSE` 环境变量**不存在**——工具输入/输出通过 **stdin JSON** 传入，不是环境变量。

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 6. stdin JSON 输入字段

所有 hook 的 `type: "command"` 接收的 JSON 通过 stdin 传入。以下是各字段的完整说明。

### 6.1 所有事件通用字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `session_id` | string | 唯一会话标识符 |
| `transcript_path` | string | 会话对话记录 JSONL 路径 |
| `cwd` | string | 当前工作目录 |
| `permission_mode` | string | `"default"`、`"plan"`、`"acceptEdits"`、`"bypassPermissions"` 等 |
| `hook_event_name` | string | 触发此 hook 的事件名（如 `"PreToolUse"`） |
| `agent_id` | string | 子代理内的唯一标识符（仅子代理中） |
| `agent_type` | string | 代理类型名（仅子代理中） |
| `effort` | object | 如 `{"level": "high"}`（per-turn/tool 事件） |

### 6.2 工具事件额外字段（PreToolUse / PostToolUse / PostToolUseFailure）

| 字段 | 类型 | 说明 |
|------|------|------|
| `tool_name` | string | 被调用的工具名：`"Bash"`、`"Read"`、`"Edit"`、`"Write"` 等 |
| `tool_input` | object | 工具参数（如 `{"command": "npm test"}` 或 `{"file_path": "..."}`） |
| `tool_response` | object | **PostToolUse** 的工具结果 |
| `tool_use_id` | string | 唯一工具调用标识符 |

### 6.3 各事件特有字段

| 事件 | 额外字段 |
|------|---------|
| **UserPromptSubmit** | `prompt` — 用户提交的文本 |
| **SessionStart** | `source` — `"startup"`/`"resume"`/`"clear"`/`"compact"`；`model` — 模型标识符 |
| **PreCompact** | `trigger` — `"manual"`/`"auto"`；`custom_instructions` — 来自 `/compact` 的用户输入 |
| **SubagentStart/Stop** | `subagent_type`、`agent_id`、`duration_ms`（Stop 时） |
| **Stop** | `stop_hook_active` — 布尔值，为 true 表示已从 Stop hook 继续过一次（**防无限循环关键字段**）；`last_assistant_message` — Claude 的最后响应文本 |
| **StopFailure** | `stop_failure_reason`、`api_status_code` |
| **TaskCreated/TaskCompleted** | `task_id`、`task_subject`、`token_count` |
| **Notification** | `notification_type`、`message`、`title` |

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 7. 完整 Hook 事件类型列表（30 种）

### 7.1 会话生命周期（Session Lifecycle）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `SessionStart` | 会话启动/恢复 | 否 | `startup`/`resume`/`clear`/`compact` |
| `SessionEnd` | 会话终止 | 否 | 退出原因 |
| `Setup` | `--init-only` 或 `--init`/`--maintenance` 配合 `-p` | 否 | — |

### 7.2 用户交互（User Interaction）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `UserPromptSubmit` | 用户提交 prompt，Claude 处理前 | **是** | 无 |
| `UserPromptExpansion` | slash command 或 MCP prompt 扩展后 | **是** | — |

### 7.3 工具调用（Tool Use）——核心

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `PreToolUse` | 工具参数构建后、执行**前** | **是** | 工具名/正则 |
| `PermissionRequest` | 权限对话框要弹出时 | **是** | 工具名/正则 |
| `PermissionDenied` | 自动模式分类器拒绝工具调用时 | 否 | — |
| `PostToolUse` | 工具调用**成功**后 | 否 | 工具名/正则 |
| `PostToolUseFailure` | 工具调用**失败**后 | 否 | 工具名/正则 |
| `PostToolBatch` | 并行工具调用批次全部解析后，下一轮模型调用前 | **是** | — |

### 7.4 通知与消息（Notifications & Messages）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `Notification` | Claude Code 发送通知时 | 否 | 通知类型 |
| `MessageDisplay` | 助理消息文本流式输出时 | 否 | — |

### 7.5 子代理与任务（Subagents & Tasks）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `SubagentStart` | 子代理（Task tool）被产生时 | 否 | 代理类型名 |
| `SubagentStop` | 子代理完成时 | **是** | 代理类型名 |
| `TaskCreated` | `TaskCreate` 创建任务时 | **是** | 无 |
| `TaskCompleted` | 任务标记为完成时 | **是** | 无 |
| `TeammateIdle` | agent team 队友即将空闲时 | **是** | — |

### 7.6 上下文管理（Context Management）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `PreCompact` | 上下文压缩前 | **是** (v2.1.105+) | `manual`/`auto` |
| `PostCompact` | 上下文压缩完成后 | 否 | `manual`/`auto` |

### 7.7 停止与错误（Stop & Error）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `Stop` | Claude 完成响应时 | **是** | — |
| `StopFailure` | 回合因 API 错误结束时 | 否 | 错误类型 |

### 7.8 配置与文件变更（Config & Files）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `ConfigChange` | 会话期间配置文件变更 | **是** | — |
| `CwdChanged` | 工作目录变更（如 `cd` 命令） | 否 | — |
| `FileChanged` | 被监视的文件发生磁盘变更 | 否 | — |
| `InstructionsLoaded` | CLAUDE.md 或 `.claude/rules/*.md` 被加载 | 否 | — |

### 7.9 工作树（Worktree）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `WorktreeCreate` | 创建 git worktree 时（可替换默认行为） | **是** | — |
| `WorktreeRemove` | 删除 worktree 时 | 否 | — |

### 7.10 启发式交互（Elicitation）

| 事件 | 触发时机 | 可阻止？ | matcher |
|------|---------|:---:|---------|
| `Elicitation` | MCP 服务器在工具调用中请求用户输入时 | **是** | — |
| `ElicitationResult` | 用户响应 MCP elicitation 后，服务器处理前 | **是** | — |

来源：[morphllm.com/claude-code-hooks](https://www.morphllm.com/claude-code-hooks)、[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 8. Matcher 语法（正则匹配）

### 8.1 基本规则

`matcher` 的匹配行为取决于 pattern 中是否包含特殊字符：

| Pattern | 行为 | 示例 |
|---------|------|------|
| 仅含字母/数字/`_`/`-`/`|`/`,` | **精确字符串匹配**，`|` 和 `,` 为或关系 | `Edit|Write` 匹配 Edit 或 Write |
| 包含任何其他字符 | **JavaScript 正则**（非锚定） | `^mcp__` 匹配所有 MCP 工具 |
| `*`、`""` 或省略 | **匹配全部** | 通配所有工具/事件 |

### 8.2 分类匹配示例

| 语法 | 示例 | 说明 |
|------|------|------|
| 简单字符串 | `"Write"` | 仅匹配 Write 工具 |
| 竖线分隔 | `"Write\|Edit"` | 匹配 Write 或 Edit |
| 通配 | `"*"` 或省略 | 匹配所有工具 |
| MCP 正则 | `"mcp__memory__.*"` | 匹配 memory MCP 服务器的所有工具 |
| 构建工具 | `"Bash"` | 匹配 Bash 工具（**区分大小写**） |
| 文件路径过滤 | 在 `if` 字段中 | `"Read(.env)"` — 按路径过滤 |

### 8.3 注意事项

- Matcher **区分大小写**——`"bash"` 不会匹配 `Bash` 工具
- 连接号名称（如 `code-reviewer`）在 Claude Code v2.1.195+ 中被视为精确匹配，旧版本需用 `^code-reviewer$` 锚定

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 9. `if` 字段——细粒度条件过滤（v2.1.85+）

`if` 字段在 matcher 之后执行，按**工具名+参数**进行更细粒度的匹配。位于单个 hook handler 内（`type` 和 `command` 同级）。

### 9.1 过滤流水线

```
事件触发 → matcher（工具名）→ if（工具名+参数）→ 执行 hook
```

两层过滤在**产生进程之前**执行，不命中的零开销。

### 9.2 v2.1.88 改进——解析式子命令匹配

在 v2.1.85 中 `if` 仅做字面前缀匹配，v2.1.88 改为**解析式子命令匹配**：

- 剥离 `VAR=value` 前缀
- 复合命令（`&&`/`||` 等）拆分子命令
- 任一子命令匹配 pattern 即触发

| 命令 | 匹配 `"Bash(git *)"`？ | 原因 |
|------|:-:|------|
| `git status` | 是 | 直接 git 命令 |
| `git push origin main` | 是 | git 带参数 |
| `ls && git push` | 是 (v2.1.88+) | 复合命令——git 子命令被解析 |
| `FOO=bar git push` | 是 (v2.1.88+) | 环境变量前缀被剥离 |
| `npm test` | 否 | 非 git 命令 |
| `echo "git push"` | 否 | git 在字符串中，非子命令 |

**安全兜底**：命令过于复杂无法解析时，hook 始终触发。

### 9.3 `if` 配置示例

```json
{
  "matcher": "Bash",
  "hooks": [
    {
      "type": "command",
      "if": "Bash(git push *)",
      "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/block-force-push.sh"
    },
    {
      "type": "command",
      "if": "Bash(rm *)",
      "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/block-rm.sh"
    }
  ]
}
```

### 9.4 文件工具的路径 `if`

| Pattern | 说明 |
|---------|------|
| `"Edit(src/**)"` | 匹配 src 目录下所有文件 |
| `"Read(.env)"` | 匹配 .env 文件读取 |
| `"Write(*.ts)"` | 匹配所有 .ts 文件写入 |

来源：[GitHub Issue #41262](https://github.com/anthropics/claude-code/issues/41262)

---

## 10. stdout JSON 输出字段

Hook 通过 stdout 输出 JSON 与 Claude Code 通信。**仅解析 stdout 的第一行 JSON。**

### 10.1 顶层字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `continue` | boolean | 是否允许继续（`false` = 阻止） |
| `systemMessage` | string | 向**用户**终端显示的消息（不注入模型上下文） |
| `stopReason` | string | 阻止原因 |
| `suppressOutput` | boolean | 是否隐藏 stdout 输出 |
| `decision` | string | 阻止指令：`"block"` + `"reason"` |
| `reason` | string | 伴随 `decision: "block"` 的解释 |
| `hookSpecificOutput` | object | 事件特定的结构化输出 |

### 10.2 `hookSpecificOutput` 的 `hookEventName` 有效值

> `PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`PostToolUseFailure`、`PermissionRequest`、`SessionStart`、`SubagentStart`、`SubagentStop`、`Stop`

**注意**：`PreCompact` 不在有效值列表中，不能用 `hookSpecificOutput`。

### 10.3 PreToolUse 输出

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "自动批准安全命令",
    "updatedInput": {"command": "npm test --coverage"}
  }
}
```

- `permissionDecision`：`"allow"`（放行）、`"deny"`（拒绝）、`"ask"`（要求用户确认）
- `updatedInput`：修改工具输入参数，在工具执行前生效

### 10.4 PostToolUse 输出

```json
{
  "decision": "block",
  "reason": "发现 lint 错误，请修复",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "第 42 行：未使用变量"
  }
}
```

**已知 Bug**：`additionalContext` 已文档化但 SDK v0.2.9 中未实现——Claude 不会看到注入的上下文。

### 10.5 Stop 输出

```json
{
  "decision": "block",
  "reason": "测试尚未运行。请运行 npm test。"
}
```

**关键**：Stop hook 中必须检查输入中的 `stop_hook_active: true`——为 true 时直接退出（exit 0），防止无限循环。

### 10.6 SessionStart 输出

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "项目使用 pnpm，Node 版本：20.x"
  }
}
```

- `additionalContext` 被注入到模型上下文
- 纯文本 stdout 也会被作为上下文注入
- **已知 Bug (v2.1.88-v2.1.132)**：`systemMessage` 不再在终端 UI 中渲染

### 10.7 UserPromptSubmit 输出

```json
{
  "decision": "block",
  "reason": "prompt 包含敏感数据",
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "当前分支：feature/auth"
  }
}
```

- `decision: "block"` 会**擦除 prompt**，`reason` 仅显示给用户
- 退出码 2 也可阻止 prompt

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)、[GitHub Issue #18534](https://github.com/anthropics/claude-code/issues/18534)、[GitHub Issue #41285](https://github.com/anthropics/claude-code/issues/41285)

---

## 11. 阻止（Blocking）机制总结

### 11.1 哪些事件可以阻止

| 事件 | 阻止方式 | 阻止效果 |
|------|---------|---------|
| `PreToolUse` | 退出码 2 / `permissionDecision: "deny"` | 取消工具调用 |
| `UserPromptSubmit` | 退出码 2 / `decision: "block"` | **擦除**用户 prompt |
| `Stop` | `decision: "block"` | 强制 Claude 继续工作 |
| `SubagentStop` | `decision: "block"` | 强制子代理继续 |
| `PostToolUse` | `decision: "block"` | 提示 Claude 处理问题 |
| `PreCompact` | 退出码 2 / `continue: false` | 阻止压缩 |
| `TaskCreated` | 退出码 2 | 回滚任务创建 |
| `TaskCompleted` | 退出码 2 / `continue: false` | 阻止任务完成 |
| `PermissionRequest` | `permissionDecision: "deny"` | 拒绝工具执行 |

### 11.2 哪些事件不可阻止

`SessionStart`、`PostToolUseFailure`、`Notification`、`PostCompact`、`SubagentStart`、`CwdChanged`、`FileChanged`、`InstructionsLoaded`、`WorktreeRemove`、`StopFailure`、`SessionEnd`、`Setup`、`PermissionDenied`、`MessageDisplay`——这些事件只能观察/附加副作用，不能阻止。

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 12. 通知类型（Notification matcher 值）

`Notification` 事件的 matcher 匹配通知类型：

| Matcher | 说明 |
|---------|------|
| `permission_prompt` | Claude 请求工具权限 |
| `idle_prompt` | Claude 空闲超过 60 秒 |
| `auth_success` | 认证成功 |
| `elicitation_dialog` | MCP elicitation 对话框显示 |
| `elicitation_complete` | Elicitation 完成 |
| `elicitation_response` | Elicitation 响应 |

---

## 13. 全局禁用

```json
{
  "disableAllHooks": true
}
```

放在 `.claude/settings.local.json` 中可全局禁用所有 hooks。也可单独禁用：

```json
{
  "disableStopHook": true,
  "disableNotificationHook": true
}
```

来源：[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)

---

## 14. 常见工具名（用于 matcher）

| 工具名 | 说明 |
|--------|------|
| `Bash` | Shell 命令执行 |
| `Read` | 读取文件 |
| `Write` | 写入文件 |
| `Edit` | 精确字符串替换编辑 |
| `Glob` | 文件模式匹配 |
| `Grep` | 文件内容搜索 |
| `WebFetch` | 获取 URL 内容 |
| `WebSearch` | 网络搜索 |
| `Agent` | 启动子代理 |
| `NotebookEdit` | Jupyter Notebook 编辑 |
| `Task` | 创建后台任务 |

MCP 工具命名格式：`mcp__<服务器名>__<工具名>`（如 `mcp__playwright__browser_screenshot`）

---

## 15. 已知问题（2025-2026）

1. **PostToolUse `additionalContext` 未实现**：[#18534](https://github.com/anthropics/claude-code/issues/18534) — 类型和文档中定义但 SDK v0.2.9 中静默忽略
2. **SessionStart `systemMessage` 不渲染**：[#41285](https://github.com/anthropics/claude-code/issues/41285) — v2.1.88 到 v2.1.132 期间终端不显示。解决方案：写入 stderr
3. **PreCompact 不支持 `hookSpecificOutput`**：[#17](https://github.com/ce-dot-net/ce-claude-marketplace/issues/17) — JSON 验证会失败。使用 `continue`/`systemMessage` 顶层字段代替
4. **仅第一行 JSON 被解析**：如果 hook 输出多行 JSON，仅第一行被读取
5. **`CLAUDE_PLUGIN_ROOT` 未设置**：[#24529](https://github.com/anthropics/claude-code/issues/24529) — hook executor 不设置此变量，插件 hooks 需手动处理
6. **PreToolUse vs PermissionRequest JSON 路径不一致**：[#19124](https://github.com/anthropics/claude-code/issues/19124) — `updatedInput` 在不同事件中嵌套路径不同

---

## 16. 快速配置示例

### 阻止危险 Bash 命令

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/block-dangerous.sh"
          }
        ]
      }
    ]
  }
}
```

### 写文件后自动格式化

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "prettier --write \"$FILE_PATH\""
          }
        ]
      }
    ]
  }
}
```

### Stop 质量门

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PROJECT_DIR}/scripts/quality-gate.sh"
          }
        ]
      }
    ]
  }
}
```

`quality-gate.sh` 模式：

```bash
#!/bin/bash
INPUT=$(cat)
IS_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active')

# 防止无限循环
if [ "$IS_ACTIVE" = "true" ]; then
  exit 0
fi

# 检查测试是否通过
if ! npm test --silent 2>/dev/null; then
  echo '{"decision":"block","reason":"测试失败，请修复后重试"}'
  exit 0
fi
exit 0
```

### SessionStart 注入上下文

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/inject-context.sh"
          }
        ]
      }
    ]
  }
}
```

来源：[claude.com/blog/how-to-configure-hooks](https://claude.com/blog/how-to-configure-hooks)

---

## 17. 信息源

| URL | 说明 |
|-----|------|
| [code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks) | **官方 Hooks 参考文档**（核心来源） |
| [code.claude.com/docs/en/agent-sdk/hooks](https://code.claude.com/docs/en/agent-sdk/hooks) | Agent SDK Hooks 文档 |
| [claude.com/blog/how-to-configure-hooks](https://claude.com/blog/how-to-configure-hooks) | Anthropic 官方博客：Hooks 配置指南 |
| [github.com/anthropics/claude-code/issues/41262](https://github.com/anthropics/claude-code/issues/41262) | `if` 字段文档补全（复合命令+环境变量前缀匹配） |
| [github.com/anthropics/claude-code/issues/18534](https://github.com/anthropics/claude-code/issues/18534) | PostToolUse additionalContext 未实现 bug |
| [github.com/anthropics/claude-code/issues/19115](https://github.com/anthropics/claude-code/issues/19115) | PreToolUse vs PostToolUse JSON schema 不一致 |
| [github.com/anthropics/claude-code/issues/19124](https://github.com/anthropics/claude-code/issues/19124) | PreToolUse vs PermissionRequest updatedInput 路径不一致 |
| [github.com/anthropics/claude-code/issues/41285](https://github.com/anthropics/claude-code/issues/41285) | SessionStart systemMessage 不渲染 bug |
| [github.com/anthropics/claude-code/issues/24529](https://github.com/anthropics/claude-code/issues/24529) | CLAUDE_PLUGIN_ROOT 环境变量未设置 |
| [www.morphllm.com/claude-code-hooks](https://www.morphllm.com/claude-code-hooks) | 30 种 Hook 事件全面总结 |
| [dev.to/rulestack/claude-code-hooks-explained](https://dev.to/rulestack/claude-code-hooks-explained-config-structure-matchers-and-a-copy-paste-pretooluse-guard-58jj) | Hooks 配置结构详解 |
| [blog.promptlayer.com/understanding-claude-code-hooks-documentation/](https://blog.promptlayer.com/understanding-claude-code-hooks-documentation/) | Hooks 文档解读 |
