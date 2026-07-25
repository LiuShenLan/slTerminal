# D4: Claude Code Hooks 与终端会话(PTY)集成研究报告

> 检索日期: 2026-07-25
> 检索范围: 官方文档 (code.claude.com) > GitHub issues/discussions > 终端模拟器文档 > 社区博客

---

## 目录

1. [Hook 的 Shell 执行环境](#1-hook-的-shell-执行环境)
2. [SessionStart Hook 环境初始化](#2-sessionstart-hook-环境初始化)
3. [Stop/SessionEnd Hook 清理模式](#3-stopsessionend-hook-清理模式)
4. [Hook 与子进程通信通道](#4-hook-与子进程通信通道)
5. [终端模拟器子进程管理机制](#5-终端模拟器子进程管理机制)
6. [环境变量传递链](#6-环境变量传递链)
7. [社区痛点与已知 Bug](#7-社区痛点与已知-bug)
8. [对 slTerminal 的启示](#8-对-slterminal-的启示)

---

## 1. Hook 的 Shell 执行环境

### 1.1 执行上下文

Claude Code hooks **在设计上不附着 TTY**。Hook 子进程的 stdin/stdout/stderr 通过管道连接回 Claude Code 进程,不通过 PTY。这是有意为之的设计决策——防止 hook 的 stdout/stderr 污染对话上下文。

来源: [claude-code-tab-title](https://github.com/franzvill/claude-code-tab-title) — 项目 README 记录了"Claude Code spawns hooks with no controlling TTY"(该工具的设计前提)

### 1.2 stdin JSON 负载

所有 hook 通过 **stdin** 接收 JSON 格式的会话上下文:

```json
{
  "session_id": "d5744d24-5fff-4171-9fb3-b11060a72329",
  "transcript_path": "/home/user/.claude/projects/.../session-id.jsonl",
  "cwd": "/home/user/my-project",
  "hook_event_name": "SessionStart",
  "source": "startup",
  "permission_mode": "default"
}
```

关键字段: `session_id`、`cwd`、`transcript_path`、`hook_event_name`、`source`(startup/resume/clear/compact)

来源: [Claude Code Hooks reference](https://code.claude.com/docs/en/hooks)

### 1.3 环境变量

Hook 可访问的**官方**环境变量:

| 变量 | 可用范围 | 用途 |
|------|----------|------|
| `CLAUDE_PROJECT_DIR` | 所有 command hook | 项目根目录绝对路径 |
| `CLAUDE_PLUGIN_ROOT` | 所有 command hook | 插件安装目录——**跨平台路径必须用此变量** |
| `CLAUDE_PLUGIN_DATA` | 所有 command hook | 插件持久化数据目录 (依赖/缓存/生成代码) |
| `CLAUDE_ENV_FILE` | SessionStart / Setup / CwdChanged / FileChanged | 环境持久化文件路径——向此文件写 `export` 语句可在**后续 Bash 命令**中生效 |
| `CLAUDE_CODE_REMOTE` | 所有 hook | 远程/Web 环境时为 `"true"` |
| `CLAUDE_CODE_BRIDGE_SESSION_ID` | Remote Control 时 (v2.1.199+) | Remote Control 会话 ID |

> **注意**:
> - `session_id` **不作为独立环境变量存在**——仅通过 **stdin JSON** (`"session_id"` 字段) 获取。若需在 hook 子进程中访问 session_id，从 stdin JSON 提取后写入 `$CLAUDE_ENV_FILE`。
> - `CLAUDE_ENV_FILE` 仅传播到 **Bash Tool 子进程**（Claude Code 在每次 Bash 命令前 source 该文件），**不传播到后续 hook 子进程**——每个 hook 从原始 shell 环境启动，看不到之前 hook 通过 CLAUDE_ENV_FILE 写入的变量。
> - `CLAUDE_CODE_CHILD_SESSION` 来自社区 bug report (#72347)，非官方文档变量。设为 `1` 时阻止子 session 的 transcript 持久化。详见 [§7.3](#73-其他关键-issue)。

**关键**: Hook 继承 Claude Code 进程的完整环境变量——即终端模拟器 → shell → `claude` CLI 的全部环境变量传递链。但 hook 不在 PTY 内执行,没有 TTY 设备可访问。

来源: [Claude Code Hooks reference](https://code.claude.com/docs/en/hooks), [Claude Code Plugins reference](https://code.claude.com/docs/en/plugins-reference)

### 1.4 cwd 行为

Hook 启动时的 cwd 取决于事件:
- **SessionStart**: cwd = 启动 claude 时的目录 (不一定等于 `CLAUDE_PROJECT_DIR`——从仓库子目录启动 claude 时,cwd 为子目录而 `CLAUDE_PROJECT_DIR` 为仓库根目录)
- **CwdChanged**: cwd = 变更后的新目录
- 各 tool hook: cwd = 触发工具调用时的当前工作目录

cwd 同时通过 stdin JSON 的 `cwd` 字段和环境变量 `CLAUDE_PROJECT_DIR` 两种途径传递。

### 1.5 与主 PTY 会话的隔离

**Hook 无法与主 PTY 会话共享环境**:
- Hook 在独立子进程中执行,stdin/stdout/stderr 通过管道连接,不经过 PTY
- 向 `/dev/tty` 写入会失败——bash 先向 stderr 输出 `"/dev/tty: No such device or address"`,Claude Code 将此 stderr 解释为 hook 错误 (产生 "startup hook error" 提示)。用 `{cmd; } 2>/dev/null || true` 包裹可抑制
- Hook 不能直接修改 Claude Code 父进程的环境变量——唯一方案是写入 `$CLAUDE_ENV_FILE`,Claude Code 在每次 Bash 命令前 source 该文件

---

## 2. SessionStart Hook 环境初始化

### 2.1 基本模式: cwd + 项目检测

```bash
#!/bin/bash
set -euo pipefail
cd "$CLAUDE_PROJECT_DIR" || exit 1

# 检测项目类型并写入 $CLAUDE_ENV_FILE
if [ -f "package.json" ]; then
  echo "export PROJECT_TYPE=nodejs" >> "$CLAUDE_ENV_FILE"
fi
if [ -f "Cargo.toml" ]; then
  echo "export PROJECT_TYPE=rust" >> "$CLAUDE_ENV_FILE"
fi
```

来源: [Anthropic 官方 hook 示例](https://github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/hook-development/examples/load-context.sh)

### 2.2 direnv / mise / rtx 集成 (最重要社区方案)

Claude Code 的 Bash 工具使用非交互式非登录 shell,因此 `~/.bashrc`、`PROMPT_COMMAND`、direnv hook 均不生效。社区标准方案:

**Hook 脚本** (`~/.claude/hooks/direnv-load.sh`):
```bash
#!/bin/bash
[ -n "$CLAUDE_ENV_FILE" ] || exit 0

ENV_SNAPSHOT="${CLAUDE_ENV_FILE}.snapshot"

# 仅在首次插入 source 行 (CLAUDE_ENV_FILE 是追加式,多 hook 共享)
if ! grep -qF "$ENV_SNAPSHOT" "$CLAUDE_ENV_FILE" 2>/dev/null; then
    echo ". \"$ENV_SNAPSHOT\"" >> "$CLAUDE_ENV_FILE"
fi

# 每次运行重新生成快照
(
    if command -v direnv >/dev/null 2>&1; then
        direnv export bash 2>/dev/null
    fi
    echo "true"  # 关键守卫: 防止 ; && 语法错误
) > "$ENV_SNAPSHOT"
```

**配置** (`~/.claude/settings.json`):
```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "bash ~/.claude/hooks/direnv-load.sh || true"
      }]
    }],
    "CwdChanged": [{
      "hooks": [{
        "type": "command",
        "command": "bash ~/.claude/hooks/direnv-load.sh || true"
      }]
    }]
  }
}
```

**为什么需要 `CwdChanged`**: monorepo 场景下,不同子目录有不同 `.envrc`,claude 切换目录后需重新加载环境。

来源: [GitHub issue #42229](https://github.com/anthropics/claude-code/issues/42229), [direnv wiki - Claude Code](https://github.com/direnv/direnv/wiki/Claude-Code)

### 2.3 Python 虚拟环境

四种方案,按推荐度排列:

| 方案 | 机制 | 配置位置 |
|------|------|----------|
| 直接解释器路径 | CLAUDE.md 指示 `.venv/bin/python script.py` | CLAUDE.md |
| `env` 块 | `.claude/settings.local.json` 预置 `VIRTUAL_ENV` + `PATH` | settings.local.json (gitignored) |
| Hook 注入环境 | SessionStart hook → `CLAUDE_ENV_FILE` 写 export | settings.json + hook 脚本 |
| PreToolUse 拦截 | 阻止裸 `python`/`pip` 命令 (exit 2) | settings.json hook |

来源: [pydevtools guide](https://pydevtools.com/handbook/how-to/how-to-configure-claude-code-to-use-virtual-environments/)

> **补充**: Conda 环境 (`conda activate`) 在非交互式 shell 中行为不同，见 [GitHub issue #2509](https://github.com/anthropics/claude-code/issues/2509)。

### 2.4 Node.js / nvm

```bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
[ -f ".nvmrc" ] && nvm use 2>/dev/null || true
echo "export PROJECT_TYPE=nodejs" >> "$CLAUDE_ENV_FILE"
```

### 2.5 关键注意事项

- **`CLAUDE_ENV_FILE` 应使用追加写入**: 多个 hook 可能共享同一文件,文件在 Bash 命令执行前被 source。使用 `>>` 追加而非 `>` 覆盖,避免后执行的 hook 擦除前面的内容。使用 snapshot 间接引用模式可进一步隔离。
- **`echo "true"` 守卫**: direnv 输出行以 `;` 结尾,Claude Code 用 `&&` 连接命令。`; &&` 导致语法错误。
- **清除 session-envs 缓存**: 若出现语法错误,删除 `~/.claude/session-envs/` 目录。

---

## 3. Stop/SessionEnd Hook 清理模式

### 3.1 Stop Hook (每轮响应后触发)

非阻塞,常用于:

**a) 状态累积 + PreCompact 保护** (最重要模式):

```
Stop hook → 提取本轮修改信息 → 写入 session 状态文件
PreCompact hook → 读取累积状态 → 注入为 systemMessage (限 2000 字符)
```

防止 compaction 后丢失关键上下文。

来源: [GitHub discussion #32407](https://github.com/anthropics/claude-code/issues/32407)

**b) Git Stash 检查点**:

```bash
# 关键: 防重入守卫
IS_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
[ "$IS_ACTIVE" = "true" ] && exit 0

git stash push --include-untracked -m "checkpoint: $(date +%H:%M:%S)"
git stash apply  # 立即恢复,不中断工作流
# 保留最近 10 个 stash,超出则删除最旧的
```

来源: [dev.to - Git Stash Checkpoint System](https://dev.to/ztor2/prevent-claude-code-from-destroying-your-project-setting-up-a-git-stash-based-checkpoint-1721)

**c) 代码审查扫描**:

在 PostToolUse/Stop 中扫描修改文件,检查 `console.log`、`debugger` 等调试残留。

### 3.2 SessionEnd Hook (会话结束时触发)

**输入负载**:
```json
{
  "session_id": "...",
  "transcript_path": "...",
  "cwd": "...",
  "reason": "clear|resume|logout|prompt_input_exit|bypass_permissions_disabled|other"
}
```

**常见用途**:

```bash
#!/bin/bash
INPUT=$(cat)
REASON=$(echo "$INPUT" | jq -r '.reason')

# /clear 时跳过完整清理
[ "$REASON" = "clear" ] && exit 0

# 归档 transcript
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path')
cp "$TRANSCRIPT" "/archive/claude-sessions/$(date +%Y%m%d_%H%M%S).jsonl"

# 释放临时资源
rm -f "/tmp/claude-lock-${SESSION_ID}"
docker stop "claude-${SESSION_ID}" 2>/dev/null || true
```

来源: [claude-session-logger npm](https://www.npmjs.com/package/claude-session-logger), [claude-mem hooks](https://github.com/thedotmack/claude-mem)

### 3.3 三 Hook 生命周期模式

| Hook | 角色 |
|------|------|
| **Stop** | 每轮提取关键信号,持久化到 session 状态文件 |
| **PreCompact** | compaction 前保护关键状态不被丢失。**可主动阻止压缩** (exit code 2 或 `continue: false`, v2.1.105+) |
| **SessionEnd** | 最终归档/清理 |

---

## 4. Hook 与子进程通信通道

### 4.1 官方通道: 无直接 PTY 通信

**Claude Code 未提供 hook 向 PTY 主进程发送命令或信号的官方 API。** Hook 与 Claude Code 的通信仅限于:

- **stdout**: 文本输出被注入对话上下文 (SessionStart/UserPromptSubmit)
- **exit code**: 0=允许, 2=阻止, 其他=error
- **JSON stdout (hookSpecificOutput)**: PreToolUse 可返回 permission decision, PostToolUse 可返回 `additionalContext`
- **CLAUDE_ENV_FILE**: 唯一的环境持久化通道

### 4.2 AgentPTY 模式 (社区方案)

[AgentPTY](https://github.com/quietforgelabs/AgentPTY) 是最完整的 hooks+PTY 集成方案:

```
外部应用 → FastAPI server → PTY session (Claude Code CLI)
    ↑                            ↓
    └── Hook callbacks (Stop/StopFailure/UserPromptSubmit) ←──┘
```

- 三个 hook 作为可靠的回车/完成信号 (替代不可靠的终端输出解析)
- 通过 `AGENTPTY_SESSION_ID` 环境变量路由回调到正确会话
- PTY 处理终端 I/O,hook 处理结构化事件

### 4.3 MCP 工具桥接

多个 MCP server 通过 PTY 工具暴露终端控制能力:
- **interactive-cli-mcp**: `send_input`, `send_keys` (含 Ctrl+C/D/Z), `wait_for` (正则轮询), `get_screen`
- **brosh**: `type`, `sendKey`
- **@iflow-mcp/so2liu-pty-mcp-server**: `send_input` (文本+特殊键)

这些工具让 Claude Code 可以**间接**控制 PTY 会话──但这是 MCP 通道而非 hook 通道。

### 4.4 已知限制: 权限提示无法通过 PTY stdin 回答

[GitHub issue #38299](https://github.com/anthropics/claude-code/issues/38299) (**功能请求, 已关闭 2026-03-25**): 通过 `pty.write()` 向 Claude Code 写入字符时,字符回显到终端但**不被权限提示 handler 消费**。权限提示通过独立机制读取输入,绕过 PTY stdin。主诉求是**新增 Permission hook API**。解决方案是使用 `PermissionRequest` hook (已经存在),支持 `hookSpecificOutput.decision.behavior` 做程序化审批。

---

## 5. 终端模拟器子进程管理机制

### 5.1 Windows Terminal

**Profiles + commandline 属性** 是核心机制:

| Shell | 保持打开 | 语法 | 状态保留 |
|-------|----------|------|----------|
| PowerShell | `-NoExit` | `"commandline": "pwsh.exe -NoExit \"cmd\""` | 完整保留 |
| CMD | `/k` | `"commandline": "cmd.exe /k \"script.bat\""` | 完整保留 |
| WSL/bash | `exec bash` | `"commandline": "wsl -e bash -c 'cmd\\; exec bash'"` | 仅环境变量 |

> **注意**: WSL 命令行示例来自通用 Windows Terminal profiles 知识,非引用的 SuperUser 页面内容。

**环境变量注入**:
```json
"commandline": "cmd /c set VAR=val & pwsh.exe -nologo"
```

来源: [StackOverflow - Windows Terminal profiles](https://superuser.com/questions/1756704)

### 5.2 iTerm2

| 机制 | 触发方式 | 用途 |
|------|----------|------|
| Shell Integration | shell rc 注入 markup | CWD 跟踪、APS 基础 |
| Automatic Profile Switching | 路径/主机名/用户名/作业名 **通配符**匹配 (hostname 16 + job 4 + user 2 + path 1 = 最高 23 分) | 动态切换 profile |
| Triggers | 终端输出文本正则匹配 → 执行动作 | 自动化任务 (2025 新增 SGR 样式 trigger、命名标记) |
| Smart Selection | 五级精度 (very_low/low/normal/high/very_high) 正则 | 智能文本选择 (如 URL 识别) |
| AI Chat (3.6.0) | LLM 直接与终端交互 | AI 辅助命令行操作 |

**关键限制**: Shell Integration 需在每个目标机器上安装 (含远程 SSH)。

来源: [iTerm2 Automatic Profile Switching](https://iterm2.com/3.6/documentation-automatic-profile-switching.html)

### 5.3 WezTerm

**Shell Integration 架构**:
- **集成方式**: 用户手动在 `.bashrc`/`.zshrc` 中 source `wezterm.sh` (或由包管理器在 `/etc/profile.d/` 自动激活)——**非**通过环境变量注入机制
- **OSC 7**: CWD 跟踪
- **OSC 133**: 提示符/命令边界标记
- **OSC 1337 (User Vars)**: pane 级键值对——只能由 shell 写入、终端 Lua 配置读取,**应用不可读**。形成环境变量流 (父→子) vs User Vars (shell→终端) 的边界
- **关键坑**: `run_child_process` 继承终端模拟器环境,不继承 shell export 的变量

来源: [WezTerm Shell Integration docs](https://wezterm.org/shell-integration.html)

### 5.4 Warp

- **Shell 集成**: 通过内部 PTY 协议实现 CWD 跟踪和 shell 状态检测 (具体注入机制未在官方文档中公开)
- **Agents 3.0**: `/plan` 规划→执行,支持全终端交互 (REPL、GDB、top), Terminal-Bench 得分 52%
- **自定义 Hook / MCP**: 社区请求中 (GitHub issue #6857),尚未发布

来源: [Warp Agents 3.0 blog](https://www.warp.dev/blog/agents-3-full-terminal-use-plan-code-review-integration), [Warp Windows blog](https://www.warp.dev/blog/building-warp-on-windows)

### 5.5 各终端机制对比

| 能力 | Windows Terminal | iTerm2 | WezTerm | Warp |
|------|:---:|:---:|:---:|:---:|
| 启动命令注入 | commandline 参数 | Send text at start | default_prog + args | 有限 |
| 环境变量注入 | commandline `set` 链 | profile env 段 | set_environment_variables | - |
| 生命周期 hook | 无 | Shell Integration + Triggers | Shell Integration + User Vars | — (未公开文档) |
| CWD 跟踪 | OSC 7 (需 shell 配置) | OSC 7 (Shell Integration) | OSC 7 (Shell Integration) | PTY 协议 |
| 程序化控制 | wt.exe CLI | Python API | Lua API + Mux | — |
| AI/Agent 集成 | 无 | AI Chat (3.6.0) | 无 | Agents 3.0 (52% Terminal-Bench) |

---

## 6. 环境变量传递链

### 6.1 完整传递链

```
操作系统/桌面环境
    │  设置: PATH, HOME, USER, ...
    ▼
终端模拟器 (Windows Terminal / iTerm2 / WezTerm / slTerminal)
    │  追加: TERM, COLORTERM, TERM_PROGRAM, TERM_PROGRAM_VERSION
    │  slTerminal 追加: COLORTERM=truecolor, TERM=xterm-256color, TERM_PROGRAM=slTerminal
    │  (注入时机: pty_spawn 阶段,在子进程环境块中直接写入)
    ▼
Shell 进程 (pwsh / bash / zsh)
    │  继承终端模拟器的全部环境变量
    │  追加: 用户 rc 文件中的 export/Set-Item
    │  追加: shell integration 注入的变量
    ▼
Claude Code CLI (claude 命令)
    │  继承 shell 的全部环境变量
    │  设置: CLAUDECODE=1
    │  设置: CLAUDE_PROJECT_DIR, CLAUDE_SESSION_ID, CLAUDE_ENV_FILE
    ▼
Hook 子进程
    │  继承 Claude Code 的全部环境变量
    │  通过 stdin JSON 接收 session_id, cwd 等额外上下文
    │  stdout 写入 → 注入对话上下文
    │  CLAUDE_ENV_FILE 写入 → 持久化到后续 Bash 命令
    ▼
Bash Tool 子进程 (每次工具调用)
    继承 Claude Code 环境 + source $CLAUDE_ENV_FILE
```

### 6.2 关键传递边界

| 边界 | 传递方向 | 机制 | 限制 |
|------|----------|------|------|
| 终端→Shell | 父→子 (继承) | 进程 fork/exec 环境块 | 单向, shell 不能回写 |
| Shell→Claude | 父→子 (继承) | 进程 spawn 环境块 | 部分变量被 Claude 覆盖 |
| Hook→Bash 命令 | 文件中介 | `$CLAUDE_ENV_FILE` source | 追加式,多 hook 共享 |
| Hook→主进程 | stdout + exit code | 管道 | 无 TTY,不能发信号 |
| Shell→终端 | OSC 序列 | DCS/OSC escape codes | 单向,仅元数据非环境 |

### 6.3 跨跳丢失问题

- **SSH**: `COLORTERM`、`TERM_PROGRAM` 不会自动透过 SSH——需 `SendEnv`/`AcceptEnv` 配置
- **tmux/screen**: `TERM` 被覆盖为 `screen-256color`,`COLORTERM` 丢失
- **`run_child_process`** (WezTerm): 继承终端模拟器环境而非 shell 环境——shell export 变量不可见

---

## 7. 社区痛点与已知 Bug

### 7.1 Windows ConPTY 特定问题

**#1: SessionStart hook 导致终端输入冻结** ([#23554](https://github.com/anthropics/claude-code/issues/23554)) — **已关闭 (Duplicate/Locked, 2026-02-09)**

任何 `SessionStart` command hook 在 Windows 上导致终端输入无限期冻结。根因: SessionStart hook 的子进程干扰 **Windows ConPTY stdin 处理**,损坏 console input buffer 状态。其他 hook 类型 (PreToolUse, PostToolUse, UserPromptSubmit, PreCompact) 不受影响——它们在终端完全初始化后才触发。

**对 slTerminal 的影响**: slTerminal 使用自定义 ConPTY 创建 (`flags=0x7`),SessionStart hook 的子进程 spawn 可能与 ConPTY stdin 产生竞态。需注意 hook 子进程不应继承 PTY stdin handle。此问题已关闭修复。

**#2: UserPromptSubmit hook 收到空 stdin** ([#48009](https://github.com/anthropics/claude-code/issues/48009)) — **已关闭 (Duplicate/Locked, 2026-04-18)**

Windows 上 `UserPromptSubmit` hook 的 stdin 管道为空。可能原因: Node.js → cmd.exe → hook 进程的 stdin relay 在 Windows 上断裂。

**解决方案**: 配置 Git Bash 为 hook runner:
```json
"env": {
  "CLAUDE_CODE_GIT_BASH_PATH": "C:\\Users\\...\\Git\\bin\\bash.exe"
}
```

### 7.2 CLAUDE_ENV_FILE 相关 Bug

**#3: CLAUDE_ENV_FILE 为空字符串** ([#15840](https://github.com/anthropics/claude-code/issues/15840)) — **已关闭 (Fixed, 2026-04-09)**

`CLAUDE_ENV_FILE` 变量存在但值为空字符串,导致 `echo ... >> ""` 失败。影响 v2.0.76–v2.1.70。已在后续版本修复。

**#4: /clear 后 CLAUDE_ENV_FILE 未生效** ([#14433](https://github.com/anthropics/claude-code/issues/14433)) — **已关闭 (Fixed, 2026-03-05)**

`/clear` 创建新 session 后,hook 正确地将数据写入了**新的** env 文件（路径已更新），但 Bash 命令从未 source 这个新文件。问题在于 **sourcing 机制缺失**,而非路径指向旧文件。已在后续版本修复。

### 7.3 其他关键 Issue

**#5: 缺少程序化 `/cd` 能力** ([#69159](https://github.com/anthropics/claude-code/issues/69159)) — **仍 OPEN**

Hook 无法改变 session 工作目录。提议: hook stdout 返回 `setCwd` 字段触发 `/cd`,并自动触发 `CwdChanged` 事件。

**#6: 交互式工具无 TTY** ([#29740](https://github.com/anthropics/claude-code/issues/29740))

`AskUserQuestion` 等交互式工具在 hook 子进程中执行,stdin/stdout/stderr 连接到 socket 而非 TTY,导致交互 UI 静默失败。

**#7: `CLAUDE_CODE_CHILD_SESSION` 阻止 transcript 持久化** ([#72347](https://github.com/anthropics/claude-code/issues/72347)) — **已关闭 (Fixed, 2026-07-03)**

继承 `CLAUDE_CODE_CHILD_SESSION=1` 的新交互 session 静默跳过 transcript 写入 (无 `.jsonl` 文件)。解决方案: `unset CLAUDE_CODE_CHILD_SESSION` 或设 `CLAUDE_CODE_FORCE_SESSION_PERSISTENCE=1`。此变量来自社区发现，非官方文档。

---

## 8. 对 slTerminal 的启示

### 8.1 架构定位

slTerminal 作为原生终端模拟器,在 Claude Code 生态中处于传递链的**第二层**(终端模拟器层)。其关键职责:

1. **环境变量注入**: `COLORTERM=truecolor`、`TERM=xterm-256color`、`TERM_PROGRAM=slTerminal` 已在 `pty_spawn` 阶段正确注入到子进程环境块。这保障了 Claude Code 能检测到 truecolor 能力。

2. **ConPTY 稳定性**: Windows ConPTY `flags=0x7` (禁用 PASSTHROUGH_MODE) 避免了 hook 子进程干扰 PTY stdin 的问题 (#23554 同根因)。

3. **Shell Integration**: slTerminal 的 shell-integration.ps1 注入 OSC 7 (CWD) + OSC 133 A/B/C/D (提示符边界+退出码),为 Claude Code 提供了命令边界检测能力。

### 8.2 可增强方向

| 方向 | 描述 | 优先级 |
|------|------|--------|
| CLAUDE_ENV_FILE 可视化 | 在 UI 中展示当前 session 的环境变量快照 (来自 `CLAUDE_ENV_FILE`) | 低 |
| Hook 状态指示 | 当 Claude Code 运行 hook 时,在页签或状态栏显示指示器 | 低 |
| session-envs 管理 | 提供清除 `~/.claude/session-envs/` 缓存的 UI 入口 (解决 `; &&` 语法错误) | 低 |
| 环境变量传递链诊断 | 展示 "终端→Shell→Claude→Hook" 各层环境变量差异的可视化工具 | 低 |

### 8.3 不需要做的事

- **不需要在 slTerminal 中实现 hooks 引擎**: hooks 是 Claude Code 层面的功能,在 Claude Code 内部处理。slTerminal 只需保障 PTY 层的稳定性和环境变量正确传递。
- **不需要在终端层模拟 CLAUDE_ENV_FILE**: 这是 Claude Code 的内部机制,由 Claude Code 进程管理。

---

## 参考来源

| 来源 | URL | 类型 |
|------|-----|------|
| Claude Code Hooks Reference | https://code.claude.com/docs/en/hooks | 官方文档 |
| Claude Code Env Vars | https://code.claude.com/docs/en/env-vars | 官方文档 |
| Claude Code Hooks Guide | https://code.claude.com/docs/en/hooks-guide | 官方文档 |
| Anthropic hooks 官方示例 | https://github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/hook-development/examples/load-context.sh | 官方代码 |
| SessionStart freeze Windows #23554 | https://github.com/anthropics/claude-code/issues/23554 | Bug Report |
| Empty stdin on Windows #48009 | https://github.com/anthropics/claude-code/issues/48009 | Bug Report |
| CLAUDE_ENV_FILE empty #15840 | https://github.com/anthropics/claude-code/issues/15840 | Bug Report |
| /clear env file sourcing #14433 | https://github.com/anthropics/claude-code/issues/14433 | Bug Report |
| Permission hook PTY stdin #38299 | https://github.com/anthropics/claude-code/issues/38299 | Feature Request |
| direnv via hooks #42229 | https://github.com/anthropics/claude-code/issues/42229 | Guide |
| direnv wiki - Claude Code | https://github.com/direnv/direnv/wiki/Claude-Code | Community Wiki |
| Power user hooks patterns #32407 | https://github.com/anthropics/claude-code/issues/32407 | Discussion |
| Programmatic /cd #69159 | https://github.com/anthropics/claude-code/issues/69159 | Feature Request |
| Conda environment #2509 | https://github.com/anthropics/claude-code/issues/2509 | Question |
| Interactive tools no TTY #29740 | https://github.com/anthropics/claude-code/issues/29740 | Bug Report |
| Child session persistence #72347 | https://github.com/anthropics/claude-code/issues/72347 | Bug Report |
| AgentPTY | https://github.com/quietforgelabs/AgentPTY | Community Project |
| claude-code-tab-title (no controlling TTY) | https://github.com/franzvill/claude-code-tab-title | Community Project |
| Claude Code Plugins Reference | https://code.claude.com/docs/en/plugins-reference | 官方文档 |
| Claude Code SessionEnd #4649 | https://github.com/anthropics/claude-code/issues/4649 | Feature Request |
| Git stash checkpoint | https://dev.to/ztor2/prevent-claude-code-from-destroying-your-project-setting-up-a-git-stash-based-checkpoint-1721 | Community Blog |
| pydevtools venv guide | https://pydevtools.com/handbook/how-to/how-to-configure-claude-code-to-use-virtual-environments/ | Community Guide |
| Conda environment #2509 | https://github.com/anthropics/claude-code/issues/2509 | Question |
| Windows Terminal profiles | https://superuser.com/questions/1756704 | Documentation |
| iTerm2 Automatic Profile Switching | https://iterm2.com/3.6/documentation-automatic-profile-switching.html | 官方文档 |
| WezTerm Shell Integration | https://wezterm.org/shell-integration.html | 官方文档 |
| Warp Agents 3.0 | https://www.warp.dev/blog/agents-3-full-terminal-use-plan-code-review-integration | 官方博客 |
| Warp Windows ConPTY fork | https://www.warp.dev/blog/building-warp-on-windows | 官方博客 |
| claude-session-logger | https://www.npmjs.com/package/claude-session-logger | npm Package |
| claude-mem hooks architecture | https://github.com/thedotmack/claude-mem/blob/main/docs/hooks-architecture.mdx | Community Docs |

---

*报告生成日期: 2026-07-25*
*所有链接访问日期: 2026-07-25*
