# slterm_runtime_protocol（core 层）

> Runtime v2 跨进程线缆类型与平台无关校验的共享权威：只借用字符串，不做序列化与路径合法性判断。

## 决策与红线

- **零生产依赖**：类型只承载借用字符串，不引入 serde 或任何生产依赖；序列化归调用方。原因：协议 crate 必须被壳、CLI 与 hook 同时复用且保持轻量。边界：仅测试依赖可出现在 `dev-dependencies`。
- **路径定位，不传内部 ID**：移交载荷只经 `project_root`/`cwd`/`path` 定位，不携带 `ProjectId`/`PanelId` 等内部 ID；路径字段以 `project:`/`group:`/`panel:` 开头即校验失败。原因：内部 ID 不跨进程稳定，路径是唯一外部可表达的定位方式。
- **平台分层**：协议层只校验 schema 版本、非空与内部 ID 前缀；Windows 路径合法性归平台层。原因：路径语义依赖操作系统，协议层无法判定。
- **schema 单点**：机器可读契约以 `methods/handover.schema.json` 为唯一真值源，经 `HANDOFF_SCHEMA_JSON` 内嵌。原因：双写会漂移。边界：改字段必须同步 schema、Rust 类型与测试。
- **版本化**：`HANDOFF_SCHEMA_VERSION` 与 schema 的 `const` 同步，未知版本一律拒绝。
- **单 op 语义**：一次移交只含一个 `HandoffOp`，不定义操作序列；拒绝原因以 `HandoffReject` 枚举登记，不新增自由文本原因。

## 验收与测试

- 定向测试：`cargo test -p slterm_runtime_protocol --test lib_tests -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- schema 可解析性、op/reply 形状、版本/空值/内部 ID 校验与借用字段构造是必测面。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增 op、reply 字段或拒绝原因：先改 schema，再改 Rust 类型与 `validate`，最后补 `lib_tests` 用例。
