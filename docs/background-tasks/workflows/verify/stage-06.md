# Stage 06 逐项验证断言（唯一真值源）

> stage-06-docs 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；文档口径必须对照 Stage 01-05 完成后的真实代码核实（防文档撒谎）——发现文档与代码不符判 not_fixed。
> 用例计数类断言以全量测试 agent 输出的 cargo test / npm test 统计行为取数口径。

## 断言清单

- **DOC-01**：根 `.claude/CLAUDE.md` 模块索引表含两行——`src/features/backgroundTasks | ../src/features/backgroundTasks/CLAUDE.md` 与 `src-tauri/src/background_tasks | ../src-tauri/src/background_tasks/CLAUDE.md`（grep 各命中一次）。
- **DOC-01**：`src-tauri/src/background_tasks/CLAUDE.md` 存在且含：静态切片注册表 U2 形态理由、顺序写死「校验→落盘→内存」、单写通道复用 settings.rs 禁第二通道、锁序 CONFIG_WRITE_LOCK→SETTINGS_SAVE_LOCK、spawn/emit 包装层 L1 豁免登记（既定豁免表）。
- **DOC-01**：`src/features/backgroundTasks/CLAUDE.md` 存在且含：注册表家族契约 #13、订阅生命周期（首个订阅者读配置+立即一轮+interval / 末退订停）、tick 静默 vs manual error 失败策略、force 恒 true 理由、注册触发点登记（useAgentHistory.ts 与 BackgroundTasksPage.tsx 顶部 import ./tasks）。
- **DOC-01**：grep `plan_balance_set_interval` 在全部 CLAUDE.md（根 + src/** + src-tauri/** + e2e-tests/）零命中。
- **DOC-01**：`src-tauri/src/CLAUDE.md` settings.rs 节——白名单口径含 `background_tasks::SETTINGS_KEY`、无 plan_balance 键表述；含 `save_settings_blocking` 同步写通道句。`src-tauri/src/plan_balance/CLAUDE.md` 轮询间隔节已改写（含：通用件上提 background_tasks / plan_balance_set_interval 退役走 background_tasks_set_config / enabled=false 停轮询+快照保留+footer 隐藏 / 默认 60→10s / poll_once_executor 保留）——Read 逐点确认。
- **DOC-01**：`src/features/agentHistory/CLAUDE.md`「数据流与刷新时机」节含订阅调度器快照 / triggerNow / force 恒 true / scan 退役口径；`src/features/navTree/CLAUDE.md` FE-19 节改写（挂载即扫→订阅首轮、刷新钮=triggerNow）+ 余量 footer 行含「enabled=false 不渲染（F12）」+ 测试模式行含 background-tasks 两测试文件。
- **DOC-01**：`src/ipc/CLAUDE.md` planBalance 命令段 grep `setPlanBalanceInterval` 零命中；含「backgroundTasks 命令（F12）」段（list/set_config/onBackgroundTasksUpdated）；契约文件清单含 `ipc-background-tasks-contract.test.ts`。`src/types/CLAUDE.md` 对照表含 `backgroundTasks.ts ↔ src-tauri/src/background_tasks/mod.rs` 行。`src/__tests__/CLAUDE.md` 全局 mock 清单含 `../ipc/backgroundTasks` 行 + F12 迁移映射行（settings-plan-balance 退役 → settings-background-tasks 新增）。
- **DOC-01**：`CONTEXT.md`「全局组」行含「后台定时任务」（无「套餐余量查询频率」残留）；新增三术语存在——**后台定时任务** / **扫描执行体** / **触发来源**（grep 各命中）。
- **DOC-02**：`.claude/test-inventory.md`——settings.rs 行 27 例、plan_balance/mod.rs 行 14 例、ipc-plan-balance-contract 行 12 例；新增 background_tasks 模块行 + ipc-background-tasks-contract / settings-background-tasks / background-tasks-scheduler / background-tasks-session-refresh 行；settings-plan-balance 行已删；E2E 区含 background-tasks.e2e.ts 6 例行。全部计数与全量测试 agent 输出的统计行一致（不一致判 not_fixed 并给出两侧数值）。
- **DOC-02**：豁免清单新增两条——background_tasks spawn/emit 包装层（L1 不可测，兜底 L4+人工）、tick 失败静默 E2E 豁免（兜底 L2+人工）；变更日志含 2026-08-31 F12 行。
- **DOC-03**：grep `ADR-0013` 命中 `.claude/adr.md`；条目含四决策点（双端各自抽象 / 任务元数据单点在后端注册表 / 订阅者计数生命周期 / 配置变更经 emit 事件感知）；根 `.claude/CLAUDE.md` F12 行「决策见 ADR-0013」不再断链（adr.md 有对应条目即不断链）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. （环境豁免，2026-08-31 登记）命令 5 预期 exit 127：本机 rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，见 .claude/test-inventory.md 豁免表）；测试类断言以「测试存在性 grep + `cargo check --tests` 编译级 + clippy」为兜底。
