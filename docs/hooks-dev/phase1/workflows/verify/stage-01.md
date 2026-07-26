# Stage 01 Verify：后端 hooks 模块骨架 + 信号 watcher

> 断言与 Stage 01 完成后的真实中间态一致。

## 文件存在性

- [ ] `P1-BE-01` `src-tauri/src/hooks/mod.rs` 存在且被 `src-tauri/src/lib.rs` 引用。
- [ ] `P1-BE-02` `src-tauri/src/hooks/signal.rs` 存在。
- [ ] `P1-BE-03` `src-tauri/src/hooks/watcher.rs` 存在。
- [ ] `P1-BE-04` `src-tauri/src/hooks/inject.rs` 存在。
- [ ] `P1-BE-05` `src-tauri/assets/slterm-hook-reporter.js` 存在。

## DTO 与命令注册

- [ ] `P1-BE-01` `HookEventPayload` 含 8 个字段：`panelId`、`event`、`timestamp`、`sessionId`、`transcriptPath`、`cwd`、`toolName`、`notificationType`。
- [ ] `P1-BE-01` `HookInjectionStatus` 含 `status` 与 `version`；`status` 枚举值序列化为 `"injected"` / `"notInjected"` / `"outdated"`。
- [ ] `P1-BE-06` `src-tauri/src/lib.rs` 的 `generate_handler!` 宏含 `hooks_inject`、`hooks_uninstall`、`hooks_injection_status`。
- [ ] `P1-BE-07` `src-tauri/src/lib.rs` 的 `.setup()` 闭包调用 `hooks::start_signal_watcher`。

## 信号处理

- [ ] `P1-BE-02` `parse_signal_file` 对合法 JSON 返回 `Some(HookEventPayload)`。
- [ ] `P1-BE-02` `parse_signal_file` 对缺 `panelId` 的 JSON 返回 `None`。
- [ ] `P1-BE-02` `parse_signal_file` 对非法 JSON 返回 `None`。
- [ ] `P1-BE-03` watcher 监听 `~/.slterminal/hooks-events/`（`dirs::home_dir()` 解析）。
- [ ] `P1-BE-03` watcher 线程名为 `hook-signal-watcher`。
- [ ] `P1-BE-03` Create/Modify 事件触发后，调用 `app_handle.emit("hook-event", payload)`。
- [ ] `P1-BE-03` 解析失败/缺 panelId 时仅 warn，不 panic，仍尝试删除文件。

## 注入/卸载/状态

- [ ] `P1-BE-04` `hooks_inject` 原子写 `~/.slterminal/hooks/slterm-hook-reporter.js`。
- [ ] `P1-BE-04` `hooks_inject` 非法 JSON 时返回 `AppError` 且不改动 `~/.claude/settings.json`。
- [ ] `P1-BE-04` `hooks_inject` 幂等：已存在 slTerminal 段时替换，不产生重复 matcher 组。
- [ ] `P1-BE-04` `hooks_uninstall` 移除全部含 `slterm-hook-reporter` 子串的 matcher 组。
- [ ] `P1-BE-04` `hooks_uninstall` 删除脚本目录并清空信号目录。
- [ ] `P1-BE-04` `hooks_injection_status` 能返回 `injected` / `outdated` / `notInjected`。
- [ ] `P1-BE-05` 脚本任何异常路径均以 `process.exit(0)` 结束，不向 stderr 输出。
- [ ] `P1-BE-05` 脚本缺失 `SLTERM_PANEL_ID` 时直接 `process.exit(0)`。

## 测试

- [ ] `P1-BE-08` `cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1` 通过。
- [ ] `P1-BE-09` inject/uninstall/status 单元测试通过。

## 静态检查

- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过。
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` 通过。
