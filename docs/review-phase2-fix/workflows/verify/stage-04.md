# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-16**：grep `src-tauri/Cargo.toml` tokio 行 features 含 `"sync"`
- **SEC-16**：grep `src-tauri/src/state.rs` 含 `project_root_lock: tokio::sync::Mutex<()>`（AppState 字段与 `new()` 初始化各一处）
- **SEC-16**：`set_project_root_impl` 全程持锁（Read 确认：`lock.lock().await` 在 canonicalize/spawn_blocking 之前获取，guard 存活至函数尾）
- **SEC-16**：命令层 `set_project_root` 调用传入 `&state.project_root_lock`（Read 确认）
- **SEC-16**：`src-tauri/src/state.rs` 含新增用例 `set_project_root_serializes_concurrent_calls`（grep 命中），且全量 L1 测试通过
- **SEC-16**：`src-tauri/src/CLAUDE.md` state.rs 节含 project_root_lock 字段与 SEC-16 串行化语义（grep 命中）

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
