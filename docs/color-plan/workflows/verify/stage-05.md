# Stage 05 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 仅新增/修改测试与 inventory；生产代码零改动。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | TST-02 | `src/__tests__/scheme-registry.test.ts` 存在，用例数 ≥12（`it(`/`test(` 计数）；覆盖：register/get/getAll/getDefaultId、setActive 已知/未知 id（回退 + warn）、getActive 默认 darcula、重复注册覆盖、_reset 隔离、darcula 四段完整性 | Grep 计数 + Read 覆盖面对照 |
| 2 | TST-03 | `src/__tests__/overrides.test.ts` 存在，用例数 ≥5；覆盖：dockviewVarStyle 20 条/allotmentVarStyle 2 键/editorTheme 透出/editorColorOverrides 扩展/setActive 后跟随切换 | Grep 计数 + Read |
| 3 | TST-02/03 | 两文件均使用 `_reset()` 隔离（beforeEach 或等效），不依赖执行序 | Grep `_reset` |
| 4 | TST-04 | 四文件 `theme.test.ts`/`main-bootstrap.test.tsx`/`gitshow-panel.test.tsx`/`hooks-config-jsonmode.test.tsx` + L3 `theme-options.test.ts` 零改动；若有改动，diff 须在报告附理由（失效原因 = 值漂移则属失败项，非测试问题） | git diff + 报告 |
| 5 | TST-05 | `.claude/test-inventory.md` 含 `scheme-registry.test.ts` 与 `overrides.test.ts` 登记及用例数；`colors.test.ts` 计数与实际一致（`npx vitest run colors --reporter=verbose` 产出数或文件内 `it(` 计数核对） | Grep + 计数比对 |
| 6 | 中间态 | 本 Stage diff 仅含：两新测试文件 + `.claude/test-inventory.md`（+ TST-04 附理由的四文件，若触） | git diff --name-only HEAD |

## 全量测试（全部通过为门禁）

- `npm test` → exit 0（L2 全量，含两新文件）
