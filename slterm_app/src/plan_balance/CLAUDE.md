# 套餐余量（plan_balance）

> DeepSeek/Kimi 响应解析与展示值类型。

## 决策与红线

- **纯解析边界**：解析函数只接受 `serde_json::Value`，不发现来源，不读取 CLI 配置或环境认证材料，不接触凭据、网络与调度。原因：响应算法不得与旧认证通道、全局状态或路径推导耦合。边界：请求与凭据属于独立执行层，不得混入解析函数。
- **结果不含凭据**：结果类型只承载冻结态、金额与时间窗，不含 token；错误消息不得插值认证材料。测试只使用罐装响应，不读取真实账号或配置。
- **DeepSeek 语义**：从 `balance_infos[0]` 读取 `total_balance` 与 `currency`；空数组、字段缺失或非字符串均失败。
- **Kimi 语义**：采用 2026-08 实测口径；冻结由 `totalQuota.remaining <= 0` 判定，5 小时窗读取 `limits[i].detail`，7 天窗读取顶层 `usage`。`used` 优先，`remaining` 换算回退；非冻结时任一窗口失败则整体失败。真实响应快照锚点为 `parse_real_response_snapshot`。
- **缺失不造数**：字段缺失或不可解析返回错误，不把缺失数据伪造为 0；百分比输出夹到 0–100。
- **调度状态机**：`service::PlanBalanceService` 按来源维护凭据代次、在途时间、最后成功、下次到期、结果与错误；自动准入要求已启用且到期且无在途，手动全量刷新按实际发起时间计算 5 秒冷却。过期只在「错过完整周期且无新成功结果」时成立；请求未超时且未错过周期时保持查询中；失败保留旧结果并记录错误，不推进最后成功时间。凭据替换或删除递增代次并清空旧结果，迟到回执丢弃。原因：防重入、冷却与过期判定必须有单一内存事实，旧账号结果不得回填。边界：服务不执行网络、不持定时器、不感知导航可见性；周期夹在 30–3600 秒，结果只存内存。`client::PlanBalanceClient` 只接受 `PlanBalanceSource` 的固定 HTTPS 端点（DeepSeek `/user/balance`、Kimi `/coding/v1/usages`），token 只来自该来源的专用凭据目标，经 `Authorization: Bearer` 在请求组装点使用；零重定向、8 秒总超时、响应上限 1 MiB，非 200 与超预算明确失败且不猜内容。原因：套餐查询不得读取 CLI 认证材料，也不得被设置或环境改写端点。边界：错误只含封闭分类，不含 token、路径或正文；回环端点注入只存在于测试构建；真实服务出网只作人工验收。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 罐装 JSON 覆盖正常值、缺失字段、类型错误、冻结态、双窗选择、换算回退、百分比夹取与真实响应快照。
- 客户端必测面：固定端点、专用凭据请求头、Kimi 双窗、零重定向、1 MiB 预算与缺凭据零请求；回环夹具只接受显式本地地址，`integration_plan_credentials` 同时断言 CLI 假凭据不进入请求。
- 调度必测面：`plan_balance_single_source_no_reentry`、`plan_balance_manual_refresh_cooldown`、`plan_balance_stale_after_full_period_and_loading_rules`、`plan_balance_generation_discards_late_results`、`plan_balance_settings_commit_and_visibility`、`integration_plan_balance_service_schedule`；导航隐藏与退出停止由后台任务订阅契约覆盖。

## 操作

- 响应字段语义变化时同步更新对应解析函数与罐装用例；实证口径变化须带日期或响应快照锚点。
