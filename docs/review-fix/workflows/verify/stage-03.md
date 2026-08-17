# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **中间态注意**：本 Stage 后命令数 = **32**（33 - pty_reattach），计数断言按 32 判定。

## 断言清单

- **SEC-03**：`grep -ri reattach src/ src-tauri/src/ src-tauri/tests/ e2e-tests/` 零命中；残留命中若为 ring buffer/E1 机制注释，逐处 Read 确认性质（代码符号残留判 not_fixed）
- **SEC-03**：E1 机制保留——`src-tauri/src/state.rs` 的 `ring_buffer_append` 与 ring/channel 设施仍存在（语义式：只删命令入口，未误删重连机制）
- **SEC-07**：`src-tauri/src/lib.rs` 的 `generate_handler!` 恰 32 条命令（逐条计数，对照脚本头契约清单：ping, get_windows_build_number, set_project_root, pty_spawn, pty_write, pty_resize, pty_kill, fs_read_file, fs_write_file, fs_read_dir, fs_create_dir, fs_delete, fs_rename, save_settings, load_settings, save_projects, load_projects, git_status, git_diff, git_file_at_head, git_rollback, git_unstage, notify_watch, agent_hooks_inject, agent_hooks_uninstall, agent_hooks_injection_status, agent_hooks_restore_statusline, agent_hooks_config_read, agent_hooks_config_write, agent_history_scan, agent_history_delete, agent_history_read_title）
- **SEC-07**：`src-tauri/capabilities/default.json` 恰 32 条 `allow-<cmd>` 自定义命令权限（逐条与 lib.rs 清单一一对应）；既有插件权限保留未删；`_p0-07-note` 不存在（grep 零命中）
- **SEC-07**：`src-tauri/build.rs` 含 `AppManifest::new().commands(`（grep 命中）
- **SEC-07**：`src-tauri/gen/schemas/` 产物中 32 条 `allow-` 权限名与 capabilities 所写一致（Read gen 产物核对；若 gen 目录因环境未生成，判 partial 并注明「以构建期产物为准」，不判 not_fixed）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
