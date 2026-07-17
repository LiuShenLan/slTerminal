# Stage 3 逐项验证断言（唯一真值源）

> stage-03-spawn-hardening.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-07**：`spawn.rs` 的 `try_wait`/`wait` 有 `WAIT_OBJECT_0` 显式比较，其余异常值（含 `WAIT_FAILED`）返回 `io::Error`
- **BE-08**：`clone_killer` 无 `.expect` panic 路径
- **BE-09**：`spawn.rs` 无锁中毒 `.expect(...)` panic 残留（`grep "poisoned"` 只出现在错误消息字符串中）
- **BE-10**：`build_env_block` 无 HashMap 迭代，用 `Vec` 保持顺序
- **BE-16**：`grep -c "SAFETY" src-tauri/src/pty/spawn.rs` ≥ 6
- **BE-13**：`reader.rs` 中本轮数据全为启动序列（返回 None）时不置 `startup_drained`，仅出现真实非启动输出后才置位
- **守卫**：`compute_conpty_flags` 的 4 条测试原样未动，任何 build 均返回 0x7

## 全量测试（5 条全部通过，含 conpty_custom 13 条、reader strip 相关 28 条）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
