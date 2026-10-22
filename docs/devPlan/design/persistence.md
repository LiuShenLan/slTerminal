# 持久化与恢复

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 结构化工作区状态使用本地 SQLite 单库：项目、布局、终端簿、侧栏与视图状态经同一写线程与事务提交。应用设置保留 `settings.json` 与上一代备份；崩溃快照、隔离区、Runtime 端点、实例锁与更新状态使用本地根文件；窗口几何是迁移后能力。本文件是状态库 schema、事务、去抖、损坏隔离与侧栏/视图状态 schema 的唯一所有者。

### 数据根与文件

两个根由 `slterm_settings::paths` 单点解析；其他模块不得拼接：

```text
<data>/                         # 漫游根，SLTERM_CONFIG_DIR 覆盖，默认 %APPDATA%\slterm
  state/
    settings.json               # 应用设置；.bak；损坏隔离
    quarantine/                 # 设置损坏隔离件
<local>/                        # 本地根，SLTERM_LOCAL_DIR 覆盖，默认 %LOCALAPPDATA%\slterm
  state/
    state.db                    # 结构化状态库
    state.db.bak                # 正常关闭时的可恢复备份
    state.db-wal / state.db-shm # SQLite 运行时文件
    instance.lock               # 单实例句柄锁
    runtime.port                # Runtime v2 端点与 token
    window.json                 # 迁移后能力；规格内不创建、不冲刷，归 platform-windows
    recovery/<project-id>/<panel-id>.json
    quarantine/                 # 状态库/快照隔离件
    completion/history.jsonl
    completion/directories.json
    usage/                      # statusline 用量信号，每面板 JSON
    agent-sessions/isolation/   # 历史会话删除隔离区；启动清扫，容量受数据根配额约束
    hooks/                      # 注入脚本、锁与 statusline 备份
    update/                     # prompt/downloads/handoffs/last-handoff
    backup/                     # 加密备份恢复点
  logs/                         # 日志轮转文件
  audit/                        # audit.log
  crash/                        # panic.log 与崩溃标记
```

- 旧 `settings.json`、`session.json`、旧 `runtime.port`、旧 `updates/` 等文件位于漫游根旧位置；新代码不读、不删、不视为损坏。
- `state.db` 在本地根，项目根路径不随漫游配置跨机复制；应用设置是唯一漫游状态。
- 窗口几何属于迁移后能力：规格内不创建 `window.json`，关闭链不冲刷窗口几何，也不进入当前数据根清理范围。
- 本文件列出的托管路径与文件写入经 `security.md` 的 `AppManaged` 作用域；`settings_store`、状态库写线程与快照/隔离逻辑共用该入口的审计与原子写规则。
- 历史会话删除隔离区位于 `<local>/state/agent-sessions/isolation/`：删除/撤销由 app provider 经 `PathAuthority` 执行；5 秒窗口结束、提示关闭、窗口关闭或应用退出后永久化，启动清扫遗留项视为永久删除；容量受数据根总配额约束。
- 目录创建在首次写入前完成；读取缺失文件不等于损坏。
- 测试同时注入 `SLTERM_CONFIG_DIR` 与 `SLTERM_LOCAL_DIR`；生产根永不被测试触碰。

### 状态库 schema

```sql
PRAGMA user_version = 1;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA foreign_keys = ON;

CREATE TABLE workspace (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  active_project_id INTEGER,
  next_project_id   INTEGER NOT NULL,
  next_group_id     INTEGER NOT NULL,
  next_panel_id     INTEGER NOT NULL
);

CREATE TABLE projects (
  id           INTEGER PRIMARY KEY,
  root         TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  sort_index   INTEGER NOT NULL
);

CREATE TABLE project_layouts (
  project_id  INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  layout_json TEXT NOT NULL
);

CREATE TABLE project_terminals (
  project_id     INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  terminals_json TEXT NOT NULL
);

CREATE TABLE sidebar (
  id          INTEGER PRIMARY KEY CHECK (id = 1),
  layout_json TEXT NOT NULL
);

CREATE TABLE view_states (
  view_id    TEXT PRIMARY KEY,
  state_json TEXT NOT NULL
);

CREATE TABLE recovery_index (
  project_id  INTEGER NOT NULL,
  panel_id    INTEGER NOT NULL,
  file_name   TEXT NOT NULL,
  saved_at_ms INTEGER NOT NULL,
  text_bytes  INTEGER NOT NULL,
  PRIMARY KEY (project_id, panel_id)
);
```

- `layout_json` 是 `ProjectLayoutDto`；`terminals_json` 是 `TerminalsDto`；两者字段与恢复对账归 `domain-model.md`。`sidebar.layout_json` 是侧栏布局；`view_states.state_json` 是各视图声明状态。
- `view_states` 每个视图一行；`state_json` 内部按视图 owner 声明的作用域键控（导航树展开集合按 `ProjectId`，滚动为应用级单值），持久化层只存取、不解释内部键；单视图 256 KiB 上限由该视图全部项目/节点状态共享，超限拒绝写入并提示。
- `workspace.next_*` 是全局 ID 游标；恢复时取 `max(next_*, maxSeen + 1, maxDir + 1)`，`maxDir` 为 `recovery/` 目录名中的最大值。状态库不保存真实路径；项目根真实路径在运行时解析。
- `projects.root` 是归一化词法路径并唯一；`sort_index` 是项目顺序。
- 所有 JSON 载荷在写入前由前台生成不可变字节；后台线程不得回读实体。
- 单载荷上限：布局 4 MiB、终端簿 1 MiB、侧栏 256 KiB、单视图 256 KiB；超限拒绝写入并通知。状态库总大小软上限 256 MiB，超限只读并提示，不自动删除数据。这些上限是持久化设计的契约值，其他文档引用本节，不复述数值。
- `PRAGMA user_version` 精确匹配；更高或更低均不兼容。不提供升级/降级函数。

### 写入模型与事务

```rust
pub enum StateFacet {
    Workspace,
    ProjectLayout(ProjectId),
    ProjectTerminals(ProjectId),
    Sidebar,
    ViewState(ViewId),
}

pub enum WriteMode { Debounced(Duration), Immediate }

pub struct PersistenceCoordinator {
    // 单写线程持有唯一可写连接；前台只提交字节快照
}

impl PersistenceCoordinator {
    pub fn mark(&mut self, facet: StateFacet, snapshot: Vec<u8>, mode: WriteMode);
    pub fn mark_settings(&mut self, snapshot: Vec<u8>, mode: WriteMode);
    pub fn retry(&mut self, facet: StateFacet);
    pub async fn flush(&mut self, deadline: Duration) -> FlushReport;
    pub async fn shutdown(&mut self, deadline: Duration) -> FlushReport;
}
```

- 每个 facet 至多一个待写快照；新快照替换旧快照并抬高 facet generation。写入在途期间到达的新快照保留为 dirty，提交后再次调度。
- 到期或显式 `Immediate` 时，把当前全部 dirty facet 合入一个事务提交；设置 JSON 独立写入，不进入状态库事务。
- 事务成功后清空对应 generation；失败保留 dirty 与错误，前台显示失败徽标/通知，`retry` 只重试失败 facet。
- `flush` 把所有待写 facet 的 due 置为当前时刻，等待设置与状态库均落地或超时；不取消去抖计时器，不丢最新快照。
- 关闭冲刷分类：
  - `Guarded`：设置、状态库全部 dirty facet。仅在窗口关闭链生效；失败中止关闭，提供重试/取消。
  - `BestEffort`：崩溃快照清理、隔离件清理、更新状态收尾。失败记录日志并继续。
- 冲刷范围边界：项目移除不复用窗口关闭链，只提交项目相关状态库事务，成功后再删除快照目录；批量关闭只按工作区关闭守卫冲刷被关闭面板的待写自动保存（面板级），不触发全局设置、侧栏布局或视图状态冲刷。
- 设置只读或持续失败时不进入“重试—失败”循环：守卫只提供放弃与取消。

去抖：

| 域 | 去抖 | 合并 |
|---|---|---|
| 应用设置 | 2 秒 | 同域连续修改合并为最新完整设置快照；显式提交/导入/恢复默认立即 |
| 项目索引与活跃项目 | 立即提交 | 连续重命名/排序/切换活跃项目合并为同一事务 |
| 项目布局 | 2 秒 | 结构、顺序、激活面板、聚焦组、面板参数合并为最新完整布局 |
| 终端编号/cwd | 300ms | 合并为最新终端簿 |
| 终端自定义标题 | 立即 | 当前事务包含终端簿与布局的最新快照 |
| 侧栏布局 + 视图状态 | 500ms | 同事务提交 dirty facet |
| 崩溃快照 | 停止编辑 2 秒；连续编辑最长 30 秒 | 按面板合并；旧结果丢弃 |

### 跨 facet 一致性

- **项目创建**：一个事务插入 `projects`、空 `project_layouts`、空 `project_terminals`，并更新 `workspace.active_project_id` 与游标。失败则项目不存在。
- **项目移除**：一个事务删除项目行（级联布局/终端簿）与 `recovery_index` 行，并更新活跃项目。事务成功后再删除 `recovery/<id>/` 文件；文件删除失败留待启动清扫，不影响索引。
- **终端创建/关闭**：一个事务同时更新 `project_terminals` 与 `project_layouts`；不再需要先写终端簿再写布局。
- **面板移动/拆分/合并/参数变更**：只提交 `project_layouts`。
- **根重定位**：一个事务更新 `projects.root`；布局中的项目相对路径在恢复时按新根解析。
- **侧栏与视图状态**：同一事务提交；视图状态独立，损坏只影响该视图。
- **设置与工作区状态**：彼此独立；关闭冲刷统一等待，不承诺跨根原子性。

恢复对账必须容忍任意已提交状态；状态库事务保证同一事务内不会出现半套引用。

### 启动恢复

本节描述状态库自身的恢复子序列；应用启动链在打开状态库前已读取设置（见 `architecture.md` §启动链），步骤编号不表示应用级启动顺序。

1. 打开 `<local>/state/state.db`；缺失则创建 schema 与空 `workspace` 行。
2. 校验 `PRAGMA user_version` 与 `PRAGMA quick_check`。
   - 版本不匹配或检查失败：先校验 `state.db.bak` 的版本与完整性。备份可用时显示项目数据错误界面，提供 `从备份恢复`、`重试` 与 `以空状态继续`；选择恢复时先隔离损坏主库与 WAL/SHM，再以备份重建主库。
   - 备份缺失或备份也损坏：把主库、WAL/SHM 与备份移入 `<local>/state/quarantine/`，显示项目数据错误界面，提供 `重试` 与 `以空状态继续`；用户可手动取回隔离件。
   - `以空状态继续` 创建新库；隔离失败时保持只读并拒绝写入。
3. 读取 `workspace` 与 `projects`；索引缺失/非法按空工作区处理。
4. 按项目读取 `layout_json` 与 `terminals_json`，执行 `domain-model.md` 的恢复对账。单项目载荷解析失败：该项目空布局/空终端簿，原始 JSON 移入 `quarantine/`，并入启动聚合警告；其他项目不受影响。
5. 读取 `sidebar` 与 `view_states`；单行解析失败只回退该视图默认并提示一次。
6. 扫描 `recovery/` 与 `recovery_index`：无索引的文件经校验后补索引；无文件的索引行删除；损坏快照移入 `quarantine/` 并从提示排除。
7. 读取设置 JSON；损坏按设置域规则回退 `.bak`/默认。
8. 启动警告聚合为一条通知；布局恢复不产生焦点意图、不抢键盘焦点。

### 崩溃快照与隔离

- 快照文件位于 `<local>/state/recovery/<project-id>/<panel-id>.json`；索引行在 `recovery_index`。快照键固定为 `(ProjectId, PanelId)`，文件路径与索引主键都由该键派生；文件重命名、另存为或面板类型转换不改变键。仅保存脏的可编辑编辑器与 Markdown 源码态文本；不保存终端输出、Diff 工作区、剪贴板、凭据。
- 编辑器与 Markdown 共用同一 `RecoveryStore`、同一配额与淘汰规则；载荷保存目标文件路径，供恢复时按当前磁盘内容重建外部修改基线。
- 写入顺序：先原子写快照文件，再 upsert 索引行；索引写失败时下次启动按孤儿文件处理。删除顺序：先删索引行，再删文件；文件删除失败留待清扫。
- 单文件文本上限 64 MiB，总配额 256 MiB，保留 7 天。超出总配额按 `saved_at_ms` 从旧到新淘汰，不淘汰当前面板最新快照；仍不足则跳过并提示一次。
- 项目移除提交事务后删除该项目快照目录与索引行；保存成功、撤销回干净、面板正常关闭删除对应快照与索引行；正常关闭窗口清理全部快照与索引行。
- 恢复只重建文本缓冲并标脏，不写回磁盘、不恢复光标/滚动/撤销；按当前磁盘内容重建外部修改基线。

### 设置 JSON

- `settings.json` 继续经 `settings_store`：键白名单、段级浅合并、1 MiB 上限、审计包装、临时文件 + 原子替换、上一代 `.bak`。
- `settings_store` 暴露受审计包装的提交入口，私有写核心只由该入口调用；`PersistenceCoordinator` 经该入口写入，导入/导出/恢复默认走同一通道。
- 损坏：先复制到 `<data>/state/quarantine/`，隔离成功后尝试 `.bak`，双损回默认并提示；版本高于当前进入只读，禁止降级覆盖。
- 设置与状态库的失败互不代偿：设置失败不阻止工作区打开；状态库失败不阻止设置加载。

### 损坏、备份与只读

- 状态库事务与 WAL 保证崩溃原子性；不逐事务复制整库。
- 正常关闭完成最终提交后，用 SQLite backup API 生成 `<local>/state/state.db.bak`；备份失败只记录并通知，不阻止关闭。启动时主库不可用且备份完整时提供“从备份恢复”入口，先隔离损坏主库，再以备份重建。
- 复用策略：主库恢复失败不自动用空库覆盖；备份可用时优先尝试备份恢复，备份缺失或也损坏时才把损坏库与备份一并移入 `quarantine/`，用户可手动取回。
- 只读场景：磁盘满、权限不足、本地根不可写、状态库超限。只读时工作区可浏览，写命令返回可见错误；关闭守卫只提供放弃/取消。
- 状态库与备份文件不写入日志、审计或诊断包内容。

### 加密备份归档与恢复点

- 归档格式为 `SLTBK002 | salt | nonce | ciphertext`：AES-256-GCM，魔数作 AAD，Argon2id 派生密钥；口令只用于本次操作，不持久化。旧 `TRMBK001` 及更早归档拒绝并提示“旧格式不受支持”。
- 类别：应用设置、项目注册表、项目布局、终端簿、侧栏与视图状态、Provider 元数据（仅 ID、显示元数据与 hint）。凭据值、Hooks/CLI 配置、AI 会话正文、终端内容、剪贴板、崩溃快照与日志不入包。
- 导出：收集选定类别 → 按 `security.md` 的敏感键表剥除 → 加密 → 原子写；失败不留下半写文件。
- 导入：版本/格式校验 → 解密 → 敏感键复验 → 预览 → 备份原字节 → 原子替换；失败回滚原字节。
- 恢复点：一次导出/恢复生成一个恢复点，记录类别、目标集合、时间与归档路径；撤回只作用于对应恢复操作，跨重启清单保留到下一次成功恢复或用户手动清理。恢复点目标集合不匹配时拒绝。
- 恢复点文件位于 `<local>/state/backup/`；旧 `backup-recovery/` 不读、不删、不视为恢复点。敏感键表归 `security.md`，本文件只定义归档、类别与恢复点状态机。

### 线程与生命周期

- 前台线程只生成 DTO 字节快照、更新内存状态、派发命令与重绘；禁止在 GPUI 前台做 SQLite 调用。
- 单写线程持有唯一可写连接；读连接只用于启动恢复、诊断与备份。WAL 允许读不阻塞写，但业务读取仍以前台内存为准。
- 关闭链的完整顺序归 `architecture.md` §关闭链；本文件只约束写线程生命周期：冲刷完成前不关闭连接，最终提交与备份完成后关闭写连接。
- 写线程崩溃或 panic：转为启动期错误，不得静默丢写；下次启动按事务恢复。

### 可注入时钟与故障注入

```rust
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
    fn wall_clock_ms(&self) -> i64;
}

pub enum FaultPoint { Open, Read, Write, Commit, Backup, Checkpoint, Remove }

pub enum FaultAction { Fail(io::ErrorKind), Delay(Duration), CorruptPage }
```

- 测试注入点：打开失败、事务提交失败、WAL 写失败、备份失败、checkpoint 失败、删除失败、页损坏。
- `ManualClock::advance` 驱动 2s/500ms/300ms/立即语义；测试禁止真实 `sleep`。
- 状态库测试使用临时目录与独立 `state.db`；不触碰生产根。

### 无迁移策略

- 新代码不读取旧 `session.json`、旧根 `settings.json`、旧 `.bak`、旧隔离件、旧 `runtime.port` 或旧更新状态。
- 旧文件不删除、不改写、不在启动时判定为损坏。
- 新数据路径与旧路径不重叠；`slterm_settings::settings_path()` 指向 `<data>/state/settings.json`。
- 目标态只接受当前 `user_version`；无升级函数、无兼容字段映射、无“尽力解析”。

### 持久化测试接缝

- schema：空库创建、`user_version` 不匹配、`quick_check` 失败隔离、隔离失败只读；主库损坏且备份可用/备份损坏/主库与备份双损三种启动路径。
- 事务：项目创建/移除、终端创建/关闭、根重定位的原子性与中间态恢复。
- 去抖：手动时钟验证各域阈值前不写、阈值后一次事务；连续变更只提交最后快照；`Immediate` 合并 dirty facet。
- generation：写入在途产生新快照，旧提交完成不回退状态；失败保留 dirty。
- 载荷：布局/终端簿/侧栏/视图超限拒绝；单行 JSON 损坏只回退该 facet。
- 快照：索引与文件双向对账、配额淘汰、7 天保留、同路径多项目、项目移除清理、损坏隔离；键固定 `(ProjectId, PanelId)`，重命名/另存为/跨类型转换后仍按原键恢复与删除；编辑器与 Markdown 共用配额。
- 设置：`.bak` 备份失败中止、双损回默认、版本过高只读。
- 关闭链：3 秒超时、Guarded 失败重试/取消、最终终端簿事务、备份失败只记录；项目移除与批量关闭不触发全局设置/侧栏冲刷。
- 路径隔离：漫游根与本地根分别注入临时目录；生产 `%APPDATA%`/`%LOCALAPPDATA%` 不被触碰。
