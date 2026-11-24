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
- **Agent 关联身份与写目标**：`agent.list` 接受空参数，`agent.get` 携带 `agent_id`；结果是 `AgentResource` 关联集合。对外身份有原生会话 ID 时是 `kind:native_session_id`，否则是 `kind@terminal_id`；同一身份可关联多个终端。状态是封闭文本 `idle｜working｜blocked｜done｜unknown`。`agent.input` 与 `agent.close` 是写方法，必须同时携带 `agent_id + terminal_id + generation`，以身份与 PTY 代次共同唯一定位。`agent.read` 与 `agent.wait` 复用 `TerminalReadResponse`／`TerminalWaitResponse` 结果形状，参数另含同一三元组；等待语义与 `terminal.wait` 相同。`agent.start`／`agent.resume`／`agent.fork` 是写方法，携带 CLI 类型或来源与会话身份及可选 `project_root`／`cwd`；命令构造与能力校验由 application 的 AgentCatalog 裁定，schema 只保证非空与路径形状。原因：Agent 事实必须来自同一 PTY 映射与 hook 状态，客户端不得自行拼 CLI 名单或猜测会话；只凭 Agent 身份可能歧义。边界：本 crate 只定义线缆形状；关联派生、纯 Shell 行过滤、身份与代次匹配由 application 裁定。
- **Git diff**：`git.diff` 携带可选 `project_id`、仓库相对 `path`（`/` 分隔、无 `.`/`..`、不以分隔符开头）与可选游标；结果是 `GitDiffResponse`（项目身份、路径、正文 revision、本页 `data` 与可选 `next_cursor`），分页与 `stale_cursor` 语义同终端读取。原因：Agent 与客户端审阅差异必须有界读取。边界：本 crate 只定义形状；差异来源、渲染、编码与超限拒绝由 application 裁定。
- **Git 状态**：`git.status` 携带可选 `project_id`（缺省为活跃项目），返回仓库根、分支、HEAD、上游、ahead／behind 与 porcelain v2 条目集合；条目字段是封闭原始状态与派生 staged／unstaged。原因：Runtime 与 Explorer 必须消费同一份 Git 状态事实。边界：本 crate 只定义线缆形状；项目解析、授权、子进程读取与错误映射由 application 裁定；正文读取（diff）走分页方法，不并入状态响应。
- **Git 破坏写前置**：`git.rollback` 携带可选 `project_id`、仓库相对 `path` 与封闭 `expected`（`modified｜deleted`）；`git.delete` 携带同一路径形状与 `expected`（`untracked｜staged_added`），另有缺省 `permanent=false`。两者都是写方法，结果固定为 `OperationResource`；`expected` 是专用前置条件，不接受通用 force 参数。原因：放弃未暂存更改与删除未跟踪/暂存新增都会销毁用户内容，调用方必须声明它看到的形态，服务端执行前复核。边界：路径、授权、状态重检与写盘由 application 裁定；回收站不可用明确失败，不由服务端隐式升级为永久删除。

- **Git 打开方法**：`git.open_diff` 与 `git.open_deleted` 是写方法（结果 `OperationResource`），各自携带可选 `project_id` 与仓库相对 `path`。`git.open_diff` 只接受当前映射为差异的条目（modified／renamed／conflict），`git.open_deleted` 只接受映射为删除的条目；映射不一致返回 `invalid_request`，不打开其他形态的落点。原因：Runtime 不得形成第二套打开映射或绕过 Commit 行的固定落点。边界：仓库绑定捕获、授权与真实视图打开由 application 裁定；ignored 与未出现在状态中的路径一律拒绝。

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
