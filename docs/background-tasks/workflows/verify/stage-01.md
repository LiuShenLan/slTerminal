# Stage 01 逐项验证断言（唯一真值源）

> stage-01-backend 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-01**：`src-tauri/src/background_tasks/registry.rs` 存在；含 `pub static TASKS` 且条目数 = 2（task_id 为 `"planBalance"`、`"sessionRefresh"`）；`RUNTIMES` 数组长度 2 且有等长守卫测试；`resolve_task_config` 与 `find` 存在；`SETTINGS_KEY: &str = "backgroundTasks"` 定义于本文件（pub(crate)）。
- **BE-01**：registry.rs 内含测试 `tasks_registry_key_set_locked`、`resolve_task_config` 系列（≥6 例）——grep `fn .*resolve_task_config` 计数 ≥6。
- **BE-02**：`src-tauri/src/background_tasks/mod.rs` 存在；含 `BackgroundTaskInfo`（serde camelCase 六字段：task_id/title/enabled/interval_sec/interval_min/interval_max）；含 `start_background_tasks`、`spawn_poller`、`set_config_core`、`background_tasks_list`、`background_tasks_set_config` 五个符号；含 `CONFIG_WRITE_LOCK`；`background_tasks_set_config` 内有 `emit("background-tasks-updated"`。
- **BE-02**：set_config_core 语义式断言——校验先于落盘先于内存更新（Read 确认顺序：find/Validation → save_settings_blocking → rt.enabled/interval_sec.store）；落盘读-改-写按 taskId 子键合并（Read 确认存在 `section.get(task_id)` 合并逻辑，非整体替换）。
- **BE-02**：mod.rs 内含测试 ≥9 例（serde 键集合 / list / set_config 合法双写 / 子键合并 / 越界拒绝 / 未知 taskId / 双 None 拒绝 / 磁盘内存一致）——grep `fn set_config\|fn list_\|serde_key_set` 计数核对。
- **BE-03**：`src-tauri/src/plan_balance/mod.rs` 中 grep `SETTINGS_KEY|resolve_poll_interval|start_plan_balance_poller|plan_balance_set_interval|POLL_INTERVAL_SEC|INTERVAL_SEC_KEY` 零命中；含 `pub fn poll_once_executor`；既有测试保留（merge_slot 4 例 + poll_once 5 例 + serde 4 例 + get_plan_balance 1 例——grep `fn merge_slot_|fn poll_once_|serde_key_set|fn get_plan_balance_empty_initial` 计数核对）。
- **BE-04**：`src-tauri/src/settings.rs` 白名单第 5 键为 `crate::background_tasks::SETTINGS_KEY`（grep 命中）；`crate::plan_balance::SETTINGS_KEY` 全仓零命中；含 `pub(crate) fn save_settings_blocking`；`save_settings` async 命令为 spawn_blocking 包装形态（Read 确认）；含测试 `save_accepts_background_tasks_key` 与 `save_rejects_plan_balance_key`。
- **BE-04**：`save_settings_blocking` 全仓恰好两处引用（settings.rs 内部 + background_tasks/mod.rs）——grep 计数 = 2（定义处除外）。
- **BE-05**：`src-tauri/src/lib.rs` 含 `mod background_tasks;`；setup 中调 `background_tasks::start_background_tasks`（且 `start_plan_balance_poller` 零命中）；generate_handler 含 `background_tasks::background_tasks_list` 与 `background_tasks::background_tasks_set_config`，不含 `plan_balance_set_interval`。
- **BE-05**：`src-tauri/build.rs` commands 含 `background_tasks_list`/`background_tasks_set_config`、不含 `plan_balance_set_interval`，注释含「37 条」；`src-tauri/capabilities/default.json` 含 `allow-background-tasks-list`/`allow-background-tasks-set-config`、不含 `allow-plan-balance-set-interval`。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. （环境豁免，2026-08-31 登记）命令 5 预期 exit 127：本机 rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，见 .claude/test-inventory.md 豁免表）；测试类断言以「测试存在性 grep + `cargo check --tests` 编译级 + clippy」为兜底。
