# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FIX-TE-02**：`src/theme/index.ts` re-export 列表含 `AGENT_STATUS_USAGE_COLORS`（grep 命中）。
- **FIX-TE-02**：`colors.test.ts` 含 `AGENT_STATUS_USAGE_COLORS` describe——3 token（low/medium/high）合法 hex + 精确值 `#629755`/`#BBB529`/`#F44747`（grep 命中 + 用例绿）。
- **FIX-TE-02**：`agent-status-view.test.tsx` 用量条 describe 含分段颜色断言——percent <50 / 50-80 / >80 三档断言 `backgroundColor` 等于对应 token 值（grep `backgroundColor` 命中 + 用例绿）。
- **FIX-TE-03**：`test.e2e.ts` 中「Agent Status 视图出现行」相关用例为 active（该用例 `it.skip` 已移除，grep `it.skip` 于 Agent Status describe 内仅剩 toast 用例 3 一条）。
- **FIX-TE-03**：存在静态行用例（断言行出现 + 🟡 + 用量条容器）与动态四态用例（信号文件驱动 PreToolUse→⚡ / Stop→✅ / SessionEnd→行消失）两条 active 用例（Read 确认断言点）。
- **FIX-TE-03**：`test.e2e.ts` 中不存在「E2E 环境无 claude 进程，hook 事件不可用」失实注释（grep 零命中）。
- **FIX-TE-03**：wdio 实跑中上述两用例通过（以测试 agent 的 `npm run wdio` 输出为准）。
- **FIX-TE-04**：`run-wdio.cjs` 含 settings.json 备份/还原逻辑——启动时备份 `~/.slterminal/settings.json`，`process.on('exit'` 钩子里同步还原（grep 命中 + Read 确认还原覆盖原文件不存在/存在两分支）。
- **FIX-TE-04**：`test.e2e.ts` 侧栏两用例前置含 zones 重置（经 `__slterm_e2e_moveSideViewButton` 将 projects/explorer/commit/agent-status 归位 top），不再只重置 open（Read 确认）。
- **FIX-TE-04**：wdio 实跑 20 条 active 全绿（含原 2 条确定性失败用例）。
- **FIX-TE-04**：E2E 跑后真实 `~/.slterminal/settings.json` 与跑前一致（测试 agent 在 wdio 前后各取一次文件快照 diff，无差异；原文件不存在的情形跑后仍不存在）。
- **FIX-TE-05**：`diff-panel.test.tsx` 用例 12 的 `waitFor` 含 `[data-e2e="diff-left"] .cm-content` 存在断言（grep 命中）；npm test 全量绿（首跑即绿，无 flaky 重跑）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `npm run build:e2e`
6. `npm run wdio`（测试 agent 在运行本命令前后对 `~/.slterminal/settings.json` 取快照并 diff，结果写入报告）
