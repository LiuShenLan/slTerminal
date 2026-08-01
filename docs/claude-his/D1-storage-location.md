# Claude Code 会话存储机制 — 存储位置与目录结构

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论

## 关键发现

### 发现 1: 默认存储目录 — `~/.claude/projects/<project>/<session-id>.jsonl`

- 来源: https://code.claude.com/docs/en/sessions (截至 2026-08-01 检索)
- 详情: 官方「Manage sessions」文档原文：「By default, transcripts are stored as JSONL at `~/.claude/projects/<project>/<session-id>.jsonl`」。会话随工作过程持续写入本地 transcript 文件，退出或 `/clear` 后仍可恢复。官方「Explore the .claude directory」文档的 Application data 表同样列出 `projects/<project>/<session>.jsonl`（完整对话 transcript：每条消息、工具调用、工具结果）。同一项目目录下可同时存在多个会话文件（CLI 与 Claude for Mac 共享 `~/.claude/projects/`）。

### 发现 2: 目录命名规则 — 工作目录绝对路径编码（非字母数字字符替换为 `-`）

- 来源: https://code.claude.com/docs/en/sessions (截至 2026-08-01 检索)
- 详情: 官方文档原文：「`<project>` is your working directory path with non-alphanumeric characters replaced by `-`」。即 `~/.claude/projects/` 下的子目录名 = 启动 Claude Code 时工作目录的绝对路径编码：路径分隔符与冒号等非字母数字字符全部替换为 `-`。实测形态（多来源一致）：
  - macOS/Linux: `/Users/username/myapp` → `-Users-username-myapp`（contextspectre、ccrider、DevelopersIO 三处示例一致，前导一个 `-`）
  - Windows: `C:\Users\you\app` → `C--Users-you-app`（盘符 `C` 原样保留，`C:` 的冒号与紧随的 `\` 连续替换产生双破折号；形态由官方规则推导，GitHub Issue #54066 一手证据 `C:\dev\foo_bar` → `C--dev-foo-bar` 与 claude-session-parser 文档 `C:\Users\Seven\foo` → `C--Users-Seven-foo` 佐证）；盘符字母保留并参与编码：`Y:\path\to\project` → `y--path-to-project`（GitHub Issue #38186 一手证据，小写盘符 + `\`、`:` 均替换为 `-`）
  - **目录键由启动目录决定，而非 git root**：同一仓库从不同子目录启动会落入不同项目目录（如 `-myproject/` 与 `-myproject-src/`），见 contextspectre。
  - **编码有损，不可逆**：claude-teleport 说明手动复制 `~/.claude` 到另一台机器无效，因为「every session is pinned to the exact path your project sat at」且目录名丢弃了路径信息（「the piece the folder name throws away」）；真实绝对路径需额外 manifest 记录。GitHub Issue #54066 证实 Windows 上同一路径可出现两个变体目录（见发现 4）。

### 发现 3: 文件类型 — `.jsonl` transcript、子代理 transcript、auto memory 与全局历史

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md (截至 2026-08-01 检索；内容基于 Claude Code 2.0.22/2.0.29)
- 详情: 三个核心存储位置：`~/.claude/history.jsonl`（全局命令历史）、`~/.claude/projects/[project-path]/[sessionId].jsonl`（主会话 transcript）、`~/.claude/projects/[project-path]/agent-[agentId].jsonl`（子代理 transcript）。JSONL 每行一个 JSON 对象：
  - **首行 summary**：`type: "summary"` + `summary`（人类可读标题）+ `leafUuid`（最终消息 UUID）
  - **user/assistant 行**：`uuid`、`parentUuid`（首条为 null，消息构成 parentUuid 树）、`timestamp`（ISO 8601）、`sessionId`（与文件名 UUID 一致）、`cwd`、`gitBranch`、`version`（Claude Code 版本）、`userType`、`isSidechain`；assistant 行另含 `requestId` 与 `message` 对象（`model`/`id`/`content` 块数组/`stop_reason`/`usage`）
  - **system 行**：`subtype`、`content`、`level`、`isMeta`
  - **file-history-snapshot 行**：`trackedFileBackups` 映射
  - `/resume` 续开会话时**追加到同一 .jsonl**，`sessionId` 保持不变
  - contextspectre 补充：`slug`（人类可读名）、`permissionMode` 等字段；行类型含 `queue-operation`、`progress`（记账类，占磁盘不占上下文）
- 相关目录（官方「Explore the .claude directory」Application data 表，截至 2026-08-01 检索）：
  - `projects/<project>/memory/` — auto memory（`MEMORY.md` 索引 + 主题 `*.md`，跨会话自写笔记）
  - `projects/<project>/<session>/subagents/` 与 `projects/<project>/<session>/tool-results/` — 子代理 transcript 与大工具输出溢出文件（官方当前文档形态；SDK 文档 `subpath` 示例为 `subagents/agent-<id>`；ccrider 基于 2.0.x 描述平铺的 `agent-[agentId].jsonl`——两种形态均有来源，可能随版本变化，实现时需兼容探测）
  - `history.jsonl` — 每个输入的 prompt（带时间戳与项目路径，上箭头回忆用，不清除）
  - `sessions/` — 每个运行中会话一个小文件（并发/崩溃检测，退出即删）
  - `file-history/<session>/`（编辑前快照）、`plans/`（plan mode 文件）、`tasks/`（任务列表）、`session-env/`（会话环境元数据）、`debug/`（--debug 日志）、`paste-cache/`、`image-cache/`、`shell-snapshots/`、`backups/`（~/.claude.json 迁移前备份）
- **官方明确警告不要直接解析 transcript**（https://code.claude.com/docs/en/sessions）：「The entry format is internal to Claude Code and changes between versions, so scripts that parse these files directly can break on any release」——官方建议用 `/export` 或脚本接口；hooks/status line 接收 `transcript_path` 字段可定位 transcript 文件。

### 发现 4: Windows 与跨平台路径差异（坑与变体）

- 来源: https://code.claude.com/docs/en/claude-directory (截至 2026-08-01 检索)
- 详情: 官方原文：「On Windows, `~/.claude` resolves to `%USERPROFILE%\.claude`」；设置 `CLAUDE_CONFIG_DIR` 后所有路径改到该目录下。Windows 平台已知问题（均为一手 issue 证据）：
  - **下划线分裂（#54066，closed as not planned，2026-04-27 开，Claude Code 2.1.119）**：含 `_` 的路径如 `C:\dev\foo_bar` 会产生两个并存目录——`C--dev-foo-bar/`（分隔符 `\`、`:`、`_` 全替换为 `-`）与 `C--dev-foo_bar/`（保留 `_`）；不同子系统（memory vs session）写入不同变体，造成静默数据分裂。标签含 `data-loss`。作者建议统一路径键清洗函数 + 一次性合并迁移。
  - **EXDEV 跨盘 rename 失败（#32533，closed as duplicate，2026-03-09 开，Windows 11 10.0.26200；#34710，closed，2026-03-15 开）**：Desktop app 的会话索引写在 `%AppData%\Claude\claude-code-sessions\`，先写 `.json.tmp` 再 `fs.rename()` 成 `.json`；TEMP 与 AppData 跨盘时 rename 报 `EXDEV: cross-device link not permitted`，`.json.tmp` 滞留不被读取，会话静默消失；底层 JSONL 数据在 `~/.claude/projects/` 完好。变通：手动把 `.json.tmp` 复制为 `.json`。
  - **映射网络驱动器（#38186，closed as not planned，2026-03-24 开，Extension v2.1.81）**：扩展用 `realpathSync()` 把 `Y:\path` 解析成 UNC（`\192.168.x.x\share\...`），会话目录名随之变成 `--192-168-x-x-share-path-to-project`，而盘符访问的会话存在 `y--path-to-project`，历史显示为 0。变通：`New-Item -ItemType Junction` 建目录联接（`mklink /J` 在部分场景报 "local NTFS volumes required"）。
  - **VS Code 扩展存储（#26166，open，2026-02-16 开）**：扩展数据在 `~/.vscode/globalStorage/anthropic.claude-code/`；官方文档仅在该目录的卸载清理处提及，其与 CLI 的共享历史关系未文档化（issue 仍在 open）。
  - **跨平台迁移**：claude-teleport 导入时「detects the OS and translates everything, including Windows drive letters and backslashes」——重编码目录名并重写 transcript 内路径（默认重写 `cwd` 字段，`--deep` 重写所有路径），说明 transcript 内容中嵌有绝对路径。

### 发现 5: 目录结构、sessionId 与恢复机制的关系

- 来源: https://code.claude.com/docs/en/sessions (截至 2026-08-01 检索)
- 详情: sessionId 为 UUID，与文件名一致（ccrider/contextspectre 确认）；`claude --resume` 打开会话选择器，`claude --resume <name>` 按名恢复，`claude --resume <session-id>` 直接按 ID 恢复（`claude -p` 或 Agent SDK 创建的会话不进选择器但可传 ID 恢复）。**sessionId 查找限定在会话启动目录及其 git worktrees 内**——在其他目录 `--resume <session-id>` 报 `No conversation found with session ID: <session-id>`。`/cd` 移动会话后存储位置随之迁移（v2.1.169+）；`/branch` / `--fork-session` 复制 transcript 并生成新 sessionId（原文件不动）。SDK 侧（https://code.claude.com/docs/en/agent-sdk/session-storage）的 `SessionKey = { projectKey, sessionId, subpath }` 直接对应磁盘布局：`projectKey` 是工作目录的稳定 filesystem-safe 编码，`subpath`（如 `subagents/agent-<id>`）指向子代理 transcript 或 sidecar 文件。`transcript_path` 字段（hooks/status line 输入）是定位单个会话 transcript 的官方接口。

### 发现 6: 官方文档说明与可配置项（保留期、重定向、禁用）

- 来源: https://dev.classmethod.jp/en/articles/claude-code-conversation-history-retention/ (2026-07-04 发布)
- 详情: `cleanupPeriodDays` 设置项——默认 **30 天**，整数值 ≥1，Claude Code 启动时自动删除超过该天数的会话文件（已删无法恢复）；可配置在用户（`~/.claude/settings.json`）、项目（`.claude/settings.json`）、本地（`.claude/settings.local.json`）三层作用域。官方「Explore the .claude directory」确认：`projects/`、`file-history/`、`plans/`、`debug/`、缓存等路径由该清除机制覆盖（默认 30 天）；`history.jsonl`、`stats-cache.json`、`remote-settings.json`、`sessions/`、`plugins/` 不在清除范围内。相关可配置项（官方 sessions / claude-directory 文档）：
  - `CLAUDE_CONFIG_DIR` 环境变量 — 把存储移出 `~/.claude`
  - `CLAUDE_CODE_SKIP_PROMPT_HISTORY` — 所有模式下禁止写 transcript 与 prompt 历史
  - `--no-session-persistence`（配 `claude -p`）/ SDK `persistSession: false` — 单次非交互运行不持久化
  - `claude project purge <path>`（需 v2.1.124+）— 删除单项目全部状态（transcript、memory、tasks、history.jsonl 匹配行、`~/.claude.json` 条目；purge 计划输出示例直接展示目录名形态：`dir: /home/user/.claude/projects/-home-user-work-my-repo`）
  - **明文存储警告**（官方原文）：「Transcripts and history are not encrypted at rest. OS file permissions are the only protection」——工具读过的文件内容/命令输出/凭据都会落入 `projects/<project>/<session>.jsonl`。
  - 官方文档页：https://code.claude.com/docs/en/claude-directory（目录全览）、https://code.claude.com/docs/en/sessions（会话管理）、https://code.claude.com/docs/en/agent-sdk/session-storage（SDK 持久化）、https://code.claude.com/docs/en/env-vars（环境变量）。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/claude-directory | 官方文档 | `~/.claude` 全目录结构；`projects/<project>/<session>.jsonl`、`subagents/`、`tool-results/`、`memory/`、`file-history/`、`sessions/`、`history.jsonl`；Windows 上 `~/.claude` = `%USERPROFILE%\.claude`；cleanupPeriodDays 默认 30 天；明文不加密警告；`claude project purge`（v2.1.124+） |
| https://code.claude.com/docs/en/sessions | 官方文档 | 原文 `~/.claude/projects/<project>/<session-id>.jsonl`；`<project>` = 工作目录路径非字母数字替换为 `-`；格式内部化禁止直接解析；`--resume`/`--continue`/`/resume`；sessionId 查找限定当前目录+worktrees；`CLAUDE_CONFIG_DIR`/`cleanupPeriodDays`/`CLAUDE_CODE_SKIP_PROMPT_HISTORY`/`--no-session-persistence`；`transcript_path` |
| https://code.claude.com/docs/en/agent-sdk/session-storage | 官方文档 | SDK 默认写 JSONL 到 `~/.claude/projects/`；`SessionStore` 接口；`SessionKey = { projectKey, sessionId, subpath }`，subpath 示例 `subagents/agent-<id>`；mirror 双写机制；`persistSession: false` 与 store 互斥 |
| https://github.com/ppiankov/contextspectre/blob/main/docs/session-architecture.md | 技术文档 | 目录名 = 启动目录绝对路径编码（斜杠→`-`，前置 `-`），非 git root；`<uuid>.jsonl` 与 sessionId 一致；CLI 与 Claude for Mac 共享目录；行类型 user/assistant/system/queue-operation/progress/file-history-snapshot；修改文件影响所有读取实例 |
| https://github.com/neilberkman/ccrider/blob/main/research/schema.md | 源码研究 | `history.jsonl` + `projects/[path]/[sessionId].jsonl` + `agent-[agentId].jsonl` 三位置；JSONL 逐字段 schema（summary 首行、uuid/parentUuid/timestamp/sessionId/cwd/gitBranch/version/userType）；`/resume` 追加同一文件 sessionId 不变；parentUuid 树 + leafUuid |
| https://dev.classmethod.jp/en/articles/claude-code-conversation-history-retention/ | 技术博客 | 存储路径实测 `~/.claude/projects/-Users-username-myapp/`；cleanupPeriodDays 默认 30、启动时删除、三层配置作用域；明文存储 + 权限 `-rw-------`；文章日期 2026-07-04 |
| https://github.com/anthropics/claude-code/issues/54066 | GitHub issue（closed as not planned，2026-04-27） | Windows 含 `_` 路径产生双目录（`C--dev-foo-bar/` vs `C--dev-foo_bar/`），memory/session 静默分裂，标签 data-loss；环境 Windows 11 10.0.26200 + v2.1.119；关联 #19972（编码规则） |
| https://github.com/anthropics/claude-code/issues/32533 | GitHub issue（closed as duplicate，2026-03-09） | Desktop app 会话索引 `%AppData%\Claude\claude-code-sessions\`；`.json.tmp` → rename 跨盘 EXDEV 失败致会话不可见；底层 `~/.claude/projects/` JSONL 完好 |
| https://github.com/anthropics/claude-code/issues/34710 | GitHub issue（closed，2026-03-15） | TEMP 与 AppData 跨盘时全部会话重启丢失；EXDEV 日志实证；变通：`.json.tmp` → `.json` 批量复制 |
| https://github.com/anthropics/claude-code/issues/38186 | GitHub issue（closed as not planned，2026-03-24） | 映射网络驱动器 `realpathSync` → UNC；目录名 `y--path-to-project` vs `--192-168-x-x-share-path-to-project` 不匹配致历史为 0；变通：New-Item Junction |
| https://github.com/anthropics/claude-code/issues/26166 | GitHub issue（open，2026-02-16） | VS Code 扩展存储 `~/.vscode/globalStorage/anthropic.claude-code/` 未文档化；与 CLI 共享历史的关系待官方说明 |
| https://github.com/gowtham-sai-yadav/claude-teleport | 第三方工具 | 手动复制 `~/.claude` 失效根因（会话钉在原始路径、目录名丢弃路径信息）；导入时按目标 OS 重编码目录名 + 重写 transcript 内路径（默认 `cwd` 字段）；`CLAUDE_CONFIG_DIR` 可重定位 |
