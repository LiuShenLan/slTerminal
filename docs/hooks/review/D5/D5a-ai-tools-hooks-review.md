# D5a AI 编程工具 Hook 系统 -- 事实核查

## 错误 1: Cursor "18 个事件"计数不完整

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 14、604)
- **原声称**: "完整事件列表（18 个）"、"Hook 事件数: 18"
- **错误类型**: 事实错误
- **正确信息**: Cursor 实际文档（2026年7月）列出 **至少 21 个** hook 事件。D5a 遗漏了: `beforeTabFileRead`、`afterTabFileEdit`、`workspaceOpen`。此外 `beforeSubmitPrompt`、`afterAgentResponse`、`afterAgentThought`、`subagentStart`、`subagentStop`、`stop` 是 Cursor 3.11 (2026年7月) 新增。D5a 的列表含 18 个，但缺失 3 个已文档化的事件
- **反证来源**: 
  - Cursor 官方文档 `cursor.com/docs/hooks` — 21 个事件分 9 个类别
  - Cursor 3.11 changelog `cursor.com/en-US/changelog` — 2026年7月新增

## 错误 2: Cursor "五层配置体系"将 Rules 层次与 Hooks 层次混淆

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 45-53)
- **原声称**: "五层配置体系: 1. Team Rules 2. Project Rules 3. User Rules 4. AGENTS.md 5. Hooks"
- **错误类型**: 事实错误（分类混淆）
- **正确信息**: 
  - Rules 系统（4层）与 Hooks 系统（3-4层）是**独立配置维度**
  - Hooks 自身的配置层级是: Enterprise(`/etc/cursor/hooks.json`) → Team(cloud) → Project(`.cursor/hooks.json`) → User(`~/.cursor/hooks.json`) = **4层**
  - Rules 是另一套: Team Rules → Project Rules(`.cursor/rules/*.mdc`) → User Rules → AGENTS.md = **4层**
  - D5a 将两个独立维度混为一个"五层"，造成结构误解
- **反证来源**: 
  - Cursor Hooks 文档 — 独立的 `.cursor/hooks.json` 层级配置
  - Cursor Rules 文档 — 独立的 `.cursor/rules/` 目录结构

## 错误 3: Windsurf "12 个事件"实际是 11+1（transcript 是子事件）

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 73-89)
- **原声称**: 列出 12 个独立事件
- **错误类型**: 事实错误（轻微夸大）
- **正确信息**: Windsurf 文档 `docs.devin.ai/desktop/cascade/hooks` 确认该列表，但 `post_cascade_response_with_transcript` 被描述为 `post_cascade_response` 的"follow-up 事件"（附带完整 JSONL transcript）。严格说是 11 个独立事件 + 1 个子事件。此外 D5a 将 `post_setup_worktree` 列为独立事件——这个在文档中的定位需验证（主要事件表通常只列 11 个）
- **反证来源**: 
  - `docs.devin.ai/desktop/cascade/hooks` — 官方文档确认 11+1 结构
  - `dev.to/digitalapplied/windsurf-swe-15-cascade-hooks-complete-developer-guide-20fh` — 社区指南一致

## 错误 4: Windsurf 收购金额"约 2.5 亿美元"未经独立核实

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 67)
- **原声称**: "以约 2.5 亿美元收购"
- **错误类型**: 来源不支撑
- **正确信息**: 所有公开来源确认 Cognition 收购 Codeium/Windsurf（2025年），但**收购金额未被任何可靠来源独立披露**。"约2.5亿美元"在本次核查的所有搜索结果中均未出现——多家技术报道（apidog.com, dev.to, theblockbeats.news）均未提金额
- **反证来源**: 
  - `apidog.com/blog/whats-new-in-devin-2026/` — 确认收购但不提金额
  - `wwt.com/article/partner-pov-the-future-of-windsurf-at-cognition` — 同上

## 错误 5: Copilot CLI "13+6" 钩子计数有误导

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 134, 148, 606)
- **原声称**: "JSON 配置文件 Hook（13 个事件）" + "SDK 扩展 Hook（6 个）"
- **错误类型**: 事实错误（双重计数）
- **正确信息**: JSON 配置文件的 13 个事件名（sessionStart/sessionEnd/userPromptSubmitted/preToolUse/postToolUse/postToolUseFailure/permissionRequest/preCompact/agentStop/subagentStart/subagentStop/errorOccurred/notification）与 SDK 的 6 个核心 hook（onSessionStart/onUserPromptSubmitted/onPreToolUse/onPostToolUse/onErrorOccurred/onSessionEnd）**高度重叠**。它们是同一事件集在不同配置层的表达，不应相加为 19 个不同事件。正确计数应为 ~13 个不同事件，两种配置方式
- **反证来源**: 
  - D5a 自身 (行 134-138 vs 行 142-148) — JSON 13 事件名与 SDK 6 事件名明显重叠
  - `github.com/github/copilot-sdk` — SDK onSessionStart 对应 JSON sessionStart

## 错误 6: aider "24 个斜杠命令"严重低估

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 286、609)
- **原声称**: "内置 24 个斜杠命令"、"24 个内置命令，不可扩展"
- **错误类型**: 过时信息
- **正确信息**: aider v0.86.x (2026年5月) 已有 **约 43 个** in-chat 斜杠命令，含新增的 `/read-only`、`/editor-model`、`/weak-model`、`/think-tokens`、`/reasoning-effort`、`/cost`、`/save`、`/load`、`/paste`、`/copy`、`/lint`、`/help` 等。命令数远超 24。此外，第三方桌面包装器 AiderDesk 支持**自定义斜杠命令**（`.aider-desk/commands/`），但核心 aider 本身确实没有自定义命令扩展机制——这点表述准确
- **反证来源**: 
  - `computingforgeeks.com/aider-cheat-sheet/` — 完整命令列表
  - `deepwiki.com/Aider-AI/aider/2.3-commands-and-user-interactions` — 命令系统文档

## 错误 7: Claude Code hook 事件列表缺少最新事件

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 329-345)
- **原声称**: 列出 12 个生命周期事件
- **错误类型**: 事实错误（轻微不完整）
- **正确信息**: D5a 列出了 12 个核心事件 + 2 个实验性事件（ElicitationResult、WorktreeCreate）。但 `PermissionRequest`/`PermissionDenied`（D5b 行 247 提及）、`FileChanged` 在 D5a 列表中缺失。D5b 自身 (行 247) 列出官方文档支持 8 种核心事件，与 D5a 的 12 个列表矛盾——说明 D5a 和 D5b 之间对 Claude Code 事件计数不一致
- **反证来源**: 
  - D5b 子报告 (行 247) — 声称官方支持 8 种核心事件
  - D5a 子报告 (行 331-345) — 列出 12 种
  - 内部矛盾：两子报告对同一产品的 hook 事件计数差 4 个

## 错误 8: Claude Code "SessionStart" vs "Setup" 描述矛盾

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 333 vs D5b 行 15)
- **原声称**: D5a 列出 `Setup` 为独立一次性准备事件；D5b 在 disler 仓库中列出 `Setup` 和 `SessionStart` 为不同事件；官方 8 核心中不含 Setup
- **错误类型**: 内部矛盾
- **正确信息**: `Setup` 在 Claude Code hooks 中是否为独立"事件"还是配置钩子的"类型"存在歧义。官方文档将 Setup 列为 hook type 而非 hook event——它是 "一次性准备阶段" 的 hook 注册点，与 SessionStart 不同。D5a 将其计为独立事件可能不准确
- **反证来源**: 
  - `code.claude.com/docs/en/hooks` — Setup 作为 hook 类型出现
  - 社区文档将 Setup 视为事件——歧义源于官方自身文档

## 错误 9: Codex CLI "10 个事件"和"与 Claude Code 同契约"缺乏验证

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 183-184、608)
- **原声称**: "10 个生命周期事件"、"stdin JSON / stdout JSON（与 Claude Code 同契约）"
- **错误类型**: 来源不支撑（部分）
- **正确信息**: 
  - "10 个事件"在本次核查中**未能验证**（WebSearch 预算耗尽前未触达 Codex 官方 hook 文档）。OpenAI 文档 `developers.openai.com/codex/hooks` 可能包含不同计数
  - Codex CLI 使用 **JSON-RPC over stdio（JSONL 格式）**，其 App Server 协议是**双向** JSON-RPC（服务器可主动发起请求），与 Claude Code 的**单向** stdin→stdout 单次调用模型有显著区别。说"相同契约"不准确
  - Codex hooks 在 2026 年 4 月才稳定化——比报告声称的时间线晚
- **反证来源**: 
  - `openai.com/index/unlocking-the-codex-harness/` — App Server 架构描述，确认为双向 JSON-RPC
  - `developers.openai.com/codex/changelog` — 2026年4月 hooks 稳定化

## 错误 10: Gemini CLI 事件列表缺失重要上下文（免费版已弃用）

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 220-268)
- **原声称**: 完整描述 Gemini CLI hooks 系统
- **错误类型**: 过时信息
- **正确信息**: Google 于 2026年6月18日弃用了 Gemini CLI 免费/个人层级，推荐迁移到 **Antigravity CLI (`agy`)**。开源 `gemini` 二进制和付费 API key 不受影响。D5a 作为 2026-07-25 的研究，未提及此重大生态变化——对于评估"该工具的未来 hook 系统发展方向"，这是关键信息
- **反证来源**: geminicli.com 公告 (2026-06-18)

## 错误 11: Cursor events 列表中部分事件分类错误

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 16-29)
- **原声称**: `beforeShellExecution`、`afterShellExecution`、`beforeReadFile`、`afterFileEdit`、`beforeMCPExecution`、`afterMCPExecution` 等事件描述
- **错误类型**: 事实错误
- **正确信息**: 根据 Cursor 文档:
  - 只有 `beforeShellExecution` 和 `beforeMCPExecution` **可以阻断**操作（返回 `continue/permission` 决定）。`preToolUse`/`postToolUse` 虽然存在，但**不是** Cursor 的主要阻断机制
  - `beforeReadFile` 在 D5a 中被描述为可"交互/通知"，但实际文档中其阻断能力与 `beforeShellExecution` 不在同一级别
  - D5a 将 `permission` 字段三态（allow/deny/ask）归因于通用字段，但实际上这个响应格式是 beforeShellExecution/beforeMCPExecution 专属的
- **反证来源**: Cursor hooks 官方文档

## 错误 12: aider 与 AiderDesk 混淆

- **文件+行号**: `D5a-ai-tools-hooks.md` (行 273-301)
- **原声称**: aider 的描述集中在核心功能
- **错误类型**: 事实错误（遗漏重要上下文）
- **正确信息**: D5a 完全未提及 **AiderDesk**——这是 aider 的第三方桌面包装器，提供 30+ 生命周期 hook 事件、自定义斜杠命令、自定义工具注册等完整扩展系统。虽然 AiderDesk 不是 aider 核心的一部分，但它是 aider 生态中"怎么实现 hook"的主要答案。D5a 应至少注明这个第三方方案的存在
- **反证来源**: `deepwiki.com/hotovo/aider-desk/2.8-extension-system` — AiderDesk 扩展系统
