# Stage 01 逐项验证断言（唯一真值源）

> stage-01-backend.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态：本 Stage 后前端未实现——不断言任何 src/ 前端产物；门禁五命令仍全跑（tsc/eslint/npm test 应零变化全绿）。

## 断言清单

### PB-BE-01 依赖

- **PB-BE-01-a**：grep `^ureq = \{ version = "3"` 命中 src-tauri/Cargo.toml，且同行含 `features = ["json"]`
- **PB-BE-01-b**：grep `tokio = \{ version = "1"` 行含 `"time"`（src-tauri/Cargo.toml）
- **PB-BE-01-c**：`cargo tree --manifest-path src-tauri/Cargo.toml -p ureq` 显示 3.x（本断言需 verify agent 执行该命令取数——构建级证据，cargo 命令排队属正常）

### PB-BE-02 mod.rs（DTO + 状态机 + 轮询/命令）

- **PB-BE-02-a**：src-tauri/src/plan_balance/mod.rs 存在；grep `pub struct PlanBalanceInfo` / `pub struct AmountInfo` / `pub struct WindowsInfo` / `pub struct WindowInfo` / `pub struct FetchOutcome` 五处命中；四 DTO 均带 `#[serde(rename_all = "camelCase")]`
- **PB-BE-02-b**：grep `static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>>` 命中 mod.rs（模块级静态，D4——**不得**出现在 state.rs/AppState 中，grep `PlanBalance` src-tauri/src/state.rs 零命中）
- **PB-BE-02-c**：grep `pub(crate) fn merge_slot` / `pub(crate) fn poll_once_with` / `pub(crate) fn resolve_poll_interval` / `pub fn start_plan_balance_poller` 命中 mod.rs
- **PB-BE-02-d**：grep `emit("plan-balance-updated"` 命中 mod.rs；Read `apply_snapshot` 确认变化才 emit（比较含 updated_at 的整体 PartialEq，D5）
- **PB-BE-02-e**：grep `pub async fn get_plan_balance` / `pub async fn refresh_plan_balance` 命中 mod.rs，且二者返回 `Result<Vec<PlanBalanceInfo>, AppError>`；Read refresh 确认恒 Ok 语义（D6：单来源失败不整体 Err，仅 spawn_blocking join 失败才 Err）
- **PB-BE-02-f**：serde 键集合测试存在且锁死六键——grep `planId` 与 `updatedAt` 命中 mod.rs 测试块；cargo test 输出中 plan_balance 模块用例全绿（据测试 agent 结果判定）
- **PB-BE-02-g**：Read resolve_poll_interval 确认：默认 60、合法区间 10–3600、越界/缺失/损坏回退默认（规格 §9 口径）

### PB-BE-03 source.rs

- **PB-BE-03-a**：src-tauri/src/plan_balance/source.rs 存在；grep `pub trait PlanSource` / `fn source_id` / `fn resolve` 命中；grep `static SOURCES` 命中且为静态切片形态（U2）
- **PB-BE-03-b**：grep `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` 命中 source.rs；Read resolve_env 确认四类 None 分支（JSON 损坏/env 缺失/BASE_URL 缺失/token 缺失或空）
- **PB-BE-03-c**：home 解析为模块内自建 home_dir + HomeDirGuard（grep `HOME_DIR_OVERRIDE` 命中 source.rs）；**不得**跨模块调用 hooks——grep `hooks::` src-tauri/src/plan_balance/ 零命中（D2，硬约束 #2）

### PB-BE-04 query.rs

- **PB-BE-04-a**：src-tauri/src/plan_balance/query.rs 存在；grep `pub trait PlanQuery` / `fn plan_id` / `fn base_urls` / `fn fetch` 命中；grep `static QUERIES` 命中
- **PB-BE-04-b**：grep `fn normalize_base_url` 命中；Read 确认实现仅 `to_lowercase()` + `trim_end_matches('/')`（**不得**含 trim/空白处理——规格 §4.2 字面）
- **PB-BE-04-c**：grep `fn find_query_by_url` 命中且签名参数化（接受 queries 切片参数，L1 可注入）；grep `fn http_agent` / `fn query_err` 命中
- **PB-BE-04-d**：Read query_err 确认错误消息只含 planId + 错误类别（HTTP 状态码/超时/网络错误），不拼 token

### PB-BE-05/06 双套餐

- **PB-BE-05-a**：src-tauri/src/plan_balance/deepseek.rs 存在；grep `https://api.deepseek.com/anthropic`（base_urls）与 `https://api.deepseek.com/user/balance`（fetch URL）命中；grep `fn parse_deepseek_balance` 命中；超时 5s（grep `from_secs(5)`）
- **PB-BE-06-a**：src-tauri/src/plan_balance/kimi.rs 存在；grep `https://api.kimi.com/coding` 与 `/coding/v1/usages` 命中；grep `fn parse_kimi_usages` / `fn remaining_percent` 命中；超时 8s（grep `from_secs(8)`）
- **PB-BE-06-b**：Read parse_kimi_usages 确认四语义：① `totalQuota.used == "1"` → frozen 且不要求窗口解析；② 5h 窗 duration==300 && timeUnit=="TIME_UNIT_MINUTE" 优先否则 limits[0]；③ 7d = 顶层 usage；④ 非触顶任一窗失败整体 Err
- **PB-BE-06-c**：Read remaining_percent 确认按**字符串**解析 used/limit（规格口径）+ clamp 0–100 + limit<=0 返回 None（D13）

### PB-BE-07 三处注册 + 白名单

- **PB-BE-07-a**：三处注册齐全——grep `plan_balance::get_plan_balance` 与 `plan_balance::refresh_plan_balance` 命中 src-tauri/src/lib.rs（generate_handler! 内）；grep `"get_plan_balance"` 与 `"refresh_plan_balance"` 命中 src-tauri/build.rs；grep `allow-get-plan-balance` 与 `allow-refresh-plan-balance` 命中 src-tauri/capabilities/default.json（D11，缺一判 not_fixed）
- **PB-BE-07-b**：grep `mod plan_balance;` 命中 lib.rs；grep `start_plan_balance_poller` 命中 lib.rs setup 块
- **PB-BE-07-c**：build.rs 注释计数改「当前 36 条」（grep `36` 命中 build.rs 注释行）
- **PB-BE-07-d**：grep `&str; 5` 命中 src-tauri/src/settings.rs，且白名单含 `"planBalance"`；grep `save_accepts_plan_balance_key` 命中 settings.rs 测试块

### PB-BE-08 模块文档

- **PB-BE-08-a**：src-tauri/src/plan_balance/CLAUDE.md 存在，且含「token」「静态切片」「豁免」关键节（grep 三词命中）；Read 确认按子文件模板成文（存在理由/关键约束与决策/外部坑红线/测试模式），不含「职责」「文件表」式内容
- **PB-BE-08-b**：grep `plan_balance` 命中 .claude/CLAUDE.md 模块索引表（新增行）

### 红线（语义式，不限变量名，须 Read 代码确认）

- **SEC-TOKEN-a**：src-tauri/src/plan_balance/ 全部文件中，DTO/serde 序列化结构不存在任何 token 字段（Read mod.rs 四 DTO 字段清单确认仅 checklist 定义的形态）
- **SEC-TOKEN-b**：plan_balance 模块内所有 `tracing::` 调用点与 `Err`/`AppError` 构造点，消息不插值 token 与 Authorization 头（Grep `tracing::` 与 `AppError::` 逐处 Read 确认——ureq 错误 Display 不含请求头，禁止自行拼接）
- **SEC-CFG**：plan_balance 模块无业务 `#[cfg(windows)]`（grep 零命中；硬约束 #9——`#[cfg(test)]` 不受限）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
