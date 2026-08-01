# 历史会话查询方式 — 文件系统扫描与第三方查询

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论

## 关键发现

### 发现 1: 存储约定已形成社区共识 — `~/.claude/projects/<编码项目路径>/<session-uuid>.jsonl`

- 来源: https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md (2026-08-01 检索) / https://github.com/daymade/claude-code-skills/blob/main/daymade-claude-code/claude-code-history-files-finder/SKILL.md (2026-08-01 检索)
- 详情: 目录名 = 会话启动时工作目录的绝对路径、`/` 替换为 `-`（如 `-Users-you-code-my-project` 对应 `/Users/you/code/my-project`），**不是 basename**。每个会话一个 `.jsonl` 文件、文件名即 session UUID。文件内每行一个 JSON 事件，相关事件类型为 `user`（提示词）、`assistant`（响应，含 `tool_use` 块）、`summary`（一行摘要，即 `/resume` 选择器所显示内容）。每个事件携带 `uuid`、`parentUuid`、`timestamp`（ISO 8601）、`sessionId`、`cwd` 字段。该目录约定被 claudeview（`~/.claude/projects/<hash>/<session-id>.jsonl`）、session-index（`%USERPROFILE%\.claude\projects`，Windows）、ccvault（`~/.claude/projects` 自动检测）、cc-sessions（"valid UUID filenames" 过滤）等多工具确认。

### 发现 2: 三类工具路线 — 直接扫描（无索引）/ SQLite 索引 / UI 查看器

- 来源: https://github.com/sinzin91/search-sessions (2026-08-01 检索) / https://github.com/negipo/cclens (2026-08-01 检索) / https://github.com/Ethan-YS/ccvault (2026-08-01 检索)
- 详情:
  - **直接扫描**：search-sessions（Rust）无索引无数据库，单二进制直接扫 `~/.claude/projects/` 下 JSONL（README 称总量可达约 1.6GB），子秒级：索引元数据搜索 18ms、`--deep` 全文搜索配 ripgrep 约 280ms（未装 ripgrep 时 Rust 内置实现约 1s，README 建议安装以获得 3-5 倍加速）。
  - **SQLite 索引**：cclens（Rust）把 JSONL 解析为 SQLite 数据库（`~/.cache/cclens/index.db`），提取 branch、时间戳、消息数等元数据，提供全文搜索；ccsearch（Rust）用 SQLite FTS5 + 向量双重索引。
  - **UI 查看器**：ccvault（Python 3.8+，仅标准库，零依赖）扫描 `~/.claude/projects` 建本地存档 `~/.ccvault/archive`，web UI 只读浏览、可导出 Markdown/zip。

### 发现 3: 恢复命令输出是核心价值 — 结果直接给 `claude -r <id>` / `claude --resume <id>`

- 来源: https://github.com/cc-deck/cc-session (2026-08-01 检索) / https://github.com/chronologos/cc-sessions (2026-08-01 检索) / https://github.com/madzarm/ccsearch (2026-08-01 检索)
- 详情:
  - cc-session（Rust，rayon 并行 I/O）扫 `~/.claude/projects/`，逐文件读首几行跳过 `file-history-snapshot` 条目和内部标记，提取第一条真实用户消息、项目路径、git branch、时间戳；生成带引号路径的 `cd '<project-path>' && claude -r <session-id>` 并复制到剪贴板（arboard）。README 声称"2,000+ sessions 在 500ms 内"（截至 2026-08-01 为 README 声称值，未经独立基准验证）。
  - cc-sessions 明确指出内置 `/resume` 仅显示"当前项目 + 当前机器"的会话，其交互模式对项目名/摘要做 fuzzy 匹配，Enter 在原项目目录恢复，`--fork` 生成新 session ID。
  - ccsearch 的"One-Key Resume"：选中会话按 Enter 自动运行 `claude --resume <id>`。
  - search-sessions 输出含即贴即用的 `claude -r 7897c935-2069-4b75-bbad-a3fac62ea59c` 恢复命令（前缀 `cd <project> &&`）。

### 发现 4: 搜索技术栈 — ripgrep 用于深度扫描，fuzzy/BM25/向量用于排名

- 来源: https://github.com/raine/claude-history (2026-08-01 检索) / https://github.com/madzarm/ccsearch (2026-08-01 检索) / https://github.com/sinzin91/search-sessions (2026-08-01 检索)
- 详情:
  - claude-history（Rust，截至 2026-08-01 约 426 stars）自带模糊搜索实现（README 未提 fzf/ripgrep）："field-aware relevance scoring"——标题、项目名、摘要中的命中权重高于正文；支持前缀匹配、词边界感知、工具输出索引（"search also includes tool results"）；引号括起做精确/大小写敏感匹配；Ctrl+T 切换语义搜索（本地下载 embedding 模型）。会话列表按 JSONL 文件 mtime 计算"3 days ago"列与排序加权。
  - ccsearch 混合搜索：会话按 4000 字符重叠分块索引；BM25（SQLite FTS5）在混合模式下加权 3x；语义向量用 `all-MiniLM-L6-v2`（384 维，约 80MB ONNX 模型，首次使用下载）；两者经 Reciprocal Rank Fusion 合并。模式：默认混合、`--exact` 纯字面、`--semantic` 纯语义。搜索前自动检测新增/变更的会话，无需手动重建索引。
  - search-sessions 深度搜索用 ripgrep（可用时），元数据搜索不用。

### 发现 5: 标题/摘要来源 — summary 事件、AI 生成标题、首条用户消息三路并用

- 来源: https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md (2026-08-01 检索) / https://github.com/maleta/claude-sessions (2026-08-01 检索) / https://github.com/MrPickering/session-index (2026-08-01 检索)
- 详情:
  - 指南工作流：从每个 JSONL 文件提取首个 `summary` 事件（即 `/resume` 选择器显示的摘要行），按文件 mtime 排序列最近会话；jq 按 `timestamp` 过滤时间范围。
  - claude-sessions 插件：Stop hook（1+ 条用户消息后分析、每 5 条附加消息再分析）与 SessionEnd hook 触发 `claude -p --model haiku` 生成标题/摘要/主题/状态；写入 `~/.claude/session-tracker/sessions-data.js`（单一真值源）+ 每项目 `SESSION_SUMMARIES.md`。
  - session-index 对 Claude 会话用"记录的 AI 标题"，摘要优先取记录的 last prompt、回退首条有用用户消息（正文截断 240 字符）。
  - cc-sessions 从文件内容提取 "cwd, first message, summary, custom title"，用户经 `/rename` 重命名的会话显示 ★ 前缀。

### 发现 6: 导出/转换类 — 把 JSONL 变人类可读的 HTML/Markdown

- 来源: https://pypi.org/project/claude-code-transcripts/ (2026-08-01 检索，v0.6) / https://github.com/simonw/claude-code-transcripts (2026-08-01 检索，via 博客与搜索摘要) / https://github.com/Ethan-YS/ccvault (2026-08-01 检索)
- 详情:
  - claude-code-transcripts（Simon Willison，Apache-2.0，Python ≥3.10）：四个子命令 `local`（交互选择 `~/.claude/projects` 最近会话，默认显示 10 个）、`web`（Claude API 拉取）、`json`（单文件）、`all`（全部本地会话生成带主索引的存档）；输出 `index.html` 提示词/commit 时间线 + 分页 `page-001.html`；`--gist` 经 gh CLI 上传 GitHub Gist。安装 `uv tool install claude-code-transcripts` 或 `uvx` 即用。相关博客文章标题："A new way to extract detailed transcripts from Claude Code"（alldevblogs 转载 Simon Willison）。截至 2026-08-01 该仓库约 1,554 stars（来源为搜索摘要，非仓库页直接确认）。
  - ccvault 已知限制：Claude Code 不存储模型 thinking 文本（仅加密签名），故 thinking 无法展示或导出——对任何想解析 thinking 的工具都适用。
  - cicada 导出/导入用 zip，"contain the raw JSONL session file plus a manifest"，导入的会话仍可 `claude --resume`。

### 发现 7: 监控/分析型工具 — 实时状态而非事后查询

- 来源: https://github.com/Curt-Park/claudeview (2026-08-01 检索) / https://github.com/base-14/cicada (2026-08-01 检索)
- 详情:
  - claudeview：k9s 风格 TUI，"reads `~/.claude/` directly — no hooks, no config"；层级 drill-down projects → sessions → agents → tool calls；数据源含主会话 `~/.claude/projects/<hash>/<session-id>.jsonl`、子代理 `.../<session-id>/subagents/agent-<id>.jsonl`、`~/.claude/plugins/`、`~/.claude/settings.json`；解析为增量 offset 式（"only new bytes are read on each tick"），1 秒刷新。
  - cicada（Go，Homebrew `base-14/tap/cicada` 或 `go install`）：读本地 `~/.claude/` 数据，无服务器无数据库；视图含 usage heatmap、sessions-per-day sparkline、streaks、项目 drill-down、工具排名（内置 + MCP）。
  - 子代理转录文件约定（`<session-id>/subagents/agent-<id>.jsonl`）被 claudeview 与 claude-code-transcripts 的 `--include-agents`（默认排除）双边确认。

### 发现 8: 技能化封装 — 把查询能力注入 Claude Code 自身

- 来源: https://github.com/negipo/cclens (2026-08-01 检索) / https://github.com/daymade/claude-code-skills/blob/main/daymade-claude-code/claude-code-history-files-finder/SKILL.md (2026-08-01 检索) / https://github.com/sinzin91/search-sessions (2026-08-01 检索)
- 详情:
  - cclens `install` 子命令向 `~/.claude/skills/` 复制三个 skill：`cclens-searching-history`、`cclens-exporting-history`、`cclens-resuming-from-history`。
  - search-sessions 官方 README 建议粘贴 "Set up https://github.com/sinzin91/search-sessions as a /search-sessions skill." 让 Claude 自行安装配置。
  - daymade 的 SKILL.md + `scripts/analyze_sessions.py`（子命令 `list`/`search`/`stats`）：搜索字段覆盖 messages、thinking 文本（非签名）、tool inputs/results、queue 内容、附件、最后提示词、系统/摘要内容、自定义标题、file-history 快照中的原始路径；时间过滤用 JSONL 记录内部时间戳而非文件 mtime；跨多配置目录（`~/.claude`、`~/.claude-profiles/<name>`、`CLAUDE_CONFIG_DIR`）+ `~/.claude/history-sources.json` 注册的长期存档；`--codex` 扩至 Codex rollouts。配套 `scripts/recover_content.py` 可从 file-history 快照恢复已删除文件的精确字节（"exact captured bytes ... including post-Write edits and binary files"），写 `recovery_report.txt` 含 SHA-256。

### 发现 9: 内置命令局限与原始工具工作流（社区普遍做法）

- 来源: https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md (2026-08-01 检索) / https://github.com/chronologos/cc-sessions (2026-08-01 检索)
- 详情: 指南建议先试内置 `/resume`、`claude --resume`、`claude --continue`、`/rewind`、`/export`、`/insights`、`/clear [name]`，仅当需要跨项目全文搜索、时间范围过滤或批量分析时才降级到文件层。原始命令工作流：
  - 找当前项目最近会话：`ls -lt ~/.claude/projects/*/*.jsonl | head -n 1`
  - 跨项目全文搜索：`rg -l --no-ignore -g '*.jsonl' 'term' ~/.claude/projects/`
  - 提取全部 assistant 文本：`cat session.jsonl | jq -r 'select(.type == "assistant") | .message.content[] | select(.type == "text") | .text'`
  - 时间范围：`find ... -mtime -7` 或 jq `select(.timestamp >= "..." and .timestamp < "...")`
  - shell helper 建议写入 `.zshrc`/`.bashrc`：`claude-find()` 列出含关键词的会话文件（新→旧）、`claude-sessions()` 输出最近 N 个会话摘要
  - 提示词历史：`~/.claude.json` 的 `projects.<path>.history` 仅有每项目最近提示词字符串，不含响应与完整会话
  - 注意事项：`message.content` 可能是字符串或块列表（jq 匹配前需 `tostring`）；部分行是元数据（加 `select(.type != null)` 过滤）；"/resume 是 per-project 的，需 cd 回原目录或直接搜文件"；"file mtimes shift, IDs don't"——有价值会话 ID 应记在持久处。

### 发现 10: 会话过期与数据持久性风险（对终端模拟器功能设计的约束）

- 来源: https://hn.svelte.dev/item/46352875 (2026-08-01 检索，Hacker News 线程摘要) / https://www.alldevblogs.com/article/simon-willison/a-new-way-to-extract-detailed-transcripts-from-claude-code (2026-08-01 检索)
- 详情: Simon Willison 在 Hacker News 线程中提及 **Claude Code 默认 30 天过期会话记录**（来源为 HN 讨论摘要，非官方文档逐字确认），并发布过关闭该行为的文章；为此他构建了分享工具链（Codex/Claude Code 时间线渲染器）与 `cc_pre.py`（`rg --pre` 预处理脚本，把 JSONL 重排格式提升可搜索性，用法 `cd ~/.claude/projects && rg --pre cc_pre.py 'search term'`）。对"终端模拟器查询历史会话"功能而言，30 天过期意味着文件系统扫描方案的覆盖面有上限，且不可恢复的过期会话需考虑内置过期开关或导出存档。**注意**：30 天默认值与关闭方法未在本轮检索中逐字核实于官方文档，实施前需以官方文档（code.claude.com/docs）为准。

### 发现 11: Windows 平台支持情况

- 来源: https://github.com/MrPickering/session-index (2026-08-01 检索) / https://github.com/madzarm/ccsearch (2026-08-01 检索) / https://github.com/Ethan-YS/ccvault (2026-08-01 检索)
- 详情: session-index 明确列出 Windows 数据源 `%USERPROFILE%\.claude\projects`（或 `$CLAUDE_CONFIG_DIR/projects`），环境变量 `SESSION_SCAN_ROOT`/`SESSION_SCAN_LIMIT`/`CLAUDE_CONFIG_DIR`/`PORT`。ccsearch 提供 PowerShell 安装脚本（`irm .../install.ps1 | iex`）。ccvault 提供 `ccvault.bat` Windows 一键启动器。多数 Rust 工具（claude-history、cclens、cc-session、cc-sessions、search-sessions、ccsearch）本身跨平台，但安装文档以 Homebrew/curl 为主，Windows 二进制覆盖参差不齐——对 slTerminal（Windows 原生）是空白点。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://github.com/tokenbender/agent-guides/blob/main/claude-conversation-search-guide.md | 社区指南 | 存储约定（目录=/ 替换为 -、文件名=UUID、summary 事件）、rg/jq 工作流、shell helper、/resume per-project 局限 |
| https://github.com/raine/claude-history | 源码仓库 | Rust fuzzy 搜索 TUI：field-aware 评分、工具输出索引、语义搜索（Ctrl+T）、mtime 排序、Ctrl+R 恢复/Ctrl+F fork、`--resume` 交接 `claude --resume` |
| https://github.com/sinzin91/search-sessions | 源码仓库 | 无索引扫描 ~/.claude/projects（约 1.6GB）；index 18ms / deep ripgrep 280ms / 内置 1s；输出 `claude -r <uuid>`；`--project`/`--since`/`--until` 过滤 |
| https://github.com/negipo/cclens | 源码仓库 | SQLite 索引（`~/.cache/cclens/index.db`）；query/list/browse/show/export/install（3 skills）；`\|` OR 搜索；branch/日期过滤 |
| https://github.com/madzarm/ccsearch | 源码仓库 | BM25(FTS5)×3 + all-MiniLM-L6-v2 + RRF 混合搜索；4000 字符重叠分块；一键 `claude --resume`；PowerShell 安装脚本 |
| https://github.com/cc-deck/cc-session | 源码仓库 | rayon 并行扫描 ~/.claude/projects；跳过 file-history-snapshot 提取首条用户消息；`cd '...' && claude -r <id>` 剪贴板；README 声称 2000+ sessions <500ms |
| https://github.com/chronologos/cc-sessions | 源码仓库 | 跨项目/跨机器列出；UUID 文件名过滤 + 内容提取（cwd/first message/summary/custom title）；fuzzy 交互 + `--list` 表格；`--fork`；`/rename` 会话 ★ 标记 |
| https://github.com/Ethan-YS/ccvault | 源码仓库 | Python 零依赖只读存档（`~/.ccvault/archive`）；resume 快照自动去重（`--no-dedupe` 关闭）；thinking 不存储（仅加密签名）为已知限制；ccvault.bat Windows 启动器 |
| https://github.com/maleta/claude-sessions | 源码仓库 | 插件：Stop/SessionEnd hooks 触发 `claude -p --model haiku` 生成标题/摘要/主题；`SESSION_SUMMARIES.md`；静态 web UI（`~/.claude/session-tracker/index.html`） |
| https://github.com/MrPickering/session-index | 源码仓库 | 零依赖本地 dashboard（README 原文 "dependency-free"）；Windows 路径 `%USERPROFILE%\.claude\projects`；AI 标题 + last prompt 摘要（240 字符截断）；`SESSION_SCAN_ROOT`/`SESSION_SCAN_LIMIT`/`CLAUDE_CONFIG_DIR` 环境变量 |
| https://github.com/daymade/claude-code-skills/blob/main/daymade-claude-code/claude-code-history-files-finder/SKILL.md | 源码仓库（skill） | analyze_sessions.py（list/search/stats）跨多配置目录 + history-sources.json 存档；`--all-projects`/`--codex`/`--from-date`/`--to-date`/`--exclude-session`；recover_content.py 从 file-history 恢复删除文件字节 |
| https://github.com/Curt-Park/claudeview | 源码仓库 | k9s 风格 TUI；增量 offset 式 JSONL 解析；层级 projects→sessions→agents→tool calls；subagents 转录路径 `<session-id>/subagents/agent-<id>.jsonl`；token 用量窗口 |
| https://github.com/base-14/cicada | 源码仓库 | Go TUI 分析：heatmap、sparkline、streaks、项目 drill-down、工具排名；zip 导出/导入（raw JSONL + manifest），导入仍可 `claude --resume` |
| https://pypi.org/project/claude-code-transcripts/ | 包注册表 | v0.6（Simon Willison）：local/web/json/all 四命令；index.html + 分页 HTML；`--gist` 发布；`uv tool install` |
| https://github.com/simonw/claude-code-transcripts | 源码仓库 | 同上工具仓库（约 1,554 stars，截至 2026-08-01 为搜索摘要值）；动机：JSONL 对人类"practically useless" |
| https://hn.svelte.dev/item/46352875 | 社区讨论 | HN 线程（"A year of vibes"）摘要：Claude Code 默认 30 天过期会话记录；`cc_pre.py` rg --pre 预处理脚本 |
| https://www.alldevblogs.com/article/simon-willison/a-new-way-to-extract-detailed-transcripts-from-claude-code | 技术博客（转载） | 宣布 claude-code-transcripts 的博客文章：会话是"better interface ... than even Claude Code itself"；静态 HTML 分享定位 |
| https://www.piwheels.org/project/ccq/ | 包注册表 | ccq 0.2.0（2026-06-21 发布，纯 Python）："Query your own Claude Code agent history with DuckDB, read-only, straight over the JSONL transcripts"；`pip3 install ccq` |
| https://github.com/anthropics/claude-code/issues/4483 | 官方 issue | feature request：`/export-all`（含完整会话内容，不受 compaction 与终端缓冲限制）——反映内置导出能力的局限 |
