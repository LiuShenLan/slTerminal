# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 完成后 transcriptPath（camelCase）全仓零残留；transcript_path（snake_case）仅存三类豁免形态。

## 断言清单

- **KZ-2（更名零残留）**：全仓 grep `transcriptPath` 零残留（豁免：`docs/` 历史文档、迁移溯源注释——逐处 Read 确认属豁免形态，其余一律判 not_fixed）；grep `transcript_path` 残留仅存于三类（逐处 Read 确认）：① `e2e-tests/hooks.e2e.ts` 模拟 claude hook stdin 协议字段；② claude provider 内部语义（`scan_transcript_usage` 函数名/JSONL 解析，claude 合法领地）；③ reporter 的 `data.transcript_path` 读取
- **KZ-2（后端 DTO）**：`src-tauri/src/hooks/signal.rs` 字段为 `usage_source_path: Option<String>` 且带 serde default（Read 确认）；**不存在 serde alias**（决策 1——旧键信号降级 None）；serde 键集合测试含 `usageSourcePath` 键；存在「无 usageSourcePath 信号 → None」用例形态；L1 全绿
- **KZ-2（reporter）**：`src-tauri/src/hooks/claude/slterm-hook-reporter.js` payload 键为 `usageSourcePath`（值 = `data.transcript_path || null` 形态）；SCRIPT_VERSION = 3；`inject.rs` 模板内嵌校验断言新版本；C10 契约未削弱（Read 确认任何路径 exit(0)、不写 stderr）
- **KZ-2（前端内部状态）**：`src/panels/terminal/TerminalRegistry.ts` AgentSessionInfo 字段为 `usageSourcePath`（可选）；`useXterm.ts`/`useAgentStatus.ts`/`historyModel.ts`/`HistorySessionList.tsx` 消费方字段更名完成且 basename 回退逻辑语义不变（Read 对照——仅字段名变化，回退分支结构不动）
- **KZ-3（trait/命令/wrapper）**：`src-tauri/src/hooks/provider.rs` trait 参数为 `usage_source_path` 且文档注明「路径语义由具体 CLI 解释」（Read 确认）；`src-tauri/src/hooks/mod.rs` 命令参数 camelCase `usageSourcePath`；`src/ipc/agentHooks.ts` wrapper 为 `contextUsage(cliId, usageSourcePath)`；`src/__tests__/ipc-agent-hooks-contract.test.ts` 键集合精确断言 = `["cliId", "usageSourcePath"]`；L2 全绿
- **KZ-3（L1 测试名）**：`usage.rs` 测试名含 `usage_source_path`（原 `transcript_path` 测试名零残留）；`.claude/test-inventory.md` 对应行同步（frontend-consumers 单点负责口径）
- **E2E 随行**：`e2e-tests/agent.e2e.ts`、`hooks.e2e.ts`（:107/126 camelCase 键；:170 stdin snake_case 不动）、`history.e2e.ts` 信号构造键更名完成（grep 确认）；L4 全绿
- **文档同步**：`src-tauri/src/hooks/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src/types/CLAUDE.md`、`src/features/agentStatus/CLAUDE.md`、`src/features/agentHistory/CLAUDE.md`、`src/panels/CLAUDE.md`、`CONTEXT.md`（术语条目更名）七处与代码终态一致（Read 对照核实）；「版本过旧」重注入链路列入人工验证点，不强制自动化

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——其余命令全部完成后单独串行执行）
