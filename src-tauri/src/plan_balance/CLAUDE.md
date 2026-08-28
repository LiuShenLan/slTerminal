# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/plan_balance` 是 F10 编码套餐余量查询模块：读 user 层 `~/.claude/settings.json` 的 env 判定套餐（当前 deepseek/kimi 两家的 Anthropic 兼容端点），定时轮询外部 API，把余量快照推给前端。外部 API 语义（两套餐响应结构差异、kimi 月限额触顶态、全有或全无解析）、token 安全红线与轮询编排口径无法从代码自证，必须文档化。

## 关键约束与决策

### 注册表形态 = 静态切片（U2，偏离硬约束 #13 可变单例）

照 `hooks/provider.rs` 先例：`source.rs` 的 `SOURCES` 与 `query.rs` 的 `QUERIES` 均为静态切片。偏离 #13 可变单例的理由：Rust 无 side-effect import（#13 先例全为前端模块），测试经参数化查找（`find_query_by_url` 注入）覆盖，「一行注册」目标不变——新增套餐 = 新实现 + `QUERIES` 一行。

### 快照存储 = 模块级静态（D4）

`SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>>` 不入 `AppState`——照 `hooks/mod.rs` WATCHER 先例（避免 state.rs 循环依赖）；快照仅本模块读写，非跨模块共享。

### emit 口径含 updated_at（D5）

快照整体 `PartialEq` 比较（**含 updated_at**）：成功查询必刷新 updated_at 即视为变化 → emit；失败保留旧值 → 不 emit；来源集合变化 → emit。规格 §6「有变化才 emit」+ §7 updated_at=「最近成功查询」+ §8.2 tooltip「上次更新」三方一致的唯一口径。

### refresh 恒返回 Ok（D6）

`refresh_plan_balance` 恒返回 `Ok(最新快照)`：单来源失败按 §6 保留旧值，不整体 Err；仅 spawn_blocking join 失败才 Err。前端用返回值直接更新，事件通道照常。

### home 解析自建不跨模块（D2）

`source.rs` 自建 `home_dir()` + `HomeDirGuard`（照抄 `hooks/claude/mod.rs` 模式），**禁止跨模块调用** `hooks::claude`——硬约束 #2 模块不穿透；应用 settings 读取经 `crate::app_dir::app_data_dir()`（顶层共享件，不构成穿透，D3）。

### 轮询间隔与轮询任务

`resolve_poll_interval` 读应用 settings.json 的 `planBalance.intervalSec`：默认 60，合法 10–3600，越界/缺失/损坏回退默认。`start_plan_balance_poller` 由 lib.rs setup 调用，`tokio::time::interval` 首次 tick 立即执行第一轮（D8）；随进程退出结束，单实例无生命周期管理。

### 轮询间隔键须在白名单

`planBalance` 为 settings 白名单第 5 键（SEC-11，settings.rs）——手改文件，读取侧在本模块，白名单回归由 settings.rs `save_accepts_plan_balance_key` 守卫。

## 外部坑/红线

- **token 不出后端**：DTO 六键 serde 键集合精确匹配测试锁死（无 token 字段）；本模块所有 tracing!/Err 构造消息禁止插值 token 与 Authorization 头（ureq 错误 Display 不含请求头，构造错误消息时禁止自行拼接）。
- **URL 归一化只小写化 + 去尾斜杠**（规格字面）：不加 trim，`trim_end_matches('/')` 不处理空白。
- **kimi 数值字段按字符串解析**（规格 §5.2 口径）：`used`/`limit`/`totalQuota.used` 均按字符串读——实证偏差（如 API 返回数字）走人工实测确认，不擅自放宽。
- **kimi 全有或全无**：非触顶时任一窗口解析失败 → 整体 Err（防窗口重置瞬间 limits 不完整致 5h 段丢失）。
- **kimi 触顶判定**：`totalQuota.used == "1"`（字符串比较）→ frozen=true，不要求窗口解析成功。
- **fetch 必须 spawn_blocking**：ureq 是纯阻塞 HTTP，硬约束 #3。

## 测试模式

- `merge_slot` / `poll_once_with` / `resolve_poll_interval` 全部参数化/注入，L1 不触网不触盘：`poll_once_with` 的 resolve/fetch 闭包注入；`resolve_poll_interval` 经 `AppDataDirGuard` 注入 tempdir。
- 解析纯函数（`parse_deepseek_balance` / `parse_kimi_usages` / `resolve_env`）罐装 JSON 全测。
- serde 键集合精确匹配（照 hooks/mod.rs `assert_status_key_set` 先例）——token 红线守卫。
- `get_plan_balance` 命令核心经 current_thread runtime block_on 直测（照 hooks/mod.rs:443 先例）。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| plan_balance 真实 HTTP 查询（ureq fetch）与 tokio 轮询任务本体 | 真实外部 API 依赖 + Tauri 运行时（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖（罐装 JSON/参数化编排）+ L2 UI 四场景 + 人工实测（真实账号一轮）——登记于 test-inventory（F10） |
