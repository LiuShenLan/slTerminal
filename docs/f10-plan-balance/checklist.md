# F10 编码套餐余量展示 — 变更清单（checklist）

> 真值源：需求规格 `docs/coding-plan-balance-spec.md`。本文档由 `/systematic-changes-plan` 产出（2026-08-28 批准），执行走 `/systematic-changes-execute`。
> 组织说明：不采用 P0-P4 优先级——优先级由 Stage 依赖顺序表达（Stage 01 后端 → 02 前端 → 03 文档）。
> 前缀：PB-BE（后端）/ PB-FE（前端）/ PB-DOC（文档与登记）。

## 决策记录（执行 agent 不做设计判断，全部照做）

### 用户裁定（2026-08-28）

| # | 决策点 | 裁定 |
|---|--------|------|
| U1 | footer 与「添加项目」钮相对位置 | **树滚动区与添加项目钮之间**（footer 自带顶部发丝线） |
| U2 | 后端注册表形态 | **静态切片 + 参数化查找**（照 `hooks/provider.rs` 先例；偏离规格 §10 字面引用的 #13 可变单例形态，理由：#13 先例全为前端模块，Rust 无 side-effect import；测试经参数化 lookup 注入，无需 `_reset`；「一行注册」目标不变） |
| U3 | plan-icons 图片 | **用户已放入** `public/plan-icons/deepseek.png`、`kimi.png`（已 Glob 核实存在） |

### 计划期决策（写死）

| # | 决策点 | 决策 | 依据 |
|---|--------|------|------|
| D1 | 后端模块位置 | 新建 `src-tauri/src/plan_balance/`（mod.rs / source.rs / query.rs / deepseek.rs / kimi.rs） | 硬约束 #2 按功能分模块 |
| D2 | home 解析 | plan_balance/source.rs 内自建 `home_dir()` + `HomeDirGuard`（**照抄模式，不跨模块调用** `hooks::claude::home_dir`） | 硬约束 #2 模块不穿透；规格 §4.1「复用先例」= 复用模式 |
| D3 | 应用 settings 读取 | 经 `crate::app_dir::app_data_dir()`（顶层共享件，BE-16 专为上提共享而设，不构成穿透） | app_dir.rs:1-11 注释 |
| D4 | 快照存储 | 模块级静态 `static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>>`（不入 AppState） | hooks/mod.rs:62 WATCHER 先例（「避免 state.rs 循环依赖」）；快照仅本模块读写，非跨模块共享 |
| D5 | emit 口径 | 快照整体 `PartialEq` 比较（**含 updated_at**）：成功查询必刷新 updated_at 即视为变化 → emit；失败保留旧值 → 不 emit；来源集合变化 → emit | 规格 §6「有变化才 emit」+ §7 updated_at=「最近成功查询」+ §8.2 tooltip「上次更新」三方一致的唯一口径 |
| D6 | refresh 语义 | `refresh_plan_balance` 恒返回 `Ok(最新快照)`（单来源失败按 §6 保留旧值，不整体 Err；仅 spawn_blocking join 失败才 Err）；前端用返回值直接更新，事件通道照常 | 用户点击期待即时反馈；失败静默（规格 §6） |
| D7 | 节流侧 | 5s 节流在**前端**（hook 内 `lastRefreshRef` 时间戳比较）；后端不节流 | 规格 §12 L2 测试范围含「点击刷新节流」；单窗口单实例 |
| D8 | 轮询启动 | `lib.rs` setup 中 `plan_balance::start_plan_balance_poller(app.handle().clone())`；`tauri::async_runtime::spawn` + `tokio::time::interval`，**首次 tick 立即执行第一轮** | 规格 §6；tokio 需加 `"time"` feature |
| D9 | 「尚无成功值」占位 | 后端快照含占位项 `{ frozen:false, amount:None, windows:None, updated_at:0 }` → 前端渲染 `--` 行；DTO 不变（amount/windows 本为 Option） | 规格 §8.2 第四场景；隐藏态 §8.3 不含「查询失败」 |
| D10 | ureq | `ureq = { version = "3", features = ["json"] }`；`Agent::config_builder().timeout_global(...)` 自建 agent（顶层 `ureq::get` 无超时不可用）；4xx/5xx 默认即 `Err(Error::StatusCode)`（`http_status_as_error` 默认 true）；错误 Display 不含请求头，token 安全 | context7 /algesten/ureq 官方文档核实 |
| D11 | 新命令三处注册 | `lib.rs generate_handler!` + `build.rs AppManifest::commands` + `capabilities/default.json allow-<cmd>` | src-tauri/src/CLAUDE.md 红线（SEC-07）；build.rs:14-16 注释「缺一即 invoke reject」；lib.rs P0-07 注释为 Tauri 默认行为描述，**以三处注册现状为准** |
| D12 | 时间格式 | `formatResetTime`：<1h → `Xm 后重置`；<24h → `Xh Ym 后重置`（m=0 时 `Xh 后重置`）；≥24h → `M月d日 HH:mm 重置`（月日**无**前导零，时分有前导零）；diff≤0 → `0m 后重置`（clamp，规格未覆盖，人工验证点）；解析失败/缺失 → null（省略该段） | 规格 §8.2 示例（3h42m / 9月2日 14:00） |
| D13 | 百分比换算 | `remaining_percent = clamp(round((1 - used/limit) * 100), 0, 100) as u8`；used/limit 字符串转 f64；limit 缺失或为 0 → 该窗口解析失败 | 规格 §5.2/§7/§8.2 |
| D14 | 测试用例编号 | 引用特性号 **F10**（test-inventory 覆盖要点列惯例：F7/F8/F9 同法） | test-inventory.md L2 表先例 |
| D15 | 不新增 ADR | 设计决策已在规格与本清单留档；注册表形态偏离理由写入 plan_balance/CLAUDE.md | ADR-0011 代码自证原则 |
| D16 | CONTEXT.md | 四术语已登记（CONTEXT.md:240-253，已 Read 核实）——**不改**，仅作 verify 验证项 | 实读核实 |

### 安全红线（规格 §7，verify 语义式断言）

- token 不出后端：DTO 六键 serde 键集合精确匹配测试锁死（无 token 字段）；plan_balance 模块内 tracing/错误消息**禁止插值 token 与 Authorization 头**（语义式断言：Read 全部 tracing!/Err 构造点确认）。

### 波及面排查结论（已 grep 实查）

- **无**任何测试断言「命令总数/capabilities 条目数」→ 三处注册加 2 条无既有断言波及。
- setup.ts 新增 `../ipc/planBalance` 全局 mock：既有 156 个测试文件无此 import，纯增量；nav-tree 两测试渲染 NavTree 时 footer 因 mock 返回 `[]` 整块不渲染，**既有断言零改动**。
- settings.rs 既有白名单拒绝用例（`save_rejects_illegal_top_level_key` 用 `"theme"`/`"evil"`）不受第 5 键影响。
- tsconfig include `["src","test/**/*"]`：本次前端新文件全在覆盖内，门禁标准五件套即可。

---

## PB-BE-01 Cargo.toml 依赖

1. **位置**：`src-tauri/Cargo.toml:42`（`tokio = { version = "1", features = ["rt", "sync"] }` 行）+ [dependencies] 段尾
2. **现状**：无 ureq；tokio features 仅 `["rt", "sync"]`（无 `time`，`tokio::time::interval` 不可用）
3. **修复步骤**：
   1. tokio 行改为 `tokio = { version = "1", features = ["rt", "sync", "time"] }`
   2. dependencies 段尾追加：`ureq = { version = "3", features = ["json"] }  # F10 套餐余量查询（纯阻塞 HTTP，spawn_blocking 调用）`
   3. `cargo build --manifest-path src-tauri/Cargo.toml` 验证拉取编译通过（ureq 3 默认 rustls，无 OpenSSL 依赖）
4. **测试同步**：无（编译级）
5. **文档同步**：无
6. **验证**：grep `^ureq` src-tauri/Cargo.toml 命中；grep `"time"` 命中 tokio 行；`cargo tree -p ureq` 显示 3.x

## PB-BE-02 plan_balance/mod.rs — DTO + 快照状态机 + 轮询/命令编排

1. **位置**：新建 `src-tauri/src/plan_balance/mod.rs`
2. **现状**：不存在
3. **修复步骤**：新建文件，照抄以下骨架（注释保留）：
   ```rust
   //! 套餐余量模块（F10）——读 user 层 settings.json 判定套餐 → 轮询查询 → 事件推送
   //!
   //! 红线：token 不出后端——DTO 无 token 字段；本模块 tracing/错误消息禁止插值
   //! token 与 Authorization 头（ureq 错误 Display 不含请求头，构造错误消息时禁止自行拼接）。

   pub mod deepseek;
   pub mod kimi;
   pub mod query;
   pub mod source;

   use std::sync::Mutex;
   use std::time::Duration;
   use tauri::Emitter;

   use crate::app_dir::app_data_dir;
   use crate::error::AppError;
   use query::{find_query_by_url, PlanQuery, QUERIES};
   use source::{PlanSource, SOURCES};

   // ── DTO（规格 §7；serde camelCase ↔ src/types/planBalance.ts，硬约束 #4） ──

   #[derive(Debug, Clone, PartialEq, serde::Serialize)]
   #[serde(rename_all = "camelCase")]
   pub struct PlanBalanceInfo {
       pub source_id: String, // v1 恒 "claude"
       pub plan_id: String,   // "deepseek" | "kimi"
       pub frozen: bool,      // kimi 月限额触顶
       pub amount: Option<AmountInfo>,
       pub windows: Option<WindowsInfo>,
       /// 最近成功查询 unix 秒；0 = 尚无成功值（前端渲染 --）
       pub updated_at: u64,
   }

   #[derive(Debug, Clone, PartialEq, serde::Serialize)]
   #[serde(rename_all = "camelCase")]
   pub struct AmountInfo {
       pub value: String, // 原样透传 total_balance
       pub currency: String,
   }

   #[derive(Debug, Clone, PartialEq, serde::Serialize)]
   #[serde(rename_all = "camelCase")]
   pub struct WindowsInfo {
       pub five_hour: WindowInfo,
       pub seven_day: WindowInfo,
   }

   #[derive(Debug, Clone, PartialEq, serde::Serialize)]
   #[serde(rename_all = "camelCase")]
   pub struct WindowInfo {
       pub remaining_percent: u8, // 剩余 = 100 - 已用
       pub resets_at: Option<String>, // ISO 字符串，可缺失
   }

   /// 套餐查询产出（不含 source_id/plan_id/updated_at——由编排层补全）
   #[derive(Debug, Clone, PartialEq)]
   pub struct FetchOutcome {
       pub frozen: bool,
       pub amount: Option<AmountInfo>,
       pub windows: Option<WindowsInfo>,
   }

   // ── 快照存储（模块级静态——照 hooks/mod.rs WATCHER 先例：仅本模块读写，
   //    不入 AppState 避免 state.rs 循环依赖） ──

   static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>> = Mutex::new(None);

   #[cfg(test)]
   pub(crate) fn reset_snapshot_for_test() {
       let _ = SNAPSHOT.lock().unwrap().take();
   }

   fn unix_now() -> u64 {
       std::time::SystemTime::now()
           .duration_since(std::time::UNIX_EPOCH)
           .map(|d| d.as_secs())
           .unwrap_or(0)
   }

   // ── 快照合并与轮询编排（纯函数/参数化，L1 全测） ──

   /// 快照槽合并（规格 §6）：planId 变化丢弃旧值；查询失败保留同 planId 旧值；
   /// 成功采用新值；失败且无旧值 → 占位（updated_at=0，前端显 --）
   pub(crate) fn merge_slot(
       old: Option<&PlanBalanceInfo>,
       source_id: &str,
       plan_id: &str,
       result: Result<FetchOutcome, AppError>,
       now: u64,
   ) -> PlanBalanceInfo {
       let retained = old.filter(|o| o.plan_id == plan_id); // planId 变化 → 丢弃
       match result {
           Ok(o) => PlanBalanceInfo {
               source_id: source_id.into(), plan_id: plan_id.into(),
               frozen: o.frozen, amount: o.amount, windows: o.windows, updated_at: now,
           },
           Err(e) => {
               tracing::warn!(source = source_id, plan = plan_id, error = %e, "套餐余量查询失败，保留旧值");
               retained.cloned().unwrap_or(PlanBalanceInfo {
                   source_id: source_id.into(), plan_id: plan_id.into(),
                   frozen: false, amount: None, windows: None, updated_at: 0,
               })
           }
       }
   }

   /// 一轮拉取编排（D6 最小可测性：resolve/fetch 参数化，L1 不触网不触盘）。
   /// 返回新快照；与旧快照比较有变化才 emit（emit 判定在 apply_snapshot）。
   pub(crate) fn poll_once_with(
       old: &[PlanBalanceInfo],
       resolve: impl Fn(&'static dyn PlanSource) -> Option<(String, String)>,
       fetch: impl Fn(&'static dyn PlanQuery, &str) -> Result<FetchOutcome, AppError>,
       now: u64,
   ) -> Vec<PlanBalanceInfo> {
       let mut out = Vec::new();
       for source in SOURCES {
           // 来源消失（settings 缺失/env 缺失/token 空）→ 本轮从数组移除（§8.3）
           let Some((base_url, token)) = resolve(*source) else { continue };
           // 未命中任何套餐 → 静默降级（§4.2）
           let Some(q) = find_query_by_url(&base_url, QUERIES) else { continue };
           out.push(merge_slot(
               old.iter().find(|i| i.source_id == source.source_id()),
               source.source_id(), q.plan_id(), fetch(*q, &token), now,
           ));
       }
       out
   }

   /// 生产一轮拉取（spawn_blocking 内调用）
   fn poll_once_production(now: u64) -> Vec<PlanBalanceInfo> {
       poll_once_with(
           &SNAPSHOT.lock().unwrap().clone().unwrap_or_default(),
           |s| s.resolve(),
           |q, token| q.fetch(token),
           now,
       )
   }

   /// 应用新快照：变化才 emit（含 updated_at 参与比较——成功查询必刷新
   /// updated_at 即视为变化；失败保留旧值不变不 emit；D5）
   fn apply_snapshot(app_handle: &tauri::AppHandle, new: Vec<PlanBalanceInfo>) {
       let mut guard = SNAPSHOT.lock().unwrap();
       if guard.as_ref() != Some(&new) {
           *guard = Some(new.clone());
           let _ = app_handle.emit("plan-balance-updated", new);
       }
   }

   // ── 轮询间隔（规格 §9）：应用 settings.json 的 planBalance.intervalSec；
   //    默认 60，合法 10–3600，越界/缺失/损坏回退默认 ──

   const DEFAULT_INTERVAL_SEC: u64 = 60;
   const MIN_INTERVAL_SEC: u64 = 10;
   const MAX_INTERVAL_SEC: u64 = 3600;

   pub(crate) fn resolve_poll_interval() -> Duration {
       let read = || -> Option<u64> {
           let path = app_data_dir().ok()?.join("settings.json");
           let content = std::fs::read_to_string(path).ok()?;
           let root: serde_json::Value = serde_json::from_str(&content).ok()?;
           root.get("planBalance")?.get("intervalSec")?.as_u64()
       };
       match read() {
           Some(v) if (MIN_INTERVAL_SEC..=MAX_INTERVAL_SEC).contains(&v) => Duration::from_secs(v),
           _ => Duration::from_secs(DEFAULT_INTERVAL_SEC),
       }
   }

   // ── 轮询任务（lib.rs setup 调用；随进程退出结束，单实例无生命周期管理） ──

   pub fn start_plan_balance_poller(app_handle: tauri::AppHandle) {
       let interval = resolve_poll_interval();
       tauri::async_runtime::spawn(async move {
           let mut ticker = tokio::time::interval(interval);
           loop {
               ticker.tick().await; // 首次立即触发——启动即跑第一轮
               let now = unix_now();
               let handle = app_handle.clone();
               match tokio::task::spawn_blocking(move || poll_once_production(now)).await {
                   Ok(new) => apply_snapshot(&handle, new),
                   Err(e) => tracing::warn!(error = %e, "套餐余量轮询任务异常"),
               }
           }
       });
   }

   // ── 命令（三处注册：lib.rs + build.rs + capabilities，D11） ──

   #[tauri::command]
   pub async fn get_plan_balance() -> Result<Vec<PlanBalanceInfo>, AppError> {
       Ok(SNAPSHOT.lock().unwrap().clone().unwrap_or_default())
   }

   /// 立即刷新（D6）：执行一轮拉取，更新快照并按 D5 判定 emit，恒返回最新快照
   #[tauri::command]
   pub async fn refresh_plan_balance(app_handle: tauri::AppHandle) -> Result<Vec<PlanBalanceInfo>, AppError> {
       let now = unix_now();
       let new = tokio::task::spawn_blocking(move || poll_once_production(now))
           .await
           .map_err(|e| AppError::TaskJoin(e.to_string()))?;
       apply_snapshot(&app_handle, new.clone());
       Ok(new)
   }
   ```
4. **测试同步**：同文件 `#[cfg(test)] mod tests`（全部经参数化注入，不触网不触盘；用例注释标 F10）：
   - `plan_balance_info_serde_key_set`：序列化后键集合精确 = `["amount","frozen","planId","sourceId","updatedAt","windows"]`（照 hooks/mod.rs:264 `assert_status_key_set` 先例）；AmountInfo/WindowsInfo/WindowInfo 同样键集合断言（`["currency","value"]`/`["fiveHour","sevenDay"]`/`["remainingPercent","resetsAt"]`）——**token 红线守卫**（多键即红）
   - `merge_slot_success_adopts_new`、`merge_slot_failure_retains_same_plan`、`merge_slot_plan_change_discards_old`、`merge_slot_failure_no_old_gives_placeholder`（占位形态：updated_at=0 + 全 None）
   - `poll_once_resolve_none_removes_source`（来源消失 → 数组空）、`poll_once_url_unmatched_skips`（未命中 → 空）、`poll_once_failure_retains`（失败保留旧值）、`poll_once_plan_change_discards`（planId 切换 → 占位/新值无残留）、`poll_once_success_replaces`（成功采用）
   - `resolve_poll_interval_default_when_missing`（AppDataDirGuard 注入 tempdir 无文件 → 60）、`_valid_value`（写入 `{"planBalance":{"intervalSec":120}}` → 120）、`_out_of_range_falls_back`（5 与 9999 → 60）、`_corrupt_json_falls_back`（损坏 → 60）
   - `get_plan_balance_empty_initial`（reset 后命令核心返回空数组；`tokio::runtime::Builder::new_current_thread` block_on 照 hooks/mod.rs:443 先例）
5. **文档同步**：无（模块文档在 PB-BE-08）
6. **验证**：`cargo test plan_balance` 全绿；grep `static SNAPSHOT` 命中；grep `emit("plan-balance-updated"` 命中 mod.rs

## PB-BE-03 plan_balance/source.rs — PlanSource trait + claude user 层来源

1. **位置**：新建 `src-tauri/src/plan_balance/source.rs`
2. **现状**：不存在；模式原型 `hooks/claude/mod.rs:52-87`（home_dir/HomeDirGuard）、`hooks/provider.rs:56-64`（静态切片）
3. **修复步骤**：新建文件：
   ```rust
   //! 余量来源（规格 §4.1/§10）：trait + 静态切片注册表（U2：照 hooks/provider.rs
   //! 先例，偏离 #13 可变单例——Rust 无 side-effect import，测试经参数化注入）
   //!
   //! home 解析照 hooks/claude/mod.rs home_dir()/HomeDirGuard 模式自建（D2：
   //! 硬约束 #2 模块不穿透，不跨模块调用 hooks::claude）

   use std::path::PathBuf;

   /// 余量来源 trait：输入 = 无，输出 = Option<(baseUrl, token)>；解析格式由各来源自定
   pub trait PlanSource: Send + Sync + std::fmt::Debug {
       /// 来源标识（DTO source_id，按注册序 emit）
       fn source_id(&self) -> &'static str;
       /// 解析 (baseUrl, token)；文件缺失/env 缺失/字段缺失/token 为空 → None（静默降级 §8.3）
       fn resolve(&self) -> Option<(String, String)>;
   }

   /// claude user 层 settings.json 来源（v1 唯一来源）
   #[derive(Debug)]
   pub struct ClaudeUserSettingsSource;

   static CLAUDE_USER_SOURCE: ClaudeUserSettingsSource = ClaudeUserSettingsSource;

   /// 静态注册表（新增来源 = 新实现 + 此处一行）
   pub(crate) static SOURCES: &[&dyn PlanSource] = &[&CLAUDE_USER_SOURCE];

   impl PlanSource for ClaudeUserSettingsSource {
       fn source_id(&self) -> &'static str { "claude" }
       fn resolve(&self) -> Option<(String, String)> {
           resolve_env(&claude_settings_content()?)
       }
   }

   fn claude_settings_content() -> Option<String> {
       let path = home_dir()?.join(".claude").join("settings.json");
       std::fs::read_to_string(path).ok() // 不存在/不可读 → None（静默降级）
   }

   /// env 段提取（纯函数，L1 全测）：JSON 损坏/BASE_URL 缺失/token 缺失或为空 → None
   fn resolve_env(content: &str) -> Option<(String, String)> {
       let root: serde_json::Value = match serde_json::from_str(content) {
           Ok(v) => v,
           Err(e) => {
               tracing::warn!(error = %e, "claude settings.json 解析失败，余量来源静默降级");
               return None;
           }
       };
       let env = root.get("env")?;
       let base_url = env.get("ANTHROPIC_BASE_URL")?.as_str()?;
       let token = env.get("ANTHROPIC_AUTH_TOKEN")?.as_str()?;
       if token.is_empty() { return None; }
       Some((base_url.to_string(), token.to_string()))
   }

   // ── home 解析（照 hooks/claude/mod.rs 先例模式，cfg(test) 注入守卫） ──
   #[cfg(test)]
   static HOME_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
   #[cfg(test)]
   pub(crate) struct HomeDirGuard(Option<PathBuf>);
   #[cfg(test)]
   impl HomeDirGuard {
       pub(crate) fn set(dir: &std::path::Path) -> Self {
           let mut slot = HOME_DIR_OVERRIDE.lock().unwrap();
           let prev = slot.clone();
           *slot = Some(dir.to_path_buf());
           HomeDirGuard(prev)
       }
   }
   #[cfg(test)]
   impl Drop for HomeDirGuard {
       fn drop(&mut self) { *HOME_DIR_OVERRIDE.lock().unwrap() = self.0.clone(); }
   }
   fn home_dir() -> Option<PathBuf> {
       #[cfg(test)]
       {
           if let Some(d) = HOME_DIR_OVERRIDE.lock().unwrap().clone() {
               return Some(d);
           }
       }
       dirs::home_dir()
   }
   ```
4. **测试同步**：同文件 tests：`resolve_env` 纯函数 6 例（正常/JSON 损坏/env 缺失/BASE_URL 缺失/token 缺失/token 空串）；`resolve` 命令层 2 例（HomeDirGuard 注入 tempdir：无文件 → None；落盘合法 settings → Some 且 token 原样返回）
5. **文档同步**：无
6. **验证**：`cargo test plan_balance::source` 全绿；grep `static SOURCES` 命中

## PB-BE-04 plan_balance/query.rs — PlanQuery trait + URL 归一化匹配

1. **位置**：新建 `src-tauri/src/plan_balance/query.rs`
2. **现状**：不存在
3. **修复步骤**：
   ```rust
   //! 套餐查询注册表（规格 §4.2/§10）：URL 匹配集 + fetch trait；静态切片（U2）

   use crate::error::AppError;
   use super::FetchOutcome;

   /// 套餐查询 trait：新增套餐 = 新实现 + QUERIES 一行注册
   pub trait PlanQuery: Send + Sync + std::fmt::Debug {
       fn plan_id(&self) -> &'static str;
       /// URL 匹配集（元素须为已归一化形态：小写、无尾斜杠）
       fn base_urls(&self) -> &'static [&'static str];
       /// 阻塞 HTTP 查询（调用方负责 spawn_blocking，硬约束 #3）；
       /// 错误消息禁止含 token/Authorization（红线）
       fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError>;
   }

   static DEEPSEEK: super::deepseek::DeepSeekQuery = super::deepseek::DeepSeekQuery;
   static KIMI: super::kimi::KimiQuery = super::kimi::KimiQuery;

   /// 按注册序（emit 数组顺序即此序）
   pub(crate) static QUERIES: &[&dyn PlanQuery] = &[&DEEPSEEK, &KIMI];

   /// URL 归一化（规格 §4.2 字面）：小写化 + 去尾部斜杠
   pub(crate) fn normalize_base_url(url: &str) -> String {
       url.to_lowercase().trim_end_matches('/').to_string()
   }

   /// 归一化后与匹配集逐项精确相等（参数化查找供 L1 注入，照 lookup_provider 先例）
   pub(crate) fn find_query_by_url<'a>(
       base_url: &str,
       queries: &'a [&'a dyn PlanQuery],
   ) -> Option<&'a dyn PlanQuery> {
       let normalized = normalize_base_url(base_url);
       queries.iter().copied().find(|q| q.base_urls().contains(&normalized.as_str()))
   }

   /// ureq agent 工厂（D10）：timeout_global 覆盖连接/读写全程；4xx/5xx 默认即 Err
   pub(crate) fn http_agent(timeout: std::time::Duration) -> ureq::Agent {
       ureq::Agent::config_builder()
           .timeout_global(Some(timeout))
           .build()
           .into()
   }

   /// 查询错误统一映射（消息只含 planId + 错误类别，禁止拼 token）
   pub(crate) fn query_err(plan_id: &str, e: ureq::Error) -> AppError {
       let kind = match &e {
           ureq::Error::StatusCode(code) => format!("HTTP {code}"),
           ureq::Error::Timeout(_) => "超时".to_string(),
           _ => "网络错误".to_string(),
       };
       tracing::warn!(plan = plan_id, error = %e, "套餐余量 HTTP 查询失败");
       AppError::Unknown(format!("套餐 {plan_id} 查询失败: {kind}"))
   }
   ```
4. **测试同步**：同文件 tests：`normalize_base_url` 4 例（大写宿主/单尾斜杠/多尾斜杠/已归一不变）；`find_query_by_url` 4 例（命中 deepseek/命中 kimi/大小写+尾斜杠归一后命中/未命中 None）；注册表序断言 1 例（QUERIES plan_id 序 = ["deepseek","kimi"]）
5. **文档同步**：无
6. **验证**：`cargo test plan_balance::query` 全绿；grep `normalize_base_url` 命中

## PB-BE-05 plan_balance/deepseek.rs

1. **位置**：新建 `src-tauri/src/plan_balance/deepseek.rs`
2. **现状**：不存在；规格 §5.1
3. **修复步骤**：
   ```rust
   //! deepseek 套餐（规格 §5.1）：GET /user/balance，取 balance_infos[0]

   use std::time::Duration;
   use crate::error::AppError;
   use super::query::{http_agent, query_err, PlanQuery};
   use super::{AmountInfo, FetchOutcome};

   #[derive(Debug)]
   pub struct DeepSeekQuery;

   const TIMEOUT: Duration = Duration::from_secs(5);

   impl PlanQuery for DeepSeekQuery {
       fn plan_id(&self) -> &'static str { "deepseek" }
       fn base_urls(&self) -> &'static [&'static str] { &["https://api.deepseek.com/anthropic"] }
       fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError> {
           let resp = http_agent(TIMEOUT)
               .get("https://api.deepseek.com/user/balance")
               .header("Accept", "application/json")
               .header("Authorization", &format!("Bearer {token}"))
               .call()
               .map_err(|e| query_err(self.plan_id(), e))?;
           let body: serde_json::Value = resp
               .into_body()
               .read_json()
               .map_err(|e| AppError::Unknown(format!("deepseek 响应读取失败: {e}")))?;
           parse_deepseek_balance(&body)
       }
   }

   /// 响应解析（纯函数，罐装 JSON 可测）：
   /// balance_infos[0] 的 total_balance/currency；空数组/字段缺失/非字符串 → Err
   pub(crate) fn parse_deepseek_balance(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
       let first = body
           .get("balance_infos")
           .and_then(|v| v.as_array())
           .and_then(|a| a.first())
           .ok_or_else(|| AppError::Unknown("deepseek 响应缺 balance_infos[0]".into()))?;
       let value = first.get("total_balance").and_then(|v| v.as_str())
           .ok_or_else(|| AppError::Unknown("deepseek 响应缺 total_balance".into()))?;
       let currency = first.get("currency").and_then(|v| v.as_str())
           .ok_or_else(|| AppError::Unknown("deepseek 响应缺 currency".into()))?;
       Ok(FetchOutcome {
           frozen: false,
           amount: Some(AmountInfo { value: value.into(), currency: currency.into() }),
           windows: None,
       })
   }
   ```
4. **测试同步**：同文件 tests 6 例（正常 CNY/正常 USD/空 balance_infos 数组/缺 balance_infos 键/缺 total_balance/多币种只取 [0]）；fetch 真实网络登记豁免
5. **文档同步**：无
6. **验证**：`cargo test plan_balance::deepseek` 全绿

## PB-BE-06 plan_balance/kimi.rs

1. **位置**：新建 `src-tauri/src/plan_balance/kimi.rs`
2. **现状**：不存在；规格 §5.2（kimi-usage.js 语义已内化于规格，仓内无该文件——已 grep 核实）
3. **修复步骤**：
   ```rust
   //! kimi 套餐（规格 §5.2）：GET /coding/v1/usages，双时间窗 + 月限额触顶态

   use std::time::Duration;
   use crate::error::AppError;
   use super::query::{http_agent, query_err, PlanQuery};
   use super::{FetchOutcome, WindowInfo, WindowsInfo};

   #[derive(Debug)]
   pub struct KimiQuery;

   const TIMEOUT: Duration = Duration::from_secs(8);

   impl PlanQuery for KimiQuery {
       fn plan_id(&self) -> &'static str { "kimi" }
       fn base_urls(&self) -> &'static [&'static str] { &["https://api.kimi.com/coding"] }
       fn fetch(&self, token: &str) -> Result<FetchOutcome, AppError> {
           let resp = http_agent(TIMEOUT)
               .get("https://api.kimi.com/coding/v1/usages")
               .header("Authorization", &format!("Bearer {token}"))
               .call()
               .map_err(|e| query_err(self.plan_id(), e))?;
           let body: serde_json::Value = resp
               .into_body()
               .read_json()
               .map_err(|e| AppError::Unknown(format!("kimi 响应读取失败: {e}")))?;
           parse_kimi_usages(&body)
       }
   }

   /// 剩余百分比（D13）：(1 - used/limit) * 100 四舍五入，clamp 0–100；
   /// 数值字段为字符串（规格口径），limit 缺失/为 0/非字符串 → None（该窗口失败）
   fn remaining_percent(used: Option<&str>, limit: Option<&str>) -> Option<u8> {
       let used: f64 = used?.parse().ok()?;
       let limit: f64 = limit?.parse().ok()?;
       if limit <= 0.0 { return None; }
       Some(((1.0 - used / limit) * 100.0).round().clamp(0.0, 100.0) as u8)
   }

   /// 单窗口解析：used/limit/resetTime（resetTime 路径由调用方给）
   fn parse_window(entry: &serde_json::Value, reset_path: &[&str]) -> Result<WindowInfo, AppError> {
       let percent = remaining_percent(
           entry.get("used").and_then(|v| v.as_str()),
           entry.get("limit").and_then(|v| v.as_str()),
       )
       .ok_or_else(|| AppError::Unknown("kimi 窗口 used/limit 解析失败".into()))?;
       let resets_at = reset_path
           .iter()
           .fold(Some(entry), |v, k| v?.get(k))
           .and_then(|v| v.as_str())
           .map(|s| s.to_string());
       Ok(WindowInfo { remaining_percent: percent, resets_at })
   }

   /// 响应解析（纯函数，罐装 JSON 可测；规格 §5.2 全有或全无）：
   /// 1. totalQuota.used == "1" → frozen=true（不要求窗口解析成功，windows=None）
   /// 2. 5h 窗：limits[] 中 window.duration==300 && window.timeUnit=="TIME_UNIT_MINUTE"
   ///    优先，否则 limits[0]；limits 空 → 失败
   /// 3. 7d 窗：顶层 usage 字段
   /// 4. 非触顶时任一窗口失败 → 整体 Err（防窗口重置瞬间 limits 不完整致 5h 段丢失）
   pub(crate) fn parse_kimi_usages(body: &serde_json::Value) -> Result<FetchOutcome, AppError> {
       let frozen = body
           .get("totalQuota")
           .and_then(|q| q.get("used"))
           .and_then(|v| v.as_str())
           == Some("1");
       if frozen {
           return Ok(FetchOutcome { frozen: true, amount: None, windows: None });
       }
       let limits = body.get("limits").and_then(|v| v.as_array())
           .ok_or_else(|| AppError::Unknown("kimi 响应缺 limits".into()))?;
       let five_hour_entry = limits
           .iter()
           .find(|l| {
               l.get("window").and_then(|w| w.get("duration")).and_then(|d| d.as_u64()) == Some(300)
                   && l.get("window").and_then(|w| w.get("timeUnit")).and_then(|t| t.as_str())
                       == Some("TIME_UNIT_MINUTE")
           })
           .or_else(|| limits.first())
           .ok_or_else(|| AppError::Unknown("kimi 响应 limits 为空".into()))?;
       let five_hour = parse_window(five_hour_entry, &["detail", "resetTime"])?;
       let usage = body.get("usage")
           .ok_or_else(|| AppError::Unknown("kimi 响应缺 usage".into()))?;
       let seven_day = parse_window(usage, &["resetTime"])?;
       Ok(FetchOutcome {
           frozen: false,
           amount: None,
           windows: Some(WindowsInfo { five_hour, seven_day }),
       })
   }
   ```
4. **测试同步**：同文件 tests 10 例（双窗正常值与剩余换算/300min 优先命中/无 300min 回退 limits[0]/limits 空整体 Err/5h limit="0" 整体 Err/7d limit 缺失整体 Err/触顶 frozen=true 且窗口缺失仍 Ok/非触顶 usage 缺失整体 Err/resetTime 缺失 → resets_at=None/used>limit clamp 0）
5. **文档同步**：无
6. **验证**：`cargo test plan_balance::kimi` 全绿

## PB-BE-07 命令三处注册 + settings 白名单第 5 键

1. **位置**：
   - `src-tauri/src/lib.rs:1-11`（mod 区）/ :93-99（setup）/ :100-135（generate_handler!）
   - `src-tauri/build.rs:17-52`（commands 清单）
   - `src-tauri/capabilities/default.json:18-51`（permissions）
   - `src-tauri/src/settings.rs:17`（白名单）
2. **现状**：
   - lib.rs mod 区无 plan_balance；setup 仅两行 hooks 调用；handler 34 条
   - build.rs:16 注释「清单须与 lib.rs 的 generate_handler! 注册保持一致（当前 34 条）」
   - capabilities 34 条 allow-*，末条 `allow-agent-history-read-title`
   - settings.rs:17 `const SETTINGS_ALLOWED_KEYS: [&str; 4] = ["fontSize", "keybindings", "sideBar", "colorScheme"];`
3. **修复步骤**：
   1. lib.rs mod 区按现行顺序插入 `mod plan_balance;`（projects 后、pty 前）
   2. lib.rs setup 块 `hooks::reinject_statusline_on_startup();` 后加一行：`plan_balance::start_plan_balance_poller(app.handle().clone()); // F10 套餐余量轮询`
   3. generate_handler! 末尾（`agent_history::agent_history_read_title,` 后）加：`plan_balance::get_plan_balance,` / `plan_balance::refresh_plan_balance,`
   4. build.rs commands 数组末尾加 `"get_plan_balance",` / `"refresh_plan_balance",`，注释「当前 34 条」改「当前 36 条」
   5. capabilities permissions 末尾加 `"allow-get-plan-balance",` / `"allow-refresh-plan-balance"`
   6. settings.rs:17 改 `const SETTINGS_ALLOWED_KEYS: [&str; 5] = ["fontSize", "keybindings", "sideBar", "colorScheme", "planBalance"];`；:13-16 注释键清单同步加 planBalance（注明「F10 轮询间隔，手改文件，读取侧在 plan_balance 模块」）
4. **测试同步**：settings.rs tests 加 1 例 `save_accepts_plan_balance_key`（`{"planBalance":{"intervalSec":120}}` 放行且 save/load 往返一致——防白名单回归）；既有拒绝用例不动
5. **文档同步**：无（settings 口径在 PB-DOC-02）
6. **验证**：grep 三处注册一致（lib.rs handler `plan_balance::` 2 处 / build.rs `"get_plan_balance"` `"refresh_plan_balance"` / capabilities `allow-get-plan-balance` `allow-refresh-plan-balance`）；grep `&str; 5` settings.rs 命中；`cargo test settings` 全绿

## PB-BE-08 plan_balance/CLAUDE.md 新建（硬约束：新建模块同步创建）

1. **位置**：新建 `src-tauri/src/plan_balance/CLAUDE.md`；登记 `/.claude/CLAUDE.md` 模块索引表（`src-tauri/src/pty` 行区域加一行 `| src-tauri/src/plan_balance | ../src-tauri/src/plan_balance/CLAUDE.md |`）
2. **现状**：不存在；模板 = 根 CLAUDE.md「子文件模板」（存在理由 → 关键约束与决策 why → 外部坑/红线 → 测试模式非显而易见部分）
3. **修复步骤**：新建模块文档，要点（按模板成文，以下内容必须覆盖）：
   - 存在理由：F10 余量查询——外部 API 语义（两套餐响应结构、触顶态、全有或全无）与 token 红线无法从代码自证
   - 决策：静态切片注册表偏离 #13 的理由（U2）；快照模块级静态先例引用（D4）；emit 含 updated_at 口径（D5）；refresh 恒 Ok（D6）；home_dir 照抄不跨模块（D2）
   - 红线：token 不出后端（DTO 键集合测试锁死 + 日志禁止插值）；URL 归一化只小写+去尾斜杠（不加 trim，规格字面）；kimi 数值字段按字符串解析（规格口径，实证偏差走人工实测）
   - 测试模式：参数化 poll_once_with/merge_slot 纯函数全测；fetch 真实 HTTP 与轮询任务本体豁免（指向 test-inventory 登记行）
4. **测试同步**：无
5. **文档同步**：本项即文档；根模块索引行同步
6. **验证**：文件存在且含「token」「静态切片」「豁免」关键节；根 CLAUDE.md 模块索引 grep `plan_balance` 命中

## PB-FE-01 types/planBalance.ts — DTO 双边对应

1. **位置**：新建 `src/types/planBalance.ts`
2. **现状**：不存在；先例 `src/types/agentHistory.ts`（全 camelCase 接口）
3. **修复步骤**：
   ```ts
   // 套餐余量 DTO（F10）——与 src-tauri/src/plan_balance/mod.rs 双边对应（硬约束 #4）
   // Rust snake_case ↔ TS camelCase 由 Tauri 自动转换

   /** 单来源套餐余量（updatedAt=0 表示尚无成功值 → 行显 --） */
   export interface PlanBalanceInfo {
     sourceId: string; // v1 恒 "claude"
     planId: string; // "deepseek" | "kimi"
     frozen: boolean; // kimi 月限额触顶
     amount: AmountInfo | null;
     windows: WindowsInfo | null;
     updatedAt: number;
   }

   export interface AmountInfo {
     value: string; // 原样透传 total_balance
     currency: string;
   }

   export interface WindowsInfo {
     fiveHour: WindowInfo;
     sevenDay: WindowInfo;
   }

   export interface WindowInfo {
     remainingPercent: number; // 剩余 = 100 - 已用
     resetsAt: string | null; // ISO 字符串，可缺失
   }
   ```
4. **测试同步**：键集合断言在 PB-FE-04 契约测试中（DTO 键集合精确匹配后端 serde 输出）
5. **文档同步**：无
6. **验证**：`npx tsc --noEmit` 绿；文件键名与 PB-BE-02 serde 测试断言集合一致（verify 人工对照）

## PB-FE-02 ipc/planBalance.ts + index.ts 登记

1. **位置**：新建 `src/ipc/planBalance.ts`；改 `src/ipc/index.ts:16`（notification 行后）
2. **现状**：index.ts:3-16 现 14 个领域导出；先例 notify.ts:28-35（onFsEvent 返回 unsubscribe）
3. **修复步骤**：
   ```ts
   // 套餐余量 IPC（F10）——快照拉取 / 立即刷新 / 更新事件订阅
   import { invoke } from "@tauri-apps/api/core";
   import { listen } from "@tauri-apps/api/event";
   import type { PlanBalanceInfo } from "../types/planBalance";

   /** 拉取当前快照（挂载时一次；后端尚未有快照 → 空数组） */
   export function getPlanBalance(): Promise<PlanBalanceInfo[]> {
     return invoke("get_plan_balance");
   }

   /**
    * 立即刷新（点击余量行）：后端执行一轮拉取并返回最新快照。
    * 单来源失败按规格 §6 保留旧值不整体报错；调用方 catch 仅防御（console.error）。
    */
   export function refreshPlanBalance(): Promise<PlanBalanceInfo[]> {
     return invoke("refresh_plan_balance");
   }

   /** 订阅余量更新（后端有变化才推送）；返回 unsubscribe，卸载时调用 */
   export function onPlanBalanceUpdated(
     callback: (payload: PlanBalanceInfo[]) => void,
   ): () => void {
     const unlisten = listen<PlanBalanceInfo[]>("plan-balance-updated", (event) =>
       callback(event.payload),
     );
     return () => {
       unlisten.then((fn) => fn());
     };
   }
   ```
   index.ts 加 `export * as planBalance from "./planBalance";`（notification 行后）
4. **测试同步**：PB-FE-04
5. **文档同步**：PB-DOC-01（ipc/CLAUDE.md 加 planBalance 段）
6. **验证**：grep `plan-balance-updated` src/ipc/planBalance.ts 命中；grep `export \* as planBalance` src/ipc/index.ts 命中

## PB-FE-03 setup.ts 全局 mock + __tests__/CLAUDE.md 登记（TQ-CI-05 同步义务）

1. **位置**：`src/__tests__/setup.ts:104-110`（agentHooks mock 块后）；`src/__tests__/CLAUDE.md`「全局 mock 策略（setup.ts）」节
2. **现状**：setup.ts 全局 mock 三类（ipc/notify、ipc/agentHooks、@tauri-apps/api/window）；__tests__/CLAUDE.md「全局 mock 策略」节列三条
3. **修复步骤**：
   1. setup.ts agentHooks mock 块后追加：
      ```ts
      // 全局 mock：ipc/planBalance（F10——NavTree footer 挂载即 invoke/listen，
      // 避免既有 nav-tree 等测试触发真实 IPC）
      vi.mock("../ipc/planBalance", () => ({
        getPlanBalance: () => Promise.resolve([]),
        refreshPlanBalance: () => Promise.resolve([]),
        onPlanBalanceUpdated: () => () => {},
      }));
      ```
   2. __tests__/CLAUDE.md mock 清单三条后加第四条：`` `../ipc/planBalance`：`getPlanBalance`/`refreshPlanBalance` resolve 空数组，`onPlanBalanceUpdated` 返回 no-op 取消函数（F10）。``
4. **测试同步**：本项即测试基建；既有 nav-tree 两测试零改动（mock 返回 [] → footer 隐藏）
5. **文档同步**：本项含 __tests__/CLAUDE.md
6. **验证**：`npm test` 全绿（既有用例零回归即为本项证据）；grep `ipc/planBalance` setup.ts 与 __tests__/CLAUDE.md 双命中

## PB-FE-04 ipc-plan-balance-contract.test.ts — IPC 契约 + 事件解包 + DTO 键集合

1. **位置**：新建 `src/__tests__/ipc-plan-balance-contract.test.ts`
2. **现状**：先例 `ipc-agent-history-contract.test.ts`（describeIpcContract 四维）+ `ipc-agent-hooks-contract.test.ts:197-227`（listen 解包手写）；工厂 `helpers/ipc-contract.ts`
3. **修复步骤**：
   1. 文件顶部按先例覆盖全局 mock：`vi.mock("../ipc/planBalance", async (importOriginal) => importOriginal<typeof import("../ipc/planBalance")>());`
   2. `describeIpcContract("planBalance", [...])` 两组四维（命令名 `get_plan_balance`/`refresh_plan_balance`、无参、返回透传、异常传播）
   3. listen 解包手写 2 例（照 agent-hooks 先例）：捕获 `listen("plan-balance-updated", handler)` → 构造 `{ payload }` → callback 收到解包数组；unsubscribe 调用链
   4. DTO 键集合断言 1 例：构造全字段 PlanBalanceInfo 字面量，`expect(Object.keys(info).sort())` 精确等于 `["amount","frozen","planId","sourceId","updatedAt","windows"]`（与后端 serde 测试互为双边锁）
4. **测试同步**：本项即测试
5. **文档同步**：无（test-inventory 在 PB-DOC-03）
6. **验证**：`npx vitest run ipc-plan-balance` 全绿

## PB-FE-05 planBalanceModel.ts 纯函数 + 测试

1. **位置**：新建 `src/features/navTree/planBalanceModel.ts`；测试 `src/__tests__/plan-balance-model.test.ts`
2. **现状**：不存在；时间格式化先例 `agentHistory/historyModel.ts:90-112`（粒度不符，不复用）
3. **修复步骤**（语义写死，D12/规格 §5.1/§8.2）：
   ```ts
   // 套餐余量展示纯函数（F10 规格 §8.2）——全部颜色/布局在组件层，本文件零依赖 theme

   import type { PlanBalanceInfo } from "../../types/planBalance";

   /** 货币符号映射（§5.1）：CNY→¥、USD→$、未知 → 原货币代码 */
   export function currencySymbol(currency: string): string {
     if (currency === "CNY") return "¥";
     if (currency === "USD") return "$";
     return currency;
   }

   /** logo 路径约定（§10.3）：/plan-icons/<planId>.png（无映射表） */
   export function planLogoSrc(planId: string): string {
     return `/plan-icons/${planId}.png`;
   }

   const MINUTE_MS = 60_000;
   const HOUR_MS = 60 * MINUTE_MS;
   const DAY_MS = 24 * HOUR_MS;

   /**
    * 重置时间格式化（D12）：<1h → `Xm 后重置`；<24h → `Xh Ym 后重置`（m=0 → `Xh 后重置`）；
    * ≥24h → `M月d日 HH:mm 重置`（月日无前导零）；diff≤0 → `0m 后重置`（clamp）；
    * 缺失/解析失败 → null（调用方省略该段）
    */
   export function formatResetTime(resetsAt: string | null, nowMs: number): string | null {
     if (!resetsAt) return null;
     const t = Date.parse(resetsAt);
     if (Number.isNaN(t)) return null;
     const diff = Math.max(0, t - nowMs);
     if (diff < HOUR_MS) return `${Math.ceil(diff / MINUTE_MS)}m 后重置`;
     if (diff < DAY_MS) {
       const h = Math.floor(diff / HOUR_MS);
       const m = Math.round((diff % HOUR_MS) / MINUTE_MS);
       return m > 0 ? `${h}h ${m}m 后重置` : `${h}h 后重置`;
     }
     const d = new Date(t);
     const pad = (n: number) => String(n).padStart(2, "0");
     return `${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())} 重置`;
   }

   /** 「上次更新 HH:mm:ss」（updatedAt=0 → null） */
   export function formatUpdatedAt(updatedAt: number): string | null {
     if (updatedAt <= 0) return null;
     const d = new Date(updatedAt * 1000);
     const pad = (n: number) => String(n).padStart(2, "0");
     return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
   }

   /** 行文案四场景（§8.2）：frozen → 已冻结；金额；双窗；无数据 → -- */
   export function rowText(info: PlanBalanceInfo): string {
     if (info.frozen) return "已冻结";
     if (info.amount) return `${currencySymbol(info.amount.currency)}${info.amount.value}`;
     if (info.windows) {
       return `5h ${info.windows.fiveHour.remainingPercent}% · 7d ${info.windows.sevenDay.remainingPercent}%`;
     }
     return "--";
   }

   /** tooltip 文案（§8.2）：kimi 双窗重置段（缺失省略）+ 上次更新；frozen/无数据固定文案 */
   export function rowTooltip(info: PlanBalanceInfo, nowMs: number): string {
     if (info.frozen) return "月限额触顶，Kimi Code 已冻结";
     if (!info.amount && !info.windows) return "查询中 / 查询失败重试中";
     const parts: string[] = [];
     if (info.windows) {
       const f = formatResetTime(info.windows.fiveHour.resetsAt, nowMs);
       const s = formatResetTime(info.windows.sevenDay.resetsAt, nowMs);
       if (f) parts.push(`5h ${f}`);
       if (s) parts.push(`7d ${s}`);
     }
     const updated = formatUpdatedAt(info.updatedAt);
     if (updated) parts.push(`上次更新 ${updated}`);
     return parts.join(" · ");
   }
   ```
4. **测试同步**：`plan-balance-model.test.ts` 约 20 例：currencySymbol 3（CNY/USD/未知原样）；planLogoSrc 1；formatResetTime 8（<1h 上取整/整 1h 边界归 <24h 档/Xh Ym/m=0 省略/≥24h 绝对无前导零/缺失 null/非法串 null/未来跨天）；formatUpdatedAt 2（正常/0→null）；rowText 4（四场景）；rowTooltip 4（kimi 全段/单窗 resetTime 缺失省略/deepseek 仅上次更新/frozen 与无数据固定文案）
5. **文档同步**：无
6. **验证**：`npx vitest run plan-balance-model` 全绿

## PB-FE-06 usePlanBalance hook + PlanBalanceFooter + NavTree 挂载 + 组件测试

1. **位置**：
   - 新建 `src/features/navTree/usePlanBalance.ts`、`src/features/navTree/PlanBalanceFooter.tsx`
   - 改 `src/features/navTree/NavTree.tsx:653-655`（树区 div 闭合与「添加项目」钮注释之间）
   - 新建 `src/__tests__/plan-balance-footer.test.tsx`
2. **现状**：NavTree.tsx:628-653 树区 div，:655-672 底部「添加项目」钮（`borderTop: 1px solid ${SEPARATOR_BG}`, flexShrink: 0）
3. **修复步骤**：
   1. `usePlanBalance.ts`：
      ```ts
      // F10 余量数据 hook：挂载拉一次 + 订阅 plan-balance-updated + 点击刷新（前端节流 5s，D7）
      import { useCallback, useEffect, useRef, useState } from "react";
      import type { PlanBalanceInfo } from "../../types/planBalance";
      import {
        getPlanBalance,
        onPlanBalanceUpdated,
        refreshPlanBalance,
      } from "../../ipc/planBalance";

      /** 点击刷新节流窗口（规格 §6：连点在窗口内忽略） */
      export const REFRESH_THROTTLE_MS = 5_000;

      export function usePlanBalance() {
        const [items, setItems] = useState<PlanBalanceInfo[]>([]);

        useEffect(() => {
          let cancelled = false;
          getPlanBalance()
            .then((v) => { if (!cancelled) setItems(v); })
            .catch((e) => { console.error("get_plan_balance 初始拉取失败", e); });
          const unlisten = onPlanBalanceUpdated(setItems);
          return () => { cancelled = true; unlisten(); };
        }, []);

        const lastRefreshRef = useRef(0);
        const refresh = useCallback(() => {
          const now = Date.now();
          if (now - lastRefreshRef.current < REFRESH_THROTTLE_MS) return;
          lastRefreshRef.current = now;
          refreshPlanBalance()
            .then(setItems)
            .catch((e) => { console.error("refresh_plan_balance 失败，保留旧值", e); });
        }, []);

        return { items, refresh };
      }
      ```
   2. `PlanBalanceFooter.tsx`（U1：树与添加项目钮之间；无命中来源整块不渲染含发丝线）：
      ```tsx
      // F10 套餐余量 footer——树滚动区与「添加项目」钮之间的固定区（U1）
      // 全部来源无展示 → 整块不渲染（含发丝线，规格 §8.3）；行点击 = 立即刷新（节流 5s）
      import React, { useState } from "react";
      import type { PlanBalanceInfo } from "../../types/planBalance";
      import { DIM_FG, SEPARATOR_BG } from "../../theme";
      import { nameStyle, ROW_HEIGHT, rowBaseStyle } from "./navStyles";
      import { planLogoSrc, rowText, rowTooltip } from "./planBalanceModel";
      import { usePlanBalance } from "./usePlanBalance";

      const PlanBalanceRow: React.FC<{ info: PlanBalanceInfo; onRefresh: () => void }> = ({
        info, onRefresh,
      }) => {
        const [hovered, setHovered] = useState(false);
        // logo 文件缺失 → onError 隐藏仅显文本（规格 §8.1 不裂图）
        const [logoFailed, setLogoFailed] = useState(false);
        return (
          <div
            data-e2e="plan-balance-row"
            onClick={onRefresh}
            onMouseEnter={() => setHovered(true)}
            onMouseLeave={() => setHovered(false)}
            title={rowTooltip(info, Date.now())}
            style={{
              ...rowBaseStyle(false, hovered, ROW_HEIGHT),
              color: DIM_FG, // fg-3（§8.1）
              fontSize: 12,
            }}
          >
            {!logoFailed && (
              <img
                src={planLogoSrc(info.planId)}
                width={14} height={14}
                style={{ flexShrink: 0, display: "block" }}
                alt=""
                onError={() => setLogoFailed(true)}
              />
            )}
            <span style={nameStyle}>{rowText(info)}</span>
          </div>
        );
      };

      export const PlanBalanceFooter: React.FC = () => {
        const { items, refresh } = usePlanBalance();
        if (items.length === 0) return null; // §8.3：整块（含发丝线）不渲染
        return (
          <div
            data-e2e="plan-balance-footer"
            style={{
              borderTop: `1px solid ${SEPARATOR_BG}`, // 发丝线（§8.1）
              flexShrink: 0,
              padding: "0 8px",
            }}
          >
            {items.map((info) => (
              <PlanBalanceRow key={info.sourceId} info={info} onRefresh={refresh} />
            ))}
          </div>
        );
      };
      ```
   3. NavTree.tsx 树区 div 之后、「{/* 底部「添加项目」钮…」注释之前插入：
      ```tsx
      {/* F10 套餐余量 footer（树与添加项目钮之间；无命中来源整块不渲染） */}
      <PlanBalanceFooter />
      ```
      import 区加 `import { PlanBalanceFooter } from "./PlanBalanceFooter";`
4. **测试同步**：`plan-balance-footer.test.tsx` 约 10 例（文件级 vi.mock 自定义 `../../ipc/planBalance` 实现接管 setup.ts 全局 mock）：
   - 四场景渲染（deepseek `¥12.34` / kimi `5h 62% · 7d 45%` / frozen `已冻结` / 占位 `--`）
   - 隐藏态：items=[] → footer 容器不存在
   - 初始拉取：挂载即调 getPlanBalance 并渲染返回值
   - 事件订阅：捕获 onPlanBalanceUpdated 回调 → 推新数组 → 行文案更新
   - 点击节流：连点两次 → refreshPlanBalance 仅调 1 次；`vi.spyOn(Date, "now")` 推进 5s+ 后再点 → 第 2 次调用；返回值更新行
   - logo：fireEvent.error(img) → img 消失文本保留；logo src = `/plan-icons/kimi.png`
   - tooltip：kimi 行 title 含「5h … 后重置 · 7d … · 上次更新」
5. **文档同步**：无（navTree/CLAUDE.md 在 PB-DOC-01）
6. **验证**：`npx vitest run plan-balance-footer` 全绿；`npm test` 全绿（含既有 nav-tree 用例零回归）

## PB-DOC-01 navTree/CLAUDE.md + ipc/CLAUDE.md 修订

1. **位置**：`src/features/navTree/CLAUDE.md`（行结构契约 / 硬约束 / 测试模式 / data-e2e 契约节）；`src/ipc/CLAUDE.md`（Event 模式节后）
2. **现状**：navTree CLAUDE.md 行结构契约无 footer 条目，data-e2e 契约四值；ipc CLAUDE.md Event 模式仅 `onFsEvent`/`onAgentEvent`
3. **修复步骤**：
   1. navTree CLAUDE.md 行结构契约节加一条：「套餐余量 footer（F10）：树滚动区与添加项目钮之间固定区（U1）；每来源一行 28px fg-3，logo 14px 缺失 onError 隐藏；全部来源无展示整块不渲染（含发丝线）；行点击 = 立即刷新（前端节流 5s）；颜色全 token 无例外」
   2. data-e2e 契约加 `plan-balance-footer` / `plan-balance-row`
   3. 测试模式节 L2 文件清单加 `plan-balance-model.test.ts` / `plan-balance-footer.test.tsx` / `ipc-plan-balance-contract.test.ts`
   4. ipc CLAUDE.md Event 模式段 `onFsEvent`/`onAgentEvent` 句加 `onPlanBalanceUpdated`；新增一小段「planBalance 命令（F10）」：`getPlanBalance` 挂载拉快照 / `refreshPlanBalance` 恒 Ok 返回最新快照（失败保留旧值，D6）/ `onPlanBalanceUpdated` 订阅 `plan-balance-updated`（有变化才推送，含 updatedAt 口径 D5）
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：grep `plan-balance-footer` navTree/CLAUDE.md 命中；grep `onPlanBalanceUpdated` ipc/CLAUDE.md 命中；文档描述与代码一致（Read 核对，防文档撒谎）

## PB-DOC-02 src-tauri/src/CLAUDE.md 白名单口径

1. **位置**：`src-tauri/src/CLAUDE.md`「settings.rs — 浅合并 + 保存互斥 + 白名单」节 SEC-11 行
2. **现状**：「顶层键白名单 `["fontSize", "keybindings", "sideBar", "colorScheme"]` + 序列化后大小上限 1MB」
3. **修复步骤**：白名单清单改五键（加 `planBalance`），补注「F10 轮询间隔（手改文件，读取侧 plan_balance 模块 resolve_poll_interval，越界回退 60s）」
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：grep `planBalance` src-tauri/src/CLAUDE.md 命中

## PB-DOC-03 test-inventory.md 登记（计数实跑校准 + 豁免）

1. **位置**：`.claude/test-inventory.md` 表头总数行 / 计数口径行 / 既定豁免清单表 / L1 表 / L2「IPC 层」表与「导航树」表
2. **现状**：全量 3634（Rust 742 + 前端 2710 + L3 142 + E2E 40）；豁免表四列格式；L1 34 文件、L2 156 文件
3. **修复步骤**：
   1. L1 表加 5 行（plan_balance 五文件各一行），settings.rs 行用例数 25→26，覆盖要点标 F10
   2. L2 IPC 层表加 `ipc-plan-balance-contract.test.ts` 行；「导航树」表加 `plan-balance-model.test.ts` / `plan-balance-footer.test.tsx` 两行；段小计、表头总数同步（**实跑取数**：`cargo test` 总数 + `grep -c '#\[test]'` 双核对、`npm test` Vitest 报告数）
   3. 豁免表加一行：「plan_balance 真实 HTTP 查询（ureq fetch）与 tokio 轮询任务本体 | 真实外部 API 依赖 + Tauri 运行时（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖（罐装 JSON/参数化编排）+ L2 UI 四场景 + 人工实测（真实账号一轮） | F10」
   4. 计数口径行 L1 文件数 34→39、L2 文件数 156→159（以实际新增为准核对）
4. **测试同步**：本项即登记
5. **文档同步**：本项即文档
6. **验证**：三处计数一致（表头/段小计/行级和）；豁免表新行四列齐全；新增行均含 F10 标记

## PB-DOC-04 CONTEXT.md 与需求索引核实（验证项，不改）

1. **位置**：`CONTEXT.md:240-253`；`.claude/CLAUDE.md:193`
2. **现状**（已实读核实）：四术语（编码套餐/套餐余量/用量窗口/余量来源）已登记；F10 已入需求编号索引
3. **修复步骤**：无改动——仅 verify 断言确认两处在 Stage 03 完成后仍与实现口径一致（术语「余量来源」描述与 source.rs 行为一致；F10 索引行描述与最终实现一致，若实现口径漂移则修订索引行描述）
4. **测试同步**：无
5. **文档同步**：条件性（仅漂移时）
6. **验证**：Read CONTEXT.md:240-253 与 CLAUDE.md:193，对照最终实现无失实
