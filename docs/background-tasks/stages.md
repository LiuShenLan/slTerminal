# F12 后台定时任务——Stage 划分

- 清单：`docs/background-tasks/checklist.md`（BE-01~05 / FE-01~08 / E2E-01~03 / DOC-01~03，共 19 项）
- 划分原则：Stage 内文件零重叠（分工表文件清单 = 脚本 prompt 触碰文件全集）；Stage 间串行 + 每 Stage commit；文档固定最后 Stage
- 跨边界契约：见 checklist.md「跨边界契约」节——各 Stage 脚本头部原文引用，双端 agent 不各自推断

## Stage 01 后端骨架（BE-01 ~ BE-05）

**改动项**：BE-01 / BE-02 / BE-03 / BE-04 / BE-05

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| backend-tasks | BE-01 / BE-02 / BE-03 | `src-tauri/src/background_tasks/registry.rs`（新建）、`src-tauri/src/background_tasks/mod.rs`（新建）、`src-tauri/src/plan_balance/mod.rs` |
| backend-wiring | BE-04 / BE-05 | `src-tauri/src/settings.rs`、`src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json` |

文件零重叠证明：两 agent 文件清单无交集（registry.rs/mod.rs/plan_balance vs settings/lib/build/capabilities）。backend-wiring 引用 `crate::background_tasks::SETTINGS_KEY` 与 `background_tasks::background_tasks_list/set_config` 路径——契约见 checklist 契约区，写死于脚本头部。

**实现要点**：
- `save_settings_blocking` 抽取是 BE-02 与 BE-04 的衔接点：backend-tasks 的 `set_config_core` 调用 `crate::settings::save_settings_blocking`（backend-wiring 提供）——两 agent 并行写，全量测试阶段汇合编译。
- 锁序单向：CONFIG_WRITE_LOCK（background_tasks）→ SETTINGS_SAVE_LOCK（settings.rs 内部），无环。
- plan_balance 删 10 例测试后保留 14 例零改动。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-01.md`。

**commit message**：`feat(background-tasks): F12 后端任务骨架——注册表/poller 驱动/配置命令 + plan_balance 执行体下沉`

## Stage 02 前端基建（FE-01 ~ FE-03）

**改动项**：FE-01 / FE-02 / FE-03

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| frontend-ipc | FE-01 / FE-02 | `src/types/backgroundTasks.ts`（新建）、`src/ipc/backgroundTasks.ts`（新建）、`src/ipc/index.ts`、`src/ipc/planBalance.ts`、`src/__tests__/ipc-background-tasks-contract.test.ts`（新建）、`src/__tests__/ipc-plan-balance-contract.test.ts`、`src/__tests__/setup.ts` |
| frontend-scheduler | FE-03 | `src/features/backgroundTasks/types.ts`（新建）、`scheduler.ts`（新建）、`sessionRefreshTask.ts`（新建）、`tasks.ts`（新建）、`index.ts`（新建）、`src/__tests__/background-tasks-scheduler.test.ts`（新建）、`src/__tests__/background-tasks-session-refresh.test.ts`（新建） |

文件零重叠证明：types/ipc/tests-setup vs features/backgroundTasks+两测试，无交集。frontend-scheduler 的 scheduler.ts 调 `listBackgroundTasks`（frontend-ipc 新建）——命令名/签名写死于脚本头部契约。

**实现要点**：
- setup.ts 全局 mock 在本 Stage 加入——Stage 03 nav-tree 测试（真实 useAgentHistory → 调度器 activate → listBackgroundTasks）依赖它先行到位。
- 调度器快照通用形状 `{ state, data }`；sessionRefresh 执行体经 `prev` 参数做部分失败隔离。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-02.md`。

**commit message**：`feat(background-tasks): F12 前端基建——IPC wrapper/契约测试 + 调度器注册表 + sessionRefresh 扫描执行体`

## Stage 03 消费改造（FE-04 ~ FE-06）

**改动项**：FE-04 / FE-05 / FE-06

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| history-consumer | FE-04 / FE-05 | `src/features/agentHistory/useAgentHistory.ts`、`src/features/navTree/useNavTree.ts`、`src/__tests__/agent-history-hook.test.tsx`、`src/__tests__/nav-tree.test.tsx`、`src/__tests__/nav-tree-history.test.tsx` |
| balance-footer | FE-06 | `src/features/navTree/usePlanBalance.ts`、`src/features/navTree/PlanBalanceFooter.tsx`、`src/__tests__/plan-balance-footer.test.tsx` |

文件零重叠证明：useNavTree.ts 与 usePlanBalance.ts/PlanBalanceFooter.tsx 为不同文件；测试文件各自独立。两 agent 同触 `src/features/navTree/` 目录但文件级无交集。

**实现要点**：
- 依赖 Stage 02：调度器 + ipc/backgroundTasks + setup.ts 全局 mock 均已到位。
- nav-tree 两测试文件的「挂载即扫」断言语义保留（订阅首轮即调 scanAgentHistory），force 参数断言改 true。
- footer「enabled !== true 不渲染」守卫：初始 null（未加载）也不渲染——防闪烁决策已写死。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-03.md`。

**commit message**：`feat(background-tasks): F12 消费改造——useAgentHistory 订阅调度器（scan 退役）+ footer enabled 感知`

## Stage 04 设置中心页（FE-07 / FE-08）

**改动项**：FE-07 / FE-08

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| settings-page | FE-07 / FE-08 | `src/features/settingsCenter/pages.ts`、`src/panels/settings/pages/BackgroundTasksPage.tsx`（新建）、`src/panels/settings/pages/PlanBalancePage.tsx`（删除）、`src/__tests__/settings-background-tasks.test.tsx`（新建）、`src/__tests__/settings-plan-balance.test.tsx`（删除）、`src/__tests__/settings-pages-registration.test.ts` |

单 agent（页组件/注册/测试高度耦合）。FE-08 为纯验证项（SettingsPanel.tsx 零改动——深链兜底现状已满足，settings-panel.test.tsx 既有用例覆盖）。

**实现要点**：立即提交型语义照 PlanBalancePage 先例；DTO 无 default 字段 → 行内提示只写范围不写默认值；data-e2e 新系列 `settings-background-tasks-*`。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-04.md`。

**commit message**：`feat(settings): F12 设置中心「后台定时任务」页——planBalance 页替换为通用任务行页`

## Stage 05 E2E（E2E-01 ~ E2E-03）

**改动项**：E2E-01 / E2E-02 / E2E-03

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| e2e | E2E-01 / E2E-02 / E2E-03 | `e2e-tests/settings.e2e.ts`、`e2e-tests/background-tasks.e2e.ts`（新建）、`e2e-tests/wdio.conf.ts`（仅当 specs 非 glob 时登记——先读确认） |

单 agent。e2e-tests 不在根 tsconfig include（include = src + test/**）且无独立 tsconfig——本 Stage 无 tsc/eslint 覆盖，门禁 = 全量五条（回归防线）+ **wdio 人工执行（人工验证点，门禁不内嵌）**。

**实现要点**：
- helper 复用 settings.e2e.ts / history.e2e.ts 既有模式（假 env 注入 / waitForSettingsFile / SLTERM_CLAUDE_PROJECTS_DIR 写 jsonl / nav-history-node 计数断言）。
- E2E 默认配置下 sessionRefresh（3s）与 planBalance（10s）定时任务全程在跑——既有 history.e2e.ts/settings.e2e.ts 须连带复跑确认无干扰（人工验证点）。

**人工验证点**（无法自动化，收尾实测兜底）：
1. `npm run e2e` 全量绿（含新 background-tasks.e2e.ts 六例）。
2. 真实 claude 会话：导航树历史区按配置频率自动更新（新会话出现/进行中会话标题时间与磁盘一致），与点击刷新钮结果一致。
3. 勾选禁用套餐余量 → 轮询停止、footer 隐藏、最后快照保留；重新启用 → 恢复。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-05.md`。

**commit message**：`test(e2e): F12 后台定时任务端到端——settings.e2e 适配新页 + background-tasks.e2e 六例`

## Stage 06 文档同步（DOC-01 ~ DOC-03）

**改动项**：DOC-01 / DOC-02 / DOC-03

| agent label | 负责项 | 触碰文件 |
|---|---|---|
| docs-backend | DOC-01 后半 / DOC-03 | `.claude/CLAUDE.md`（模块索引两行）、`src-tauri/src/background_tasks/CLAUDE.md`（新建）、`src-tauri/src/CLAUDE.md`、`src-tauri/src/plan_balance/CLAUDE.md`、`.claude/adr.md`（ADR-0013 补写） |
| docs-frontend | DOC-01 前半 / DOC-02 | `src/features/backgroundTasks/CLAUDE.md`（新建）、`src/features/agentHistory/CLAUDE.md`、`src/features/navTree/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src/types/CLAUDE.md`、`src/__tests__/CLAUDE.md`、`CONTEXT.md`、`.claude/test-inventory.md` |

文件零重叠证明：两 agent 各持一组 md 文件，无交集。

**实现要点**：
- test-inventory 用例计数以**执行后实跑统计**为准（cargo test / npm test 统计行），禁照抄计划预估值。
- 文档口径必须对照 Stage 01-05 完成后的真实代码核实（防文档撒谎）。
- 用内置 Write/Edit 写中文 markdown（filesystem MCP 有乱码风险）。

**验证项**：见 `docs/background-tasks/workflows/verify/stage-06.md`。

**commit message**：`docs(background-tasks): F12 文档收口——模块索引/CLAUDE.md 系列/CONTEXT 术语/ADR-0013/test-inventory`

## 收尾（Stage 06 后，人工）

1. 全量门禁五条 + `npm run e2e` 人工执行（含上方三个人工验证点）。
2. 计划产物归档：`docs/background-tasks/`（checklist/stages/execution-plan/workflows）实施完毕后删除（照 F10/F11 先例——规格 `docs/background-tasks-spec.md` 保留）。
