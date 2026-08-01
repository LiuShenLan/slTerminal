# D7-opensource-tools 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D7

---

## 错误 1: claude-code-log 的 HTML 导出"自包含"表述无来源支撑

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D7-opensource-tools.md` (第 29 行)
- **原声称**: "交互 TUI + HTML/Markdown 导出：HTML 为自包含项目索引页 + 单会话页 + 缩放时间线"
- **错误类型**: 来源不支撑
- **正确信息**: claude-code-log README 确认的 HTML 输出为 "Top level index with project cards and statistics"（顶层项目索引页，路径 `~/.claude/projects/index.html`）+ "Generate separate HTML files for each session with navigation links"（单会话页）+ "Generate an interactive, zoomable timeline grouped by message times"（缩放时间线）。README 全文未以任何形式（"self-contained"/"standalone"）声明 HTML 文件自包含——"自包含"属性属文档作者推断。对照同文档 ccview 条目：其"自包含 HTML"有 README 原文 "HTML - Dark-themed, **self-contained**, syntax-highlighted" 支撑，而 claude-code-log 无对应原文。
- **反证来源**: https://raw.githubusercontent.com/daaain/claude-code-log/main/README.md — "Generate separate HTML files for each session with navigation links"、"Top level index with project cards and statistics"、"Generate an interactive, zoomable timeline grouped by message times"；全文检索无 "self-contained" / "standalone" 字样

## 无法验证

- **第 5 行** "5 组关键词 WebSearch + 14 个来源页面 WebFetch 全文提取"：检索过程声称，外部无法复现。来源清单实际登记 28 个 URL，与 "14 个来源页面" 的对应关系未说明（可能部分来源经搜索摘要而非全文提取，文档未区分）。原因：WebFetch 无法获知原始检索过程。

## 核查说明

除上述 1 项外，其余全部声称均与来源一致，逐项核验结果如下（共 28 个来源 URL、约 70 项具体声称）：

- **官方文档**（code.claude.com/docs/en/sessions）：`--continue`/`--resume`/`--resume <name>`/`--resume <session-id>`/`--from-pr`/会话内 `/resume`、session ID 查找限定当前项目目录及 git worktrees、错误消息 "No conversation found with session ID"、恢复内容边界（history/model/agent/permission mode 除 plan/bypassPermissions/active goal/scheduled tasks）、`--mcp-config`/`--settings`/`--plugin-dir` 需重传、`--fork-session`、选择器快捷键（Space/Ctrl+R// /Ctrl+A/Ctrl+B）、`-n`/`/rename`、JSONL 存储路径（`~/.claude/projects/<项目编码名>/<session-id>.jsonl`，编码=非字母数字替换为 `-`）——全部吻合
- **star 数**（GitHub API 精确值 vs 文档）：hex/claude-sessions 31、wallfacer 10、latte3cup 1、CCHV 1979（页面显示 2.0k，与文档"2.0k stars"一致）、claude-code-log 1180（页面显示 1.2k，一致）、chat-browser 1、Chill-AI-Space 7（+183 commits）、cc-conversation-search 26、retrace 6（+2 commits）、cc-session 13、search-sessions 36、sessionhub 6、claude-code-manager 27（+591 commits）、kwhitley/claude-code-viewer 2——全部一致；全部仓库存在、未归档、未删除
- **工具声称**（每个仓库 README 逐条比对）：hex（UUID 预分配 `.cs/local/state`、`--name`+`/color`、keychain、shadow ref、PID 锁+statusline heartbeat、`cs -search`/`cs -doctor`/`cs -usage`、tmux spawner、WSL2 全支持/Git Bash 降级原文、Windows Credential Manager、MIT）、wallfacer（Go、SQLite 路径、'it never touches the agents' files'、trash 删除、四 agent、无 Windows、自研 resume、MIT）、latte3cup（Tauri 2+xterm.js+FastAPI、PriuS2/RemoteCode 迁移、20MB/36MB、--resume/-s 原文、.ps1 脚本+WebView2、MIT）、StanislavBG（Electron cockpit、npx 安装、Linux/macOS only、MIT）、Divyanshubansaldb（纯 Bash、六个 `/session:*` 命令、`~/.claude/sessions/` JSON、macOS/Linux、MIT）、CCHV（Tauri v2+React 19、5 语言、28 助手、worktree 分组、v1.15.0 元数据解析 cwd 原文、Codex 复制命令 `cd '<cwd>' &&` 前缀、`cchv-server --serve`+token+SSE、Windows x64 exe/zip、MIT）、claude-code-log（Python TUI、`--detail` 五档、`--compact`、自然语言日期过滤、commit SHA 链接、`c` 键 `claude -r`、pip/uvx 安装、archive-session 扩展 macOS/Windows-MSYS 标注、MIT）、chat-browser（Flask 双形态、导出 zip 命名/`--since last|incremental`/退出码 0/1/2、`venv\Scripts\activate`+CI Ubuntu/Windows、BSL-1.0、纯只读）、ccview（v1.0.1 2026-04-04、bubbletea/lipgloss/glamour+modernc sqlite、`--web` 端口 3333、导出向导含 sub-agent 目录、`windows_amd64.zip`+`%LOCALAPPDATA%\ccview`、无 resume、MIT）、chat-explorer（FTS5+WebSocket+9876+cleanupPeriodDays+MCP server 两工具、MIT）、claudecode-history-viewer（npm 包、Node>=18、端口 3747、三源、npx -y）、Web-GUI（File System Access、Chrome/Edge 86+、Gist、双语、MIT）、Chronicle（SwiftUI+Linux CLI、FTS5、`claude --resume <uuid>`、iCloud 仅元数据、四安装方式、MIT）、Chill-AI-Space（Next.js 16+better-sqlite3、'email client' 定位、Windows 平台表四行、"On Windows, Claude Code stores sessions in %USERPROFILE%\\.claude\\projects\\" 原文、`-p --resume --max-turns 80`+可选 skip-permissions、页面无 license 声明）、xreader（零依赖、4317、三视图、进程检查、goal/latest-state、图片画廊/笔记/resume、100% 本地、MIT）、ccvault（零依赖、四合一、chat only、自动去重、单/整项目/全部导出、从不修改 transcript、MIT）、retrace（FTS5 索引路径、unicode61 remove_diacritics、bm25、`--exact`、`claude --resume {id}` 模板+环境变量覆写、chdir 原文、`-p --no-session-persistence`、无网络无 daemon、60-95ms/475 会话实测、无 Windows、MIT）、cc-session（2000+ 会话 <500ms、300ms debounce、syntect/表格/URL、`--since`/`--last`、Alt-G、`cd '<project-path>' && claude -r`、剪贴板仅 macOS/Linux X11/Wayland、MIT）、search-sessions（单二进制免索引、18ms/280ms/~1s 原文、--project/--since/--until/--date、OpenClaw、/search-sessions skill、`cd ~/Projects/myapp && claude -r <uuid>` 输出、MIT）、cc-conversation-search（JIT 索引零 AI 调用、混合索引、FTS5+embeddings 属贡献区、日历过滤、meta-conversation 排除、`resume <MESSAGE_UUID>`、uv/plugin 安装、无 Windows、MIT）、sessionhub（MIRROR/DIGEST/INDEX 三层原文、~10GB→数百 MB、SSH+launchd/systemd、/sessionhub find skill、无 resume 命令、`uv tool install git+...` 安装、无 Windows、MIT）、claude-session-index（FTS5 全索引+跨会话综合、skill 安装、毫秒级查询、MIT）、cclens（Rust→SQLite、branch/date/project+OR 搜索、三 skill 含 resume、MIT）、claude-session-tools（纯 Python 零依赖、parentUuid 树原文、MIT）、claudescope（npx claudescope-cli、零配置零网络、多 CLI、MIT）、claude-code-manager（marketplace id `vishalguptax.claude-manager`、Preact+signals+valibot、1400+ 单测、Apache 2.0、Ctrl+Alt+C on Windows/Linux、cmd.exe hook path FAQ 原文、SessionStart hook 自动装入全局 settings.json+`{sessionId, ppid}` 记录+`vscode.Terminal.processId` 匹配、shell-execution event 原文、sessions.resumeIn 四选项）
- **内部一致性**：正文/工具总表/来源清单三方 star 数、Windows 声明枚举（"明确声明 Windows 的仅 6 家"）、成熟度评估、共性坑位（chdir、元数据解析 cwd、cleanupPeriodDays 默认 30、parentUuid 树）——相互一致，无内部矛盾

> 注：源文档对部分"补充"工具 star 数标 "—"（来源页面未提供），实际 GitHub 页面均显示数字（如 Divyanshubansaldb 14、chat-explorer 14、Web-GUI 72、Chronicle 46、ai-session-manager 3、ccvault 7、claude-session-index 27、cclens 1、claudescope 0、claude-session-tools 0）；此为文档"star 数仅列 WebFetch 页面明确显示者"约定下的保守省略，非错误，供知悉。
