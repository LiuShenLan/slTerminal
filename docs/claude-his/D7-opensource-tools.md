# 会话管理生态实践 — 开源 session 管理工具

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论
> 检索方式: 5 组关键词 WebSearch + 14 个来源页面 WebFetch 全文提取；star 数均为 GitHub 页面截至 2026-08-01 显示值

## 生态总览

Claude Code 所有会话以 JSONL 形式存储于 `~/.claude/projects/<项目编码名>/<session-id>.jsonl`（官方文档确认），这成为整个生态的共同数据源。社区工具几乎全部以**只读方式扫描该目录**，提取 session id 后生成或执行 `claude --resume <id>`（短别名 `claude -r <id>`）实现恢复——这是所有工具与 `claude --resume` 的集成范式。生态分四类：会话管理（TUI/桌面工作台）、历史查看（web GUI/桌面 app）、搜索（FTS/语义）、备份导出。

## 关键发现

### 发现 1: 官方 resume 机制是生态集成底座
- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 检索)
- 详情: 官方提供 `claude --continue`（恢复当前目录最近会话）、`claude --resume`（交互式选择器）、`claude --resume <name>`（按名称）、`claude --resume <session-id>`（按 ID，含 headless `-p` 会话）、`claude --from-pr <number>`、会话内 `/resume` 切换。**关键约束**：session ID 查找限定当前项目目录及其 git worktrees——从错误目录执行会报 "No conversation found with session ID"，这正是社区工具普遍在 resume 前 `cd` 到原项目目录的原因。恢复时还原 conversation history、model、agent、permission mode（除 `plan`/`bypassPermissions`）、active goals、未过期 scheduled tasks；`--mcp-config`/`--settings`/`--plugin-dir` 等 flags 需重新传入。`--resume --fork-session` 可分支副本。选择器快捷键：`Space` 预览、`Ctrl+R` 重命名、`/` 搜索、`Ctrl+A` 全部项目、`Ctrl+B` 当前分支。会话可 `-n <name>` 命名、`/rename` 会话内改名。

### 发现 2: 会话管理类 — CLI/TUI 与桌面工作台
- 来源: https://github.com/hex/claude-sessions | https://github.com/pradipta/wallfacer | https://github.com/latte3cup/claude-session-manager (2026-08-01 检索)
- 详情:
  - **hex/claude-sessions（工具名 `cs`）**：Bash + Rust TUI 混合。31 stars。每会话预分配 conversation UUID 存入 `.cs/local/state`，`cs <name>` 经 `claude --resume <uuid>` 精确恢复（避免 `--continue` 误选兄弟会话）；同时注入 `--name` 与每会话 `/color`。特性：OS keychain 密文、每会话 git 版本化 + 崩溃恢复 shadow ref、PID 会话锁 + statusline 心跳、跨会话搜索、tag/归档、tmux spawner、`cs -doctor`/`cs -usage`。安装 `bash -c "$(curl -fsSL .../install.sh)"`。**Windows 分两档**：WSL2 全支持（"Everything works: session launch, the tmux spawner, secrets, and the TUI"）；Git Bash/MSYS2 仅会话簿记（"cannot launch Claude Code or use the tmux spawner"），secrets 有 `powershell.exe` 时走 Windows Credential Manager。MIT。
  - **pradipta/wallfacer**：Go 写的终端会话管理器（全屏 TUI + 一次性 CLI）。10 stars。只读索引 `~/.claude/projects/`，元数据存 SQLite（`~/.local/share/wallfacer/`），"it never touches the agents' files"。支持 rename/tag/group/fuzzy search/安全删除（`rm` 进 trash，仅 `--purge` 永久）。多 agent（Claude Code、Cursor CLI、Kiro CLI、Codex），pluggable agent adapters。**无 Windows 支持声明**（安装仅 Homebrew macOS/Linux + `go install`）。resume 走自研 `wallfacer resume <ref>`（"Reopen a session in its original directory"），非直接调 `claude --resume`。MIT。
  - **latte3cup/claude-session-manager**：Rust + Tauri 2.x + xterm.js + Python FastAPI + React/TS 桌面工作台（基于 PriuS2/RemoteCode 迁移自 Electron，~20MB 应用 / ~36MB 内存）。1 star。管理 Claude Code/OpenCode/Kilo Code 多会话 + 文件浏览器 + Git 面板 + Monaco 编辑器（LSP）。**session suspend/resume 经 `--resume`/`-s`**（"Suspend sessions and resume with CLI session recovery"）。Windows 证据：README 提供 `.venv\Scripts\pip install ...`（Windows 激活语法）+ 仓库内 `setup.ps1`/`start-dev.ps1`/`build-release.ps1` PowerShell 脚本 + Tauri 运行时列 "System WebView2/WebKit"。MIT。
- 补充: **StanislavBG/claude-code-session-manager**（Electron "cockpit"：多 tab 终端 + 调度队列 + 本地 Whisper 语音，`npx claude-code-session-manager@latest`，**Linux/macOS only**，MIT）与 **Divyanshubansaldb/claude-code-session-manager**（纯 Bash，加 `/session:save` `/session:retrieve` `/session:update` `/session:delete` `/session:clean` `/session:id` slash 命令，存 JSON 于 `~/.claude/sessions/`，macOS/Linux，MIT）——来源: https://github.com/StanislavBG/claude-code-session-manager | https://github.com/Divyanshubansaldb/claude-code-session-manager。

### 发现 3: 历史查看类 — web GUI 与桌面 app（规模最大品类）
- 来源: https://github.com/jhlee0409/claude-code-history-viewer | https://github.com/daaain/claude-code-log | https://github.com/cppalliance/claude-code-chat-browser | https://pkg.go.dev/github.com/shivamstaq/ccview (2026-08-01 检索)
- 详情:
  - **jhlee0409/claude-code-history-viewer（CCHV）**：**生态最大项目，2.0k stars**。Rust（Tauri v2）+ React 19/TypeScript/Zustand/Vite，5 语言 i18n。统一查看 28 个 AI 编程助手（Claude Code、Copilot、Gemini CLI、Codex、Cline、Cursor、Aider、OpenCode 等）。功能：会话浏览（worktree 分组）、全局搜索、token/cost 分析仪表盘、Session Board、settings/MCP 管理、实时文件监听。**Windows 桌面 x64 `.exe` / `.zip (portable)` 明确提供**（平台徽标 "macOS | Windows | Linux"）。**v1.15.0 起 "Project name and the `claude --resume` working directory are now resolved from session metadata instead of the lossy folder encoding"**——即从会话元数据解析 resume 工作目录，解决编码目录名问题；上下文菜单可复制 "session ID, resume command, file path"，Codex 会话复制的命令带 `cd '<cwd>' &&` 前缀。另提供 headless server 模式（`cchv-server --serve`，token auth + SSE）。MIT。
  - **daaain/claude-code-log**：Python CLI，**1.2k stars**。交互 TUI + HTML/Markdown 导出：HTML 为顶层项目索引页 + 单会话页 + 缩放时间线；Markdown 为 GitHub-Flavored + `--detail full/high/low/minimal/user-only` 五档 + `--compact`（供喂回 LLM）。日期范围自然语言过滤、token 统计、commit SHA 链接。**TUI 按 `c` 键执行 `claude -r <sessionId>` 恢复会话**。安装 `pip install claude-code-log` 或 `uvx claude-code-log@latest`。无 Windows 声明（社区扩展 archive-session 标注 Cross-platform macOS/Windows-MSYS）。MIT。
  - **cppalliance/claude-code-chat-browser**：Python Flask web GUI + CLI 双形态。1 star。项目卡片仪表盘、全文搜索、工具调用渲染、可折叠 thinking 块、明暗主题。**导出最强**：CLI 导出 Markdown + YAML frontmatter（含 token/tool/thinking/model）；web/API 批量 zip（全量/增量/最近一天），zip 命名 `claude-code-export-last-MM-DD-YYYY-MM-DD.zip`；`--since last|incremental` 模式；退出码 0/1/2 语义。**Windows 明确支持**：README 激活行 `venv\Scripts\activate` + CI 跑 "Ubuntu and Windows (Python 3.12, Node 20)"。REST API 机器可读错误码。BSL-1.0。无 resume 功能（纯只读）。
  - **shivamstaq/ccview**：Go（bubbletea/lipgloss/glamour + modernc.org/sqlite 纯 Go）TUI，v1.0.1（2026-04-04）。Claude Code + OpenCode 双数据源（OpenCode 读 SQLite DB）。分栏 TUI + `--web`（端口 3333）+ 导出向导（自包含 HTML/Markdown/JSONL，含 sub-agent 目录 + index.html）。**Windows prebuilt `windows_amd64.zip` 明确提供**（PowerShell 安装命令写入 README，装到 `%LOCALAPPDATA%\ccview`）。纯查看器，**无 resume 命令**。MIT。
- 补充: **drewburchfield/claude-code-chat-explorer**（SQLite FTS5 全文搜索 + WebSocket 实时更新 + 移动端适配，端口 9876，尊重 `cleanupPeriodDays` 保留期，**内置 MCP server** 暴露 `search_conversations`/`search_within_conversation`）| **wonderomg/claude-history-viewer**（npm `claudecode-history-viewer`，Node 18+，端口 3747，支持 Claude Code/Cursor/Codex 三源，`npx -y claudecode-history-viewer`）| **binggg/Claude-Code-Web-GUI**（纯浏览器运行，File System Access API，零服务器，Chrome/Edge，支持 Gist 分享，中英双语）| **josephyaduvanshi/claude-history-manager（Chronicle）**（macOS 原生 SwiftUI + Linux CLI，SQLite FTS5，`claude --resume <uuid>` 终端恢复，iCloud 元数据同步，transcripts 不出机器）| **kwhitley/claude-code-viewer**（Svelte API+UI，约 2 stars，小项目）——来源: https://github.com/drewburchfield/claude-code-chat-explorer | https://www.npmjs.com/package/claudecode-history-viewer | https://github.com/binggg/Claude-Code-Web-GUI | https://github.com/JosephYaduvanshi/claude-history-manager | https://opencollective.ecosyste.ms/projects/375064

### 发现 4: 搜索类 — FTS 与语义检索并存的"找回会话"工具
- 来源: https://github.com/vmax/retrace | https://github.com/cc-deck/cc-session | https://github.com/sinzin91/search-sessions | https://github.com/akatz-ai/cc-conversation-search | https://github.com/esc5221/sessionhub (2026-08-01 检索)
- 详情:
  - **vmax/retrace**：Python（uv 打包），FTS5 SQLite 索引（`~/.cache/retrace/index.db`）。6 stars（早期项目，2 commits）。搜索 Claude Code + Codex 双源，`unicode61 remove_diacritics` 分词、bm25 排序、`--exact` 模式。**resume 集成最规范**：`retrace resume <ref>` 生成默认模板 `claude --resume {id}`（`RETRACE_CLAUDE_RESUME` 可覆写），且 **"Resuming chdirs into the directory the session started in"**——明确针对官方 "No conversation found with session ID" 约束。摘要功能 shell 出 `claude -p --no-session-persistence`。隐私姿态：无网络无 daemon，"never writes to a transcript file"。实测 ~60-95ms 全命令搜索（475 会话 / 20,685 消息）。注意 Claude Code `cleanupPeriodDays`（默认 30）裁剪后只能索引磁盘现存内容。无 Windows 声明。MIT。
  - **cc-deck/cc-session**：Rust TUI。13 stars。2000+ 会话 <500ms 扫描；300ms debounce 后台深度搜索全文；会话查看器（syntect 语法高亮、markdown 表格、点击 URL）；`--since 7d/2w/1m`、`--last N` 时间过滤；`Alt-G` 项目分组。**resume 生成 `cd '<project-path>' && claude -r <session-id>`**（正确引号路径），复制到剪贴板或 Enter 直接执行。安装 Homebrew / install.sh / `cargo install --git`。**无 Windows 支持**（剪贴板支持仅 "macOS, Linux X11/Wayland"）。MIT。
  - **sinzin91/search-sessions**：Rust 单二进制，**无索引步骤、无数据库**。36 stars。双模式：index search（元数据 ~18ms）+ deep search（全文 ~280ms ripgrep / ~1s Rust fallback）。`--project`/`--since`/`--until`/`--date today` 过滤，OpenClaw 支持。**以 skill 形态装入 Claude Code**（`/search-sessions` slash 命令），结果打印 `cd ~/Projects/myapp && claude -r <uuid>`。安装 Homebrew (macOS/Linux) / `cargo install search-sessions`。无 Windows 声明。MIT。
  - **akatz-ai/cc-conversation-search**：Python，26 stars。**"hybrid extraction and JIT indexing"**：搜索前即时建索引，零 AI 调用；混合索引（全文 + 智能提取摘要）；语义层面用 SQLite FTS5（README 明示 embeddings 属未来贡献区）。日历过滤 `--date yesterday/--since/--until`；排除 meta-conversation（搜索结果不混入搜索工具自身调用）。**resume 返回 `cd <project-path> && claude --resume <session-id>`**；`resume <MESSAGE_UUID>` 子命令直达。安装 `uv tool install cc-conversation-search` 或插件 `/plugin marketplace add akatz-ai/cc-conversation-search`。无 Windows 声明（文档路径全 Unix 风格）。MIT。
  - **esc5221/sessionhub**：Python，6 stars。跨机聚合：**MIRROR（原始日志原地不动）/ DIGEST（精简会话 gzip）/ INDEX（每会话一行元数据）三层**，~10GB transcripts → 数百 MB；SSH 同步 + launchd/systemd 定时拉取。Claude Code skill 集成（`/sessionhub find ...`）。**无 resume 命令，无 Windows 声明**（sync 定时器仅 macOS/Linux）。MIT。
- 补充: **lee-fuhr/claude-session-index**（SQLite FTS5 全索引 + 跨会话综合 + 以 Claude Code skill 安装，`sessions "webhook debugging"` 查询毫秒级）| **negipo/cclens**（Rust CLI → SQLite，branch/date/project 过滤、OR 搜索语法、Markdown 导出、三个内建 skill: search/export/resume）| **aurora-thesean/claude-session-tools**（纯 Python 零依赖，**fork-aware：按 `parentUuid` 把 JSONL 解析成树而非线性日志**——纠正生态普遍假设）| **JoniMartin27/claudescope**（`npx claudescope-cli`，零配置零网络浏览器仪表盘，多 CLI 支持 Codex/Cursor/Aider/Gemini/Copilot）——来源: https://github.com/lee-fuhr/claude-session-index | https://github.com/negipo/cclens | https://github.com/aurora-thesean/claude-session-tools | https://github.com/JoniMartin27/claudescope

### 发现 5: Web UI 与备份导出类
- 来源: https://github.com/Chill-AI-Space/claude-session-manager | https://github.com/Ethan-YS/ccvault (2026-08-01 检索)
- 详情:
  - **Chill-AI-Space/claude-session-manager**：Next.js 16（App Router/Turbopack）+ TypeScript + React 19 + better-sqlite3 的 localhost web UI（"email client for your Claude conversations"）。7 stars，183 commits。双栏布局：左会话列表右完整对话；markdown 渲染 + 可折叠工具调用；浏览器内回复（SSE 流式）；token 分析、Gemini 语义深搜、自动重试、通知。**Windows 明确支持**："Works on macOS, Linux, and Windows"，平台表 Windows 列：Browse & read Yes / Reply Yes / Live session detection Partial / Open in terminal No；**"On Windows, Claude Code stores sessions in `%USERPROFILE%\.claude\projects\`"** 自动探测。**resume 集成独特**：浏览器回复 = 一次性进程 `claude -p "your message" --resume <session-id> --max-turns 80`（需 `--dangerously-skip-permissions` 可选）。**仓库页面无 license 声明**。
  - **xreader/ai-session-manager**：Node.js 零依赖（无需 npm install）只读 web dashboard，端口 4317。列表 + Gantt 时间线 + kanban 三视图；进程检查判定 running/idle/closed 实时状态；首尾 prompt 生成 goal/latest-state 摘要；图片画廊、笔记、会话恢复。100% 本地，不改 Claude 文件。
  - **Ethan-YS/ccvault**：Python 零依赖。**备份/浏览/搜索/导出四合一**：浏览器 UI（文件夹式侧栏 + 聊天气泡视图 + 可折叠工具调用 + "chat only" 模式）、过滤、归档、自动去重；导出单会话/整项目/全部为 Markdown 或 zip。100% 本地，从不修改原始 transcript。

### 发现 6: 编辑器集成类 — VS Code 侧栏管理器
- 来源: https://github.com/vishalguptax/claude-code-manager (2026-08-01 检索)
- 详情: **Claude Code Manager**（marketplace id `vishalguptax.claude-manager`，原 Claude Manager）。TypeScript/Preact + `@preact/signals` + valibot，27 stars，591 commits，1400+ 单测，Apache 2.0。100% 本地零网络。会话管理（resume/continue/restore-workspace/pin/rename/fork/import/export）、全文搜索、skill/命令/hooks/MCP 管理、多账号、token 用量。**Windows 覆盖**：快捷键 "Ctrl+Alt+C on Windows/Linux"，FAQ 明确 "cmd.exe (no shell integration) is covered by the hook path"。**resume 检测双通道**：① `SessionStart` hook 自动装入全局 `~/.claude/settings.json`，记录 `{sessionId, ppid}` 到 `~/.claude/.claude-manager/active-sessions.json`，扩展用 `vscode.Terminal.processId` 对 PID 匹配定位宿主终端；② 带 shell integration 的 VS Code 终端中 "`claude --resume <id>` typed at any prompt is caught directly from the shell-execution event"。`sessions.resumeIn` 设置决定 resume 打开位置（auto/terminal/extension/ask）。

## 生态成熟度评估

1. **品类规模**：历史查看器最成熟——CCHV（2.0k stars）与 claude-code-log（1.2k stars）是事实标准；搜索类次之（多为 <50 stars 的新项目，但技术路线多元：FTS5、免索引 grep、语义/JIT）；会话管理器最小众（多 <50 stars，尚无主导者，属蓝海）。
2. **数据契约高度统一**：所有工具读 `~/.claude/projects/` JSONL + 提取 session id + 生成 `claude --resume <id>` / `claude -r <id>`，无任何工具修改 Claude 自有文件——"只读 + 外部索引"是生态共识。新工具接入成本极低。
3. **Windows 支持是明显短板**：明确声明 Windows 的仅 6 家——CCHV（x64 exe/zip 桌面）、ccview（windows_amd64.zip）、Chill-AI-Space（%USERPROFILE% 自动探测）、cppalliance chat-browser（CI 含 Windows）、hex/claude-sessions（仅 WSL2/Git Bash 降级）、claude-code-manager（VS Code 跨平台）。wallfacer、cc-session、retrace、search-sessions、sessionhub、cc-conversation-search、claude-code-log 均无 Windows 声明——**对 Windows 原生终端模拟器（本项目）是空档**。
4. **与本项目技术栈（Rust + Tauri 2）最相关的参照**：CCHV（Rust/Tauri v2 桌面，2.0k stars，Windows 验证可行）与 latte3cup/claude-session-manager（Tauri 2 + xterm.js + FastAPI，验证 Tauri 壳 + 会话恢复的组合可行）。二者证明：Tauri 2 桌面壳 + 读 `~/.claude/projects/` + `claude --resume` 命令行恢复，是已验证的组合路径。
5. **共性坑位**（本项目设计须规避）：① session ID 查找限定项目目录——resume 必须 `cd` 回会话起始目录（retrace 明示、cc-session 生成 `cd && claude -r`）；② 编码目录名不可靠——CCHV v1.15.0 改为从会话元数据解析工作目录；③ `cleanupPeriodDays`（默认 30）裁剪 transcripts——索引存在时效边界；④ fork/子代理结构——aurora-thesean 指出 JSONL 实为 `parentUuid` 树而非线性日志。

## 工具总表

| 工具 | 仓库 | stars (截至 2026-08-01) | 分类 | 语言 | 安装 | Windows | resume 集成 |
|------|------|------|------|------|------|---------|-------------|
| cs (claude-sessions) | github.com/hex/claude-sessions | 31 | 会话管理 | Bash + Rust TUI | curl 一键脚本 | WSL2 全支持 / Git Bash 降级 | `claude --resume <预分配 uuid>` |
| wallfacer | github.com/pradipta/wallfacer | 10 | 会话管理 | Go | brew / go install | 无声明 | 自研 `wallfacer resume <ref>` |
| claude-session-manager (latte3cup) | github.com/latte3cup/claude-session-manager | 1 | 会话管理(桌面) | Rust+Tauri 2 + Python FastAPI | 源码构建 / cargo tauri build | 有（.ps1 脚本 + WebView2） | suspend/resume 经 `--resume`/`-s` |
| claude-code-session-manager (StanislavBG) | github.com/StanislavBG/claude-code-session-manager | — | 会话管理(桌面) | Electron | `npx claude-code-session-manager@latest` | Linux/macOS only | — |
| claude-code-session-manager (Divyanshubansaldb) | github.com/Divyanshubansaldb/claude-code-session-manager | — | 会话管理(slash) | Bash | 手动安装 | macOS/Linux | `/session:retrieve` 等 |
| CCHV | github.com/jhlee0409/claude-code-history-viewer | 2000 | 历史查看(桌面) | Rust+Tauri 2 + React 19 | brew cask / Release exe/zip | **Windows x64 .exe/.zip 明确** | 复制 resume 命令；v1.15.0 元数据解析 cwd |
| claude-code-log | github.com/daaain/claude-code-log | 1200 | 历史查看/导出 | Python | `pip install claude-code-log` / uvx | 无声明 | TUI 按 `c` → `claude -r <sessionId>` |
| claude-code-chat-browser | github.com/cppalliance/claude-code-chat-browser | 1 | 历史查看/导出 | Python Flask + JS | pip + `python app.py` | **CI 含 Windows + venv\Scripts 激活** | 无（纯只读） |
| ccview | github.com/shivamstaq/ccview | —（pkg.go.dev 不显示） | 历史查看(TUI) | Go (bubbletea) | go install / prebuilt | **windows_amd64.zip 明确** | 无（纯查看器） |
| claude-code-chat-explorer | github.com/drewburchfield/claude-code-chat-explorer | — | 历史查看(web) | JS (SQLite FTS5) | 自托管 | — | 无（MCP server 供查询） |
| claude-history-viewer | npmjs.com/package/claudecode-history-viewer | — | 历史查看(web) | Node 18+ | `npx -y claudecode-history-viewer` | — | — |
| Claude-Code-Web-GUI | github.com/binggg/Claude-Code-Web-GUI | — | 历史查看(浏览器) | 纯前端 (File System Access) | 打开页面即用 | Chrome/Edge | — |
| Chronicle | github.com/JosephYaduvanshi/claude-history-manager | — | 历史查看(桌面) | SwiftUI + Linux CLI | brew/pkg/dmg/zip | macOS + Linux CLI | `claude --resume <uuid>` |
| claude-session-manager (Chill-AI-Space) | github.com/Chill-AI-Space/claude-session-manager | 7 | Web UI 管理 | Next.js 16 + TS + SQLite | npm install && npm start | **明确支持（%USERPROFILE% 探测）** | `claude -p "msg" --resume <id> --max-turns 80` |
| ai-session-manager | github.com/xreader/ai-session-manager | — | Web UI 管理 | Node.js 零依赖 | 免 npm install | — | 会话恢复（只读） |
| cc-conversation-search | github.com/akatz-ai/cc-conversation-search | 26 | 搜索 | Python | `uv tool install cc-conversation-search` / plugin | 无声明 | `cd <path> && claude --resume <id>` |
| retrace | github.com/vmax/retrace | 6 | 搜索 | Python | `uv tool install git+...` | 无声明 | `retrace resume` → `claude --resume {id}`（chdir 起始目录） |
| cc-session | github.com/cc-deck/cc-session | 13 | 搜索(TUI) | Rust | brew / install.sh / cargo | 无（剪贴板仅 mac/Linux） | 生成 `cd '<path>' && claude -r <id>` |
| search-sessions | github.com/sinzin91/search-sessions | 36 | 搜索 | Rust | brew (mac/Linux) / cargo | 无声明 | 结果附 `cd ... && claude -r <uuid>` |
| sessionhub | github.com/esc5221/sessionhub | 6 | 搜索/跨机聚合 | Python | `uv tool install git+...` | 无声明（定时器仅 mac/Linux） | 无（skill 查询） |
| claude-session-index | github.com/lee-fuhr/claude-session-index | — | 搜索 | — (SQLite FTS5) | Claude Code skill | — | skill 返回会话 |
| cclens | github.com/negipo/cclens | — | 搜索 | Rust CLI | — | — | skill resume |
| claude-session-tools | github.com/aurora-thesean/claude-session-tools | — | 搜索(解析层) | Python 纯函数 | — | — | fork-aware 树解析 |
| ccvault | github.com/Ethan-YS/ccvault | — | 备份导出 | Python 零依赖 | — | — | 无 |
| Claude Code Manager | github.com/vishalguptax/claude-code-manager | 27 | 编辑器集成 | TypeScript/Preact | `code --install-extension vishalguptax.claude-manager` | **支持（cmd.exe hook 路径）** | resume/continue 一键 + PID 定位终端 |

> "—" = 来源页面未提供该信息，不作推测。star 数仅列 WebFetch 页面明确显示者。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/sessions | 官方文档 | resume/continue/from-pr 全部命令、JSONL 存储、session ID 目录约束、恢复内容边界、选择器快捷键 |
| https://github.com/hex/claude-sessions | 源码仓库 | cs 会话管理器：UUID 预分配 resume、WSL2/Git Bash 两档 Windows 支持、31 stars |
| https://github.com/pradipta/wallfacer | 源码仓库 | Go TUI 会话管理器：SQLite 只读索引、多 agent、10 stars、无 Windows 声明 |
| https://github.com/latte3cup/claude-session-manager | 源码仓库 | Tauri 2 + xterm.js 桌面工作台：--resume/-s 恢复、PowerShell 脚本证据、1 star |
| https://github.com/StanislavBG/claude-code-session-manager | 源码仓库 | Electron cockpit：Linux/macOS only、npx 安装 |
| https://github.com/Divyanshubansaldb/claude-code-session-manager | 源码仓库 | Bash slash 命令会话管理（/session:save 等） |
| https://github.com/jhlee0409/claude-code-history-viewer | 源码仓库 | CCHV：2.0k stars、Tauri v2、Windows exe/zip、v1.15.0 resume cwd 元数据解析、28 助手 |
| https://github.com/daaain/claude-code-log | 源码仓库 | 1.2k stars、Python TUI+HTML/MD 导出、`c` 键 `claude -r`、--detail 五档 |
| https://github.com/cppalliance/claude-code-chat-browser | 源码仓库 | Flask web+CLI、zip/Markdown 导出、CI 含 Windows、BSL-1.0 |
| https://pkg.go.dev/github.com/shivamstaq/ccview | 包文档 | Go TUI v1.0.1(2026-04-04)、windows_amd64.zip 安装命令、纯查看器 |
| https://github.com/drewburchfield/claude-code-chat-explorer | 源码仓库 | SQLite FTS5 web、端口 9876、MCP server、cleanupPeriodDays 尊重 |
| https://www.npmjs.com/package/claudecode-history-viewer | 包注册表 | Node 18+、端口 3747、CC/Cursor/Codex 三源 |
| https://github.com/binggg/Claude-Code-Web-GUI | 源码仓库 | 纯浏览器 File System Access、Gist 分享、中英双语 |
| https://github.com/JosephYaduvanshi/claude-history-manager | 源码仓库 | Chronicle：macOS SwiftUI + Linux CLI、`claude --resume <uuid>`、iCloud 同步 |
| https://github.com/Chill-AI-Space/claude-session-manager | 源码仓库 | Next.js 16 web UI、Windows 明确支持（%USERPROFILE%）、`-p --resume --max-turns 80` 回复 |
| https://github.com/xreader/ai-session-manager | 源码仓库 | Node 零依赖 dashboard、端口 4317、三视图 + 实时状态 |
| https://github.com/akatz-ai/cc-conversation-search | 源码仓库 | 26 stars、JIT 索引混合提取、日历过滤、plugin/skill 安装 |
| https://github.com/vmax/retrace | 源码仓库 | FTS5 双源搜索、resume chdir 起始目录、RETRACE_CLAUDE_RESUME 模板、6 stars |
| https://github.com/cc-deck/cc-session | 源码仓库 | Rust TUI 13 stars、2000+ 会话 <500ms、`cd && claude -r` 生成 |
| https://github.com/sinzin91/search-sessions | 源码仓库 | 36 stars、免索引双模式、/search-sessions skill、18ms/280ms 实测 |
| https://github.com/esc5221/sessionhub | 源码仓库 | MIRROR/DIGEST/INDEX 三层、SSH 跨机、6 stars |
| https://github.com/lee-fuhr/claude-session-index | 源码仓库 | SQLite FTS5 全索引 + skill 安装 |
| https://github.com/negipo/cclens | 源码仓库 | Rust CLI → SQLite、OR 搜索、三 skill |
| https://github.com/aurora-thesean/claude-session-tools | 源码仓库 | parentUuid 树解析、纠正 JSONL=线性日志假设 |
| https://github.com/JoniMartin27/claudescope | 源码仓库 | npx claudescope-cli 零配置仪表盘、多 CLI |
| https://github.com/Ethan-YS/ccvault | 源码仓库 | Python 零依赖备份/浏览/搜索/导出四合一 |
| https://github.com/vishalguptax/claude-code-manager | 源码仓库 | VS Code 扩展：27 stars、PID+hook 双通道 resume 检测、Apache 2.0 |
| https://opencollective.ecosyste.ms/projects/375064 | 生态镜像 | kwhitley/claude-code-viewer：Svelte 本地查看器、约 2 stars |
