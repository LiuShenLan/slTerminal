# Stage 02 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 后 `colors.ts`/`index.ts`/`colors.test.ts` 已切 facade；**消费点未迁移**（四文件 oneDark 仍在，Stage 03 才做）；`main.tsx` 未动（Stage 04 才做）。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | FAC-01 | `src/theme/colors.ts` 导出名集合恰 31 个，与 stages.md C1 清单逐字一致（5 组 + ERROR_BANNER 3 + 标量 22 + ROOT_CSS_VARS） | Grep `export` 名单比对 C1 |
| 2 | FAC-01 | 31 导出值代理 `schemeRegistry.getActive()`——语义式：Read 确认导出值经 getActive() 求值（getter/模块求值均可），**非色值字面量直写** | Read |
| 3 | FAC-01 | 死配置零残留：全仓 grep `DROPDOWN_BG`、`APP_BG_SECONDARY`、`whitespaceOnly`、`--sl-bg-secondary` 四处均零命中（含测试文件，但 `docs/` 计划文档豁免） | Grep 四词 |
| 4 | FAC-01 | `SIDEBAR_COLORS.selected` 保留（值 `#37373D` 经 facade 透出）；`EXPLORER_COLORS` 无 selected 键 | Grep + Read |
| 5 | FAC-01 | `ON_ACCENT_FG` 存在且透出值 `#FFFFFF` | Read |
| 6 | FAC-01 | `ROOT_CSS_VARS` 键集合恰 `{ "--sl-bg-primary", "--sl-fg-primary" }`（2 键） | Read |
| 7 | FAC-02 | `src/theme/index.ts` 含 C1 的 31 个 re-export + 追加 `schemeRegistry`、`./schemes`、`./overrides` 导出 | Read |
| 8 | TST-01 | `src/__tests__/colors.test.ts` 六处同步完成：import 无 DROPDOWN_BG/APP_BG_SECONDARY 且有 ON_ACCENT_FG；GIT_GUTTER 3 键断言；EXPLORER 5 键断言；通用 UI 标量计数 24；ROOT_CSS_VARS describe 键集合断言恰 2 且含 `--sl-fg-primary` 值 `#cdd6f4` 断言、无 `--sl-bg-secondary` 断言 | Read 六处 |
| 9 | 中间态 | `src/main.tsx` 零改动（BOOT 在 Stage 04）；四 oneDark 消费文件零改动（CON 在 Stage 03） | git diff --name-only HEAD 比对 |
| 10 | 中间态 | 本 Stage diff 仅含：`src/theme/colors.ts`、`src/theme/index.ts`、`src/__tests__/colors.test.ts` | git diff |

## 全量测试（全部通过为门禁）

- `npx tsc --noEmit` → exit 0
- `npx eslint src/` → exit 0
- `npm test` → exit 0（重点：`colors.test.ts` 全绿）
