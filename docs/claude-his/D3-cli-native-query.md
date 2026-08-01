# 历史会话查询方式 — CLI 原生查询能力

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论

## 关键发现

### 发现 1: `claude --resume` 是唯一官方"列出历史会话"入口，且为交互式选择器（session picker）

- 来源: https://code.claude.com/docs/en/cli-reference (截至 2026-08-01)
- 详情: 官方 CLI 参考对 `--resume`/`-r` 的原文定义：

  > "Resume a specific session by ID or name, or show an interactive picker to choose a session. The picker and name search include sessions that added this directory with `/add-dir`; passing a session ID searches only the current project directory and its git worktrees. As of v2.1.144, background sessions appear in the picker marked with `bg`"

  三种调用形式（https://code.claude.com/docs/en/sessions，截至 2026-08-01）：

  | 命令 | 行为（官方原文） |
  |------|----------------|
  | `claude --continue` | "Resumes the most recent session in the current directory" |
  | `claude --resume` | "Opens the session picker" |
  | `claude --resume <name>` | "Resumes the named session directly" |
  | `claude --from-pr <number>` | "Opens the session picker filtered to sessions linked to that pull request" |
  | `/resume` | "Switches to a different conversation from inside an active session" |

### 发现 2: session picker 的行为细节（过滤维度：项目 / worktree / 分支 / 搜索）

- 来源: https://code.claude.com/docs/en/sessions (截至 2026-08-01)
- 详情: 官方 "Use the session picker" 一节给出完整快捷键表：

  | 快捷键 | 动作（官方原文） |
  |--------|----------------|
  | `↑` / `↓` | Navigate between sessions |
  | `→` / `←` | Expand or collapse grouped sessions |
  | `Enter` | Resume the highlighted session |
  | `Space` | Preview the session content. `Ctrl+V` also works on terminals that don't capture it as paste |
  | `Ctrl+R` | Rename the highlighted session |
  | `/` 或除 Space 外任意可打印字符 | "Enter search mode and filter sessions"（粘贴 GitHub/GitLab/Bitbucket PR 链接可按 PR 查找） |
  | `Ctrl+A` | "Show sessions from all projects on this machine. Press again to return to the current repository" |
  | `Ctrl+W` | "Show sessions from all worktrees of the current repository"（仅多 worktree 仓库显示） |
  | `Ctrl+B` | "Filter to sessions from the current git branch. Press again to show all branches" |
  | `Esc` | Exit the session picker or search mode |

  选择器默认范围：当前 worktree 的会话（后台会话标 `bg`）+ 经 `/add-dir` 添加过当前目录的会话。每行显示：名称（用户设置）或 AI 生成标题/会话摘要/首条 prompt、距上次活动时间、git 分支、文件大小；`Ctrl+A` 全项目视图额外显示项目路径。按名恢复跨当前仓库及其 worktrees 解析：精确匹配直接恢复；**歧义名称** `claude --resume <name>` 会打开选择器并把名称预填为搜索词，`/resume <name>` 则直接报错（须无参运行 `/resume` 打开选择器）。

  行为边界（同页原文）：`claude -p`（print 模式）或 Agent SDK 创建的会话**不出现**在选择器中，但可用 `claude --resume <session-id>` 按 ID 恢复；session ID 查找限定启动目录的项目目录及其 git worktrees，跨目录报错 `No conversation found with session ID: <session-id>`。v2.1.211 起首条 prompt 为 `/loop` 的会话不显示在选择器；v2.1.169 起 `/cd` 移动的会话转入新目录存储。

### 发现 3: 相关标志全集（cli-reference 逐词确认）

- 来源: https://code.claude.com/docs/en/cli-reference (截至 2026-08-01)
- 详情: 与历史会话查询相关的全部标志及其官方原文描述：

  | 标志 | 官方描述（原文） |
  |------|----------------|
  | `--continue`, `-c` | "Load the most recent conversation in the current directory. Includes sessions that added this directory with `/add-dir`" |
  | `--resume`, `-r` | 见发现 1 |
  | `--fork-session` | "When resuming, create a new session ID instead of reusing the original (use with `--resume` or `--continue`)" |
  | `--session-id` | "Use a specific session ID for the conversation (must be a valid UUID)" |
  | `--from-pr` | "Open the session picker filtered to sessions linked to a specific pull request. Accepts a PR number, a GitHub or GitHub Enterprise PR URL, a GitLab merge request URL, or a Bitbucket pull request URL" |
  | `--name`, `-n` | "Set a display name for the session, shown in `/resume` and the terminal title. You can resume a named session with `claude --resume <name>`" |
  | `--no-session-persistence` | "Disable session persistence so sessions are not saved to disk and cannot be resumed. Print mode only" |

  后台会话管理子命令（同页原文）：`claude attach <id>`（"Attach to a background session in this terminal"）、`claude logs <id>`（"Print recent output from a background session"）、`claude respawn <id>`（"Restart a background session ... with its conversation intact"）、`claude rm <id>`（"Remove a background session from the list. The conversation transcript stays on your local machine, available through `claude --resume`"）、`claude stop <id>`（"Stop a background session. Also accepts `claude kill`"）。另有 `claude project purge [path]`（"Delete all local Claude Code state for a project: transcripts, task lists, debug logs, file-edit history, prompt history lines..."）。

  官方文档注明：`claude --help` 并不列出全部标志——"`claude --help` does not list every flag, so a flag's absence from `--help` does not mean it is unavailable."

### 发现 4: 截至 2026-08-01 不存在原生非交互式"列出会话"命令

- 来源: https://github.com/anthropics/claude-code/issues/16901 (截至 2026-08-01) + https://github.com/anthropics/claude-code/pull/34168 (截至 2026-08-01)
- 详情: 无 `claude --list-sessions` 类 CLI 标志，也无 `/list-sessions` 内置 slash 命令。issue #16901（qsimeon 于 2026-01-08 提出，标签 `area:core`/`area:tui`/`enhancement`，优先级 Medium）原文指出两个 UX 缺口：

  > "No way to list sessions without resuming - `claude --resume` forces an interactive selection interface"；"No way to delete sessions - There's no CLI command or slash command to delete a specific session by ID"

  提议方案：`/list-sessions` + `/delete-session <id>`，或 CLI 形式 `claude --list-sessions` / `claude --delete-session <session-id>`。issue 页面截至检索日显示 **Closed**；关闭关联的 PR #34168（"feat(plugins): Sessions plugin for listing and deleting sessions"，提议 `/sessions:list` / `/sessions:delete` 插件，含孤儿文件检测与三文件清理）截至检索日仍为 **Open 未合并**，且 PR 描述自述："The CLI flag approach (`claude --list-sessions`, `claude --delete-session`) mentioned in #16901 would require changes to the core codebase."（注：issue 显示 Closed 与 PR 未合并并存，两者快照均取自 2026-08-01，细节建议以 GitHub 页面复核。）

### 发现 5: 非交互查询的官方替代路径（SDK / headless / 原始文件）

- 来源: https://code.claude.com/docs/en/agent-sdk/typescript + https://code.claude.com/docs/en/headless + https://code.claude.com/docs/en/sessions (均截至 2026-08-01)
- 详情: 三条官方支持的非交互路径：

  1. **Agent SDK `listSessions()`**（TypeScript 官方参考原文）：`function listSessions(options?: ListSessionsOptions): Promise<SDKSessionInfo[]>;` —— "Discovers and lists past sessions with light metadata. Filter by project directory or list sessions across all projects." 参数 `dir`（缺省列出全部项目）、`limit`、`includeWorktrees`（默认 true）；返回 `SDKSessionInfo[]`（含 `sessionId`、`summary`、`lastModified`、`fileSize`、`customTitle`、`firstPrompt`、`gitBranch`、`cwd`、`tag`、`createdAt`），按 `lastModified` 降序。SDK `query()` 的 Options 含 `resume`（"Session ID to resume"）、`continue`（"Continue the most recent conversation"）、`forkSession`（"When resuming with `resume`, fork to a new session ID instead of continuing the original session"）。
  2. **headless 续问已知会话**：`claude -p --resume <session-id>`（官方示例：`claude -p --resume <session-id> --output-format json "summarize what we changed" | jq -r '.result'`）；`--output-format json` 返回 `session_id` 可用于脚本链式续问（官方示例：`session_id=$(claude -p "Start a review" --output-format json | jq -r '.session_id')` → `claude -p "Continue that review" --resume "$session_id"`）。
  3. **原始 transcript 文件**：官方说明 "transcripts are stored as JSONL at `~/.claude/projects/<project>/<session-id>.jsonl`, where `<project>` is your working directory path with non-alphanumeric characters replaced by `-`"，且警告 "The entry format is internal to Claude Code and changes between versions"——官方建议脚本化用 `/export` 或上述脚本接口，而非直接解析 JSONL。存储位置可经 `CLAUDE_CONFIG_DIR` 迁移，保留期默认 30 天（`cleanupPeriodDays`），`CLAUDE_CODE_SKIP_PROMPT_HISTORY` 可全局禁止落盘。

### 发现 6: 恢复会话的还原范围（对终端模拟器集成有约束意义）

- 来源: https://code.claude.com/docs/en/sessions (截至 2026-08-01)
- 详情: 恢复会话时还原：会话历史全文（含工具调用与结果）、模型（除非被 `--model` 或 `ANTHROPIC_MODEL` 族环境变量覆盖）、agent（`--agent` 启动的会话继续以该 agent 运行）、权限模式（`plan` 与 `bypassPermissions` 永不还原）、活跃 goal（turn 计数/计时器/token 基线重置）、未过期的定时任务。**不还原**的启动标志：`--mcp-config`、`--settings`、`--plugin-dir`、`--fallback-model`、`/add-dir` 添加的目录——恢复时需重新传入；`settings.json`/`settings.local.json` 启动时自动重读。Pro/Max 计划下，非活动约 1 小时以上且超 100,000 tokens 的会话恢复时会弹"从摘要恢复 / 原样恢复 / 不再询问"对话框。

### 发现 7: 版本演进时间线（CHANGELOG 原文）

- 来源: https://github.com/anthropics/claude-code/blob/69da5e826937e3ee68cb21e37bd9728d9209fd5f/CHANGELOG.md (截至 2026-08-01；该 changelog 只含版本号不含日期)
- 详情:

  | 版本 | 条目（原文） |
  |------|-------------|
  | 0.2.93 | "Resume conversations from where you left off from with `claude --continue` and `claude --resume`" |
  | 1.0.27 | "`/resume` slash command to switch conversations within Claude Code" |
  | 2.0.12 | "Avoid mentioning hooks in `/resume` summaries" |
  | 2.0.27 | "Added current branch filtering and search to session resume screen for easier navigation" |
  | 2.0.64 | "Added named session support: use `/rename` to name sessions, `/resume <name>` in REPL or `claude --resume <name>` from the terminal to resume them"；"Fixed `--system-prompt` being ignored when using `--continue` or `--resume` flags"；"Improved `/resume` screen with grouped forked sessions and keyboard shortcuts for preview (P) and rename (R)" |

### 发现 8: 社区对"非交互列出会话"的补充方案

- 来源: https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md (截至 2026-08-01)
- 详情: 官方无 `claude --list` 等价物，社区指南推荐直接检索 `~/.claude/projects/` 下的 JSONL：全文扫描 `rg -l --no-ignore -g '*.jsonl' 'term' ~/.claude/projects/`；结构化查询用 jq 筛选 `user` 事件、按 `timestamp` 排序、提取会话摘要（`select(.type == "summary")` 首条）；时间范围过滤 `find ~/.claude/projects -name '*.jsonl' -mtime -7`；`~/.claude.json` 的 `projects.<path>.history` 存每项目最近 prompt 字符串（仅用户提问，无回复）。指南给出两个 shell 辅助函数（`claude-find` 基于 rg 按关键词新到旧；`claude-sessions` 打印最近 N 个会话摘要），获取 session ID 后用 `claude --resume <session-id>` 重开。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/cli-reference | 官方文档 | `--resume`/`-r`、`--continue`/`-c`、`--fork-session`、`--session-id`、`--from-pr`、`--name`/`-n`、`--no-session-persistence` 原文；后台会话 attach/logs/respawn/rm/stop；`project purge`；"`--help` 不列全标志"声明 |
| https://code.claude.com/docs/en/sessions | 官方文档 | 恢复入口五命令表、session picker 全快捷键表、按名/按 ID 恢复语义、歧义名行为、还原范围、恢复对话框、transcript 存储位置与格式警告、`claude -p --resume` 脚本示例 |
| https://code.claude.com/docs/en/headless | 官方文档 | `claude -p` 非交互模式；`--continue`/`--resume` 与 `--output-format json` 链式续问会话的完整示例；`-p` 会话不出现在 picker（见 sessions 页） |
| https://code.claude.com/docs/en/commands | 官方文档 | 确认 `/resume` 存在（"returns to an earlier conversation"）及 `/clear`、`/branch`、`/cd`、`/add-dir` 条目中对 `/resume`/`--resume` 的交叉引用 |
| https://code.claude.com/docs/en/agent-sdk/typescript | 官方文档 | `listSessions(options)` 签名与描述原文、`SDKSessionInfo[]` 字段表；`query()` Options 的 `resume`/`continue`/`forkSession` 条目 |
| https://github.com/anthropics/claude-code/issues/16901 | 源码仓库（issue） | 无 `--list-sessions`/`/list-sessions` 的 feature request（2026-01-08，qsimeon）；页面截至 2026-08-01 显示 Closed |
| https://github.com/anthropics/claude-code/pull/34168 | 源码仓库（PR） | 提议 sessions 插件（`/sessions:list`/`/sessions:delete`，项目级默认、孤儿检测、三文件清理）；截至 2026-08-01 Open 未合并 |
| https://github.com/anthropics/claude-code/blob/69da5e826937e3ee68cb21e37bd9728d9209fd5f/CHANGELOG.md | 源码仓库 | 0.2.93 引入 `--continue`/`--resume`；1.0.27 `/resume`；2.0.27 分支过滤+搜索；2.0.64 命名会话 |
| https://linuxcommandlibrary.com/man/claude | 社区（man 页） | `-r ID, --resume ID` — "Resume a specific conversation by session ID or name."；`-c, --continue` — "Continue the most recent conversation."（不含 fork-session/from-pr 等新标志，页面未更新） |
| https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md | 技术博客 | 无原生 list 命令的确认；rg/jq/find 检索 `~/.claude/projects/*.jsonl` 的非交互方案；`~/.claude.json` prompt 历史；claude-find/claude-sessions shell 辅助 |

> 备注：正文引用的标志名、命令名、快捷键与官方描述均逐词取自上述页面（截至 2026-08-01 快照）。社区第三方工具（claude-sesh、agent-history、claude-log 等 npm/GitHub 项目）的细节未经官方页面逐词核实，未纳入正文断言，仅提示存在此类生态。
