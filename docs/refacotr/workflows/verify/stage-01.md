# Stage 1 逐项验证断言（唯一真值源）

> stage-01-sandbox.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-01a**：`set_project_root` 出现在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中；`src-tauri/capabilities/default.json` 含该命令
- **SEC-01b**：`src-tauri/src/fs/mod.rs` 的 `fs_read_dir` 已接收 `State<AppState>` 参数并调用 `validate_path_within_root`
- **SEC-01c**：`state.rs` 的 `validate_path_within_root` 中相对路径先以 project_root 为基准 join 再 `dunce::canonicalize`；目标不存在时返回 Err；不存在 `None => return Ok(())` 跳过逻辑（`#[cfg(test)]` 豁免除外）
- **SEC-01d**：`src/stores/projects.ts` 与 `e2e-tests/helpers.ts` 均调用了 `setProjectRoot`（经 `src/ipc/fs.ts` wrapper）
- **SEC-06**：`src-tauri/capabilities/default.json` 不含 `wdio-webdriver`
- **DOC-05c**：`state.rs` 的 `job_object` 字段无 `#[cfg(windows)]` 条件初始化残留；非 Windows 路径用 `JobHandle::new_dummy()` 无条件初始化

## 全量测试（5 条全部通过）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
