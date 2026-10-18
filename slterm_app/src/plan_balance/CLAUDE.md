# 套餐余量(plan_balance)

## 决策与红线

- **token 不出凭据域(SEC-18)**:DTO/快照/推送载荷无 token 字段(类型层防线);
  tracing/错误消息禁止插值 token 与 Authorization 头;测试夹具一律 `sk-test`
  假值;真实凭据只存 user 层 `~/.claude/settings.json`(仓库外)。
- **双通道裁定**:plan_balance 只读 user 层 settings.json 的 `env` 段,不读
  Windows 凭据管理器;ai_providers 走凭据库且不读 user 层文件。两侧由
  `plan_balance_reads_user_settings_not_the_credential_store` 源扫描锁死。
- **查询语义原样(F10)**:URL 归一化只小写化 + 去尾斜杠(不加 trim);kimi
  实证红线(冻结判定 / totalQuota 无 used / 5h 窗 detail 内层 / remaining
  恒在、used 可缺 / 数值按字符串解析 / 全有或全无);真实响应快照锚点
  `parse_real_response_snapshot` 防 API 漂移。
- **快照槽四态 + emit 判定**:整体 `PartialEq`(含 updated_at)有变化才推;
  失败保留旧值不推;`refresh_plan_balance` 恒 Ok,错误不冒泡到 UI。
- **推送形态 = 模块级 watch channel(D10-3)**:多消费者各自订阅,值本体在
  `SNAPSHOT`;有界 1 槽合并连续推送,唤醒后读最新快照。
- **执行面**:`poll_once_executor: fn()` 由 09 篇 `background_tasks` 的
  `planBalance` 条目(默认 10s,合法 10–3600s)驱动;阻塞 HTTP(ureq)全程在
  GPUI background executor,不占 UI 线程。
- **`SecretBytes::as_bytes` 借出白名单(B60)**:仅 `secrets.rs` /
  `ai_providers.rs`(句柄语义用例)与 `plan_balance/deepseek.rs` /
  `plan_balance/kimi.rs`(请求头组装);新增调用点须先登记 10 篇。

## 步骤接缝

- 壳启动链已接(M10.12):`app_shell::run_shell` → `start_background_poll`
  (注册表订阅 + GPUI background executor 循环;首轮立即,后续按注册表节拍;
  任务 Drop = 退订;enabled 门控走 `background_tasks` 原子量)。
- UI 展示/刷新入口消费 `current_snapshot()` / `subscribe()` /
  `refresh_plan_balance()`,归 06 设置页/HUD。`subscribe()` 同时返回订阅
  那一刻的当前快照(B64),注册与首推之间不丢值。
- 真实 provider API 出网豁免登记于 `.claude/test-exemptions.md`(M10.11 条)。
