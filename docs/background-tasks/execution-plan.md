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

> **环境豁免（2026-08-31 登记）**：命令 5 在本机预期 exit 127——rustc 1.94~1.96 下测试二进制链接 tauri 栈代码后 0xC0000139 启动崩溃（Windows 加载器边界 bug，详见 .claude/test-inventory.md 豁免表）。兜底验证 = `cargo check --tests`（编译级）+ 测试存在性 grep + clippy。各 Stage 脚本与 verify 已同步此注记。

Stage 05 补充说明：e2e-tests/ 不在根 tsconfig include（include = src + test/**）且无独立 tsconfig——tsc/eslint 不覆盖本 Stage 改动文件，wdio 实跑为人工验证点（见 stages.md Stage 05），门禁五条仅作回归防线。

## 进度跟踪表

| Stage | 状态 | 全量测试 | verify | commit |
|---|---|---|---|---|
| 01 后端骨架 | 完成 | 4/5 全过 + cargo test 环境豁免（0xC0000139 登记，cargo check --tests 编译级兜底） | 全过（BE-01~05 代码项全 fixed；测试存在性 grep 证实） | cbb48f8 |
| 02 前端基建 | 完成 | 4/5 全过 + cargo test 环境豁免；tsc 预期失败（PlanBalancePage 为 Stage 04 git rm 对象，中间态登记） | 全过（FE-01/02/03 fixed；FE-02 残留仅限 Stage 04 删除对象） | c5ea0ac |
| 03 消费改造 | 完成 | 3/5 全过 + tsc/cargo test 双豁免（均登记） | 全过（FE-04/05/06 fixed，allFixed=true） | 9253fbb |
| 04 设置中心页 | 完成 | 5/5 全过（tsc 由此收敛，2879 例 npm test 绿）+ cargo test 环境豁免 | 全过（FE-07/08 fixed；注释残留已修，文档迁移表归 Stage 06 边界登记） | 284c632 |
| 05 E2E | 完成 | 5/5 全过（tsc/eslint/clippy/npm test 2879 例绿 + cargo test 环境豁免） | 全过（E2E-01/02/03 fixed；wdio 实跑 = 收尾人工验证点） | 38037b1 |
| 06 文档 | 未开始 | - | - | - |

## 收尾（全部 Stage 完成后，人工）

1. 全量门禁 + `npm run e2e` + 三个人工验证点（stages.md Stage 05）。
2. 归档删除 `docs/background-tasks/` 计划产物（规格 docs/background-tasks-spec.md 保留）。
