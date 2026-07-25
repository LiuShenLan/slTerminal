# Claude Code Hooks 文档验证 — 冲突裁决与汇总

> 裁决日期: 2026-07-25
> 验证范围: 17 个源文件 → 17 个 review 文件 (2074 行)
> 5 个 subagent + 交叉引用 agent 并行验证

---

## 一、跨方向矛盾裁决

### 冲突 1: 配置层级优先级顺序

| 文档 | 声称 |
|------|------|
| D1 §7.2 | managed > **local > project > user** |
| D2 main §1.2 | managed > **user > project > local** |
| D2 detail §1 | "项目级优先于用户级" (project > user) |

**裁决**: D1 正确。官方文档 (code.claude.com/docs/en/settings) 明确: `settings.local.json` 优先级最高（覆盖 project 和 user），project 次之，user 最低。D2 main 和 D2 community 的排序是**错误**的。

### 冲突 2: CLAUDE_SESSION_ID 环境变量

| 文档 | 声称 |
|------|------|
| D1 §7.3 | `CLAUDE_SESSION_ID` 是 hook 环境变量 |
| D2 main §6.1 | `CLAUDE_SESSION_ID` 是 hook 环境变量 |
| D4 §2.1 | `CLAUDE_SESSION_ID` 是 hook 环境变量 |
| D2 review | 正确名称是 `CLAUDE_CODE_SESSION_ID` |
| D4 review | 该变量**不作为环境变量存在**，仅在 stdin JSON 中 |

**裁决**: D4 review 正确——`CLAUDE_SESSION_ID` 不是 hook 子进程的原生环境变量。官方 hook 文档列出的环境变量仅含 `CLAUDE_PROJECT_DIR`、`CLAUDE_PLUGIN_ROOT`、`CLAUDE_ENV_FILE`、`CLAUDE_CODE_REMOTE`。`session_id` 仅存在于部分事件的 stdin JSON 中。D1/D2/D4 均错误。

### 冲突 3: Hook 事件总数

| 文档 | 声称 |
|------|------|
| D1/D2 多处 | "30 个" |
| D2 review | TypeScript SDK `HookEvent` 类型仅 21 个，CLI 文档约 25-27 个 |
| D5 | "12+" |

**裁决**: "30" 来自社区总结 (morphllm.com)，非官方数字。官方 TypeScript SDK 的 `HookEvent` 联合类型含 21 个事件。加上 SDK 不支持但 CLI 存在的事件 (StopFailure, PermissionDenied, CwdChanged, FileChanged, InstructionsLoaded, Elicitation, ElicitationResult)，实际约 27-28 个。建议统一为"27+ 个事件"并注明来源。

### 冲突 4: Notification matcher 值数量

| 文档 | 声称 | 数量 |
|------|------|------|
| D1 早期版本 | 4 个 | 4 |
| D1 更新后 / D2 | 6 个 | 6 |
| D2 review / D1 review | 8 个 | 8 (含 agent_needs_input, agent_completed) |

**裁决**: 8 个正确。`agent_needs_input` 和 `agent_completed` 在 v2.1.198+ 加入。D1/D2 均遗漏。

### 冲突 5: PreCompact 是否可阻止

| 文档 | 声称 |
|------|------|
| D1, D2 main | 是 (v2.1.105+) |
| D2 schema | 否 |
| D2 community | 否 |

**裁决**: 是，PreCompact 可阻止。D2 schema 和 D2 community 错误。

### 冲突 6: "stdin/stdout JSON 协议是 6 工具事实标准"

| 文档 | 声称 |
|------|------|
| D5 汇总 | 6 工具均使用相同 stdin JSON → stdout JSON 协议 |
| D5 review | Copilot CLI 和 Codex CLI 使用 JSON-RPC over stdio，非原始 stdin→stdout |

**裁决**: D5 review 正确。Cursor/Windsurf/Gemini CLI/Claude Code 确实使用原始 stdin→stdout JSON。但 Copilot CLI (JSON-RPC over stdio with `joinSession()`) 和 Codex CLI (bidirectional JSON-RPC with JSONL) 协议层不兼容。D5 的"相同传输契约"表述过度概括。

---

## 二、各方向错误统计

| 方向 | 源文件 | Review 文件 | 错误数 | 严重 | 中等 | 轻微 |
|------|--------|------------|--------|------|------|------|
| D1 | 5 | 5 | 35 | 4 | 13 | 18 |
| D2 | 6 | 6 | 29 | 5 | 12 | 12 |
| D3 | 1 | 1 | 8 | 3 | 3 | 2 |
| D4 | 1 | 1 | 25 | 8 | 10 | 7 |
| D5 | 5 | 5 | 41 | 6 | 21 | 14 |
| **总计** | **18** | **18** | **138** | **26** | **59** | **53** |

---

## 三、最高优先级错误 (影响架构决策)

### P0-1: CLAUDE_SESSION_ID 不存在 (D1/D2/D4 均错)
三个方向的环境变量表都错误包含了 `CLAUDE_SESSION_ID`。slTerminal 若基于此设计 hook 集成，运行时读取到空值。

### P0-2: WezTerm Shell Integration 注入机制虚构 (D4)
D4 声称的三个环境变量 (`ZDOTDIR`/`BASH_ENV`/`XDG_CONFIG_HOME`) 在 WezTerm 官方文档中**零出现**。此为 D4 检索 agent 自行推测，非来源于文档。

### P0-3: Warp 虚构术语 (D4 + D5)
`SourcedRcFileForWarp`、`OSC 777` 在引用来源中均不存在。D4 和 D5 应删除或标注为推测。

### P0-4: 配置层级顺序 — D2 完全相反 (D2)
D2 main 和 D2 community 将 user 放在 project/local 之上，与官方文档相反。直接影响 slTerminal 的 settings UI 设计。

### P0-5: 7 个 Windows Bug 中 6 个已关闭 (D4)
D4 用现在时描述的 7 个 Windows 问题中，6 个已关闭修复。slTerminal 风险评估被显著高估。

### P0-6: `--safe-mode` 标志不存在 (D3)
全局搜索零结果。最接近的替代方案是 `--bare` 或 `"disableAllHooks": true`。

### P0-7: Session Transcript 格式严重错误 (D3)
声称的"5 种类型 + summary 首条"在 v2.1.216 实测中不成立。实际有 7+ 种类型，首条是 `custom-title`，`summary` 类型不存在。

### P0-8: 3 个 VS Code Marketplace 扩展不存在 (D2)
cited 的 Tingly-Dev.tingly-debug、augustocdias.tasks-shell-input、afterschool.depot 均返回 404。D2 的 VS Code UI 参考章节引用了不存在的扩展。

---

## 四、内部矛盾汇总 (D2 自身最严重)

D2 内部有 5 处自身矛盾:
1. PreCompact 可阻止性: D2-main 说是，D2-schema/community 说否
2. CLAUDE_ENV_FILE 范围: D2-main 说 4 事件，D2-detail 说 1 事件
3. 事件数量: D2-main 说 30，D2-schema 仅列 20
4. 5 个事件的 matcher 支持: D2-schema 说"不支持"，其他 D2 文件说"支持"
5. 配置层级: D2-main 与 D2-detail 矛盾

D2-schema (02-settings-json-schema.md) 质量最差——单独贡献了 3 处内部矛盾 + 遗漏 10 个事件。

---

## 五、已验证准确的核心声称

以下跨方向声称经 5 个 subagent 一致确认为正确:
- exit code 0/1/2 语义 (0=allow, 2=block, 1=warning)
- 5 种 handler 类型 (command/http/mcp_tool/prompt/agent)
- JSON Schema 存在于 SchemaStore (`json.schemastore.org/claude-code-settings.json`)
- $schema 字段被修复 (2025-09)
- Hook 无控制 TTY (设计决策)
- CLAUDE_ENV_FILE 是环境持久化的唯一通道
- PostToolUse 三态可见性 (#11224)
- 50K (changelog) vs 10K (代码) 阈值偏差
- hooks.log 48GB 问题 (#16047)
- 所有 15+ 个 GitHub Issues 真实存在
- OSC 9;4 4 状态定义和终端支持矩阵
- claude-hud 使用 statusLine API (非 hooks)
- Cursor/Windsurf/Gemini CLI 使用 stdin/stdout JSON
- aider 无正式 hook 系统
