# Claude Code Hooks 官方文档研究与记录

> 日期：2026-07-25
> 来源：Claude Code 官方文档 (code.claude.com/docs/en/hooks) + Agent SDK 文档 (code.claude.com/docs/en/agent-sdk/hooks) + 社区资料交叉验证

---

## 1. 概述

Hooks 是用户定义的自动化处理程序（Shell 命令、HTTP 回调、LLM 提示、MCP 工具调用或子代理评估），在 Claude Code 会话的特定生命周期点自动执行。

**与 CLAUDE.md 的区别**：CLAUDE.md 中的指令可能被模型忽略（概率性），但 hooks 是**确定性**的——它们总是在事件发生时触发，且可以阻止、修改或增强 Claude 的行为。

---

## 2. 所有 Hook 事件完整列表（30 个）

### 2.1 会话生命周期

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 1 | **SessionStart** | 会话开始或恢复 | 否 | `source`: `"startup"` / `"resume"` / `"clear"` / `"compact"` |
| 2 | **Setup** | `--init-only`、`--init` 或 `--maintenance` 标志时 | 否 | 触发标志 |
| 3 | **SessionEnd** | 会话终止时 | 否 | `reason`: `"clear"` / `"resume"` / `"logout"` / `"prompt_input_exit"` / `"bypass_permissions_disabled"` / `"other"` |
| 4 | **ConfigChange** | 会话期间配置文件变更 | 是（`policy_settings` 不可阻止——仅限审计/日志用途） | 配置来源：`user_settings` / `project_settings` / `local_settings` / `policy_settings` / `skills` |
| 5 | **CwdChanged** | 工作目录变更 | 否 | 无 matcher |
| 6 | **FileChanged** | 监听的文件在磁盘上变更 | 否 | 文件名模式 |
| 7 | **InstructionsLoaded** | CLAUDE.md 或 `.claude/rules/*.md` 加载时 | 否 | 加载原因 |

### 2.2 用户交互

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 8 | **UserPromptSubmit** | 用户提交提示词，处理前 | **是** | `prompt`: 用户输入的文本 |
| 9 | **UserPromptExpansion** | 斜杠命令/MCP 提示词展开为完整提示词时 | **是** | 命令名称 |
| 10 | **Stop** | 主代理完成响应输出 | **是** | `stop_hook_active`: 防止无限循环 |
| 11 | **StopFailure** | 轮次因 API 错误结束 | 否 | 错误类型：`rate_limit` / `authentication_failed` / `billing_error` / `overloaded` / `server_error` |

### 2.3 工具执行

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 12 | **PreToolUse** | 工具执行前（最常用的 hook） | **是** | `tool_name`、`tool_input`、`tool_use_id` |
| 13 | **PermissionRequest** | 权限对话框出现时 | **是** | `tool_name`、`tool_input` |
| 14 | **PermissionDenied** | 自动模式分类器拒绝工具调用时 | 否 | `tool_name` |
| 15 | **PostToolUse** | 工具执行成功后 | 软阻止 | `tool_name`、`tool_input`、`tool_response`、`tool_use_id` |
| 16 | **PostToolUseFailure** | 工具执行失败后 | 软阻止 | `tool_name`、`tool_input`、错误详情 |
| 17 | **PostToolBatch** | 并行工具调用批次全部完成后 | **是** | 无 matcher |

### 2.4 子代理与团队

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 18 | **SubagentStart** | 子代理被创建时 | 否 | 代理类型（Explore、Plan、自定义名称） |
| 19 | **SubagentStop** | 子代理完成时 | **是** | `stop_hook_active` |
| 20 | **TeammateIdle** | 代理团队成员即将空闲时 | **（exit 2）** | 无 matcher |
| 21 | **TaskCreated** | 通过 TaskCreate 创建任务时 | **是** | 无 matcher |
| 22 | **TaskCompleted** | 任务标记为完成时 | **是** | 无 matcher |

### 2.5 上下文压缩

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 23 | **PreCompact** | 上下文压缩前（v2.1.105+） | **是** | `trigger`: `"manual"` / `"auto"`，`custom_instructions` |
| 24 | **PostCompact** | 压缩完成后 | 否 | 同上 |

### 2.6 通知与 MCP

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 25 | **Notification** | Claude 发送系统通知时 | 否 | `notification_type`: `"permission_prompt"` / `"idle_prompt"` / `"auth_success"` / `"elicitation_dialog"` / `"elicitation_complete"` / `"elicitation_response"` / `"agent_needs_input"` / `"agent_completed"` |
| 26 | **MessageDisplay** | 助手消息文本显示时 | 否 | 显示文本 |
| 27 | **Elicitation** | MCP 服务器请求用户输入时 | **是** | MCP 服务器名称 |
| 28 | **ElicitationResult** | 用户响应 MCP 弹出输入时 | **是** | MCP 服务器名称 |

### 2.7 版本控制（Worktree）

| # | 事件名 | 触发时机 | 是否可阻止 | 关键输入字段 |
|---|--------|----------|------------|------------|
| 29 | **WorktreeCreate** | 工作树被创建时 | **是** | 无 matcher，可返回工作树路径 |
| 30 | **WorktreeRemove** | 工作树被移除时 | 否 | 无 matcher |

---

## 3. Hook 输入格式（stdin JSON）

### 3.1 通用字段（所有事件均有）

```json
{
  "session_id": "abc123...",
  "transcript_path": "~/.claude/projects/.../session.jsonl",
  "cwd": "/current/working/directory",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse"
}
```

| 字段 | 说明 |
|------|------|
| `session_id` | 唯一会话标识符 |
| `transcript_path` | 对话记录文件路径（JSONL） |
| `cwd` | hook 触发时的当前工作目录 |
| `permission_mode` | `"default"` / `"plan"` / `"acceptEdits"` / `"bypassPermissions"` |
| `hook_event_name` | 事件名称 |

### 3.2 工具事件专用字段（PreToolUse / PostToolUse / PostToolUseFailure / PermissionRequest）

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "npm install",
    "description": "Install dependencies"
  },
  "tool_use_id": "toolu_01ABC123..."
}
```

| 字段 | 说明 |
|------|------|
| `tool_name` | 即将执行的工具名称（如 `Bash`、`Write`、`Edit`、`Read`、`mcp__server__tool`） |
| `tool_input` | 工具特定参数对象 |
| `tool_use_id` | 工具调用的唯一 ID，可关联 PreToolUse 和 PostToolUse |

### 3.3 PostToolUse 额外字段

```json
{
  "hook_event_name": "PostToolUse",
  "tool_name": "Edit",
  "tool_input": { "file_path": "/path/to/file.ts" },
  "tool_response": {
    "success": true,
    "output": "...",
    "exitCode": 0
  },
  "tool_use_id": "toolu_01ABC123..."
}
```

### 3.4 UserPromptSubmit 额外字段

```json
{
  "hook_event_name": "UserPromptSubmit",
  "prompt": "Fix the bug in auth.ts"
}
```

### 3.5 Notification 额外字段

```json
{
  "hook_event_name": "Notification",
  "notification_type": "permission_prompt"
}
```

`notification_type` 取值：
- `"permission_prompt"` — 需要用户授权工具时（立即触发）
- `"idle_prompt"` — Claude 完成响应后在主提示符等待约 60 秒后触发
- `"auth_success"` — 认证成功时（立即触发）
- `"elicitation_dialog"` — MCP 弹出对话框时
- `"elicitation_complete"` — MCP 弹出表单提交/关闭
- `"elicitation_response"` — MCP 弹出响应发回 server
- `"agent_needs_input"` — 后台 session 等待用户输入（v2.1.198+）
- `"agent_completed"` — 后台 session 完成或失败（v2.1.198+）

### 3.6 SessionStart 额外字段

```json
{
  "hook_event_name": "SessionStart",
  "source": "startup"
}
```

### 3.7 Stop / SubagentStop 额外字段

```json
{
  "hook_event_name": "Stop",
  "stop_hook_active": false
}
```

> 注意：处理 Stop hook 时必须检查 `stop_hook_active` 防止无限循环。

### 3.8 PreCompact 额外字段

```json
{
  "hook_event_name": "PreCompact",
  "trigger": "auto",
  "custom_instructions": "..."
}
```

---

## 4. Hook 输出格式（stdout JSON）

Hook 通过 stdout 返回 JSON 或纯文本。退出码决定行为。

### 4.1 顶层通用字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `continue` | `bool` | `false` 时立即停止 Claude 执行 |
| `stopReason` | `string` | 停止时显示的消息 |
| `suppressOutput` | `bool` | `true` 时隐藏 hook 的 stdout/stderr |
| `systemMessage` | `string` | 显示给用户的系统消息 |
| `decision` | `string` | `"block"` — 阻止操作（旧格式，逐步废弃） |
| `reason` | `string` | 阻止原因说明 |
| `sessionTitle` | `string` | 设置会话标题（UserPromptSubmit） |
| `watchPaths` | `string[]` | 额外文件监听路径（CwdChanged、FileChanged） |

### 4.2 hookSpecificOutput（事件专用）

大部分控制字段位于 `hookSpecificOutput` 内：

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Safe command",
    "updatedInput": { "command": "modified npm install" },
    "additionalContext": "Extra context injected for Claude"
  }
}
```

#### PreToolUse 输出字段

| 字段 | 说明 |
|------|------|
| `permissionDecision` | `"allow"` / `"deny"` / `"ask"` / `"defer"` |
| `permissionDecisionReason` | 决策原因（显示给用户） |
| `updatedInput` | 修改后的工具输入参数（部分更新，仅传需修改的字段） |
| `additionalContext` | 注入到 Claude 上下文的额外信息 |

> 旧格式 `decision: "block"` 已废弃，应使用 `permissionDecision: "deny"`。

#### PermissionRequest 输出字段

> 注意：`updatedInput` 在 PermissionRequest 中位于 `hookSpecificOutput.decision` 内部，与 PreToolUse 的结构不同。

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "allow",
      "updatedInput": { "command": "npm run lint" },
      "message": "...",
      "interrupt": false
    }
  }
}
```

#### PostToolUse 输出字段

| 字段 | 说明 |
|------|------|
| `decision` | `"block"` 阻止继续 |
| `additionalContext` | 注入额外上下文 |
| `updatedToolOutput` | 替换工具输出文本 |

#### UserPromptSubmit 输出字段

| 字段 | 说明 |
|------|------|
| `decision` | `"block"` + `reason` — 阻止提示词处理 |
| `additionalContext` | 注入额外上下文 |
| `systemMessage` | 显示给用户的系统消息 |
| `sessionTitle` | 设置会话标题 |

> exit code 2 + `decision: "block"` 会**擦除提示词**，仅向用户显示 reason（不传递给 Claude）。

#### Stop 输出字段

| 字段 | 说明 |
|------|------|
| `decision` | `"block"` 强制 Claude 继续工作 |
| `reason` | 继续原因（传递给 Claude） |
| `continue` | `true` 继续执行 |

> 必须检查 `stop_hook_active` 防止无限循环。

#### SessionStart 输出字段

| 字段 | 说明 |
|------|------|
| `additionalContext` | 注入到会话上下文中（多个 hook 的 additionalContext 会拼接） |

#### 纯文本输出

如果 hook 不输出 JSON，stdout 的**纯文本**也会被作为 `additionalContext` 注入到 Claude 的上下文中。

### 4.3 权限决策优先级

当多个 hook 返回冲突的权限决策时：**deny > defer > ask > allow**。任一 hook 返回 `deny` 即阻止操作。

### 4.4 退出码行为

| 退出码 | 含义 |
|--------|------|
| **0** | 成功。stdout 被解析为 JSON 决策或纯文本 additionalContext |
| **2** | 阻止操作（仅支持阻止的事件）。stderr 发送给 Claude 作为可纠正反馈（非直接显示给用户），Claude 可据此自我纠正 |
| **其他非0** | 非阻止错误。stderr 第一行显示给用户；执行继续 |

---

## 5. Matcher 机制

Matcher 用于按事件子类型过滤 hook 是否触发。

### 5.1 匹配规则

| Matcher 值 | 行为 |
|-----------|------|
| `"*"`、`""` 或省略 | 匹配所有 |
| 仅含字母、数字、`_`、`-`、空格、`,`、`|` | **精确字符串匹配**（如 `"Write|Edit"` 匹配 Write 或 Edit） |
| 包含任何正则特字符 | **JavaScript 正则**，非锚定（如 `"mcp__memory__.*"`、`"^Notebook"`） |

### 5.2 事件对应的 matcher 语义

| 事件 | matcher 匹配目标 |
|------|-----------------|
| PreToolUse / PostToolUse / PostToolUseFailure | 工具名称（`Bash`、`Write`、`Edit`、`mcp__*` 等） |
| PermissionRequest / PermissionDenied | 工具名称 |
| Notification | `notification_type`（`permission_prompt`、`idle_prompt`、`auth_success`、`elicitation_dialog`、`elicitation_complete`、`elicitation_response`、`agent_needs_input`、`agent_completed`） |
| SessionStart | `source`（`startup`、`resume`、`clear`、`compact`） |
| PreCompact / PostCompact | 触发类型（`manual`、`auto`） |
| SubagentStart / SubagentStop | 子代理类型名称 |
| FileChanged | 文件名模式（如 `".envrc|.env"`） |
| ConfigChange | 配置来源 |
| UserPromptSubmit | **不支持 matcher**（每次提示词提交均触发） |
| CwdChanged | 不支持 matcher |

### 5.3 MCP 工具命名

MCP 工具遵循命名约定：`mcp__<server>__<tool>`

```json
{ "matcher": "mcp__memory__.*" }     // 匹配某 MCP 服务器的所有工具
{ "matcher": "mcp__.*" }             // 匹配所有 MCP 工具
{ "matcher": "mcp__github__create_issue" }  // 匹配特定 MCP 工具
```

### 5.4 `if` 条件过滤器（权限规则）

工具事件专用的额外过滤器：

```json
{
  "matcher": "Bash",
  "if": "Bash(git *)",
  "hooks": [...]
}
```

- `"Bash(git *)"` — 仅匹配以 `git ` 开头的 Bash 命令
- `"Edit(*.ts)"` — 仅匹配编辑 `.ts` 文件
- `if` 不支持正则，仅简单通配符

---

## 6. Hook 类型

共五种 handler 类型。

### 6.1 `command` — Shell 命令（默认）

```json
{
  "type": "command",
  "command": "python3 $CLAUDE_PROJECT_DIR/.claude/hooks/validate.py",
  "timeout": 60,
  "statusMessage": "正在验证..."
}
```

**exec 形式**（v2.1.139+，直接启动二进制）：

```json
{
  "type": "command",
  "args": ["python3", "$CLAUDE_PROJECT_DIR/.claude/hooks/validate.py", "--strict"],
  "timeout": 60
}
```

**异步执行**：

```json
{
  "type": "command",
  "command": "send-metrics.sh",
  "async": true
}
```

字段：
- `command`：Shell 命令字符串（通过 shell 执行；存在 `args` 时为可执行文件路径）
- `args`：参数数组（直接 exec，无 shell 解释）
- `timeout`：超时秒数（默认见下方）
- `shell`：`"bash"` 或 `"powershell"`（Windows 无 Git Bash 时回退 powershell；`args` 存在时忽略）
- `async`：异步后台运行（fire-and-forget，不阻塞触发动作）
- `asyncRewake`：后台运行且退出码 2 时唤醒 Claude，stderr（为空则 stdout）作为 system reminder 展示；隐含 `async`
- `statusMessage`：运行期间显示的自定义 spinner 文本

> 2026-07-31 修订：原记载的 `asyncTimeout`（异步超时毫秒）系误记——它是异步执行的**返回值字段**，不是 settings.json 配置字段；原示例同步修正。`allowedEnvVars` 为 `http` handler 专属，不属于 `command`。

### 6.2 `http` — HTTP Webhook（v2.1.63+）

```json
{
  "type": "http",
  "url": "https://my-webhook.example.com/hook",
  "headers": { "Authorization": "Bearer ${MY_TOKEN}" },
  "allowedEnvVars": ["MY_TOKEN"]
}
```

- 将事件 JSON 作为 POST body 发送到 URL
- 响应 body 使用相同的 JSON 输出格式
- 需要 `allowedEnvVars` 显式声明 URL/header 中的环境变量
- 非 2xx 响应为**非阻止错误**（需返回 2xx + JSON 才能阻止）

### 6.3 `mcp_tool` — MCP 工具调用（v2.1.118+）

```json
{
  "type": "mcp_tool",
  "server": "my-mcp-server",
  "tool": "validate_edit",
  "input": { "path": "${tool_input.file_path}" }
}
```

- 在已连接的 MCP 服务器上直接调用工具（无子进程）
- `${path}` 支持点号路径从 hook 输入 JSON 中插值
- 工具文本输出等同于 command hook 的 stdout

### 6.4 `prompt` — LLM 提示词评估

```json
{
  "type": "prompt",
  "prompt": "Evaluate if Claude completed all requested tasks.",
  "model": "claude-sonnet-4-20250514",
  "timeout": 30
}
```

- 单轮 Claude 评估调用
- `$ARGUMENTS` 占位符注入 hook 输入 JSON
- 模型返回结构化 JSON：`{ "ok": true }` 允许，`{ "ok": false, "reason": "..." }` 阻止
- `model` 默认使用快速模型（Haiku）
- `continueOnBlock`（可选）：返回 `ok: false` 时把 reason 反馈给 Claude 并继续当前 turn，而非停止【2026-07-31 补】

### 6.5 `agent` — 子代理验证（多轮子代理，已正式支持）

```json
{
  "type": "agent",
  "prompt": "Verify the code changes follow our architecture guidelines.",
  "timeout": 120
}
```

- 生成**多轮子代理**，拥有工具访问权限（Read、Grep、Glob、Bash）
- 用于复杂代码验证
- 已从实验性升级为正式支持的 handler 类型
- 可选 `model`（默认快速模型）；**无 `description`/`subagent_type` 字段**——后两者是内置 Agent 工具的输入参数，非 hook handler 字段【2026-07-31 官方核实】

### 6.6 通用字段

| 字段 | 说明 |
|------|------|
| `type` | 必需。`"command"` / `"http"` / `"mcp_tool"` / `"prompt"` / `"agent"` |
| `if` | 权限规则过滤器，仅工具事件 |
| `timeout` | 超时秒数 |
| `statusMessage` | 自定义 spinner 文本 |
| `once` | `true` 时每会话仅执行一次后移除（仅 skills） |

### 6.7 默认超时时间

| 事件/类型 | 默认超时 |
|----------|---------|
| command/http/mcp_tool（大部分事件） | **600 秒** |
| command/http/mcp_tool（UserPromptSubmit） | **30 秒** |
| command/http/mcp_tool（MessageDisplay） | **10 秒** |
| prompt | **30 秒** |
| agent | **60 秒** |
| SessionEnd | **1.5 秒**（可通过 `CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS` 覆盖） |

---

## 7. 配置格式（settings.json）

### 7.1 基本结构（三层嵌套 JSON）

```json
{
  "hooks": {
    "EventName": [
      {
        "matcher": "ToolPattern",
        "hooks": [
          {
            "type": "command",
            "command": "your-command-here",
            "timeout": 60
          }
        ]
      }
    ]
  }
}
```

三层嵌套：
1. **事件名** — 生命周期节点（`PreToolUse`、`Stop` 等）
2. **Matcher 组** — 过滤何时触发
3. **Hook handler** — 实际执行的命令/http/prompt/agent

### 7.2 配置位置（优先级从低到高）

| 文件 | 作用域 | 是否可分享 |
|------|--------|-----------|
| `~/.claude/settings.json` | 用户级（所有项目） | 否，本地仅自己 |
| `.claude/settings.json` | 单项目 | **是，可提交** |
| `.claude/settings.local.json` | 单项目（个人） | 否，应 gitignore |
| 托管策略设置 | 组织级 | 管理员控制 |
| 插件 `hooks/hooks.json` | 插件启用时 | 随插件打包 |

### 7.3 环境变量

| 变量 | 可用范围 | 说明 |
|------|---------|------|
| `CLAUDE_PROJECT_DIR` | 所有 hook | 项目根路径 |
| `CLAUDE_PLUGIN_ROOT` | 插件 hook | 插件安装目录 |
| `CLAUDE_PLUGIN_DATA` | 插件 hook | 插件数据目录 |
| `CLAUDE_ENV_FILE` | SessionStart, CwdChanged, FileChanged | 文件路径：写入 `export KEY=VALUE` 行以持久化环境变量 |
| `CLAUDE_CODE_REMOTE` | 所有 hook | 远程 Web 环境中为 `"true"` |
| `CLAUDE_CODE_BRIDGE_SESSION_ID` | 所有 hook（v2.1.199+） | Remote Control 会话 ID |

---

## 8. UI/视觉反馈能力

### 8.1 Notification Hook + 桌面通知

Claude Code 通过 `Notification` hook 支持多种桌面通知方案：

**Notification matcher 值**（共 8 个）：

| matcher | 触发时机 | 延迟 |
|---------|---------|------|
| `permission_prompt` | Claude 需要权限运行工具 | 立即 |
| `idle_prompt` | Claude 完成等待用户输入 | ~60 秒 |
| `auth_success` | 认证成功 | 立即 |
| `elicitation_dialog` | MCP 弹出对话框 | 立即 |
| `elicitation_complete` | MCP 弹出表单提交/关闭 | 立即 |
| `elicitation_response` | MCP 弹出响应发回 server | 立即 |
| `agent_needs_input` | 后台 session 等待用户输入（v2.1.198+） | 不定 |
| `agent_completed` | 后台 session 完成或失败（v2.1.198+） | 不定 |

**Windows 桌面通知示例**：

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "idle_prompt",
        "hooks": [{
          "type": "command",
          "command": "pwsh -Command \"Import-Module BurntToast; New-BurntToastNotification -Text 'Claude Code', '等待输入' -Sound Default\""
        }]
      },
      {
        "matcher": "permission_prompt",
        "hooks": [{
          "type": "command",
          "command": "pwsh -Command \"Import-Module BurntToast; New-BurntToastNotification -Text 'Claude Code', '需要授权' -Sound Default\""
        }]
      }
    ]
  }
}
```

**macOS 通知示例**：

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [{
          "type": "command",
          "command": "osascript -e 'display notification \"Claude Code needs your attention\" with title \"Claude Code\"'"
        }]
      }
    ]
  }
}
```

**Linux 通知示例**：

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "idle_prompt",
        "hooks": [{
          "type": "command",
          "command": "notify-send 'Claude Code' 'Ready for input' && paplay /usr/share/sounds/freedesktop/stereo/complete.oga"
        }]
      }
    ]
  }
}
```

### 8.2 三 Hook 组合策略（推荐）

为获得最佳用户体验，建议组合三个 hook：

1. **`Stop` hook** — 立即感知 Claude 完成（无 60 秒延迟）
2. **`Notification` + `permission_prompt`** — 权限弹窗即时通知
3. **`Notification` + `idle_prompt`** — 空闲兜底（~60 秒）

### 8.3 statusMessage

Hook 运行期间可在 CLI 显示自定义 spinner 文本：

```json
{
  "type": "command",
  "command": "npx prettier --write ${tool_input.file_path}",
  "statusMessage": "正在格式化代码..."
}
```

### 8.4 systemMessage

hook 输出的 `systemMessage` 字段可直接向用户显示消息：

```json
{
  "systemMessage": "已自动格式化 src/index.ts"
}
```

### 8.5 suppressOutput

隐藏 hook 的 stdout/stderr 不向用户显示：

```json
{
  "suppressOutput": true
}
```

### 8.6 已知限制

- **无 `AskUserQuestion` 通知**：交互式中途提问不触发任何 hook（[GitHub issue #13830](https://github.com/anthropics/claude-code/issues/13830)）
- **Plan approval 无 hook**："Accept this plan?" 提示不触发 hook（[GitHub issue #19283](https://github.com/anthropics/claude-code/issues/19283)）
- **VS Code 扩展限制**：Notification hook（`permission_prompt`、`idle_prompt`）在 VS Code 扩展中不触发，仅 `Stop` hook 可工作（[GitHub issue #28774](https://github.com/anthropics/claude-code/issues/28774)）
- **`idle_prompt` 延迟**：约 60 秒空闲后才触发（[GitHub issue #32634](https://github.com/anthropics/claude-code/issues/32634)）

---

## 9. Agent SDK Hooks

来源：https://code.claude.com/docs/en/agent-sdk/hooks

### 9.1 与 CLI Hooks 的差异

Agent SDK（TypeScript：`@anthropic-ai/claude-agent-sdk`，Python：`claude_agent_sdk`）支持内联回调函数形式的 hook，与 CLI 的 JSON 配置文件形式有差异。

### 9.2 TypeScript SDK 回调示例

```typescript
import { query, HookCallback, PreToolUseHookInput } from "@anthropic-ai/claude-agent-sdk";

const protectEnvFiles: HookCallback = async (input, toolUseID, { signal }) => {
  const preInput = input as PreToolUseHookInput;
  const toolInput = preInput.tool_input as Record<string, unknown>;
  const filePath = toolInput?.file_path as string;

  if (filePath?.endsWith(".env")) {
    return {
      hookSpecificOutput: {
        hookEventName: preInput.hook_event_name,
        permissionDecision: "deny",
        permissionDecisionReason: "Cannot modify .env files",
      }
    };
  }
  return {}; // 允许
};

for await (const msg of query({
  prompt: "Update DB config",
  options: {
    hooks: {
      PreToolUse: [{ matcher: "Write|Edit", hooks: [protectEnvFiles] }]
    }
  }
})) {
  console.log(msg);
}
```

**回调签名**：`(input, toolUseID, context) => HookJSONOutput`
- `input`：类型化 hook 输入（如 `PreToolUseHookInput`）
- `toolUseID`：`string | undefined`，关联 PreToolUse / PostToolUse
- `context`：含 `signal`（`AbortSignal`）用于取消

### 9.3 Python SDK 回调示例

```python
from claude_agent_sdk import (
    ClaudeSDKClient, ClaudeAgentOptions, HookMatcher,
    PreToolUseHookInput,
)

async def protect_env_files(input_data, tool_use_id, context):
    file_path = input_data["tool_input"].get("file_path", "")
    if file_path.endswith(".env"):
        return {
            "hookSpecificOutput": {
                "hookEventName": input_data["hook_event_name"],
                "permissionDecision": "deny",
                "permissionDecisionReason": "Cannot modify .env files",
            }
        }
    return {}

options = ClaudeAgentOptions(
    hooks={
        "PreToolUse": [HookMatcher(matcher="Write|Edit", hooks=[protect_env_files])]
    }
)
```

**Python 特有**：
- `async_`（替代 `async`，避免 Python 关键字冲突）
- `continue_`（替代 `continue`）
- 2025 年更新：`PreToolUseHookInput`、`PostToolUseHookInput` 等 TypedDict 提供完整类型提示
- `include_hook_events=True` 将 hook 生命周期事件作为 `HookEventMessage` 对象发入消息流

### 9.4 SDK 事件可用性差异

**仅在 TypeScript/CLI 中可用的额外事件**（Python SDK 暂不支持）：
- `PostToolBatch`、`MessageDisplay`、`SessionStart`、`SessionEnd`、`Setup`
- `TeammateIdle`、`TaskCompleted`、`ConfigChange`、`WorktreeCreate`、`WorktreeRemove`

**Python SDK 核心事件**（支持）：
- `PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`UserPromptSubmit`
- `Stop`、`SubagentStart`、`SubagentStop`、`PreCompact`、`PermissionRequest`、`Notification`

---

## 10. 完整配置示例

### 10.1 基本配置

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "git status --short && echo '---' && cat TODO.md"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 $CLAUDE_PROJECT_DIR/.claude/hooks/validate-bash.py",
            "timeout": 30
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "jq -r '.tool_input.file_path' | xargs npx prettier --write",
            "timeout": 15,
            "statusMessage": "格式化中..."
          }
        ]
      }
    ]
  }
}
```

### 10.2 阻止危险命令

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 -c \"\nimport json, sys\ndata = json.load(sys.stdin)\ncmd = data['tool_input'].get('command', '')\nif 'rm -rf /' in cmd or 'DROP TABLE' in cmd:\n    print(json.dumps({'hookSpecificOutput': {'hookEventName': 'PreToolUse', 'permissionDecision': 'deny', 'permissionDecisionReason': '危险命令已阻止'}}))\n    sys.exit(0)\n\"",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

### 10.3 自动注入项目上下文

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "echo '{\"additionalContext\": \"Current branch: '$(git branch --show-current)'. Last commit: '$(git log -1 --oneline)'.\"}'"
          }
        ]
      }
    ]
  }
}
```

### 10.4 Stop Hook 强制继续

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "cat <<'EOF'\n{\n  \"decision\": \"block\",\n  \"reason\": \"请确保所有测试通过后再结束。运行 npm test 验证。\"\n}\nEOF"
          }
        ]
      }
    ]
  }
}
```

---

## 11. 最佳实践与注意事项

1. **修改配置后热重载**：部分 hooks 配置支持会话中热重载（ConfigChange hook 可检测 settings.json 变更），但通过插件 `hooks.json` 注册的 hooks 仅启动时加载。完全重启以确保所有配置生效。
2. **stdin 是 JSON**：所有 hook 输入通过 stdin 传入，需在脚本中 `json.load(sys.stdin)` 解析。
3. **stdout 优先 JSON**：输出 JSON 格式决策可获得最完整的控制能力；纯文本被作为 additionalContext 注入。
4. **permissionDecision 优于 decision**：`permissionDecision` 是新格式，`decision: "block"` 已逐步废弃。
5. **hookSpecificOutput 不可省略**：PreToolUse、PostToolUse、SessionStart 的决策必须在 `hookSpecificOutput` 内，裸顶层字段会被忽略。
6. **updatedInput 结构差异**：PreToolUse 中 `updatedInput` 是 `hookSpecificOutput` 的直接子字段；PermissionRequest 中嵌套在 `hookSpecificOutput.decision.updatedInput` 内。
7. **Stop hook 防无限循环**：始终检查输入的 `stop_hook_active` 字段。
8. **测试 hook**：先用 shell 脚本 + `echo '...' | my-hook.sh` 手动测试 JSON 输入输出。
9. **`/hooks` 命令**：在 Claude Code 中输入 `/hooks` 打开只读 hook 浏览器，查看所有可用事件和已配置 hook 数量。

---

## 12. 来源汇总

| 内容 | 来源 |
|------|------|
| 30 个 hook 事件完整列表及字段 | https://code.claude.com/docs/en/hooks |
| 中文版文档 | https://code.claude.com/docs/zh-CN/hooks |
| 配置格式与五种类型 | https://code.claude.com/docs/en/hooks |
| 输入/输出 JSON 结构 | 官方文档 + 社区验证 (GitHub issues, skills repositories) |
| Agent SDK hooks | https://code.claude.com/docs/en/agent-sdk/hooks |
| 桌面通知配置 | 社区实践验证 (martin.hjartmyr.se, madflex.de, npm @dagim_s/claude-code-notification) |
| 已知问题/限制 | GitHub issues: #13830, #19283, #28774, #32634, #19124, #15664 |
| Hook 配置指南 | https://code.claude.com/docs/en/hooks-guide |
| 权限决策优先级 | Agent SDK 文档 (anthropics/claude-agent-sdk-python) |
