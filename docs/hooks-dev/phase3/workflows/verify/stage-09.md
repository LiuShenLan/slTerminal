# Stage 09 逐项验证断言

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-TE-18**：`e2e-tests/test.e2e.ts` 新增 L4 用例；用例经 `__slterm_e2e_createProject` 创建 tempdir 项目 → 打开 hooksConfig 面板 → 切到 **project 层** → JSON 模式写入合法 hooks 配置 → `.click()` 保存 → 断言 `<tempdir>/.claude/settings.json` mtime 更新 + hooks 内容正确 + **预置的其他字段（如 `permissions`）merge 后原样保留**（三项断言齐全）。
- **P3-TE-18 安全约束**：用例断言目标文件为 tempdir 项目内 `.claude/settings.json`（语义式：Read 用例代码确认不出现 home 目录 user 层路径、不调用 user 层写入）；未修改任何 `src/` 或 `src-tauri/` 生产代码（仅 `e2e-tests/`）。
- **P3-TE-18 门禁补充**：若本 Stage 改动了 `e2e-tests/helpers.ts`，`npx vite build` 已执行且通过（该文件不在根 tsconfig include）。

## 全量测试

1. `npm run build:e2e`
2. `npm run wdio`
3. （条件）`npx vite build`——仅当 `e2e-tests/helpers.ts` 有改动
