# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> cargo 系断言数据取自全量测试 agent 的单点执行产出（verify agent 不重跑）。

## 断言清单

- **TQ-COV-01**：`src-tauri/src/lib.rs` 含 `pub fn install_panic_hook` 与 `write_crash_log`（grep 命中）；`src-tauri/src/main.rs` 仅调用 `slterminal_lib::install_panic_hook()` 且不再含内联 `set_hook`（grep -c `set_hook` main.rs 为 0）；`write_crash_log` 两个 L1 用例存在且通过（cargo test 产出 + grep 用例名命中 lib.rs）。
- **TQ-COV-03**：`src-tauri/src/pty/spawn.rs` 测试模块含 `join_with_timeout` 两用例（finished→true / blocked 短 timeout→false，grep 命中）；`src-tauri/src/pty/CLAUDE.md` 含容量超限 kill 清理等残余豁免登记（grep 命中）。
- **TQ-COV-04**：`src-tauri/src/hooks/signal.rs` 新增超限/读失败/emit 失败仍删除 3 例（grep 用例名命中）；`src-tauri/src/hooks/watcher.rs` 新增目录重建/停止信号 2 例（grep 命中）；全部通过（cargo test 产出）。
- **TQ-COV-05**：`src-tauri/Cargo.toml` 含 `[dev-dependencies]` 段与 `tracing-test`（grep 命中）；`src-tauri/src/hooks/claude/config.rs` 含 `traced_test` 标注的审计日志两用例（grep `logs_contain` 命中）。
- **TQ-COV-06**：`src-tauri/src/git/mod.rs` 函数覆盖 ≥ 80%（执行期 llvm-cov 产出为据）或残余未覆盖函数逐条登记于 `src-tauri/src/git/CLAUDE.md`（grep 登记句命中）；若有死函数删除，clippy 无 dead_code 告警（clippy 产出）。
- **TQ-L1-01**：`src-tauri/src/settings.rs` 的 `run_save_with_retry` 文档注释含「容忍度声明」（grep 命中）。
- **TQ-L1-03**：`src-tauri/tests/pty_integration_tests.rs` 的 SPAWN_LOCK 上方注释含「锁边界声明」（grep 命中）。
- **TQ-L1-05**：`src-tauri/tests/pty_integration_tests.rs` 前 3 行含 `#![cfg(windows)]`（Read 确认）。

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（单点执行，cargo 排队属正常）
