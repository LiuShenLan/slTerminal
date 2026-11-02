# Runtime v2 控制面合同

> 登记 `runtime.hello` 与 `instance.handover` 的操作语义和安全边界；字段形状只由生成的 [JSON Schema](runtime-api-v2.schema.json) 拥有。

## 决策与红线

- 传输使用 UTF-8 JSON Lines 与 JSON-RPC 2.0，禁止 batch 和无响应通知。首条请求必须是 `runtime.hello`，协议版本固定为 2；Runtime 与 handover 使用各自凭据和作用域。
- 凭据只用于认证比较，不进入日志、错误文本、端点元数据或调试输出；不提供 token 环境变量或 v1 兼容入口。
- `instance.handover` 只接受仅聚焦、打开既有项目、打开文件和新建终端。打开或新建不得隐式创建项目；路径与 cwd 必须重新经过 PathPolicy 和命令目录校验，显式 cwd 未授权或无效时拒绝，不回退到活跃项目。
- 响应的 `result` 与 `error` 必须恰好一个存在。标准 JSON-RPC 数值码与封闭 `error.data.code` 不得混用。

## 验收与测试

- `runtime.hello` 校验版本、client_id、凭据和作用域，并返回 store_epoch、stream_epoch、revision、协议与 schema 散列及当前方法能力。
- JSON Lines 解析拒绝 batch、非法 UTF-8、未知字段、重复字段、无效路径和超过 128 KiB 的请求；认证失败不得泄露协议或凭据细节。
- schema 测试逐字节比对随包资产，并用真实 hello 与全部 handover 形态验证；未知方法必须同时被 DTO 与 schema 拒绝。

## 操作

- 重新生成 schema：`cargo run -p slterm_runtime_protocol --bin runtime-schema --features schema`。
- 定向测试：`cargo test -p slterm_runtime_protocol --test lib_tests --features schema -- --test-threads=1`。
