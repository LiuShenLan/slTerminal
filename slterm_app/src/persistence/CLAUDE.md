# persistence（application 持久化适配）

> 独占 SQLite 连接、物理 schema、损坏分流与灾难副本；不承载 GPUI 状态或领域 JSON 第二通道。

## 决策与红线

- **后台 I/O**：数据库打开、事务、完整性检查与备份不得在 GPUI 前台执行；生产只由启动协调器与各库专用写线程调用。原因：磁盘与锁等待不能冻结窗口。边界：core 不得依赖本模块，测试只使用临时 `AppPaths`。
- **当前 schema**：两个库分别固定 `application_id` 与 `user_version=1`，仅接受当前物理 schema，不建立旧库升级链。原因：持久结构必须有单一权威。边界：更高版本 query-only，不隔离、不降级、不覆盖；结构损坏、非本应用库与存储合同失败分别报告。
- **文本主键非空**：SQLite rowid 表的 `TEXT PRIMARY KEY` 不隐含 `NOT NULL`，所有文本身份键必须显式声明非空。原因：否则空身份可绕过主键唯一性并形成无法寻址的记录。边界：复合主键各列同样逐项非空；测试必须真实插入 `NULL`，不能只检查 schema 文本。
- **灾难副本**：Online Backup 只写入由本模块 `create_new` 保留的新临时库，完成 application、版本、完整性与外键复核后才算成功。原因：任何失败都不得留下可被误认为有效的副本。边界：原子替换与 ACL 保留由统一文件写所有者完成，本模块不覆盖既有目标。
- **数据边界**：正文、终端输出、AI 会话正文与凭据不进入 SQLite；错误展示不得回显路径、SQL、正文或底层 SQLite 消息。旧 JSON 与历史材料不读取、不迁移、不删除。
- **恢复材料显式清理**：Journal 步骤、隔离项与快照索引不得随父记录或 Document 隐式级联删除。原因：外部文件仍需按指纹、状态与维护流程处理，数据库级联会提前丢失定位事实。边界：项目布局等纯数据库记录可按项目删除契约级联；文件-backed 材料只能由对应所有者显式清理。
- **Operation 幂等派发**：登记结果必须区分 `Registered` 与 `Existing`，调用方只能派发 `Registered`；同 `store_epoch＋client_id＋mutation_id` 且方法、参数 DTO 版本、参数散列一致时返回原记录，任一不同即冲突。原因：断线重试不得重复产生副作用，也不得用重试输入覆盖原操作。边界：`operation_id` 另作唯一身份；`OutcomeUnknown` 只可重查，不得自动重发。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests persistence -- --test-threads=1`。
- 临时真实 SQLite 覆盖 schema、WAL、外键、单行与幂等约束、事务回滚、当前／高版本分流和损坏不改写；工作区另行覆盖 Online Backup 与 Operation 登记去重、冲突、状态转换和结果重查，学习库覆盖去重、UTF-8 字节上限与学习代次，且不出现副本／导出 API。

## 操作

- 修改 schema 必须同步对象定义、完整性检查、备份验证与约束测试；不接受只加字段的隐式迁移。
- 新库路径只从 `AppPaths` 获取，不读取环境变量或另拼数据根。
