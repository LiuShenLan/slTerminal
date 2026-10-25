# 套餐余量（plan_balance）

> DeepSeek/Kimi 响应解析与展示值类型。

## 决策与红线

- **纯解析边界**：解析函数只接受 `serde_json::Value`，不发现来源，不读取 CLI 配置或环境认证材料，不接触凭据、网络与调度。原因：响应算法不得与旧认证通道、全局状态或路径推导耦合。边界：请求与凭据属于独立执行层，不得混入解析函数。
- **结果不含凭据**：结果类型只承载冻结态、金额与时间窗，不含 token；错误消息不得插值认证材料。测试只使用罐装响应，不读取真实账号或配置。
- **DeepSeek 语义**：从 `balance_infos[0]` 读取 `total_balance` 与 `currency`；空数组、字段缺失或非字符串均失败。
- **Kimi 语义**：采用 2026-08 实测口径；冻结由 `totalQuota.remaining <= 0` 判定，5 小时窗读取 `limits[i].detail`，7 天窗读取顶层 `usage`。`used` 优先，`remaining` 换算回退；非冻结时任一窗口失败则整体失败。真实响应快照锚点为 `parse_real_response_snapshot`。
- **缺失不造数**：字段缺失或不可解析返回错误，不把缺失数据伪造为 0；百分比输出夹到 0–100。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 罐装 JSON 覆盖正常值、缺失字段、类型错误、冻结态、双窗选择、换算回退、百分比夹取与真实响应快照。

## 操作

- 响应字段语义变化时同步更新对应解析函数与罐装用例；实证口径变化须带日期或响应快照锚点。
