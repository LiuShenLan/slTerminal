# review-07 文档一致性

> 维度：文档不撒谎——描述与代码终态对账。只写问题。

## 问题条目

### WD-1（P3）根需求编号索引漏登跨模块标识符 F9

- 位置：`.claude/CLAUDE.md:178-183`（对照 `src/features/agentStatus/CLAUDE.md:15` / `src/features/agentHistory/CLAUDE.md:17` / `src/panels/CLAUDE.md:227、230、252、270` / `.claude/test-inventory.md:115、153、292`）
- 问题：根文件「需求编号索引」表列出 F2–F8，但 F9（CLI 品牌 logo 机制）已在 agentStatus、agentHistory、panels/terminal、workspace、test-inventory 多处跨模块引用，却未在根表登记。
- 后果：开发者从根索引无法查到 F9 定义，只能依赖上下文推断，降低文档可追踪性。
- 修复建议：在 `.claude/CLAUDE.md`「需求编号索引」表追加 F9 行，例如：`| F9 | 特性 | 终端页签/侧栏 CLI 品牌 logo（按命令行首 token 匹配 profile.iconSrc）|`。
- 来源：独立发现

### WD-2（P3）hooks 配置 JSON 模式文档仍称有自动补全

- 位置：`src/panels/CLAUDE.md:152`、`src/features/hooksConfig/CLAUDE.md:21`、`src/features/hooksConfig/schema/index.ts:15`（对照 `src/panels/hooksConfig/JsonMode.tsx:6-8`、`src/panels/CLAUDE.md:302`）
- 问题：上述文档/注释称 hooks 配置 JSON 模式提供「补全/悬停/波浪线」，但 `JsonMode.tsx` 头部注释明确「无自动补全（Ctrl+Space）——验收后决策删除（2026-08-01）」，且 imports 仅含 `jsonSchemaHover` / `jsonSchemaLinter`，无 `jsonCompletion`。同一份 `src/panels/CLAUDE.md` 的文件表 line 302 已改为「无自动补全」，说明 line 152 与 hooksConfig 相关描述为残留旧说法。
- 后果：开发者按文档预期补全功能存在，实际缺失，导致功能理解与验收口径不一致。
- 修复建议：统一删除「补全/」字样，改为「悬停/波浪线」：`src/panels/CLAUDE.md:152`、`src/features/hooksConfig/CLAUDE.md:21`、`src/features/hooksConfig/schema/index.ts:15`。
- 来源：独立发现

### WD-3（P3）CONTEXT.md CLI profile 术语对后端注册表描述失实

- 位置：`CONTEXT.md:75-76`（对照 `src-tauri/src/hooks/provider.rs:28-44`、`src-tauri/src/agent_history/provider.rs:26-38`）
- 问题：术语表称 CLI profile「前后端各有 profile 注册表」，但后端并无统一的 CLI profile 注册表，只有按能力拆分的 `CliHooksProvider` 注册表（`hooks/provider.rs`）与 `CliHistoryProvider` 注册表（`agent_history/provider.rs`），二者均以 cliId 为键。
- 后果：可能误导开发者认为存在后端统一的 `CliProfileRegistry`，造成架构理解偏差。
- 修复建议：改为「前端为统一的 `CliProfileRegistry`；后端按能力拆分为 hooks/history 两个 cliId 键注册表（分别见 `hooks/provider.rs` 与 `agent_history/provider.rs`）」。
- 来源：独立发现

### WD-4（P3）AgentHistorySections 注释把复合键写成单 sessionId

- 位置：`src/features/agentHistory/AgentHistorySections.tsx:49`（对照 `src/features/agentHistory/historyModel.ts:118`、`src/features/agentHistory/HistorySessionList.tsx:222`、`src/features/agentHistory/CLAUDE.md:65、121`）
- 问题：`AgentHistorySectionsProps` 注释将 `activeStatuses` 描述为「Map<sessionId, status>」，但实现与模块文档均使用复合键 `cliId|sessionId`（MC-313）。
- 后果：消费方若按注释以单 sessionId 查表，无法命中运行中状态，导致历史区四态不同步。
- 修复建议：将注释改为「Map<cliId|sessionId, status>」。
- 来源：独立发现

## 已检查范围

1. **根 `.claude/CLAUDE.md`**：模块索引入口准确，需求编号索引存在 F9 漏登（WD-1）。
2. **`src/features/cliProfiles/CLAUDE.md`**：文件表 6 项与磁盘一致；`Workspace.tsx:30` import 一致；`matchByCommand` 语义与 `cliProfileRegistry.ts:36-45` 实现一致。
3. **`src/features/agentHistory/CLAUDE.md`**：文件表 9 项一致；四步编排与 `restoreSession.ts:70-128` 一致；操作矩阵与 `historyContextMenu.ts:62-86` 一致；已知限制与代码一致；发现 `AgentHistorySections.tsx` 注释失实（WD-4）。
4. **`src-tauri/src/hooks/CLAUDE.md`**：8 个 `#[cfg(test)]` 段共 147 用例已实查核对，9 文件表一致，命令签名与 `mod.rs` 一致；发现 JSON 模式补全描述失实（WD-2）。
5. **`src-tauri/src/agent_history/CLAUDE.md`**：6 文件表一致，70 用例已实查核对，SEC-05 与 `ops.rs:23-57` 一致。
6. **`src/features/agentStatus/CLAUDE.md` 与 `src/features/hooksConfig/CLAUDE.md`**：行建模/用量口径/schema 单点描述与代码一致；`hooksConfig/CLAUDE.md` 补全描述失实（WD-2）。
7. **`src/panels/CLAUDE.md` hooksConfig 段（:146-157）**：hub 选择行过滤/持久化/dirty 守卫/空态与 `HooksConfigPanel.tsx` 一致；line 152 补全描述失实（WD-2）。
8. **`CONTEXT.md`**：cliId/agent-event/agent_history/复合键等术语与代码一致；CLI profile 后端注册表描述失实（WD-3）。
9. **`.claude/test-inventory.md`**：multi-cli 相关条目与测试文件对账一致；L1 592 / L3 138 / L4 39 用例数已实查，`it(`/`#[test]` 口径吻合，L2 文件数 139 已核对。
10. **`e2e-tests/CLAUDE.md`**：helpers.ts 清单（含 `__slterm_e2e_registerMockCliProfile`）与 `e2e-tests/helpers.ts` 一致。
11. **代码内注释**：multi-cli 改动文件头部注释与行为一致；`AgentHistorySections.tsx` 行内注释失实（WD-4）。
