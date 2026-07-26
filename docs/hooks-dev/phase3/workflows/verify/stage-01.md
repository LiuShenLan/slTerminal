# Stage 01 逐项验证断言

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **P3-BE-01**：`src-tauri/src/hooks/config.rs` 存在；文件内实现 user/project/local 三层路径解析函数；user 层使用 `dirs::home_dir()`；project/local 层路径基于 `project_path` 拼接 `.claude/settings.json` / `.claude/settings.local.json`。
- **P3-BE-02**：`src-tauri/src/hooks/config.rs` 内实现 `hooks_config_read`，文件不存在/损坏返回 `Ok(Value::Null)`；非法 `layer` 返回 `AppError::Validation`。
- **P3-BE-03**：`src-tauri/src/hooks/config.rs` 内实现 `hooks_config_write`；`content` 非 Object 时返回 Validation 错误；使用 `NamedTempFile` + `persist` 原子写；无 `.bak` 逻辑。
- **P3-BE-04**：`src-tauri/src/hooks/mod.rs` 包含 `pub mod config;`，并在命令注册列表中导出 `config::hooks_config_read` / `config::hooks_config_write`。
- **P3-BE-05**：`src-tauri/src/lib.rs` 的 `generate_handler!` 宏包含 `hooks_config_read` 与 `hooks_config_write`。
- **P3-BE-06**：user 层命令实现中不调用 `validate_path_within_root`；路径解析使用 `dirs::home_dir()`。
- **P3-BE-07**：project/local 层命令实现中在拼接路径前调用 `validate_path_within_root`。
- **P3-BE-08**：非法参数走 `AppError::Validation`；IO 错误走 `AppError::Io`/`IoKind`。
- **P3-TE-01**：`src-tauri/src/hooks/config.rs` 底部存在 `#[cfg(test)] mod tests`；包含 user 层读取/写入/父目录创建/Null 降级用例。
- **P3-TE-02**：同一测试模块包含 project/local 路径解析与沙箱失败分支用例。

## 全量测试

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml hooks_config -- --test-threads=1`

*注：filter 使用命令名前缀 `hooks_config`；执行期若测试模块/函数命名不同，以实际命中为准，但不得跳过 L1 测试。*
