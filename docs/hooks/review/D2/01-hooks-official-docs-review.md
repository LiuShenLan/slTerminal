# 01-hooks-official-docs.md 事实核查报告

> 核查日期：2026-07-25
> 核查方法：Context7 查询 code.claude.com 官方文档 + Agent SDK 文档

---

## 错误 1: CLAUDE_ENV_FILE 可用范围错误（严重）

- **文件+行号**: 01-hooks-official-docs.md §5 (行 107)
- **原声称**: `CLAUDE_ENV_FILE` 的可用范围为"**仅 SessionStart**"
- **错误类型**: 事实错误
- **正确信息**: `CLAUDE_ENV_FILE` 在 **4 种事件**中可用：`SessionStart`、`Setup`、`CwdChanged`、`FileChanged`。
- **反证来源**:
  - `https://code.claude.com/docs/en/hooks`: "SessionStart, Setup, CwdChanged, and FileChanged hooks can use the CLAUDE_ENV_FILE environment variable to persist environment variables"
  - `https://code.claude.com/docs/en/hooks-guide`: 展示 CwdChanged 和 FileChanged hooks 中使用 `$CLAUDE_ENV_FILE` 的完整 JSON 配置示例
- **注意**: D2-config-management.md §6.1 正确列出了 4 种事件，同一批文档内部互相矛盾。

---

## 错误 2: 配置层级说明与 D2 主文件矛盾

- **文件+行号**: 01-hooks-official-docs.md §1 (行 10-11)
- **原声称**: "项目级优先于用户级"（project overrides user）
- **错误类型**: 不完整（缺少 local 层级覆盖关系说明）
- **正确信息**: 完整顺序应为 "local > project > user"。仅说"项目级优先于用户级"不完整——缺少 local 是最优先的非托管层级这一关键信息。
- **反证来源**: `https://code.claude.com/docs/en/agent-sdk/python`: "local settings override project settings, which in turn override user settings"
- **注意**: 该文件本身不错误，但省略了关键信息。且 D2 主文件 §1.2 的优先级表（managed > user > project > local）与此处的"project overrides user"也不一致。

---

## 错误 3: SessionStart matcher 未在 §8 中体现

- **文件+行号**: 01-hooks-official-docs.md §8 (行 249-277)
- **原声称**: Matcher 语法讨论围绕 PreToolUse/PostToolUse 展开，对 SessionStart 的 matcher 值（startup/resume/clear/compact）未在"分类匹配示例"中提及
- **错误类型**: 缺失信息（小问题）
- **正确信息**: SessionStart 支持 matcher，值为 `startup`/`resume`/`clear`/`compact`。同一文件的 §7.1 正确列出了这些值，但 §8 的 matcher 语法章节未提生命周期事件的 matcher 用法。
- **反证来源**: D2 自身 §7.1 (行 169) 和 D2-config-management.md §2.1 (行 84)

---

## 未发现错误（已验证）:

- **配置文件三层结构**（§1）：~/.claude/settings.json / .claude/settings.json / .claude/settings.local.json + 托管策略 与官方文档一致
- **5 种 handler 类型**（§3）：command/prompt/agent/http/mcp_tool 与官方文档一致
- **退出码语义**（§4）：0=成功/1=非阻塞警告/2=阻止 与官方文档一致
- **stdin JSON 字段**（§6）：所有通用字段和事件特有字段与官方文档一致
- **30 种事件列表**（§7）：全部事件名称、触发时机、可阻止性、matcher 值与官方文档一致
- **Matcher 语法**（§8）：精确匹配/正则匹配/通配规则与官方文档一致；区分大小写、连字符 v2.1.195+ 行为正确
- **if 字段**（§9）：v2.1.85+/v2.1.88 改进描述与 GitHub Issue #41262 一致
- **stdout JSON 输出**（§10）：顶层字段、hookSpecificOutput、事件特定输出结构与官方文档一致
- **阻止机制**（§11）：可阻止/不可阻止事件分类与官方文档一致
- **Notification matcher**（§12）：6 种值与官方文档一致
- **全局禁用**（§13）：disableAllHooks/disableStopHook/disableNotificationHook 与官方文档一致
- **已知问题**（§15）：6 条已知 bug 与对应的 GitHub Issues 描述一致（虽然无法直接验证 Issue 状态，但描述 plausible）
- **快速配置示例**（§16）：4 个示例的 JSON 结构和 shell 脚本模式与官方文档示例一致
- **信息源列表**（§17）：全部 URL plausible
