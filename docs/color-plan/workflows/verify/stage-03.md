# Stage 03 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 后消费点已迁移；`main.tsx`/`App.tsx`/`App.css` **未动**（Stage 04 才做）。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | CON-01~04 | `useCodeMirror.ts`/`GitShowPanel.tsx`/`DiffPanel.tsx`/`JsonMode.tsx` 四文件均含 `editorColorOverrides` 引用（经 theme barrel import） | Grep 四文件 |
| 2 | CON-01~04 | 全仓 `src/` grep `@codemirror/theme-one-dark` 仅 `src/theme/schemes/darcula.ts` 一处命中 | Grep |
| 3 | CON-04 | `JsonMode.tsx` 事件导航 hover 色引用 `ON_ACCENT_FG` token——语义式：Read :213 附近确认 style.color 赋值为 token 引用而非任何色值字面量（不限实现写法）；全文件无 `#FFFFFF`/`#fff` 字面量 | Read + Grep |
| 4 | CON-05 | `PageDockviewHost.tsx` 含 `dockviewVarStyle` 引用且根 div style 展开其返回值（className `dockview-theme-dark` 保留） | Grep + Read |
| 5 | CON-06 | `Workspace.tsx` 根容器 style 合并 `allotmentVarStyle()` 返回值 | Grep + Read |
| 6 | CON-06 | `SideBarArea.tsx` 零改动（CSS 变量继承覆盖，不改本体） | git diff 不含该文件 |
| 7 | FAC-03 | `src/panels/terminal/theme.ts` theme 段无 25 键色值字面量残留——语义式：Read 确认 `theme` 字段值来自 `getActive().terminal` 展开，非逐键字面量 | Read |
| 8 | FAC-03 | terminal/theme.ts 非色选项原位保留：`drawBoldTextInBrightColors`、`vtExtensions`、`scrollback` 均仍存在于该文件 | Grep 三词 |
| 9 | 零改动声明 | `theme.test.ts`/`gitshow-panel.test.tsx`/`hooks-config-jsonmode.test.tsx`/`test/terminal/theme-options.test.ts` 零改动且测试绿（失效即值漂移，属失败项） | git diff + 测试结果 |
| 10 | 中间态 | 本 Stage diff 仅含分工表七文件：useCodeMirror/GitShowPanel/DiffPanel/JsonMode/PageDockviewHost/Workspace/panels-terminal-theme | git diff --name-only HEAD |

## 全量测试（全部通过为门禁）

- `npx tsc --noEmit` → exit 0
- `npx eslint src/` → exit 0
- `npm test` → exit 0
- `npm run test:l3` → exit 0（重点：`theme-options.test.ts` 5 例绿）
