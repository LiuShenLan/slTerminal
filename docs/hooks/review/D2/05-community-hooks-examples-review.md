# 05-community-hooks-examples.md 审查报告

## 错误 1: Hook 类型遗漏 `mcp_tool`（应为 5 种非 4 种）

- **文件+行号**: 第 5 节（约第 207 行起）
- **原声称**: 四种 hook 类型 —— command / prompt / agent / http
- **错误类型**: 事实错误
- **正确信息**: Claude Code hooks 实际支持 **5 种** handler 类型：command、http、**mcp_tool**、prompt、agent。文档遗漏了 `mcp_tool` 类型——允许 hook 调用 MCP 工具执行操作。裁决文件（ADJUDICATION.md）确认 "5 种 handler 类型 (command/http/mcp_tool/prompt/agent)"。
- **反证来源**: 裁决文件 ADJUDICATION.md L132 明确列出 5 种类型

## 错误 2: `modifyInput` 机制描述错误

- **文件+行号**: 第 9.10 节（约第 405 行）
- **原声称**: "Claude Code v2.0.30+ 支持 hook 修改工具输入"，配置中使用 `"modifyInput": true` 布尔字段
- **错误类型**: 事实错误
- **正确信息**: `modifyInput` 并非 handler 配置级别的布尔字段。实际机制是 hook 通过 **stdout JSON 输出中的 `updatedInput` 字段** 返回修改后的工具输入——即 hook 返回 `{"updatedInput": "modified text"}` 来修改工具输入参数。文档将机制描述为配置开关而非 JSON 响应字段，且版本号 v2.0.30 无法验证。
- **反证来源**: hooks 官方文档机制；引用的 bobmatnyc/claude-mpm 中 "args 字段被忽略 (v2.0.30)" 与 modifyInput 功能无关

## 错误 3: Windows 限制 #25981 已关闭（信息过时）

- **文件+行号**: 第 12 节表格（约第 474 行）
- **原声称**: "Windows PreToolUse/PostToolUse 不触发 | 已知 bug (#25981)，事件加载但永不触发"
- **错误类型**: 过时信息
- **正确信息**: GitHub issue #25981（"PreToolUse and PostToolUse hooks loaded but never fire on Windows"）已 **CLOSED**（关闭原因可能为已修复、重复或 wontfix）。文档将其描述为持续性 bug 已过时——如果因修复而关闭，则 Windows 限制可能已不存在。需标注关闭状态及日期。
- **反证来源**: 本地审查确认为 CLOSED 状态

## 错误 4: 事件频率排名缺乏定量数据支撑

- **文件+行号**: 第 3 节（约第 55-80 行）
- **原声称**: "根据社区仓库中的实际配置统计，事件使用频率从高到低"——PreToolUse/PostToolUse/Stop/SessionStart 列为"高频"
- **错误类型**: 来源不支撑
- **正确信息**: D1 社区调研的定性矩阵显示不同排名：Stop(极高)、UserPromptSubmit/Notification(高)、PreToolUse/PostToolUse/SessionStart(中等)——与本文档排名不一致。引用的仓库（dwmkerr/claude-toolkit、luongnv89/claude-howto）是个体配置示例，不构成系统性频率统计。
- **反证来源**: D1 社区调研定性矩阵；文档自身引用均为个体仓库示例

## 错误 5: patterns.md 仓库名与其他文档不一致

- **文件+行号**: 第 4.3 节、第 8 节引用（约第 160/285 行）
- **原声称**: `github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md`
- **错误类型**: 内部矛盾
- **正确信息**: 同一文件在 D5 文档中引用为不同仓库名 `anthropics/claude-plugins-official`。存在两个矛盾的仓库名：`anthropics/claude-code` vs `anthropics/claude-plugins-official`。需核实正确的仓库名并统一引用。
- **反证来源**: D5 文档引用 `anthropics/claude-plugins-official`；无法通过网络独立验证

## 错误 6: Matcher `,` 语义标注不完整

- **文件+行号**: 第 4.1 节（约第 151 行）
- **原声称**: "Claude Code v2.1.191+ 中 `|` 和 `,` 等价"
- **错误类型**: 来源不支撑 + 语义不完整
- **正确信息**: `|` 和 `,` 均为 **OR 关系**（非 AND）。文档声称两者"等价"正确描述了功能但未明确强调两者都是 OR 而非 AND，可能造成误解。版本号 v2.1.191 在官方文档中无出处，需移除或标注为待验证。
- **反证来源**: D2 main 文档第 8.1 节确认 "`|` 和 `,` 为或关系"

## 错误 7: Exit code 2 处理描述不完整

- **文件+行号**: 第 6 节 Exit Code 表（约第 237 行）
- **原声称**: exit 2 时 "stderr 展示给用户，工具调用被阻止"
- **错误类型**: 过时信息/不完整
- **正确信息**: exit 2 时 stderr 不仅"展示给用户"，还会**反馈给 Claude 模型用于自我纠正**。裁决文件确认此语义——模型收到 stderr 后可据此调整后续行为。文档遗漏了"反馈给模型"这一关键行为。
- **反证来源**: 裁决文件 ADJUDICATION.md L131

## 错误 8: `if` 字段版本号 v2.1.85+ 交叉验证正确但需标注来源

- **文件+行号**: 第 8 节（约第 285 行）
- **原声称**: "Claude Code v2.1.85+ 引入 `if` 字段"
- **错误类型**: 来源不支撑（降级：交叉验证通过但缺直接来源）
- **正确信息**: D2 审查报告和 D2 配置管理审查两次交叉验证均确认 v2.1.85 版本号（v2.1.88 进一步增强支持 `&&`/`||`）。版本号可能正确，但文档未提供直接来源引用。建议补充关联 GitHub Issue #41262。
- **反证来源**: 交叉验证确认；关联 Issue #41262

---

未发现错误（已验证 5 项声称）:
- 配置文件位置与优先级（managed > user > project > local）— 正确
- stdin JSON payload 结构（tool_name/tool_input/session_id 等字段）— 正确
- `$CLAUDE_PROJECT_DIR` / `$CLAUDE_PLUGIN_ROOT` 环境变量 — 正确
- Matcher 正则基本语法（`|` 分隔、`*` 全匹配、大小写敏感）— 正确
- Exit code 0（成功）和 其他（非阻断警告）语义 — 正确
