# 持久化与恢复

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

持久化层是唯一写盘通道：所有文件经 `PersistenceCoordinator` 串行写入，使用临时文件 + 原子替换、上一代 `.bak`、损坏隔离与 generation 防旧快照覆盖。设置编码复用 `slterm_settings` 与 `settings_store` 语义；文件原语复用 `atomic_file`。

### 文件划分

| 文件 | 内容 | 去抖 | 所有者 |
|---|---|---|---|
| `state/settings.json` | 应用设置全部域 | 2 秒；显式提交立即 | 设置与持久化 |
| `state/workspace.json` | 项目列表、顺序、显示名、根路径、活跃项目、全局 ID 游标 | 立即入队 + 合并 | 工作区壳 |
| `state/sidebar.json` | 侧栏布局：可见意图、宽度、比例、槽位、按钮组织/显隐 | 500ms | 侧栏框架 |
| `state/views/<view-id>.json` | 各侧栏视图声明的界面状态 | 500ms | 各视图 |
| `state/projects/<id>/layout.json` | 布局树、面板组、顺序、激活面板、聚焦组、面板参数 | 2 秒 | 工作区壳 |
| `state/projects/<id>/terminals.json` | 终端编号计数、逐终端编号/自定义标题/最近 cwd | 编号与 cwd 300ms；自定义标题立即 | 终端面板 |
| `state/recovery/<project-id>/<panel-id>.json` | 崩溃恢复快照（仅未保存文本） | 停止编辑 2 秒；连续编辑最长 30 秒刷新 | 编辑器 / Markdown |
| `state/runtime.port` | Runtime 端点与 token | 启动时 | Runtime API |
| `state/instance.lock` | 单实例 owner 锁 | 启动时 | 单实例服务 |

不新增全局 `session.json`。旧根文件不读、不删。

### 通用信封与版本策略

除 `settings.json` 复用设置段模型外，所有新文件使用统一信封：

```json
{
  "schema_version": 1,
  "generation": 12,
  "payload": { }
}
```

实际落盘可把 `payload` 字段平铺到根（减少嵌套），但语义仍是“版本 + 代数 + 载荷”。字段级示例见下节。

- `schema_version`：整数，精确等于当前支持版本；更高或更低均判不兼容。不提供升级/降级函数。
- `generation`：每个文件独立单调递增；写入方在内存中分配，完成后记录 `last_written_generation`。迟到的低代数完成结果被丢弃，绝不覆盖新状态。
- 追加可选字段：`serde(default)`，不升版本；未知字段读取时忽略。
- 字段改名/删除/类型变化/语义变化：升 `schema_version`，旧文件判不兼容。
- 单文件大小上限（解析前检查）：设置 1 MiB（契约）；工作区索引 1 MiB；单项目布局 4 MiB；终端簿 1 MiB；侧栏 256 KiB；单个视图状态 256 KiB；崩溃快照文本 64 MiB。超限按损坏/不支持处理，不截断解析。

### `state/workspace.json`

```json
{
  "schema_version": 1,
  "generation": 4,
  "active_project": "project:7",
  "next_project_id": 8,
  "next_panel_group_id": 41,
  "next_panel_id": 97,
  "projects": [
    {
      "id": "project:7",
      "root": "D:/work/a",
      "display_name": "a"
    }
  ]
}
```

- `projects` 数组顺序即导航树顺序；显示名可重复。
- `root` 为归一化词法路径；真实路径在运行时解析，不进文件。
- `active_project` 必须存在于 `projects`；恢复时无效则取第一个项目，空列表为 `null`。
- `next_*` 是全局分配器游标；恢复时与已见最大 ID 取较大者。
- 项目布局不内联，避免单文件无界增长与单项目损坏扩散。

### `state/projects/<id>/layout.json`

```json
{
  "schema_version": 1,
  "generation": 12,
  "project_id": "project:7",
  "focused_group": "group:3",
  "tree": {
    "type": "split",
    "direction": "left_right",
    "ratio_permille": 500,
    "first": { "type": "group", "id": "group:3" },
    "second": { "type": "group", "id": "group:4" }
  },
  "groups": [
    { "id": "group:3", "panels": ["panel:11", "panel:12"], "active": "panel:11" },
    { "id": "group:4", "panels": ["panel:13"], "active": "panel:13" }
  ],
  "panels": [
    { "id": "panel:11", "params": { "kind": "terminal", "revision": 1 } },
    { "id": "panel:12", "params": {
        "kind": "editor", "revision": 1,
        "path": { "scope": "project_relative", "value": "src/a.ts",
                  "read_only": false, "save_as_grant": false },
        "word_wrap": true, "language_override": null } },
    { "id": "panel:13", "params": {
        "kind": "diff", "revision": 1,
        "head_path": "src/a.ts",
        "workspace_path": { "scope": "project_relative", "value": "src/a.ts",
                            "read_only": false, "save_as_grant": false },
        "view": "side_by_side", "word_wrap": false, "sync_scroll": true,
        "split_permille": 500, "collapsed_side": null, "swapped": false } }
  ]
}
```

各类型特有字段：

| 类型 | 字段 |
|---|---|
| terminal | `revision`；编号/标题/cwd 在 `terminals.json` |
| editor | `path`、`read_only`、`word_wrap`、`language_override` |
| markdown | `path`、`read_only`、`view`（`rendered`/`source`）、`outline_open`、`zoom_percent`、`scroll_anchor { anchor, offset_px }` |
| diff | `head_path`、`workspace_path`、`view`（`side_by_side`/`inline`）、`word_wrap`、`sync_scroll`、`split_permille`、`collapsed_side`（`null`/`head`/`workspace`）、`swapped` |
| gitshow | `repo_path` |
| settings | `selected_page` |

规则：

- `tree` 的叶是 `group` 节点，不直接是面板；面板属于组。
- `ratio_permille` 取值 `1..=999`；越界在恢复时夹取或按 500 处理。
- `groups[].panels` 顺序即页签顺序；`active` 必须在 `panels` 内。
- `panels` 与 `groups` 的引用必须闭合；不闭合项在恢复时丢弃。
- 空组不写盘；关闭最后面板后布局变更立即进内存并 2 秒去抖。
- 冲突快照 Diff 不持久化；恢复只接受默认模式 Diff。

### `state/projects/<id>/terminals.json`

```json
{
  "schema_version": 1,
  "generation": 9,
  "next_ordinal": 3,
  "terminals": [
    { "panel": "panel:11", "ordinal": 0, "custom_title": "后端",
      "cwd": "D:/work/a/src" },
    { "panel": "panel:12", "ordinal": 1, "custom_title": null,
      "cwd": null }
  ]
}
```

- 每条对应一个终端面板；非终端 `panel` 记录在恢复时丢弃。
- `ordinal` 在项目内唯一；重复时保留最小者，其余按 `next_ordinal` 重分配。
- `custom_title` 去除首尾空白后为空则写 `null`；最长 128 字符。
- `cwd` 为绝对路径；恢复时校验目录存在且位于允许根内，否则置 `null`，运行时回退项目根并提示。
- `next_ordinal` 只增不减；恢复时取 `max(next_ordinal, 已见最大 ordinal + 1)`。

### `state/sidebar.json`

```json
{
  "schema_version": 1,
  "generation": 5,
  "visible": true,
  "width_px": 250.0,
  "split_permille": 500,
  "slots": { "top": "nav", "bottom": null },
  "views": [
    { "id": "nav", "visible": true, "slot": "top", "order": 0 },
    { "id": "explorer", "visible": true, "slot": "top", "order": 1 },
    { "id": "commit", "visible": true, "slot": "top", "order": 2 },
    { "id": "agent-files", "visible": true, "slot": "top", "order": 3 }
  ]
}
```

- 恢复清洗规则由侧栏框架拥有：宽度 160–720、比例 10%–90%、未知视图静默丢弃、同槽冲突保留按钮顺序靠前者。
- `views` 只记布局态；各视图内部状态在 `state/views/<id>.json`，损坏只影响该视图。
- 窗口几何持久化属范围外增强，不进入本文件。

### `state/views/<view-id>.json`

```json
{
  "schema_version": 1,
  "generation": 3,
  "state": { }
}
```

- `state` 的模式由视图自身拥有（导航树展开/滚动、Commit 折叠、文件浏览器展开等）；框架只负责按 `view-id` 存取与损坏回退。
- 视图状态不得包含凭据、终端内容、文件内容或搜索词等瞬态值。
- 未注册 `view-id` 的文件不读、不删；对应视图恢复默认。

### 崩溃快照 `state/recovery/<project-id>/<panel-id>.json`

```json
{
  "schema_version": 1,
  "generation": 7,
  "project": "project:7",
  "panel": "panel:12",
  "kind": "editor",
  "path": { "scope": "project_relative", "value": "src/a.ts",
            "read_only": false, "save_as_grant": false },
  "encoding": "utf-8",
  "bom": false,
  "eol": "lf",
  "text": "…",
  "text_bytes": 12345,
  "saved_at_unix_ms": 1760000000000
}
```

- 仅保存脏的可编辑编辑器与 Markdown 源码态文本；不保存终端输出、Diff 工作区、剪贴板、凭据。
- 恢复只重建文本缓冲并标脏，不写回磁盘、不恢复光标/滚动/撤销。
- 快照路径与布局路径独立；恢复时按当前磁盘内容重新建立外部修改基线。

### 原子写、备份与损坏隔离

单次写入顺序：

1. 取目标同目录临时文件 `.name.slterm-tmp-<pid>-<seq>`，`create_new` 打开。
2. 写入全部字节，`sync_all`。
3. 复制目标已有显式 ACL（复用 `atomic_file` 的尽力策略；失败只告警）。
4. 备份当前文件为 `<name>.bak`；备份失败即中止本次写入，原文件与旧备份不变。
5. `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` 原子替换。
6. 失败时删除临时文件，目标保持不变。

- 所有写入串行：单实例 + 单写线程，不需要跨进程锁；进程内不再为每个文件加第二把锁。
- 备份只保留上一代；不做多代历史。
- 损坏判定：文件缺失不等于损坏；解析失败、根结构非法、`schema_version` 不匹配、超过大小上限均视为损坏/不兼容。
- 损坏处理：
  - 先复制到隔离文件 `<stem>.corrupted-<unix_nanos>.json`，隔离成功后才允许删除/重建主文件。
  - 隔离失败：保留原文件，进入只读或默认路径，拒绝覆盖写入。
  - 设置文件：隔离后尝试从 `.bak` 重建；双损回默认并提示。
  - 工作区索引：阻断工作区，错误界面；用户选择“以空状态继续”时先隔离再允许首次写入。
  - 单项目布局/终端簿：该项目空布局 + 聚合警告；隔离后下次写入重建。
  - 侧栏/视图状态：回默认，提示一次；不影响主区。
  - 崩溃快照：隔离到 `state/quarantine/` 并从恢复提示中排除；不阻止启动。
- 版本高于当前：只读模式，禁止写入与降级覆盖；设置中心显示只读原因；关闭守卫只提供放弃/取消。

`settings_store` 的调整：保留键白名单、段级浅合并、1 MiB 上限、审计包装；备份失败从“仅告警”改为“中止写入”，与规格一致。写函数仍私有，唯一调用方改为 `PersistenceCoordinator`。

### 去抖、合并写与 flush coordinator

去抖与合并：

| 域 | 去抖 | 合并语义 |
|---|---|---|
| 应用设置 | 2 秒 | 同域内连续修改合并为最新完整设置快照；显式提交/导入/恢复默认立即 |
| 项目索引 | 立即入队 | 连续重命名/排序/切换活跃项目合并为最新索引 |
| 侧栏布局 + 视图状态 | 500ms | 布局与各视图状态分别合并；布局先写 |
| 项目布局 | 2 秒 | 结构、顺序、激活面板、聚焦组、面板参数合并为最新完整布局 |
| 终端编号/cwd | 300ms | 合并为最新终端簿 |
| 终端自定义标题 | 立即 | 触发终端簿立即写入；与待写 300ms 字段合并 |
| 崩溃快照 | 停止编辑 2 秒；连续编辑最长 30 秒 | 按面板合并；旧代数结果丢弃 |
| 项目创建/移除 | 立即 | 按跨文件顺序执行 |

`PersistenceCoordinator` 结构：

```rust
pub enum FileKey {
    Settings,
    Workspace,
    Sidebar,
    ViewState(ViewId),
    ProjectLayout(ProjectId),
    ProjectTerminals(ProjectId),
    Recovery { project: ProjectId, panel: PanelId },
}

pub enum WriteMode { Debounced(Duration), Immediate }

pub enum FileState {
    Clean,
    Dirty { snapshot: Arc<[u8]>, generation: u64, due: Instant },
    Writing { generation: u64 },
    Failed { error: PersistError, snapshot: Arc<[u8]>, generation: u64 },
}

pub struct PersistenceCoordinator {
    states: HashMap<FileKey, FileState>,
    worker: PersistenceWorker,       // 单线程串行
    clock: Arc<dyn Clock>,
    io: Arc<dyn PersistenceIo>,
}

impl PersistenceCoordinator {
    pub fn mark(&mut self, key: FileKey, snapshot: Vec<u8>, mode: WriteMode);
    pub fn retry(&mut self, key: FileKey);
    pub async fn flush(&mut self, keys: &[FileKey], deadline: Duration) -> FlushReport;
    pub async fn shutdown(&mut self, deadline: Duration) -> FlushReport;
}
```

规则：

- 每个 `FileKey` 至多一个在途写入；在途期间新快照只替换 `Dirty` 快照并抬高 generation。
- 写入完成时比较 generation：完成代数低于当前最新代数则丢弃完成事件，不把状态回退为 `Clean`。
- `flush` 把所有待写键的 `due` 强制设为当前时刻，等待全部 `Clean` 或超时；不取消去抖计时器，不丢最新快照。
- 写失败进入 `Failed`，保留快照与错误；前台显示失败徽标/通知；`retry` 只重试该键。
- 关闭冲刷按 `FlushClass` 分类：
  - `Guarded`：设置、侧栏、工作区索引、项目布局、终端簿。失败中止关闭，提供重试/取消。
  - `BestEffort`：崩溃快照清理、视图状态。失败记录日志并继续。
- 设置只读或持续失败时不进入“重试—失败”循环：守卫只提供放弃与取消。

### 跨文件提交顺序与一致性

- **项目创建**：先写 `state/projects/<id>/layout.json`（空布局）与 `terminals.json`，成功后写 `workspace.json` 追加项目。布局失败则创建失败，索引不变。崩溃在索引前：孤儿项目目录保留，不读取、不自动删除，分配器跨过其 ID。
- **项目移除**：先写 `workspace.json` 移除条目与更新活跃项目，成功后再删除项目目录与 `state/recovery/<id>/`。索引失败则移除中止，项目保持。崩溃在删除前：孤儿目录保留。
- **新增终端面板**：先写 `terminals.json`（编号/标题/cwd），后写 `layout.json`（引用面板）。崩溃在布局前：终端条目成为孤儿，恢复时丢弃；编号不回收。
- **关闭终端面板**：先写 `layout.json`（移除引用），后写 `terminals.json`（移除条目）。崩溃在终端簿前：孤儿条目恢复时丢弃。
- **面板移动/拆分/合并/参数变更**：只写 `layout.json`。
- **根重定位**：只写 `workspace.json`；项目相对路径在恢复时按新根解析。
- **侧栏布局与视图状态**：先写 `sidebar.json`，后写各 `state/views/*.json`；视图状态文件缺失/损坏只影响该视图。
- **设置与其它域**：彼此独立；关闭冲刷时统一等待。

一致性恢复原则：引用方先移除、被引用方后移除；新增时被引用方先写入、引用方后写入。恢复对账必须容忍任一中间态，且不因中间态删除用户可见的更多数据。

### 启动恢复

- 读取顺序：配置域并行 → `workspace.json` → 各项目布局/终端簿并行 → 崩溃快照扫描。
- 布局急切恢复：全部项目、全部面板实例在首帧前建立；不按需懒加载。
- 布局恢复只读持久化数据；恢复产生的归一化/程序化变化不写回，避免反向覆盖。
- PTY 并发创建上限 8；应用级总数上限 64。超出上限的终端显示失败态，关闭其他终端后可重试。
- 单项目布局损坏：隔离原文件 → 该项目空布局 → 聚合警告；其他项目正常。
- 未知面板类型：丢弃 → 项目其余面板正常 → 聚合警告；全部丢弃则项目空态。
- 项目根不可访问：项目仍加载，文件型面板按自身错误态显示；导航树警示。
- 活跃项目无效：取列表第一个；列表为空：无活跃项目。
- 启动警告聚合为一条通知；启动期间通知排队，界面挂载后按剩余时长显示。
- 恢复不产生焦点意图、不抢键盘焦点。

### 关闭链落盘

- 冲刷环节按 `Guarded` 键强制写入，最多 3 秒；成功或失败可见。
- PTY 终止环节最多 3 秒；失败只记录。
- 项目数据保存环节以最终内存状态重新生成 DTO，写 `workspace.json`、全部 `layout.json` 与 `terminals.json`，最多 3 秒；失败只记录。
- 清理环节删除全部崩溃快照（用户已选择关闭；放弃修改同样清理），关闭 watcher/后台任务，停止写线程。
- 关闭被取消：不删除快照；写线程继续运行，去抖恢复。
- 关闭过程中新变更：命令拒绝；冲刷等待期间产生的变更合并到最终快照（若已进入项目数据保存环节则不再接受）。

### 崩溃快照配额、清理与恢复提示

- 单文件文本上限 64 MiB；超过上限不写快照，界面提示一次。
- 总配额 256 MiB；写入前统计 `state/recovery/` 总量，超出时按 `saved_at_unix_ms` 从旧到新淘汰，不淘汰当前面板的最新快照；仍不足则跳过并提示一次。
- 保留时长 7 天；启动扫描删除超期快照。
- 同路径多项目：快照键为 `(ProjectId, PanelId)`，路径只作展示；两个项目各持独立快照，恢复提示分别列出项目名、路径与时间。
- 项目移除：删除该项目全部快照。
- 保存成功、撤销回干净、面板正常关闭：删除对应快照。
- 启动提示：聚合列出快照；提供逐项/全部恢复与丢弃；所属项目不存在时恢复到当前活跃项目，无活跃项目时提示先创建项目并保留快照。
- 快照损坏：隔离后排除，提示一次，不阻止启动。

### 可注入时钟与故障注入

```rust
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;          // 单调时钟，驱动去抖
    fn wall_clock_ms(&self) -> i64;    // 快照时间、隔离文件名
}

pub trait PersistenceIo: Send + Sync + 'static {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn backup(&self, path: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn list(&self, dir: &Path) -> io::Result<Vec<DirEntry>>;
}
```

故障注入：

```rust
pub struct FaultPlan { rules: Vec<FaultRule> }
pub enum FaultAction { Fail(io::ErrorKind), TruncateTemp(usize), Delay(Duration) }
pub enum FaultPoint { Read, Backup, WriteTemp, Sync, Replace, Remove, List }
```

- 测试注入点：备份失败、替换失败、临时文件半写、读损坏、写超时、迟到完成。
- 生产实现包 `atomic_file`：临时文件、`sync_all`、ACL 复制、`MoveFileExW`。
- `ManualClock::advance` 驱动 2s/500ms/300ms/立即语义；测试禁止真实 `sleep`。
- 隔离戳使用注入时钟；同戳冲突追加序号，绝不覆盖上一代隔离件。

### 无迁移策略

- 新代码不读取旧 `session.json`、旧根 `settings.json`、旧 `.bak` 或旧隔离件。
- 旧文件不删除、不改写、不在启动时判定为损坏。
- 新数据路径与旧路径不重叠；`slterm_settings::settings_path()` 指向新路径，旧路径不再被任何生产代码引用。
- 目标态内部只接受当前 `schema_version`；无升级函数、无兼容字段映射、无“尽力解析”。
- 无法识别的记录按损坏/未知处理，不阻止启动（工作区索引失败除外，按启动错误界面处理）。

### 持久化测试接缝

- DTO 往返：字段级相等；可选字段缺省；版本不匹配拒收。
- 恢复对账：未知类型、空组、孤儿面板、重复 ID、重复去重键、无效 active/focused、终端簿孤儿、越界 cwd。
- 去抖：手动时钟验证各域在阈值前不写、阈值后写一次；连续变更只写最后快照。
- generation：写入在途时产生新快照，旧完成事件不把状态回退；迟到结果丢弃。
- 原子性：临时文件不残留；替换失败保留原文件；崩溃在替换前主路径仍是上一代。
- 备份：备份失败中止写入；`.bak` 是替换前完整旧文档。
- 损坏：隔离成功后才重建；隔离失败保留原文件并只读；多代隔离不互相覆盖。
- 跨文件：项目创建/移除、终端创建/关闭的中间态恢复幂等。
- 快照：配额淘汰、7 天保留、同路径多项目、项目移除清理、损坏隔离。
- 关闭链：3 秒超时、冲刷失败重试/取消、PTY/项目保存失败继续、快照清理。
- 路径隔离：所有测试使用临时数据根；生产 `%APPDATA%` 不被触碰。
