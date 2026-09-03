# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/plan_balance` 是 F10 编码套餐余量查询模块：读 user 层 `~/.claude/settings.json` 的 env 判定套餐（当前 deepseek/kimi 两家的 Anthropic 兼容端点），定时轮询外部 API，把余量快照推给前端。外部 API 语义（两套餐响应结构差异、kimi 配额耗尽冻结态、全有或全无解析、2026-08 实证修正的字段漂移）、token 安全红线与轮询编排口径无法从代码自证，必须文档化。

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

### 轮询编排已上提 background_tasks（F12），本模块保留执行体

轮询通用件（间隔内存原子量/首轮立即执行/每轮末按当前内存间隔 sleep/读盘钳制初始化）随 F12 上提 `src-tauri/src/background_tasks` 骨架，本模块只保留套餐语义执行体 `poll_once_executor`（一轮拉取 + 快照应用，resolve/fetch/merge/emit 口径含 updated_at 比较不变）与快照存储。配置机制随之变化：

- `plan_balance_set_interval` 命令退役——间隔/enabled 统一经 `background_tasks_set_config` 写通道配置（注册表元数据：默认 10s，合法 10–3600，默认启用，见 `registry.rs` TASKS）；
- enabled=false 停轮询：poller 轮首检查退出循环，**快照保留**（前端 footer 经 `background-tasks-updated` 事件感知隐藏；重启用即重显最后快照）；
- 默认间隔 60 → 10s（骨架注册表值，越界/缺失/损坏钳制回退默认随之）；
- 行为不变：每轮末按当前内存间隔 sleep，运行期改值下一轮即生效。

### 配置键归域已上提 background_tasks

`SETTINGS_KEY`（= "backgroundTasks"）随 F12 上提 `background_tasks::registry`，settings.rs 白名单第 5 键经 `crate::background_tasks::SETTINGS_KEY` 常量引用（防字面量漂移）；白名单守卫由 settings.rs `save_accepts_background_tasks_key`（放行）与 `save_rejects_plan_balance_key`（旧 `planBalance` 键退役防回归）承担。本模块不再持有任何 settings 键常量。

## 外部坑/红线

- **token 不出后端**：DTO 六键 serde 键集合精确匹配测试锁死（无 token 字段）；本模块所有 tracing!/Err 构造消息禁止插值 token 与 Authorization 头（ureq 错误 Display 不含请求头，构造错误消息时禁止自行拼接）。测试夹具 token 一律假值占位符（`sk-test` 形态，SEC-18）；真实凭据只经 `source.rs` 读 user 层 `~/.claude/settings.json`（仓库外），禁止以真实值替换夹具或写入任何 git 追踪文件。
- **URL 归一化只小写化 + 去尾斜杠**（规格字面）：不加 trim，`trim_end_matches('/')` 不处理空白。
- **kimi 结构实证（2026-08-28 curl 实测 + 社区审计，修正规格 §5.2 假定）**：`GET /coding/v1/usages` + `Authorization: Bearer`（非 X-Kimi-Authorization）；5h 窗数值（`used`/`limit`/`remaining`/`resetTime`）承载于 **`limits[i].detail` 内层**（外层无）；7d 窗为顶层 `usage` 对象；`remaining` 恒在、`used` 可缺（两种账号形态均实证，事实不变）→ **展示口径 = 已用百分比（2026-09 起，用户偏好展示已用量而非剩余）：`used` 优先**（used/limit×100）、**remaining 换算回退**（(limit−remaining)/limit×100）——回退路径可行恰因「remaining 恒在」实证；`.round()` + clamp 0–100 保留；limit 缺失/≤0/不可解析 → 该窗口失败（全有或全无）。`totalQuota` **无 `used` 字段**（实测可为空对象 `{}`）。真实响应快照锚点：`kimi.rs::parse_real_response_snapshot`。
- **kimi 数值字段按字符串解析**（实证口径）：`used`/`limit`/`remaining` 均为字符串，`.as_str()` 读取——若 API 返回数字形态，须先实测确认再放宽。
- **kimi 全有或全无**：非冻结时任一窗口解析失败 → 整体 Err（防窗口重置瞬间 limits 不完整致 5h 段丢失）。
- **kimi 冻结判定（实证修正）**：`totalQuota.remaining` 字符串 parse 后 ≤ 0 → frozen=true（配额耗尽冻结）；totalQuota 缺失/空对象/remaining 非数字或非 0 → 未冻结；冻结时不要求窗口解析成功（windows=None 仍 Ok）。
- **fetch 必须 spawn_blocking**：ureq 是纯阻塞 HTTP，硬约束 #3。

## 测试模式

- `merge_slot` / `poll_once_with` 全部参数化/注入，L1 不触网不触盘：`poll_once_with` 的 resolve/fetch 闭包注入。执行体 `poll_once_executor` 与轮询循环本体由 background_tasks 骨架驱动（emit 在 `apply_snapshot` 内，需 AppHandle），其 L1 豁免登记于 background_tasks/CLAUDE.md。
- 解析纯函数（`parse_deepseek_balance` / `parse_kimi_usages` / `resolve_env`）罐装 JSON 全测。kimi 解析含真实响应快照锚点（`parse_real_response_snapshot`，防下次 API 漂移）+ 双形态变体（detail 含/不含 used、totalQuota 缺失/空对象/非数字）。
- serde 键集合精确匹配（照 hooks/mod.rs `assert_status_key_set` 先例）——token 红线守卫。
- `get_plan_balance` 命令核心经 current_thread runtime block_on 直测（照 hooks/mod.rs:443 先例）。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| plan_balance 真实 HTTP 查询（ureq fetch） | 真实外部 API 依赖（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖（罐装 JSON/参数化编排）+ L2 UI 四场景 + 人工实测（真实账号一轮）——登记于 test-exemptions（F10） |
