# Phase 3 执行计划 — Hooks 双模式配置面板

> 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）见 `/systematic-changes-execute`，本文件只写任务特定编排参数。

---

## Stage 表

| Stage | 内容 | 项数 | 并行 agent 数 | 依赖 |
|-------|------|------|---------------|------|
| 01 | 后端三层配置读写命令 | 10 | 2 | — |
| 02 | IPC 封装、DTO、matcher 引擎、configModel | 6 | 2 | — |
| 03 | 面板骨架、注册、数据 hook、store | 9 | 2（串行） | 01、02 |
| 04 | Schema 内嵌与 JSON 模式 | 4 | 1 | 03 |
| 05 | GUI 表单模式（Master-Detail） | 5 | 2 | 03 |
| 06 | 双模式同步与保存安全 | 4 | 1 | 04、05 |
| 07 | 单条启停（ADR-0002）与 F2 并入 | 6 | 2（串行） | 06 |
| 08 | 面板入口全局命令 | 3 | 1 | 03 |
| 09 | L4 E2E 关键路径 | 1 | 1 | 全部代码 Stage |
| 10 | 文档同步 | 5 | 1 | 全部 |

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
| 02 | `src/ipc/hooksConfig.ts` `src/ipc/index.ts` `src/types/hooksConfig.ts` `src/panels/hooksConfig/matcherEngine.ts` `src/panels/hooksConfig/configModel.ts` |
| 03 | `src/panels/hooksConfig/index.ts` `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/useHooksConfig.ts` `src/panelRegistry.ts` `src/panels/index.ts` `src/stores/hooksConfig.ts` `src/stores/index.ts` `src/App.tsx` `src/__tests__/panel-registry.test.ts` |
| 04 | `src/features/hooksConfig/schema/claude-code-settings.json` `src/panels/hooksConfig/JsonMode.tsx` `src/panels/hooksConfig/MatcherTester.tsx` `src/panels/hooksConfig/HooksConfigPanel.tsx` `package.json` `package-lock.json` |
| 05 | `src/panels/hooksConfig/GuiMode.tsx` `src/panels/hooksConfig/EventTree.tsx` `src/panels/hooksConfig/HandlerForm.tsx` `src/panels/hooksConfig/HooksConfigPanel.tsx` |
| 06 | `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/useHooksConfig.ts` `src/panels/hooksConfig/configModel.ts` |
| 07 | `src/stores/hooksConfig.ts` `src/panels/hooksConfig/HooksConfigPanel.tsx` `src/panels/hooksConfig/EventTree.tsx` `src/panels/hooksConfig/HandlerForm.tsx` |
| 08 | `src/features/shortcuts/commandCatalog.ts` `src/features/shortcuts/globalCommands.ts` `src/__tests__/command-catalog.test.ts` |
| 09 | `e2e-tests/test.e2e.ts`（必要时 `e2e-tests/helpers.ts`） |
| 10 | `src/panels/CLAUDE.md` `src/ipc/CLAUDE.md` `src/stores/CLAUDE.md` `src/features/shortcuts/CLAUDE.md` `.claude/test-inventory.md` |

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
| 01-08 | `""` |
| 09 | `"本 Stage 只追加 E2E 测试，禁止修改生产代码"` |
| 10 | `"本 Stage 只改文档，禁止修改生产代码"` |

---

## 进度跟踪表

| Stage | 状态 | commit SHA | 通过测试 | 遗留 failedItems |
|-------|------|------------|----------|------------------|
| 01 | 未开始 | — | — | — |
| 02 | 未开始 | — | — | — |
| 03 | 未开始 | — | — | — |
| 04 | 未开始 | — | — | — |
| 05 | 未开始 | — | — | — |
| 06 | 未开始 | — | — | — |
| 07 | 未开始 | — | — | — |
| 08 | 未开始 | — | — | — |
| 09 | 未开始 | — | — | — |
| 10 | 未开始 | — | — | — |

---

## 待执行期确认清单

| # | 项 | 推荐值 | 影响范围 |
|---|----|--------|----------|
| 1 | `global.openHooksConfig` 默认键 | `Ctrl+Shift+H` | Stage 08；如被浏览器拦截则降级为 `Ctrl+Alt+H` |

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
