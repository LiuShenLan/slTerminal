# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-01**：`src-tauri/src/pty/shell.rs` 的 `validate_shell_allowlist` 对**含路径分隔符**的输入做真实路径一致性校验（canonicalize 后与 PATH 解析结果比对——语义式，须 Read 代码确认逻辑存在，不限变量名）；**纯文件名**输入维持 `file_name` 比对原逻辑（Read 确认未改坏）
- **SEC-01**：新增 L1 用例存在——伪造绝对路径（如系统目录外同名 cmd.exe）被拒绝、PATH 解析出的合法绝对路径被放行（grep 测试函数名/断言）
- **BE-01**：`src-tauri/src/pty/spawn.rs` 存在 `MAX_PTY_SESSIONS` 常量且值为 32
- **BE-01**：`pty_spawn` 的会话数超限检查位于 `SPAWN_LOCK` 持锁区间内（语义式——须 Read 确认检查在锁内，防并发超发）；超限返回 Err（Validation 类）
- **BE-01**：新增 L1 用例存在（上限判定/满员拒绝）
- **SEC-02**：`grep "fs::metadata(" src-tauri/src/hooks/signal.rs src-tauri/src/hooks/watcher.rs` 零命中（已全部改 `symlink_metadata`）
- **SEC-02**：`process_signal_file_with` 与 `collect_signal_files` 对 symlink 文件仅删除不读取（语义式，须 Read 确认无 read_to_string 于 symlink 分支）
- **SEC-02**：新增 L1 用例存在；Windows symlink 特权失败的测试含 skip 与注释说明（Read 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
