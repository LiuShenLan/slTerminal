# Stage 09 逐项验证断言

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-TE-18**：`e2e-tests/test.e2e.ts` 新增 L4 用例；用例通过 `browser.execute` 或 helper 打开 hooksConfig 面板；在 JSON 模式写入合法 hooks 配置；点击保存按钮；断言目标 settings.json 的 mtime 更新且 JSON 内容正确。
- **P3-TE-18b**：未修改任何 `src/` 或 `src-tauri/` 生产代码（仅修改 `e2e-tests/`）。

## 全量测试

1. `npm run build:e2e`
2. `npm run wdio`
