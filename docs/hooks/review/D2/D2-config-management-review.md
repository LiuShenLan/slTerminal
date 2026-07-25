# D2-config-management.md 事实核查报告

> 核查日期：2026-07-25
> 核查方法：Context7 查询 code.claude.com 官方文档 + Agent SDK 文档

---

## 错误 1: 配置层级优先级顺序错误（严重）

- **文件+行号**: D2-config-management.md §1.2 (行 59-64) + §14.1 (行 724)
- **原声称**: 优先级从高到低为 `托管策略 > ~/.claude/settings.json(用户) > .claude/settings.json(项目) > .claude/settings.local.json(本地)`。即 `managed > user > project > local`。
- **错误类型**: 事实错误
- **正确信息**: 正确的优先级顺序是 `managed > local > project > user`。本地设置覆盖项目设置，项目设置覆盖用户设置。托管策略始终最高优先级。
- **反证来源**:
  - `https://code.claude.com/docs/en/debug-your-config`: "closer scope overrides the broader one in the order **local, then project, then user**"
  - `https://code.claude.com/docs/en/agent-sdk/python`: "local settings override project settings, which in turn override user settings"
  - `https://code.claude.com/docs/en/agent-sdk/typescript`: "local settings override project settings, which in turn override user settings"
- **影响**: D2 §14.1 的配置层级合并可视化描述也复用了此错误顺序，需同步修正。

---

## 错误 2: Matcher 精确匹配字符集缺少空格

- **文件+行号**: D2-config-management.md §4.1 (行 262)
- **原声称**: "仅含字母/数字/`_`/`-`/`|`/`,`" 视为精确字符串匹配
- **错误类型**: 事实错误（不完整）
- **正确信息**: 官方文档包含 **空格**："letters, digits, `_`, `-`, **spaces**, `,`, and `|`"
- **反证来源**: `https://code.claude.com/docs/en/hooks`（Matcher patterns 节）
- **注意**: D1 声称包括空格，D2 的 charset 与 D1 不一致但 D1 是正确的。D2 丢掉了空格。

---

## 错误 3: FileChanged/StopFailure matcher 精确匹配字符集更窄，未注明

- **文件+行号**: D2-config-management.md §4.1 (行 259-264) + §2.8 (行 147)
- **原声称**: 所有事件的 matcher 精确匹配字符集相同
- **错误类型**: 缺失信息
- **正确信息**: `FileChanged` 和 `StopFailure` 事件的 matcher 有**更窄**的精确匹配字符集——仅接受字母、数字、`_`、`|`。对于这两个事件，连字符(`-`)、空格、逗号会强制走正则表达式路径，且只有 `|` 作为分隔符。
- **反证来源**: `https://code.claude.com/docs/en/hooks`: "The `FileChanged` and `StopFailure` events have a narrower exact-match set, accepting only letters, digits, `_`, and `|`. For these events, a hyphen, space, or comma forces the matcher onto the regular-expression path, and only `|` separates alternatives."

---

## 错误 4: §7.2 不可阻止事件汇总不完整

- **文件+行号**: D2-config-management.md §7.2 (行 465)
- **原声称**: 不可阻止事件列表 14 个
- **错误类型**: 不完整
- **正确信息**: §7.1 列出 9 个可阻止事件 + §7.2 列出 14 个不可阻止事件 = 23 个。总事件 30 个。缺失 7 个事件的阻止性说明（ConfigChange、Elicitation、ElicitationResult、UserPromptExpansion、WorktreeCreate 均为可阻止；PermissionDenied、Setup 为不可阻止）。建议汇总表覆盖全部 30 个事件。
- **反证来源**: D2 自身 §2 子表中的可阻止标记（内洽矛盾）

---

## 未发现错误（已验证）:

- **30 种事件列表**（§2）：所有事件名称和触发时机与官方文档一致
- **5 种 handler 类型**（§3）：command/http/mcp_tool/prompt/agent 与官方文档一致
- **退出码语义**（§6.2）：exit 0/1/2 行为与官方文档一致
- **Notification matcher 6 值**（§2.4）：与官方文档一致。官方还提到 v2.1.198+ 新增 agent_needs_input/agent_completed（时效性差异，非错误）
- **CLAUDE_ENV_FILE 4 事件范围**（§6.1）：SessionStart/Setup/CwdChanged/FileChanged 与官方文档一致
- **PreCompact 可阻止**（§2.6）：与官方文档 "block compaction by exiting with code 2" 一致
- **超时默认值**（§8.1）：command/http/mcp_tool 600s、prompt 30s、agent 60s 与官方文档一致
- **if 字段**（§4.3）：v2.1.85+/v2.1.88 描述与官方文档和 GitHub Issue #41262 一致
- **SchemaStore URL**：`https://json.schemastore.org/claude-code-settings.json` 为公认 Schema 托管地址
- **GitHub Issues**（§13）：所有编号为 plausible 格式，无法直接验证但不影响可信度

---

## 其他

1. **B8 "args 字段被忽略"** 和 **B9 "URL 无变量展开"**（§13，行 708-709）未标注 Issue 编号，建议补充来源或标记为"社区报告/未确认"。
2. **§14.4 `schemars` crate** 为 slTerminal 实现建议，非 D2 研究错误项，无需标记。
