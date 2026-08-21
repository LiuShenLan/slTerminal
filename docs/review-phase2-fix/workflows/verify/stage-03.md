# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-15**：`src-tauri/src/pty/shell.rs` 的 `paths_match` 为三臂 match——双成功精确比较 / 双侧失败归一化字符串比较 / 单侧失败 `_ => false` 拒绝（Read 确认三臂语义，不限变量名）
- **SEC-15**：grep `src-tauri/src/pty/shell.rs` 无「安全语义不弱化」残留；函数文档注释含「单侧失败即拒绝」语义表述（Read 确认）
- **SEC-15**：`src-tauri/src/pty/shell.rs` 含新增用例 `paths_match_single_side_failure_rejected`（grep 命中），且全量 L1 测试通过
- **SEC-15**：`src-tauri/src/pty/CLAUDE.md` 白名单段表述为「双侧失败才回退」（Read 确认，无「不拒绝，回退」旧表述残留）
- **SEC-17**：grep `src-tauri/src/hooks/claude/config.rs` 含 `target: "audit"` 且位于 user 层写入分支（Read 确认 `matches!(l, Layer::User)` 守卫）
- **SEC-17**：`src-tauri/src/hooks/CLAUDE.md` 含 SEC-17 威胁模型登记（grep 「UX 层」或「审计」命中）；`.claude/test-inventory.md` 豁免清单含 SEC-17 行
- **BE-22**：`src-tauri/src/notify/mod.rs` 的 `notify_watch` 前置校验经 `tokio::task::spawn_blocking` 执行（Read 确认：读锁取 root 快照后校验在 spawn_blocking 闭包内，TaskJoin 错误映射存在）
- **BE-24**：grep `src-tauri/src/state.rs` 含「写锁中毒」warn（`apply_project_root` Err 臂 match 形态，Read 确认）；`src-tauri/src/CLAUDE.md`「std Mutex 中毒保持现状」节含 BE-24 例外登记
- **BE-25**：grep `src-tauri/src/notify/mod.rs` 含 `eq_ignore_ascii_case`（位于 `is_excluded_path`）；`is_excluded_path_matches_all_seven_dirs` 测试含大小写变体断言（grep `Node_Modules` 命中）

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
