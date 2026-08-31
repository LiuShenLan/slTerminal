# Stage 02 逐项验证断言（唯一真值源）

> stage-02-frontend-infra 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：`src/types/backgroundTasks.ts` 存在；含 `BackgroundTaskInfo` 六字段（taskId/title/enabled/intervalSec/intervalMin/intervalMax）；含 `BACKGROUND_TASK_IDS = ["planBalance", "sessionRefresh"]`；含 `PLAN_BALANCE_TASK_ID` 与 `SESSION_REFRESH_TASK_ID` 常量导出。
- **FE-02**：`src/ipc/backgroundTasks.ts` 存在，含 `listBackgroundTasks` / `setBackgroundTaskConfig` / `onBackgroundTasksUpdated` 三导出；`setBackgroundTaskConfig` 实现中 undefined 键不入 payload（Read 确认有条件构造 args）；invoke 命令名为 `background_tasks_list` / `background_tasks_set_config`（逐字）。
- **FE-02**：`src/ipc/index.ts` 含 `export * as backgroundTasks from "./backgroundTasks";`；`src/ipc/planBalance.ts` 中 grep `setPlanBalanceInterval` 零命中（`getPlanBalance`/`refreshPlanBalance`/`onPlanBalanceUpdated` 保留）。`setPlanBalanceInterval` 在 src/ 的残留命中仅允许出现于 `src/panels/settings/pages/PlanBalancePage.tsx` 与 `src/__tests__/settings-plan-balance.test.tsx`——两文件是 Stage 04（FE-07）git rm 对象，本 Stage 不判 not_fixed（跨 Stage 中间态，2026-08-31 登记）。
- **FE-02**：`src/__tests__/ipc-background-tasks-contract.test.ts` 存在——含两命令四维段、onBackgroundTasksUpdated 手写模拟驱动、`BackgroundTaskInfo` 六键精确匹配、`BACKGROUND_TASK_IDS` 值集断言；`src/__tests__/ipc-plan-balance-contract.test.ts` 中 grep `setPlanBalanceInterval` 零命中。
- **FE-02**：`src/__tests__/setup.ts` 含 `../ipc/backgroundTasks` 全局 mock（listBackgroundTasks 默认两任务 + setBackgroundTaskConfig + onBackgroundTasksUpdated no-op）。
- **FE-03**：`src/features/backgroundTasks/` 下 `types.ts`/`scheduler.ts`/`sessionRefreshTask.ts`/`tasks.ts`/`index.ts` 五文件齐备；`tasks.ts` 内容 = side-effect import `./sessionRefreshTask`；`index.ts` 不 import tasks（Read 确认 barrel 不触发注册）。
- **FE-03**：scheduler.ts 语义式断言（Read 确认，不限标识符细节）：首个订阅者触发配置读取 + 立即一轮 + interval；最后退订停 interval；runOnce 有 running 闸门（tick/manual 共用）；tick 失败不改快照、manual 失败置 error；applyConfig 支持启停与改频率；applyLocal 存在。
- **FE-03**：sessionRefreshTask.ts 语义式断言：遍历 `cliProfileRegistry.getAll()` 且按 `capabilities.history` 过滤；`scanAgentHistory` 调用第二参恒 `true`；部分失败保留该 provider 旧数据（prev 按 cliId 过滤）；全部失败 throw。
- **FE-03**：`src/__tests__/background-tasks-scheduler.test.ts` 与 `background-tasks-session-refresh.test.ts` 存在且 npm test 通过（结果以全量测试 agent 输出为准）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. （环境豁免，2026-08-31 登记）命令 5 预期 exit 127：本机 rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，见 .claude/test-inventory.md 豁免表）；测试类断言以「测试存在性 grep + `cargo check --tests` 编译级 + clippy」为兜底。
7. （跨 Stage 中间态，2026-08-31 登记）命令 1 tsc 预期失败：`src/panels/settings/pages/PlanBalancePage.tsx` 引用已移除的 `setPlanBalanceInterval`（FE-02 删导出），该文件是 Stage 04（FE-07）git rm 对象——tsc 在 Stage 04 删除后收敛；本 Stage 判定以「除 PlanBalancePage.tsx 及其级联错误（TS7006）外零错误」为准，不判 not_fixed。
