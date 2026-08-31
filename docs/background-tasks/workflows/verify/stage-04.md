# Stage 04 逐项验证断言（唯一真值源）

> stage-04-settings-page 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-07**：`src/features/settingsCenter/pages.ts` 含 `import BackgroundTasksPage from "../../panels/settings/pages/BackgroundTasksPage"` 与注册段（`id: "backgroundTasks"`、`title: "后台定时任务"`、`group: "global"`、`order: 20`）；grep `PlanBalancePage` 在 src/ 零命中。
- **FE-07**：`src/panels/settings/pages/BackgroundTasksPage.tsx` 存在；`src/panels/settings/pages/PlanBalancePage.tsx` 不存在（Glob 零命中）。新页含 data-e2e 五系列：`settings-background-tasks-page` / `-row-{taskId}` / `-enabled-{taskId}` / `-interval-{taskId}` / `-error-{taskId}`（grep 逐系列命中）。
- **FE-07**：BackgroundTasksPage 渲染语义式断言（Read 确认）——任务行由 `listBackgroundTasks()` 返回清单 map 渲染（通用 TaskRow 纯渲染，无按 taskId 硬编码的分支渲染）；勾选立即提交 `setBackgroundTaskConfig(taskId, { enabled })`；频率非法（非数/非整数/越界/空串）→ 行内红字 = 范围文案（`${intervalMin}–${intervalMax} 秒`，无默认值字样）不提交不 toast；提交成功收尾含 sessionRefresh `applyConfig` 与 planBalance `refreshPlanBalance()` 反馈闭环。
- **FE-07**：`src/__tests__/settings-background-tasks.test.tsx` 存在且 npm test 通过（用例覆盖：两行渲染 + 范围提示「10–3600 秒」「2–300 秒」/ list 失败空态 / 勾选提交 + 行更新 + refreshPlanBalance / applyConfig 调用 / 非法频率红字不提交 / 合法频率规范化回显 / set reject toast + 输入保留）；`src/__tests__/settings-plan-balance.test.tsx` 不存在（Glob 零命中）。
- **FE-07**：`src/__tests__/settings-pages-registration.test.ts` 断言数组含 `{ id: "backgroundTasks", group: "global", order: 20 }` 且 grep `planBalance`（页 id 上下文）零命中本文件；mock 路径改 `../panels/settings/pages/BackgroundTasksPage`。
- **FE-07**：grep `settings-plan-balance` 在 src/ 零命中（e2e-tests/ 由 Stage 05 处理，本断言限 src/）。
- **FE-08**（纯验证项，零代码改动）：Read `src/panels/settings/SettingsPanel.tsx` 确认深链失配回退逻辑仍在（saved 页 id 经 `registry.get(saved)` 校验，失配回退 global 组第一页）；既有用例「selectedPage 失效 → 回退全局组第一页」在全量 npm test 中通过。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. （环境豁免，2026-08-31 登记）命令 5 预期 exit 127：本机 rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，见 .claude/test-inventory.md 豁免表）；测试类断言以「测试存在性 grep + `cargo check --tests` 编译级 + clippy」为兜底。
