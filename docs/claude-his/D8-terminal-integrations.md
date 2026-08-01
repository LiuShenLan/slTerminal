# 会话管理生态实践 — 终端/工作区集成

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论
> 范围: 社区如何在终端模拟器/工作区/IDE 层面集成 Claude Code 会话管理（wrapper 脚本、tmux/zellij 集成、VS Code 扩展、Web dashboard、fzf picker），以及这些实践对桌面终端应用（如 slTerminal）的可借鉴模式。

## 关键发现

### 发现 1: 官方基线 — Claude Code 原生会话命令与作用域限制

Claude Code 官方提供完整的会话恢复命令族（截至 2026-08-01）：

| 命令 | 行为 |
|------|------|
| `claude --continue` | 恢复当前目录最近一次会话 |
| `claude --resume` | 打开交互式 session picker |
| `claude --resume <name>` | 直接恢复指定名称的会话 |
| `claude --from-pr <number>` | 打开 picker 并过滤到与该 PR 关联的会话 |
| `/resume` | 从活跃会话内部切换到其他会话 |

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01)
- 详情: 关键约束——**session ID 查找作用域限定在当前项目目录及其 git worktrees**：`session ID lookup is scoped to the current project directory and its git worktrees, so a session created elsewhere reports No conversation found with session ID: <session-id>`。这是社区所有 wrapper 工具存在的前提。`claude -p` 或 Agent SDK 创建的会话不出现于 picker，但可经 `claude --resume <session-id>` 恢复。picker 支持 `Ctrl+W`（扩展到仓库全部 worktrees）、`Ctrl+A`（扩展到本机全部项目）、`Ctrl+B`（按 git 分支过滤）、`Ctrl+R`（重命名）、`Space`（预览）。跨仓库选择会话时，Claude Code 复制 `cd` + resume 命令到剪贴板而不是直接恢复——桌面终端集成需注意此行为。

### 发现 2: 官方数据层 — transcript 存储格式与索引

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01)
- 详情: transcripts 默认存为 JSONL：`~/.claude/projects/<project>/<session-id>.jsonl`，其中 `<project>` 为工作目录路径把非字母数字字符替换为 `-` 的编码。每行是 message/tool use/metadata 的 JSON 对象。官方明确警告**条目格式为内部格式，版本间可变，直接解析可能在任意 release 破坏**——官方推荐脚本经 `/export` 或脚本接口（`claude -p --resume <id> --output-format json`、hooks 的 `transcript_path` 字段）消费会话数据。可配置项：`CLAUDE_CONFIG_DIR`（迁移存储位置）、`cleanupPeriodDays`（默认 30 天保留期，settings.json 中设置）、`CLAUDE_CODE_SKIP_PROMPT_HISTORY`（禁止写 transcript）、`--no-session-persistence`（单次非交互运行不持久化）。另有多方工具引用 `~/.claude/history.jsonl` 作为索引文件（clauhist 读取之，VS Code 扩展 issue #60610 揭示 UI 会话列表由 history.jsonl 驱动，见发现 7）。

### 发现 3: shell wrapper 模式 — fzf picker + eval 目录持久化 + claude --resume

社区最普遍的集成形态：shell 函数包装扫描工具，输出 `cd <project> && claude --resume <id>` 命令并经 `eval` 执行，使恢复后的目录切换在 Claude 退出后仍保持。

- **claude-recall**（npm `@yyyeader/claude-recall`，Node ≥18，fzf 可选推荐）：
  - 来源: https://registry.npmjs.org/@yyyeader/claude-recall (2026-08-01)
  - 详情: 推荐的 `cr()` zsh/bash wrapper 完整代码——`cmd=$(claude-recall "$@" 2>/dev/null | tail -1)` 后 `echo "$cmd" | grep -q "^cd "` 匹配则 `eval "$cmd"`。用法：`cr`（fzf 浏览全部会话）、`cr <keyword>`（搜索+自动 cd+恢复）、`cr -s <id>`（会话 ID 前缀恢复，如 `-s 4a7f`；多匹配显示消歧列表）、`-l` 列表模式、`-j` JSON 输出、`-p <project>` 按项目过滤。路径解码用 "greedy path resolution"：目录名形如 `-Users-you-work-e2b-infra`，`-` 既可能是分隔符也可能是目录名的一部分（如 `e2b-infra`），算法先尝试最长目录名、检查磁盘存在性、再回退更短分段，正确解析为 `/Users/you/work/e2b-infra` 而非 `/work/e2b/infra`。
- **claude-code-tools**（PyPI，含 `find-claude-session`）：
  - 来源: https://pypi.org/project/claude-code-tools/0.2.5/ (2026-08-01)
  - 详情: 推荐 `fcs()` shell 函数——`eval "$(find-claude-session --shell "$@" | sed '/^$/d')"`（去空行后 eval，目录切换持久化）；或 source `fcs-function.sh`。用法 `fcs "k1,k2,k3"`（当前项目搜索）、`fcs -g`（全项目）、`fcs --global`；交互选择后有操作菜单（Resume 默认 / 显示文件路径 / 复制文件）；恢复用 `claude -r`；提示 "You can also use find-claude-session directly, but directory changes won't persist after exiting Claude Code"。
- **clauhist**（Rust crate，运行时依赖 fzf）：
  - 来源: https://docs.rs/crate/clauhist/1.0.1 (2026-08-01)
  - 详情: 读取 `~/.claude/history.jsonl`（非逐目录扫描），按最近活动排序，每行显示活动时间戳、`✓`/`✗`（项目目录是否仍存在）、项目路径、首消息预览、消息数。Enter 在项目目录中打开 `claude --resume`；`Ctrl-/` 切换预览面板。shell 集成 `eval "$(clauhist init zsh)"` 使当前 shell 在 Claude 退出后停留在项目目录（`cd -` 返回）。`✗` 会话目录已删除时恢复仍可用但 cd 失败、Claude 在启动位置打开。

### 发现 4: tmux 集成模式 A — 会话级管理（每项目一个 tmux session + popup picker）

- **tmux-claude-session-manager**（craftzdog，tpm 插件）：
  - 来源: https://github.com/craftzdog/tmux-claude-session-manager (2026-08-01)
  - 详情: 每个项目一个 detached tmux session（命名 `claude-<hash-of-dir>`）运行 `claude`，中央 picker（`prefix + u`）列出每个运行中的 Claude agent。关键设计——**以 Claude 进程而非 tmux 会话为身份**："pairs each running Claude with the tmux pane it occupies by joining `pid` → `tty` → pane"，同一项目多个 agent 各占一行；"Nothing here scans processes for a `claude` command name"。状态读取不靠 hook：`claude agents --json` 是运行态真相源（"the source of truth for what is running and how it is doing"），每个 session 自报 `busy`/`waiting`/`idle` 状态，需要关注的 agent（waiting/idle）排序置顶。picker 为 fzf 驱动，含实时画面预览（`capture-pane`）；launcher（`prefix + y`）在当前目录弹出 popup 启动；`ctrl-x` 杀进程。依赖：tmux ≥ 3.2（`display-popup`）、fzf、jq（解析 `claude agents --json`）、Claude Code ≥ 2.1.139（`claude agents` 命令）。注意：README 未提及 `claude --resume`——恢复是经 tmux attach 语义实现（"re-attach"），非会话 picker 语义。
- **claude-launcher**（imprakharshukla，`c` 命令 TUI）：搜索结果摘要显示其支持 fzf 模糊搜索全部会话消息、按项目分组在 tmux 中恢复（或直接在 Ghostty 中）、书签、fork、活跃会话指示（来源: https://github.com/imprakharshukla/claude-launcher，搜索摘要，未直接抓取）。

### 发现 5: tmux 集成模式 B — 进程级管理（transcript 关键词搜索 + 直接 resume）

- **tmux-claude-code**（MaxGhenis，tpm 插件）：
  - 来源: https://github.com/MaxGhenis/tmux-claude-code (2026-08-01)
  - 详情: `prefix-C-r` 按关键词恢复——"Resume a session by keyword (searches transcript content)"，检查每个会话的前几条用户消息，排除当前活跃会话，解析正确工作目录后 "opens a new pane with `claude --resume`"；`prefix-C-b` 用 fzf 预览浏览全部会话；`prefix-c` 新建窗格。CLI 命令（`cc` 符号链接）：`cc list`（最近会话含 active/inactive 状态）、`cc detect`（检测哪些 pane 跑着 Claude Code）、`cc resume <keywords>`（跨全部 transcript 关键词搜索恢复）。JSONL 存储路径 `~/.claude/projects/`，恢复时交叉引用 JSONL 的 `cwd` 字段解码项目路径（"decodes the project path (cross-referencing the JSONL `cwd` field)"）。pane 检测三层策略：`pane_current_command` → `pgrep` 子进程 → 提示符内容回退，即使 `pane_current_command` 报 `zsh`（CC 经 `zsh -c "claude ...; exec zsh"` 启动）也能识别。窗格创建用 `env -u CLAUDECODE` 防嵌套会话检测。依赖：tmux 3.2+、Python 3.6+、fzf 可选（仅浏览功能）。

### 发现 6: Web dashboard 模式 — 本地浏览器 UI 读 ~/.claude/projects/

Claude Code 无内置历史查看器，第三方 dashboard 全部本地读取 JSONL 文件（不上传数据）：

- **ccboard**（Rust，TUI + Web）：
  - 来源: https://docs.rs/crate/ccboard/0.16.3 (2026-08-01)
  - 详情: History tab（Tab 5）= 时间线 + CSV/JSON/Markdown 导出（`ccboard export sessions --output sessions.csv`，`--format json`/`--format md`）+ 全文搜索；Sessions tab（2）= 三栏布局 + 实时状态图标（●/◐/✓）+ 会话类型（CLI/IDE/Agent）+ CPU/RAM/Tokens + 会话查看器（Full JSONL replay，regex 搜索 `/` + `n`/`N`）；Analytics tab（3）= 预算跟踪、30 天预测、小时热力图、异常检测。运行 `ccboard`（TUI）、`ccboard web`（默认端口 3333，Web 与 TUI 100% 功能对等，12 个页面）、`ccboard both`。首次 `ccboard setup` 向 `~/.claude/settings.json` 注入 hooks 实现实时会话监控。
- **cc-history-viewer**（npm，浏览器专用）：
  - 来源: https://registry.npmjs.org/cc-history-viewer (2026-08-01)
  - 详情: `npx cc-history-viewer` 启动本地 API server（端口 3080，`--port` 可改，`--no-open` 禁自动开浏览器）并打开 `http://localhost:3080`。Dashboard：token 用量日线趋势（Today/Last 3 days/Last 7 days 或自定义范围）、按项目用量 donut 图、按模型族（Opus/Sonnet/Haiku）拆分的 input/output/cache-read/cache-creation token + 估算 USD 成本。Project History 侧栏：每个 `~/.claude/projects/` 项目带会话数与 last activity，无限滚动。会话消息视图：工具调用 JSON 展开、思考块、全文搜索、按角色/内容类型/工具名过滤。Live 模式浮层实时流式展示活跃会话（子代理独立卡片，主视图每秒轮询）。JSON API 供二次开发：`GET /api/projects`、`GET /api/projects/:id/sessions?limit=&offset=`、`GET /api/projects/:id/sessions/:sessionId`、`GET /api/usage?from=&to=`。仅绑定 localhost，无遥测，MIT。

### 发现 7: VS Code 扩展 — 官方会话历史 UI 及其已知局限

- 来源: https://code.claude.com/docs/en/vs-code (2026-08-01)
- 详情: 官方扩展面板顶部有 **Session history** 按钮：按关键词搜索或按时间浏览（Today/Yesterday/Last 7 days）、点击任意会话以完整消息历史恢复、悬停可重命名/删除、新会话自动获得 AI 生成标题（基于首条消息）。对话框双 tab：**Local** 与 **Web**——Web tab 恢复 claude.ai 云端会话（需 Claude.ai Subscription；仅带 GitHub 仓库启动的 web 会话出现；恢复为本地加载、不同步回云端）。扩展与 CLI 共享同一会话历史：`claude --resume` 可在终端继续扩展会话。扩展自带 URI handler `vscode://anthropic.claude-code/open`，支持 `session` 查询参数恢复指定会话（须属于当前打开的工作区，找不到则新建会话）。
- **已知局限**（社区 issue，截至 2026-08-01）：
  - 来源: https://github.com/anthropics/claude-code/issues/49095 (2026-08-01，**Closed as not planned**)
  - 详情: 扩展会话历史 picker **作用域限定当前 workspace**——用户报告磁盘上 72 个会话文件分布在 14 个项目文件夹，UI 只能看到当前工作区的 1-3 个会话；提议 "All Projects" 第三 tab（聚合全部 `~/.claude/projects/*/`、每行显示会话标题+项目名+修改时间+消息数、按关键词/项目/日期过滤）。被维护者标记 Closed as not planned（不计划实施）。同一 issue 内关联 #46862/#47581/#30599/#47945 等会话管理相关诉求；作者提及社区扩展 `agsoft.claude-history-viewer` 填补跨项目浏览空白。
  - 来源: https://github.com/anthropics/claude-code/issues/60610（issue，未直接抓取，状态未确认）
  - 详情（搜索摘要 2026-08-01）: 扩展内创建的会话不出现于历史下拉——`session.jsonl` 在磁盘存在但未追加到 `~/.claude/history.jsonl`（UI 列表的数据源），10 个会话仅 4 个被索引。
  - 来源: https://github.com/anthropics/claude-code/issues/56104（issue，未直接抓取，状态未确认）
  - 详情（搜索摘要 2026-08-01）: 扩展 v2.1.126 回归——侧栏 Local 面板与 `/resume` 无历史会话，而同捆 CLI `claude --resume` 列表正常。
  - 来源: https://github.com/anthropics/claude-code/issues/50170（issue，未直接抓取，状态未确认）
  - 详情（搜索摘要 2026-08-01）: Windows 映射网络驱动器（`X:` → `\\server\share`）无会话——扩展 `fs.realpath()` 把映射盘解析为 UNC 目标，产生与 `claude.exe` 不同的项目 key 哈希（`--server-share-myproject` vs `X--myproject`）；建议修复：Windows 上跳过 realpath。
  - 另有 #47746（worktree 会话不出现）、#45814（重启后单会话分裂为多条目）、#9258（symlink 目录历史丢失，CLI 正常）——均指向同一结论：**数据在磁盘上安全，问题在扩展 UI 的列表现层**；CLI `claude --resume` 在扩展 UI 失败时反复被确认仍正确列出会话。

### 发现 8: 语义检索路径 — SQLite FTS5 索引 + MCP server（resume-resume）

- 来源: https://github.com/eidos-agi/resume-resume (2026-08-01)
- 详情: "Search, read, and merge your Claude Code session history in plain English. MCP server + TUI." `cr` TUI 带 paste box：粘贴 resume 命令（`cd … && claude --resume <id>`）、裸会话 ID、或死会话终端的原始聊天文本——解析 ID 后搜索 `~/.claude/projects/*/` 找到所属项目（"parses the id out of whatever you pasted, searches `~/.claude/projects/*/` for the owning project"），补 `--dangerously-skip-permissions`/`--enable-auto-mode`（缺失时），显示证明对话框后 `cd` + `exec` 进入 Claude——直接修复 "No conversation found"（错误目录）失败。检索管线：括号粘贴检测 → UUID/`rollout-` 提取 → 归一化 → SQLite **FTS5** 检索（in-order 5 词 n-gram 覆盖打分，0.25 置信度门限）。性能实测：索引查询 p50 6ms @166MB、94ms @3GB（对比约 5.5s 暴力扫描）；逐字聊天文本 100% 找回、0% 开错会话、诱饵文本 0 误报；mtime-delta 增量刷新。跨工具合并：索引同时覆盖 `~/.claude/projects/`（Claude Code）与 `~/.codex/sessions/`（Codex CLI），`merge_context(id, mode)` 支持 summary/messages/hybrid 三种模式桥接两工具会话。崩溃恢复 `boot_up(hours)` 找中断会话 + 并行 `git status --porcelain` 扫描 50+ 仓库脏状态（按会话新近度+脏文件数打分，`/bookmark` 命令记录 done/paused/blocked/handoff 生命周期态，Stop hook 兜底）。MCP 工具集：`search_sessions`（5,000+ 会话约 3s 全文本搜索，BM25 排序）、`recent_sessions`、`read_session`、`session_summary`（Haiku 生成一次永久缓存）、`session_xray` 等。安装：`pip install resume-resume` + `claude mcp add resume-resume -- resume-resume-mcp`。依赖 Python 3.11+。

### 发现 9: 项目选择器模式 — 以项目为粒度的恢复（claude+）

- 来源: https://larcombe.tech/blog/claude-plus-project-chooser.html (2026-08-01)
- 详情: claude+ 是单文件 bash 脚本（内嵌 Python 块），"No dependencies beyond fzf and python3"。解析 `~/.claude/projects/` 的 mangled path 编码目录名，按项目展示：真实目录名、会话数、最后活动日期、会话摘要（上次做了什么）；有会话的项目青色置顶按活动排序，无会话的目录灰色在下。选择有会话的项目 → "it launches `claude --continue` to resume where you left off"；无会话 → 全新会话。`+ NEW PROJECT` 选项创建目录 + `git init` + 空 `CLAUDE.md` 后启动；`~ NO PROJECT` 选项无项目上下文启动。安装：`cp claude+ ~/.local/bin/claude+` + `chmod +x`。开源地址 github.com/tlarcombe/claude-project-chooser。恢复粒度是**项目**而非会话——适合跨项目频繁切换、每个项目通常只有一个活跃会话的工作流。

### 发现 10: 会话恢复的完整性语义 — 恢复后保留什么（对集成的约束）

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01)
- 详情: 恢复的会话还原：完整对话历史（含工具调用与结果）、模型（retired/被 availableModels 禁止/`--model` flag 覆盖/provider 专用部署 ID 时不还原）、agent（`--agent` 启动的会话继续为该 agent，含系统提示/工具限制/模型）、权限模式（`plan` 与 `bypassPermissions` 永不还原）、active goal（回合数/计时器/token 基线重置）、未过期的 scheduled tasks。**不还原**的启动配置：`--mcp-config`、`--settings`、`--plugin-dir`、`--fallback-model`、`--add-dir` 目录——恢复时须重新传入；`settings.json`/`settings.local.json` 启动时重读不需重传。Pro/Max 计划下恢复超过约 1 小时不活跃且 >100,000 tokens 的会话时弹三选项对话框：Resume from summary（立即 `/compact` 摘要化，后续请求携带摘要而非全历史）/ Resume full session as-is（全量重载重新缓存）/ Don't ask me again。**集成启示**：第三方启动器直接调 `claude --resume <id>` 即可获得全部还原语义，无需自行实现上下文恢复；wrapper 需注意 `--dangerously-skip-permissions` 等 flag 的透传（resume-resume 显式补 flag 的行为印证）。

### 发现 11: 对桌面终端应用（如 slTerminal）的可借鉴模式 — 扫描→展示→恢复流程架构

综合上述实践，社区工具呈现统一的三段式架构，可直接映射到桌面终端应用的 UI：

| 阶段 | 社区做法 | 代表工具 | 对 slTerminal 的启示 |
|------|---------|---------|---------------------|
| 扫描 | 读 `~/.claude/projects/*/<id>.jsonl`（逐目录扫描）或 `~/.claude/history.jsonl`（索引）；路径编码需逆向解码（`-` 分隔符与目录名歧义，greedy/longest-first 解码 + 磁盘存在性校验；tmux-claude-code 交叉引用 JSONL `cwd` 字段） | claude-recall、tmux-claude-code、clauhist、claude+ | 后端 Rust 可直接扫描本地 JSONL（slTerminal 已有 `~/.claude` 访问先例——hooks 模块）；或订阅官方 hooks `transcript_path` 增量索引；注意官方警告 JSONL 格式版本间可变 |
| 展示 | fzf 行式列表（活动时间/项目/首消息预览/`✓` 目录存在性）；tmux popup 内实时 pane 画面预览（`capture-pane`）；Web dashboard 的 donut/热力图/时间线；运行态状态（`claude agents --json` → busy/waiting/idle，优先级置顶） | tmux-claude-session-manager、ccboard、cc-history-viewer | 终端 app 内建面板可复刻 picker 键盘交互（↑↓/搜索/Enter 恢复）；「运行中会话」行可与 slTerminal 现有 Agent Status 视图（claudeSession 行建模）呼应 |
| 恢复 | `cd <project> && claude --resume <session-id>` 原语；shell wrapper 用 `eval` 保持目录持久化；tmux 层用 attach 语义；跨项目选择时官方 picker 本身只复制 cd+resume 命令到剪贴板 | claude-recall `cr()`、find-claude-session `fcs()`、cc resume | 终端 app 内恢复 = 直接 spawn PTY 且 cwd 设为目标项目目录 + `claude --resume <id>`，天然免去 eval/目录持久化问题；与官方 picker 的「剪贴板方案」相比是原生集成优势 |
| 增量 | mtime-delta 增量刷新索引；SQLite FTS5 全文本（p50 6ms）；性能目标：千级会话亚秒 | resume-resume | 首发索引 + 增量更新 + 关键词全文搜索为可选增强，非 MVP 必需 |

其他可借鉴点：
- **状态来源分离**：tmux-claude-session-manager 以 `claude agents --json` 为运行态唯一真相源，不扫进程名（macOS pane 只能见父 shell）；pane 检测需多层策略（`pane_current_command` → pgrep → 内容回退）。
- **恢复失败契约**：picker 恢复失败时官方打印 `Failed to resume the conversation` 并带重试命令、退出码 1；集成 UI 应展示而非吞掉该错误。
- **数据所有权**：官方明示 JSONL 为内部格式（"scripts that parse these files directly can break on any release"），集成层需隔离解析代码、设计格式变化容忍（schema 校验/降级），或以官方脚本接口（`claude -p --resume <id> --output-format json`）替代直接解析。
- **会话名 vs 标题 vs ID**：v2.1.196+ 未命名会话也有默认显示名（`my-app-3f` 风格），但默认名和 AI 生成标题都不是 resume handle——`claude --resume <name>` 只匹配用户设置的名字。集成层恢复应以 session ID 为准。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/sessions | 官方文档 | `--continue`/`--resume`/`--from-pr`/`/resume` 命令、picker 键位（Ctrl+A/W/B/R）、作用域限制（当前目录+worktrees）、`~/.claude/projects/<project>/<session-id>.jsonl` 存储、JSONL 格式可变警告、恢复还原语义、resume-from-summary 对话框 |
| https://code.claude.com/docs/en/vs-code | 官方文档 | VS Code 扩展 Session history 按钮（Local/Web 双 tab、关键词/时间浏览、AI 标题、重命名/删除）、`vscode://anthropic.claude-code/open?session=` URI、扩展与 CLI 共享历史 |
| https://github.com/craftzdog/tmux-claude-session-manager | 源码仓库 | tpm 插件、`prefix+u` agent picker、`prefix+y` launcher、`claude agents --json` 状态真相源、pid→tty→pane 关联、capture-pane 实时预览、tmux ≥3.2/fzf/jq/CC ≥2.1.139 依赖 |
| https://github.com/MaxGhenis/tmux-claude-code | 源码仓库 | tpm 插件、`prefix-C-r` 关键词恢复、`prefix-C-b` fzf 浏览、`cc list/detect/resume` CLI、JSONL `cwd` 字段交叉引用解码、`env -u CLAUDECODE`、三层 pane 检测 |
| https://registry.npmjs.org/@yyyeader/claude-recall | 包注册表 README | `cr()` shell wrapper 完整代码、fzf 浏览/关键词/`-s` 前缀恢复、greedy path resolution 算法、`-j` JSON 输出、Node ≥18 |
| https://pypi.org/project/claude-code-tools/0.2.5/ | 包注册表 README | `find-claude-session`、`fcs()` eval wrapper、`--shell` 模式目录持久化、`claude -r` 恢复、`fcs-function.sh`、交互操作菜单 |
| https://docs.rs/crate/clauhist/1.0.1 | 包注册表 README | 读 `~/.claude/history.jsonl`、fzf 交互、`✓/✗` 目录存在标记、`eval "$(clauhist init zsh)"` 集成、`cd -` 返回 |
| https://docs.rs/crate/ccboard/0.16.3 | 包注册表 README | TUI+Web 双形态、History/Sessions/Analytics tab、`ccboard web` 端口 3333、`ccboard export`、`ccboard setup` 注入 hooks、100% TUI parity |
| https://registry.npmjs.org/cc-history-viewer | 包注册表 README | 浏览器 dashboard、端口 3080、token 趋势/项目 donut/成本分解、Project History 侧栏、Live 模式、JSON API（/api/projects 等）、`--no-open` |
| https://github.com/eidos-agi/resume-resume | 源码仓库 | MCP server + TUI、`cr` paste box（命令/ID/原文）、SQLite FTS5（p50 6ms）、跨工具合并（Codex）、`boot_up` 崩溃恢复、dirty repo 扫描、`session_summary` Haiku 缓存 |
| https://github.com/anthropics/claude-code/issues/49095 | 社区 issue | 跨项目全局会话历史请求、72 会话/14 项目只见 1-3 条、All Projects tab 设计、**Closed as not planned**、关联 4 个相关 issue、第三方 `agsoft.claude-history-viewer` |
| https://larcombe.tech/blog/claude-plus-project-chooser.html | 技术博客 | 单 bash 脚本项目选择器、fzf+python3、项目级 `claude --continue` 恢复、`+ NEW PROJECT`/`~ NO PROJECT` 选项、解析 mangled 目录名 |
| https://github.com/anthropics/claude-code/issues/60610 | 社区 issue（搜索摘要，未直接抓取） | 扩展会话缺失于历史下拉——磁盘有 `session.jsonl` 但未追加 `~/.claude/history.jsonl`；10 会话仅 4 个被索引 |
| https://github.com/anthropics/claude-code/issues/56104 | 社区 issue（搜索摘要，未直接抓取） | 扩展 v2.1.126 回归：侧栏/`/resume` 无历史会话，CLI `claude --resume` 正常 |
| https://github.com/anthropics/claude-code/issues/50170 | 社区 issue（搜索摘要，未直接抓取） | Windows 映射网络驱动器无会话——`fs.realpath()` 解析为 UNC 导致项目 key 哈希不一致（`--server-share-*` vs `X--*`） |
| https://github.com/imprakharshukla/claude-launcher | 源码仓库（搜索摘要，未直接抓取） | TUI（`c` 命令）fzf 搜索全部会话、tmux/Ghostty 恢复、书签、fork、活跃指示 |

## 检索缺口与后续建议

- **zellij 集成**：本次检索未获直接可验证的 zellij 插件案例；搜索结果摘要提及 npm 包 `@halooojustin/cch`（"Claude Code History"，自然语言搜索后经 AI 在 Zellij 或 tmux 中恢复）与 `@txmxthy/rvu`（workmux 沙箱容器恢复），均未直接抓取验证。如需覆盖 zellij 可补充检索。
- **rofi picker**：未发现 rofi 集成案例——桌面 Linux 场景的会话选择基本被 fzf 与 TUI 覆盖。
- **官方 history.jsonl 格式**：clauhist 与 issue #60610 均引用该文件但官方 sessions 文档未描述其结构，需另行验证（可与本系列 D2-transcript-jsonl-format.md 对照）。
- **版本时效**：本报告引用的命令/行为以 2026-08-01 检索到的官方文档为准；Claude Code 迭代较快（文内 min-version 标注：2.1.139/2.1.169/2.1.191/2.1.196/2.1.198/2.1.203/2.1.211/2.1.216），集成实现时需复核最新版本。
