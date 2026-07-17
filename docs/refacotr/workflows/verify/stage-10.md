# Stage 10 逐项验证断言（唯一真值源）

> stage-10-test-quality.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 只修改测试与测试基建——生产代码应零改动。

## 断言清单

- **TE-04**：`grep "thread::sleep" src-tauri/tests/pty_integration_tests.rs` 仅剩轮询循环内的短 sleep（无固定时长盲等）；marker 轮询上限 10s
- **TE-08**：`pool.rs` 测试中 `make_test_watcher` 线程真实监听 `stop_rx`；LRU 淘汰/替换用例断言 `is_running() === false`
- **TE-09**：`use-xterm-output.test.ts` 无 `if (writeCalls.length > 0)` 条件断言（先断言 length > 0 再断言内容）
- **TE-10**：抽查 `close-handler`/`editor-confirm`/`e2e-gating`/`explorer-*` 测试文件中的 `waitFor` 调用，均带显式 `{ timeout: ... }`（2000~5000）
- **TE-11**：`use-xterm-*.test.ts`、`use-file-tree.test.ts`、`explorer-notify`/`html-panel.test.tsx` 中 debounce/定时器用例统一 `vi.useFakeTimers()` + `advanceTimersByTime`（无真实 setTimeout 等待）
- **TE-12**：`setup.ts` 含 notify mock 需显式覆盖的警告注释；canvas spy 用 beforeAll/afterAll 包装
- **守卫**：`compute_conpty_flags` 的 4 条测试原样未动（`git diff` 不涉及）
- **生产代码零改动**：`git status --porcelain src/ src-tauri/src/` 中非测试文件无变更

## 全量测试（6 条全部通过）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
