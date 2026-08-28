# F10 编码套餐余量展示 — 需求规格

## 1. 背景与目标

在导航树视图（nav 侧栏视图）底部固定区域展示当前编码套餐的余量，让用户不切出终端即可感知剩余额度/用量。

判定链：读取 user 层 `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL` → 命中套餐（Coding Plan）→ 按套餐对应的查询方式（硬编码 URL + 解析逻辑）用 `env.ANTHROPIC_AUTH_TOKEN` 查询 → 展示。

## 2. 术语

新术语（同步登记 `CONTEXT.md`）：

- **编码套餐**（Coding Plan）：编码 CLI 背后的计费订阅方（deepseek / kimi），由 user 层 settings.json 的 `env.ANTHROPIC_BASE_URL` 判定。
- **套餐余量**（Plan Balance）：套餐当前剩余可用量。两种形态：金额余额 / 时间窗用量。
- **用量窗口**（Usage Window）：时间窗计费的限流窗口（5 小时滚动窗 / 7 天窗），含剩余百分比与重置时间。
- **余量来源**（Plan Source）：判定套餐的配置文件来源。v1 仅 claude user 层 settings.json；结构上可扩展（未来 codex 等其他 CLI 的配置文件）。

## 3. 范围

**做**：deepseek 金额余额、kimi 双时间窗用量、点击手动刷新、轮询间隔可配置（手改配置文件）。

**不做（非目标）**：

- project / local 层 settings.json 合并（仅 user 层，已知限制见 §11）
- 独立设置 GUI
- 多币种账户的逐币种罗列（仅取 `balance_infos[0]`）
- E2E（L4）测试（真实网络依赖，登记豁免）

## 4. 数据源与套餐判定

### 4.1 余量来源（Plan Source）

- v1 唯一来源：`~/.claude/settings.json`（user 层），提取 `env.ANTHROPIC_BASE_URL` 与 `env.ANTHROPIC_AUTH_TOKEN`。
- home 目录解析复用 hooks 模块 `home_dir()` 先例（含测试注入守卫模式）。
- 来源抽象为 trait：输入 = 无，输出 = `Option<(baseUrl, token)>`；解析格式（JSON/TOML/ini）由各来源实现自定，对判定链透明。

### 4.2 BASE_URL → 套餐匹配

- 每个套餐声明 **URL 匹配集**（`baseUrls: string[]`）：
  - deepseek：`["https://api.deepseek.com/anthropic"]`
  - kimi：`["https://api.kimi.com/coding"]`
- 匹配前归一化：**小写化 + 去尾部斜杠**，然后与匹配集逐项精确相等比较。
- 同一套餐可有多个 URL 别名（用户明示后续会出现）；新增别名 = 往匹配集加一行。
- 未命中任何套餐 → 该来源无展示（静默降级）。

## 5. 套餐查询实现

每个套餐一个查询实现，统一输出 `PlanBalanceInfo`（§7 DTO）。HTTP 客户端：**ureq**（纯阻塞、无 tokio 依赖；调用走 `spawn_blocking`，硬约束 #3）。

### 5.1 deepseek

- 请求：`GET https://api.deepseek.com/user/balance`，头 `Accept: application/json`、`Authorization: Bearer <token>`，超时 5s。
- 解析：取 `balance_infos[0]` 的 `total_balance`（字符串）与 `currency`（如 `"CNY"`/`"USD"`）。`balance_infos` 为空或字段缺失 → 本次查询失败。
- 货币符号映射在前端（纯函数）：`CNY→¥`、`USD→$`、未知 → 原货币代码。

### 5.2 kimi

- 请求：`GET https://api.kimi.com/coding/v1/usages`，头 `Authorization: Bearer <token>`，超时 8s。
- 窗口定位（沿用 kimi-usage.js 参考语义）：
  - **5 小时窗**：`limits[]` 中 `window.duration == 300 && window.timeUnit == "TIME_UNIT_MINUTE"` 者优先；否则取 `limits[0]`。
  - **7 天窗**：响应顶层 `usage` 字段。
  - 各窗口百分比 = `used / limit`（数值字段为字符串，统一转换）；`limit` 缺失或为 0 → 该窗口解析失败。
  - 重置时间：5h 取窗口 `detail.resetTime`，7d 取 `usage.resetTime`（ISO 字符串，可缺失）。
- **月限额触顶态**：`totalQuota.used == "1"` 表示月限额触顶、账号冻结 → `frozen = true`，此时不要求窗口解析成功。
- **全有或全无**：非触顶时任一窗口解析失败 → 本次拉取整体失败（保留旧值），防止 API 在窗口重置瞬间返回不完整 `limits` 导致 5h 段丢失。

## 6. 轮询与推送

- **后端 tokio 定时任务**，随应用启动启动、随进程退出结束（单窗口单实例，无额外生命周期管理）。
- 每个周期：重读 settings.json（运行期改配置最迟一个周期内生效）→ 判定套餐 → 查询 → 与上一快照比较，**有变化才 emit** Tauri 事件。
- 事件：`plan-balance-updated`，payload = `PlanBalanceInfo[]`（全部命中来源，按注册序）。
- 前端：挂载时 invoke `get_plan_balance` 拉一次当前快照 + 订阅事件。
- **点击余量行 = 立即刷新**（invoke `refresh_plan_balance`），节流 5s（连点在节流窗口内忽略）。
- **查询失败策略**：网络失败 / 超时 / 非 200 / 解析失败 → 保留上次成功值静默重试（与 kimi-usage.js 快照策略一致）。
- **套餐切换**：planId 变化时丢弃旧值（不残留旧套餐数字）；从有套餐变为无套餐 → 该来源从数组移除（前端隐藏该行）。

## 7. DTO（双边对应，硬约束 #4）

Rust `snake_case` ↔ TS `camelCase`：

```rust
struct PlanBalanceInfo {
    source_id: String,          // 来源标识，v1 恒 "claude"
    plan_id: String,            // "deepseek" | "kimi"
    frozen: bool,               // kimi 月限额触顶
    amount: Option<AmountInfo>,       // 金额形态（deepseek）
    windows: Option<WindowsInfo>,     // 时间窗形态（kimi）
    updated_at: u64,            // 最近成功查询的 unix 秒
}
struct AmountInfo { value: String, currency: String }   // value 原样透传 total_balance
struct WindowsInfo { five_hour: WindowInfo, seven_day: WindowInfo }
struct WindowInfo { remaining_percent: u8, resets_at: Option<String> }  // 剩余 = 100 - used
```

**安全红线**：token 不出后端——DTO 不含 token；后端日志（含 tracing 各 level）禁止打印 token 与完整 Authorization 头。

## 8. UI 规格

### 8.1 位置与结构

- **导航树视图内部底部固定区**：`NavTree` 组件内树滚动区之下的固定 footer，不随树滚动。
- 视觉：顶部 1px 发丝线（默认档 `rgba(255,255,255,0.055)`）分隔；每来源一行，行高 28（同导航树行契约）；文本 fg-3（`DIM_FG`）；全部颜色走 `theme/colors.ts` token（硬约束 #6，无新例外）。
- logo 14px（同会话行 logo 尺寸），路径 `public/plan-icons/<planId>.png`（与 `public/cli-icons/` 隔离）。logo 文件缺失时省略 logo 仅显示文本（不裂图）。

### 8.2 展示文案

| 场景 | 行内容 | tooltip |
|------|--------|---------|
| deepseek 正常 | `[logo] ¥12.34` | 「上次更新 HH:mm:ss」 |
| kimi 正常 | `[logo] 5h 62% · 7d 45%` | 「5h 3h42m 后重置 · 7d 9月2日 14:00 重置 · 上次更新 HH:mm:ss」（resetTime 缺失的窗口省略对应段） |
| kimi 触顶 | `[logo] 已冻结` | 「月限额触顶，Kimi Code 已冻结」 |
| 首次查询尚无成功值 | `[logo] --` | 「查询中 / 查询失败重试中」 |

- 百分比 = **剩余**（100 − 已用）。
- 重置时间格式：距现在 <24h → 相对（`Xh Ym 后重置`，<1h 显 `Xm 后重置`）；≥24h → 绝对（`M月d日 HH:mm 重置`）。
- 行点击 = 立即刷新（节流 5s）；cursor: pointer。

### 8.3 隐藏态（整块/整行不渲染）

- settings.json 不存在、`env` 段缺失、BASE_URL 缺失或未匹配任何套餐；
- token 缺失或为空；
- 多来源时按行独立判定，全部来源无展示 → 整个 footer 区（含发丝线）不渲染。

## 9. 配置

- 应用 settings.json（exe 同级，便携分发）新增键 `planBalance: { "intervalSec": 60 }`；`SETTINGS_ALLOWED_KEYS` 白名单加 `planBalance`（第 5 键）。
- 默认 60 秒；合法范围 10–3600，越界/缺失回退默认。无 GUI，手改文件后由读取侧生效（启动时读取；运行期改动最迟在下次前端 loadFromDisk 场景生效——v1 接受重启生效）。
- 间隔变更后定时任务按新间隔重建（读取侧在启动时一次性应用）。

## 10. 扩展性设计（注册表家族契约，硬约束 #13）

两个后端注册表 + 前端 logo 约定：

1. **PlanSourceRegistry**（后端）：来源 trait（`fn resolve(&self) -> Option<(baseUrl, token)>`）。新增来源（如 codex 配置文件）= 新实现 + 一行注册。
2. **PlanQueryRegistry**（后端）：查询 trait（`fn base_urls(&self) -> &[String]` + `fn fetch(&self, token) -> Result<PlanBalanceInfo>`）。新增套餐 = 新实现 + 一行注册。
3. **前端 logo**：`public/plan-icons/<planId>.png` 路径约定，前端按 DTO `planId` 拼路径，无需映射表。

## 11. 已知限制

- 仅读 user 层 settings.json：项目经 project/local 层覆盖 `ANTHROPIC_BASE_URL` 时，展示的仍是 user 层套餐，可能与该项目实际计费方不一致。
- 单窗口单实例前提下无多实例并发写问题；settings.json 只读不写（本功能），与 hooks 注入的写入无冲突。

## 12. 测试策略（硬约束 #11）

- **L1（Rust）**：URL 归一化与匹配集判定（大小写/尾斜杠/别名/未命中）；deepseek 响应解析（罐装 JSON：正常/空 balance_infos/缺字段/USD）；kimi 响应解析（罐装 JSON：双窗口正常/300min 优先与 limits[0] 回退/limit 为 0/触顶/窗口不完整整体失败）；剩余百分比换算；失败保留旧快照与 planId 切换丢弃逻辑；token 缺失 → None。定时任务与真实网络调用登记豁免（test-inventory，原因：真实外部 API 依赖；兜底 = 解析与状态机 L1 全覆盖 + L2 UI）。
- **L2（前端）**：行渲染四场景（金额/双窗/触顶/`--`）、tooltip 文案与重置时间格式化、隐藏态矩阵、点击刷新节流、货币符号映射、`planId→logo` 路径拼接、事件订阅 + 初始拉取。
- **用例清单同步**：新增用例登记 `.claude/test-inventory.md`。
