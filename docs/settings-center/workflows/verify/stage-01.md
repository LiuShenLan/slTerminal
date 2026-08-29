# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SC-BE-01a**：`src-tauri/src/plan_balance/mod.rs` 存在模块级 `static POLL_INTERVAL_SEC: AtomicU64`（grep `POLL_INTERVAL_SEC` 命中 ≥3 处：声明/poller 读取/set_interval 写入），且 import `std::sync::atomic::{AtomicU64, Ordering}`
- **SC-BE-01b**：poller 已弃 ticker——`tokio::time::interval(` 在 mod.rs 零命中（语义式：须 Read `start_plan_balance_poller` 确认 loop 结构 = 先 poll（首轮立即，D8 语义保留）后 `tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SEC.load(Ordering::Relaxed)))`，即每轮末按当前内存值 sleep，非固定 period）
- **SC-BE-01c**：存在 `#[cfg(test)] pub(crate) fn reset_poll_interval_for_test()`（grep 命中），测试用例 `poll_interval_memory_default_is_60` 绿（测试 agent 产出判定）
- **SC-BE-02a**：存在 `pub async fn plan_balance_set_interval`（grep 命中）；语义式 Read 确认命令体顺序 = 校验 10–3600（越界 Err Validation 且不落盘不写内存）→ `crate::settings::save_settings` 落盘 → `POLL_INTERVAL_SEC.store`（顺序颠倒或缺步判 partial）
- **SC-BE-02b**：4 新用例（set_interval_valid_persists_and_updates_memory / set_interval_below_min_rejected / set_interval_above_max_rejected / set_interval_disk_memory_consistent）绿（测试 agent 产出判定）
- **SC-BE-03**：三处注册——`grep -c "plan_balance_set_interval" src-tauri/src/lib.rs src-tauri/build.rs` 各 1；`grep -c "allow-plan-balance-set-interval" src-tauri/capabilities/default.json` =1
- **SC-BE-04a**：`src-tauri/src/settings.rs` 白名单含 `crate::plan_balance::SETTINGS_KEY`（grep 命中）且数组仍 5 项（Read 确认 fontSize/keybindings/sideBar/colorScheme 四键字面量保留 + 注释说明归域口径）
- **SC-BE-04b**：mod.rs 存在 `SETTINGS_KEY` / `INTERVAL_SEC_KEY` 常量声明且 `resolve_poll_interval` 经常量读取（grep `"planBalance"` 字面量在 mod.rs 仅存于常量声明行——其它读取处均已归常量；测试用例 settings_key_constants_value 绿）
- **SC-BE-04c**：`resolve_poll_interval` 磁盘读取函数保留（既有 4 例 resolve_poll_interval_* 测试绿——语义式：函数未被删除或内联进原子量，启动初始化仍经磁盘读取）

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
