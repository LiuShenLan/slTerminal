# slterm_runtime_protocol（core 层）

> Runtime v2 跨进程协议与 Operation 状态语义的独立 crate 边界。

## 决策与红线

- **旧协议不回流**：不得恢复 Runtime v1 方法、独立 handover schema、token 环境变量或端口文件凭据。原因：目标协议统一由 LocalControlServer 与 v2 schema 承载。边界：crate 边界保留不等于提供兼容入口。
- **不预实现目标协议**：没有对应实现任务时不得先加占位 DTO、空方法族或序列化壳。新增协议以 `docs/devPlan/design/runtime.md` 为合同。
- **方法注册表单源**：线缆方法名、可用作用域、读写类别、幂等语义与参数／结果 schema 只在本 crate 的方法注册表登记；hello 能力摘要由注册表派生，schema 入口按方法遍历。原因：路由、capabilities、describe 与 schema 漂移会让客户端观察到不存在的合同。边界：注册表只登记真实可用方法，作用域集合表达方法合同的可用面，服务端的认证与开放面仍由各作用域策略决定；新增方法先登记注册表，再接线服务器与生成资产。
- **操作状态只前进**：只允许 `Queued → Running` 与 `Running → Succeeded｜Failed｜Cancelled｜OutcomeUnknown`。原因：跳过执行或终态复活会破坏幂等查询与恢复事实。边界：`OutcomeUnknown` 是可查询终态，不自动重发；持久化与恢复由所属服务实现。
- **依赖方向**：不得依赖内部领域 crate；Serde 与可选 schemars 只按目标协议实际需要有界引入。GPUI 与平台渲染框架禁入生产依赖。

## 验收与测试

- 定向测试：`cargo test -p slterm_runtime_protocol --test lib_tests -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 旧 Runtime 与独立 handover 资产退役由 `scripts/tests/test_runtime_retirement.py` 机械守卫。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增协议：同步 DTO、生成 schema、随包资产与 crate 测试，不手维护第二份字段真值。
