# Claude Code 会话存储机制 — transcript JSONL 数据格式

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论
>
> **重要前提**：Claude Code 的 transcript 格式是**未文档化的内部格式**（undocumented internal format），Anthropic 官方未发布字段级规范。本报告全部字段信息来自对真实 `~/.claude/projects/` 文件的社区逆向观测（ccrider/skiplevel/agentyard）、Claude Code 源码提取文档（mintlify 镜像 / claude-code-source-all-in-one）以及社区解析工具实现。以下每个字段名均在对应来源页面中逐词确认。

## 关键发现

### 发现 1: 存储位置与文件命名规则

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md（探测版本 2.0.29）；https://dev.classmethod.jp/en/articles/claude-code-conversation-history-retention/（2026-07-04）
- 详情: 会话以 JSONL 文件存储于 `~/.claude/projects/[project-path]/[sessionId].jsonl`；`[project-path]` 是工作目录绝对路径的编码——非字母数字字符（含 `/`、`_`、`:` 等）全部替换为 `-`（如 `/Users/neil/xuku/invoice` → `-Users-neil-xuku-invoice`；Windows 例 `C:\dev\foo_bar` → `C--dev-foo-bar`，双破折号来自 `:` 与 `\` 连续替换）。主会话文件名为 `<uuid>.jsonl`，uuid 为 UUIDv4，文件名主干即记录的 `sessionId` 字段。子代理会话为 `agent-[agentId].jsonl`（另有 `agent-<id>.meta.json` 元数据，含 `agentType`/`worktreePath`/`description`/`name`/`toolUseId` 字段，来源: https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md，探测版本 2.1.145）。另有全局命令历史 `~/.claude/history.jsonl`，条目为 `{"display": ..., "pastedContents": {}, "timestamp": <Unix 毫秒>, "project": <绝对路径>}`。**布局演进**：Claude Code 源码提取文档（https://mintlify.wiki/sanbuphy/claude-code-source-code/architecture/session-persistence）显示早期布局为 `~/.claude/projects/<hash>/sessions/<session-id>.jsonl` 且 sessionId 为 `<prefix><8 个字母数字>` 短格式（前缀 b=Bash 任务/a=Agent 任务/r=Remote 任务/t=teammate 任务/无前缀=主交互会话），与当前 `<encoded-path>/<uuidv4>.jsonl` 布局不同——说明布局经历过版本演进。

### 发现 2: 每条记录的信封字段（envelope）

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md；https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md；https://github.com/repowise-dev/skiplevel/blob/main/docs/schema-notes.md（观察日期 2026-06-11，版本 ~2.1.x）
- 详情: user/assistant/system 条目共享的信封字段（逐字）：`type`（条目类型判别符）、`uuid`（每条消息 UUIDv4）、`parentUuid`（前一条消息的 uuid，首条为 null，形成树）、`timestamp`（ISO 8601 UTC，如 `2026-05-13T05:17:57.966Z`，毫秒精度带 `Z` 后缀）、`sessionId`（与会话文件名一致）、`cwd`（发送消息时的工作目录，会话内可变）、`gitBranch`（非 git 仓库时为空字符串而非 null）、`version`（Claude Code 版本号，如 `"2.0.29"`/`"2.1.51"`）、`userType`（通常 `"external"`，另有 `"internal"` 取值，来源: agentyard）、`entrypoint`（典型取值 `"cli"`）、`isSidechain`（布尔，true 标记分支会话；**实测注意**：agentyard 在单个 2.5MB transcript 样本（派生了三个子代理）中 `isSidechain=true` 的记录为零——「252」指其本机项目目录总数，与该观测无关，不可用作子代理识别依据）、`isMeta`（布尔，true 表示 harness 注入的元消息，仅部分条目有）。assistant 条目另有 `requestId`（`req_` 前缀 API 请求标识）。`cwd` 是权威项目路径——agentyard 指出目录名编码 `path.replace('/', '-').replace('.', '-')` 有损、无法反推真实路径，必须读记录内 `cwd` 字段。

### 发现 3: type 字段枚举全景（截至 2026-08-01 观测）

- 来源: https://github.com/pedropaulovc/claude-code-types（9 种，TypeScript 类型定义）；https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md；https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md
- 详情: 各来源观测到的顶层 `type` 值合并如下（不同版本观测到子集不同，未知类型必须忽略而非报错）：

| type | 含义 | 关键字段 |
|------|------|---------|
| `summary` | 会话文件首行 | `summary`（人类可读标题）、`leafUuid`（最后一条消息的 uuid，可为 null） |
| `user` | 用户输入或工具结果载体 | `message`、可选 `thinkingMetadata`/`toolUseResult` |
| `assistant` | 模型响应与工具调用 | `message`、`requestId` |
| `system` | 合成消息（命令输出/压缩边界/错误等） | `subtype`、`content`、`level`、`isMeta` |
| `file-history-snapshot` | 文件备份簿记（undo/restore） | `messageId`、`isSnapshotUpdate`、`snapshot`（含 `trackedFileBackups` 映射文件路径→`backupFileName`/`version`/`backupTime`） |
| `permission-mode` | 权限模式记录，**恰好三字段** | `permissionMode`、`sessionId`——多任何字段加载器即拒绝（来源: toolpath writing-compatible-jsonl.md） |
| `progress` | 工具执行中的流式进度 | `data`（`ProgressData`）、`toolUseID`、`parentToolUseID`（claude-code-types 源码） |
| `queue-operation` | 后台任务/排队用户消息 | — |
| `attachment` | harness 附件（deferred tools 等） | `attachment`（`type` 为 `memory`/`skill`/`file`/`context`，源码文档） |
| `ai-title` | 自动生成的会话标题 | 覆写式写入，last wins（agentyard） |
| `last-prompt` | 上次提示词 | 覆写式写入，last wins（agentyard） |
| `pr-link` | 关联的 pull request | — |
| `mode` | 模式记录 | — |
| `saved_hook_context` | hook 执行上下文 | — |

- 早期版本差异: claude-threads SPECS（https://raw.githubusercontent.com/nibzard/claude-threads/refs/heads/main/SPECS.md，记录内 `version` 为 1.0.63）观测到 type 枚举仅 `user|assistant` 两种，未见 summary/system 等——格式随版本增长。
- 注意: https://github.com/aresbit/claude-save/blob/main/CLAUDE_JSONL_SCHEMA_ANALYSIS.md 提出的 `tool_call`/`tool_result`/`file_operation`/`context_change`/`error`/`metadata`/`session_event` 等类型为**推测性假设**（文档自述 "likely contain these additional data types"），与真实格式不符，不作事实引用。

### 发现 4: user 条目结构（type: "user"）

- 来源: https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md；https://github.com/repowise-dev/skiplevel/blob/main/docs/schema-notes.md；https://github.com/gutnikov/claude-session-player/issues/2（issue 已由 PR #14 关闭，enhancement）
- 详情: `message` 对象为 `{"role": "user", "content": <string 或 ContentPart[] >}`。**content 双形态**：直接提示为字符串；工具结果载体为数组（几乎都是 `tool_result` 块）。判定规则：content 数组含任意 `type == "tool_result"` 块 → 工具结果载体；`isMeta=true` → 不可见元消息；字符串 content 含 `<local-command-stdout>` → 本地命令输出。可选字段 `toolUseResult`（工具结果摘要：字符串，或对象含 `stdout`/`stderr`/`interrupted`/`isImage`；WebFetch 结果另有 `bytes`/`code`/`codeText`/`result`/`durationMs`/`url`；TodoWrite 另有 `oldTodos`/`newTodos`——来源: ccrider schema.md）与 `thinkingMetadata`（`level`: high/medium/low、`disabled`、`triggers`）。压缩续接摘要以 `isCompactSummary: true` 的 user 事件出现，内容以 "This session is being continued from a previous conversation..." 开头（skiplevel 2026-06-11 观测）。中断标记原文：`[Request interrupted by user]` 与 `[Request interrupted by user for tool use]`。`tool_result` 块结构：`{"tool_use_id": "toolu_...", "type": "tool_result", "content": <string 或 {"text": "..."} 数组>, "is_error": false}`，多段 content 用 `\n` 连接；被拒绝的工具调用 `is_error: true`。

### 发现 5: assistant 条目结构（type: "assistant"）

- 来源: https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md；https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md
- 详情: 信封字段 + `requestId`，`message` 为完整 Anthropic API 消息对象：`model`（如 `claude-sonnet-4-5-20250929`；agentyard 实测 `claude-opus-4-7` 与 `<synthetic>`——后者是恢复会话时回填的占位符，非真实模型）、`id`（Anthropic API message ID，`msg_` 前缀）、`type`（恒为字面量 `"message"`）、`role`（`"assistant"`）、`content`（**必须是数组**——裸字符串会直接导致加载器崩溃，纯文本也要写成 `[{"type": "text", "text": "..."}]`；此约束来自 toolpath writing-compatible-jsonl.md 的经验实证）、`stop_reason`（`"end_turn"`/`"tool_use"`/`"stop_sequence"`/`"max_tokens"`/null——磁盘上经常为 null，因为条目在流式 API 响应完成前就持久化）、`stop_sequence`（string 或 null）、`usage`（仅 assistant 条目携带）。`isApiErrorMessage` 标志（可选）：API 错误消息标记，UI 特殊样式 + 主循环跳过 stop hooks + 触发错误恢复（源码文档）。

### 发现 6: usage token 字段结构

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md；https://github.com/repowise-dev/skiplevel/blob/main/docs/schema-notes.md；https://docs.rs/claude-wrapper/0.11.0/claude_wrapper/history/struct.SessionSummary.html
- 详情: `message.usage` 对象字段（snake_case，与 Anthropic API usage 一致，截至 2026-08-01）：

| 字段 | 含义 |
|------|------|
| `input_tokens` | 输入 token 数 |
| `output_tokens` | 输出 token 数 |
| `cache_creation_input_tokens` | 缓存创建输入 token |
| `cache_read_input_tokens` | 缓存读取输入 token |
| `service_tier` | 如 `"standard"` |
| `cache_creation` | 对象，含 `ephemeral_5m_input_tokens`、`ephemeral_1h_input_tokens` |
| `total_cost_usd` | 单条成本——claude-wrapper 0.11.0 实测**当前版本恒为 null**（字段存在但写 null），成本核算需自行按 token 计算 |

- 解析注意: **token/成本核算必须按 `message.id` 去重**——skiplevel（2026-06-11，~2.1.x 实测）发现一条 API 消息被拆成多行 JSONL（每 content block 一行），每行重复相同 `message.id` 与逐字节相同的 usage；实测 7166 条 assistant 行折叠为 2708 个唯一 id（重复行 usage 比对 4458 same / 0 different），不去重会高估约 2.6 倍。
- 另一视角（项目内交叉印证）: 本项目 `src-tauri/src/hooks/usage.rs`（见 @../src-tauri/src/hooks/CLAUDE.md）已按 `message.usage.input_tokens` 等字段解析 transcript，与本文档字段结构一致——说明该 schema 已被项目现有代码实测验证。

### 发现 7: system 条目结构（type: "system"）

- 来源: https://github.com/wuwangzhang1216/claude-code-source-all-in-one/blob/main/claude-code-deep-analysis/08-message-types.en.md（源码分析）；https://github.com/pedropaulovc/claude-code-types；https://github.com/gutnikov/claude-session-player/issues/2
- 详情: 字段为 `subtype`、`content`（常为 XML 格式，如 `<command-name>/resume</command-name>`）、`level`（`"info"`/`"warning"`/`"error"`，源码中 `type Severity = 'info' | 'warning' | 'error'`）、`isMeta`。源码文档列出 14 个 subtype（称"more than 15 subtypes"）：`compact_boundary`、`microcompact_boundary`、`api_error`、`api_metrics`、`turn_duration`、`informational`、`away_summary`、`scheduled_task_fire`、`bridge_status`、`local_command`、`memory_saved`、`stop_hook_summary`、`agents_killed`、`permission_retry`。`compact_boundary` 标记压缩发生位置（供 `getMessagesAfterCompactBoundary` 查询），旧源码文档另记录其含 `summary` 字段；`turn_duration` 含 `durationMs`/`messageCount`（skiplevel）。

### 发现 8: content block 类型与结构

- 来源: https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md；https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md
- 详情: 磁盘上常见的 content block 类型：

| type | 结构 | 说明 |
|------|------|------|
| `text` | `{"type": "text", "text": "..."}` | 纯文本 |
| `thinking` | `{"type": "thinking", "thinking": "...", "signature": "..."}` | signature 为 base64 约 450 字符的加密证明；**无有效签名在恢复会话时会被丢弃**，构造有效签名"不可能不借助 Anthropic 签名源"；空字符串 thinking 配有效签名合法（被编辑内容） |
| `tool_use` | `{"type": "tool_use", "id": "toolu_...", "name": "Bash", "input": {...}, "caller"?: {...}}` | 内置工具名称 PascalCase（`Read`/`Bash`/`Grep`），MCP 工具命名空间化 `mcp__<server>__<tool>`；`caller` 可选（如 `{"type": "direct"}`） |
| `tool_result` | `{"type": "tool_result", "tool_use_id": "toolu_...", "content": <string 或数组>, "is_error": false}` | 由**下一条 user 条目**携带（API 要求工具结果在 user 角色消息中）；`tool_use_id` 与发起方 `tool_use.id` 配对 |

- 其他 API 级类型（磁盘上少见，宽容解析器应保留未知类型而非报错）：`image`、`document`、`redacted_thinking`、`server_tool_use`、`web_search_tool_result`。

### 发现 9: 消息线程结构（parentUuid 树 + leafUuid）

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md；https://raw.githubusercontent.com/nibzard/claude-threads/refs/heads/main/SPECS.md
- 详情: 消息经 `parentUuid` → `uuid` 链接形成树（主线程 + `isSidechain: true` 分支 + 代理子线程）。重建顺序：从 `parentUuid: null` 的根出发，沿 `uuid → parentUuid` 链接；首行 `summary` 的 `leafUuid` 指向最后一条消息。`/resume` 续接时消息**追加到同一 .jsonl 文件**，`sessionId` 不变。`cwd` 字段会话内可变，可按命令重建工作目录上下文。claude-session-player 的分类规范（claude-code-session-protocol-schema.md 第 3 节 "Common Envelope Fields"、第 4 节 "Message Types Reference"）确认此树模型。

### 发现 10: Claude Code 源码内部消息类型（7 种）

- 来源: https://github.com/wuwangzhang1216/claude-code-source-all-in-one/blob/main/claude-code-deep-analysis/08-message-types.en.md
- 详情: 源码级 `type Message = AssistantMessage | UserMessage | SystemMessage | ProgressMessage | AttachmentMessage | ToolUseSummaryMessage | TombstoneMessage`。关键类型字段（源码原文）：`AssistantMessage = { type: 'assistant', uuid: string, message: { content: ContentBlock[], usage: Usage }, isApiErrorMessage?: boolean }`；`TombstoneMessage = { type: 'tombstone', targetUUID: string }`（流式错误恢复的"追溯性撤回"标记——模型流式输出后出错/降级时用 targetUUID 定位要撤回的消息）；`ProgressMessage = { type: 'progress', toolUseId: string, content: string }`；`AttachmentMessage = { type: 'attachment', attachment: { type: 'memory'|'skill'|'file'|'context', content: string } }`。`normalizeMessagesForAPI`（`utils/messages.ts`，全代码库最大工具文件 5512 行）经 `.filter(isAPIRelevant)`（滤除 Progress/Tombstone 等）→ `.map(toAPIFormat)` → `.reduce(mergeConsecutive)` 将内部类型归一化为 API 消息。一个 AssistantMessage 可同时含 thinking/text/tool_use 三种 block，顺序为"先思考、后回复、再调工具"。注意：此文档为第三方源码逆向（非 Anthropic 官方），且 `tombstone` 类型未在任何磁盘观测来源中出现——以磁盘观测（发现 3）为准。

### 发现 11: 官方写入机制与 resume 语义

- 来源: https://mintlify.wiki/sanbuphy/claude-code-source-code/architecture/session-persistence（Claude Code 源码提取，早期版本）
- 详情: 写入由 `recordTranscript()`（`src/utils/sessionStorage.ts`）处理，append-only JSONL。**写入策略差异**：user 消息 `await` 阻塞写（崩溃恢复保障——最后一条 user 消息必然落盘）；assistant 消息 fire-and-forget 顺序保持队列（不阻塞 agent 循环）；progress 内联写、下次查询去重；"on result yield / cowork eager flush" 冲刷。resume 机制：`getLastSessionLog()` 扫描 `~/.claude/projects/<hash>/sessions/` 返回最近修改的 .jsonl（或 `--resume <id>` 匹配）；逐行解析为类型化条目，截断/损坏行跳过；`compact_boundary` 存在时从边界重建 messages[]，压缩摘要作为一条 assistant 消息前置。CLI: `--continue`（当前 cwd 最近会话）、`--resume <id>`、`--fork-session`（新 sessionId + 复制历史）；斜杠命令 `/resume` 列出最近会话（开始时间/消息数/最后消息预览）。崩溃恢复后工具重执行无幂等跟踪（源码原文 caveat）。

### 发现 12: 关键解析坑（实测经验）

- 来源: https://github.com/repowise-dev/skiplevel/blob/main/docs/schema-notes.md（2026-06-11，~2.1.x）；https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md
- 详情: ① 一条 API 消息拆多行、按 `message.id` 去重（见发现 6，~2.6x 高估）；② 文件从几 KB 到 20+ MB，必须流式读取；③ 读活动文件时容忍 EOF 处一条截断 JSON 行；④ 字段可缺失、形状会漂移（"fields can be missing, shapes can drift, and the parser must never crash"——skiplevel 解析器设计原则）；⑤ 未知类型必须忽略而非报错（agentyard 实证 schema 增长：2.5MB 样本含 `permission-mode` 与 `queue-operation`，148K 样本没有）；⑥ `ai-title`/`last-prompt` 是覆写式写入（last wins），列表页只扫文件名、内容惰性加载；⑦ 子代理是独立文件（`agent-<id>.jsonl`），勿从父 transcript 提取；⑧ `isSidechain=true` 实测零记录，不能作为子代理识别依据；⑨ 时间戳 `Z` 后缀——Python 3.10 `fromisoformat` 无法解析需先归一化；⑩ 元/非人类内容以 `<` 开头（`<local-command-caveat>`/`<command-name>`/`<system-reminder>`）或 `isMeta: true`。

### 发现 13: 写出兼容 JSONL 的规则（供未来写回/注入场景参考）

- 来源: https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/writing-compatible-jsonl.md
- 详情: 若要让 `claude --resume <uuid>` 或会话选择器加载自写文件，经验实证约束：文件必须在 `~/.claude/projects/<sanitized-cwd>/<uuidv4>.jsonl`；`cwd` 须为规范路径（macOS `/tmp` → `/private/tmp`）；assistant `message.content` 必须数组（裸字符串崩溃加载器），user 两种形态均可；`permission-mode` 条目恰好三字段（`type`/`permissionMode`/`sessionId`，多出 `uuid`/`timestamp` 等即被拒）；每个 `tool_use` 必须有同 `tool_use_id` 的 `tool_result` 跟进（`stop_reason: "tool_use"` 无结果会"confuse the loader"）；thinking 块须有效 signature（无法自行构造）；信封字段惯例 `userType: "external"`、`entrypoint: "cli"`、`gitBranch` 空串非 null、`isSidechain` 显式 false；assistant 通常有非 null `model`/`id`（`msg_` 前缀）/`type: "message"`；`stop_reason` 安全值 `"end_turn"`/`"tool_use"`/null。**最小可用会话**：目录存在 + `<uuid>.jsonl` 存在 + 首行合法三字段 `permission-mode` + 至少一条完整 user 条目。**可继续会话**：再加至少一个带干净 `stop_reason` 的 assistant 回合。往返保留规则：保留未知字段（新字段增量出现）、保留 content 数组顺序（text 先于 tool_use、thinking 先于 text 影响回放语义）。

### 发现 14: 格式版本演进与稳定性

- 来源: https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md；https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md；https://mintlify.wiki/sanbuphy/claude-code-source-code/architecture/session-persistence；https://raw.githubusercontent.com/nibzard/claude-threads/refs/heads/main/SPECS.md
- 详情: **格式无显式版本号**——"Format version is not explicitly stored"，`version` 字段记录的是 Claude Code 应用版本，可作格式演进代理。观测到的演进证据：① 布局：早期 `~/.claude/projects/<hash>/sessions/<session-id>.jsonl`（短 sessionId `<prefix><8 字符>`，~1.0.x 时代）→ 当前 `~/.claude/projects/<encoded-cwd>/<uuidv4>.jsonl`；② 类型增长：claude-threads SPECS（`version` 1.0.63）仅见 `user|assistant` → 2.0.29（ccrider）见 summary/user/assistant/system/file-history-snapshot 五种 → 2.1.x（skiplevel/agentyard）新增 permission-mode/progress/queue-operation/attachment/ai-title/last-prompt/mode/saved_hook_context 等；③ agentyard（2.1.145）结论 "Assume schema growth"——解析器必须容忍未知类型；④ 多行拆分怪癖在 2.1.x 存在（skiplevel 2026-06-11 实测）。稳定核心：`type` 判别 + `parentUuid` 树 + `message` 嵌套结构自早期版本至今不变。

### 发现 15: 保留期与删除机制（历史查询的时效性约束）

- 来源: https://dev.classmethod.jp/en/articles/claude-code-conversation-history-retention/（2026-07-04）
- 详情: `cleanupPeriodDays` 配置项（settings.json）默认 30 天，接受 ≥1 的整数；**在 Claude Code 启动时**自动删除超过 N 天的会话文件（作者实测环境中最老存留文件恰为 30 天）。已删除的历史不可恢复——加大配置不能找回。transcript 以**未加密明文**存储，工具读取的凭据（如 `.env` 内容）会原样写入。禁用会话持久化：环境变量 `CLAUDE_CODE_SKIP_PROMPT_HISTORY` 或非交互 `-p` 模式加 `--no-session-persistence`。**对"查询历史 session 并恢复"功能的意义**：解析历史前须确认文件未被清理策略删除；明文特性也意味着历史查询功能会在本地展示敏感内容。

### 发现 16: 社区解析工具生态（可作为解析器实现参考）

- 来源: https://github.com/simonw/claude-code-transcripts（Simon Willison，2025-12-25 前后发布，据其博客 "A new way to extract detailed transcripts from Claude Code"）；https://github.com/utensils/claudex（v0.13.0）；https://github.com/FredyRivera-dev/claude_converter；https://docs.rs/claude-wrapper/0.11.0/claude_wrapper/history/struct.SessionSummary.html；https://github.com/gutnikov/claude-session-player/issues/2
- 详情: ① **simonw/claude-code-transcripts**（Python）：最广泛使用的 HTML 导出工具，命令 `local`/`web`/`json`/`all`，直接读 `~/.claude/projects` 下 JSONL（`web` 命令因 Claude 非官方 API 变动当前损坏）；② **utensils/claudex**（Rust CLI+库）：读六种代理 transcript（Claude Code `~/.claude/projects/` 等）索引到 SQLite（`~/.claudex/index.db`，FTS5 全文搜索），命令含 summary/cost/search/sessions——其解析器是字段级格式的独立实现；③ **FredyRivera-dev/claude_converter**（Python，零依赖）：`load_session(path)` 返回原始 record 列表，`session_to_messages(path)` 平铺为 HF messages 格式；块转换规则：text 原样、thinking → `<thinking>...</thinking>`、tool_use → `<tool_use name='...'>{input}</tool_use>`、tool_result → `<tool_result>...</tool_result>`、system 记录跳过、空串跳过；④ **claude-wrapper**（Rust）：`SessionSummary` 字段 `session_id`/`project_slug`/`message_count`/`first_timestamp`/`last_timestamp`/`title`/`first_user_preview`/`total_cost_usd`/`total_tokens`/`size_bytes`；message_count 只计 user+assistant 条目，标题来自 `ai-title` 条目，token 从 assistant 条目 `message.usage` 汇总——可作会话列表 UI 的数据模型参考；⑤ **claude-session-player**（Python）：逐行分类渲染，LineType 16 变体（分类规则见发现 4/7），防御式解析（跳过空行与 JSON 失败行）。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://raw.githubusercontent.com/neilberkman/ccrider/main/research/schema.md | 社区逆向 schema | 最完整的字段级 schema（2.0.29 实测）：信封字段、type 枚举、usage 字段、toolUseResult/thinkingMetadata、存储路径 |
| https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/messages.md | 社区文档 | message 对象结构、content 双形态规则、content block 类型、thinking signature 要求 |
| https://github.com/empathic/toolpath/blob/main/docs/agents/formats/claude-code/writing-compatible-jsonl.md | 社区文档 | 写回兼容规则：UUIDv4/规范 cwd/assistant content 必须数组/permission-mode 三字段/最小可用会话 |
| https://github.com/simonw/claude-code-transcripts | 社区工具 | Simon Willison 的 HTML 导出工具，路径 `~/.claude/projects`，命令 local/web/json/all（web 当前损坏） |
| https://github.com/pedropaulovc/claude-code-types | 社区类型定义 | TypeScript 接口 9 种 type（UserEntry/AssistantEntry/SystemEntry/FileHistorySnapshotEntry/PrLinkEntry/ProgressEntry/QueueOperationEntry/SavedHookContextEntry/SummaryEntry） |
| https://github.com/repowise-dev/skiplevel/blob/main/docs/schema-notes.md | 社区逆向笔记 | 2.1.x（2026-06-11）实测：一行 API 消息拆多行需按 message.id 去重（2.6x）、中断标记原文、isCompactSummary、usage 四字段 |
| https://raw.githubusercontent.com/nibzard/claude-threads/refs/heads/main/SPECS.md | 社区规格 | 早期版本（1.0.63）观测：type 仅 user\|assistant、message {role, content}、tool_result 结构 |
| https://github.com/aresbit/claude-save/blob/main/CLAUDE_JSONL_SCHEMA_ANALYSIS.md | 社区分析（推测性） | 仅 user/assistant 处理为实测；tool_call/file_operation 等类型为未验证假设，不采用 |
| https://mintlify.wiki/sanbuphy/claude-code-source-code/architecture/session-persistence | 源码提取（早期版） | recordTranscript 写入策略（user 阻塞/assistant 队列）、resume 机制、compact_boundary 重建、旧布局 <hash>/sessions/、sessionId 短格式 |
| https://github.com/wuwangzhang1216/claude-code-source-all-in-one/blob/main/claude-code-deep-analysis/08-message-types.en.md | 源码逆向分析 | 7 种内部消息类型、TombstoneMessage/ProgressMessage/AttachmentMessage、14 个 system subtype、normalizeMessagesForAPI |
| https://dev.classmethod.jp/en/articles/claude-code-conversation-history-retention/ | 技术博客 | cleanupPeriodDays 默认 30 天启动时删除、明文存储、CLAUDE_CODE_SKIP_PROMPT_HISTORY、路径命名（2026-07-04） |
| https://raw.githubusercontent.com/wannabefro/agentyard/4912a42d31c74dd1efbf98d4cb327fc77f623b93/docs/research/claude-code.md | 社区研究 | 2.1.145 + 252 项目实测：ai-title/last-prompt last wins、isSidechain 零记录、目录编码有损、子代理 meta.json、schema 增长 |
| https://github.com/FredyRivera-dev/claude_converter | 社区工具 | 零依赖 Python 转换器：load_session/session_to_messages、块→HF 格式转换规则 |
| https://github.com/utensils/claudex | 社区工具 | Rust 解析六代理 transcript → SQLite FTS5 索引（v0.13.0） |
| https://docs.rs/claude-wrapper/0.11.0/claude_wrapper/history/struct.SessionSummary.html | 社区库文档 | SessionSummary 字段、message_count 只计 user+assistant、total_cost_usd 当前恒 null、title 来自 ai-title 条目 |
| https://github.com/gutnikov/claude-session-player/issues/2 | 社区 issue（已关闭） | LineType 16 变体分类规则（user/assistant/system/progress 子类型）、防御式解析；PR #14 关闭，状态 closed/enhancement |
