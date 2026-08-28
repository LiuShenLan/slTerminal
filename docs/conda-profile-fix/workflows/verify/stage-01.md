# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **B17-FIX-1**：`grep -n '"-NoProfile"' src-tauri/src/pty/shell.rs` 零命中——`cmd.arg("-NoProfile")` 与 `"-NoProfile".to_string()` 均不存在（注释中的禁止说明不含带引号形态，若有命中须 Read 确认该行为注释且不带引号字面量）
- **B17-FIX-2**（语义式）：Read `build_pwsh_command` 与 `build_pwsh_info` 函数体——两函数产出的参数序列仅含 `-NoLogo`、`-NoExit`、`-EncodedCommand` + base64 脚本四项，不得含任何 profile 抑制参数（不限拼写：`-NoProfile` 及 `-nop` 等 PowerShell 前缀缩写均属违规）
- **B17-FIX-3**：shell.rs 模块头注释（L1-7 区间）、`resolve_shell` 文档注释、`build_pwsh_command`/`build_pwsh_info` 文档注释中不再出现「自动加入 -NoProfile」式旧口径描述，且含 B17 禁止说明（Read 确认，文档描述须与函数体真实参数一致，不撒谎）
- **TE-B17-1**：shell.rs `#[cfg(test)] mod tests` 内存在 `test_pwsh_args_no_noprofile_b17` 用例（`grep -n 'test_pwsh_args_no_noprofile_b17' src-tauri/src/pty/shell.rs` 命中），且用例断言两条构建路径（resolve_shell_info / resolve_shell）args 不含 `-NoProfile`（Read 确认断言语义，非仅函数名存在）
- **TE-B17-2**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿且输出含 `test_pwsh_args_no_noprofile_b17 ... ok`（依测试 agent 报告判定）
- **B17-FIX-4**：`e2e-tests/terminal.e2e.ts:412` 附近的 `-NoProfile` 保持原样未被改动（Read 确认该行仍存在——本项为「不动」决策的防误改守卫）

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
