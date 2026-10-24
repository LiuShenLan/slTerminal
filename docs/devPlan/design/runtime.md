# Runtime v2 控制面

> 规定本机 JSON-RPC、资源方法、认证、持久 Operation、事件续传和随包 schema 的实现边界。

## 决策与红线

### 传输、认证与发现

- LocalControlServer 合并单实例移交与 Runtime，始终只监听 loopback；OS 单实例锁独立。Hooks 仍使用独立命名管道，不能借 Runtime 传输。
- 仅提供 JSON-RPC 2.0，每行一个 UTF-8 对象；禁止 batch、客户端无响应通知和 v1 信封。标准错误使用 JSON-RPC 数值码，领域错误使用封闭 `error.data.code`。
- 首条请求必须为 `runtime.hello`，携带协议版本、client_id、凭据和作用域 `runtime｜handover`。认证失败关闭连接，不返回协议预言机，不记录请求行；认证通过后才解析后续详细参数。
- Runtime 与 handover 使用两个独立凭据目标、同一服务器和端点。凭据由 Windows CSPRNG 生成 256 位随机值，目标包含本地根稳定散列，隔离测试和不同数据根。
- `slterm runtime discover --json` 是唯一输出 Runtime token 的入口；端点文件只含非敏感元数据并限制当前用户 ACL。普通 CLI 从凭据门面取认证材料，不接受 token 环境变量或命令行注入。
- 删除或轮换 Runtime token 只断开 Runtime 会话，不影响 handover；设置界面只显示状态和非敏感端点，不回显 token。
- 连接上限 64、请求上限 128 KiB、每连接在途请求 8、响应上限 1 MiB、握手超时 5 秒、等待最长 600 秒。所有读取、订阅和等待都可取消并受关闭状态约束。

### 握手、快照与事件

- 握手返回 store_epoch、stream_epoch、revision、协议／schema SHA-256 和能力摘要。`runtime.describe` 返回方法注册与能力，`runtime.snapshot` 返回同一资源模型的不可变快照。
- 全局事件环保留最近 1,024 个轻量事件，每客户端待发送队列 64 项；事件只携带资源身份、代次和必要元数据，终端正文只经分页读取，事件只携带输出 revision。
- 事件续传必须同时匹配 store_epoch、stream_epoch 和 revision，否则返回 `resync_required`。普通进程重启只更换 stream_epoch；整库恢复、重置或灾难恢复更换 store_epoch。
- 客户端滞后只终止其订阅并要求重新快照，不反压 UI、PTY 或其他客户端。取消订阅、资源销毁、关闭开始和应用退出后不得回填过期事件。
- RuntimeHub 发布不可变快照与轻量事件；前台状态更新先于事件发布，事件环有界，缺口不能靠重复事件补偿。

### 资源与方法面

方法注册表同时驱动路由、capabilities、describe 和 schema 根定义。目标方法族如下；字段形状只由生成 schema 拥有。

| 资源 | 方法 |
|---|---|
| 元能力 | `runtime.hello`、`runtime.describe`、`runtime.snapshot`、`events.subscribe`、`events.unsubscribe`、`operation.get`、`operation.wait`、`app.focus`、`app.close` |
| Project | `project.list`、`project.activate`、`project.rename`、`project.reorder`、`project.remove` |
| PanelGroup | `panel_group.split`、`panel_group.merge`、`panel_group.resize` |
| Panel | `panel.open_file`、`panel.open_settings`、`panel.activate`、`panel.move`、`panel.save`、`panel.close` |
| Terminal | `terminal.create`、`terminal.input`、`terminal.execute`、`terminal.read`、`terminal.wait`、`terminal.reconnect`、`terminal.rename`、`terminal.close` |
| Agent | `agent.list`、`agent.get`、`agent.start`、`agent.resume`、`agent.fork`、`agent.input`、`agent.read`、`agent.wait`、`agent.close` |
| Git | `git.status`、`git.diff`、`git.rollback`、`git.delete`、`git.open_diff`、`git.open_deleted` |
| 单实例 | `instance.handover`，只含 focus、open_project、open_file、new_terminal |

- 不开放 Runtime 新增项目或重定位根；不开放通用 unstage、stage、commit、fetch、worktree、网络 Git、历史删除或任意文件操作。
- 删除 `runtime.orchestrate`、通用 `command.execute`、`send_key`、`procs`、`delegate`、旧 window／tab／pane 资源和 Git worktree 编排。`agent.fork` 只表示 CLI 原生历史分支。
- `panel_group.split` 从指定源 Panel 拆分并同时提供落点内容；merge 按确定顺序合并组；不通过空组或伪造 Panel 绕过布局规则。
- `terminal.execute` 只面向可信 Shell 行，携带 TerminalId 与 PTY 代次；`terminal.input` 是显式原始输入能力，二者都不模拟键盘事件。接受补全或注入命令不隐式追加 Enter。
- 路径目标选择顺序为显式 project_root → cwd 最深匹配 → 两者都未提供时才使用活跃项目。显式 cwd 未授权即失败，不改投活跃项目；Runtime 不隐式创建项目。
- `instance.handover` 与 Runtime 复用同一 PathPolicy、CommandRegistry、保存守卫和关闭状态机；显式路径不自动授予根外读取或写入能力。

### 写操作与幂等

- 每个写请求携带 client_id、mutation_id、规范参数散列和可选 expected_revision；服务器重新计算参数散列，失配拒绝。资源代次、授权代次和 store_epoch 在执行前复核。
- Operation 先持久登记再派发，保存 store_epoch、operation_id、client_id、mutation_id、method、参数散列、资源代次、状态、结果／错误和可选 journal_id。
- 状态为 `Queued → Running → Succeeded｜Failed｜Cancelled｜OutcomeUnknown`。相同幂等键与参数返回原 Operation，同键不同参数为 mutation_conflict；Operation 不自动过期。
- `decision_required` 是结构化失败终态，调用方修正策略后使用新 mutation_id。脏关闭未指定保存／丢弃策略时返回该错误，不弹模态、不自动丢弃。
- 纯数据库操作按事务结果与提交指纹判定；多步骤操作由所属 Journal 专门恢复器对账。终端输入、进程写入及无法证明副作用的操作中断后标为 OutcomeUnknown，绝不自动重发。
- `operation.get` 与 `operation.wait` 可跨断线查询；等待受 600 秒上限和关闭状态约束。结果不明必须可查询，不要求客户端盲目重放。

### 关闭与读取安全

- `app.close`、`panel.close`、`project.remove` 和更新安装复用 CloseCoordinator；调用方必须显式提供 Document、Hooks 和运行终端处置策略，缺少决策返回 decision_required。
- 关闭开始冻结新写，完成或明确终止在途请求；读请求只能访问当前仍获授权的资源。关闭服务撤销监听、端点记录和线程，不留孤儿。
- 终端读取使用不可变缓冲快照和分页游标，响应不超过 1 MiB；新输出使旧游标或 revision 失效。Git diff、Agent read 等可能超限的读取也使用有界页面与游标续读，不能把截断内容伪装成完整结果。读取不返回凭据、剪贴板或未授权文件内容。
- 破坏操作使用专属版本／指纹前置条件，不使用通用 `force:true`；审计只记录允许字段与哈希，不记录正文或 token。

### 协议 crate 与随包契约

- `slterm_runtime_protocol` 持有全部请求、响应、事件、发现和错误 DTO，生产依赖使用 Serde；`schemars` 放在可选 `schema` feature。
- 在该 crate 增加 schema 生成 binary，不新增 xtask crate。生成 `assets/runtime-api-v2.schema.json`，测试生成到临时文件并逐字节比较。
- `instance.handover` 进入同一 v2 schema，删除旧独立 handover schema。CLI 使用相同 DTO，不自行拼协议信封。
- `assets/runtime-control-api.md` 只写操作语义、安全边界和示例；字段形状以生成 schema 为准。发布只收 v2 资产，hello／describe 携带 schema SHA-256。
- JSON Schema 负责结构校验，Serde 与领域服务负责跨字段、路径、授权和代次校验；schema 通过不代表操作已获授权。

## 验收与测试

- 协议测试覆盖 JSON Lines、UTF-8、请求／响应上限、batch 拒绝、首包认证、错误码、作用域隔离和认证失败无协议预言机。
- 真实 GUI／CLI conformance 覆盖 hello、describe、snapshot、全部资源方法、凭据轮换、发现输出和 v1 拒绝；测试使用临时 AppPaths 与 `sk-test` 假凭据。
- 写操作测试覆盖重复 mutation、参数冲突、断线重查、expected_revision、decision_required、OutcomeUnknown、恢复后的 store_epoch 隔离和不自动重发。
- 事件测试覆盖 1,024 环、64 队列、续传三元组、缺口 resync、终端正文分页和滞后不反压 UI。
- 关闭测试覆盖非交互脏 Document／Hooks／PTY 决策、关闭中拒绝、端点撤销和无孤儿线程。
- schema 测试逐字节比较生成资产，校验 hello／describe 散列、CLI DTO 一致性和发布包只含 v2 合同。

## 操作

- 新增方法先登记方法注册表、资源 ID、权限、Operation 语义、事件和 schema 测试，再实现路由；禁止在 CLI 另写请求结构。
- 修改线缆字段必须重新生成 schema、更新随包参考和 conformance，不在本设计文件复制字段补丁。
- 凭据、端点和日志变更必须检查所有持久材料、备份、诊断与错误路径不含 token。
