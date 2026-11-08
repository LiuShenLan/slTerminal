# persistence（application 持久化适配）

> 独占 SQLite 连接、物理 schema、损坏分流与灾难副本；不承载 GPUI 状态或领域 JSON 第二通道。

## 决策与红线

- **后台 I/O**：数据库打开、事务、完整性检查与备份不得在 GPUI 前台执行；生产只由启动协调器与各库专用写线程调用。原因：磁盘与锁等待不能冻结窗口。边界：core 不得依赖本模块，测试只使用临时 `AppPaths`。
- **当前 schema**：两个库分别固定 `application_id` 与 `user_version=1`，仅接受当前物理 schema，不建立旧库升级链。原因：持久结构必须有单一权威。边界：更高版本 query-only，不隔离、不降级、不覆盖；结构损坏、非本应用库与存储合同失败分别报告。
- **文本主键非空**：SQLite rowid 表的 `TEXT PRIMARY KEY` 不隐含 `NOT NULL`，所有文本身份键必须显式声明非空。原因：否则空身份可绕过主键唯一性并形成无法寻址的记录。边界：复合主键各列同样逐项非空；测试必须真实插入 `NULL`，不能只检查 schema 文本。
- **灾难副本**：Online Backup 只写入由本模块 `create_new` 保留的新临时库，完成 application、版本、完整性与外键复核后才算成功。原因：任何失败都不得留下可被误认为有效的副本。边界：原子替换与 ACL 保留由统一文件写所有者完成，本模块不覆盖既有目标。
- **数据边界**：正文、终端输出、AI 会话正文与凭据不进入 SQLite；错误展示不得回显路径、SQL、正文或底层 SQLite 消息。旧 JSON 与历史材料不读取、不迁移、不删除。
- **Workspace 写线程**：`WorkspaceWriter` 独占工作区库写连接；ViewState 按 ViewKey 500 毫秒去抖，同键只保留最新待写副本；侧栏组织状态按 500 毫秒去抖且只保留最新 `SidebarState`；项目列表、布局、逻辑终端与活跃项按 2 秒去抖，只保留最新 `ProjectSnapshot`，保存请求经有界队列非阻塞提交。原因：前台不得等待磁盘，连续修改不得放大写盘或排队副本。边界：写失败取消定时重试但保留副本，后续修改、flush 与 shutdown 均重试；容量 1 的失败信号只负责提示，待写副本才是重试事实；UI 只持共享命令队列与独立终态队列的 client，普通队列满时最终状态仍可经终态队列非阻塞移交；writer owner 与失败转发线程由 GPUI 外 `GlobalCloseRuntime` 独占，正常退出在窗口销毁前 shutdown 并回执，异常断连由 `application.run` 返回后的 join 兜底，终态接收端由写线程独占，`Flush` 与 `Shutdown` 均先排空终态再写同一待写集合，最后终态覆盖普通队列旧副本；shutdown 先在后台排空终态再最终冲刷，失败必须记录脱敏诊断；启动维护或初始化失败不得回退无 client 窗口，运行写失败只经壳事件投递不含路径、SQL、键或状态正文的泛化提示；flush/shutdown 会等待结果，只在后台执行体调用，Drop 复用退出冲刷路径。
- **侧栏组织状态单通道**：`persistence::sidebar` 只在 `app_state` 单行保存 core 规范 SidebarState DTO；侧栏 upsert 与项目快照写分别只更新所属列，互不覆盖。原因：组织状态与私有 ViewState、项目布局是不同持久事实，不能形成第二序列化或整行重写。边界：无记录与整体损坏分流，字段级修复保留警告；启动未发生显式编辑时不回写默认态，普通队列拒收的最新状态经独立终态通道在 shutdown 前补交。
- **项目聚合单通道**：`persistence::project` 保存前先经事务外 pure preflight 完成 `ProjectSnapshot` 领域校验、全部项目／终端无损编码、活跃项与 DTO 版本转换，再于同一 Immediate 事务内保存完整 `ProjectSnapshot`：项目顺序、活跃项、稳定组身份、规范 `ProjectLayout` 与被布局引用的逻辑终端记录；事务内只读取数据库并消费 Prepared 数据，不重复编码，写锁占用不得令非法输入等待 busy_timeout 或改报数据库错误。不得另建 Terminal 写队列或跨表半提交。原因：项目、布局与恢复元数据只有一致提交才能作为重启事实，保留终端也不能因快照重写而重建。边界：布局内容不变不推进 revision，变化时单调递增且耗尽拒写；保留项目在事务内先腾空根唯一约束，允许项目间交换根且不重建终端；终端身份不得改绑项目或编号，项目计数器不得回退，新 Terminal 编号不得低于该项目已存计数器高水位，仅 cwd、Shell、自定义标题与运行意图可更新；公共加载在同一 Immediate 事务读取完整快照，避免混合并发提交世代；保存与加载均须复核新旧快照的项目关系、布局 DTO、组身份、Terminal 引用、归属与编号，损坏、孤儿或不支持版本分别报错，不得静默修复或丢记录；任何失败整体回滚，非 Unicode 路径拒绝有损编码。
- **ViewState 单通道**：`persistence::view_state` 独占视图状态 SQLite 读写；记录保存完整规范 ViewKey 与 ViewState DTO，版本列必须与 DTO 一致，任一版本来源超前均拒写且不覆盖，作用域只由匹配 ViewKey 推导，revision 从零单调递增且耗尽拒写。原因：关闭重开与重启必须共享一份状态，损坏与缺失不能混淆。边界：项目清理将项目作用域删除与 Nav／Explorer 分区更新放在同一 Immediate 事务，AgentRoot 清理只更新 AgentFiles 对应根分区；任一步失败整体回滚并保留已提交记录供重试，缺失身份不产生新 revision，不得清空整个应用级 SideView 或影响无关视图。
- **恢复材料显式清理**：Journal 步骤、隔离项与快照索引不得随父记录或 Document 隐式级联删除。原因：外部文件仍需按指纹、状态与维护流程处理，数据库级联会提前丢失定位事实。边界：项目布局等纯数据库记录可按项目删除契约级联；文件-backed 材料只能由对应所有者显式清理。
- **运行与世代单点**：正常重启只更换 `run_id` 并保留 `store_epoch`，上一运行的正常退出事实必须可供启动恢复读取；只有显式维护提交才能同时按当前 `run_id` 与预期旧世代更换 `store_epoch`。原因：普通重启不得使 Runtime 幂等命名空间失效，迟到的旧运行关闭或维护写也不得改写新事实。边界：首次初始化才采用调用方给定的初始世代；更高版本只读，同值更换与空身份拒绝，旧运行记录不因轮换删除。
- **启动维护先于写线程**：主实例在 OS 单实例锁后按工作区库打开 → `begin_run` → `MaintenanceRunner` 空注册表对账 → writer 与失败转发器 → 领域恢复 → 服务 → `GlobalCloseRuntime` → GPUI 窗口组合；只有维护 `Ready` 才创建 writer，并沿用同一次 `begin_run` 的当前元数据与上一运行事实。维护需人工处理或执行失败经独立封闭错误分类上抛，展示脱敏；已登记运行保持 `clean_shutdown=false`，组合根立即无窗口返回，不启动领域恢复、服务或普通后台任务。原因：写线程与运行副作用必须建立在全部持久事实安全之后。边界：本切片不提供只读诊断 UI，后续另行实现；禁止回到无 client 降级启动。
- **世代写闸门**：Operation 与 Journal 的登记、状态转换和步骤事实写入必须在同一 Immediate 事务内匹配 `workspace_meta` 当前 `store_epoch`。原因：恢复或撤回轮换后，迟到的旧 Runtime 不得借幂等重试或恢复写重新产生副作用。边界：无运行元数据同样拒写；读取、结果重查与未完成 Journal 枚举不受限，旧记录保留，轮换只为新世代重开幂等命名空间。
- **关闭意图持久合同**：`close_intents` 头表独占 scope、可选项目身份、世代、提交指纹与状态，`close_objects` 只保存对象键、代次与决定；空关闭集合同样完整登记，对象身份由 application 构造，本端口不解释 Terminal／Document 类型。原因：关闭恢复必须读取一份规范事实，不能在每个对象上重复头状态或猜测对象领域。边界：登记与读取均在 Immediate 事务内完成，登记先在事务外构造 EncodedRegistration 完成全部字段校验与编码，再进入事务核对当前世代与 close_id 冲突；写锁占用不得令非法登记等待 busy_timeout 或改报数据库错误，冲突不改写；读取顺序确定，未知枚举、作用域项目不一致、空身份、错误指纹长度与负代次均为损坏，不静默修复；提交状态只允许 `Collected → StateCommitted → PtyStopped → CleanupCommitted → Completed` 顺序推进，并在同一事务核对预期旧状态、指纹与当前世代，冲突与失败不改写，Completed 保留历史但不进入未完成枚举；ProjectSnapshot 与 Collected 关闭意图的组合提交只经 `WorkspaceWriterClient` 非阻塞封闭 Command，由唯一 writer 在同一 Immediate 事务完成快照保存、关闭登记与 StateCommitted 推进后一次 commit，调用方只等待独立回执，队列满或断连立即报错且不另开 SQLite 连接；组合提交开启事务前必须完成快照 Prepared 与关闭 EncodedRegistration 两侧 preflight，事务 helper 不重复编码，任一步失败整体回滚，外部不能观察 Collected 中间态；组合提交前先将终态排入同一待写集合，仅成功后清除旧项目待写，失败保留供重试，之后新到普通保存仍可覆盖；后续状态推进同样只经 `WorkspaceWriterClient` 的非阻塞封闭 Command 与独立回执，由唯一 writer 执行，调用方不得另开 SQLite 连接或直接写库。通用组合提交拒绝 Project scope；项目关闭初始提交只保存候选中的其他项目与新活跃项，目标项目、布局与终端记录必须保留到 PtyStopped 后的专用 cleanup；cleanup 在同一 Immediate 事务精确清理项目 ViewState 与 Nav／Explorer 分区、恰好删除一条项目记录并经外键级联删除纯数据库布局与终端，再推进 CleanupCommitted，任一步失败整体回滚并允许同一 current 重试，Project 的 PtyStopped→CleanupCommitted 只能经专用 cleanup，通用 advance 拒绝该边；Completed 继续走通用 advance。writer 执行 cleanup 前先将终态并入待写集合并预先完成待写 SideView 转换，仅成功后同步清除旧项目快照、目标项目待写视图与待写 Nav／Explorer 目标分区，保留无关字段修改；失败保留待写副本，禁止只清 project_snapshot 而让旧 SideView 复活项目分区。局部关闭所需当前 `store_epoch` 由 `AppRootPersistence` 在既有启动恢复连接的 `metadata` 读取中取得并移交 AppRoot，不新增第二读取通道；writer 存在但当前世代缺失或无效时关闭必须明确失败，不能以无 writer 的内存降级绕过启动硬闸门。项目快照恢复失败时，新局部关闭必须在调用组合提交前拒绝，不得借 writer 原子事务绕过普通与终态项目写屏障；既有失败关闭只按原持久阶段重试。
- **Operation 幂等派发**：登记结果必须区分 `Registered` 与 `Existing`，调用方只能派发 `Registered`；同 `store_epoch＋client_id＋mutation_id` 且方法、参数 DTO 版本、参数散列一致时返回原记录，任一不同即冲突。原因：断线重试不得重复产生副作用，也不得用重试输入覆盖原操作。边界：`operation_id` 另作唯一身份；`OutcomeUnknown` 只可重查，不得自动重发。
- **Journal 专门恢复**：Journal 与全部步骤、Operation 关联必须在同一事务登记；通用存储只提供未完成事实与显式状态边，不解释 `kind`、不自动重放或删除材料。原因：只有所属类型的恢复器能根据目标 DTO、前后指纹与真实文件状态判定提交、回滚或继续。边界：状态不明进入 `NeedsAttention` 并保留全部事实；终态步骤与指纹不可改写，清理另走显式审计流程。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests persistence -- --test-threads=1`。
- 临时真实 SQLite 覆盖 schema、WAL、外键、单行与幂等约束、事务回滚、当前／高版本分流和损坏不改写；工作区另行覆盖 Online Backup、运行标记、正常／异常上次运行、`store_epoch` 更换与旧世代写拒绝、提交后异常退出的 WAL 恢复、Operation 登记去重与结果重查、Journal 全状态恢复观察与关联回滚、关闭意图空／多对象往返、冲突不改写、损坏分流、顺序推进与未完成过滤、ViewState 规范读写／作用域清理／写失败重试、项目聚合 2 秒最新快照去抖、失败重试、队列满拒收、断连冲刷与混合报告，学习库覆盖去重、UTF-8 字节上限与学习代次，且不出现副本／导出 API。

## 操作

- 修改 schema 必须同步对象定义、完整性检查、备份验证与约束测试；不接受只加字段的隐式迁移。
- 新库路径只从 `AppPaths` 获取，不读取环境变量或另拼数据根。
