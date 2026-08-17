# Stage 14 逐项验证断言（唯一真值源）

> stage-14 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-35**：`src/features/index.ts`、`src/panels/index.ts`、`src/features/agentHistory/index.ts`、`src/features/commit/index.ts` 四文件不存在（Glob 确认）；`grep -rn "from.*features/index\|from.*panels/index\|agentHistory/index\|commit/index" src/ e2e-tests/` 零命中（相对路径 import 形态逐一排除）
- **FE-35**：`PANEL_GIT_SHOW`、`PANEL_DIFF`、`PANEL_HOOKS_CONFIG`、`terminalTabConfig` 处置与 grep 证据一致（语义式：有消费者改常量引用保留、零消费者已删——逐常量 grep 现状核对 agent 报告证据）
- **FE-35**：`src/workspace/index.ts`、`src/panels/terminal/index.ts` 残留 re-export 均有消费（抽查 3 条 grep 消费方）；`src/ipc/index.ts` 的 `ping()` 保留且注释含「测试专用」（Read 确认）；`src/ipc/window.ts` 的 `setFocus` 已删（grep 零命中含消费方）
- **BE-20**：`grep "allow(dead_code)" src-tauri/src/hooks/signal.rs` 零命中；clippy 全量零警告（门禁命令 3 佐证）
- **BE-17**：`grep "#\[cfg(windows)\]" src-tauri/src/lib.rs src-tauri/src/fs/mod.rs src-tauri/src/settings.rs src-tauri/src/agent_history/claude/ops.rs`——命中仅豁免残留且逐处有注释标注豁免理由（Read 逐处确认）；其余已改 `cfg!(windows)` 运行时分支（grep `cfg!(windows)` 命中对照）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
