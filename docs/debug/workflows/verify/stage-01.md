# Stage 1 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DBG-1**：`src/ipc/pty.ts` 三个 wrapper 签名与 payload 均含 `panelId`——grep `panelId` 命中 ≥6 处（3 签名 + 3 payload）；Read 确认 `invoke("pty_write", { sessionId, panelId, data: Array.from(data) })`、`invoke("pty_resize", { sessionId, panelId, cols, rows })`、`invoke("pty_kill", { sessionId, panelId })` 三个 payload 形态。
- **DBG-2**：`src/` 与 `e2e-tests/` 内**不存在任何**缺 `panelId` 的 `pty.write(`/`pty.resize(`/`pty.kill(` 生产调用（测试文件除外）——grep `pty\.(write|resize|kill)\(` 逐命中 Read 确认参数列表；8 处调用点（`useXterm.ts` writeToPty/E2E helper/onData/cleanup kill/字号 resize、`usePtyResize.ts` 两处、`App.tsx` 关窗 forEach）逐一确认。
- **DBG-2a**（语义式，防改名迎合）：`App.tsx` 关窗清理段中 kill 的 panelId 必须来自 `TerminalRegistry.getAll()` 返回 Map 的**键**（forEach 第二参数或等价迭代），不得是硬编码字符串或 entry 内不存在字段的 undefined（须 Read 代码确认）。
- **DBG-3**：`src/__tests__/ipc-contract.test.ts` 的 write/resize/kill 三条参数断言期望对象均含 `panelId` 键（grep `panelId` 在该文件命中 ≥3 处期望对象）；三条异常传播用例调用签名带 panelId 实参。
- **DBG-4**：该文件存在契约守卫 describe（或等价分组），对 `pty_write`/`pty_resize`/`pty_kill` 各断言一次 payload 键集合精确匹配（如 `Object.keys(args).sort()` 比对）——Read 确认三条断言存在且语义为"键恰好为……"而非仅"包含 panelId"。
- **DBG-CONTRACT**（意图核对）：`pty_spawn`/`pty_reattach` 签名与调用**未被改动**（git diff 确认本次改动不涉及这两个函数；后端 Rust 代码零改动——`git diff --stat src-tauri/` 为空）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
