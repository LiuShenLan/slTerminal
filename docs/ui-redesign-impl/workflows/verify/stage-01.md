# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **TH-01**：Read `src/theme/schemes/types.ts` 确认——EditorScheme overrides 含 `syntax` 子组恰 9 键（property/string/number/keyword/function/type/operator/punctuation/comment）+ `plainText`/`lineNumber`/`lineNumberActive`；UiTokens 含 `accentFg`/`selectionHoverBg`/`titlebarBg`；新键带 JSDoc 消费注释
- **TH-02**：Read `src/theme/schemes/linear.ts` 抽点断言：`panelBg:"#0a0a0b"`、`sidebarFg:"#ece9e4"`、`titlebarBg:"#141416"`、`terminal.background:"#0a0a0b"`、`terminal.red:"#d9706b"`、`terminal.brightBlack:"#7d7871"`、`editor.overrides.syntax.keyword:"#b48ce0"`、`editor.overrides.plainText:"#b3aea6"`、`libraries.dockview["--dv-group-view-background-color"]:"#0a0a0b"`、`libraries.allotment.focusBorder:"#6e9ff2"`；对象标注 `: ColorScheme`；id 为 linear
- **TH-03**：`src/theme/schemes/index.ts` 注册 linear（grep `register(linear)` 命中）；Glob `src/theme/schemes/darcula.ts` 不存在
- **TH-04**：grep `DEFAULT_SCHEME_ID = "linear"` 命中 `src/theme/schemeRegistry.ts`
- **TH-05**：grep `"linear"` 命中 `src/main.tsx`（默认 schemeId 赋值处）；grep `darcula` 在 `src/main.tsx` 零命中
- **TH-06**：grep `#0a0a0b` 命中 `index.html` 与 `src-tauri/tauri.conf.json`；Read `src/main.tsx` 错误页段确认 background `#0a0a0b`、外层 color `#ece9e4`、错误消息 span color `#d9706b`；grep `#1e1f22` 在上述三文件零命中
- **TH-07**：grep `editorSyntaxHighlight` 命中 `src/theme/overrides.ts`（导出函数）；grep `syntaxHighlighting` 同文件命中；Read 确认 editorColorOverrides 新增规则全部带 `&.cm-editor` 前缀（不限规则写法，须 Read 确认选择器形态）
- **TH-08**：grep `editorSyntaxHighlight` 命中全部 5 文件（`src/panels/editor/useCodeMirror.ts`、`src/panels/gitshow/GitShowPanel.tsx`、`src/panels/diff/DiffPanel.tsx`、`src/panels/hooksConfig/JsonMode.tsx`——diff 文件 2 处）；Read 每处确认在扩展数组中位于 `editorTheme` **之前**
- **TH-09**：grep `ACCENT_FG`、`SELECTION_HOVER_BG`、`TITLEBAR_BG` 均命中 `src/theme/colors.ts`；grep `darcula` 在 `src/theme/index.ts` 零命中
- **TH-10**：`grep -ri "darcula" src/ test/ --include=*.ts --include=*.tsx` 零命中（代码与测试无 darcula 残留；CLAUDE.md 文档字样归 Stage 09 清理，不在本 Stage 中间态内）；`npm test` 通过（依测试 agent 结果）
- **TH-11**：`npm run test:l3` 通过（依测试 agent 结果）；grep `#d9706b` 命中 `test/terminal/theme-options.test.ts`

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
7. `npx vite build`
