# L1 Rust 测试质量审查报告

## 执行摘要

- 审查范围：`src-tauri/tests/*.rs` 全部 7 个集成测试文件 + `src-tauri/src/` 内全部含 `#[test]` 的源文件（34 个文件）。
- 基线命令：`cargo test --manifest-path D:/data/learn/code/slTerminal/src-tauri/Cargo.toml -- --test-threads=1`
- 基线结果：726 个用例通过，0 失败，0 忽略。
- 复跑验证：对 9 组高可疑用例各连续运行 3 次，全部通过。

---

## 问题清单

### [中] M-1：`.claude/test-inventory.md` 头总计数与明细表不一致
- **维度**: inventory
- **证据**: `.claude/test-inventory.md:5` 表头登记“Rust 724 + 前端 2633 + L3 138 + E2E 40”；`grep -R '#[test]'` 实查 `src-tauri` 得 726；基线 `cargo test` 实跑 726 passed。
- **证据类型**: 静态推断 + 实证
- **问题**: 作为“项目用例数唯一真值源”，表头 L1 总数 724 与 34 个文件明细相加的 726 不一致，导致跨文档引用用例数时失实。
- **建议**: 将表头 `Rust 724` 修正为 `726`；在 inventory 顶部增加“总数 = 各文件明细之和”的自检说明，并建立每次新增用例后核对总数的流程。

### [低] L-1：`settings::concurrent_saves_never_torn` 使用含重试的测试专用辅助函数
- **维度**: 并发 / mock 合理性
- **证据**: `src-tauri/src/settings.rs:419-430` 定义 `run_save_with_retry`（5 次 × 50ms 重试）；`:438-460` `concurrent_saves_never_torn` 通过该辅助函数调用真实 `save_settings`；`:70-101` 生产 `save_settings` 在 `SETTINGS_SAVE_LOCK` 保护下只做一次 persist，失败即返回 `Err`。
- **证据类型**: 静态推断
- **问题**: 测试路径比生产路径更宽容。若 `SETTINGS_SAVE_LOCK` 被意外移除或失效，5 次重试可能掩盖真实的并发 persist 竞态，使测试无法在第一时机暴露回归。
- **建议**: 在 `run_save_with_retry` 注释中明确“该重试仅用于测试容忍杀软扫描窗口，不代替生产锁”；或评估是否将有限重试下沉到 `save_settings` 生产层，使测试与生产行为一致。

### [低] L-2：条件 skip 用例在 CI 上可能空跑，覆盖率不确定
- **维度**: 覆盖率
- **证据**: `src-tauri/src/pty/shell.rs:630-639` `test_allowlist_accepts_real_alias_when_present` 在找不到 `%LOCALAPPDATA%\Microsoft\WindowsApps` 时直接 `return`；`src-tauri/src/hooks/signal.rs:439-466` `process_symlink_signal_deletes_without_read` 在 `try_create_symlink` 失败时 `return`；`src-tauri/src/hooks/watcher.rs:291-317` `collect_excludes_symlink_files` 同样在 symlink 创建失败时 `return`；`src-tauri/src/notify/mod.rs:1189-1222` symlink 相关测试在创建失败时跳过；`src-tauri/src/agent_history/claude/ops.rs:438-512` 三个 symlink 测试均在创建失败时跳过。
- **证据类型**: 静态推断
- **问题**: 上述用例依赖 Windows Store 应用执行别名或管理员/开发者模式权限。CI runner 不满足条件时，对应分支不会被实际执行，但用例仍计为“通过”，造成覆盖率假象。
- **建议**: 在 CI 文档或 inventory 中标注这些用例的有效覆盖率依赖 runner 环境；必要时在具备权限的 CI job 中显式开启开发者模式或预置 alias，以锁定真实分支覆盖。

### [低] L-3：PTY 集成测试使用独立 `SPAWN_LOCK`，不验证生产锁
- **维度**: 并发 / mock 合理性
- **证据**: `src-tauri/tests/pty_integration_tests.rs:7` 定义 `static SPAWN_LOCK: Mutex<()>`；`:10-36` `spawn_cmd()` 使用该锁串行化 spawn；`src-tauri/src/pty/spawn.rs:1127-1138` 生产路径使用 `state.pty.spawn_lock.clone()` 并在锁内完成 `create_conpty_pair` + `spawn_conpty_child`。
- **证据类型**: 静态推断
- **问题**: 集成测试的锁与生产锁不是同一把锁，只能保证测试自身不并发 spawn，无法验证生产 `AppState.pty.spawn_lock` 是否真正串行化、是否覆盖到了临界区起点到终点的全部范围。
- **建议**: 在集成测试文件头部注释中说明“本锁仅用于测试目标并发隔离，生产锁由 `spawn.rs` 内 `pty_capacity_*` / `validate_spawn_request` 等用例间接守卫”；如未来能暴露生产锁，补充一把共用锁的集成测试。

### [低] L-4：`std::sync::Mutex` 中毒错误分支缺少回归用例
- **维度**: 覆盖率
- **证据**: `src-tauri/src/settings.rs:70-72` `.lock().map_err(|_| AppError::Unknown(...))`；`src-tauri/src/state.rs:140` `project_root_lock: tokio::sync::Mutex<()>` 及 `:254` 持锁调用 `set_project_root_impl`；`src-tauri/src/pty/spawn.rs` 多处 `Arc<Mutex<...>>` 持锁。没有任何用例主动制造锁中毒场景来验证错误映射。
- **证据类型**: 静态推断
- **问题**: 锁中毒分支是公开错误契约的一部分。虽然项目决策认为持锁临界区无 panic、中毒不可达，但一旦临界区后续引入可能 panic 的代码，缺乏回归用例将无法及时发现中毒处理是否正确。
- **建议**: 在相关模块 CLAUDE.md 中明确登记“Mutex 中毒路径无回归用例”；若未来持锁临界区引入高风险操作，优先补充中毒场景测试或改用不中毒的同步原语。

### [低] I-1：`pty_integration_tests.rs` 未加 Windows 平台守卫（原 Info 级）
- **维度**: 稳定性与确定性
- **证据**: `src-tauri/tests/pty_integration_tests.rs:1-36` 全文件硬编码 `cmd.exe`、未使用 `#[cfg(windows)]` 或 `cfg!(windows)` 分支。
- **证据类型**: 静态推断
- **问题**: 项目定位为 Windows-only，但文件在任意平台都会编译运行；非 Windows 环境下 `cmd.exe` 不存在，测试会直接失败，降低本地可移植性与 CI 配置灵活性。
- **建议**: 为整个测试文件或每个测试添加 `#[cfg(windows)]` 守卫，并在非 Windows 平台上跳过这些测试。

### [低] I-2：`conpty_api::write_if_size_differs` 仅按文件大小判定 vendor 升级（原 Info 级）
- **维度**: 断言有效性 / mock 合理性
- **证据**: `src-tauri/src/pty/conpty_api.rs:152-162` `write_if_size_differs` 仅在 `metadata.len() != bytes.len()` 时覆盖；`:265-274` 测试 `write_if_size_differs_overwrites_only_on_size_mismatch` 用 `b"new-vendor-bytes"`（长度 16）替换原内容（长度 16），再用 `b"same-len-456"`（长度 12）验证同大小不覆盖。
- **证据类型**: 静态推断
- **问题**: 测试准确覆盖了“按大小判定”的当前实现，但该实现存在隐式假设：不同 vendor 版本大小必然不同。若两个版本大小相同但内容不同，升级会被跳过，可能导致 Win10 捆绑 ConPTY 组件未实际更新。
- **建议**: 评估 vendor 更新场景下大小相同但内容不同的概率；如不可接受，将判定逻辑改为内容哈希，并同步更新测试；如接受，在 `pty/CLAUDE.md` 中明确登记该假设。

---

## 审查覆盖声明

### 已审阅文件

- `src-tauri/tests/common/mod.rs`
- `src-tauri/tests/ci_config_tests.rs`
- `src-tauri/tests/git_status_tests.rs`
- `src-tauri/tests/git_diff_tests.rs`
- `src-tauri/tests/git_file_at_head_tests.rs`
- `src-tauri/tests/git_rollback_tests.rs`
- `src-tauri/tests/git_unstage_tests.rs`
- `src-tauri/tests/pty_integration_tests.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/fs/mod.rs`
- `src-tauri/src/notify/mod.rs`
- `src-tauri/src/notify/pool.rs`
- `src-tauri/src/pty/spawn.rs`
- `src-tauri/src/pty/shell.rs`
- `src-tauri/src/pty/reader.rs`
- `src-tauri/src/pty/conpty_api.rs`
- `src-tauri/src/hooks/mod.rs`
- `src-tauri/src/hooks/provider.rs`
- `src-tauri/src/hooks/signal.rs`
- `src-tauri/src/hooks/watcher.rs`
- `src-tauri/src/hooks/claude/mod.rs`
- `src-tauri/src/hooks/claude/config.rs`
- `src-tauri/src/hooks/claude/inject.rs`
- `src-tauri/src/agent_history/mod.rs`
- `src-tauri/src/agent_history/provider.rs`
- `src-tauri/src/agent_history/claude/mod.rs`
- `src-tauri/src/agent_history/claude/scan.rs`
- `src-tauri/src/agent_history/claude/jsonl.rs`
- `src-tauri/src/agent_history/claude/ops.rs`
- `src-tauri/src/app_dir.rs`
- `src-tauri/src/projects.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/lib.rs`

### 基线执行命令与结果

```bash
cargo test --manifest-path D:/data/learn/code/slTerminal/src-tauri/Cargo.toml -- --test-threads=1
```

| 测试目标 | 用例数 | 结果 |
|----------|--------|------|
| `src/lib.rs` inline | 621 | ok |
| `src/main.rs` | 0 | ok |
| `tests/ci_config_tests.rs` | 1 | ok |
| `tests/git_diff_tests.rs` | 32 | ok |
| `tests/git_file_at_head_tests.rs` | 8 | ok |
| `tests/git_rollback_tests.rs` | 10 | ok |
| `tests/git_status_tests.rs` | 41 | ok |
| `tests/git_unstage_tests.rs` | 6 | ok |
| `tests/pty_integration_tests.rs` | 7 | ok |
| Doc-tests | 0 | ok |
| **合计** | **726** | **全绿** |

### 复跑验证记录

对以下过滤词各连续运行 3 次，全部通过：

- `set_project_root_serializes_concurrent_calls`（`state.rs`，`tokio::Mutex` 串行化）
- `concurrent_saves_never_torn`（`settings.rs`，`SETTINGS_SAVE_LOCK` + 测试重试）
- `pty_kill_all_kills_all_sessions`（`spawn.rs`，真实 cmd.exe 会话清理）
- `test_session_removal_does_not_cascade`（`spawn.rs`，会话 Map 隔离）
- `test_sessions_with_same_panel_id_independent`（`spawn.rs`，同 panel_id 隔离）
- `event_loop_*`（`notify/mod.rs`，真实 watcher 线程 + debounce）
- `watcher_stop_*` / `watcher_drop_*`（`hooks/watcher.rs`，线程生命周期）
- `start_signal_watcher_*`（`hooks/mod.rs`，全局静态 watcher 幂等）
- `test_allowlist_accepts_real_alias_when_present`（`shell.rs`，真实应用执行别名）
