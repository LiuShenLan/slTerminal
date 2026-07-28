# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FIX-DOC-01**：`.claude/test-inventory.md` 中以下失实关键词零命中（grep 逐个验证）：`parse_context_usage`、`read_context_usage_file`、`total_tokens`、`exit_code`、`轮询`（agent-status-hook 行语境）。
- **FIX-DOC-01**（语义式）：test-inventory.md 中 mod.rs / usage.rs / ipc-contract / ipc-hooks-contract / notifications / agent-status-hook / agent-status-view / L4 各行覆盖描述与对应测试文件**当前实际内容**一致（须 Read 双方对照核实，防文档撒谎）；用例数与实跑统计一致（npm test / cargo test 尾行、L4 active=20）。
- **FIX-DOC-01**：changelog 段括注增量构成与本次 Stage 1-4 实际新增一致（Read 确认）。
- **FIX-DOC-02**：`src/ipc/CLAUDE.md` notification.ts 条目含 `sendClickableNotification`（grep 命中），且签名描述与 `src/ipc/notification.ts` 当前代码一致（Read 双方对照）；「thin wrapper」句不再将 notification 归为"直接 re-export 不添加额外逻辑"（Read 确认）。
- **FIX-DOC-02**：`src/lib/CLAUDE.md` 文件表含 `panelId.ts`（grep 命中）。
- **FIX-DOC-02**：`src/workspace/CLAUDE.md` 含 `pageApis`（grep 命中），且 __dockviewApi 三站点重指向描述与 `Workspace.tsx`/`pageApis.ts` 当前代码一致（Read 双方对照）。
- **FIX-DOC-02**：`src/panels/CLAUDE.md` TerminalRegistry 条目含 `subscribe`（grep 命中）。
- **FIX-DOC-02**：`src/features/sideViews/CLAUDE.md` useAgentStatus 条目含订阅增删/null 跳过/标题查找语义（Read 确认与 `useAgentStatus.ts` 当前代码一致）。
- **FIX-DOC-02**：`e2e-tests/CLAUDE.md` 含 settings.json 备份/还原描述（grep 命中）且与 `run-wdio.cjs` 当前实现一致（Read 双方对照）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
