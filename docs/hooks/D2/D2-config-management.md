# D2 -- Claude Code Hooks 配置管理 UI 研究报告

> 研究日期：2026-07-25
> 数据来源：Claude Code 官方文档（Context7 + code.claude.com）、Agent SDK 文档、GitHub Issues、SchemaStore、VS Code 官方文档、JetBrains 官方文档

---

## 目录

1. [Hooks 配置完整 Schema](#1-hooks-配置完整-schema)
2. [30 种 Hook 事件类型全表](#2-30-种-hook-事件类型全表)
3. [5 种 Hook Handler 类型及字段](#3-5-种-hook-handler-类型及字段)
4. [Matcher 正则语法与 `if` 细粒度过滤](#4-matcher-正则语法与-if-细粒度过滤)
5. [stdin JSON 输入与 stdout JSON 输出](#5-stdin-json-输入与-stdout-json-输出)
6. [环境变量与退出码语义](#6-环境变量与退出码语义)
7. [阻止（Blocking）机制总结](#7-阻止blocking机制总结)
8. [超时默认值与执行模型](#8-超时默认值与执行模型)
9. [settings.json JSON Schema 校验](#9-settingsjson-json-schema-校验)
10. [VS Code 配置 UI 参考](#10-vs-code-配置-ui-参考)
11. [JetBrains 配置 UI 参考](#11-jetbrains-配置-ui-参考)
12. [社区配置案例与模式](#12-社区配置案例与模式)
13. [已知 Bug 与限制](#13-已知-bug-与限制)
14. [对 slTerminal hooks 配置 UI 的启示](#14-对-slterminal-hooks-配置-ui-的启示)

---

## 1. Hooks 配置完整 Schema

### 1.1 三层嵌套结构

Hooks 在 settings.json 中采用**三级嵌套结构**：

```
HookEvent (事件) → MatcherGroup[] (matcher + hooks[]) → HookHandler[] (type + 执行参数)
```

```jsonc
{
  "hooks": {
    "<HookEvent>": [           // 第1层：hook 事件类型（30种）
      {
        "matcher": "regex",    // 第2层：matcher 组
        "hooks": [             // 第3层：hook handler 数组
          {
            "type": "command|http|mcp_tool|prompt|agent",
            // ... handler 特定字段
          }
        ]
      }
    ]
  }
}
```

> 来源：https://code.claude.com/docs/en/hooks

### 1.2 配置文件层级

优先级从高到低为: 托管策略 > local > project > user。本地设置覆盖项目设置，项目设置覆盖用户设置。托管策略始终最高优先级。

| 优先级 | 文件 | 作用域 | 共享 |
|--------|------|--------|------|
| 最高 | 托管策略（企业） | 全局强制 | -- |
| 最高（非托管） | `.claude/settings.local.json` | 单项目本地 | 应 gitignore |
| 中 | `.claude/settings.json` | 单项目 | 可提交到仓库 |
| 最低 | `~/.claude/settings.json` | 所有项目 | 否 |

### 1.3 全局禁用

```jsonc
{ "disableAllHooks": true }                          // 禁用全部
{ "disableStopHook": true }                          // 仅禁用 Stop
{ "disableNotificationHook": true }                  // 仅禁用 Notification
```

> 来源：https://code.claude.com/docs/en/hooks

---

## 2. 30 种 Hook 事件类型全表

### 2.1 会话生命周期

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `SessionStart` | 会话启动/恢复 | 否 | `startup`/`resume`/`clear`/`compact` |
| `SessionEnd` | 会话终止 | 否 | 退出原因 |
| `Setup` | `--init-only` 或 `--maintenance` + `-p` | 否 | -- |

### 2.2 用户交互

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `UserPromptSubmit` | 用户提交 prompt，Claude 处理前 | **是** | -- |
| `UserPromptExpansion` | slash command / MCP prompt 展开后 | **是** | -- |

### 2.3 工具调用（核心--最常用）

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `PreToolUse` | 工具参数构建后、执行前 | **是** | 工具名/正则 |
| `PermissionRequest` | 权限对话框弹出前 | **是** | 工具名/正则 |
| `PermissionDenied` | auto mode 分类器拒绝工具 | 否 | -- |
| `PostToolUse` | 工具调用成功后 | 否* | 工具名/正则 |
| `PostToolUseFailure` | 工具调用失败后 | 否 | 工具名/正则 |
| `PostToolBatch` | 并行工具批次全部解析后 | **是** | -- |

> *PostToolUse 不能阻止执行，但可通过 `decision: "block"` 反馈给 Claude

### 2.4 通知与消息

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `Notification` | Claude Code 发送通知 | 否 | 通知类型 |
| `MessageDisplay` | 助理消息流式输出 | 否 | -- |

**Notification matcher 值**：`permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`, `elicitation_complete`, `elicitation_response`

### 2.5 子代理与任务

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `SubagentStart` | 子代理 spawn | 否 | 代理类型名 |
| `SubagentStop` | 子代理完成 | **是** | 代理类型名 |
| `TaskCreated` | TaskCreate 创建任务 | **是** | -- |
| `TaskCompleted` | 任务标记完成 | **是** | -- |
| `TeammateIdle` | agent team 队友空闲 | **是** | -- |

### 2.6 上下文管理

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `PreCompact` | 上下文压缩前 | **是** (v2.1.105+) | `manual`/`auto` |
| `PostCompact` | 上下文压缩后 | 否 | `manual`/`auto` |

### 2.7 停止与错误

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `Stop` | Claude 完成响应 | **是** | -- |
| `StopFailure` | 回合因 API 错误结束 | 否 | 错误类型 |

### 2.8 配置与文件变更

| 事件 | 触发时机 | 可阻止 | matcher |
|------|---------|:---:|---------|
| `ConfigChange` | 配置文件变更 | **是** | -- |
| `CwdChanged` | `cd` 命令 | 否 | -- |
| `FileChanged` | 被监视文件磁盘变更 | 否 | 文件名 |
| `InstructionsLoaded` | CLAUDE.md / `.claude/rules/*.md` 加载 | 否 | -- |

### 2.9 工作树

| 事件 | 触发时机 | 可阻止 |
|------|---------|:---:|
| `WorktreeCreate` | git worktree 创建 | **是** |
| `WorktreeRemove` | worktree 删除 | 否 |

### 2.10 启发式交互

| 事件 | 触发时机 | 可阻止 |
|------|---------|:---:|
| `Elicitation` | MCP 服务器请求用户输入 | **是** |
| `ElicitationResult` | 用户响应后 | **是** |

> 来源：https://code.claude.com/docs/en/hooks、https://www.morphllm.com/claude-code-hooks
> 子代理详细文档：`01-hooks-official-docs.md` 第 7 节

---

## 3. 5 种 Hook Handler 类型及字段

### 3.1 类型总览

| 类型 | 说明 | 适用事件 | 社区使用频率 |
|------|------|---------|:----------:|
| `command` | 执行 shell 命令，stdin 接收 JSON 输入 | 全部 | >90% |
| `prompt` | LLM 单轮评估 | PreToolUse, PostToolUse, PermissionRequest 等 | ~5% |
| `agent` | spawn 子代理验证 | 同 prompt | <2% |
| `http` | POST JSON 到 URL | 全部 | <3% |
| `mcp_tool` | 调用 MCP 工具 | 全部 | <1% |

> 来源：https://code.claude.com/docs/en/hooks
> 子代理详细文档：`05-community-hooks-examples.md` 第 5 节

### 3.2 Command Hook

```jsonc
{
  "type": "command",
  "command": "string",        // 必需。shell 命令或可执行文件路径
  "args": ["string"],         // 可选。参数数组（直接执行，绕过 shell）
  "async": false,             // 可选。默认 false。true 时后台执行
  "asyncRewake": false,       // 可选。async 完成后重新唤醒 agent
  "shell": "string",          // 可选。指定 shell 解释器
  "timeout": 60,              // 可选。超时秒数，覆盖默认
  "if": "condition",          // 可选。v2.1.85+ 细粒度参数过滤
  "allowedEnvVars": ["VAR1"]  // 可选。环境变量白名单
}
```

### 3.3 HTTP Hook

```jsonc
{
  "type": "http",
  "url": "string",                        // 必需。POST URL
  "method": "POST",                       // 可选。默认 POST
  "headers": { "Authorization": "..." },  // 可选。HTTP 头
  "body": "json_string",                  // 可选。请求体
  "timeout": 30,                          // 可选。超时秒数
  "allowedEnvVars": ["MY_TOKEN"]          // 可选。环境变量白名单
}
```

### 3.4 MCP Tool Hook

```jsonc
{
  "type": "mcp_tool",
  "server": "string",   // MCP 服务器名称
  "tool": "string",     // 工具名称
  "args": {},           // 可选。JSON 参数
  "timeout": 30         // 可选。超时秒数
}
```

### 3.5 Prompt Hook（LLM 单轮评估）

```jsonc
{
  "type": "prompt",
  "prompt": "string",   // 必需。LLM 评估提示词
  "timeout": 30         // 可选。默认 30s
}
```

**不适用于**：Notification, SessionEnd, ConfigChange, FileChanged, CwdChanged 等非对话上下文事件。

### 3.6 Agent Hook（子代理 spawn）

```jsonc
{
  "type": "agent",
  "prompt": "string",         // 必需。子代理任务 prompt
  "description": "string",    // 可选。短描述（3-5 词）
  "subagent_type": "string",  // 可选。子代理类型
  "model": "sonnet|opus|haiku|fable",  // 可选
  "timeout": 60               // 可选。默认 60s
}
```

> 来源：https://code.claude.com/docs/en/hooks（Hook handler fields）
> 子代理详细文档：`01-hooks-official-docs.md` 第 3 节

---

## 4. Matcher 正则语法与 `if` 细粒度过滤

### 4.1 Matcher 基本规则

| Pattern | 行为 | 示例 |
|---------|------|------|
| 仅含字母/数字/`_`/`-`/空格/`|`/`,` | **精确字符串匹配**，`|` 和 `,` 为或 | `Edit|Write` 匹配 Edit 或 Write |
| 包含其他字符 | **JavaScript 正则**（非锚定） | `^mcp__` 匹配所有 MCP 工具 |
| `"*"`, `""`, 或省略 | **匹配全部** | 通配所有工具/事件 |

**注意**：
- 区分大小写--`"bash"` 不匹配 `Bash` 工具
- 连字符名称（如 `code-reviewer`）v2.1.195+ 视为精确匹配，旧版需 `^code-reviewer$`
- MCP 工具命名：`mcp__<服务器名>__<工具名>`
- **FileChanged 和 StopFailure 的 matcher 字符集更窄**：仅接受字母、数字、`_`、`|`。对于这两个事件，连字符(`-`)、空格、逗号会强制走正则表达式路径，且只有 `|` 作为分隔符。

> 来源：https://code.claude.com/docs/en/hooks、https://code.claude.com/docs/en/whats-new/2026-w27
> 子代理详细文档：`01-hooks-official-docs.md` 第 8 节

### 4.2 常见工具名（用于 matcher）

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

### 4.3 `if` 字段 -- 细粒度条件过滤（v2.1.85+）

`if` 字段在 **matcher 之后**、进程 spawn 之前执行，按**工具名+参数**进行更细粒度的匹配。**不命中的 hook 零开销**。

```jsonc
{
  "matcher": "Bash",
  "hooks": [
    {
      "type": "command",
      "if": "Bash(git push *)",     // 仅拦截 git push 命令
      "command": ".../hooks/block-force-push.sh"
    },
    {
      "type": "command",
      "if": "Bash(rm *)",           // 仅拦截 rm 命令
      "command": ".../hooks/block-rm.sh"
    }
  ]
}
```

**v2.1.88 改进**：解析式子命令匹配
- 剥离 `VAR=value` 前缀
- 复合命令（`&&`/`||`）拆分子命令
- `ls && git push` → git 子命令被解析 → 匹配 `"Bash(git *)"`

**文件工具的路径 `if`**：
- `"Edit(src/**)"` -- src 目录下所有文件
- `"Read(.env)"` -- .env 文件读取
- `"Write(*.ts)"` -- 所有 .ts 文件写入

> 来源：https://github.com/anthropics/claude-code/issues/41262
> 子代理详细文档：`01-hooks-official-docs.md` 第 9 节、`05-community-hooks-examples.md` 第 8 节

---

## 5. stdin JSON 输入与 stdout JSON 输出

### 5.1 stdin JSON 输入（Command Hook 接收）

**所有事件通用字段（BaseHookInput）**：

```jsonc
{
  "session_id": "abc123",
  "transcript_path": "/home/user/.claude/projects/.../transcript.jsonl",
  "cwd": "/home/user/my-project",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "agent_id": "...",       // 仅子代理
  "agent_type": "...",     // 仅子代理
  "effort": { "level": "high" }
}
```

**PreToolUse 额外字段**：
```jsonc
{
  "tool_name": "Bash",
  "tool_input": { "command": "npm test" },
  "tool_use_id": "toolu_01..."
}
```

**各事件特有字段**：
- UserPromptSubmit: `prompt` -- 用户提交的文本
- SessionStart: `source` (`startup`/`resume`/`clear`/`compact`), `model`
- PreCompact: `trigger` (`manual`/`auto`), `custom_instructions`
- Stop: `stop_hook_active` (防无限循环关键字段), `last_assistant_message`
- StopFailure: `stop_failure_reason`, `api_status_code`
- Notification: `notification_type`, `message`, `title`
- SubagentStart/Stop: `subagent_type`, `agent_id`, `duration_ms` (Stop)

> 子代理详细文档：`01-hooks-official-docs.md` 第 6 节

### 5.2 stdout JSON 输出（Hook 返回给 Claude Code）

**仅解析 stdout 的第一行 JSON。**

顶层字段：
- `continue`: boolean -- 是否允许继续
- `systemMessage`: string -- 向用户终端显示的消息
- `stopReason`: string -- 阻止原因
- `suppressOutput`: boolean -- 是否隐藏输出
- `decision`: string -- `"block"` + `"reason"` 阻止指令
- `hookSpecificOutput`: object -- 事件特定输出

**PreToolUse 输出**（可阻止/修改）：
```jsonc
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "理由",
    "updatedInput": { "command": "npm test -- --coverage" }  // 修改工具输入
  }
}
```

**Stop 输出**（防无限循环）：
```jsonc
{
  "decision": "block",
  "reason": "测试未通过，请修复"
}
```
> **关键**：Stop hook 必须检查 `stop_hook_active: true` 时直接 exit 0，防止无限循环。

**SessionStart 输出**（注入上下文）：
```jsonc
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "项目使用 pnpm，Node 版本：20.x"
  }
}
```

> 子代理详细文档：`01-hooks-official-docs.md` 第 10 节

---

## 6. 环境变量与退出码语义

### 6.1 可用环境变量

| 变量 | 说明 | 范围 |
|------|------|------|
| `CLAUDE_PROJECT_DIR` | 项目根目录绝对路径 | 所有 hooks |
| `CLAUDE_CODE_REMOTE` | 远程/Web 环境为 `"true"` | 所有 hooks |
| `CLAUDE_ENV_FILE` | 持久化环境变量的文件（`export FOO=BAR >> $CLAUDE_ENV_FILE`） | **仅** SessionStart, Setup, CwdChanged, FileChanged |
| `CLAUDE_PLUGIN_ROOT` | 插件目录绝对路径 | 仅插件 hooks |
| `CLAUDE_CODE_BRIDGE_SESSION_ID` | Remote Control 会话 ID (v2.1.199+) | Remote Control |
| `HOME`, `PATH`, `USER` 等 | 标准环境变量 | 所有 hooks |

> **关键注意**：`CLAUDE_TOOL_INPUT` 和 `CLAUDE_TOOL_RESPONSE` 环境变量**不存在**--工具输入/输出通过 **stdin JSON** 传入，不是环境变量。

> 来源：https://code.claude.com/docs/en/hooks
> 子代理详细文档：`01-hooks-official-docs.md` 第 5 节

### 6.2 退出码语义

| 退出码 | 含义 | 行为 |
|:------:|------|------|
| **0** | 成功 | 允许操作继续；stdout JSON 被解析处理 |
| **1** | 非阻塞错误 | stderr 作为警告显示，操作继续 |
| **2** | **阻塞错误** | **阻止操作**；stderr 反馈给 Claude |
| 其他 | 同退出码 1 | 非阻塞错误 |

> **关键**：仅退出码 2 阻止操作。退出码 1 是非阻塞警告，操作照常执行。

> 来源：https://code.claude.com/docs/en/hooks

---

## 7. 阻止（Blocking）机制总结

### 7.1 可阻止的事件

| 事件 | 阻止方式 | 效果 |
|------|---------|------|
| `PreToolUse` | exit 2 / `permissionDecision: "deny"` | 取消工具调用 |
| `UserPromptSubmit` | exit 2 / `decision: "block"` | **擦除**用户 prompt |
| `Stop` | `decision: "block"` | 强制 Claude 继续 |
| `SubagentStop` | `decision: "block"` | 强制子代理继续 |
| `PostToolUse` | `decision: "block"` | 提示 Claude 处理问题 |
| `PreCompact` | exit 2 / `continue: false` | 阻止压缩 |
| `TaskCreated` | exit 2 | 回滚任务创建 |
| `TaskCompleted` | exit 2 / `continue: false` | 阻止任务完成 |
| `PermissionRequest` | `permissionDecision: "deny"` | 拒绝工具执行 |

### 7.2 不可阻止的事件

`SessionStart`, `PostToolUseFailure`, `Notification`, `PostCompact`, `SubagentStart`, `CwdChanged`, `FileChanged`, `InstructionsLoaded`, `WorktreeRemove`, `StopFailure`, `SessionEnd`, `Setup`, `PermissionDenied`, `MessageDisplay` -- 仅可观察/附加副作用。

> 子代理详细文档：`01-hooks-official-docs.md` 第 11 节

---

## 8. 超时默认值与执行模型

### 8.1 默认超时

| Hook 类型 | 默认超时 | 备注 |
|-----------|---------|------|
| command | 600s (10min) | UserPromptSubmit 降为 30s，MessageDisplay 降为 10s |
| http | 600s (10min) | 同上 |
| mcp_tool | 600s (10min) | 同上 |
| prompt | 30s | -- |
| agent | 60s | -- |

可逐 hook 通过 `timeout` 字段覆盖。

> 来源：https://code.claude.com/docs/en/hooks-guide

### 8.2 执行模型

- **并行执行**：同一 matcher 组内多个 handler 并行执行
- **自动去重**：完全相同 handler（type + 参数均相同）自动去重，防止多层级 settings 合并时重复执行
- **匹配顺序**：按 matcher 组注册顺序依次匹配

---

## 9. settings.json JSON Schema 校验

### 9.1 官方 JSON Schema（已存在）

Claude Code 已有官方 JSON Schema，托管于 **SchemaStore**：

**`https://json.schemastore.org/claude-code-settings.json`**

由 Anthropic 直接维护，覆盖所有已文档化的 settings 字段：
- `permissions`（allow/deny/defaultMode/additionalDirectories）
- `hooks`（全部 ~30 种事件类型 + matcher + handler 字段）
- `sandbox`, `mcpServers`, `enabledPlugins`, `env`, `model`, `teammateMode`
- `autoMode`, `parentSettingsBehavior`, `disableBundledSkills`, `enforceAvailableModels`

> 来源：https://github.com/anthropics/claude-code/issues/2783
> 子代理详细文档：`02-settings-json-schema.md` 第 1-2 节

### 9.2 `$schema` 字段支持（2025-09 已修复）

在 settings.json 顶部加一行即可启用 IDE 补全和校验：

```jsonc
{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "hooks": { ... }
}
```

早期版本（约 2025-07 之前）会拒绝 `$schema` 字段，**2025-09 已修复**。

### 9.3 VS Code 集成（三种方式）

| 方式 | 复杂度 | 适用场景 |
|------|--------|---------|
| `$schema` 内联 | 最低 | 大多数用户，加一行即可 |
| [Claude Code Settings Helper 扩展](https://marketplace.visualstudio.com/items?itemName=jpegg-dev.claude-code-settings-helper) | 低 | 功能最强（IntelliSense + 校验 + 覆盖可视化） |
| 手动 `json.schemas` 映射 | 中 | jsonc 文件、特殊路径 |

> 子代理详细文档：`02-settings-json-schema.md` 第 3 节

### 9.4 第三方校验工具

| 工具 | 说明 |
|------|------|
| `npx claude-config-doctor` | Schema 校验 + OpenRouter 配置检查 + CI/CD 支持 |
| `npx @carlrannaberg/cclint` | JSON 语法 + schema 校验，覆盖 hooks 结构 |
| `agnix_core` (Rust) | 程序化校验，CC-SET 规则集 |

> 子代理详细文档：`02-settings-json-schema.md` 第 7 节

### 9.5 已知 schema 相关问题

1. `enabledPlugins` + 未安装插件 = schema 校验崩溃
2. Managed settings 无声失效--类型错误导致整个文档静默丢弃
3. 官方文档缺少 Schema 链接（已有 Issue 请求添加）
4. Schema 同步滞后--新字段上线后 SchemaStore 可能不同步

> 子代理详细文档：`02-settings-json-schema.md` 第 5 节

---

## 10. VS Code 配置 UI 参考

### 10.1 总体设计理念

VS Code **没有为 tasks.json/launch.json 提供完整图形化编辑器**。用户直接编辑 JSON 文件，通过以下机制降低门槛：

| 机制 | 说明 |
|------|------|
| **JSON Schema 驱动 IntelliSense** | `$schema` 声明 → 自动补全、悬停文档、错误波浪线 |
| **模板生成** | 选择调试器类型 → 自动生成带注释的模板 JSON |
| **下拉选择器** | `configurations` 以 dropdown 切换 |
| **Command Palette 引导** | `Tasks: Configure Task` 交互式选择 |
| **自动检测** | 扫描 npm scripts/gulpfile/tsconfig 自动生成 task |

> 来源：https://code.visualstudio.com/docs/editor/tasks
> 子代理详细文档：`03-vscode-config-ui-reference.md` 第 1 节

### 10.2 tasks.json 关键字段（对 slTerminal 有参考价值）

- **`inputs` 字段**：用户输入变量（`pickString` 下拉、`promptString` 文本、`command` 执行）
- **`presentation`**：控制终端行为（reveal/echo/focus/panel/clear）
- **`dependsOn`**：任务依赖链
- **`problemMatcher`**：解析输出为问题列表
- **RunOptions**：自动运行时机（`folderOpen`/`defaultBuildTask`）

> 子代理详细文档：`03-vscode-config-ui-reference.md` 第 2 节

### 10.3 Custom Editor API（扩展性参考）

VS Code 提供 `CustomTextEditorProvider` + `CustomEditorProvider`，支持为 JSON 配置构建**自定义 WebView 编辑器**：
- 使用 `jsonc-parser` 的 `modify()` 保留 Undo 历史
- WebView 与扩展通信通过 `postMessage`

> 子代理详细文档：`03-vscode-config-ui-reference.md` 第 4 节

### 10.4 VS Code Settings Editor（双模式典范）

VS Code settings 提供**双模式编辑**：
1. GUI 表单（搜索、过滤、下拉、复选框）
2. JSON 编辑器（`$schema` + 文档注释）

这是最适合 slTerminal hooks 配置 UI 参考的模式。

> 子代理详细文档：`03-vscode-config-ui-reference.md` 第 7 节

---

## 11. JetBrains 配置 UI 参考

### 11.1 External Tools 配置界面

**主列表页（Master 视图）**：
- 按 Group 分组列表，checkbox 控制启用/禁用
- 工具栏：`+`(添加) / `-`(删除) / 编辑 / 排序 / 复制
- 排序决定菜单顺序

**编辑对话框（Detail 视图）**：
- 分组表单布局（基本信息 → 工具设置 → 高级选项）
- 每个字段左对齐 label + 右侧控件
- Program 字段含文件浏览按钮 + 插入宏按钮

### 11.2 File Watchers 配置界面

- 模板选择（下拉选择预设如 Prettier/ESLint → 自动填充表单）
- **渐进式披露**：基本字段展开，高级选项（工作目录、环境变量）默认折叠
- 文件类型过滤器 + 作用域选择

### 11.3 JetBrains 通用设计模式

- **MVC 三层拆分**：UI 层 (Form/Configurable) → Settings 层 (POJO) → PersistentStateComponent 持久化
- **Master-Detail 模式**：左侧列表 + 右侧编辑区
- 间距规范：无关控件 12px 间距，分组 20px 间距
- UI DSL：Kotlin `panel { row { ... } group { ... } }` 声明式布局

> 来源：https://www.jetbrains.com/help/idea/settings-tools-external-tools.html
> 子代理详细文档：`04-jetbrains-config-ui-reference.md` 第 1-5 节

### 11.4 对 slTerminal 最有价值的模式

1. **Master-Detail 布局**：事件列表(左) + 配置详情(右)
2. **渐进式披露**：基础字段展开，高级字段折叠
3. **模板系统**：新建 hook 时选择模板 → 自动填充默认值
4. **启用/禁用开关**：每行 checkbox 控制
5. **宏变量选择器**：点击插入变量到 command 字段

> 子代理详细文档：`04-jetbrains-config-ui-reference.md` 第 5 节

---

## 12. 社区配置案例与模式

### 12.1 事件使用频率

根据社区仓库统计（10+ 项目）：

| 频率 | 事件 | 主要用途 |
|------|------|---------|
| **高** | PreToolUse | 安全校验、命令拦截、文件写保护 |
| **高** | PostToolUse | 自动格式化、lint、日志 |
| **高** | Stop | 质量门禁、任务完成检查 |
| **高** | SessionStart | 注入 git status、TODO 列表、项目上下文 |
| **中** | UserPromptSubmit | 动态追加上下文 |
| **中** | Notification | 桌面通知 |
| **低** | SessionEnd, SubagentStart/Stop, PreCompact | 清理、日志 |

> 子代理详细文档：`05-community-hooks-examples.md` 第 3 节

### 12.2 社区项目典型案例

| 项目 | 类型 | 特色 |
|------|------|------|
| dwmkerr/claude-toolkit | 工具集 | 4 种事件全覆盖，模板化脚本结构 |
| luongnv89/claude-howto | 指南 | 9 个 hook 示例，PreToolUse 安全命令审计 |
| rulestack 系列 | 教程 | PreToolUse 护栏 + bash 检查脚本模式 |
| 社区安全方案 | 安全 | `rm -rf /*` 模式匹配 + exit 2 阻断 |
| 自动格式化方案 | 质量 | PostToolUse `Write|Edit` + prettier/eslint |
| direnv 集成 | 环境 | FileChanged `.envrc|.env` + `$CLAUDE_ENV_FILE` |

> 子代理详细文档：`05-community-hooks-examples.md` 第 9-10 节

### 12.3 多 Hook 组织方式

1. **按事件分组**：一个顶级事件 → 多个 matcher 组 → 各组一个 handler
2. **单事件多规则**：同一 matcher 下多个 `if` 条件 + 不同 handler
3. **单规则多 hook**：同一 matcher 组下多个 handler 并行执行
4. **目录结构**（社区最佳实践）：
   - `.claude/hooks/scripts/security/` -- 安全脚本
   - `.claude/hooks/scripts/quality/` -- 质量脚本
   - `.claude/hooks/scripts/workflow/` -- 工作流脚本
   - `.claude/settings.json` -- hooks 配置

> 子代理详细文档：`05-community-hooks-examples.md` 第 10 节

---

## 13. 已知 Bug 与限制

| 编号 | 问题 | Issue | 状态 |
|------|------|-------|------|
| B1 | PostToolUse `additionalContext` 未实现 | [#18534](https://github.com/anthropics/claude-code/issues/18534) | SDK v0.2.9 未修 |
| B2 | SessionStart `systemMessage` 不渲染 | [#41285](https://github.com/anthropics/claude-code/issues/41285) | v2.1.88-v2.1.132 |
| B3 | PreCompact 不支持 `hookSpecificOutput` | [ce-claude-marketplace#17](https://github.com/ce-dot-net/ce-claude-marketplace/issues/17) | 已知 |
| B4 | PreToolUse vs PermissionRequest `updatedInput` 路径不一致 | [#19124](https://github.com/anthropics/claude-code/issues/19124) | 已知 |
| B5 | `CLAUDE_PLUGIN_ROOT` 环境变量未设置 | [#24529](https://github.com/anthropics/claude-code/issues/24529) | 已知 |
| B6 | `enabledPlugins` + 未安装插件 = 崩溃 | [#20752](https://github.com/anthropics/claude-code/issues/20752) | 已知 |
| B7 | Managed settings 类型错误静默丢弃 | [#59051](https://github.com/anthropics/claude-code/issues/59051) | 已知 |
| B8 | `args` 字段被忽略（command hook） | -- | 社区报告 |
| B9 | URL 无变量展开（http hook） | -- | 社区报告 |
| B10 | Windows 下 hooks 不触发 | [#25981](https://github.com/anthropics/claude-code/issues/25981) | 已知 |

> 子代理详细文档：`01-hooks-official-docs.md` 第 15 节、`02-settings-json-schema.md` 第 5 节

---

## 14. 对 slTerminal hooks 配置 UI 的启示

### 14.1 功能优先级矩阵

| 需求 | 优先级 | 依据 | 实现难度 |
|------|:------:|------|:------:|
| **JSON Schema 驱动补全 + 校验** | P0 | VS Code 最佳实践；SchemaStore 已有先例 | 低 |
| **双模式编辑**（GUI 表单 + JSON 编辑器） | P0 | VS Code settings 模式；降低门槛 | 中 |
| **事件类型分组列表**（Master-Detail） | P0 | 30 种事件需要组织；JetBrains 模式 | 中 |
| **专用 handler 表单**（5 种类型各不同） | P1 | JetBrains External Tools 模式 | 中 |
| **Matcher 测试/预览** | P1 | 用户需要验证正则能匹配目标工具 | 低 |
| **模板生成** | P1 | VS Code "Configure Task" + JetBrains 模板 | 低 |
| **启用/禁用开关** | P2 | JetBrains checkbox 模式 | 低 |
| **宏变量选择器** | P2 | JetBrains 插入宏按钮 | 低 |
| **配置层级合并可视化** | P2 | 4 层覆盖（managed > local > project > user） | 高 |
| **YAML 替代 JSON** | P3 | JSON 注释不便，YAML 更友好 | 中 |
| **Import/Export** | P3 | 社区分享 hook 配置 | 低 |

### 14.2 推荐 UI 布局

采用 **Master-Detail** 布局：

```
┌──────────────────────┬──────────────────────────────┐
│ 左侧面板（事件列表）   │ 右侧面板（配置编辑）           │
│                      │                              │
│ PreToolUse      [2]  │ ┌─ JSON 编辑器 ─────────────┐ │
│ PostToolUse     [1]  │ │ {                           │ │
│ Stop            [1]  │ │   "PreToolUse": [{         │ │
│ SessionStart    [1]  │ │     "matcher": "Bash",     │ │
│ Notification    [0]  │ │     "hooks": [...]         │ │
│ UserPromptSubmit[0]  │ │   }]                       │ │
│ ...                   │ │ }                           │ │
│                      │ └────────────────────────────┘ │
│ [+ 添加 Hook]        │                              │
│                      │ 模式切换: [GUI 表单 | JSON]   │
│                      │ [matcher 测试工具]             │
└──────────────────────┴──────────────────────────────┘
```

### 14.3 表单控件映射（GUI 模式）

| Hook 字段 | 控件类型 | 说明 |
|-----------|---------|------|
| 事件类型 | 下拉选择 | 30 种事件，分组展示 |
| `matcher` | 文本输入 + 快速模板按钮 + 实时测试预览 | 正则，区分大小写 |
| `type` | 下拉/Radio 组 | command / http / mcp_tool / prompt / agent |
| `command` | 文本输入 + 文件选择器 | 支持 `$CLAUDE_PROJECT_DIR` 变量 |
| `args` | 多行列表编辑 | 参数数组 |
| `url` | URL 输入 | HTTP handler |
| `headers` | 键值对表格编辑 | HTTP headers |
| `timeout` | 数字输入 + 滑块 + 默认值标记 | 1-600 秒 |
| `async` | 复选框 | 布尔值 |
| `allowedEnvVars` | Tag 多选 | 环境变量白名单 |
| `shell` | 文本 + 下拉建议 | shell 路径 |
| `if` | 文本输入 | 细粒度条件 |
| 启用/禁用 | 复选框 | 不删除即可禁用 |

### 14.4 JSON Schema 生成策略（slTerminal 层面）

1. 从 Rust DTO 通过 `schemars` crate 自动生成 JSON Schema
2. 编译期嵌入前端
3. CodeMirror 6 通过 `@codemirror/lang-json` + `codemirror-json-schema` 提供实时校验和补全
4. 发布到 SchemaStore（`slterminal-hooks-config.json`）为社区提供 IDE 支持

### 14.5 阶段实施建议

**Phase 1（MVP）**：
1. JSON 编辑器模式（CM6 + JSON Schema 补全/校验）
2. 事件类型快速导航
3. 模板插入按钮
4. matcher 测试工具

**Phase 2（增强）**：
5. GUI 表单编辑器（每种 handler 类型专用表单）
6. 层级合并视图
7. 启用/禁用开关

**Phase 3（完善）**：
8. Import/Export hook 配置
9. 宏变量选择器
10. YAML 模式支持

---

## 附录：子代理产出文件

| 文件 | 内容 | 规模 |
|------|------|------|
| [01-hooks-official-docs.md](./01-hooks-official-docs.md) | 官方文档详解：30 种事件、5 种 handler、stdin/stdout 结构、`if` 字段、已知 bug | ~450 行 |
| [02-settings-json-schema.md](./02-settings-json-schema.md) | SchemaStore schema、`$schema` 支持、VS Code 三种集成、第三方校验工具 | ~200 行 |
| [03-vscode-config-ui-reference.md](./03-vscode-config-ui-reference.md) | tasks.json/launch.json Schema、Custom Editor API、jsonc-parser、设计启示 | ~350 行 |
| [04-jetbrains-config-ui-reference.md](./04-jetbrains-config-ui-reference.md) | External Tools / File Watchers UI、Master-Detail 模式、Kotlin UI DSL | ~350 行 |
| [05-community-hooks-examples.md](./05-community-hooks-examples.md) | 10+ 项目案例、使用频率、matcher 模式、组织方式、最佳实践 | ~670 行 |

---

## 主要参考来源

| 来源 | URL | 访问日期 |
|------|-----|---------|
| Claude Code Hooks 官方参考 | https://code.claude.com/docs/en/hooks | 2026-07-25 |
| Claude Code Hooks 指南 | https://code.claude.com/docs/en/hooks-guide | 2026-07-25 |
| Claude Code Agent SDK (TS) | https://code.claude.com/docs/en/agent-sdk/typescript | 2026-07-25 |
| Claude Code Agent SDK (Python) | https://code.claude.com/docs/en/agent-sdk/python | 2026-07-25 |
| Claude Code Agent SDK Hooks | https://code.claude.com/docs/en/agent-sdk/hooks | 2026-07-25 |
| Claude Code Plugin Reference | https://code.claude.com/docs/en/plugins-reference | 2026-07-25 |
| Morphllm Hooks 总结 | https://www.morphllm.com/claude-code-hooks | 2026-07-25 |
| Anthropic 官方博客 | https://claude.com/blog/how-to-configure-hooks | 2026-07-25 |
| SchemaStore (官方 JSON Schema) | https://json.schemastore.org/claude-code-settings.json | 2026-07-25 |
| VS Code Settings Helper 扩展 | https://marketplace.visualstudio.com/items?itemName=jpegg-dev.claude-code-settings-helper | 2026-07-25 |
| VS Code Tasks 文档 | https://code.visualstudio.com/docs/editor/tasks | 2026-07-25 |
| VS Code Tasks Schema 附录 | https://code.visualstudio.com/docs/reference/tasks-appendix | 2026-07-25 |
| JetBrains External Tools 文档 | https://www.jetbrains.com/help/idea/settings-tools-external-tools.html | 2026-07-25 |
| GitHub Issue #2783 ($schema 支持) | https://github.com/anthropics/claude-code/issues/2783 | 2026-07-25 |
| GitHub Issue #41262 (`if` 字段) | https://github.com/anthropics/claude-code/issues/41262 | 2026-07-25 |
| GitHub Issue #18534 (additionalContext) | https://github.com/anthropics/claude-code/issues/18534 | 2026-07-25 |
| dwmkerr/claude-toolkit | https://github.com/dwmkerr/claude-toolkit | 2026-07-25 |
| luongnv89/claude-howto | https://github.com/luongnv89/claude-howto | 2026-07-25 |
