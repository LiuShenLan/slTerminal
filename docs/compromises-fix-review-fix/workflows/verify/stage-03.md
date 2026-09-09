# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-01**：`rg "\.join\(\)" src-tauri/src` 仅命中 `src-tauri/src/thread_join.rs`（守卫白名单处——Read 确认命中行在 thread_join.rs 内，其余文件零命中；语义式：生产+测试全仓不存在其他裸 join 调用点，不限变量名，须逐命中行 Read 确认）。
- **BE-01**：`rg "join_with_timeout" src-tauri/src/notify/mod.rs src-tauri/src/hooks/watcher.rs` 命中（三处生产换装 + 两处测试换装）；Read 确认超时分支为 `tracing::warn` + detach（不再无界阻塞）。
- **BE-01**：`rg "KILL_JOIN_TIMEOUT" src-tauri/src` 零命中（不留别名）；`rg "mod thread_join" src-tauri/src/lib.rs` 命中；thread_join.rs 含 `mod join_tests`（spawn/reader 两测试组合并随迁——Read 确认两组用例在内）。
- **BE-01**：`CleanupPlan`/`plan_cleanup_after_join_timeout` 仍在 `src-tauri/src/pty/reader.rs`（未随迁——Read 确认）；`src-tauri/src/pty/CLAUDE.md` 含 join 守卫登记（rg "join" 命中，Read 抽查口径）。
- **BE-02**：`rg "partition_point" src-tauri/src/fs/mod.rs` 命中；`rg "to_lowercase" src-tauri/src/fs/mod.rs` 命中（sort_key 含 isDir+小写名——Read 确认形态）；游标编码为 base64("D\0"+lower / "F\0"+lower) 形态（Read 确认）；两用例 `growth_no_dup_no_hole`、`beyond_end_empty_page` 存在（rg 命中）且 cargo test 绿（测试 agent 结果承载）。
- **BE-02**：前端游标消费零改动（`git diff --stat src/` 本 Stage 应零前端文件——Read git status/diff 确认 cursor opaque 契约不变）；边界登记在 `src-tauri/src/fs/CLAUDE.md` 与 `src/ipc/CLAUDE.md`（rg "keyset\|游标" 两文件各命中）。
- **BE-03**：`rg "ignored" src-tauri/src/git/mod.rs` 零命中（:185 注释已删——若命中须 Read 确认是否为该注释残留）。
- **BE-04**：Read `src-tauri/src/CLAUDE.md` 守卫节——命令逐字为 `rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs"`；实跑该命令 exit 非零且无输出（零命中生产代码，自匹配消除——verify agent 实跑验证）。
- **BE-06**：`rg -c "重采样" src-tauri/src/agent_history/claude/scan.rs` ≥ 3（注释 + 两轮代码路径）；Read 确认断言两轮制形态（首轮越门槛 → sleep 2s → 重采样 20 次 → 仍越才 panic，消息附两轮中位）；基准用例绿（测试 agent 结果承载——L1 全量绿含 scan_bench）。

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
