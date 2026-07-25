# Claude Code Hooks 社区实际使用案例与模式

> 调研日期：2026-07-25
> 数据来源：GitHub 公开仓库、Anthropic 官方文档、社区博客

## 1. 概述

Claude Code hooks 是在特定会话事件（工具调用前后、会话启停等）自动执行的自定义脚本/命令，通过 JSON 配置文件定义。社区中 hooks 主要用于安全校验、自动格式化、权限审批、会话上下文注入等场景。

---

## 2. 配置文件位置与优先级

| 优先级 | 文件路径 | 作用域 | 备注 |
|--------|---------|--------|------|
| 最高 | 托管策略（企业） | 全局强制 | macOS: `/Library/Application Support/ClaudeCode/managed-settings.json` |
| 最高（非托管） | `.claude/settings.local.json` | 项目本地 | gitignored |
| 中 | `.claude/settings.json` | 项目级 | 可提交到仓库共享 |
| 最低 | `~/.claude/settings.json` | 用户级（所有项目） | 不分享 |

来源：[Claude Code Hooks 官方文档](https://code.claude.com/docs/en/hooks) / [dwmkerr/claude-toolkit](https://github.com/dwmkerr/claude-toolkit)

---

## 3. 事件类型使用频率分析（定性评估，基于社区仓库观察）

根据社区公开仓库中的配置观察（非系统性统计），事件使用频率大致分布：

### 3.1 高频事件（最常用）

| 事件 | 触发时机 | 可阻断 | 社区主要用途 |
|------|---------|--------|-------------|
| **PreToolUse** | 任何工具执行前 | 是（exit 2） | 安全校验、命令拦截、文件写保护 |
| **PostToolUse** | 工具执行成功后 | 否（仅反馈） | 自动格式化（Prettier）、lint、日志记录 |
| **Stop** | Claude 完成响应后 | 是（continue） | 质量门禁、强制继续直到任务完成 |
| **SessionStart** | 会话开始/恢复 | 否（添加上下文） | 注入 git status、TODO 列表、sprint 上下文 |

### 3.2 中频事件

| 事件 | 触发时机 | 可阻断 | 社区主要用途 |
|------|---------|--------|-------------|
| **UserPromptSubmit** | 用户提交提示词后 | 是 | 动态追加上下文（sprint 优先级、错误日志） |
| **Notification** | 通知发送时 | 否 | 桌面通知（macOS osascript） |
| **PostToolUseFailure** | 工具执行失败后 | 否 | 错误追踪、自动重试建议 |
| **PreCompact** | 上下文压缩前 | 是 (v2.1.105+) | 备份 transcript、验证压缩内容 |

### 3.3 低频事件

| 事件 | 触发时机 | 社区主要用途 |
|------|---------|-------------|
| **SessionEnd** | 会话终止 | 清理临时文件 |
| **SubagentStart** / **SubagentStop** | 子代理启停 | 子代理输出质量校验 |
| **PostCompact** | 上下文压缩后 | 日志记录 |
| **PermissionRequest** | 权限弹窗触发 | 自动审批安全命令 |
| **PermissionDenied** | 权限被拒绝后 | 日志记录、替代建议 |
| **ConfigChange** | 配置变更 | 配置热重载通知 |
| **FileChanged** | 被监视的文件变更 | 自动触发检查 |
| **StopFailure** | Stop 失败 | 错误处理 |

来源：[dwmkerr/claude-toolkit hook-development SKILL.md](https://github.com/dwmkerr/claude-toolkit) / [luongnv89/claude-howto](https://github.com/luongnv89/claude-howto) / [Claude Code 官方 hooks 参考](https://code.claude.com/docs/en/hooks)

---

## 4. Matcher 正则表达式实际写法

### 4.1 基础规则

- 简单字符串 = 精确匹配：`"Write"` 仅匹配 Write 工具
- 空字符串 `""` 或 `"*"` = 匹配所有工具
- `|` 分隔 = 多工具匹配：`"Edit|Write"` 匹配 Edit 或 Write
- 完整正则表达式：`"mcp__.*"` 匹配所有 MCP 工具
- **大小写敏感**：`"bash"` 不会匹配 `Bash` 工具
- Claude Code v2.1.191+ 中 `|` 和 `,` 等价，均为 OR 关系（非 AND）

### 4.2 社区通用 Matcher 模式

```json
// 文件操作：拦截所有文件写入
{ "matcher": "Write|Edit" }

// 文件操作：仅拦截特定扩展名
{ "matcher": "Write|Edit", "hooks": [{ "if": "Write(*.env)" }] }

// Shell 命令：拦截所有 Bash 调用
{ "matcher": "Bash" }

// Shell 命令：仅拦截 git 相关
{ "matcher": "Bash", "hooks": [{ "if": "Bash(git *)" }] }

// MCP 工具：匹配所有 MCP 操作
{ "matcher": "mcp__.*" }

// MCP 工具：匹配特定 MCP 服务器
{ "matcher": "mcp__memory__.*" }

// MCP 工具：匹配破坏性操作
{ "matcher": "mcp__.*__delete.*" }

// MCP 工具：匹配特定的 PR 创建
{ "matcher": "mcp__github__create_pull_request" }

// 全匹配（慎用）
{ "matcher": "" }
{ "matcher": "*" }
{ "matcher": ".*" }

// Notebook 操作
{ "matcher": "Notebook.*" }

// 搜索工具
{ "matcher": "Glob|Grep" }

// 子代理任务
{ "matcher": "Task" }
```

### 4.3 事件特定 Matcher

某些事件类型的 matcher 匹配的不是工具名：

| 事件 | Matcher 含义 | 示例值 |
|------|-------------|--------|
| SessionStart | 会话来源 | `"startup"` / `"resume"` / `"clear"` / `"compact"` |
| Notification | 通知类型 | `"permission_prompt"` / `"idle_prompt"` / `"auth_success"` |
| PreCompact/PostCompact | 压缩触发方式 | `"manual"` / `"auto"` |
| SubagentStart/SubagentStop | 代理类型 | `"general-purpose"` / `"Explore"` / `"Plan"` |
| FileChanged | **字面文件名**（非正则） | `".envrc|.env"`（`|` 分隔，不解析为正则） |

来源：[Anthropic claude-plugins-official patterns.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md) / [Claude Code 官方 hooks 参考](https://code.claude.com/docs/en/hooks)

---

## 5. Hook 类型——五种执行方式

### 5.1 Command Hook（最常用，占比 >90%）

```json
{
  "type": "command",
  "command": ".claude/hooks/my-script.sh",
  "timeout": 600,
  "async": false,
  "statusMessage": "校验中..."
}
```

- 通过 shell 执行脚本/命令
- 通过 stdin 接收 JSON payload
- 通过 exit code 控制行为
- 支持 `args` 字段替代 `command`（execve 模式，避免 shell 注入）

### 5.2 Prompt Hook

```json
{
  "type": "prompt",
  "prompt": "Review this tool call: $ARGUMENTS",
  "model": "claude-haiku-4-5-20251001",
  "timeout": 30
}
```

- 调用另一个 Claude 模型评估/审查
- 适合需要语言理解的校验场景

### 5.3 Agent Hook

```json
{
  "type": "agent",
  "prompt": "Verify tests pass after this change.",
  "timeout": 60
}
```

- 启动子代理处理任务
- 适合需要多步骤验证的场景

### 5.4 HTTP Hook

```json
{
  "type": "http",
  "url": "http://localhost:4000/api/hooks",
  "headers": { "X-Secret": "$HOOK_SECRET" },
  "allowedEnvVars": ["HOOK_SECRET"],
  "timeout": 30
}
```

- 向远程端点发送 POST 请求
- 适合集成外部 CI/CD 或通知系统
- URL 中不支持环境变量插值

### 5.5 MCP Tool Hook

```json
{
  "type": "mcp_tool",
  "server": "my-server",
  "tool": "my-tool",
  "args": {},
  "timeout": 30
}
```

- 调用 MCP 服务器工具
- 适合通过 MCP 协议执行操作

来源：[luongnv89/claude-howto](https://github.com/luongnv89/claude-howto) / [felipeelias/hook-lab](https://github.com/felipeelias/hook-lab)

---

## 6. Exit Code 约定

| Exit Code | 含义 | 行为 |
|-----------|------|------|
| **0** | 成功 | stdout JSON 被解析为决策，工具调用继续 |
| **2** | 阻断 | stderr 展示给用户并反馈给 Claude 模型用于自我纠正，**工具调用被阻止** |
| 其他 | 非阻断警告 | stderr 展示给用户，工具调用继续 |

### PreToolUse 阻断返回格式（exit 0 + stdout JSON）

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "禁止的破坏性命令"
  }
}
```

来源：[dwmkerr/claude-toolkit](https://github.com/dwmkerr/claude-toolkit) / [Claude Code 官方 hooks 参考](https://code.claude.com/docs/en/hooks)

---

## 7. stdin 环境变量参考

Hook 脚本从 stdin 接收 JSON payload，通过 `jq` 或 Python `json.load(sys.stdin)` 解析：

```bash
#!/bin/bash
input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name // empty')
command=$(echo "$input" | jq -r '.tool_input.command // empty')
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')
session_id=$(echo "$input" | jq -r '.session_id // empty')
transcript_path=$(echo "$input" | jq -r '.transcript_path // empty')
hook_event_name=$(echo "$input" | jq -r '.hook_event_name // empty')
cwd=$(echo "$input" | jq -r '.cwd // empty')
```

关键环境变量：

| 变量 | 含义 |
|------|------|
| `$CLAUDE_PROJECT_DIR` | 项目根目录 |
| `$CLAUDE_PLUGIN_ROOT` | 插件根目录（插件 hook 用） |

来源：[MuhammadUsmanGM/claude-code-best-practices](https://github.com/MuhammadUsmanGM/claude-code-best-practices) / [89jobrien/steve](https://github.com/89jobrien/steve)

---

## 8. `if` 字段——参数级过滤

Claude Code v2.1.85+ 引入 `if` 字段，在 matcher 基础上进一步过滤工具参数（参见 [GitHub Issue #41262](https://github.com/anthropics/claude-code/issues/41262)）：

```json
{
  "matcher": "Bash",
  "hooks": [{
    "type": "command",
    "if": "Bash(git *)",
    "command": "./check-git-policy.sh"
  }]
}
```

常见 `if` 模式：

| `if` 模式 | 匹配示例 |
|-----------|---------|
| `"Bash(git *)"` | `git push`、`git commit` |
| `"Bash(npm test *)"` | `npm test -- --watch` |
| `"Bash(git push *)"` | `git push origin main` |
| `"Edit(*.ts)"` | 编辑 `.ts` 文件 |
| `"Write(*.env*)"` | 写入 `.env` 相关文件 |

来源：[Anthropic claude-plugins-official patterns.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md)

---

## 9. 社区项目实际配置案例

### 9.1 安全导向配置（joaoariedi/ai-assisted-development-framework）

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "~/.claude/hooks/quality-before-commit.sh", "timeout": 120 }]
      },
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "~/.claude/hooks/block-sensitive-files.sh" }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          { "type": "command", "command": "~/.claude/hooks/format-after-edit.sh", "timeout": 15 },
          { "type": "command", "command": "~/.claude/hooks/run-tests-after-edit.sh", "timeout": 30 }
        ]
      }
    ],
    "Notification": [
      { "matcher": "", "hooks": [{ "type": "command", "command": "~/.claude/hooks/notify-on-block.sh", "timeout": 5 }] }
    ],
    "Stop": [
      { "matcher": "", "hooks": [{ "type": "command", "command": "~/.claude/hooks/stop-quality-check.sh", "timeout": 10 }] }
    ]
  }
}
```

来源：<https://github.com/joaoariedi/ai-assisted-development-framework>

### 9.2 模块化 Bash 校验器（Samuell1/.claude）

完整的个人 Claude Code 配置，hooks 拆分为独立关注点：

- `prefer-tools.ts` — 阻止 Bash 执行命令（有专用工具替代时：`cat` -> Read，`grep` -> Grep）
- `rewrite-pm.ts` — 自动重写 npm/npx/yarn/pnpm 为 bun
- `permissions.ts` — 分解复合命令（`&&`、`|`、`;`、`$()`）逐子命令校验
- `statusline.ts` — 自定义状态栏
- 共享库：`hooks/lib/` 下 shell 解析、glob 匹配、设置缓存

来源：<https://github.com/samuell1/.claude>

### 9.3 TypeScript 类型安全 Hook 系统（johnlindquist/claude-hooks）

npm 包 `claude-hooks`（v2.4.0，32 次发布），运行 `npx claude-hooks` 自动生成配置：

```
.claude/
├── settings.json          // 自动生成 hook 配置
└── hooks/
    ├── index.ts           // 类型化 handler（PreToolUse, PostToolUse, Notification, Stop）
    ├── lib.ts             // 共享工具
    └── session.ts         // 会话管理
```

特点：完整 TypeScript 类型安全、IntelliSense 支持所有 hook payload。

来源：<https://github.com/johnlindquist/claude-hooks>

### 9.4 生产级插件生态（@greyhaven/claude-code-config）

npm 包，12 个插件包，4 个生产 hook：

- `subagent-context-preparer.py` — 优化子代理上下文
- `security-validator.py` — 安全校验
- `prompt-enhancer.py` — 增强用户提示词
- `work-completion-assistant.py` — 工作完成追踪

CLI 命令：`claude-config install-hooks`、`claude-config doctor`、`claude-config backup-settings`

来源：<https://www.npmjs.com/package/@greyhaven/claude-code-config>

### 9.5 HTTP Telemetry（felipeelias/hook-lab）

Web 仪表盘监控 Claude Code hook 事件：

```json
{
  "hooks": {
    "PreToolUse": [
      { "hooks": [{ "type": "http", "url": "http://localhost:4000/api/hooks", "timeout": 1 }] }
    ],
    "PostToolUse": [
      { "hooks": [{ "type": "http", "url": "http://localhost:4000/api/hooks", "timeout": 1 }] }
    ]
  }
}
```

来源：<https://github.com/felipeelias/hook-lab>

### 9.6 构建系统风格（vscarpenter/claude-code-build-system）

双层配置模式：

- **全局 `~/.claude/settings.json`**：个人拒绝列表（rm、sudo、chmod、.env*、secrets）、开发工具允许列表、全局审计/capture/memory hooks
- **项目 `.claude/settings.json`**：团队基线权限 + 项目特定 hooks

来源：<https://github.com/vscarpenter/claude-code-build-system>

### 9.7 破坏性命令拦截（多种实现）

**Bash 脚本实现**（claude-code-best-practices）：
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/validate-command.sh"
        }]
      }
    ]
  }
}
```

**Python 实现**（secondsky/claude-skills）：
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{
          "type": "command",
          "command": "python3 ~/.claude/hooks/dangerous-command-guard.py"
        }]
      }
    ]
  }
}
```

**claude-ignore**（li-zhixin/claude-ignore）——类似 .gitignore 的文件读取拦截：
```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Read", "hooks": [{ "type": "command", "command": "claude-ignore" }] }
    ]
  }
}
```

来源：<https://github.com/MuhammadUsmanGM/claude-code-best-practices> / <https://github.com/secondsky/claude-skills> / <https://github.com/li-zhixin/claude-ignore>

### 9.8 Git Guardrails（mattpocock/skills）

防止危险 Git 操作的 Skill 封装：
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "~/.claude/hooks/block-dangerous-git.sh"
      }]
    }]
  }
}
```

来源：<https://github.com/mattpocock/skills>

### 9.9 LSP 质量门禁（seanchatmangpt/lsp-max）

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash|Edit|Write",
      "hooks": [{
        "type": "command",
        "command": "lsp-max-cli gate check"
      }]
    }]
  }
}
```

来源：<https://github.com/seanchatmangpt/lsp-max>

### 9.10 输入修改模式（updatedInput）

Hook 可以通过 stdout JSON 输出中的 `updatedInput` 字段修改工具输入参数：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit",
        "hooks": [{
          "type": "command",
          "command": "/path/to/edit-guard.py"
        }]
      },
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "/path/to/bash-timeout.py"
        }]
      }
    ]
  }
}
```

Hook 脚本返回 `{"updatedInput": "modified text"}` 即可修改工具输入，不需要配置级别的 `modifyInput` 布尔字段。

来源：<https://github.com/bobmatnyc/claude-mpm>

---

## 10. 多 Hook 配置组织方式

### 10.1 按事件分组（最常见）

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [...] },
      { "matcher": "Edit|Write", "hooks": [...] }
    ],
    "PostToolUse": [
      { "matcher": "Edit|Write", "hooks": [...] }
    ],
    "Stop": [
      { "matcher": "", "hooks": [...] }
    ]
  }
}
```

### 10.2 单事件多规则

同一事件下多个 matcher 规则形成数组，**顺序无关**，各自独立触发：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "./hooks/validate-bash.sh" }
        ]
      },
      {
        "matcher": "Edit|Write",
        "hooks": [
          { "type": "command", "command": "./hooks/format.sh" }
        ]
      }
    ]
  }
}
```

### 10.3 单规则多 Hook（顺序执行）

同一 matcher 下的多个 hook **顺序执行**：

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          { "type": "command", "command": "./hooks/format-after-edit.sh", "timeout": 15 },
          { "type": "command", "command": "./hooks/run-tests-after-edit.sh", "timeout": 30 }
        ]
      }
    ]
  }
}
```

### 10.4 脚本文件组织

社区主流做法：hooks 脚本集中在 `.claude/hooks/` 目录：

```
.claude/
├── settings.json
└── hooks/
    ├── block-rm.sh
    ├── validate-bash.sh
    ├── format.sh
    ├── secrets-check.py
    └── lib/
        ├── common.sh
        └── utils.py
```

脚本路径在 settings.json 中通过 `$CLAUDE_PROJECT_DIR` 引用：

```json
{ "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/block-rm.sh" }
```

来源：[joaoariedi/ai-assisted-development-framework](https://github.com/joaoariedi/ai-assisted-development-framework) / [vscarpenter/claude-code-build-system](https://github.com/vscarpenter/claude-code-build-system)

---

## 11. 社区最佳实践

### 11.1 性能

- Hook 脚本保持 **< 1 秒**执行时间（PreToolUse/PostToolUse 尤其关键）
- 不要在 PreToolUse/PostToolUse 中运行重操作（构建、`tsc`、`webpack`）
- 耗时校验放到 Stop 事件中

### 11.2 安全

- 使用**窄 matcher**（`"Edit|Write"` + `if` 条件），避免 `"*"` 全匹配
- Hook 脚本应引用**本地路径**（`$CLAUDE_PROJECT_DIR`），避免 `curl | sh`
- 检查第三方项目的 `.claude/` 目录——SessionStart hooks 自动执行
- 不受信任项目中使用 `"disableAllHooks": true`

### 11.3 错误处理

- exit 0 = 放行，exit 2 = 阻断，exit 1 = 非阻断警告
- Hook 脚本不应产生过多 stdout（SessionStart/UserPromptSubmit 的 stdout 会成为上下文）
- 用 `jq` 或 Python `json.load()` 解析 stdin payload，不用 grep/sed

### 11.4 开发调试

- 使用 `/hooks` 命令在交互式会话中查看已配置 hooks（只读浏览器）
- 修改配置后需开启新会话生效
- Hook 脚本独立测试后再集成

来源：[dwmkerr/claude-toolkit](https://github.com/dwmkerr/claude-toolkit) / [Claude Code 官方博客](https://claude.com/blog/how-to-configure-hooks)

---

## 12. 已知限制与注意

| 限制 | 详情 | 来源 |
|------|------|------|
| **Windows PreToolUse/PostToolUse 不触发** | 已知问题（#25981，已 CLOSED——可能已修复），事件加载但永不触发；UserPromptSubmit 正常 | [GitHub Issue #25981](https://github.com/anthropics/claude-code/issues/25981) |
| **`args` 字段被忽略** | 命令行 hook 定义中 `args` 字段被忽略 (v2.0.30) | [bobmatnyc/claude-mpm](https://github.com/bobmatnyc/claude-mpm) |
| **HTTP URL 无环境变量展开** | HTTP hook 的 url 字段不支持 `$VAR` 插值 | [社区实践总结] |
| **敏感信息暴露** | MCP 服务器的环境变量对所有 hooks 可见 | [GitHub Issue #6981](https://github.com/anthropics/claude-code/issues/6981) |

---

## 13. 社区资源汇总

| 项目 | 类型 | 链接 |
|------|------|------|
| Anthropic 官方 patterns.md | 官方文档 | <https://github.com/anthropics/claude-plugins-official/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md> |
| Anthropic 官方 hooks 指南博客 | 官方博客 | <https://claude.com/blog/how-to-configure-hooks> |
| Claude Code 官方 hooks 参考 | 官方文档 | <https://code.claude.com/docs/en/hooks> |
| awesome-claude-code-hooks | 社区收集 | <https://github.com/ithiria894/awesome-claude-code-hooks> |
| claude-code-best-practices | 教程+示例 | <https://github.com/MuhammadUsmanGM/claude-code-best-practices> |
| claude-toolkit | 插件生态 | <https://github.com/dwmkerr/claude-toolkit> |
| claude-hooks (npm) | TypeScript 工具 | <https://github.com/johnlindquist/claude-hooks> |
| @greyhaven/claude-code-config (npm) | 企业配置工具 | <https://www.npmjs.com/package/@greyhaven/claude-code-config> |
| claude-code-build-system | 参考配置 | <https://github.com/vscarpenter/claude-code-build-system> |
| claude-howto | 教程文档 | <https://github.com/luongnv89/claude-howto> |
| claude-ignore | .claudeignore 工具 | <https://github.com/li-zhixin/claude-ignore> |
| ai-assisted-development-framework | 安全配置 | <https://github.com/joaoariedi/ai-assisted-development-framework> |
| hook-lab | HTTP 遥测 | <https://github.com/felipeelias/hook-lab> |
| Samuell1/.claude | 个人完整配置 | <https://github.com/samuell1/.claude> |
| mattpocock/skills | Git guardrails Skill | <https://github.com/mattpocock/skills> |

---

## 14. 对 slTerminal 的启示

基于社区实践，slTerminal hooks 实现可参考以下模式：

1. **Matcher 正则引擎**：社区大量使用 `"Write|Edit"`、`"mcp__.*"` 等多工具匹配，正则引擎需支持 `|` 分隔符和通配符
2. **Exit code 控制**：0/2/其他三层含义是事实标准，阻断必须用 exit 2
3. **stdin JSON payload**：所有成熟 hook 系统都通过 stdin 传入结构化 JSON（含 `tool_name`、`tool_input`、`session_id` 等字段）
4. **`if` 字段参数级过滤**：`"Bash(git *)"` 等模式是区分度更高的过滤方式，社区 v2.1.85+ 已普遍使用
5. **异步非阻塞 hooks**：PostToolUse 常用 `"async": true` 避免阻塞用户操作
6. **多 hook 顺序执行**：同 matcher 下多 hook 按数组顺序执行，任一 exit 2 即可阻断
7. **Windows 限制**：Claude Code 在 Windows 上曾存在 PreToolUse/PostToolUse 不触发的问题（#25981，已 CLOSED），slTerminal 的 hooks 实现可填补同类平台差异
