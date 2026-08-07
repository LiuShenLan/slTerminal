# Stage 01 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 仅新建 5 文件；`colors.ts`/`index.ts`/panels/ 等一切既有文件**未动**（facade 切换在 Stage 02）。
> 取数口径：本 Stage 门禁仅 tsc + eslint，断言全部静态核实（Read/Grep/git diff）。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | SCH-01 | `src/theme/schemes/types.ts` 存在，含 `ColorScheme`/`UiTokens`/`TerminalPalette`/`EditorScheme`/`LibraryOverrides` 五个导出接口 | Grep `export interface` 计数与名单 |
| 2 | SCH-01 | 接口槽位带 JSDoc 消费注释——语义式：抽查 ui/terminal/editor/libraries 四段各至少一处槽位，注释须说明该色在前端何处起作用（非空泛描述） | Read 抽查 |
| 3 | SCH-02 | `src/theme/schemes/darcula.ts` 存在；ui 段 6 组键数 = gitFile 7 / gitGutter 3 / explorer 5 / sidebar 8 / agentStatusUsage 3 / errorBanner 3 | Read 数键 |
| 4 | SCH-02 | ui 标量含 `ON_ACCENT_FG`，值 `"#FFFFFF"`；不含 `DROPDOWN_BG`/`APP_BG_SECONDARY` 槽位 | Grep |
| 5 | SCH-02 | ui 各组值与 colors.ts 现状值逐字一致（零视觉变化）——抽查：gitFile.modified=`#6897BB`、gitGutter 无 whitespaceOnly、sidebar.selected=`#37373D`、explorer 无 selected、errorBanner.BG=`#5A1D1D` | Read 比对 |
| 6 | SCH-02 | terminal 段恰 25 键（清单照 spec §4.3），值与 `src/panels/terminal/theme.ts` 现状 theme 段逐字一致——抽查 background/foreground/brightRed | Read 数键 + 比对 |
| 7 | SCH-02 | editor 段 theme 为 oneDark 直 import 透出（文件含 `from "@codemirror/theme-one-dark"`）；overrides 含 lint 7 键/searchMatch 4 键/background | Grep + Read 数键 |
| 8 | SCH-02 | libraries 段 dockview 恰 20 条映射 + allotment 恰 2 键（照 spec §4.5） | Read 数键 |
| 9 | SCH-02 | 文件头注释交叉引用 fail-safe 三处：`index.html`、`tauri.conf.json`、`main.tsx` 均出现 | Grep 三词 |
| 10 | SCH-03 | `src/theme/schemes/index.ts` 存在且含 `register(darcula)` 调用（side-effect 注册） | Grep |
| 11 | SCH-04 | `src/theme/schemeRegistry.ts` 存在，七方法齐全：`register`/`get`/`getActive`/`setActive`/`getAll`/`getDefaultId`/`_reset` | Grep 方法名 |
| 12 | SCH-04 | setActive 未知 id → 回退 darcula + `console.warn`——语义式：Read 确认存在回退分支与 warn 调用，非仅类型约束 | Read |
| 13 | SCH-05 | `src/theme/overrides.ts` 存在，四导出签名合 stages.md C2：`dockviewVarStyle()`/`allotmentVarStyle()` 返回 `Record<string, string>`；`editorTheme` 为 Extension；`editorColorOverrides()` 返回 Extension | Read |
| 14 | 中间态 | 本 Stage 既有文件零改动：`git diff --name-only HEAD` 产出仅含 5 个新文件（untracked）+ `docs/color-plan/`；`src/theme/colors.ts`、`src/theme/index.ts` 不在 diff 中 | git diff / git status |
| 15 | 中间态 | `src/theme/colors.ts` 仍为旧态 32 导出（facade 未切换——Stage 02 才做） | Read / Grep `DROPDOWN_BG` 仍在 |

## 全量测试（全部通过为门禁）

- `npx tsc --noEmit` → exit 0
- `npx eslint src/` → exit 0
