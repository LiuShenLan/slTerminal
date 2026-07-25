# D1: Claude Code Hooks 视觉/交互反馈研究报告

> 检索日期：2026-07-25
> 研究范围：Claude Code hooks 生命周期事件、终端状态指示器、同类工具可视化方案、社区讨论

---

## 目录

1. [Claude Code Hooks 事件体系](#1-claude-code-hooks-事件体系)
2. [终端进度/状态指示器标准](#2-终端进度状态指示器标准)
3. [第三方工具生态](#3-第三方工具生态)
4. [社区讨论与 Feature Request](#4-社区讨论与-feature-request)
5. [对 slTerminal 的启示](#5-对-slterminal-的启示)

---

## 1. Claude Code Hooks 事件体系

### 1.1 事件完整列表（30+ 个）

> 来源：[Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks)
> 补充来源：[GitHub - claude-code-ultimate-guide hooks reference](https://github.com/FlorianBruniaux/claude-code-ultimate-guide/blob/main/guide/core/hooks-events-reference.md)

按生命周期阶段分组：

#### 会话生命周期（每会话一次）

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **SessionStart** | 会话启动/恢复 | `startup`/`resume`/`clear`/`compact` | 否 | 加载上下文、设置环境变量 |
| **SessionEnd** | 会话终止 | `reason` | 否 | 清理、日志 |
| **Setup** | `--init`/`--maintenance` | `init`/`maintenance` | 否 | 项目初始化 |

#### 用户交互（每轮一次）

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **UserPromptSubmit** | 用户提交提示词 | 否 | **是** | 提示词校验/修改 |
| **UserPromptExpansion** | 斜杠命令展开 | 命令/技能名 | **是** | 展开控制 |
| **Stop** | Claude 完成响应 | 否 | 可强制继续 | 后处理、状态重置 |
| **StopFailure** | API 错误导致终止 | `error` 类型 | 否 | 错误通知 |

#### 工具调用（每次工具执行）

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **PreToolUse** | 工具参数生成后、执行前 | `tool_name` (如 `Write\|Bash`) | **是** | 安全门禁、输入修改 |
| **PostToolUse** | 工具成功执行后 | `tool_name` | 可 block 反馈 | Lint、格式化、审计 |
| **PostToolUseFailure** | 工具执行失败后 | `tool_name` | 否 | 错误日志、告警 |
| **PostToolBatch** | 并行工具全部完成后 | 否（无 matcher，matcher 配置会被静默忽略） | **是** | 批量后处理 |

#### 权限系统

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **PermissionRequest** | 权限弹窗显示 | `tool_name` | **是** | 自定义权限策略 |
| **PermissionDenied** | 自动模式拒绝工具 | `tool_name` | 否 | 审计日志 |

#### 通知系统

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **Notification** | 系统通知发送 | `notification_type`（见下） | 否 | 自定义通知路由 |

**Notification matcher 值**：
- `permission_prompt` — 需要用户授权工具
- `idle_prompt` — 空闲超过 60 秒
- `auth_success` — 认证成功
- `elicitation_dialog` — MCP 工具弹窗
- `elicitation_complete` — 交互完成
- `elicitation_response` — 收到响应
- `agent_needs_input` — 后台 session 等待用户输入（v2.1.198+）
- `agent_completed` — 后台 session 完成或失败（v2.1.198+）

> **已知问题（#9575）**：Notification 钩子在 v2.0.15 中触发不稳定，仅约 25-30% 的权限提示实际触发。

#### Agent/子代理

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **SubagentStart** | 子代理启动 | `agent_type` | 否 | 子代理监控 |
| **SubagentStop** | 子代理完成 | `agent_type` | 可强制继续 | 子代理后处理 |
| **TeammateIdle** | 队友代理空闲 | 否（无 matcher） | 可强制继续 | 代理协调 |
| **TaskCreated** | 新任务创建 | 否 | **是** | 任务管理 |
| **TaskCompleted** | 任务完成 | 否（无 matcher） | **是** | 任务管理 |

#### 上下文/压缩

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **PreCompact** | 上下文压缩前 | `manual`/`auto` | **是** (v2.1.105+) | 压缩控制 |
| **PostCompact** | 压缩完成后 | 触发类型 | 否 | 压缩后处理 |

#### 环境变更

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **ConfigChange** | 配置文件变更 | `source` | **是** | 配置同步 |
| **CwdChanged** | 工作目录变更 | 否 | 否 | 目录跟踪 |
| **FileChanged** | 监视文件变更 | 文件名 | 否 | 文件同步 |
| **InstructionsLoaded** | CLAUDE.md 加载 | `load_reason` | 否 | 指令跟踪 |
| **WorktreeCreate** | Git worktree 创建 | 路径 | **是** | worktree 管理 |
| **WorktreeRemove** | Git worktree 删除 | 路径 | 否 | worktree 管理 |

#### MCP 交互

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **Elicitation** | MCP 请求用户输入 | MCP 服务器名 | **是** | 交互控制 |
| **ElicitationResult** | 用户响应 MCP | MCP 服务器名 | **是** | 交互后处理 |

#### 其他

| 事件 | 触发时机 | Matcher | 可阻断 | 用途 |
|------|---------|---------|--------|------|
| **MessageDisplay** | 消息展示 | 否 | 否 | 消息处理 |

### 1.2 Hook 配置格式

> 来源：[Hooks reference](https://code.claude.com/docs/en/hooks)
> 补充：[dev.to - Claude Code hooks explained](https://dev.to/rulestack/claude-code-hooks-explained-config-structure-matchers-and-a-copy-paste-pretooluse-guard-58jj)

三层嵌套结构：

```json
{
  "hooks": {
    "HookEventName": [
      {
        "matcher": "ToolPattern",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/hook.sh",
            "timeout": 60
          }
        ]
      }
    ]
  }
}
```

| 层级 | 说明 |
|------|------|
| **Hook Event** | 生命周期事件名 (`PreToolUse`, `Notification`, `Stop` 等) |
| **Matcher Group** | 按条件过滤 (`"Bash"`, `"Edit|Write"`, `"*"`) |
| **Hook Handler** | 实际执行的处理器 |

**Handler 类型（5 种）**：

| 类型 | 描述 | 支持决策 | 超时默认 |
|------|------|---------|---------|
| `command` | Shell 命令，stdin 接收 JSON | 是 | 600s |
| `prompt` | 单轮 LLM 评估 | 是 | 30s |
| `agent` | 多轮子代理（有工具访问） | 是 | 60s |
| `http` | POST 到外部 URL | 是 | 600s |
| `mcp_tool` | 调用 MCP 工具 | 是 | 600s |

> **注意**：`Notification`、`SessionEnd`、`PreCompact`、`PostCompact` 仅支持 `command`、`http`、`mcp_tool`（不支持 `prompt`、`agent`）。`SessionStart`、`Setup` 仅支持 `command`、`mcp_tool`。

**Matcher 语法**：

| 模式 | 示例 | 匹配 |
|------|------|------|
| 精确匹配 | `"Write"` | 仅 Write 工具 |
| 管道 OR | `"Write|Edit"` | Write 或 Edit |
| 正则 | `"mcp__.*"` | 所有 MCP 工具 |
| Bash 子模式 | `"Bash(git:*)"` | 仅 git 命令 |
| 通配 | `"*"` 或空或省略 | 匹配全部 |

**`statusMessage` 字段**：
每个 handler 可设置 `"statusMessage"`，运行期间在终端显示自定义 spinner 消息。

### 1.3 Hook 输出 JSON 格式

> 来源：同上

**关键区别**：不同事件使用不同的 JSON 输出格式。

| 事件 | Block/Deny 字段 | 位置 |
|------|-----------------|------|
| **PreToolUse** | `permissionDecision: "allow"|"deny"|"ask"` | `hookSpecificOutput` 内 |
| **PostToolUse** | `decision: "block"` | 顶层 |
| **UserPromptSubmit** | `decision: "block"` | 顶层 |
| **PermissionRequest** | `behavior: "allow"|"deny"` | `hookSpecificOutput.decision` 内 |
| **Stop / SubagentStop** | `decision: "block"` | 顶层 |

**PostToolUse 完整输出示例**：
```json
{
  "decision": "block",
  "reason": "Lint errors found",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "Error on line 42"
  },
  "continue": true,
  "suppressOutput": true
}
```

**PreToolUse 的 `updatedInput` 字段**（v2.0.10+）允许修改工具参数：
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "updatedInput": { "command": "modified command" }
  }
}
```

---

## 2. 终端进度/状态指示器标准

### 2.1 OSC 9;4 进度序列

> 来源：
> - [Microsoft - Set the progress bar in the Windows Terminal](https://learn.microsoft.com/en-us/windows/terminal/tutorials/progress-bar-sequences)
> - [WezTerm Issue #6581](https://github.com/wezterm/wezterm/issues/6581)
> - [Terminal.Gui Issue #2302](https://github.com/gui-cs/Terminal.Gui/issues/2302)

**格式**：`ESC ] 9 ; 4 ; <state> ; <progress> ST`

| State | 含义 | 视觉 |
|-------|------|------|
| 0 | 隐藏进度 | 清除 |
| 1 | 正常进度 (0-100%) | 绿色进度条 |
| 2 | 错误状态 | 红色 |
| 3 | 不确定（脉冲动画） | spinner |
| 4 | 警告/暂停 | 黄色 |

**Windows Terminal**（v1.6+）完整支持：
- 标签页头显示进度环
- Windows 任务栏显示进进度条覆盖层
- 后台窗口也可通过任务栏显示进度

**终端支持现状**：

| 终端 | 支持程度 |
|------|---------|
| Windows Terminal | 完整支持（含任务栏集成） |
| ConEmu | 原始出处，完整支持 |
| WezTerm | 处理中（#6581），通过 Lua 回调暴露状态 |
| Ghostty | 支持 |
| Kitty | 支持 |
| macOS 终端 (iTerm2 等) | 不支持 |

**典型使用模式**：
```
开始 → state=3（不确定态）
运行 → state=1（百分比更新）
错误 → state=2
暂停 → state=4
完成 → state=0（清除）
```

**PowerShell 示例**：
```powershell
Write-Host -NoNewline ("`e]9;4;1;50`a")  # 50% 进度
Write-Host -NoNewline ("`e]9;4;0;0`a")   # 清除
```

### 2.2 iTerm2 Shell Integration

> 来源：
> - [iTerm2 Shell Integration](https://iterm2.com/shell_integration.html)
> - [iTerm2 Badges Documentation](https://iterm2.com/3.0/documentation-badges.html)

**核心机制**：

1. **Badge 系统**：在终端右上角显示文本标签，格式 `\(variableName)`
   - 系统变量：`\(session.hostname)`, `\(session.username)`, `\(session.path)`
   - 用户变量：通过 `iterm2_set_user_var` 设置

2. **自定义变量（OSC 1337）**：
   ```bash
   printf "\033]1337;SetUserVar=%s=%s\007" foo $(echo -n bar | base64)
   ```

3. **Shell Integration Marks（OSC 133）**：
   - `OSC 133 ; A ST` — 提示符开始
   - `OSC 133 ; B ST` — 提示符结束
   - `OSC 133 ; C ST` — 命令执行前
   - `OSC 133 ; D ; exit_code ST` — 命令退出状态

4. **Python API**：iTerm2 提供完整的 Python 脚本 API，可编程控制 tab 颜色、badge、窗口布局等

**对 hooks 可视化的意义**：
- iTerm2 的 badge + Python API 组合是实现 "hook 状态 → tab 指示器" 的技术基础
- 第三方工具（claude-iterm2 等）利用此 API 实现 tab 颜色/Badge 动态变化

### 2.3 其他终端相关能力

**WezTerm Lua 事件系统**：
- 通过 Lua 配置 hook 可自定义 tab 标题、颜色、进度显示
- OSC 9;4 进度状态暴露为 Lua 可读属性
- 支持 multiplexer 重连时自动同步进度状态

**Kitty Remote Control**：
- 支持 `kitty @` 命令远程控制终端
- 可设置 tab 颜色、标题、OSC 序列

**Windows Terminal 增强**：
- `setProgress` 序列支持任务栏进度覆盖层（对后台终端尤其有价值）
- Shell integration 的 `auto-mark` 支持提示符边界检测

---

## 3. 第三方工具生态

### 3.1 分类总览

按可视化方式分类：

#### A. Tab 颜色/指示器类

| 工具 | 终端 | 可视化方式 | Hook 事件 | 实现架构 |
|------|------|-----------|-----------|---------|
| **claude-iterm2** | iTerm2 (macOS) | Tab 颜色(蓝/黄/绿/红) + Badge 文本 + 动画(gradient/flash/pulse) | Stop, PreToolUse, Notification 等 | Hook 脚本 → iTerm2 Python API |
| **claude-code-iterm2-tab-status** | iTerm2 (macOS) | Tab 标题前缀(⚡/💤/🔴闪烁) | Stop, PreToolUse | Hook → JSON 文件 → Python 轮询 |
| **tabby-claude-status** | Tabby (Win/Mac/Linux) | Tab 底部边框色 + emoji 前缀 + 进度条 + 活动点 + 任务栏闪动/覆盖层 | 全量事件 | Hook → `%TEMP%/tabby-claude-status.json` → Tabby 插件 |

#### B. HUD/状态栏类

| 工具 | 终端 | 可视化方式 | Hook 事件 | 实现架构 |
|------|------|-----------|-----------|---------|
| **claude-hud** | 任意终端（Claude Code statusline API） | 终端底部 2-4 行状态栏：模型/项目/上下文条/用量/工具活动/代理/代办 | 原生 statusline（非 hook） | stdin JSON + transcript JSONL 解析 |
| **vibe-term** | tmux（任意终端） | 顶部常驻 HUD 条带：会话 tabs + 状态(Working/Idle/Blocked) + 上下文用量(红绿灯色) | SessionStart, Stop, PreToolUse 等 | Hook → 信号 → tmux panel |

#### C. TUI/仪表盘类

| 工具 | 终端 | 可视化方式 | Hook 事件 | 实现架构 |
|------|------|-----------|-----------|---------|
| **@yumazak/joy** | 任意终端 | TUI 仪表盘：所有活跃会话 + 状态指示器(🔄/🟡/🟢) | 全量事件 | Hook 驱动 |
| **agent-deck** | 任意终端 (Go TUI) | 会话列表：●运行/◐等待/○空闲/✕错误 + tmux status bar 通知 | 多 hook 事件 | Hook + tmux 集成 |
| **cctiles** | 任意终端 (Rust TUI) | 网格实时视图：彩色状态指示器(空闲/工作/等待/崩溃) + 实时工具调用流 + Git 状态 | Claude Code hooks | 每 tile 跑真实 claude 进程 |
| **lazyclaude** | tmux（任意终端） | 会话列表 + 实时输出预览 + 权限弹窗覆盖层 | SessionStart, UserPromptSubmit 等 | tmux 会话 + hook 注入 |
| **ccmgr** | tmux（任意终端） | 左栏会话列表 + 右栏活跃会话 | SessionStart, Stop 等 | tmux detached sessions + hooks |
| **csm (claude-session-manager)** | 任意终端 | 会话注册表 + 上次提示/当前工具活动 + 优先级标记 | SessionStart, UserPromptSubmit | Hook → 自动注册 |

#### D. GUI/Web 类

| 工具 | 平台 | 可视化方式 | 状态指示器 |
|------|------|-----------|-----------|
| **claude-code-session-manager** | Electron (Mac/Linux) | 17 个配置页签 + AppStatusBar(模型/思考/团队/用量) | StatusBar |
| **Claude-Code-IDE** | Web (Win/Linux/Mac) | 8 并发会话 tabs + 橙色脉冲点(需要输入) + OS toast + 音频提示 | 脉冲点 + toast |
| **OpenLobby** | Web IM 风格 | IM 侧边栏 + 审批卡片 | 会话列表状态 |

### 3.2 共同架构模式

所有工具的通用架构（三步管道）：

```
Claude Code hooks 事件
    ↓
Hook 脚本写状态到信号文件/端点
    ↓
终端适配器读取信号 → 更新可视化（tab 颜色/badge/HUD 等）
```

**信号传输方式**：
- **文件方式**（最通用）：写 JSON 到 `/tmp/` 或 `%TEMP%`
- **IPC 方式**：iTerm2 Python API、Tabby plugin API
- **statusline API**（claude-hud）：stdin JSON 原生管道

**状态映射共识**：

| Hook 事件 | 通用视觉状态 |
|-----------|------------|
| `PreToolUse` / `PostToolUse` / `UserPromptSubmit` | **工作态**（蓝色/⚡/spinner） |
| `Notification` / `PermissionRequest` | **注意态**（黄色/🔴/beep） |
| `Stop` | **完成态**（绿色/✅/💤） |
| `PostToolUseFailure` | **错误态**（红色/❌/pulse） |
| `SessionStart` / `SessionEnd` | **空闲态** |

### 3.3 claude-hud（statusline 原生集成）

> 来源：[GitHub - jarrodwatts/claude-hud](https://github.com/jarrodwatts/claude-hud/blob/main/README.md)
> 补充：[dev.to - Claude Code Has a Power Meter](https://dev.to/newellpaul/claude-code-has-a-power-meter-you-just-have-to-wire-it-up-386g)

**唯一使用 Claude Code 原生 statusline API 的方案**。

默认显示（2 行）：
```
[Opus] │ my-project git:(main*)
Context █████░░░░░ 45% │ Usage ██░░░░░░░░ 25% (1h 30m / 5h)
```

可选显示：
```
◐ Edit: auth.ts | ✓ Read ×3 | ✓ Grep ×2        ← 工具活动
◐ explore [haiku]: Finding auth code (2m 15s)    ← 代理状态
▸ Fix authentication bug (2/5)                   ← 代办进度
```

**数据来源**：
- 原生 token 数据（非估算），通过 300ms 去抖窗口事件驱动更新（非固定轮询间隔）
- 解析 transcript JSONL（工具调用、子代理活动、代办进度）

**三种预设**：Full（全部）/ Essential（活动+git）/ Minimal（仅模型+上下文条）

---

## 4. 社区讨论与 Feature Request

### 4.1 官方 Feature Request

> 来源：GitHub anthropics/claude-code issues

#### #44093 — ModeChanged Hook + 程序化 `/color` 控制

- **诉求**：新增 `ModeChanged` hook 事件（切换 Default/Plan/Edit/Bypass Permissions 时触发）+ 程序化设置 `/color`（输入边框颜色）
- **动机**：红色边框（Bypass Permissions 模式）提供"不可错过"的视觉安全护栏，比小文字标签更有效
- **状态**：Closed（重复于 #42880，canonical tracking issue #42880 仍为 Open）

#### #17139 — 区分阻塞 vs 非阻塞 hook 状态的视觉指示

- **状态**：Open（最后活动 2026-01-10，距今超 6 个月，可能已被自动关闭——anthropics/claude-code 实行 stale 14 天 + 14 天自动关闭策略）

#### #43630 — PostToolUse Hook 不触发 Skill 调用

- **诉求**：Skill 工具（prompt expansion 内部处理）应触发 PostToolUse hook
- **状态**：Open

### 4.2 已知 Bug

| Issue | 描述 | 状态 |
|-------|------|------|
| [#9575](https://github.com/anthropics/claude-code/issues/9575) | Notification hook 仅 ~25-30% 率触发 | Closed（未修复，超时关闭） |
| [#11394](https://github.com/anthropics/claude-code/issues/11394) | 仅 Notification hook 从 settings.json 加载 | 部分假警报（Stop hook 实际工作正常——根因是 jq 查询 JSONL 格式不正确；但 PreToolUse/PostToolUse/UserPromptSubmit 等 hook 类型仍显示 "Found 0 hook matchers in settings"，可能为真实 bug） |

### 4.3 社区文章与工具

> 来源：
> - [dev.to - I kept losing track of my Claude Code tabs. So I fixed it.](https://dev.to/jaspersui/i-kept-losing-track-of-my-claude-code-tabs-so-i-fixed-it-3mc7)
> - [dev.to - Claude Code Has a Power Meter](https://dev.to/newellpaul/claude-code-has-a-power-meter-you-just-have-to-wire-it-up-386g)
> - [腾讯云 - Claude HUD 状态栏解析](https://cloud.tencent.com.cn/developer/article/2689441)
> - [CSDN - Claude Code 仪表盘](https://blog.csdn.net/qq_24252865/article/details/157909237)

关键社区共识：
- 多 tab 管理是 Claude Code 重度用户的核心痛点
- claude-hud statusline 是被广泛接受的 "内置" 状态反馈方案
- 第三方工具的 "hooks → 信号文件 → 终端适配器" 三步架构已成熟
- OSC 9;4 是 Windows 平台进度指示器的唯一标准

---

## 5. 对 slTerminal 的启示

### 5.1 slTerminal 的独特优势

slTerminal 是**终端外壳**（Tauri 壳 + WebView2 前端），天然比"在现有终端内跑脚本"有更强的 UI 控制力：

1. **Tab 原生控制**：Dockview 页签系统可直接设置标题、颜色、图标——不需要通过 escape sequence"旁路"控制
2. **侧栏视图系统**：SideBarArea 已有上下单槽位状态机，可直接新增一个 "Agent Monitor" 视图
3. **活动栏扩展**：可在活动栏新增一个图标按钮显示 hook 状态
4. **通知系统**：Tauri 可通过 `@tauri-apps/plugin-notification` 发送系统 toast
5. **任务栏集成**：Windows 原生，可通过 Tauri 的 `Window` API 设置任务栏进度/覆盖层图标
6. **IPC 通道**：已有 `ipc/pty`、`ipc/fs`、`ipc/clipboard`——可直接新增 `ipc/hooks` 或 `ipc/status` 通道

### 5.2 三层可视化策略建议

参考现有第三方工具的成熟方案 + slTerminal 特有优势：

| 层级 | 位置 | 可显示内容 | 实现方式 |
|------|------|-----------|---------|
| **L1: 页签级** | Dockview tab（标题/颜色/图标） | 运行态(⚡蓝)/注意态(🟡黄)/完成态(✅绿)/错误态(❌红) | Dockview API |
| **L2: 侧栏视图** | SideBarArea 新增 Agent Status 视图 | 当前会话状态详情 + 上下文用量条 + 工具活动流 + 代办进度 | 新增 sideViewDef |
| **L3: 任务栏/通知** | Windows 任务栏 + toast | 后台会话需注意（权限请求）/ 任务完成通知 | Tauri Window API |

### 5.3 关键技术路径

**方案 A — Hook 信号注入（参照第三方工具模式）**：
1. Claude Code hooks 配置写入 `~/.claude/settings.json`
2. Hook 脚本在 PTY 内执行 → 向 slTerminal 前端发送状态信号
3. 前端状态条/页签颜色根据信号实时更新

挑战：需要 Claude Code 的 hooks 配置与 slTerminal 耦合；hook 脚本需要找到向宿主终端通信的通道

**方案 B — PTY 输出扫描（被动式）**：
1. slTerminal 后端（Rust reader 线程）扫描 PTY 输出中的 hook 相关模式（如 Claude Code 的工具调用消息格式）
2. 通过 Tauri Event 推送状态到前端
3. 前端据此更新页签/HUD

优势：不需要额外配置，与 Claude Code 完全解耦
挑战：模式识别精度（假阳性/假阴性）；某些内部状态（如"空闲"）无法从输出中准确推断

**方案 C — 混合方案（推荐）**：
1. **被动扫描为主**：reader 线程扫描 OSC/ANSI 序列中的 Claude Code 进度消息（如 spinner、工具名）
2. **osc 52/osc 9;4 拦截增强**：已有 OSC 52 剪贴板拦截 + OSC 133 命令边界检测——可扩展 same pattern 拦截 Claude Code hooks 注入的自定义 OSC 序列
3. 前端侧栏视图展示聚合状态

### 5.4 最小可行方案（MVP）

参考 claude-hud 的用户反馈——**上下文用量 + 运行状态**是最高频需求：

```
[Opus] │ my-project git:(main)                     ← 模型 + 项目 + git
Context ██████░░░░ 60%  │  ◐ Running (2m 15s)      ← 上下文 + 运行状态
```

实现：slTerminal 的 PTY reader 线程扫描输出中 Claude Code 的上下文用量报告（如 `Context: 60%`），并通过 Event/Channel 推送到前端状态指示器组件。

---

## 6. 信息来源索引

| 来源 | URL | 类型 |
|------|-----|------|
| Claude Code Hooks 官方文档 | https://code.claude.com/docs/en/hooks | 官方文档 |
| Claude Code Agent SDK Hooks | https://code.claude.com/docs/en/agent-sdk/hooks | 官方文档 |
| Hooks 参考（中文） | https://code.claude.com/docs/zh-CN/hooks | 官方文档 |
| How to configure hooks (Anthropic blog) | https://claude.com/blog/how-to-configure-hooks | 官方博客 |
| hooks-events-reference (community) | https://github.com/FlorianBruniaux/claude-code-ultimate-guide | 社区指南 |
| dev.to - Hooks explained | https://dev.to/rulestack/claude-code-hooks-explained | 社区文章 |
| OSC 9;4 (Windows Terminal 文档) | https://learn.microsoft.com/en-us/windows/terminal/tutorials/progress-bar-sequences | 官方文档 |
| WezTerm OSC 9;4 Issue | https://github.com/wezterm/wezterm/issues/6581 | GitHub Issue |
| iTerm2 Shell Integration | https://iterm2.com/shell_integration.html | 官方文档 |
| iTerm2 Badges | https://iterm2.com/3.0/documentation-badges.html | 官方文档 |
| claude-hud (Jarrod Watts) | https://github.com/jarrodwatts/claude-hud | GitHub |
| claude-iterm2 (banyudu) | https://github.com/banyudu/claude-iterm2 | GitHub |
| tabby-claude-status | https://www.npmjs.com/package/tabby-claude-status | npm |
| vibe-term | https://www.npmjs.com/package/vibe-term | npm |
| claude-code-session-manager | https://github.com/StanislavBG/claude-code-session-manager | GitHub |
| agent-deck | https://github.com/asheshgoplani/agent-deck | GitHub |
| cctiles | https://github.com/WaTeR-7/cctiles | GitHub |
| lazyclaude | https://github.com/any-context/lazyclaude | GitHub |
| ccmgr | https://github.com/regmi-saugat/ccmgr | GitHub |
| csm | https://github.com/greeun/claude-session-manager | GitHub |
| Issue #44093 (ModeChanged) | https://github.com/anthropics/claude-code/issues/44093 | GitHub Issue |
| Issue #17139 (Blocking vs Non-Blocking) | https://github.com/anthropics/claude-code/issues/17139 | GitHub Issue |
| Issue #9575 (Notification hook bug) | https://github.com/anthropics/claude-code/issues/9575 | GitHub Issue |
| Issue #43630 (PostToolUse + Skill) | https://github.com/anthropics/claude-code/issues/43630 | GitHub Issue |
| dev.to - Lost track of tabs | https://dev.to/jaspersui/i-kept-losing-track-of-my-claude-code-tabs-so-i-fixed-it-3mc7 | 社区文章 |
| dev.to - Claude Code Power Meter | https://dev.to/newellpaul/claude-code-has-a-power-meter-you-just-have-to-wire-it-up-386g | 社区文章 |
| Wave Terminal Claude Code Integration | https://docs.waveterm.dev/claude-code | 官方文档 |
| Claude-Code-IDE | https://github.com/Powellga/Claude-Code-IDE | GitHub |
| OpenLobby | https://www.npmjs.com/package/openlobby | npm |

---

> 子代理补充报告（均已完成）：
> - [01-hooks-official-docs.md](./01-hooks-official-docs.md) — hooks 官方文档深入分析（30 个事件完整列表、输入/输出格式、5 种 handler 类型、UI 反馈能力）
> - [02-third-party-tools.md](./02-third-party-tools.md) — 第三方工具详细调研（25+ 工具分类分析）
> - [03-terminal-progress-standards.md](./03-terminal-progress-standards.md) — 终端进度标准详情（OSC 9;4、iTerm2 Shell Integration、WezTerm Lua 等）
> - [04-community-discussions.md](./04-community-discussions.md) — 社区讨论汇总（31 个 GitHub Issues、HN/Reddit 讨论、25+ 社区工具）
