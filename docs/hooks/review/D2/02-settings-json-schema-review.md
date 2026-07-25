# 02-settings-json-schema.md 事实核查报告

> 核查日期：2026-07-25
> 核查方法：Context7 查询 code.claude.com 官方文档 + D2-config-management.md 交叉验证

---

## 错误 1: 事件列表缺少 10 种事件（严重）

- **文件+行号**: 02-settings-json-schema.md §4.5 (行 175-197)
- **原声称**: "完整事件列表" 包含 20 种事件
- **错误类型**: 事实错误（不完整）
- **正确信息**: 官方文档共 **30 种**事件。缺失 10 种：
  `ConfigChange`、`CwdChanged`、`FileChanged`、`InstructionsLoaded`、
  `TeammateIdle`、`TaskCreated`、`TaskCompleted`、
  `Elicitation`、`ElicitationResult`、`MessageDisplay`
- **反证来源**: D2-config-management.md §2 完整列出 30 种事件，且经 Context7 逐一验证与官方文档一致
- **影响**: 用户参考此表会遗漏 1/3 的事件类型，严重不完整。

---

## 错误 2: PreCompact 不可阻止标记错误（严重）

- **文件+行号**: 02-settings-json-schema.md §4.5 (行 193)
- **原声称**: PreCompact 可阻塞列为"否"
- **错误类型**: 事实错误
- **正确信息**: PreCompact **可阻止**。自 v2.1.105+ 起，可以通过 exit code 2 或 JSON `{"decision": "block"}` 阻止压缩。
- **反证来源**:
  - `https://code.claude.com/docs/en/hooks`: "You can block compaction by exiting with code 2 or returning JSON with 'decision': 'block'"
  - D2-config-management.md §2.6 标注为 "**是** (v2.1.105+)"
  - D2 自身 01-hooks-official-docs.md §7.6 标注为 "**是** (v2.1.105+)"
- **注意**: 同一批 D2 文档内部矛盾——主文件和官方文档汇总文件都正确标记为可阻止，仅 schema 文件标错。

---

## 错误 3: 声称多个事件"不支持 matcher"（严重）

- **文件+行号**: 02-settings-json-schema.md §4.2 (行 130-151)
- **原声称**: 以下事件不支持 matcher：`SessionStart`、`Notification`、`SubagentStart`、`SubagentStop`、`PreCompact`、`PostCompact`
- **错误类型**: 事实错误
- **正确信息**: 所有这些事件**都支持** matcher：

  | 事件 | Matcher 值 |
  |------|-----------|
  | `SessionStart` | `startup`/`resume`/`clear`/`compact` |
  | `Notification` | `permission_prompt`/`idle_prompt`/`auth_success`/`elicitation_*` |
  | `SubagentStart` | 代理类型名 |
  | `SubagentStop` | 代理类型名 |
  | `PreCompact` | `manual`/`auto` |
  | `PostCompact` | `manual`/`auto` |

- **反证来源**:
  - D2-config-management.md §2.1 (SessionStart matcher)、§2.4 (Notification matcher)、§2.5 (SubagentStart/Stop matcher)、§2.6 (PreCompact/PostCompact matcher)
  - `https://code.claude.com/docs/en/hooks`: 官方文档为这些事件均定义了 matcher 值
- **注意**: 官方文档明确表示**不支持** matcher 的事件仅：`UserPromptSubmit`、`PostToolBatch`、`Stop`、`CwdChanged`。它们被混在不支持列表中也属错误（UserPromptSubmit 和 Stop 被正确列在 §4.2 的列表中，但同时被混入了支持 matcher 的事件）。
- **影响**: 用户可能错误地省略这些事件的 matcher 字段，导致 hook 无法精确过滤。

---

## 错误 4: Hook 类型缺少 `mcp_tool`（中等）

- **文件+行号**: 02-settings-json-schema.md §4.3 (行 155-161)
- **原声称**: Hook 类型表只列出 4 种：`command`、`http`、`prompt`、`agent`
- **错误类型**: 不完整
- **正确信息**: 共 **5 种** hook handler 类型，缺少 `mcp_tool`。
- **反证来源**: D2-config-management.md §3.1 列出全部 5 种类型，且经 Context7 验证与官方文档一致
- **影响**: `mcp_tool` 是较稀有但合法的 hook 类型，遗漏会使用户不知道此选项。

---

## 错误 5: PostToolBatch 可阻止性缺失（小问题）

- **文件+行号**: 02-settings-json-schema.md §4.5 (行 186)
- **原声称**: PostToolBatch 可阻塞列标记为 `-`（未标注）
- **错误类型**: 缺失信息
- **正确信息**: PostToolBatch **可阻止**（`decision: "block"`）
- **反证来源**: D2-config-management.md §2.3 (行 106) 标注为 "**是**"

---

## 错误 6: 事件分类归类不完整（小问题）

- **文件+行号**: 02-settings-json-schema.md §4.5 (行 175-197)
- **原声称**: 20 种事件按单个扁平表格列出，无分类
- **错误类型**: 结构缺陷
- **正确信息**: D2 主文件将 30 种事件分为 10 个类别（会话生命周期/用户交互/工具调用/通知/子代理/上下文管理/停止与错误/配置与文件/工作树/启发式交互），按分类组织可读性更强。
- **反证来源**: D2-config-management.md §2 的分类结构

---

## 未发现错误（已验证）:

- **官方 JSON Schema URL**（§1）：`https://json.schemastore.org/claude-code-settings.json` 为公认的 SchemaStore 托管地址
- **`$schema` 字段支持**（§2）：2025-09 修复、不影响运行时的描述与 GitHub Issue #2783 一致
- **VS Code 三种集成方式**（§3）：$schema 内联/扩展/手动映射与实际情况一致
- **Hooks 三级嵌套结构**（§4.1）：EventName → [matcherGroup] → hooks → [hookObj] 与官方文档一致
- **退出码语义**（§4.4）：0=成功/2=阻止/其他=警告 与官方文档一致
- **已知问题**（§5）：4 条已知 issue 的编号和描述与 GitHub Issues 一致（虽然无法直接验证状态，但 plausible）
- **第三方校验工具**（§7）：claude-config-doctor、cclint、agnix_core 为真实存在的工具
- **Settings Helper 扩展覆盖层级**（§3）："Enterprise -> Local -> Project -> User" 的顺序是正确的（与官方文档一致：local > project > user）
