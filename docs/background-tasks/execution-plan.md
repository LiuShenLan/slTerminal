# F12 后台定时任务——执行编排参数

- 清单：`docs/background-tasks/checklist.md`；Stage 划分：`docs/background-tasks/stages.md`
- 通用执行规则（resume 语义 / fix-loop 调用 / 时间盒 / git 操作）单一真值源 = `/systematic-changes-execute`，本文件不复制

## Stage 表

| Stage | 脚本 | verify | 改动项 | commit message |
|---|---|---|---|---|
| 01 后端骨架 | `docs/background-tasks/workflows/stage-01-backend.js` | `docs/background-tasks/workflows/verify/stage-01.md` | BE-01~05 | `feat(background-tasks): F12 后端任务骨架——注册表/poller 驱动/配置命令 + plan_balance 执行体下沉` |
| 02 前端基建 | `docs/background-tasks/workflows/stage-02-frontend-infra.js` | `docs/background-tasks/workflows/verify/stage-02.md` | FE-01~03 | `feat(background-tasks): F12 前端基建——IPC wrapper/契约测试 + 调度器注册表 + sessionRefresh 扫描执行体` |
| 03 消费改造 | `docs/background-tasks/workflows/stage-03-consumers.js` | `docs/background-tasks/workflows/verify/stage-03.md` | FE-04~06 | `feat(background-tasks): F12 消费改造——useAgentHistory 订阅调度器（scan 退役）+ footer enabled 感知` |
| 04 设置中心页 | `docs/background-tasks/workflows/stage-04-settings-page.js` | `docs/background-tasks/workflows/verify/stage-04.md` | FE-07/08 | `feat(settings): F12 设置中心「后台定时任务」页——planBalance 页替换为通用任务行页` |
| 05 E2E | `docs/background-tasks/workflows/stage-05-e2e.js` | `docs/background-tasks/workflows/verify/stage-05.md` | E2E-01~03 | `test(e2e): F12 后台定时任务端到端——settings.e2e 适配新页 + background-tasks.e2e 六例` |
| 06 文档 | `docs/background-tasks/workflows/stage-06-docs.js` | `docs/background-tasks/workflows/verify/stage-06.md` | DOC-01~03 | `docs(background-tasks): F12 文档收口——模块索引/CLAUDE.md 系列/CONTEXT 术语/ADR-0013/test-inventory` |

## fix-loop 调用规范

`Workflow({ scriptPath: "docs/background-tasks/workflows/fix-loop.js", args: { stage, failedItems, fixContext, verifyFile, constraints } })`

- `verifyFile` = 对应该 Stage 的 `docs/background-tasks/workflows/verify/stage-NN.md`（与 Stage 脚本同一真值源）
- `constraints`：各 Stage 无特殊纪律（无"只改测试"Stage）——传空串；Stage 05 传「只改 e2e-tests/ 下文件，不改 src/ 与 src-tauri/ 生产代码；若失败根因在生产代码，报告并停止」

## git add 路径（config.json workflow.gitAddPaths）

`src/` `src-tauri/` `e2e-tests/` `test/` `.claude/CLAUDE.md` `.claude/test-inventory.md` `.claude/adr.md` `CONTEXT.md` `docs/`

## 门禁命令（各 Stage 全量测试 agent 统一执行）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

Stage 05 补充说明：e2e-tests/ 不在根 tsconfig include（include = src + test/**）且无独立 tsconfig——tsc/eslint 不覆盖本 Stage 改动文件，wdio 实跑为人工验证点（见 stages.md Stage 05），门禁五条仅作回归防线。

## 进度跟踪表

| Stage | 状态 | 全量测试 | verify | commit |
|---|---|---|---|---|
| 01 后端骨架 | 未开始 | - | - | - |
| 02 前端基建 | 未开始 | - | - | - |
| 03 消费改造 | 未开始 | - | - | - |
| 04 设置中心页 | 未开始 | - | - | - |
| 05 E2E | 未开始 | - | - | - |
| 06 文档 | 未开始 | - | - | - |

## 收尾（全部 Stage 完成后，人工）

1. 全量门禁 + `npm run e2e` + 三个人工验证点（stages.md Stage 05）。
2. 归档删除 `docs/background-tasks/` 计划产物（规格 docs/background-tasks-spec.md 保留）。
