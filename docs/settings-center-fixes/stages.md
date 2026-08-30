# F11 设置中心 Review 修复——Stage 划分

> 清单真值源：`docs/settings-center-fixes/checklist.md`。优先级由 Stage 依赖顺序表达（不用 P0-P4）。每 Stage commit；Stage 内文件不重叠；文档 Stage 固定最后。

## 前置 OP（不计 Stage）

| OP | 内容 | 状态 |
|----|------|------|
| OP-01 | 数据恢复（.bak → slterminal-projects.json） | ✅ 已完成（计划期执行，恢复前确认无 slterminal 进程在跑） |
| OP-02 | 全部 Stage 落地后：用户重建普通构建 `npx tauri build --debug --no-bundle` 覆盖 E2E 构建 exe；人工验证点——①启动自动恢复 mattpocock_skills；②错误页手测（把 exe 同级 slterminal-projects.json 改为 `{"projects": 1}` → 启动应出错误页而非空白；恢复文件 → 重试按钮进应用） | 待执行 |

## Stage 01 数据防线（3 agent 并行）

| label | 负责项 | 文件 |
|-------|--------|------|
| be-01-appdir | BE-01 | `src-tauri/src/app_dir.rs` |
| be-02-tracing | BE-02 | `src-tauri/src/projects.rs` |
| fe-01-store-defense | FE-01 | `src/stores/projects.ts`、`src/__tests__/projects.test.ts` |

- 实现要点：BE-01 优先级 guard > env > exe 推导（`cfg(test)` guard 生产零编译）；FE-01 放行语义三路径（loadFromDisk 成功自动置位 / E2E 分支与「以空状态继续」经 markLoadSucceeded() 显式放行 / 删除最后一个项目时 loadSucceeded 已为 true 自然放行，无需 allowEmpty 参数）。
- 门禁命令：
  1. `npx tsc --noEmit`
  2. `npx eslint src/`
  3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
  4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  5. `npm test`
  6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- commit：`fix(projects): 数据防线——SLTERM_DATA_DIR 隔离 + 加载 tracing + loadSucceeded 空写守卫`

## Stage 02 启动链阻断 + 错误页（3 agent 并行）

| label | 负责项 | 文件 |
|-------|--------|------|
| fe-02-app-error-page | FE-02 | `src/App.tsx` |
| fe-03-hydration-gate | FE-03 | `src/panels/settings/SettingsPanel.tsx`、`src/__tests__/settings-panel-autoclose.test.tsx` |
| te-01-startup-tests | TE-01 | `src/__tests__/startup-restore.test.ts`、`startup-store-fail-warn.test.tsx`、`close-handler.test.ts`、`error-boundary.test.tsx`、`e2e-clipboard-helper.test.ts` |

- **跨 agent 契约写死**：
  - `markLoadSucceeded(): void` / `markPersistenceReady(): void` 均从 `../stores/projects` 导入（App.tsx 为 `./stores/projects`）——两函数 Stage 01 已由 FE-01 落地/既有，本 Stage 只消费不改。
  - 错误页 data-e2e 三值：`projects-load-error` / `projects-load-retry` / `projects-load-continue-empty`。
  - 「slTerminal 启动中…」文本节点保留在 `projectsLoadError === null` 分支（startup-restore 用例 4/8 依赖）。
- 门禁命令：
  1. `npx tsc --noEmit`
  2. `npx eslint src/`
  3. `npm test`
- commit：`fix(app): 启动加载失败阻断写门控 + 错误页（重试/以空状态继续）+ 设置面板水合门控`

## Stage 03 E2E 隔离 + L4 补强（3 agent 并行）

| label | 负责项 | 文件 |
|-------|--------|------|
| te-02-wdio-isolation | TE-02 | `e2e-tests/run-wdio.cjs` |
| te-03-e2e-cases | TE-03 | `e2e-tests/settings.e2e.ts`、`e2e-tests/helpers.ts` |
| fe-04-tab-close-e2e | FE-04 | `src/workspace/PageDockviewHost.tsx` |

- **跨 agent 契约写死**：
  - env 名 `SLTERM_DATA_DIR`（BE-01 后端已落地，本 Stage 只消费）。
  - × 按钮选择器 `[data-e2e="tab-close-{panelId}"]`（FE-04 提供，TE-03 用例⑪消费）。
  - helper 名 `__slterm_e2e_setSettingsDirty(panelId, dirty)`；`getSettingsPanelState` 返回新增 `panelId` 字段（TE-03 自给）。
  - ConfirmDialog 选择器：`confirm-dialog` / `confirm-cancel` / `confirm-ok`。
- 门禁命令（e2e-tests 无 tsconfig、tsc include 外——构建级门禁强制）：
  1. `npx tsc --noEmit`
  2. `npx eslint src/`
  3. `npm test`
  4. `npm run build:e2e`
  5. settings spec 实跑全绿（`npx wdio run wdio.conf.ts --spec e2e-tests/settings.e2e.ts`，由全量测试 agent 单点执行）
- 人工验证点：无（全自动化）。
- commit：`test(e2e): SLTERM_DATA_DIR 数据目录隔离 + ×关闭 dirty 守卫 L4 用例 + 用例⑧归属断言强化`

## Stage 04 L2 测试质量（3 agent 并行）

| label | 负责项 | 文件 |
|-------|--------|------|
| te-04-pages-guard | TE-04 | `src/__tests__/settings-pages-registration.test.ts`（新建） |
| te-05-hooks-page-tests | TE-05 | `src/__tests__/settings-hooks-page.test.tsx`、`src/__tests__/hooks-config-sync.test.tsx` |
| te-06-panel-assertions | TE-06 | `src/__tests__/settings-panel.test.tsx` |

- 本 Stage 只改测试，禁止改生产代码（fix-loop 调用时 args.constraints 传「本 Stage 只改测试，禁止改生产代码」）。
- 门禁命令：
  1. `npx tsc --noEmit`
  2. `npx eslint src/`
  3. `npm test`
- commit：`test(settings): pages.ts 注册守卫 + persistSelectedCli 短路用例 + saveLayout 落盘断言 + 死 mock 清理`

## Stage 05 文档收口（4 agent 并行，固定最后）

| label | 负责项 | 文件 |
|-------|--------|------|
| doc-01-02-requirements | DOC-01、DOC-02 | `docs/settings-center-requirements.md` |
| doc-03-07-inventory | DOC-03、DOC-07 | `src/__tests__/CLAUDE.md`、`.claude/test-inventory.md` |
| doc-04-e2e-docs | DOC-04 | `docs/settings-center/report.md`、`e2e-tests/CLAUDE.md` |
| doc-05-06-module-docs | DOC-05、DOC-06 | `src/features/settingsCenter/types.ts`、`src/stores/CLAUDE.md`、`src-tauri/src/settings.rs`、`src-tauri/src/CLAUDE.md` |

- 收尾：全量四级回归——`npx tsc --noEmit` / `npx eslint src/` / `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` / `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` / `npm test` / `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` / `npm run test:l3` / `npm run e2e`（全 spec）。
- DOC-07 计数以本 Stage 实跑输出校准（预测 L1 818 / L2 2852 / L3 142 / L4 51，总 3864——实跑为准）。
- commit：`docs(settings): 修复计划文档收口——规格失实改写/偏离登记/清单计数同步`

## 人工验证点汇总

| Stage | 验证点 |
|-------|--------|
| OP-02 | 普通构建覆盖 E2E exe；启动恢复 mattpocock_skills；错误页手测（损坏 json → 错误页 → 恢复 → 重试进应用） |
| Stage 03 | 无（settings spec 实跑兜底） |
