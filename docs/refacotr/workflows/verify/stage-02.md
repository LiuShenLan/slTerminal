# Stage 2 逐项验证断言（唯一真值源）

> stage-02-pty-async.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-01**：`spawn.rs` 中 `pty_spawn`/`pty_write`/`pty_resize`/`pty_reattach` 均为 `pub async fn`，且函数体内有 `spawn_blocking`
- **SEC-02**：`shell.rs` 存在 `validate_shell_allowlist`，`resolve_shell_info` 拒绝白名单外路径；`spawn.rs` 的 `pty_spawn` 对 shell 调该校验、对 cwd 调 `validate_path_within_root`
- **SEC-07**：`resolve_shell` 与 `resolve_shell_info` 共用同一白名单校验
- **SEC-08**：`state.rs` 的 `PtySession` 含 `panel_id` 字段；`pty_write`/`pty_resize`/`pty_kill` 有归属校验
- **BE-02**：`add_to_job_object` 失败路径有 `child.kill()`
- **BE-11**：`which_full_path` 使用 `std::env::split_paths`（无固定 `';'` 分割）
- **BE-12**：`SPAWN_LOCK` guard 作用域不含 `take_writer`/CPR 注入/`add_to_job_object`
- **BE-14**：`SpawnRequest` 有 `cols`/`rows` ≤ `i16::MAX` 校验（无 `as i16` 回绕）

## 全量测试（5 条全部通过，含 shell::tests 7+ 条、conpty_custom 13 条）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
