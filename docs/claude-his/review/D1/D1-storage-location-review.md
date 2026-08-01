# D1-storage-location 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D1

---

## 错误 1: Windows 路径编码示例 `-C-Users-you-app` 形态错误（官方实际为 `C--Users-you-app`），且该示例不存在于所标注的 claude-teleport README 中

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D1-storage-location.md` (第 18 行)
- **原声称**: 「Windows: `C:\Users\you\app` → `-C-Users-you-app`（claude-teleport README，搜索摘要）；盘符字母保留并参与编码：`Y:\path\to\project` → `y--path-to-project`（GitHub Issue #38186 一手证据，小写盘符 + `\`、`:` 均替换为 `-`）」
- **错误类型**: 事实错误（兼来源不支撑）
- **正确信息**: 按官方编码规则（非字母数字字符替换为 `-`）及多个一手证据，`C:\Users\you\app` 应编码为 `C--Users-you-app`：盘符 `C` 原样保留（无前导 `-`），`C:` 的冒号与紧随的反斜杠连续替换产生双破折号 `C--`。带前导 `-` 的形态只出现在 Unix 绝对路径（前导 `/` 替换）与 Git Bash/MSYS 形式路径（如 `/c/Users/...` → `-c-...`，#54865）中，`C:\...` 原生 Windows 路径不会产生前导 `-`。另：claude-teleport README 全文无此具体示例（README 仅泛称「Windows drive letters and backslashes」与「re-encodes the folder names for the target OS」），该示例无法从标注来源验证。
- **反证来源**:
  - https://github.com/anthropics/claude-code/issues/54066 — 一手证据：`C:\dev\foo_bar` 产生目录 `C--dev-foo-bar/`（「every separator (`\`, `/`, `:`, `_`) replaced with `-`」），无前导 `-`
  - https://docs.rs/claude-session-parser/latest/claude_session_parser/usage/fn.encode_project_path.html — 「C:\Users\Seven\foo → C--Users-Seven-foo」，同样 `C:\` → `C--` 双破折号、无前导 `-`
  - https://github.com/gowtham-sai-yadav/claude-teleport — WebFetch 全文检索确认 README 无 `-C-Users-you-app` 示例（「No such concrete example appears on this page」）；claude-teleport 的编码实际也兼容官方形态（否则无法重编码官方目录名）

---

## 核查通过项摘要

以下声称经 WebFetch/WebSearch 外部验证全部正确（未单列错误条目）：

- 发现 1：官方 sessions 文档原文 `~/.claude/projects/<project>/<session-id>.jsonl` 引用逐字一致；`<project>` = 工作目录路径非字母数字替换为 `-`；claude-directory Application data 表 `projects/<project>/<session>.jsonl`（「Full conversation transcript: every message, tool call, and tool result」）一致
- 发现 2：macOS/Linux 示例 `/Users/username/myapp` → `-Users-username-myapp`（contextspectre `-Users-user-dev-myproject`、ccrider `-Users-neil-xuku-invoice`、classmethod `-Users-username-myapp/` 三处一致）；启动目录决定目录键而非 git root（contextspectre 原文）；`Y:\path\to\project` → `y--path-to-project` 与 #38186 原文一致；claude-teleport「every session is pinned to the exact path your project sat at」「the piece the folder name throws away」及 manifest 记录真实路径、默认重写 `cwd`、`--deep` 重写全部路径，逐字一致
- 发现 3：ccrider 三存储位置（history.jsonl / [sessionId].jsonl / agent-[agentId].jsonl）与全部 JSONL 字段（summary 首行 leafUuid、uuid/parentUuid/timestamp/sessionId/cwd/gitBranch/version/userType/isSidechain、assistant requestId + message{model/id/content/stop_reason/usage}、system subtype/content/level/isMeta、file-history-snapshot trackedFileBackups、/resume 追加同文件 sessionId 不变）一致；官方目录表 memory/、subagents/、tool-results/、history.jsonl、sessions/、file-history/、plans/、tasks/、session-env/、debug/、paste-cache/、image-cache/、shell-snapshots/、backups/ 全部对应；transcript 格式内部化警告原文逐字一致
- 发现 4：Windows `~/.claude` = `%USERPROFILE%\.claude` 官方原文一致；#54066（closed as not planned、2026-04-27、2.1.119、`C--dev-foo-bar/` vs `C--dev-foo_bar/`、memory/session 分裂、data-loss 标签、Win11 10.0.26200、关联 #19972）全部核实；#32533（closed as duplicate、2026-03-09、`%AppData%\Claude\claude-code-sessions\`、EXDEV 日志原文、`.json.tmp` 变通、JSONL 完好）核实；#34710（closed、2026-03-15、TEMP/AppData 跨盘、EXDEV 实证、批量复制变通）核实；#38186（closed as not planned、2026-03-24、Extension v2.1.81、realpathSync→UNC、`--192-168-x-x-share-path-to-project` vs `y--path-to-project`、历史 0 会话、New-Item Junction / mklink /J「local NTFS volumes required」）全部核实；#26166（open、2026-02-16、`~/.vscode/globalStorage/anthropic.claude-code/` 仅卸载清理处提及、共享历史关系未文档化）核实
- 发现 5：sessionId UUID 与文件名一致（contextspectre/ccrider）；`--resume`/`--resume <name>`/`--resume <session-id>`、`claude -p`/SDK 会话不进选择器但可传 ID 恢复；sessionId 查找限定当前目录+worktrees（官方原文 `No conversation found with session ID`）；`/cd` v2.1.169+ 迁移；`/branch`/`--fork-session` 复制 transcript 新 sessionId 原文件不动；SDK SessionKey{projectKey, sessionId, subpath}、projectKey 为 filesystem-safe 编码、subpath 示例 `subagents/agent-<id>`、mirror 双写、persistSession:false 与 store 互斥——全部核实
- 发现 6：cleanupPeriodDays 默认 30 天、整数 ≥1、启动时删除、已删不可恢复、三层作用域（classmethod 原文一致）；官方清除范围表与「Kept until you delete them」表（history.jsonl/stats-cache.json/remote-settings.json/sessions/plugins 不在清除范围）一致；`CLAUDE_CODE_SKIP_PROMPT_HISTORY`「any mode」、`--no-session-persistence`（配 `claude -p`）、`claude project purge` v2.1.124+ 及 purge 计划输出 `dir: /home/user/.claude/projects/-home-user-work-my-repo` 逐字一致；明文存储警告原文逐字一致；classmethod 文章日期 2026-07-04 核实

---

## 备注（未列为错误，供参考）

- 发现 1「CLI 与 Claude for Mac 共享 `~/.claude/projects/`」：contextspectre（Claude for Mac 时代）明确支持该声称，且 2026-03 的 #32533/#34710 显示 Desktop app 的 JSONL 确在 `~/.claude/projects/`（物理目录共享成立）。官方 desktop 文档「Each maintains separate session history」描述的是会话历史列表/索引层面（Desktop 另有 `%AppData%\Claude\claude-code-sessions\` 索引），与物理目录共享不构成直接否定，故未列为错误；但引用 contextspectre 时未标注其为旧版（Claude for Mac）行为描述
- 发现 2「盘符字母保留」的概括与 #38186 示例 `y--path-to-project`（小写盘符）存在轻微张力，但转述与 issue 一手证据一致，未列为错误
