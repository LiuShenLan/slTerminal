# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-25 删除**：`src/features/agentHistory/HistorySessionList.tsx` 与 `HistorySessionRow.tsx` 不存在（Glob 确认）；`agentHistory/index.ts` 无 HistorySessionList/HistorySessionRow 导出（含类型，grep 零命中）
- **FE-25 引用清零**：`grep -r "HistorySessionList\|HistorySessionRow" src/` 的剩余命中全部为「原 HistorySessionList（已删）」类历史注记注释（须 Read 逐处确认，不存在 import/类型引用/JSX 使用等活引用）
- **FE-25 测试迁移**：`src/__tests__/agent-history-row.test.tsx` 已删除或改写为 NavHistoryRow 面向（Read 确认）；迁移保留的独立语义用例在全量测试中通过
- **FE-25 孤儿 helper**：执行 agent 报告中被删 helper 均附「零引用」grep 证据；有引用者保留未动（抽查 groupByCwd：若仍存在，确认有 List 以外消费方引用）
- **FE-25 注释**：panelId.ts / NavTree.tsx / useNavTree.ts / NavHistoryRow.tsx / NavContextMenu.tsx / historyContextMenu.ts 中「照 HistorySessionList」式注记已改写（grep 「照 HistorySessionList」 零命中）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
