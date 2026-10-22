# Runtime API v2 迁移

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 职责：只登记 v1 → v2 的迁移增量；协议契约、资源模型、传输鉴权、命令表、幂等、CLI/env 细节由 `../integrations/runtime-api-v2.md` 独占，本篇不复述。

### 唯一契约源

- 资源 ID 外部格式、稳定身份、快照 DTO、端口文件、环境变量、方法表、错误码、`mutation_id` 幂等规则与 `runtime.handover` 载荷均以 `../integrations/runtime-api-v2.md` 为准。
- 本篇只列删除项、落点、顺序、增量验证与不可自动化面；冲突时以协议源为准。

### 删除的 v1 面

- 资产：`assets/runtime-api-v1.schema.json`、v1 版 `runtime-control-api` 文档、v1 skill 内容。
- 代码：`legacy-attach` 编译 feature、ATTACH/PING 首行协议、v1 `RuntimeSnapshot` 投影、旧 `RuntimeHub` 的 window/tab/pane 资源类型。
- 方法：`tab.*`、`pane.*`、`window.*`；不保留别名、转发或参数翻译。
- 环境与发现：`SLTERM_PANE_ID`、旧面板组环境变量名、旧端点记录格式与旧 `runtime.port` 读取逻辑。
- 迁移期旧实例不互操作：端口文件版本不符即拒绝，旧客户端得到 `protocol_version_mismatch`，不做降级。

### 落点模块

- 新建 `slterm_runtime_protocol`：`envelope`、`resource_id`（只含线缆 `ResourceId` 的解析/格式化/错误）、`resources`、`methods`、`errors`、`schema`、`describe`、`limits`。
- 领域 ID newtype、单调分配器与持久化落 `slterm_workspace::ids`；协议 crate 不定义 `ProjectId` 等领域类型。
- Runtime 服务器统一落 `slterm_app/src/runtime_api/`：`server`、`transport`、`hub`、`dispatch`、`mutation_cache`、`cli`、`handover`、`skill_install`、`audit`、`conformance`。
- 旧 `slterm_app/src/runtime_api/` 的 v1 实现（server/transport/command/git/agent_api/orchestrate/cli/shortcuts）整体重写，不保留兼容模块。

### 迁移顺序

1. 领域骨架期落地 `slterm_workspace::ids` 与 `slterm_runtime_protocol` 的信封/`ResourceId`/handover DTO；壳与工作区里程碑实现最小端点、端点发现与 `runtime.handover` 接收端。
2. 垂直切片后：实现 v2 hub 投影、revision、订阅、project/panel/terminal 读写、UI owner 单队列、`mutation_cache` 与 `expected_revision`；跑项目/组/面板/终端 conformance 子集。
3. 随域增量：终端、Agent、Git 与 orchestrate 方法按域接入；每接入一域补对应 conformance。
4. 收敛门后：重写 CLI、env、skill、schema、文档与审计；跑完整 conformance。旧 v1 实现随“旧状态模型退役”停止；本步删除 v1 资产、旧别名与旧文档。

### conformance 增量

- 在协议源既有 conformance 之上增加迁移负向断言：v1 方法、旧环境变量、旧端口文件版本、旧 skill 路径必须不存在或失败。
- `resource_not_found` 覆盖未知项目/组/面板/agent；`idempotency_conflict` 覆盖同 `mutation_id` 不同参数。
- `runtime.handover` 覆盖仅聚焦、打开项目、打开文件、新建终端；第二实例不创建窗口。
- 双进程 smoke 覆盖端口文件格式、端点发现与移交；真实 GUI 用例条件跳过并登记豁免。

### 依赖里程碑

里程碑依赖只在 `plan.md` 的“里程碑”表定义；本文件不复制依赖列表。

### 测试与验收

- 协议：方法表、错误码集合、schema、`runtime.describe` 三向一致。
- 资源：稳定 ID 跨重排/移动/重命名/重定位/重启不变；未知资源 `resource_not_found`。
- 幂等：同 `mutation_id` 重放返回原响应；参数不同 `idempotency_conflict`；`expected_revision` 不一致 `revision_conflict`。
- 移交：根路径定位载荷；第二实例只发送移交并退出。
- 迁移：v1 资产、方法、环境变量与别名全部不可达；构建产物无 v1 依赖。

### 不可自动化面

- 真实 GUI 双进程移交、窗口还原与焦点；条件跳过用例与构建产物人工 smoke 兜底。
- 外部 AI CLI 的 hook 事件、Agent 会话身份与 delegate 回传。
- 本机防火墙、安全软件对 loopback 监听的干预。
- 其他本机用户对 token 的访问边界无法在单用户测试中完整模拟。
