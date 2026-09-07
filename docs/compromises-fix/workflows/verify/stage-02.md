# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 回退预案：若 `cargo test --lib` 仍 0xc0000139，本 Stage 不得提交——恢复 [lib] test=false + [[test]] 重组与红线，CP-040 转登记（checklist CP-040 步骤 3.7）。

## 断言清单

- **CP-040-a（embed 生效）**：`cargo test --lib -- --test-threads=1` 退出码 0（默认 lib target + --lib 形态，原 0xc0000139 通道）；`cargo test validate_spawn_request -- --test-threads=1` 退出码 0（带 filter 定向形态）。
- **CP-040-b（计数等价）**：`cargo test -- --test-threads=1` 全量退出码 0，且用例计数 N2 == N1（N1 = 改造前实测基线，登记于 commit body；起草期参考值 827）。
- **CP-040-c（结构拆除）**：`grep -n "test = false\|\[\[test\]\]" src-tauri/Cargo.toml` 零命中；`src-tauri/src/lib.rs` 含 `embed_manifest::embed_manifest_file!` 且以 `#[cfg(all(windows, test))]` 门控（Read 确认）；`src-tauri/Cargo.toml` dev-dependencies 含 `embed-manifest`；`src-tauri/build.rs` 的 `rustc-link-arg-tests` 两行保留（Read 确认）。
- **CP-040-d（红线拆除）**：`grep -n "lib_tests" .claude/CLAUDE.md` 零命中；`grep -c "lib_tests" .claude/test-exemptions.md` = 0；`.claude/test-exemptions.md` 含 TQ-COV-06 销记句（Read 确认）；`grep -n "TQ-COV-06" .claude/CLAUDE.md` 零命中。
- **CP-040-e（产物无冲突）**：`npx tauri build --debug --no-bundle` 成功；产物启动正常（人工一次：窗口正常出现）。

## 全量测试（全部通过为门禁）

1. `cargo test --lib -- --test-threads=1`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npx tauri build --debug --no-bundle`
