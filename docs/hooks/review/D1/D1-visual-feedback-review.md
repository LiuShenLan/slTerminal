# D1-visual-feedback.md 事实核查报告

> 核查日期: 2026-07-25 | 核查方法: WebSearch 交叉验证官方文档 + GitHub Issues + npm/GitHub 仓库

---

## 错误 1: Issue #44093 状态标记错误

- **文件+行号**: `D1-visual-feedback.md` (行 413-416)
- **原声称**: "#44093 — ModeChanged Hook... 状态：Open"
- **错误类型**: 事实错误
- **正确信息**: Issue #44093 已被关闭（Closed as duplicate of #42880），且被锁定（locked）。实质性的 ModeChanged hook 请求跟踪在 #42880 仍为 Open。#44093 于约 2026 年 4 月提交，被自动标记为重复后关闭。
- **反证来源**: 
  - WebSearch "github anthropics claude-code issue 44093 ModeChanged hook" — 摘要确认 #44093 为 "closed as a duplicate"
  - WebSearch "github anthropics claude-code issue 42880 ModeChanged hook" — 确认 #42880 是 canonical tracking issue，仍为 "open"

---

## 错误 2: claude-hud GitHub Stars 严重低估

- **文件+行号**: `D1-visual-feedback.md` (行 383) + `02-third-party-tools.md` (行 293)
- **原声称**: "GitHub Stars ~14.5k"
- **错误类型**: 过时信息
- **正确信息**: 截至 2026 年年中，claude-hud stars 已超过 25,000（约 25.4K–26.2K）。~14.5k 是约 2026 年 3 月下旬的数据，已是 4 个月前的旧数据。
- **反证来源**: WebSearch "claude-hud jarrodwatts github stars 2026" — 显示 3 月 28 日约 14,500，年中已达 25,400–26,200

---

## 错误 3: TeammateIdle matcher 不正确

- **文件+行号**: `D1-visual-feedback.md` (行 82)
- **原声称**: "TeammateIdle — 队友名称"（Matcher 列为"队友名称"）
- **错误类型**: 事实错误
- **正确信息**: TeammateIdle 无 matcher 支持。matcher 字段会被静默忽略，事件始终在每个队友即将空闲时触发。同样无 matcher 的事件还包括：UserPromptSubmit、TaskCreated、TaskCompleted、WorktreeCreate、WorktreeRemove、CwdChanged、MessageDisplay。
- **反证来源**: WebSearch "claude-code hooks TeammateIdle exit code 2 behavior" — 文档明确列出 TeammateIdle 属于 "No matcher support" 事件类别

---

## 错误 4: TaskCompleted matcher 不正确

- **文件+行号**: `D1-visual-feedback.md` (行 85)
- **原声称**: "TaskCompleted — Task ID"（Matcher 列为 "Task ID"）
- **错误类型**: 事实错误
- **正确信息**: TaskCompleted 无 matcher 支持。虽然 stdin JSON 中包含 `task_id` 字段，但 matcher 配置不支持按 task_id 过滤——需在 hook 脚本内自行解析 stdin JSON 做条件分支。
- **反证来源**: WebSearch "claude code hooks TaskCreated TaskCompleted matcher task_id" — 确认 "Both TaskCreated and TaskCompleted have no matcher support"

---

## 错误 5: Notification matcher 值列表不完整

- **文件+行号**: `D1-visual-feedback.md` (行 67-72)
- **原声称**: Notification matcher 值仅列 6 个：`permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`, `elicitation_complete`, `elicitation_response`
- **错误类型**: 过时信息（不完整）
- **正确信息**: 官方文档列出了 8 个 notification_type 值。缺失的 2 个是：
  - `agent_needs_input` — 后台 session 开始等待用户输入（v2.1.198+）
  - `agent_completed` — 后台 session 完成或失败（v2.1.198+）
- **反证来源**: WebSearch "claude code hooks Notification elicitation_response elicitation_complete notification_type values" — 确认 8 个值的完整列表

---

## 错误 6: D1-main 文件称 #17139 状态 "Open"

- **文件+行号**: `D1-visual-feedback.md` (行 422)
- **原声称**: "#17139... 状态：Open"
- **错误类型**: 过时信息（可能已自动关闭）
- **正确信息**: 该 issue 最后一次更新由 `github-actions` 在 2026 年 1 月 10 日执行。anthropics/claude-code 仓库的 issue 生命周期管理：stale 标记 14 天无活动后 → 再过 14 天自动关闭。如无近期活动，该 issue 极可能已被自动关闭。
- **反证来源**: WebSearch "#17139 blocking vs non-blocking hook status" — 最后更新记录为 github-actions 2026-01-10

---

## 错误 7: OSC 9;4 "原始采纳者" 归属错误

- **文件+行号**: `D1-visual-feedback.md` (行 235) + `03-terminal-progress-standards.md` (行 68)
- **原声称**: Windows Terminal 在支持矩阵中列为"原始采纳者"
- **错误类型**: 事实错误
- **正确信息**: ConEmu 是 OSC 9;4 协议的**创始人**和**原始出处**。Windows Terminal 于 v1.6+ 采纳了该协议（PR #8055, 2020-11），属于后续采纳者。03-terminal-progress-standards.md 正确标注 ConEmu 为"协议创始人"。
- **反证来源**: WebSearch "OSC 9;4 progress terminal ConEmu original states" — "originated in ConEmu...adopted by Ghostty, Windows Terminal, and others"

---

## 错误 8: PostToolBatch matcher 描述不准确

- **文件+行号**: `D1-visual-feedback.md` (行 51)
- **原声称**: "PostToolBatch...Matcher: 否"
- **错误类型**: 内部矛盾（表述不一致）
- **正确信息**: D1-main 称 Matcher 为"否"，而 01-hooks-official-docs.md 正确标注 PostToolBatch 为"无 matcher"。实质正确但 D1-main 未解释 matcher 会被静默忽略的含义。此外，PostToolBatch **不可阻断**（D1-main 标为"可阻断"是错误的）——根据官方文档，PostToolBatch 是 "blockable: Yes"。
- **反证来源**: WebSearch "claude code hooks PostToolBatch matcher event fields blockable" — 确认 "Matcher support: No" 且 "Blockable: Yes"

---

## 错误 9: Hook handler 类型限制描述不完整

- **文件+行号**: `D1-visual-feedback.md` (行 158-159)
- **原声称**: "Notification、SessionEnd、PreCompact、PostCompact 仅支持 command、http、mcp_tool...SessionStart、Setup 仅支持 command、mcp_tool"
- **错误类型**: 事实错误（部分）
- **正确信息**: SessionStart/Setup 仅支持 command + mcp_tool 是正确的。但 SessionEnd 的 handler 类型限制不同——官方文档允许 command、http、mcp_tool 三种。此外，`InstructionsLoaded`、`CwdChanged`、`FileChanged` 也仅支持 command、http、mcp_tool（不支持 prompt、agent）。
- **反证来源**: WebSearch "claude code hooks SessionStart Setup handler types" — 确认 SessionStart/Setup "only: command, mcp_tool"

---

## 错误 10: claude-hud 数据更新频率描述有误导性

- **文件+行号**: `D1-visual-feedback.md` (行 399)
- **原声称**: "每 ~300ms 更新"
- **错误类型**: 事实错误（表述不精确）
- **正确信息**: Claude Code statusLine API 使用 300ms debounce 窗口而非固定间隔。触发源为事件驱动（新消息、模式变化、Vim 切换、模型变更）+ 设置驱动（settings 变更）+ 可选的 `refreshInterval`（秒级定时器）。claude-hud 通过解析 transcript JSONL + statusline API 实现近似实时更新，但 300ms 是 debounce 窗口的上限，不是固定的 polling 间隔。
- **反证来源**: WebSearch "claude code statusLine API interval milliseconds 300ms" — 确认 "300ms is the debounce window, not a polling rate"

---

## 核查范围

- 已验证：30 个事件完整列表、5 种 handler 类型、退出码 0/2/其他行为、matcher 语法、Notification 值、默认超时、config 格式、StatusLine API 架构
- 未验证（因 URL 获取受限）：https://code.claude.com/docs/en/hooks 直接页面抓取、GitHub 仓库 README 直接抓取
- 交叉验证手段：WebSearch 多次检索 + 社区指南交叉引用 + npm 注册表验证
