# D5b Claude Code 社区实践 -- 事实核查

## 错误 1: disler 仓库"覆盖全部 13 个事件"与官方文档矛盾

- **文件+行号**: `D5b-claude-code-community.md` (行 15)
- **原声称**: "覆盖全部 13 个 hook 事件（UserPromptSubmit、PreToolUse、PostToolUse、PostToolUseFailure、Notification、Stop、SubagentStart、SubagentStop、PreCompact、SessionStart、SessionEnd、PermissionRequest、Setup）"
- **错误类型**: 内部矛盾
- **正确信息**: D5b 自身 (行 247) 声称官方支持 8 种核心事件。D5a 子报告 (行 331-345) 列出 12 个事件（含 PermissionDenied、FileChanged 等但不含 PermissionRequest）。三份清单互不一致。disler 仓库 README 确认覆盖 13 个事件——与官方文档的 8 核心事件或 12+ 事件之间的差异源自分歧：部分事件（如 Setup、PermissionRequest）在官方文档中可能被分类为 hook 类型而非 hook 事件
- **反证来源**: 
  - D5b (行 247) — 官方 8 核心事件
  - D5a (行 331-345) — 12 个事件
  - `github.com/disler/claude-code-hooks-mastery` — 覆盖 13 个（确认存在，~2.3K stars）

## 错误 2: Token 优化文章标题夸大（"83% 节省"是单项目最佳案例，非典型结果）

- **文件+行号**: `D5b-claude-code-community.md` (行 347-361)
- **原声称**: "~83% token 节省"、"8 个 Node.js hook 脚本"
- **错误类型**: 事实错误
- **正确信息**: 
  - dev.to 文章标题声称 "2.5M → 425K"（约 83%），但**这只是单个大项目的最佳案例**。文章作者本人报告：在 20 个项目上平均减少 **65.8%**，非 83%
  - 工具名为 **OpenWolf**（npm 包 `openwolf`），不是"6 个 hook 脚本"或"8 个 Node.js hook 脚本"——实际是 **6 个**生命周期 hook 脚本，D5b 写的"8 个"是错误的
  - Token 跟踪是"估算制"（~15% 精度范围），不是真实计量
  - `cerebrum.md` 合规率约 85-90%，不是 100%
- **反证来源**: 
  - `dev.to/cytostack/claude-code-used-25m-tokens...` — 原文，标题和内容一致
  - `npmjs.com/package/openwolf` — 确认 6 个 hook 脚本，非 8 个

## 错误 3: Hacker News 项目"HN URL + item ID"无法全部验证

- **文件+行号**: `D5b-claude-code-community.md` (行 145-238)
- **原声称**: 列出 9 个 HN 展示项目，每个附带 HN item ID URL
- **错误类型**: 来源不支撑
- **正确信息**: 
  - **Recall** (item 47189906): **已验证存在** — joseairosa/recall，MIT 开源，Show HN 2026年2月
  - **Han** (item 46150605): **已验证存在** — Bushido Collective，129 插件，`npx @thebushidocollective/han`
  - **Pickle Rick** (item 47091363)、**Draft** (item 48080538)、**MCR** (关联 47670002)、**TDD Guard**、**Claude Remote Approver** (item 47111171)、**Claude-Nonstop** (item 47082232)、**Claude Code Kit** (item 45789960): **未能独立验证**。HN 站点搜索对 "Pickle Rick" "Draft" "MCR" "TDD Guard" 返回零结果。这些 item ID 格式正确（8位数字 HN item ID），但内容无法确认——它们可能是在 HN 上以**不同标题**发布的，或者项目名称不准确
  - 注：item ID 47082232/47111171/45789960 等数字格式看起来是真实的 HN item ID，但 WebSearch 预算耗尽前未能逐条验证
- **反证来源**: 
  - `hn.svelte.dev/item/47189906` — Recall 确认存在
  - `news.ycombinator.com/item?id=46150605` — Han 确认存在
  - 其余 HN URL site-specific 搜索返回零结果

## 错误 4: Reddit "~34% 偏离率"缺乏单一确凿来源

- **文件+行号**: `D5b-claude-code-community.md` (行 119-125)
- **原声称**: "某用户报告 277 个 session 中 ~34% 的偏离率"、"四个独立开发者在 96 小时内收敛到同一结论"
- **错误类型**: 来源不支撑
- **正确信息**: 
  - "~34%" 偏离率: 社区普遍观测的 CLAUDE.md 规则不遵守率在 **22-40%** 范围，但 "34%" 这个精确数字在本次核查中**未找到单一定量来源**。最接近的是: Tygart Media 报告 ~30%（"~70% 遵守"）、GitHub Issue #32163 提及 "60-70% 遵守"（30-40% 偏离）、学术研究（Jaroslawicz 2025）发现最佳模型 <30% 完美合规
  - "277 个 session" 和"四个独立开发者在 96 小时内收敛": 在搜索结果中**未找到支撑**。这些具体数字没有发现对应的 Reddit 帖子或分析报告
- **反证来源**: 
  - `tygartmedia.com/claude-code-plan-mode-hooks/` — "CLAUDE.md instructions get followed roughly 70% of the time"
  - `github.com/anthropics/claude-code/issues/32163` — 社区讨论 60-70% 遵守率

## 错误 5: 通知工具列表可能不完整或部分不存在

- **文件+行号**: `D5b-claude-code-community.md` (行 408-416)
- **原声称**: 列出 6 个独立通知工具（ai-agent-notifier、claude-notify、agent-notify、claude-notifier、claude-notifications-go、ccnotify）
- **错误类型**: 来源不支撑（部分）
- **正确信息**: 
  - `ai-agent-notifier` (DevinoSolutions): GitHub 仓库可能存在
  - `agent-notify` (hellolib): GitHub 仓库可能存在
  - 其余四个工具的 GitHub 仓库（ddaikodaiko/claude-notify、felipeelias/claude-notifier、jaeinkim/claude-notifications-go、ccnotify npm 包）**未做独立验证**——WebSearch 预算耗尽前未能逐个检查
  - 6 个通知工具中可能存在某些工具已归档/不活跃或 npm 包名不匹配的情况
- **反证来源**: 无法确认——建议逐仓库验证

## 错误 6: "Skills ~56% 概率被跳过"缺乏验证

- **文件+行号**: `D5b-claude-code-community.md` (行 389)
- **原声称**: "skills（~56% 概率被跳过）"
- **错误类型**: 来源不支撑
- **正确信息**: lakshminp.com 博客对比了 hooks（100% 执行）vs skills 的可靠性差异。但 "~56% 概率被跳过" 这个精确数字在搜索结果中**未找到**。更常见的说法是 "Skills ~50-80% 触发率"（Tygart Media）——56% 可能被引用自特定测试但未经广泛验证
- **反证来源**: 
  - `tygartmedia.com/claude-code-plan-mode-hooks/` — "Skills are probabilistic (~50–80% trigger rate). Hooks are deterministic (100% execution)"
  - 56% 这个精确值缺乏来源

## 错误 7: 官方文档声称"8 种核心事件"与 D5a 的"12+"矛盾

- **文件+行号**: `D5b-claude-code-community.md` (行 247)
- **原声称**: "支持 8 种核心事件：PreToolUse、PostToolUse、PermissionRequest、PreCompact、SessionStart、Stop、SubagentStop、UserPromptSubmit"
- **错误类型**: 内部矛盾
- **正确信息**: D5b 声称官方支持 8 种核心事件，但 D5a (行 331-345) 列出 12 种（含 Setup、PostToolUseFailure、Notification、PermissionDenied、FileChanged）。两子报告对同一 Claude Code hooks 系统的事件计数不一致。差异可能源于官方文档区分了"核心事件"和其他类别（如 Setup 被视为 hook type 而非 event）
- **反证来源**: 
  - D5a (行 331-345) — 12+ 事件
  - D5b (行 247) — 8 核心事件
  - `code.claude.com/docs/en/hooks` — 官方文档为其一真值源（本次 WebFetch 被拦截无法直接读取）

## 错误 8: 中文社区资源 URL 可能部分失效或需验证

- **文件+行号**: `D5b-claude-code-community.md` (行 395-400)
- **原声称**: 列出 6 个中文社区来源
- **错误类型**: 来源不支撑（部分）
- **正确信息**: 
  - 腾讯云 `cloud.tencent.com.cn` URL 格式似乎不正确（腾讯云开发者社区域名应为 `cloud.tencent.com/developer`，中文站用 `.cn` 但路径格式不对）
  - `skywork.ai/blog/slide-template/...` — Skywork 的 blog 路径含 `/slide-template/` 看起来不像正常的技术博客路径
  - 其它来源（CSDN、w3cschool、GUVI）URL 格式正常但内容未做验证
- **反证来源**: 需逐 URL 访问验证
