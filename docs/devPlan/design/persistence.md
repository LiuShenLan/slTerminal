# 持久化与恢复材料

> 规定数据根、SQLite 物理分组、规范 DTO、快照信封、Operation 日志和原子写的唯一实现边界。

## 决策与红线

### 数据根与写入纪律

- 配置根保存 `settings.json`、设置有效备份和自定义主题单文件；本地根保存 `workspace.db`、`learning.db`、窗口几何、快照、隔离区、恢复点、端点元数据、更新材料、日志和维护请求。
- Windows 适配器经 Known Folder API 得到 `%APPDATA%/slterm` 与 `%LOCALAPPDATA%/slterm`。测试直接注入临时 AppPaths；生产环境只在启动入口读取 `SLTERM_CONFIG_DIR` 和 `SLTERM_LOCAL_DIR`。
- `workspace.db` 是结构化可移植状态与 Runtime／恢复元数据的权威；`learning.db` 独立保存学习历史。文件正文、终端输出和 AI 会话正文不进入 SQLite。
- 每个库拥有独立写连接和写线程；文件存储分别串行原子写，不共享一个全局写线程。跨存储一致性由 ManagedOperationJournal、前后指纹和恢复协议闭合。
- 所有普通文件写入使用同目录临时文件、完整 flush、rename 和必要的目录同步。保存目标 ACL 无法保留时保存失败，不 best-effort 留下权限变化。
- 设置载荷上限 1 MiB；未知命名空间本地原样保留但不导出。设置写只经 settings_store／audit 包装，核心写函数私有。

### SQLite 共同合同

- 两个库均设置固定 `application_id`、`user_version=1`、WAL、外键和忙时策略；只接受当前 schema，不建立旧库升级链。工作区库 `application_id=0x534C544D`，学习库 `application_id=0x534C4C52`；两者标识在生成 schema 的测试中固定。
- 启动先检查 application_id、user_version、完整性和 WAL 状态，再决定事务恢复、有效副本、只读或隔离。更高版本只读，不隔离、不降级、不覆盖。
- 所有跨表不变量在同一事务内提交；前台只收到提交后的不可变结果。SQLite 错误不直接拼接到用户日志，审计只记录允许字段和指纹。
- 灾难副本使用 SQLite Online Backup 写入临时库，校验 application_id、user_version、外键和完整性后原子替换。正常关闭、成功维护和显式破坏操作提交后排入副本任务；副本不是实时镜像，恢复时明确其世代。

### workspace.db 表分组

下表是物理所有权的最低集合；字段类型、外键和索引在目标 schema 定义中固定，不在 UI 或 Runtime 另建 JSON 副库。

| 表 | 关键字段与约束 | 权威内容 |
|---|---|---|
| `workspace_meta` | 单行约束、`store_epoch`、`run_id`、`clean_shutdown`、`schema_version` | 世代、运行标记和正常退出事实。 |
| `projects` | `project_id` 主键、词法 `root` 唯一、显示名、排序、根状态、`next_terminal_number` | 项目元数据、RelocationRequired 和单调终端编号计数器。 |
| `app_state` | 单行约束、可空 `active_project_id` 外键 | 活跃项目，不用布尔多行模拟唯一激活。 |
| `panel_groups` | `panel_group_id` 主键、`project_id` 外键 | 稳定布局组身份。 |
| `project_layouts` | `project_id` 主键、DTO 版本、规范布局 JSON、revision | PanelGroup 树、ViewKey、激活和尺寸；不存 PanelId。 |
| `documents` | `document_id` 主键、位置与文件身份 DTO | 被持久布局引用的 Document 元数据，不存正文。 |
| `terminals` | `terminal_id` 主键、项目外键、编号唯一、cwd、Shell、run_intent | 逻辑终端元数据与恢复意图，不存 PTY 句柄或回滚正文。 |
| `view_states` | 规范 ViewKey 主键、DTO 版本、状态 JSON、revision、作用域 | 所有持久视图状态的唯一通道。 |
| `diff_bindings` | Diff ViewKey 主键、RepositoryBinding、head_path、revision | Diff 的固定仓库和 HEAD 旧路径。 |
| `window_geometry` | 机器键主键、DTO 版本、几何 JSON | 按机器的窗口几何；跨机恢复不应用。 |
| `runtime_operations` | `operation_id` 唯一、`store_epoch＋client_id＋mutation_id` 唯一 | 幂等键、参数散列、资源代次、状态和结果／错误。 |
| `managed_journals` | `journal_id` 主键、类型、状态、store_epoch、前后指纹 | 多步骤文件与恢复操作总账。 |
| `journal_steps` | 日志与步骤键联合主键、目标、授权代次、前后指纹 | 可重放的步骤事实，不存任意正文。 |
| `close_intents` | 关闭 ID、对象、代次、决定、提交状态 | 关闭收集结果和破坏动作前置事实。 |
| `quarantine_batches` | `batch_id`、状态、绝对截止时间、完成时间、指纹 | 历史隔离批次。 |
| `quarantine_items` | 批次与项键、原始位置、顺序、来源身份、前后指纹 | 逐项隔离清单；正文在独立隔离文件。 |
| `recovery_points` | 恢复点 ID、状态、对象键、前后指纹、DPAPI 文件位置 | 逻辑恢复点索引；正文在本地原子文件。 |
| `maintenance_requests` | 请求 ID、类型、状态、暂存材料指纹、store_epoch | 待处理恢复、撤回和维护请求。 |
| `snapshot_index` | DocumentId、文件指纹、状态、扫描代次 | 可重建索引，不是快照正文权威。 |

- `view_states` 的 ViewKey 必须解析到封闭注册表。项目级状态随项目删除；应用级 SideView DTO 内按 ProjectId／AgentRootId 分区的字段由 ViewStateStore 精确清理，不能误删共享视图。
- 项目布局解析后校验所有 ProjectId、PanelGroupId、DocumentId、TerminalId 和 ViewKey 引用；未知视图过滤，结构损坏按项目分流并保留原始 DTO，不自动补空 Panel。
- `documents`、`terminals` 只保存被布局或恢复意图引用的记录。最后引用关闭且必要提交完成后才删除记录，不能让旧快照或旧 Runtime 结果复活它们。`projects.next_terminal_number` 只递增，删除终端记录不回退，保证编号跨重启不复用。
- 不建立持久 Panel 表、终端正文表、AI 正文表或第二份活跃 Agent 映射。
- 可移植备份只从 `projects`、`app_state`、`panel_groups`、`project_layouts`、`documents`、`terminals`、`view_states`、`diff_bindings`、`window_geometry` 和设置／主题逻辑投影生成；`runtime_operations`、Journal、关闭意图、隔离批次、恢复点、维护请求和 `snapshot_index` 永不进入 BackupPayload。灾难副本仍按本地恢复需要保存完整工作区库。

### 规范 DTO 与版本

- 持久 JSON 使用显式 `version`、封闭 `kind`、BTreeMap 键序、UTF-8 和有限数值；序列化后必须可逐字节重放。禁止把 Rust 调试格式、HashMap 遍历顺序或 GPUI 类型写入磁盘。
- ViewKey、ProjectLayout、ViewState、DiffBinding、BackupPayload 和 Operation 参数都有独立 DTO 版本；结构校验与领域校验分开，未知字段和未知封闭变体拒绝。
- DTO 只保存规格声明的可移植状态。路径存词法值与必要身份信息，授权本身不持久化为可跨机通行证；跨机恢复重新走 PathAuthority。
- ViewState 的状态 schema 由所属视图注册，框架只负责版本、作用域、写入和恢复过滤；未注册视图不能借 JSON 自由扩展持久字段。

### Document 快照

快照正文采用自描述原子文件，文件名只是定位提示，文件头才是权威：

```text
魔数 SLTSNAP1 与版本 1
→ 元数据长度、正文长度、SHA-256（整数按小端编码）
→ 规范元数据
→ UTF-8 正文
```

元数据包含 DocumentId、位置、文件身份、编码、BOM、行尾、磁盘基线指纹、saved_state_id、当前 TextStateId、正文散列、创建 run_id 和格式版本。元数据与正文长度、散列均校验；损坏快照隔离并提示，不直接物理删除。

- 编辑事务后进入有界快照队列，采用固定短去抖并在显式保存、关闭收集和退出前冲刷；不为每个按键同步写盘。新快照开关只控制新写入，既有快照仍可显式恢复或删除。
- 单文件正文上限 64 MiB，总配额 256 MiB；不驱逐仍打开且脏的 Document 快照。配额不足保留编辑现场并立即警告。
- 保存成功、撤销回 saved_state_id 或最后视图按已提交意图关闭后，先提交必要状态再删快照。启动时正文散列与基线相同的遗留快照按冗余材料对账，不制造脏文档。
- 有主快照按布局 DocumentId 对账，不创建重复 Panel；无主快照等待用户选择项目与落点或确认删除，只有无主材料适用 7 天保留期。
- 快照不包含只读分页、GitShow、终端输出、AI 会话正文、剪贴板、学习历史或凭据。

### learning.db

- 学习库使用独立写连接、队列和当前 schema；不进入灾难副本、普通导出、加密备份、日志或诊断。旧 JSONL／JSON 不迁移、不覆盖。
- `command_history` 以 Shell 类型与规范文本唯一，保存首尾使用时间、次数和最近 cwd；`directory_history` 保存规范词法路径、首尾使用时间和 frecency 数据。
- 命令容量 5,000 条、目录容量 2,048 条；命令文本与规范路径字段均以 64 KiB UTF-8 为上限，超限不采集、不截断正文。淘汰在同一事务中完成。损坏或高版本暂停持久学习并给显式重置，不覆盖原库。
- 清空先递增学习代次、取消待写、清内存，再事务删除并截断 WAL／压缩；失败保留可见错误，不报告已清空。

### Operation 与 ManagedOperationJournal

- Runtime 写请求先持久登记 Operation，再派发副作用。Operation 保存 store_epoch、operation_id、client_id、mutation_id、method、规范参数散列、资源代次、状态、结果／错误和可选 journal_id。
- 状态为 `Queued → Running → Succeeded｜Failed｜Cancelled｜OutcomeUnknown`；相同幂等键与参数返回原操作，同键不同参数返回 mutation_conflict。记录不自动过期。
- 单库事务按事务结果与提交指纹判定；多步骤操作只能由所属 Journal 的专门恢复器对账。终端输入、进程写入等无法证明副作用的中断标为 OutcomeUnknown，绝不自动重发。
- 日志状态为 `Prepared → Applying → Committing → Committed`，异常进入 `RollingBack`、`RolledBack`、`Failed` 或 `NeedsAttention`。状态不明保留全部副本，清理必须显式且可审计。

## 验收与测试

- 真实临时 SQLite 测试覆盖外键、单行约束、唯一幂等键、事务回滚、WAL 恢复、当前／高版本分流、Online Backup 和 store_epoch 更换。
- 属性测试覆盖规范 DTO 字节稳定、未知字段拒绝、布局引用完整性和 ViewState 作用域清理；快照测试覆盖截断、错长、错散列、编码和配额。
- 故障注入覆盖临时文件 flush、rename、目录同步、ACL 保留、数据库提交和副本替换；任何失败不得把目标写成半状态。
- 学习库测试覆盖容量、字段上限、清空代次、损坏暂停和隔离于备份／诊断；测试只使用 `sk-test` 形态假凭据。
- Operation 测试覆盖断线、重启、重复键、参数冲突、OutcomeUnknown、日志不明和恢复后的旧命名空间；不以自动重试掩盖未知副作用。

## 操作

- 修改 schema 必须同步生成定义、完整性检查、备份资格表、恢复器和崩溃测试；不接受“只加一列”的隐式迁移。
- 新增文件材料先写入对应所有者目录与清理规则，再接入快照、备份或诊断；任何正文材料默认不进入导出。
- 工作区灾难恢复、重置和备份恢复后必须重新读取 store_epoch，并通知 Runtime、任务和授权服务使旧代次失效。
