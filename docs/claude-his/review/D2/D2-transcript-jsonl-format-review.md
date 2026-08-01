# D2-transcript-jsonl-format 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D2

---

<!-- 逐条追加错误条目，格式见 output-spec.md「错误条目模板」。
     全部正确则写: 未发现错误（已验证 N 项声称）。 -->

已核查 16 个发现、约 60 项带来源声称（ccrider schema.md、toolpath messages.md / writing-compatible-jsonl.md、skiplevel schema-notes.md、agentyard、claude-code-types 源码与 index.d.ts、claude-threads SPECS、aresbit、mintlify session-persistence、08-message-types.en.md、classmethod 保留期文章、claude-session-player issue #2 与 protocol-schema.md、claude-wrapper docs.rs、simonw/claude-code-transcripts、claudex、claude_converter），共发现 4 处错误。

## 错误 1: isSidechain 零记录被错误归因于「252 个项目目录扫描」

- **文件+行号**: `docs/claude-his/D2-transcript-jsonl-format.md`（第 18 行，发现 2 详情段）
- **原声称**: "`isSidechain`（布尔，true 标记分支会话；**实测注意**：agentyard 在 252 个项目目录扫描中 `isSidechain=true` 的记录为零，不可用作子代理识别依据）"
- **错误类型**: 事实错误
- **正确信息**: agentyard 文档中「252」指本机项目目录总数（性能统计），与 isSidechain 完全无关；`isSidechain=true` 零记录的观测来自**单个样本**——"A 2.5MB transcript that spawned three subagents had zero records with `isSidechain=true`."（发现 12 ⑧ 的表述"实测零记录"正确，无 252 前缀）。
- **反证来源**: https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md
  - "On this machine: 252 project directories, ranging from active workspaces to one-off `/tmp` paths and skill-eval scratch dirs."（252 为目录数量）
  - "A scan of 252 project dirs returned in well under a second when not parsing file contents (just listing)."（252 扫描仅与列举性能相关）
  - "A 2.5MB transcript that spawned three subagents had zero records with `isSidechain=true`."（零记录来自此 2.5MB 样本）

## 错误 2: toolUseResult 字段清单含 ccrider 未记载的 filePath/structuredPatch

- **文件+行号**: `docs/claude-his/D2-transcript-jsonl-format.md`（第 48 行，发现 4 详情段）
- **原声称**: "可选字段 `toolUseResult`（工具结果摘要：字符串，或对象含 `stdout`/`stderr`/`interrupted`/`isImage`/`filePath`/`structuredPatch`；WebFetch 结果另有 `bytes`/`code`/`codeText`/`result`/`durationMs`/`url`；TodoWrite 另有 `oldTodos`/`newTodos`——来源: ccrider schema.md）"
- **错误类型**: 虚构术语（字段名在标注来源页面不存在）
- **正确信息**: ccrider schema.md 对 toolUseResult 仅列 `stdout`/`stderr`/`interrupted`/`isImage` + WebFetch 的 `bytes`/`code`/`codeText`/`result`/`durationMs`/`url` + TodoWrite 的 `oldTodos`/`newTodos`，**无 `filePath` 与 `structuredPatch`**。其余标注来源（toolpath messages.md、skiplevel、agentyard）亦未记载这两个字段。
- **反证来源**: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md
  - "Can be a string (error message) or object with:" 之后仅列举 `stdout`、`stderr`、`interrupted`、`isImage`，以及 WebFetch 场景的 "`bytes`, `code`, `codeText`, `result`, `durationMs`, `url`" 与 TodoWrite 场景的 "`oldTodos`, `newTodos` arrays"；全文无 `filePath` 或 `structuredPatch`。
  - https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md：全文无 `toolUseResult` 一词，亦无上述字段。

## 错误 3: progress 条目关键字段 toolName/toolInput 在来源中不存在

- **文件+行号**: `docs/claude-his/D2-transcript-jsonl-format.md`（第 33 行，发现 3 type 枚举表 progress 行）
- **原声称**: "| `progress` | 工具执行中的流式进度 | `toolName`、`toolInput`、`sessionId` |"
- **错误类型**: 虚构术语（字段名在来源页面不存在）
- **正确信息**: 标注来源 claude-code-types 的 ProgressEntry 源码为 `{ type: 'progress'; data: ProgressData; parentToolUseID?: string; toolUseID?: string }`（其余字段经 `extends Partial<EntryBase>` 继承，含可选 `sessionId`），**无 `toolName`、`toolInput`**；claude-session-player 对 progress 条目的分类亦按其 `data.type` 字段（bash_progress/hook_progress/agent_progress/query_update/search_results_received/waiting_for_task），而非 toolName。
- **反证来源**: https://raw.githubusercontent.com/pedropaulovc/claude-code-types/main/index.d.ts
  - "export interface ProgressEntry extends Partial<EntryBase> { type: 'progress'; data: ProgressData; parentToolUseID?: string; toolUseID?: string; }"
  - https://raw.githubusercontent.com/pedropaulovc/claude-code-types/main/docs/interfaces/ProgressEntry.md：字段表仅 `type`/`data` 必填，可选含 `toolUseID?`/`parentToolUseID?`/`sessionId?` 等，无 toolName/toolInput。

## 错误 4: 类型增长序列把 permission-mode/progress 归给 ccrider 2.0.29 观测

- **文件+行号**: `docs/claude-his/D2-transcript-jsonl-format.md`（第 120 行，发现 14 详情段 ②）
- **原声称**: "② 类型增长：claude-threads SPECS（`version` 1.0.63）仅见 `user|assistant` → 2.0.29（ccrider）见 summary/user/assistant/system/file-history-snapshot/permission-mode/progress → 2.1.x（skiplevel/agentyard）新增 attachment/ai-title/last-prompt/queue-operation/mode/saved_hook_context 等"
- **错误类型**: 事实错误（来源不支撑——把其他来源的观测归到 ccrider）
- **正确信息**: ccrider schema.md 的 Entry Types 一节仅列 5 种 type：`summary`/`user`/`assistant`/`system`/`file-history-snapshot`，**不含 `permission-mode` 与 `progress`**。permission-mode 见于 toolpath writing-compatible-jsonl.md（及 skiplevel/agentyard 的 2.1.x 样本），progress 见于 claude-code-types 与 2.1.x 观测。正确的演进序列为：1.0.63（claude-threads，user|assistant）→ 2.0.29（ccrider，5 种）→ 2.1.x（skiplevel/agentyard 新增 permission-mode/queue-operation/attachment/ai-title/last-prompt/pr-link/mode 等）。
- **反证来源**: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md
  - Entry Types 一节完整枚举："`summary` — Always the first line of a session file."、"`user` — Represents a message from the user."、"`assistant` — Represents Claude's response."、"`system` — System-generated messages (commands, status updates)."、"`file-history-snapshot` — Tracks file backup snapshots."
  - 全文无 permission-mode、progress、queue-operation、attachment、ai-title、last-prompt、mode、saved_hook_context、pr-link 等 type 值。

## 无法验证的声称（来源未覆盖，非错误）

- 发现 16 ① "最广泛使用的 HTML 导出工具"——simonw 仓库 README 无此表述（1.6k stars/196 forks 支撑流行度，但"最广泛"无法验证）
- 发现 16 ② "其解析器是字段级格式的独立实现"——claudex README 未描述 JSONL 解析实现细节
- 发现 7 "旧源码文档另记录 compact_boundary 含 summary 字段"——标注来源（08-message-types/skiplevel）未直接确认该字段
- 发现 7 "turn_duration 含 durationMs/messageCount（skiplevel）"——skiplevel 仅列 subtype 名未列字段；durationMs 有 claude-session-player issue 的 `get_duration_ms` 提取辅助间接支撑，messageCount 无来源
- 发现 12 ③ "读活动文件时容忍 EOF 处一条截断 JSON 行"——标注来源（skiplevel/agentyard）未含此条，实际支撑来自 mintlify（"Invalid or truncated lines (from a crash mid-write) are skipped"）
- 发现 12 ⑥ "列表页只扫文件名、内容惰性加载"——agentyard 的 252 目录"listing 不解析内容"为间接支撑，"列表页" UI 行为无直接来源
