# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-02**：`rg "done_rx" src-tauri/src/notify/mod.rs` ≥ 3 命中（字段声明/构造/shutdown recv）；Read shutdown() 确认 assert(join_with_timeout) 之后含 `done_rx.recv().expect(...)` 行；Read spawn 闭包确认 `done_tx.send(())` 在 event_loop 返回后（语义式——panic 回传链完整：线程 panic → done_tx 随线程死亡 drop → recv Err → expect panic → 用例红）。
- **BE-06**：Read `impl Drop for FileWatcher` 确认 drop 体仅 `self.stop();`（无信号发送/join_with_timeout 复制段）；`rg -A5 "impl Drop for FileWatcher" src-tauri/src/notify/mod.rs` 确认无 join 逻辑残留。
- **BE-03**：`rg "decode_cursor_rejects" src-tauri/src/fs/mod.rs` 3 命中（三用例名）；Read 确认三例分别覆盖：非法 base64 / 非法 UTF-8（base64 合法载荷）/ 坏 tag + 无 NUL 两形态，断言均为 `AppError::Validation` 变体匹配（不锁文案）。
- **BE-05**：`rg "BE-06" src-tauri/src/pty/spawn.rs` 零命中；Read :2155 区确认注释已文字化（历史轮次注记口径，与本计划 BE-06 无编号撞车）。
- **BE-01**：`rg "4 例" .claude/test-exemptions.md` 的 join_with_timeout 相关命中零残留（Read :14 行确认 3 例口径）；Read :14 与 :24 两行确认均为 3 例一致。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --test lib_tests notify -- --test-threads=1`（L1 定向红线）
4. `cargo test --test lib_tests read_dir -- --test-threads=1`（L1 定向红线）
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1 全量）
