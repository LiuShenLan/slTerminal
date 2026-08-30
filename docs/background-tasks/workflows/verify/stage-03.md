# Stage 03 逐项验证断言（唯一真值源）

> stage-03-consumers 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-04**：`src/features/agentHistory/useAgentHistory.ts` 返回面不含 `scan`、含 `triggerNow`（Read 返回对象确认）；grep `scanAgentHistory` 在本文件零命中（扫描改由调度器执行体承担）；含 `backgroundTaskScheduler.subscribe`（SESSION_REFRESH_TASK_ID）与 `applyLocal`；genRef/useRef 已删（grep `useRef` 零命中本文件）。
- **FE-04**：`src/__tests__/agent-history-hook.test.tsx` 含 `backgroundTaskScheduler._reset()` 与 `cliProfileRegistry.register(claudeProfile)`；mock 面含 `../ipc/backgroundTasks`；npm test 通过。
- **FE-05**：`src/features/navTree/useNavTree.ts` 中 grep `history.scan` 零命中；refresh 回调内调 `history.triggerNow()`（Read 确认）；挂载即扫 useEffect 已删（grep `void history` 仅命中 triggerNow 一处）。
- **FE-05**：`src/__tests__/nav-tree.test.tsx` 与 `nav-tree-history.test.tsx` 均含 `../ipc/backgroundTasks` mock 与调度器/profile 双 `_reset` + `claudeProfile` 注册；scanAgentHistory 断言调用参数为 `("claude", true)`（force 恒 true）——grep 两文件中 `scanAgentHistory` 断言点逐一 Read 确认无 `(…, undefined)`/单参残留；npm test 通过。
- **FE-06**：`src/features/navTree/usePlanBalance.ts` 含 `listBackgroundTasks` 与 `onBackgroundTasksUpdated` 调用、`PLAN_BALANCE_TASK_ID` 引用、`enabled` state（`boolean | null`，初始 null）；返回面含 `enabled`（Read 确认）。
- **FE-06**：`src/features/navTree/PlanBalanceFooter.tsx` 渲染守卫为 `enabled !== true || items.length === 0`（语义式：禁用在任何快照下不渲染——Read 确认守卫位置在所有渲染之前）。
- **FE-06**：`src/__tests__/plan-balance-footer.test.tsx` 含 `../ipc/backgroundTasks` mock 与三个新用例（enabled=false 隐藏 / 事件推送隐藏+重显 / list 失败回退启用）；npm test 通过。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
