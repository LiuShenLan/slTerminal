# Stage 05 Verify：L4 E2E 关键路径

> 断言与 Stage 05 完成后的真实中间态一致。

## E2E Helpers

- [ ] `P1-TE-04` `e2e-tests/helpers.ts` 中 `installAllE2eHelpers` 安装了 `__slterm_e2e_injectHooks`。
- [ ] `P1-TE-04` 安装了 `__slterm_e2e_uninstallHooks`。
- [ ] `P1-TE-04` 安装了 `__slterm_e2e_getHookInjectionStatus`。
- [ ] `P1-TE-04` helpers 仅在 `E2E_ENABLED` 时安装。

## E2E 用例

- [ ] `P1-TE-03` `e2e-tests/test.e2e.ts` 新增 describe("hooks 状态可视化")。
- [ ] `P1-TE-03` 用例 1：调用 `__slterm_e2e_injectHooks()` 后 `__slterm_e2e_getHookInjectionStatus()` 返回 `status === "injected"`。
- [ ] `P1-TE-03` 用例 2：Node 写 `UserPromptSubmit` 信号文件后，页签 DOM 含 "⚡"。
- [ ] `P1-TE-03` 用例 2：Node 写 `SessionEnd` 信号文件后，页签 DOM 不再含 "⚡"。

## 构建与运行

- [ ] `npm run build:e2e` 成功。
- [ ] `npm run wdio` 中新增用例通过。
- [ ] 生产 dist 不含 helper（`npx vite build` 后 grep `__slterm_e2e_` 无命中）。
