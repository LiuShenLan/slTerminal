# Claude Code settings.json JSON Schema 校验与自动补全

> 调研日期：2026-07-25

## 1. 官方 JSON Schema

Claude Code 已有官方 JSON Schema，托管于 SchemaStore：

**`https://json.schemastore.org/claude-code-settings.json`**

该 Schema 由 Anthropic 直接在 SchemaStore 仓库中维护，覆盖所有已文档化的 settings 字段，包括：

- `permissions`（allow / deny / defaultMode / additionalDirectories）
- `hooks`（全部 ~30 种事件类型 + matcher + command/http/prompt/agent）
- `sandbox`（bwrapPath / socatPath / credentials 等）
- `mcpServers`
- `enabledPlugins`
- `env`
- `model`
- `teammateMode`
- `autoMode`
- `parentSettingsBehavior`
- `disableBundledSkills`
- `enforceAvailableModels`
- 以及其他

来源：<https://github.com/anthropics/claude-code/issues/2783>

## 2. `$schema` 字段支持

在 settings.json 顶部添加 `$schema` 即可启用 IDE 自动补全和实时校验：

```json
{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "allow": ["Bash(git diff:*)"]
  }
}
```

**历史**：早期版本（约 2025 年 7 月之前），Claude Code 拒绝 `$schema` 字段（报 "Unrecognized field" 错误）。**2025 年 9 月底已修复**——Anthropic 协作者 bogini 确认 Claude Code 正式支持 `$schema` 字段，SchemaStore 的 Schema 也已同步最新 settings。`$schema` 不影响运行时行为，仅用于编辑器提示和校验。

来源：<https://github.com/anthropics/claude-code/issues/2783>

## 3. VS Code 集成（三种方式）

### 方式一：`$schema` 内联（推荐）

在文件顶部加一行即可，适用于 VS Code、Cursor 及任何支持 JSON Schema 的编辑器。最简单、零依赖。

### 方式二：Claude Code Settings Helper 扩展

VS Code 市场中的专用扩展：**[Claude Code Settings Helper](https://marketplace.visualstudio.com/items?itemName=jpegg-dev.claude-code-settings-helper)**

功能：
- IntelliSense 自动补全：permissions、hooks、sandbox、MCP servers、model config、plugins、env vars 等全部字段
- JSON Schema 校验：无效属性、类型不匹配、缺失字段、废弃设置的错误提示
- Settings 覆盖可视化：Enterprise -> Local -> Project -> User 继承层级

自动激活文件匹配：
- `.claude/settings.json`
- `.claude/settings.local.json`
- `~/.claude/settings.json`
- `~/.claude.json`

来源：<https://marketplace.visualstudio.com/items?itemName=jpegg-dev.claude-code-settings-helper>

### 方式三：手动 `json.schemas` 映射

在 `.vscode/settings.json`（workspace 或 user 级别）中配置：

```json
{
  "json.schemas": [
    {
      "url": "https://json.schemastore.org/claude-code-settings.json",
      "fileMatch": [
        ".claude/settings.json",
        ".claude/settings.local.json",
        "ClaudeCode/managed-settings.json"
      ]
    }
  ]
}
```

**jsonc 支持**：VS Code 扩展内置的映射**不包含 `.jsonc` 文件**。如需对 `.claude/settings.jsonc` 启用自动补全，需手动映射：

```json
{
  "json.schemas": [
    {
      "url": "https://json.schemastore.org/claude-code-settings.json",
      "fileMatch": [".claude/settings.jsonc"]
    }
  ]
}
```

社区已提交 Feature Request 要求扩展内置映射添加 jsonc 支持：<https://github.com/anthropics/claude-code/issues/13246>

## 4. Hooks 结构规范

### 4.1 基本格式

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
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

**常见错误**：未嵌套中间 `hooks` 数组，直接放 hook 对象。正确格式必须是 `EventName -> [matcherGroup] -> hooks -> [hookObj]` 三级嵌套。

### 4.2 Matcher 字段说明

大多数事件支持 `matcher` 字段，用于按工具名或事件子类型过滤。

**支持 matcher 的事件示例**：

| 事件 | Matcher 值 |
|------|-----------|
| `SessionStart` | `startup`/`resume`/`clear`/`compact` |
| `Notification` | `permission_prompt`/`idle_prompt`/`auth_success`/`elicitation_*` |
| `SubagentStart`/`SubagentStop` | 代理类型名 |
| `PreCompact`/`PostCompact` | `manual`/`auto` |
| `PreToolUse`/`PostToolUse`/`PermissionRequest` | 工具名/正则 |

**不支持 matcher 的事件**（需省略 `matcher` 字段但仍保留数组包裹）：

- `UserPromptSubmit`、`Stop`、`PostToolBatch`、`CwdChanged`

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup",
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.claude/hooks/session-start.sh",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

### 4.3 Hook 类型

| 类型 | 说明 | 适用事件 |
|------|------|---------|
| `command` | 执行 shell 命令，stdin 接收 JSON 输入 | 全部 30 事件 |
| `http` | POST 请求，body 为 JSON | 除 SessionStart/Setup 外的 28 事件 |
| `mcp_tool` | 调用 MCP 工具 | 全部 30 事件 |
| `prompt` | LLM 评估条件 | 13 事件：PreToolUse、PostToolUse、PostToolUseFailure、PostToolBatch、PermissionRequest、PermissionDenied、UserPromptSubmit、UserPromptExpansion、Stop、SubagentStop、TaskCreated、TaskCompleted、TeammateIdle |
| `agent` | 运行带工具的 agent | 同 prompt 的 13 事件 |

> 2026-07-31 官方文档核实修订：原表称 prompt/agent 仅适用 PreToolUse/PostToolUse/PermissionRequest，失实——官方为上述 13 事件。

### 4.4 Exit Code 语义

| 退出码 | 含义 |
|--------|------|
| **0** | 成功——stdout 被解析为 JSON 输出 |
| **2** | **阻塞性错误**——stderr 喂给 Claude，操作被阻止 |
| 其他 | 非阻塞错误——显示警告，执行继续 |

> 关键：**仅退出码 2 阻止操作**。退出码 1 是*非阻塞*错误，操作照常执行。策略执行必须用退出码 2。

### 4.5 完整事件列表

共 **30 种**事件，按类别分组：

**会话生命周期**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| SessionStart | 会话启动/恢复/clear/compact | 否 |
| SessionEnd | 会话终止 | 否 |
| Setup | `--init-only` / `--maintenance` 启动 | 否 |

**用户交互**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| UserPromptSubmit | 用户提交 prompt，Claude 处理前 | 是 |
| UserPromptExpansion | 用户输入命令展开为 prompt | 是 |

**工具调用**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| PreToolUse | 工具调用执行前 | **是** |
| PermissionRequest | 权限弹窗出现 | 是 |
| PermissionDenied | auto mode 分类器拒绝工具 | 否(可请求重试) |
| PostToolUse | 工具调用成功后 | 否(可设 decision:block) |
| PostToolUseFailure | 工具调用失败后 | 否 |
| PostToolBatch | 并行工具批量完成后 | **是** |

**通知与消息**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| Notification | Claude Code 发送通知 | 否 |
| MessageDisplay | 助理消息流式输出 | 否 |

**子代理与任务**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| SubagentStart | 子 agent 生成 | 否 |
| SubagentStop | 子 agent 完成 | 是(可强制继续) |
| TaskCreated | TaskCreate 创建任务 | 是 |
| TaskCompleted | 任务标记完成 | 是 |
| TeammateIdle | agent team 队友空闲 | 是 |

**上下文管理**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| PreCompact | 上下文压缩前 | **是 (v2.1.105+)** |
| PostCompact | 上下文压缩后 | 否 |

**停止与错误**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| Stop | Claude 响应完成 | **是**(可强制继续) |
| StopFailure | API 错误导致 turn 结束 | 否 |

**配置与文件变更**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| ConfigChange | 配置文件变更 | 是 |
| CwdChanged | `cd` 命令 | 否 |
| FileChanged | 被监视文件磁盘变更 | 否 |
| InstructionsLoaded | CLAUDE.md / `.claude/rules/*.md` 加载 | 否 |

**工作树**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| WorktreeCreate | worktree 创建时 | 是 |
| WorktreeRemove | worktree 删除时 | 否 |

**启发式交互**：

| 事件 | 触发时机 | 可阻塞 |
|------|---------|:------:|
| Elicitation | MCP 服务器请求用户输入 | 是 |
| ElicitationResult | 用户响应后 | 是 |

来源：<https://code.claude.com/docs/en/hooks> | <https://code.claude.com/docs/zh-CN/hooks>

## 5. 已知问题

### 5.1 `enabledPlugins` + 未安装插件 = Schema 校验崩溃

项目 settings 中设置 `enabledPlugins` 引用本机未安装的插件（如 CI 环境）时，Ajv schema 校验崩溃而非给出友好错误。

来源：<https://github.com/anthropics/claude-code/issues/20752>

### 5.2 Managed settings 缺少服务端校验

管理员推送的 `settings.json` 存储前未经校验。类型错误（如 `allowedChannelPlugins` 给字符串而非对象、`strictKnownMarketplaces` 给无效 URL）会导致整个 settings 文档在运行时被静默丢弃——仅 debug 级日志，`permissions.deny` 等安全控制无声失效。

来源：<https://github.com/anthropics/claude-code/issues/59051>

### 5.3 官方文档缺少 Schema 链接

JSON Schema 已存在且可用，但官方文档（`https://docs.anthropic.com/en/docs/claude-code/settings`）尚未显著链接它。社区已提交 Issue 请求添加。

来源：<https://github.com/anthropics/claude-code/issues/11795>

### 5.4 Schema 同步滞后

设置新字段上线后，SchemaStore 的 schema 可能短期内不同步。社区建议将 Schema 纳入 Anthropic CI，每次发布时自动推送更新。

来源：<https://github.com/anthropics/claude-code/issues/7438>

## 6. 社区方案（已废弃）

早期有一个社区维护的 Schema 包 `spences10/claude-code-settings-schema`，在官方 Schema 出现前提供了临时方案。官方 Schema 上线后该仓库已**标记为废弃**，推荐使用官方 SchemaStore 版本。

来源：<https://github.com/spences10/claude-code-settings-schema>

## 7. 第三方校验工具

### claude-config-doctor (npm)

`npx claude-config-doctor` 校验 settings 是否符合官方 JSON Schema，额外检查 OpenRouter 配置错误和模型名错误。支持 JSON 输出用于 CI/CD。

来源：<https://www.npmjs.com/package/claude-config-doctor>

### cclint / @carlrannaberg/cclint (npm)

提供 JSON 语法和 schema 校验，覆盖 `.claude/settings.json` 的 hooks 配置结构、事件类型和 matcher。

来源：<https://www.npmjs.com/package/@carlrannaberg/cclint>

### agnix_core (Rust crate)

程序化校验器（CC-SET 规则），覆盖约 15 个顶级 settings 字段，仅校验 `.claude/` 父目录下的 `settings.json`、`settings.local.json`、`managed-settings.json`。

来源：<https://docs.rs/agnix-core/latest/src/agnix_core/rules/claude_settings.rs.html>

## 8. 手动快速校验

```bash
# JSON 语法校验
jq . .claude/settings.json

# 查看已注册 hooks（Claude Code 内）
/hooks

# debug 模式检查 hook 问题
claude --debug

# 检查 hook 脚本权限
ls -la ~/.claude/hooks/
```

## 9. Summary

| 项目 | 结论 |
|------|------|
| 官方 JSON Schema | 有，`https://json.schemastore.org/claude-code-settings.json` |
| `$schema` 支持 | 已支持（2025-09 修复） |
| VS Code 补全 | 三种方式：`$schema` 内联 / 专用扩展 / 手动映射 |
| jsonc 补全 | 内置不支持，需手动映射 |
| Hooks 结构 | 三级嵌套 + exit code 2 阻塞 |
| 已知问题 | enabledPlugins 崩溃、managed settings 无声失效、文档缺链接 |
