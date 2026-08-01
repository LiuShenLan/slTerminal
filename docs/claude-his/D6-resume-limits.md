# claude --resume 用法与语义 — 边界与限制

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论

## 关键发现

### 发现 1: 跨目录 resume 的硬限制——session 查找范围限定当前项目目录 + git worktrees（官方明文）

官方文档《Manage sessions》（截至 2026-08-01 检索时为最新版）逐字确认：

> "Run this from the directory the session was started in: session ID lookup is scoped to the current project directory and its git worktrees, so a session created elsewhere reports `No conversation found with session ID: <session-id>`."

即 session ID 查找范围 = 当前项目目录 + 其 git worktrees，他处创建的 session 在此目录下 resume 必然失败并报上述错误。session 存储以启动目录的路径 slug 为键：`~/.claude/projects/<project>/<session-id>.jsonl`，`<project>` 为工作目录路径中非字母数字字符替换为 `-` 后的形式。会话选择器默认只显示：当前 worktree 的 session（含标记为 `bg` 的后台 session）+ 通过 `/add-dir` 加入过当前目录的 session；`Ctrl+W` 扩到同仓库全部 worktrees，`Ctrl+A` 扩到本机全部项目。

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 从别的目录选择无关项目的 session 时，Claude Code 不直接恢复，而是"copies a `cd` and resume command to your clipboard instead"（把 `cd` + resume 命令复制到剪贴板）。按名称 resume（`claude --resume <name>` / `/resume <name>`）可跨当前仓库及其 worktrees 解析。

### 发现 2: 目录不匹配时的报错文本是"误导性 catch-all"

- 来源: https://github.com/anthropics/claude-code/issues/35226 (2026-03-17，closed as duplicate)
- 详情: 场景：在 `~/code` 启动 claude，用绝对路径在子目录仓库 `~/code/monofolk` 工作后退出；再 `cd ~/code/monofolk` 执行 `claude --resume`，即使 session 在 `~/.claude/history.jsonl` 中且 transcript 完整，resume 仍失败，报 "invalid or expired"。报告者原文："The 'invalid or expired' message is a catch-all for 'session not found in the current directory's scope.' It doesn't distinguish between a genuinely missing session and a directory mismatch." 报告者分析根因：`--resume` 按目录限定 session 查找范围；`workspace.current_dir` 在 session 创建时设定后从不更新。修复建议 P0 为改进报错文案（如 "Session was created in ~/code — run claude --resume from that directory."），P1 为允许按全局唯一 session ID 跨目录 resume。该 issue 以 duplicate 关闭，无维护者回复，无修复版本。

### 发现 3: 目录重命名/移动导致 session 孤儿化（数据在磁盘上但不可达）

- 来源: https://github.com/anthropics/claude-code/issues/52494 (2026-04-23，closed as not planned)
- 详情: 重命名项目文件夹（`~/Claude Root/Cowork` → `~/Claude Root/dghq`）后，桌面 App 从 Recents 恢复任意 session 报逐字错误：`Claude Code returned an error result: No conversation found with session ID: 475672e6-8e2b-4be0-8fd4-07308af8ed90`。根因：存储键为路径派生 slug（如 `-Volumes-Carter-Users-jde-Claude-Root-Cowork`），改名改变 slug，文件仍在但应用不再查旧目录。变通方案（改名回去 / 旧路径建符号链接）均被报告者评价为不可用，最终放弃改名。无维护者回复，closed as not planned。

- 来源: https://github.com/anthropics/claude-code/issues/24465 (2026-02-09，closed as duplicate)
- 详情: Windows `subst` 驱动器场景（Claude Code v2.1.37）：`subst Z: D:\git\MyRepo` 后在 `Z:\` 启动会话，session 创建用驱动器盘符路径 → 存到 `~/.claude/projects/Z--/`（112 个 .jsonl）；而 `/resume` 查找时解析真实路径 → 查 `~/.claude/projects/D--git-MyRepo/`（0 个 .jsonl，仅 sessions-index.json，且条目 `originalPath: "Z:\"` 证明不一致）。`/resume` 恒报 "no sessions to resume"。报告者指出旧版本正常（属回归），同类问题可能影响 `net use` 映射盘与 junction。无维护者回复，closed as duplicate。

### 发现 4: worktree 内启动的 session，resume 时必须重复 `--worktree` 标志

- 来源: https://github.com/anthropics/claude-code/issues/28769 (2026-02-25，closed；Claude Code v2.1.58，macOS)
- 详情: 用 `--worktree` 启动的 session 退出时打印的提示省略了 worktree 标志：`Resume this session with: claude --resume bc9c1dfd-06d9-4ffe-b223-c0f60b9ada2e`。原样复制执行报逐字错误：`No conversation found with session ID: bc9c1dfd-06d9-4ffe-b223-c0f60b9ada2e`。唯一可行方式是同时指定 worktree：`claude --worktree <name> --resume bc9c1dfd-...`。说明 session ID 查找范围含 worktree 命名空间——裸 `--resume <id>` 在 worktree 外查不到。无维护者回复，关闭原因未披露（无 PR/duplicate 记录）。

### 发现 5: 权限模式恢复限制——`plan` 与 `bypassPermissions` 永不恢复（官方明文）

官方文档逐字确认：

> "Permission mode: the mode the session was in. `plan` and `bypassPermissions` are never restored; bypassing permissions must be enabled again at launch, with one of its launch flags or `permissions.defaultMode: "bypassPermissions"` in settings. `auto` is restored only when your account still meets the auto mode requirements. Pass `--permission-mode` to override the restored mode."

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 除 plan/bypass 外，`auto` 模式仅在账号仍满足 auto mode 要求时才恢复。恢复后的权限模式可用 `--permission-mode` 覆盖。

- 来源: https://github.com/anthropics/claude-code/issues/42735 (2026-04-02，closed as duplicate)
- 详情: VS Code 扩展在恢复旧会话时忽略 `claudeCode.initialPermissionMode: "bypassPermissions"`（Windows 11，claude-opus-4-6）：即使 `~/.claude/settings.json` 与 `settings.local.json` 均配置 `permissions.defaultMode: "bypassPermissions"` 且 `permissions.allow` 含通配，resumed conversation 仍回退默认模式、每次编辑都弹权限确认。会话内手动切到 bypass 有效，但切到别的会话即重置。closed as duplicate，无可见修复。

### 发现 6: 启动标志不恢复——`--mcp-config` 等必须重新传入（官方明文）

官方文档逐字确认：

> "Not every configuration flag from the original launch is restored. If the session depended on `--mcp-config`, `--settings`, `--plugin-dir`, `--fallback-model`, or directories added with `--add-dir`, pass them again when you resume; directories added mid-session with `/add-dir` aren't restored either, though the session picker still uses them to locate the session. The standard settings files, such as `settings.json` and `settings.local.json`, are re-read at launch, so configuration that lives in them doesn't need to be passed again."

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 即 `--mcp-config`、`--settings`、`--plugin-dir`、`--fallback-model`、`--add-dir` 添加的目录均不随 resume 恢复，需重新传参；标准 settings 文件在启动时重读，故其中配置无需重传。

- 来源: https://lzwjava.github.io/resuming-dangerous-flags-repeat-en (2026-04)
- 详情: 技术博客论证危险标志必须在每次 resume 时重复声明，逐字示例：`claude --resume d79629a0-c3a2-40e7-8f36-b21bf7d0d53c --dangerously-skip-permissions`。理由："Flags are not persisted for safety reasons"（安全标志视为"每次执行的显式同意"，不存于 session 状态）；"Resume restores conversation, not execution policy."（resume 恢复对话，不恢复执行策略）。心智模型：`--resume` = "reload memory"，标志 = "start with elevated privileges"，两者分层、需各自指定。

### 发现 7: session 过期与清理——`cleanupPeriodDays` 默认 30 天

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 官方文档将保留期表述为 "the 30-day retention"，可由设置项修改——表格逐字："Change the 30-day retention | `cleanupPeriodDays` | `settings.json`"。transcript 默认存为 JSONL：`~/.claude/projects/<project>/<session-id>.jsonl`；条目格式为内部格式且随版本变化（"The entry format is internal to Claude Code and changes between versions"）。

- 来源: https://github.com/anthropics/claude-code/issues/23710 (2026-02-06，closed)
- 详情: `"cleanupPeriodDays": 0` 的文档语义是 "Number of days to retain chat transcripts (0 to disable cleanup)"（永久保留），实际行为却是静默禁用全部 transcript 持久化：`appendEntry` 写入路径把 `cleanupPeriodDays === 0` 当持久化开关（逐字代码：`if (getEnv() === "test" && !A || getSettings()?.cleanupPeriodDays === 0 || isSessionPersistenceDisabled()) return;`）。后果：不写任何 .jsonl、`/resume` 报 "No conversations found"、hook 收到空 `transcript_path`、历史永久丢失（真实数据丢失案例：用户意图永久保留反被清空）。清理例程本身对 0 处理正确（retentionMs = 0 → 无任何文件达到删除年龄），不一致仅存在于写路径。截至检索日该 issue 已 closed——collaborator 2026-04-18 评论确认 **v2.1.89 起 `cleanupPeriodDays: 0` 被拒绝为无效值**（在 /status 中报错）而非静默禁用持久化，以上静默禁用行为描述仅适用于 v2.1.89 之前；报告者（Mustafa-Esoofally）建议的是纯代码级修复（清理逻辑只应影响 retention，不应控制写路径），评论中另有大正数建议但出自其他用户（`cleanupPeriodDays: 99999`）。

### 发现 8: `/resume` 选择器数量限制——50 条硬上限，另有 2.1.31–2.1.42 时代 10 条初始批次 bug

- 来源: https://github.com/anthropics/claude-code/issues/35698 (2026-03-18，closed as duplicate；Claude Code v2.1.78)
- 详情: `/resume` 选择器硬编码最多 50 个 session（报告者引用源码常量 `gmT = 50`），按最近排序；报告者 400+ session 时界面只显示 "1/50"。源码存在 `nextIndex` 返回值暗示曾规划分页，但 UI 无"加载更多"入口；无配置项可改。变通：选择器内 `/` 搜索（跨全部 session，但需记得关键词）、`claude -r <name>`、`claude --from-pr <number>`、`/rename`。

- 来源: https://github.com/anthropics/claude-code/issues/29052 (2026-02-26，closed as duplicate)
- 详情: 同一限制的早期报告：多分支重负载约 2 周后旧 session 从选择器消失，尽管数据仍在磁盘（报告者原文："the data is there — it's just not shown in the picker"）。明确区分：`cleanupPeriodDays` 管磁盘保留期（报告者设为 365 天），50 条选择器上限管 UI 展示，二者无关。提议 `"resumeSessionLimit": 100` 设置项，未实现。

- 来源: https://github.com/anthropics/claude-code/issues/26123 (2026-02-16，closed；受影响 v2.1.31–v2.1.42+，最后正常版本 v2.1.29)
- 详情: `/resume` 历史不可用，三个根因（issue 合并 12+ 相关 issue）：① `sessions-index.json` 约 2026-02-04 起停止写入（磁盘 3,080+ 个 .jsonl 仅 740 个被索引，2 月 4 日后 0% 被索引）；② 选择器初始批次硬编码 10 条（源码 `K=10` 参数），加载更多触发依赖终端高度（24 行终端下 `page_size=4`，触发条件永不满足），另有过滤器丢弃无 `firstPrompt` 且无 `customTitle` 的 "lite" session；③ Windows 多 worktree 场景大小写敏感路径比较（`dir.name === s`）静默失败（`git worktree list` 返回 `C:/...` 而项目目录存 `c--...`）。社区一行修复：`sed -i 's/async function _c1(A,q,K=10)/async function _c1(A,q,K=500)/' cli.js`。变通：`claude --resume <keyword>` / `<session-id>` 绕过选择器直接查全部 .jsonl。

### 发现 9: 版本兼容性——跨大版本 resume 失败且无迁移路径

- 来源: https://github.com/anthropics/claude-code/issues/13229 (2025-12-06，closed as not planned，带 stale 标签)
- 详情: session 文件跨版本使用时成为"混合版本文件"（JSONL 行依次来自 1.0.119 → 2.0.5 → 2.0.59），2.0.60 起拒绝此类文件，`/resume [session-id]` 报逐字错误 `Session [session-id] was not found.`（报告者概括为 "Session not found"），尽管文件存在且有效（541 MB、46,031 行，位于 `~/.claude/projects/[project]/[session-id].jsonl`）。四种恢复尝试全部失败：复制为新 session ID、打补丁版本串（1,206 处替换）、打补丁 session ID（45,042 处替换）、前置 v2 summary 头。结论：2.0.60 引入未文档化的校验机制，无用户可用的迁移路径。报告者试图降级失败——即使 VS Code 禁用自动更新，Claude 仍强制升级到最新版。无维护者回复，closed as not planned（无修复）。

- 来源: https://github.com/anthropics/claude-code/issues/46784 (2026-04-11，closed；回归)
- 详情: v2.1.98 起（v2.1.101 确认）`/resume` 不再按当前工作目录限定，列出全部目录的 session；v2.1.97 正常。git 与非 git 目录均复现。变通：`ln -sf ~/.local/share/claude/versions/2.1.97 ~/.local/bin/claude`。报告者怀疑与 v2.1.100 "Fixed `--resume` from worktrees" 有关，未被确认。无维护者回复，关闭原因未披露。

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 官方对条目格式的表述 "The entry format is internal to Claude Code and changes between versions, so scripts that parse these files directly can break on any release" ——官方承认 JSONL 条目格式随版本变化，直接解析文件的脚本可能在任何发布中失效；官方建议用 `/export` 或脚本接口替代直接解析。

### 发现 10: 长会话恢复对话框（Pro/Max 计划，官方明文）

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: "On a Pro or Max plan, when you resume a session that has been inactive for more than about an hour and is over 100,000 tokens, Claude Code restores the conversation and then opens a dialog before you send your first message." 触发条件：Pro/Max 计划 + 停用超约 1 小时 + 超 100,000 tokens。此时 prompt cache 已过期，无论选哪项，下一请求都会完整处理一次历史。对话框三个选项：**Resume from summary**（立即执行 `/compact`，历史替换为摘要 + 最近交流 + 至多 5 个最近读取文件，后续请求携带摘要）；**Resume full session as-is**（原样加载，首条消息后完整重处理并重缓存）；**Don't ask me again**（原样恢复且今后不再弹此框）。

### 发现 11: session 过大时的死局——"No response requested." 且 `/compact` 不可达

- 来源: https://github.com/anthropics/claude-code/issues/81793 (2026-07-27，**open**；Claude Code 2.1.220，macOS，Opus 1M)
- 详情: session 增长到请求开始失败时无应用内恢复途径。`/compact` 依赖把完整对话发给模型做摘要——正是失败的请求，故内置修复在恰好需要的时刻不可达（issue 原文："compaction works by sending the full conversation to the model to summarize — which is precisely the request that is failing. So the built-in fix is unreachable in the exact situation it exists for. `--resume` re-sends the same history and fails identically."）。报错文本逐字：`No response requested.`（约 10 次失败，以零 token 用量的合成 assistant turn 写入 transcript，不说明失败原因）。外部变通：解析 .jsonl、裁剪为近期轮次子集、原地改写后 `--resume` 恢复成功（约 20 个 session）——但需逆向 transcript 格式，且存在隐蔽陷阱（并行工具调用跨共享 `message.id` 的兄弟记录分布，naive 裁剪会产出损坏 transcript）。请求修复：本地 `/compact`（免模型往返）、文档化离线恢复路径、暴露真实错误。截至检索日 open，无维护者回复。

### 发现 12: 恢复时带回的内容与不带回的内容（官方明文）

- 来源: https://code.claude.com/docs/en/sessions (官方文档，访问于 2026-08-01)
- 详情: 恢复内容：对话历史（含工具调用与结果）、模型（模型被退役或 `availableModels` 不允许、启动时 `--model` 标志或 `ANTHROPIC_MODEL` 系列环境变量指定、或 Amazon Bedrock/Google Cloud Agent Platform/Microsoft Foundry 等使用 provider 专属部署 ID 时不恢复）、agent（`--agent` 启动的会话延续该 agent 的系统提示/工具限制/模型；v2.1.216 起先在原目录（已信任工作区）再在恢复目录查找项目级 agent，两处都找不到则以默认工具与系统提示恢复并显示警告 "session-agent-no-longer-available"；v2.1.216 修复了后台 agent 会话恢复时回退默认 agent 的问题）、active goal（未过期的目标及其轮次计数/计时器/token 基线重置）、scheduled tasks（未过期任务恢复；后台 Bash 与 monitor 任务不恢复）。另：`claude -p` 或 Agent SDK 创建的 session 不出现在选择器，但可传 session ID 给 `claude --resume <session-id>` 恢复。选择器选中后加载失败时报 `Failed to resume the conversation` 并附重试命令、以 exit code 1 退出。

### 发现 13: 官方跨目录恢复路径——`/cd`（v2.1.169+）

- 来源: https://code.claude.com/docs/en/whats-new/2026-w24 (2026-06-08–12 周报；版本 v2.1.166 → v2.1.176)
- 详情: 逐字："The new `/cd` command moves the current session to a different working directory without rebuilding the prompt cache: the new directory's `CLAUDE.md` is appended as a message instead of replacing the system prompt. The session relocates to the new directory's project storage, so `--resume` and `--continue` find it there."（v2.1.169）。同页其他相关条目：子代理可再生成子代理（v2.1.172，链深上限五层）。官方 sessions 文档补充：v2.1.196 起移动后的 session 即使崩溃/强制退出也不再出现在旧目录选择器（更早版本在旧路径含下划线等特殊字符且非干净退出时可能重新出现）。

### 发现 14: 跨项目全局查找是持续存在的未实现诉求

- 来源: https://github.com/anthropics/claude-code/issues/59941 (2026-05-17，closed；enhancement)
- 详情: 请求 `/resume`（选择器）默认或经开关（提议 `CLAUDE_RESUME_GLOBAL=1`）显示全部项目目录的 session。理由：数据集中存于 `~/.claude/projects/`，仅 UI 过滤阻断访问；多项目用户无法在不记得起始目录的情况下找到旧 session。报告者自建 `/ses` 命令 + shell wrapper 读 `~/.claude/projects/*/*.jsonl` 配 fzf 作为变通（依赖未文档化内部格式）。无维护者回复、无 PR、无实施痕迹，closed。

- 来源: https://github.com/anthropics/claude-code/issues/41021 (2026-03-30 创建，closed as duplicate)
- 详情: "Allow /resume to find sessions across all projects" 同类诉求（与 #59941、#28745 等同一方向），均未实现。官方现有机制为选择器内 `Ctrl+A`（本机全部项目）与跨项目选中时复制 `cd`+resume 命令到剪贴板。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/sessions | 官方文档 | 查找范围限定当前目录+worktrees、报错 `No conversation found with session ID: <session-id>`、`plan`/`bypassPermissions` 永不恢复、`--mcp-config` 等标志不恢复、30 天保留期 cleanupPeriodDays、长会话恢复对话框（>1h + >100,000 tokens）、/cd v2.1.169/v2.1.196/v2.1.211/v2.1.216 版本行为、`Failed to resume the conversation` exit code 1 |
| https://code.claude.com/docs/en/whats-new/2026-w24 | 官方文档 | /cd v2.1.169 移动 session 至新目录存储、子代理链深上限五层（v2.1.172） |
| https://github.com/anthropics/claude-code/issues/13229 | 社区讨论（issue，closed as not planned） | v1.x→v2.0.60 拒绝混版本 session 文件；`Session [session-id] was not found.`；541MB/46,031 行；无迁移路径；2025-12-06 |
| https://github.com/anthropics/claude-code/issues/35226 | 社区讨论（issue，closed as duplicate） | 目录不匹配报 "invalid or expired" catch-all；`workspace.current_dir` 创建后不更新；2026-03-17 |
| https://github.com/anthropics/claude-code/issues/26123 | 社区讨论（issue，closed） | v2.1.31–v2.1.42+ /resume 失效三根因：sessions-index.json 停写（2026-02-04）、初始批次硬编码 K=10、Windows worktree 大小写比较；最后正常 2.1.29；2026-02-16 |
| https://github.com/anthropics/claude-code/issues/52494 | 社区讨论（issue，closed as not planned） | 重命名项目文件夹孤儿化 session；`No conversation found with session ID: ...`；路径 slug 键；2026-04-23 |
| https://github.com/anthropics/claude-code/issues/42735 | 社区讨论（issue，closed as duplicate） | VS Code 扩展恢复会话忽略 initialPermissionMode；bypass 模式不持久；2026-04-02 |
| https://github.com/anthropics/claude-code/issues/23710 | 社区讨论（issue，closed） | cleanupPeriodDays: 0 静默禁用全部 transcript 持久化；/resume 报 "No conversations found"；建议用大正数；2026-02-06 |
| https://github.com/anthropics/claude-code/issues/35698 | 社区讨论（issue，closed as duplicate） | /resume 选择器硬编码 50 条上限（`gmT = 50`）；"1/50"；v2.1.78；2026-03-18 |
| https://github.com/anthropics/claude-code/issues/29052 | 社区讨论（issue，closed as duplicate） | 50 条上限与 cleanupPeriodDays 的区分；resumeSessionLimit 提议未实现；2026-02-26 |
| https://github.com/anthropics/claude-code/issues/28769 | 社区讨论（issue，closed） | --worktree 启动的 session 裸 resume 报 `No conversation found with session ID: ...`；须 `claude --worktree <name> --resume <id>`；v2.1.58；2026-02-25 |
| https://github.com/anthropics/claude-code/issues/24465 | 社区讨论（issue，closed as duplicate） | Windows subst 盘：创建用盘符路径、查找解析真实路径；"no sessions to resume"；v2.1.37；2026-02-09 |
| https://github.com/anthropics/claude-code/issues/46784 | 社区讨论（issue，closed） | v2.1.98–v2.1.101 回归：/resume 列出全部目录 session；v2.1.97 正常；2026-04-11 |
| https://github.com/anthropics/claude-code/issues/81793 | 社区讨论（issue，**open**） | session 过大无应用内恢复；`No response requested.`；/compact 在需要时不可达；v2.1.220；2026-07-27 |
| https://github.com/anthropics/claude-code/issues/59941 | 社区讨论（issue，closed） | 全局跨项目 /resume 诉求；CLAUDE_RESUME_GLOBAL 提议未实现；2026-05-17 |
| https://lzwjava.github.io/resuming-dangerous-flags-repeat-en | 技术博客 | 危险标志每次 resume 须重传（--dangerously-skip-permissions 示例）；"Resume restores conversation, not execution policy."；2026-04 |
| https://github.com/anthropics/claude-code/issues/41021 | 社区讨论（issue，仅检索摘要未逐字核验） | "Allow /resume to find sessions across all projects" 同类未实现诉求 |
