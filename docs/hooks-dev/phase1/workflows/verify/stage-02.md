# Stage 02 Verify：PTY 注入 SLTERM_PANEL_ID

> 断言与 Stage 02 完成后的真实中间态一致。

## 注入点

- [ ] `P1-PTY-01` `src-tauri/src/pty/spawn.rs` 中 `extra_envs` Vec 含 `("SLTERM_PANEL_ID", request.panel_id)`。
- [ ] `P1-PTY-02` `src-tauri/src/pty/spawn.rs` 非 Windows fallback 路径含 `cmd.env("SLTERM_PANEL_ID", request.panel_id)`。
- [ ] `P1-PTY-01/02` 两处注入不加 shell 类型判断。

## 测试

- [ ] `P1-PTY-03` 新增 PTY env 注入测试，测试名匹配 `pty_env_injects`。
- [ ] `P1-PTY-03` 测试验证子进程环境 `SLTERM_PANEL_ID` 值等于 `request.panel_id`。
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml pty_env_injects -- --test-threads=1` 通过。
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1` 通过（不回归）。

## 静态检查

- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过。
