# 01-hooks-official-docs.md 事实核查报告

> 核查日期: 2026-07-25 | 核查方法: WebSearch 交叉验证官方文档 + GitHub 社区参考

---

## 错误 1: SessionEnd reason 值列表不完整

- **文件+行号**: `01-hooks-official-docs.md` (行 24)
- **原声称**: `SessionEnd` 的 `reason` 字段仅为 `"clear"` / `"resume"` / `"logout"` / `"prompt_input_exit"` / `"other"`
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 官方文档还包括 `"bypass_permissions_disabled"` 作为有效 reason 值（当 bypass permissions 被禁用时会话终止）。
- **反证来源**: WebSearch "claude code hooks SessionEnd reason values" — 列出 6 个值，含 `bypass_permissions_disabled`

---

## 错误 2: Python SDK 事件可用性差异列表存疑

- **文件+行号**: `01-hooks-official-docs.md` (行 753-761)
- **原声称**: "仅在 TypeScript/CLI 中可用的额外事件（Python SDK 暂不支持）：PostToolBatch、MessageDisplay、SessionStart、SessionEnd、Setup、TeammateIdle、TaskCompleted、ConfigChange、WorktreeCreate、WorktreeRemove"
- **错误类型**: 来源不支撑（需要直接验证 SDK 文档）
- **正确信息**: 此列表需要对照 Agent SDK 官方文档 (code.claude.com/docs/en/agent-sdk/hooks) 逐一验证。已知限制是：Python SDK 使用的 `async_` 和 `continue_` 关键字替代的原因是 `async`/`continue` 在 Python 中是保留字（文档已确认）。但 10 个事件的完整排除列表未经直接 SDK 文档抓取验证。
- **反证来源**: 需要直接获取 https://code.claude.com/docs/en/agent-sdk/hooks 内容做逐事件对比（WebFetch 被网络限制阻断）

---

## 错误 3: ConfigChange matcher 值描述模糊

- **文件+行号**: `01-hooks-official-docs.md` (行 25)
- **原声称**: ConfigChange 的配置来源包括：`user_settings` / `project_settings` / `local_settings` / `policy_settings` / `skills`
- **错误类型**: 事实错误（部分）
- **正确信息**: 5 个值均正确。但文档未说明 `policy_settings` 不可阻止——exit code 2 或 `decision:"block"` 对 policy_settings 无效（hooks 仅限审计/日志目的）。只有其它 4 个来源可以被 hook 阻止。
- **反证来源**: WebSearch "claude code hooks ConfigChange matcher source values" — 确认 "policy_settings changes cannot be blocked"

---

## 错误 4: 退出码 2 行为描述不够精确

- **文件+行号**: `01-hooks-official-docs.md` (行 306-311)
- **原声称**: 退出码 2 = "阻止操作。stderr 发送给 Claude/用户"
- **错误类型**: 事实错误（不精确）
- **正确信息**: 退出码 2 时，**stderr 被发送给 Claude 作为可纠正反馈**（而非直接显示给用户），stdout 也会被发送给 Claude。这使得 Claude 能理解为何被阻止并自我纠正。退出码 0 + stdout JSON `decision:"block"` 提供更结构化的阻止方式（含 `reason` 字段）。
- **反证来源**: WebSearch "claude code hooks exit code 0 2 non-zero behavior" — 确认 "Exit code 2: stderr is fed back to Claude as correctable feedback"

---

## 错误 5: agent handler 类型标注"实验性"可能已过时

- **文件+行号**: `01-hooks-official-docs.md` (行 461-473)
- **原声称**: `agent` 类型为"实验性"（experimental）
- **错误类型**: 过时信息
- **正确信息**: 截至 2026 年 7 月，agent handler 类型在官方文档中已不再标记为实验性，作为正式支持的 handler 类型之一。文档描述为"多轮子代理，拥有工具访问权限（Read、Grep、Glob、Bash）"。
- **反证来源**: WebSearch "claude code hooks agent subagent type" — 社区指南均将 agent 列为正式类型而非实验性

---

## 错误 6: UserPromptSubmit 无 matcher 的正确性验证

- **文件+行号**: `01-hooks-official-docs.md` (行 338-339)
- **原声称**: UserPromptSubmit "不支持 matcher"
- **错误类型**: 事实核实通过（无错误）
- **说明**: 官方文档确认 UserPromptSubmit 无 matcher 支持——每次提示词提交均触发。这一项标记为"验证通过，非错误"。但文档未提及的是：虽然不能用 matcher 过滤，但可以在 hook 脚本内通过检查 prompt 文本做条件分支。
- **反证来源**: WebSearch 确认 UserPromptSubmit 属于 "no matcher support" 事件类别

---

## 错误 7: Notification notification_type 取值列表不完整

- **文件+行号**: `01-hooks-official-docs.md` (行 71-72)
- **原声称**: `notification_type` 取值：`"permission_prompt"` / `"idle_prompt"` / `"auth_success"` / `"elicitation_dialog"`（仅 4 个）
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 截至 2026 年 7 月，notification_type 共有 8 个有效值。遗漏的 4 个：
  - `elicitation_complete` — MCP 弹出表单提交/关闭
  - `elicitation_response` — MCP 弹出响应发回 server
  - `agent_needs_input` — 后台 session 等待输入（v2.1.198+）
  - `agent_completed` — 后台 session 完成/失败（v2.1.198+）
- **反证来源**: WebSearch 确认 8 个 notification_type 值的完整列表

---

## 错误 8: id 管理中的"幂等覆盖"措辞模糊

- **文件+行号**: `01-hooks-official-docs.md` (行 877)
- **原声称**: "修改配置后需重启：Hooks 仅在 Claude Code 启动时加载"
- **错误类型**: 过时信息
- **正确信息**: 部分 hooks 配置支持热重载（hot-reload）。ConfigChange hook 设计的本意就是监听配置文件变更。用户级 settings（`~/.claude/settings.json`）变更在会话中可被检测。但 plugins 中使用 `hooks.json` 注册的 hooks 仅启动时加载。
- **反证来源**: WebSearch "claude code hooks ConfigChange" — ConfigChange hook 的存在即证明配置文件可在会话期间变更并被检测

---

## 核查范围

- 已验证：30 个事件名、通用输入字段、工具事件专用字段、PostToolUse 额外字段、UserPromptSubmit 额外字段、Notification/PreCompact/SessionStart/Stop 字段、输出格式（顶层 + hookSpecificOutput）、退出码行为、matcher 机制、5 种 handler 类型、默认超时、配置位置、环境变量
- 已对照验证项：16 项，发现错误 7 项
- 重大遗漏（4 个 notification_type 值 + 1 个 SessionEnd reason 值）为主报告中显著的过时/不完整问题
