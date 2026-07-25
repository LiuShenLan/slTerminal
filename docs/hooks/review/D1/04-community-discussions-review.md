# 04-community-discussions.md 事实核查报告

> 核查日期: 2026-07-25 | 核查方法: WebSearch GitHub Issues + npm registry + Hacker News 交叉验证

---

## 错误 1: Issue #44093 状态标记错误

- **文件+行号**: `04-community-discussions.md` (行 10-19)
- **原声称**: "#44093...状态：已关闭（重复于 #42880）...日期：约 2026 年 4 月"
- **错误类型**: 部分正确，但补充信息有误
- **正确信息**: 状态"Closed（duplicate of #42880）"是**正确的**（与 D1-visual-feedback.md 的"Open"标记不同，此文件正确）。但文件在描述中未明确区分 #44093 和 canonical issue #42880 各自的状态——后者仍为 Open。
- **反证来源**: WebSearch 确认 #42880 是 open 的 canonical tracking issue

---

## 错误 2: D1-visual-feedback.md 同类错误传播 — #44093 标记正确

- **文件+行号**: `04-community-discussions.md` (行 13 vs D1-visual-feedback.md 行 416)
- **原声称**: 04 文件说"已关闭（重复于 #42880）"，D1-main 说"状态：Open"
- **错误类型**: 内部矛盾
- **正确信息**: 04 文件正确（Closed），D1-main 错误（Open）。两个文件对同一 issue 的状态描述矛盾，需统一。
- **反证来源**: 上文错误 1 的验证

---

## 错误 3: Issue #9575 关闭原因描述不精确

- **文件+行号**: `04-community-discussions.md` (行 62-63)
- **原声称**: "Closed（未修复，超时关闭）"（引自 D1-main 表）
- **错误类型**: 来源不支撑（无法确认关闭原因）
- **正确信息**: 原始 issue 描述的 bug 是 Notification hook 在 v2.0.15 中仅约 25-30% 的概率触发。原因定位为"Claude Code 内部不一致地检查 Notification hooks"。通过 WebSearch 无法确认 issue 是否因超时自动关闭（anthropics/claude-code 的 automated sweep 在 14 天无活动后标记 stale + 14 天后关闭）。需要直接访问 GitHub issue 页面确认关闭原因是否为 stale/timeout 还是 fixed。
- **反证来源**: WebSearch "github anthropics claude-code issue 9575" — 确认 bug 描述与文档一致（25-30% 触发率），但无法确认最终关闭原因

---

## 错误 4: Issue #11394 定性为"假警报"不准确

- **文件+行号**: `04-community-discussions.md` (行 63)
- **原声称**: "#11394 — 假警报（用户端 JSONL 解析 bug）"
- **错误类型**: 事实错误（过于简化）
- **正确信息**: Issue 摘要显示：reporter 后来承认其 Stop hook 实际正常工作——失败原因为 jq 查询 JSONL 格式不正确。但**其他 hook 类型**（PreToolUse、PostToolUse、UserPromptSubmit）确实显示 "Found 0 hook matchers in settings"——这些可能仍是真正 bug 而非假警报。定性为纯粹"假警报"过于简化。
- **反证来源**: WebSearch "github anthropics claude-code issue 11394" — "reporter later noted Stop hook was actually executing correctly — the root problem was an incorrect jq query... However, the core bug regarding other hook types not being recognized from settings.json remained unresolved"

---

## 错误 5: Hacker News 讨论日期过于模糊

- **文件+行号**: `04-community-discussions.md` (行 146-147)
- **原声称**: "日期：约 2025 年末"、"日期：约 2025 年中期（hooks 功能首次发布）"
- **错误类型**: 过时信息
- **正确信息**: Hooks 功能是 2025 年中期首次发布的。但"约 2025 年末"和"约 2025 年中期"是估计值——需要直接访问 HN 帖子（item?id=44477756 和 item?id=44429225）确认实际发布日期。HN post IDs 可大致推断日期：ID 44429225 对应 ~2025 年中期是正确的量级。
- **反证来源**: 无法通过 WebSearch 获取 HN 帖子的精确发布日期

---

## 错误 6: "Reddit 无直接讨论结果"声明

- **文件+行号**: `04-community-discussions.md` (行 331)
- **原声称**: "Reddit 搜索（site:reddit.com 'Claude Code' hooks）无直接讨论结果"
- **错误类型**: 来源不支撑（搜索受限可能产生假阴性）
- **正确信息**: 搜索方法描述为 `site:reddit.com "Claude Code" hooks`——此搜索受限于工具能力，false negative 可能性高。Reddit 上可能有 r/ClaudeCode 或 r/ClaudeAI 子版块的讨论未被抓取。声明过于绝对。
- **反证来源**: 无直接反证；声明标记为低置信度

---

## 错误 7: 工具平台标注的准确性

- **文件+行号**: `04-community-discussions.md` (行 103-136) 多工具表格
- **原声称**: 多个第三方工具的平台/安装方式/特性描述
- **错误类型**: 部分核实（多数正确）
- **正确信息**: 以下工具描述经 npm/GitHub 搜索核实正确：
  - claude-code-tab-title (franzvill): macOS/Linux, OSC 序列
  - tabby-claude-status: Tabby/Windows, 9 个 hook 事件映射到 5 种状态 ← 此状态映射与 02-third-party-tools.md 一致
  - claude-notifications (dimokol): VS Code 扩展
  - claude-buzz (ethanplusai): macOS 通知
  以下工具未经独立验证：claude-tab-watcher (dgr8akki)、CCNotify (dazuiba)、job-finish、claude-done
- **反证来源**: npm registry + GitHub 搜索

---

## 错误 8: "claude-code-session-manager — 17 个内置页签 + Cmd-K 41+ 命令"

- **文件+行号**: `04-community-discussions.md` (行 130)
- **原声称**: "17 个内置页签（总览/终端/记忆/设置/权限/技能/插件/MCP/Hooks/调度器等），Cmd-K 命令面板 41+ 命令"
- **错误类型**: 来源不支撑
- **正确信息**: 这些具体数字（17 页签、41+ 命令）需直接验证 GitHub 仓库 (StanislavBG/claude-code-session-manager) README。WebSearch 无法确认具体数字。
- **反证来源**: 需要直接访问 https://github.com/StanislavBG/claude-code-session-manager

---

## 错误 9: "Claude Code IDE — 8 个并发 session 页签"

- **文件+行号**: `04-community-discussions.md` (行 131)
- **原声称**: "最多 8 个并发 session 页签 + 橙色脉冲点通知 + OS Toast + Monaco 编辑器面板"
- **错误类型**: 来源不支撑
- **正确信息**: "8 个并发 session" 的具体数量上限需要从 GitHub (Powellga/Claude-Code-IDE) README 直接验证。
- **反证来源**: 需要直接访问 https://github.com/Powellga/Claude-Code-IDE

---

## 错误 10: 社区工具"完全安装"声明

- **文件+行号**: `04-community-discussions.md` (行 107, 108, 118, 119, 120, 121, 122)
- **原声称**: 多个工具的安装方式描述（npm/plugin marketplace）
- **错误类型**: 部分无法验证
- **正确信息**: npm 包可通过 npm registry 验证存在性。但 `/plugin marketplace add` 命令需要 Claude Code 插件市场实际支持，无法通过 WebSearch 确认当前 marketplace 状态。`npm install -g` 路径可验证。
- **反证来源**: npm registry 确认以下包存在：@ttigger/claude-status, tabby-claude-status, claude-nudge, job-finish, @erica_s/claude-code-notify, claude-hook-notify, claude-done

---

## 核查范围

- 已验证：31 个 GitHub Issues 编号存在性、关键 issue 的状态和内容、HN 讨论存在性、npm 包存在性（约 10 个包）、第三方工具的一般描述
- 内部矛盾发现：1 项（#44093 状态在 D1-main vs 04 间矛盾）
- 低置信度项：HN 精确发布日期、具体工具的页面数/并发数声明、Reddit 讨论的假阴性、issue 关闭原因（stale vs fixed）
