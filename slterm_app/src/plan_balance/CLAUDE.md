# 套餐余量（plan_balance）

> 查询套餐余量、归一化为快照并经 watch channel 推送；凭据与网络边界集中在本模块。

## 决策与红线

- **token 不出凭据域**：DTO/快照/推送载荷无 token 字段（类型层防线）；tracing/错误消息禁止插值 token 与 Authorization 头；测试夹具一律假值；真实凭据只存 user 层 `~/.claude/settings.json`（仓库外）。原因：凭据泄露不可逆。边界：新增 DTO 字段先检查是否可能承载凭据。
- **凭据通道边界**：plan_balance 只读 user 层 settings.json 的 `env` 段，不读 Windows 凭据管理器；由 `plan_balance_reads_user_settings_not_the_credential_store` 源扫描锁死。
- **查询语义**：URL 归一化只小写化 + 去尾斜杠（不加 trim）；kimi 实证红线（冻结判定、totalQuota 无 used、5 小时窗 detail 内层、remaining 恒在而 used 可缺、数值按字符串解析、全有或全无）；真实响应快照锚点 `parse_real_response_snapshot` 防 API 漂移。
- **快照变化与推送**：整体 `PartialEq`（含 updated_at）有变化才推；失败保留旧值不推；`refresh_plan_balance` 恒 Ok，错误不冒泡到 UI。
- **推送形态 = 模块级 watch channel**：多消费者各自订阅，值本体在 `SNAPSHOT`；有界 1 槽合并连续推送，唤醒后读最新快照。原因：多消费者不应耦合组件树。边界：订阅后先读 `current_snapshot()`，覆盖「落槽后、扇出前」的漏唤醒窗口。
- **执行面**：`poll_once_executor: fn()` 由 `background_tasks` 的 `planBalance` 条目（默认 10 秒，合法 10–3600 秒）驱动；阻塞 HTTP（ureq）全程在 GPUI background executor，不占 UI 线程。
- **`SecretBytes::as_bytes` 借出白名单**：仅 `secrets.rs`（句柄语义用例）与 `plan_balance/deepseek.rs`、`plan_balance/kimi.rs`（请求头组装）；新增调用点须先登记凭据域。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 本地 HTTP 服务覆盖请求形态/响应解析/错误映射（`query.rs` 的 `serve_once`，单请求，不出网）；`kimi.rs`/`deepseek.rs` 覆盖请求头与解析。
- 编排、合并、推送与订阅语义由单元层覆盖。
- 真实 provider API 出网由构建产物人工实测，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 启动接线：`app_shell::run_shell` → `start_background_poll`（注册表订阅 + background executor 循环；首轮立即执行，后续按注册表节拍；任务 Drop = 退订；enabled 门控走 `background_tasks`）。
- UI 消费：`current_snapshot()` / `subscribe()` / `refresh_plan_balance()`。
- 新增 provider：实现解析与请求头组装，补本地 HTTP 用例与真实响应快照。
