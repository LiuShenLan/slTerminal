# Stage 7 逐项验证断言（唯一真值源）

> stage-07-frontend-mid.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-03**：`Workspace.tsx` 组件函数体无 `pageCallbacksRef.current.delete/set`（已移入 useEffect）
- **FE-04**：`PageDockviewHost.tsx` 有 disposable 收集数组 + 卸载/重触发统一 dispose
- **FE-05**：`main.tsx` 的 setInterval 有最大轮次 + clearInterval + 超时错误提示
- **FE-06**：`useFileTree.ts` 的 `refreshExpanded` 中 `gitStatus` 回调有 gen 比对
- **FE-07**：`ExplorerPanel.tsx` 的 `addPanel` 有 try-catch，成功后才注册 titleManager
- **FE-09**：`TerminalPanel.tsx` 无 `state.icon!`
- **FE-10**：`layoutSerde.ts` 的 `fromJSON` 无 `as any`
- **FE-16**：`ipc/notify.ts` 与 `types/notify.ts` 的 `detail` 可选性一致
- **FE-18**：`src/global.d.ts` 存在且 `main.tsx`/`ErrorBoundary.tsx` 无 `(window as any)`
- **FE-22**：`PageDockviewHost.tsx` 无 `panel.view?.contentComponent`
- **DOC-10**：`layoutSerde.ts` 无"创建默认终端"注释残留

## 全量测试（5 条全部通过）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
