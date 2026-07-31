# Phase 3 执行计划 — Hooks 双模式配置面板

> 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）见 `/systematic-changes-execute`，本文件只写任务特定编排参数。
> **2026-07-31 修订**：对照 Phase 2 完成后代码现状 + 官方核实 + 8 项用户拍板全面修订；Stage 项数、git add 路径、测试文件名与新 checklist.md / stages.md 严格一致。

---

## Stage 表

| Stage | 内容 | 项数 | 并行 agent 数 | 依赖 |
|-------|------|------|---------------|------|
| 01 | 后端三层 hooks 子树读写命令 | 10 | 2 | — |
| 02 | IPC 封装、DTO、eventsCatalog、matcher 引擎、configModel | 8 | 2 | — |
| 03 | 面板骨架、注册、数据 hook、store | 9 | 2（串行） | 01、02 |
| 04 | Schema 内嵌与 JSON 模式 | 4 | 1 | 03 |
| 05 | GUI 表单模式（Master-Detail） | 5 | 2 | 03 |
| 06 | 双模式同步与保存安全 | 4 | 1 | 04、05 |
| 07 | 单条启停（ADR-0002）与 F2 并入 | 6 | 2（串行） | 06 |
| 08 | 面板入口全局命令（同页单例） | 4 | 1 | 03 |
| 09 | L4 E2E 关键路径（走 project 层） | 1 | 1 | 全部代码 Stage |
| 10 | 文档同步 + 契约回查 | 6 | 1 | 全部 |

---

## 测试文件命名约定（全链一致）

执行期新增/修改的测试文件统一如下，workflow 脚本与 verify 断言引用同一组路径：

| Stage | 测试文件 |
|-------|----------|
| 01 | 单元测试内嵌 `src-tauri/src/hooks/config.rs` `#[cfg(test)] mod tests`（照 hooks 模块先例） |
| 02 | `src/__tests__/ipc-hooks-config-contract.test.ts`（P3-FE-05 契约验证，照 `ipc-hooks-contract.test.ts` 模式）、`src/__tests__/hooks-config-matcher.test.ts`（P3-TE-05）、`src/__tests__/hooks-config-model.test.ts`（P3-TE-06）、`src/__tests__/hooks-config-catalog.test.ts`（P3-TE-19） |
| 03 | `src/__tests__/hooks-config-store.test.ts`（P3-TE-07）、`src/__tests__/hooks-config-panel.test.tsx`（P3-TE-08）、更新 `src/__tests__/panel-registry.test.ts` |
| 04 | `src/__tests__/hooks-config-jsonmode.test.tsx`（P3-TE-09/10） |
| 05 | `src/__tests__/hooks-config-handlerform.test.tsx`（P3-TE-11）、`src/__tests__/hooks-config-gui.test.tsx`（P3-TE-12） |
| 06 | `src/__tests__/hooks-config-sync.test.tsx`（P3-TE-13/14） |
| 07 | `src/__tests__/hooks-config-disable.test.tsx`（P3-TE-15/16） |
| 08 | 更新 `src/__tests__/command-catalog.test.ts`、新增 `src/__tests__/hooks-config-entry.test.ts`（P3-TE-17 handler 行为） |
| 09 | `e2e-tests/test.e2e.ts` 追加用例（必要时 `e2e-tests/helpers.ts`） |
| 10 | 无（文档） |

---

## Commit message 规范

每 Stage 完成后单独 commit。前缀按变更主体选择：

- 生产代码：`feat:`（新增功能）或 `fix:`（修复）
- 测试：`test:`
- 文档：`docs:`
- 混合 Stage 以主体为准，可在正文列出多行（如 `feat: ...\ntest: ...`）

所有 commit message 结尾必须包含：

```
Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

各 Stage 推荐 message 见 `stages.md` 对应段。

---

## git add 路径枚举

执行期每 Stage commit 只能 `git add` 本 Stage 实际改动的文件。允许的根路径来自 `config.json`：`src/`、`src-tauri/`、`e2e-tests/`、`test/`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`、`docs/`。

各 Stage 典型 add 集合：

| Stage | git add 路径 |
|-------|--------------|
| 01 | `src-tauri/src/hooks/config.rs` `src-tauri/src/hooks/mod.rs` `src-tauri/src/lib.rs` |
| 02 | `src/ipc/hooksConfig.ts` `src/ipc/index.ts` `src/types/hooksConfig.ts` `src/panels/hooksConfig/eventsCatalog.ts` `src/panels/hooksConfig/matcherEngine.ts` `src/panels/hooksConfig/configModel.ts` `src/__tests__/ipc-hooks-config-contract.test.ts` `src/__tests__/hooks-config-matcher.test.ts` `src/__tests__/hooks-config-model.test.ts` `src/__tests__/hooks-config-catalog.test.ts` |
| 03 | `src/panels/hooksConfig/index.ts` `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/useHooksConfig.ts` `src/panelRegistry.ts` `src/panels/index.ts` `src/stores/hooksConfig.ts` `src/stores/index.ts` `src/App.tsx` `src/__tests__/panel-registry.test.ts` `src/__tests__/hooks-config-store.test.ts` `src/__tests__/hooks-config-panel.test.tsx` |
| 04 | `src/features/hooksConfig/schema/claude-code-settings.json` `src/panels/hooksConfig/JsonMode.tsx` `src/panels/hooksConfig/MatcherTester.tsx` `src/panels/hooksConfig/HooksConfigPanel.tsx` `package.json` `package-lock.json` `src/__tests__/hooks-config-jsonmode.test.tsx` |
| 05 | `src/panels/hooksConfig/GuiMode.tsx` `src/panels/hooksConfig/EventTree.tsx` `src/panels/hooksConfig/HandlerForm.tsx` `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/__tests__/hooks-config-gui.test.tsx` `src/__tests__/hooks-config-handlerform.test.tsx` |
| 06 | `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/useHooksConfig.ts` `src/panels/hooksConfig/configModel.ts` `src/__tests__/hooks-config-sync.test.tsx` |
| 07 | `src/stores/hooksConfig.ts` `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/EventTree.tsx` `src/panels/hooksConfig/HandlerForm.tsx` `src/__tests__/hooks-config-disable.test.tsx` |
| 08 | `src/features/shortcuts/commandCatalog.ts` `src/features/shortcuts/globalCommands.ts` `src/__tests__/command-catalog.test.ts` `src/__tests__/hooks-config-entry.test.ts` |
| 09 | `e2e-tests/test.e2e.ts`（必要时 `e2e-tests/helpers.ts`） |
| 10 | `src/panels/CLAUDE.md` `src/ipc/CLAUDE.md` `src/stores/CLAUDE.md` `src/features/shortcuts/CLAUDE.md` `src-tauri/src/hooks/CLAUDE.md` `.claude/test-inventory.md`（契约回查偏差修订时追加 `docs/hooks-dev/contract.md`） |

---

## fix-loop args 规范

当 Stage 验证失败时，主 agent 调用 `fix-loop.js` 并传入以下 args：

```jsonc
{
  "stage": 1,              // Stage 编号
  "failedItems": ["P3-BE-01", "P3-TE-01"],  // verify 返回的未通过项 ID 数组
  "fixContext": "verify agent details 证据原文", // verifyResult.details 的 JSON 字符串
  "verifyFile": "docs/hooks-dev/phase3/workflows/verify/stage-01.md",
  "constraints": ""        // Stage 特殊纪律，无则空字符串
}
```

### 各 Stage constraints

| Stage | constraints 值 |
|-------|----------------|
| 01-03、05-08 | `""` |
| 04 | `"禁止引入 ajv；保存前 schema 校验用 codemirror-json-schema 底层 json-schema-library；schema 文件须核实自包含性（无远程 $ref）"` |
| 09 | `"本 Stage 只追加 E2E 测试，禁止修改生产代码；E2E 用例禁止写真实 ~/.claude/settings.json（走 tempdir 项目 project/local 层）"` |
| 10 | `"本 Stage 只改文档，禁止修改生产代码"` |

---

## 进度跟踪表

| Stage | 状态 | commit SHA | 通过测试 | 遗留 failedItems |
|-------|------|------------|----------|------------------|
| 01 | 完成 | d847578 | L1: config 18 条 + clippy | — |
| 02 | 完成 | 989c991 | L2: 74 条（matcher 25 + model 21 + catalog 16 + contract 12）+ tsc + eslint | — |
| 03 | 完成 | 7f1a0c0 | L2: 62 条（store 21 + panel 16 + panel-registry 25）+ tsc + eslint | — |
| 04 | 完成 | 8bbffb1 | L2: 17 条 + 全量 1808 用例 + tsc + eslint + vite build | — |
| 05 | 完成 | 4954f5f | L2: 59 条（gui 21 + handlerform 38）+ 全量 1867 + tsc + eslint | — |
| 06 | 完成 | 3877cc6 | L2: 9 条 + hooks-config 全 189 + 全量 1876 + tsc + eslint | — |
| 07 | 完成 | d0e6efe | L2: 10 条 + hooks-config 全 205 + tsc + eslint | — |
| 08 | 完成 | 90af351 | L2: 21 条（catalog 14 + entry 7）+ 全量 1900 + tsc + eslint | — |
| 09 | 完成（fix-loop 1 轮） | cc61783 | L4: 26/26（含 P3-TE-18）+ build:e2e + vite build | — |
| 10 | 完成（fix-loop 1 轮） | — | —（文档；契约回查结论见 `docs/hooks-dev/phase3/contract-recheck.md`） | — |

---

## 待执行期确认清单

| # | 项 | 推荐值 | 影响范围 |
|---|----|--------|----------|
| 1 | `global.openHooksConfig` 默认键 | `Ctrl+Shift+H` | Stage 08；如被浏览器拦截则降级为 `Ctrl+Alt+H` |
| 2 | schema 自包含性（有无远程 `$ref`） | 执行期核实后预打包展开 | Stage 04；codemirror-json-schema 仅支持本地 `$ref` |
| 3 | MessageDisplay handler 支持档 | 保守按 command/http/mcp_tool 档 | Stage 02 eventsCatalog；官方明确后回改 |

---

## 文件清单

本计划产物已全部落盘：

- `docs/hooks-dev/phase3/checklist.md`
- `docs/hooks-dev/phase3/stages.md`
- `docs/hooks-dev/phase3/execution-plan.md`
- `docs/hooks-dev/phase3/workflows/stage-01-backend.js`
- `docs/hooks-dev/phase3/workflows/stage-02-ipc-model.js`
- `docs/hooks-dev/phase3/workflows/stage-03-panel-core.js`
- `docs/hooks-dev/phase3/workflows/stage-04-json-mode.js`
- `docs/hooks-dev/phase3/workflows/stage-05-gui-mode.js`
- `docs/hooks-dev/phase3/workflows/stage-06-sync-save.js`
- `docs/hooks-dev/phase3/workflows/stage-07-disable-f2.js`
- `docs/hooks-dev/phase3/workflows/stage-08-shortcut.js`
- `docs/hooks-dev/phase3/workflows/stage-09-e2e.js`
- `docs/hooks-dev/phase3/workflows/stage-10-docs.js`
- `docs/hooks-dev/phase3/workflows/fix-loop.js`
- `docs/hooks-dev/phase3/workflows/verify/stage-01.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-02.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-03.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-04.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-05.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-06.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-07.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-08.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-09.md`
- `docs/hooks-dev/phase3/workflows/verify/stage-10.md`
- `docs/hooks-dev/phase3/contract-recheck.md`（Stage 10 P3-DOC-06 契约回查结论落盘：一致项打勾 + C13-6 偏差记录，2026-08-01）

本计划关联修订产物（2026-07-31，契约与需求同步）：

- `docs/hooks-dev/contract.md`（C12 修正 + C13 全量重写为 C13-1~C13-9）
- `docs/hooks-dev/feature-plan/phase3-config-panel.md`（F6 需求全量回写）
- `docs/hooks/D1/01-hooks-official-docs.md`（§6.1/§6.4/§6.5 字段修订）
- `docs/hooks/D2/02-settings-json-schema.md`（§4.3 支持矩阵修订）
