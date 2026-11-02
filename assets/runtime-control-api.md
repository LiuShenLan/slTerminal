# Runtime v2 控制面合同

> 登记 `runtime.hello`、`instance.handover` 与端点发现的操作语义和安全边界；字段形状只由生成的 [JSON Schema](runtime-api-v2.schema.json) 拥有。

## 决策与红线

- 传输使用 UTF-8 JSON Lines 与 JSON-RPC 2.0，禁止 batch 和无响应通知。首条请求必须是 `runtime.hello`，协议版本固定为 2；Runtime 与 handover 使用各自凭据和作用域。
- 凭据只用于认证比较，不进入日志、错误文本、端点元数据或调试输出；不提供 token 环境变量或 v1 兼容入口。
- 发现记录只承载协议版本、loopback 非零端口和运行身份元数据，读取预算 4 KiB；文件 DACL 仅允许当前用户且禁止继承，撤销只能删除原发布者仍拥有的记录。认证材料独立经凭据门面获取。
- `runtime.hello` 的 `store_epoch` 取当前工作区持久世代，`stream_epoch` 取本次运行的 `run_id`，新事件流从 `revision=0` 开始；发现记录必须使用同一 `RunMarker` 与实际绑定的 server endpoint。禁止以占位符代替真实世代或合同散列。
- 合同散列均以随包 `runtime-api-v2.schema.json` 的原始字节为输入，输出 64 个小写 ASCII hex：`schema_sha256 = SHA-256(schema_bytes)`；`protocol_sha256 = SHA-256(b"slterm-runtime-v2" || schema_bytes)`。域分隔前缀后不得补 NUL、换行或其他分隔符，两个散列必须使用同一份字节且不得改动 schema 字段。
- handover 回执在服务端 worker 中有界等待，生产上限 4 秒；超时返回数值码 `-32001` 与领域码 `not_ready`，回执发送端断开返回 `-32004` 与 `closing`，事件投递 helper 的 QueueFull 等错误保持原数值码和领域码。
- `instance.handover` 只接受仅聚焦、打开既有项目、打开文件和新建终端。打开或新建不得隐式创建项目；路径与 cwd 必须重新经过 PathPolicy 和命令目录校验，显式 cwd 未授权或无效时拒绝，不回退到活跃项目。
- 响应的 `result` 与 `error` 必须恰好一个存在。标准 JSON-RPC 数值码与封闭 `error.data.code` 不得混用。

## 验收与测试

- `runtime.hello` 校验版本、client_id、凭据和作用域，并返回 store_epoch、stream_epoch、revision、协议与 schema 散列及当前方法能力。
- JSON Lines 解析拒绝 batch、非法 UTF-8、未知字段、重复字段、无效路径和超过 128 KiB 的请求；认证失败不得泄露协议或凭据细节。
- schema 测试逐字节比对随包资产，并用真实 hello、全部 handover 形态与发现记录验证；未知方法、发现附加字段及敏感字段必须同时被 DTO 与 schema 拒绝。

## 操作

- 重新生成 schema：`cargo run -p slterm_runtime_protocol --bin runtime-schema --features schema`。
- 定向测试：`cargo test -p slterm_runtime_protocol --test lib_tests --features schema -- --test-threads=1`。
