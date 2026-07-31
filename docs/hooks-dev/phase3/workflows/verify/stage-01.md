# Stage 01 逐项验证断言

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **P3-BE-01**：`src-tauri/src/hooks/config.rs` 存在；文件内实现 user/project/local 三层路径解析函数；user 层使用 `dirs::home_dir()`；project/local 层路径基于 `project_path` 拼接 `.claude/settings.json` / `.claude/settings.local.json`；父目录不存在时 `create_dir_all`。
- **P3-BE-02**：`hooks_config_read(layer, project_path)` 实现；**返回 `hooks` 子树而非整文件**（语义式：Read 代码确认提取的是根对象的 `hooks` 键）；文件不存在/无 hooks 键返回 `Ok(Value::Null)`；**JSON 损坏返回 `Err` 而非 Null**（语义式：确认损坏路径返回 Err）；非法 `layer` 返回 `AppError::Validation`。
- **P3-BE-03**：`hooks_config_write(layer, hooks, project_path)` 实现；`hooks` 非 Object 返回 Validation；**read-modify-write merge**——读原文件（不存在视为 `{}`）→ 替换根对象 `hooks` 键 → 写回，原样保留 `permissions`/`env` 等其他字段（语义式：确认不是整文件覆写）；原文件损坏 → `Err` 拒绝；`NamedTempFile` + `persist` 原子写；无 `.bak` 逻辑；IO 在 `spawn_blocking` 内。
- **P3-BE-04**：`src-tauri/src/hooks/mod.rs` 包含 `pub mod config;`，并导出 `config::hooks_config_read` / `config::hooks_config_write`。
- **P3-BE-05**：`src-tauri/src/lib.rs` 的 `generate_handler!` 宏包含 `hooks_config_read` 与 `hooks_config_write`。
- **P3-BE-06**：user 层命令实现中不调用 `validate_path_within_root`；路径解析使用 `dirs::home_dir()`。
- **P3-BE-07**：project/local 层命令实现中在拼接路径前调用 `validate_path_within_root`；缺失 `project_path` 返回 Validation；校验失败返回 `AppError::PathNotAllowed`。
- **P3-BE-08**：非法参数（layer/hooks/JSON 损坏）走 `AppError::Validation`；IO 错误走 `AppError::Io`/`IoKind`。
- **P3-TE-01**：`config.rs` 底部存在 `#[cfg(test)] mod tests`；包含 user 层：文件不存在 Null、无 hooks 键 Null、合法子树读取、原子写内容正确、父目录创建用例。
- **P3-TE-02**：同一测试模块包含：merge 保留断言（预置 `permissions`/`env` 写入后原样保留）、损坏 JSON read/write 均 Err、project/local 路径解析、沙箱失败分支、非 Object hooks 拒绝用例。

## 全量测试

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml hooks::config -- --test-threads=1`

*注：filter 使用模块路径 `hooks::config`（命中 `hooks::config::tests::*`）；若测试模块命名不同，以实际命中为准，但不得跳过 L1 测试。*
