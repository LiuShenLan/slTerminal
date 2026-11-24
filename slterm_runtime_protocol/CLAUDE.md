# slterm_runtime_protocol（core 层）

> Runtime v2 跨进程协议与 Operation 状态语义的独立 crate 边界。

## 决策与红线

- **旧协议不回流**：不得恢复 Runtime v1 方法、独立 handover schema、token 环境变量或端口文件凭据。原因：目标协议统一由 LocalControlServer 与 v2 schema 承载。边界：crate 边界保留不等于提供兼容入口。
- **不预实现目标协议**：没有对应实现任务时不得先加占位 DTO、空方法族或序列化壳。新增协议以 `docs/devPlan/design/runtime.md` 为合同。
- **方法注册表单源**：线缆方法名、可用作用域、读写类别、幂等语义与参数／结果 schema 只在本 crate 的方法注册表登记；hello 能力摘要与 `runtime.describe` 的方法清单都由注册表派生，schema 入口按方法遍历。原因：路由、capabilities、describe 与 schema 漂移会让客户端观察到不存在的合同。边界：注册表只登记真实可用方法，作用域集合表达方法合同的可用面，服务端的认证与开放面仍由各作用域策略决定；新增方法先登记注册表，再接线服务器与生成资产。
- **操作状态只前进**：只允许 `Queued → Running` 与 `Running → Succeeded｜Failed｜Cancelled｜OutcomeUnknown`。原因：跳过执行或终态复活会破坏幂等查询与恢复事实。边界：`OutcomeUnknown` 是可查询终态，不自动重发；持久化与恢复由所属服务实现。`runtime.operation_get`/`runtime.operation_wait` 的线缆视图只含身份、状态、结果与错误；写请求参数散列只经本 crate 的 `params_sha256`，客户端与服务端共用同一推导。
- **面板身份与布局参数**：Panel 与 PanelGroup 方法不暴露运行期 `PanelId`；视图身份使用规范 ViewKey JSON 文本，组身份使用稳定 `PanelGroupId` 文本。拆分参数带封闭方向、比例与逻辑视口；resize 带从根到分割节点的布尔路径。原因：PanelId 不属于持久身份，暴露后客户端可伪造运行实例。边界：路径、项目归属、空组与几何约束由 application 的布局转换裁定，schema 只保证结构与非空/有限值。
- **终端输入与执行**：`terminal.input` 携带 `terminal_id + generation + data`，`data` 必须原样写入 PTY，不追加回车；`terminal.execute` 只接受单行命令（拒绝 `\0`、`\r`、`\n`）并由服务端追加一次回车。两者都以稳定 `TerminalId` 文本与 PTY 代次定位，`generation` 必须大于零。原因：原始输入与显式执行是两种语义，自动补回车必须且只能由 execute 承担。边界：schema 只保证结构、非空与代次范围；终端身份解析、运行意图与代次匹配由 application 裁定；不得把 execute 建模为键盘事件序列。
- **终端分页读取**：`terminal.read` 携带 `terminal_id + generation + cursor?`，结果是 `TerminalReadResponse`（同身份、代次、输出 revision、本页 `data` 与可选 `next_cursor`）。游标是不透明服务端文本，绑定资源代次与输出 revision，任何变化都返回 `stale_cursor`，客户端必须重新读取。原因：终端正文只经分页读取，截断不得伪装成完整结果。边界：本 crate 只定义线缆形状与错误码；页面切分、代次校验与文本捕获由 application 的 `reads` 与 AppRoot 读取端口完成。
- **终端等待**：`terminal.wait` 携带 `terminal_id + generation + since_revision + timeout_ms?`，等到输出 revision 超过 `since_revision` 返回 `TerminalWaitResponse`；超时映射 NotReady、关闭映射 Closing，默认与上限均为 600,000 毫秒。原因：客户端不得轮询分页读取来发现新输出。边界：等待只读 revision，不返回正文；revision 与分页游标同源，变化后客户端应重新读取。
- **Agent 关联身份**：`agent.list` 接受空参数，`agent.get` 携带 `agent_id`；结果是 `AgentResource` 关联集合。对外身份有原生会话 ID 时是 `kind:native_session_id`，否则是 `kind@terminal_id`；同一身份可关联多个终端。状态是封闭文本 `idle｜working｜blocked｜done｜unknown`。原因：Agent 事实必须来自同一 PTY 映射与 hook 状态，客户端不得自行拼 CLI 名单或猜测会话。边界：本 crate 只定义线缆形状；关联派生、纯 Shell 行过滤与未知身份错误由 application 裁定；写方法必须再提供 TerminalId 与 PTY 代次唯一定位。
- **快照代次与终端生命周期写方法**：`TerminalResource` 携带当前 PTY `generation`（零表示没有存活 worker），客户端据此构造写方法参数。`terminal.close` 与 `terminal.reconnect` 都携带 `terminal_id + generation`，代次必须等于快照代次，失配映射 `revision_conflict`；reconnect 只接受已退出终端，成功分配新代次。`terminal.rename` 携带 `terminal_id + title?`，`null` 表示清除，空白与超过 128 个 Unicode 字符在协议层拒绝。原因：关闭与重连不得不命中重连后的另一个进程，标题输入必须有单一上限。边界：三者按稳定逻辑身份执行，不暴露运行期 `PanelId`；持久阶段语义由 application 的关闭协调器、重连实现与候选快照决定。
- **两阶段参数解析与写信封**：`RuntimeRequest` 先固定信封与方法名，再把 `params` 按方法二次解析为封闭结构；同构参数（例如同为 `{project_id}` 的两个写方法）不再靠 untagged 判别。写方法参数统一包在 `MutationParams<P>`：`client_id`、`mutation_id` 必填，`expected_revision` 可选，具体意图在 `request`。写方法结果固定为 `OperationResource`。原因：方法数增长后 untagged 集合既不可判别也不可维护。边界：未知字段由目标 DTO 的 `deny_unknown_fields` 拒绝；跨字段与方法语义（路径、授权、代次）不属于 schema 职责。
- **事件通知与续传**：`RuntimeEvent` 只含 revision、封闭事件类型名、资源身份文本与资源代次，不含正文、路径内容或凭据；`EventNotification` 是服务器单向通知，客户端不得回复。订阅参数固定为 `store_epoch + stream_epoch + since_revision` 三元组；不匹配或缺口以 `resync_required` 拒绝，客户端必须重新快照。原因：轻量事件与不可变快照共同构成客户端状态，正文走分页读取。边界：事件环、队列容量、滞后裁剪与发布所有权由 application 实现；本 crate 只定义线缆形状与封闭错误码。
- **依赖方向**：不得依赖内部领域 crate；Serde 与可选 schemars 只按目标协议实际需要有界引入。GPUI 与平台渲染框架禁入生产依赖。

## 验收与测试

- 定向测试：`cargo test -p slterm_runtime_protocol --test lib_tests -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 旧 Runtime 与独立 handover 资产退役由 `scripts/tests/test_runtime_retirement.py` 机械守卫。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增协议：同步 DTO、生成 schema、随包资产与 crate 测试，不手维护第二份字段真值。
