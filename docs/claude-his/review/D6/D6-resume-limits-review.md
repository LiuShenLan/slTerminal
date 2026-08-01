# D6-resume-limits 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D6

---

核查范围：14 条发现、17 个来源 URL。官方文档（code.claude.com/docs/en/sessions、whats-new/2026-w24）、11 个 GitHub issue（35226/52494/24465/28769/42735/23710/35698/29052/26123/13229/46784/81793/59941/41021）、技术博客（lzwjava）全部经 WebFetch/API 逐字比对（issue 状态、版本号、报错文本、日期、引文）。官方文档 9 处逐字引用全部一致；除以下 3 条错误外，其余声称（含 #81793 open 状态、#13229 错误文本 `Session [session-id] was not found.`、#28769 worktree 场景逐字错误、w24 周报 /cd 引文等）均与来源吻合。

## 错误 1: 发现 7 声称报告者建议"用大正数（如 3650）"——3650 在 issue #23710 中不存在

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D6-resume-limits.md` (第 67 行)
- **原声称**: "截至检索日该 issue 标记 closed，页面无修复细节；报告者建议值：用大正数（如 3650）而非 0。"
- **错误类型**: 内容不支撑（虚构数字——来源页面不存在）
- **正确信息**: 报告者（Mustafa-Esoofally）的建议是纯代码级修复——从 `appendEntry` 中删除 `cleanupPeriodDays === 0` 检查（"The cleanup period should only affect the cleanup/retention logic, not the write path."），从未建议任何替代数值；"3650" 在 issue 正文与全部 12 条评论中均不出现。评论中确有大正数建议，但来自其他用户且数值不同（bengous 建议 `cleanupPeriodDays: 99999`）。文档将虚构数字归于报告者，来源不支撑。
- **反证来源**: https://api.github.com/repos/anthropics/claude-code/issues/23710 —— "The string '3650' appears nowhere in the issue body. The reporter never suggests substituting a large positive number. Instead, they argue the field should not control the write path at all, and their suggested fix is purely to delete the `cleanupPeriodDays === 0` check from `appendEntry`."；https://api.github.com/repos/anthropics/claude-code/issues/23710/comments —— bengous（2026-02-06）："His fix: 'set `cleanupPeriodDays: 99999`.'"

## 错误 2: 发现 7 声称"页面无修复细节"——实际存在维护者修复评论（v2.1.89 起拒绝该值）

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D6-resume-limits.md` (第 67 行)
- **原声称**: "截至检索日该 issue 标记 closed，页面无修复细节"
- **错误类型**: 事实错误（同时构成过时信息）
- **正确信息**: 该 issue 有修复细节且远早于检索日（2026-08-01）：collaborator ashwin-ant 于 2026-04-18 评论 "This was fixed in **v2.1.89** — `cleanupPeriodDays: 0` is now rejected as invalid (with an error shown in /status) instead of silently disabling all transcript persistence."。据此，文档把 `cleanupPeriodDays: 0` 静默禁用持久化描述为现行行为（"实际行为却是静默禁用全部 transcript 持久化"）在 v2.1.89+ 已不成立，未提该修复构成过时信息。
- **反证来源**: https://api.github.com/repos/anthropics/claude-code/issues/23710/comments —— 评论 11（ashwin-ant, COLLABORATOR, 2026-04-18T04:15:32Z）："This was fixed in **v2.1.89** — `cleanupPeriodDays: 0` is now rejected as invalid (with an error shown in /status) instead of silently disabling all transcript persistence."

## 错误 3: 发现 14 中 #41021 日期"2026-05 前后"错误——实际创建于 2026-03-30

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D6-resume-limits.md` (第 116 行)
- **原声称**: "- 来源: https://github.com/anthropics/claude-code/issues/41021 (2026-05 前后，检索摘要——未经页面逐字核验)"
- **错误类型**: 事实错误
- **正确信息**: issue #41021 创建于 2026-03-30（标题 "Allow /resume to find sessions across all projects"，closed as duplicate），文档所标"2026-05 前后"偏晚约 1.5 个月。该条虽自标"未经页面逐字核验"，现经页面核验确认日期不符，且其余内容（标题、"均未实现"）与页面吻合。
- **反证来源**: https://github.com/anthropics/claude-code/issues/41021 —— "**Created:** Mar 30, 2026, by user `lasthalf`"；"**Close reason:** Duplicate (badged 'Closed as duplicate')"
