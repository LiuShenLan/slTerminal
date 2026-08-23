# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 覆盖率类断言数据取自门禁命令 `npm run test:coverage` 产出（verify agent 不重跑，据测试 agent 报告判定）。

## 断言清单

- **TQ-COV-07**：终端三 hook 错误分支新用例存在（grep 新用例名/关键字：`重试耗尽`、`WRITE_FAIL_TOAST_THRESHOLD`、`PTY resize 失败` 等命中对应测试文件）；v8 复测 `useTerminalInstance.ts`/`useXterm.ts`/`usePtyResize.ts` 函数覆盖均 ≥ 85%（或残余分支在测试文件注释说明）。
- **TQ-COV-08**：`workspace-page-dockview.test.tsx` 含针对 PageDockviewHost 未覆盖分支的新用例（Read 确认新增 describe/it）；v8 复测 `PageDockviewHost.tsx` 行覆盖 ≥ 90%。07 报告误写 `DockviewHost.tsx` 已翻案——断言对象始终是 `PageDockviewHost.tsx`（语义式：不以不存在文件名为据判 not_fixed）。
- **TQ-COV-09**：NavPageRow 交互新用例存在（重命名 Enter/Escape、chevron stopPropagation 等，grep 命中）；ExplorerPanel 错误横幅/加载占位新用例存在（grep `explorer-load-error`/`explorer-error-banner` 命中测试文件）；v8 复测 NavPageRow 行 ≥ 85%、ExplorerPanel ≥ 88%。
- **TQ-COV-10**：`src/__tests__/ipc-window.test.ts` 存在且三 wrapper reject 用例齐全（Glob + grep `minimize`/`toggleMaximize`/`close` 命中）；sideViewDefs 守卫用例断言注册 id 集合精确为 `["nav", "explorer", "commit"]`（grep 命中）；v8 复测 sideViewDefs 函数 100%、ipc/window 行 ≥ 90%。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:coverage`（覆盖率断言数据源）
