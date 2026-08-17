# Stage 13 逐项验证断言（唯一真值源）

> stage-13 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **中间态注意**：本 Stage 后命令数 = **34**（S05 后 33 + pty_kill_all），计数断言按 34 判定。

## 断言清单

- **FE-22**：`src/panelRegistry.ts` 的 components 映射存在 ErrorBoundary 包裹（语义式：HOC 统一包裹——Read 确认单点改动而非逐面板散落）；新增 L2 用例存在（抛错面板不影响同页其他面板）
- **FE-28**：`src/App.tsx` 的 TitleBar、Workspace 容器、NotificationListener、ConfirmDialogHost、ToastHost 五处各有 ErrorBoundary（grep 逐处命中）
- **FE-23**：`useAgentStatus.ts` 初始扫描存在 generation 守卫（语义式：genRef 或等效——setRows 前检查过期；Read 确认）
- **FE-24**：`useXterm.ts` 的 readHistoryTitle 存在 isDisposedRef 守卫（语义式：卸载后忽略过期结果——Read 确认）
- **FE-25**：`useHooksConfig.ts` 的 setLayer async IIFE 有 try/catch；confirmDiscard 的 timeout id 存 ref 且 cleanup 有 clearTimeout（Read 确认）
- **FE-26**：`pageApis.ts` 的 `switchToPageAndFocus` 签名含 `signal?: AbortSignal`；轮询循环检查 aborted（Read 确认）；调用点传入 Controller 并在卸载/再次点击时 abort（grep 调用点逐处确认）
- **FE-27**：`restoreSession.ts` 的 `waitFor` 含 AbortSignal 参数且循环前检查 aborted；恢复编排共享一个 Controller，新恢复发起时 abort 旧的（语义式，Read 确认）
- **BE-07**：`src-tauri/src/notify/mod.rs` 存在单批 paths 阈值合并为 Rescan 的逻辑（语义式：超阈值不再逐路径 emit——Read 确认阈值常量）；新增 L1 用例存在
- **BE-08**：`pty_kill_all` 命令存在于 spawn.rs（返回 u32）；`lib.rs` `generate_handler!` 恰 34 条（逐条计数）；`capabilities/default.json` 含 `allow-pty_kill_all`；`src/ipc/pty.ts` 有 `ptyKillAll()` wrapper；`src/App.tsx` 关闭序列在 Registry kill 之后调用 ptyKillAll 兜底（Read 确认顺序）
- **BE-09**：`src-tauri/src/state.rs` 的 `git_repo_cache` 存在容量 8 的淘汰逻辑（语义式：LRU 或等效——Read 确认，无 lru 新依赖）；「目录切换时清除」失实注释已修正（grep 该措辞零命中或确认已改写）；新增 L1 用例存在（容量淘汰/命中复用）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
