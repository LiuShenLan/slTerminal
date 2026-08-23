# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TQ-E-03**：`e2e-tests/` 全目录不再含固定 `setTimeout(r, 350)`（grep -c 为 0）；`agent.e2e.ts`、`history.e2e.ts`、`mockcli.e2e.ts` 三处原 sleep 点均改为 `waitUntil` 条件等待（grep 命中 3 文件）。
- **TQ-E-04**：`e2e-tests/history.e2e.ts` 不再含 `.catch(() => null)`（grep -c 为 0）；createProject 结果含非空校验与失败 throw（grep 命中）。
- **TQ-E-05**：`e2e-tests/terminal.e2e.ts` 粘贴用例名含「按键链路豁免见 inventory」（grep 命中）；含系统剪贴板读回断言（grep `readText` 或 clipboard 相关调用命中）。
- **TQ-E-06**：`e2e-tests/run-wdio.cjs` 含 `restoreAll` 命名函数（grep 命中）；恢复失败逐条打印且 `process.exitCode = 1`（grep 命中）；原 exit 回调 5 段恢复/清理动作全部迁入 restoreAll（语义式：exit 钩子内不再有内联恢复逻辑，须 Read 确认）。
- **TQ-E-08**：`e2e-tests/helpers.ts` 含 `__slterm_e2e_resetSettings`（grep 命中）；`e2e-tests/wdio.conf.ts` beforeSuite 同时调用 resetProjects 与 resetSettings（grep 命中）；`src/global.d.ts` 含对应类型声明（grep 命中）。

## 全量测试（全部通过为门禁）

1. `npx vite build`（helpers.ts 不在根 tsconfig include——构建图兜底）
2. `npm run e2e`（= build:e2e + wdio 全量）

## 人工验证点（收尾实测，不在本 Stage 自动判定）

- TQ-E-06：占用 ~/.slterminal/hooks 后 `npm run wdio` → 输出失败清单 + 退出码非 0。
