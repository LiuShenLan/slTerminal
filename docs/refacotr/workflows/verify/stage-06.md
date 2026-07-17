# Stage 6 逐项验证断言（唯一真值源）

> stage-06-terminal-hooks.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-23**：`useXterm.ts` 中不存在**任何**本地 sessionId 存储（不限变量名——`sessionIdRef`/`sessionRef`/`sessionId` 变量等均属违规，防"改名迎合"）；`pty.write`/`pty.resize` 处全部经 `TerminalRegistry.get(panelId)` 取 sessionId（须 Read 代码确认，非仅 grep）
- **FE-08**：`useXterm` cleanup 含 `retryDisposableRef.current?.dispose()`
- **TE-16**：`usePtyOutput` 直写路径有 `visibleRef` 门控；NF3 测试标题与断言一致
- **FE-17**：`TerminalRegistry.getAll` 返回 `new Map(registry)` 副本
- **FE-14**：`fonts.ready` 有取消标志 + rAF 回调双重检查
- **FE-15**：`performDispose()` 提取共用，无重复 dispose 代码块
- **FE-12**：`src/panels/terminal/useFontSizeBridge.ts` 文件不存在；`grep -rn "useFontSizeBridge" src/` 无 import 残留

## 全量测试（5 条全部通过；use-xterm 系列 108+ 条全绿为本 Stage 门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
