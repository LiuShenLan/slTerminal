# 文件系统与 Git 服务

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

### 职责与边界

本域负责：路径权威与真实路径校验、允许根、文件系统读写与原子保存、系统回收站、文件监听、Git CLI 执行体、仓库根解析、status/HEAD 快照与缓存、外部修改检测。

本域不负责：项目身份与项目生命周期（归工作区域）、文件树/侧栏视图状态、Git 写工作流（Commit 仅有的文件级回滚/删除由本域提供原子步骤，工作流本身归终端）、Diff 行算法与渲染、编辑器脏状态与关闭守卫、文件打开路由。

消费者固定为：Explorer（含 Agent 全局文件树）、Commit、Diff、GitShow、Editor、Runtime API v2 的 `git.*` 方法。所有消费者只依赖本域公开的 port，不得自行 `std::fs`、自行 spawn `git` 或维护第二份 porcelain 解析。

### 决策与红线

| 规则 | 原因 | 边界 |
|---|---|---|
| 所有文件访问经 `PathAuthority` + `AccessPolicy`；面板侧禁止裸 `std::fs` | 允许根、真实路径与符号链接校验必须单点，否则每个面板都是一条穿越路径 | 测试夹具与 `test_support` 例外；审计的显式用户另存路径除外 |
| 读允许根 = 项目根 + 项目根所在仓库实际根 + 已登记 `AgentRoot`；写允许根 = 项目根 + 已登记 `AgentRoot` | 规格要求仓库根内、项目根外只读；Agent 根是显式可写作用域，编辑器可保存但不扩大项目写边界 | Commit 破坏性操作额外要求目标位于项目根内；写前经 `PathGuard` 复核；仓库根外、设备路径、盘符相对路径一律拒绝 |
| 文档身份用真实路径，文件树条目用链接项本身 | 编辑器要求符号链接/目录联接按真实目标建立身份；Explorer 要求删除/重命名只作用于链接 | 两种模式由 `LinkPolicy::{Follow, Entry}` 显式选择，调用方不得默认 |
| 全部 Git 操作经单一 `GitExecutor`，每条命令前置 `--no-optional-locks` | 读操作不得触发 index 锁或后台 gc；双执行体会产生两套状态解析 | flag 必须位于子命令之前；argv 不经 shell；Windows 统一隐藏控制台窗口 |
| 路径参数是纯 argv 元素，统一 `--literal-pathspecs` | 防止路径被解释为 glob/shell 文本 | 不拼接命令行字符串；不向 shell 传参 |
| 缓存只服务只读消费面，写路径与陈旧校验必须绕过缓存 | 写操作吃缓存会产生错误覆盖 | 状态/仓库根/HEAD 缓存 TTL 500ms；写操作完成即推进仓库代次并失效相关缓存 |
| 监听是租约，不轮询；视图不可见即释放或暂停 | 规格明确不做监听失效轮询兜底 | 监听不可用时降级为手动刷新提示；重新显示/重开视图重试 |
| 原子保存失败不降级为普通写 | 网络盘不支持原子替换时降级会产生半写文件 | 返回 `AtomicWriteUnsupported` 并保留原文件 |
| 删除优先系统回收站；不可用时才回退永久删除，且由调用方确认 | 规格要求可恢复优先，回退必须可知 | 符号链接只删链接；批量逐项执行、聚合结果 |
| 错误类型化，UI 只做文案映射 | 规格禁止统一归因；日志/错误不得含文件内容与凭据 | `FsError`/`GitError` 不携带文件内容；路径可携带 |

### 模块树

新建 app 内服务模块 `slterm_app/src/services/`（不引用 GPUI 类型；平台调用经 port 注入）。所有目录遍历、Git 子进程、文件读写与监听回调在 GPUI background executor 或服务自有线程执行，前台只接收结果与重绘信号。

```text
slterm_app/src/services/
  src/
    fs/
      mod.rs          // FileServices 门面
      path.rs         // NormalizedPath / PathRef / PathAuthority / LinkPolicy
      access.rs       // AllowedRoot / AccessPolicy / Access
      stamp.rs        // FileStamp / DiskVersion / FileId
      atomic.rs       // 原子替换与版本校验
      transfer.rs     // 树内复制/移动、冲突与跨卷回退
      trash.rs        // 回收站/永久删除
      tree.rs         // 单层目录读取、分批游标、排序
      watch.rs        // WatchService / WatchPolicy / WatchEvent
      ports.rs        // PlatformFs / PlatformWatch / Clock
      test_support.rs // FakePlatformFs / FakeWatch / 故障注入
    git/
      mod.rs          // GitServices 门面
      command.rs      // 唯一子进程构造口
      status.rs       // porcelain v2 / ignored / 分类
      head.rs         // HEAD 代次、HEAD 文件读取
      diff.rs         // HEAD 侧与工作区侧内容获取
      write.rs        // rollback / unstage / delete / 仓库写门
      cache.rs        // status / root / head 缓存
      error.rs        // GitError 与映射
      test_support.rs // FakeGitProcess / 脚本化仓库
    ports.rs          // ExplorerBackend / CommitBackend / DocumentBackend / HeadBackend
```

若总架构决定并入统一 core crate，以上模块树保持不变，仅把 crate 前缀替换为该 core 的 `services::fs`/`services::git`。

### 路径权威与真实路径校验

```rust
/// 项目根的双重表示：词法身份用于项目唯一性与历史归属；真实路径用于包含性校验。
pub struct ProjectPathAuthority {
    pub project_id: ProjectId,
    pub lexical: NormalizedPath,
    pub real: RealPath,
}

/// 显示形态保留原始大小写与分隔符；key 用于比较与哈希。
pub struct NormalizedPath {
    display: PathBuf,
    key: String,
}

pub struct RealPath(PathBuf);

pub struct PathRef {
    pub project_id: ProjectId,
    pub logical: PathBuf,   // 调用方看到的路径（链接项本身）
    pub real: PathBuf,      // 解析链接后的真实路径
    pub root: RootId,
}

pub enum LinkPolicy { Follow, Entry }

impl PathAuthority {
    pub fn resolve(root: &Path) -> Result<Self, PathError>;
    pub fn check(&self, path: &Path, policy: LinkPolicy, access: Access) -> Result<PathRef, FsError>;
    pub fn relativize(&self, path: &PathRef) -> Option<NormalizedPath>; // 项目根内相对路径
}
```

规则：

- `PathAuthority`/`AccessPolicy` 是 `design/security.md` 中 `PathGuard` 判定在本域的实现；本域不另立允许根规则。
- 归一化：正反斜杠等价、忽略大小写、折叠重复分隔符、去除末尾分隔符（盘符根与 UNC 共享根除外）、按路径段边界比较。`NormalizedPath.key` 是归一化后的比较键，`display` 保留磁盘原始形态。
- 真实路径解析：已存在路径 `canonicalize`；不存在的末段取最近存在祖先的真实路径后逐段拼接；拼接段拒绝 `.`/`..`/空段。解析结果统一去掉 `\\?\` 前缀用于比较，平台调用时再按需加回。
- 包含性校验按真实路径的路径段边界，Windows 不区分大小写；`C:/foo` 不包含 `C:/foobar`。
- 拒绝：设备路径（`\\.\`、`\\?\GLOBALROOT`）、盘符相对路径（`C:foo`）、非绝对路径、含 NUL、超过平台长路径能力的路径。
- UNC 只在允许根内允许；项目根本身为 UNC 时，其真实共享根可作为允许根。
- 根目录不可访问：`FsError::RootUnavailable`；项目仍可切换/重命名/排序，文件型操作显式失败。
- 项目重定位后重新构造 `ProjectPathAuthority`；旧 `PathRef` 不得跨重定位复用，避免词法根变化后仍按旧真实路径写入。

### 允许根

```rust
pub enum RootKind { Project, Repository, AgentGlobal }

pub struct AllowedRoot {
    pub kind: RootKind,
    pub lexical: NormalizedPath,
    pub real: RealPath,
    pub access: AccessMask, // READ | WRITE | CREATE | DELETE
}

pub struct AccessPolicy { roots: Vec<AllowedRoot> }

impl AccessPolicy {
    /// 每次操作按目标项目构造，不把全部项目根合并成全局可写域。
    pub fn for_project(
        authority: &ProjectPathAuthority,
        repo_root: Option<&RealPath>,
        agent_roots: &[RealPath],
    ) -> Self;

    pub fn explicit_user_choice(&self, path: &Path) -> Result<PathRef, FsError>;
    pub fn check(&self, path: &Path, policy: LinkPolicy, access: Access) -> Result<PathRef, FsError>;
}
```

- 读：项目根、仓库实际根、已登记 `AgentRoot`。仓库根内、项目根外为只读。
- 写：项目根、已登记 `AgentRoot`；编辑器在 `AgentRoot` 内可保存。Commit 回滚/删除再叠加「路径必须在项目根词法范围内」检查。
- 所有写入（含编辑器与 Diff 保存）在原子替换前重新解析真实路径并经 `PathGuard` 拒绝重解析点与符号链接逃逸；调用方不得跳过。
- `AgentRoot` 由 Agent 能力注册表的静态目录与设置中的自定义节点实时快照组成；目录本身的定义归 Agent 集成域，本域只消费真实路径集合。
- 显式用户选择（系统“另存为”）经 `explicit_user_choice` 生成一次性 `PathRef`，只允许编辑器保存目标；审计记录 operation/path/result，不记录文件内容。该通道不得用于文件树、Commit、GitShow 或自动打开。
- Markdown 文档中的显式本地链接经 `explicit_local_link` 生成只读单文件 `PathRef`：只允许读取该文件本身，不加载其根外资源；UNC 与设备路径仍按允许根规则拒绝。该通道只供 Markdown 面板消费，不得用于文件树、编辑器自动打开或写入。
- 所有允许根在操作开始时快照；项目切换或 Agent 节点删除不影响已开始的单次操作，但下一次操作必须重建策略。
- 终端创建的应用发起工作目录同样经 `AccessPolicy` 校验（项目根或已登记 `AgentRoot` 内）；Shell 内部 `cd` 不受限，但新终端继承目录时重新校验。

### 文件监听

```rust
pub struct WatchPolicy {
    pub scope: WatchScope,               // Tree(recursive) | Directory | File
    pub include_git_metadata: bool,      // HEAD / refs / index / packed-refs
    pub quiet: Duration,                 // 尾部去抖
    pub max_latency: Duration,           // 持续事件下的最大延迟
}

pub enum WatchEvent {
    Changed { paths: Vec<PathBuf> },
    Renamed { from: PathBuf, to: PathBuf },
    RescanRequired { paths: Vec<PathBuf> }, // 队列溢出/监听错误，消费者全量复核
}

pub struct WatchService { /* 单实例，按根引用计数 */ }

impl WatchService {
    pub fn lease(&self, owner: WatchOwner, root: &Path, policy: WatchPolicy,
                 sink: impl Fn(WatchEvent) + Send + 'static) -> Result<WatchLease, WatchError>;
    pub fn repository_roots(&self, repo: &RepositoryRoot) -> RepositoryWatchRoots;
}
```

监听配置（契约值来自各消费方规格）：

| 消费方 | 范围 | 去抖 | 最大延迟 | 备注 |
|---|---|---|---|---|
| Explorer / Agent 文件 | 项目根或 Agent 根递归 | 200ms | 1000ms | 过滤 `.git` 噪声；Agent 根只在其展开且视图可见时监听 |
| Commit | 仓库根递归 + Git 元数据 | 200ms | 1000ms | 只处理仓库根内事件；元数据只认 HEAD、当前分支引用、refs、索引、packed-refs |
| Editor | 项目根递归；根外/另存文件监听父目录 | 300ms | 1000ms | 外部修改目标时延 1s 内；溢出触发全部已打开文档复核 |
| Diff HEAD 侧 | Git 元数据 | 300ms | 1000ms | 面板激活且距上次刷新超过 10s 时补一次 |
| GitShow 过期检测 | Git 元数据 + 目标路径 HEAD 指纹 | 300ms | 1000ms | 只置过期状态，不自动替换内容 |
| Hooks 当前层文件 | 单文件父目录 | 300ms | 1000ms | 连续变化合并为一次外部修改提示 |

- 单根共享：同一根上多个租约共享一个 OS watcher；最后一个租约释放时 `unwatch`。树根递归覆盖的父目录不重复 watch。
- Git 元数据路径：普通仓库为 `<root>/.git`；链接工作树 `.git` 为文件时解析其 gitdir，并同时监听 common dir 的 refs/packed-refs。排除 `.git/objects/**` 与 `.git/logs/**`。
- 事件分类在 watcher 线程只做路径归属、去重与溢出标记，再经容量 1 的唤醒通道交给前台；不阻塞、不重扫。
- 监听失败：返回 `WatchError::Unavailable`；消费方显示“自动刷新不可用，可手动刷新”。重新显示视图、切换项目或重开视图时重建租约。GitShow 的过期检测在监听不可用时退化为窗口聚焦/面板激活时复核 HEAD 指纹，不引入轮询。
- 重命名：`notify` 的 Both 事件映射为 `Renamed { from, to }`；只有映射可用时编辑器才跟随重命名，否则按删除+新建处理。
- 写入自触发：本应用保存后先向监听器登记 `(path, 新 stamp)` 抑制一次自触发；监听事件仍以磁盘 stamp 为准，不能只凭来源标记丢弃。

### Git 执行体

```rust
pub struct GitExecutor { platform: Arc<dyn GitProcess>, clock: Arc<dyn Clock> }

impl GitExecutor {
    /// 唯一构造口；`--no-optional-locks` 固定位于子命令之前。
    pub fn command(&self, cwd: &Path) -> GitCommand;
    pub fn run(&self, cmd: GitCommand, timeout: Duration) -> Result<GitOutput, GitError>;
}

pub struct GitCommand { /* program=git, args: Vec<OsString>, cwd, env, hidden_window */ }
```

- 固定参数：`--no-optional-locks`、`-C <cwd>`；Windows 设置 `CREATE_NO_WINDOW`；stdout/stderr 有界读取。
- 超时：读命令默认 10s，写命令默认 30s；超时 `GitError::Timeout`，不无限等待。
- 环境：继承进程环境，但不注入凭据；不修改 Git 配置；不执行网络操作。
- `--literal-pathspecs` 用于 status/diff/checkout 等路径参数命令。
- 禁止 shell：所有路径是 argv 元素；错误消息可直接展示 stderr 摘要，但不得包含文件内容或凭据。
- 退出码与 stderr 分类：`classify_git_failure` 按 stderr（大小写不敏感）区分 `AccessDenied`、`Corrupt`；识别不出归具体命令错误。

### 仓库根解析

```rust
pub struct RepositoryRoot {
    pub worktree: PathBuf, // rev-parse --show-toplevel
    pub git_dir: PathBuf,  // rev-parse --git-dir（绝对化）
    pub common_dir: PathBuf,
}

pub fn repository_root(project_root: &Path) -> Result<RepositoryRoot, GitError>;
pub fn repository_root_cached(project_root: &Path) -> Result<RepositoryRoot, GitError>;
```

- 从项目根向上定位仓库；项目根可为仓库子目录。解析失败分类：目录不存在/不可访问 → `RepositoryUnavailable`；不在 worktree 内 → `NotRepository`；git 缺失 → `Unavailable`。
- 仓库根缓存键 = 起始目录，容量上限 16，TTL 500ms；解析失败不缓存。
- 链接工作树与子模块：只使用项目根所在 worktree 的实际根；嵌套仓库/子模块内部状态不单独解析。
- 仓库根是 Commit/Diff/GitShow/Explorer Git 状态的共同数据根；项目根只决定作用域与写入边界。
- 路径从 Git 输出进入 UI 前做归一化；规范化后越出仓库根的条目丢弃并计为不可见，不显示、不计数、不可操作。

### 状态快照与缓存

```rust
pub struct StatusSnapshot {
    pub digest: String,        // sha256(status stdout)，写操作防陈旧
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

pub struct StatusEntry {
    pub path: String,          // Git 原始路径，'/' 分隔，相对仓库根
    pub original: Option<String>,
    pub index: char,
    pub worktree: char,
    pub conflict: bool,
}

impl GitServices {
    pub fn status(&self, repo: &RepositoryRoot) -> Result<StatusSnapshot, GitError>;      // 权威
    pub fn status_cached(&self, repo: &RepositoryRoot) -> Result<Arc<StatusSnapshot>, GitError>;
    pub fn ignored_entries(&self, repo: &RepositoryRoot) -> Result<Vec<IgnoredEntry>, GitError>;
}
```

- `status` 使用 `git status --porcelain=v2 --branch -z --untracked-files=all --literal-pathspecs`；未跟踪目录展开为单文件条目，单文件状态从快照派生，不为路径单独 spawn。
- 已忽略分区使用独立命令 `git ls-files -z --others --ignored --exclude-standard --directory`：目录条目保留尾 `/`，不递归内部；只有 Commit 打开“显示已忽略”时才读取，默认不读、不计数。
- 状态分类（服务于 Commit/Explorer 展示）：`Added/Modified/Deleted/Renamed/Conflicted/Untracked/Unknown`；无法识别的已跟踪 XY 归 `Unknown` 并保留原始 XY。staged/unstaged 不分组展示，但保留两个字段供派生。
- 缓存：状态单槽按仓库根原样匹配，TTL 500ms；同根并发读取共享一次子进程，后到者等待。仓库根缓存独立，容量上限 16。
- 双代次：`WorktreeEpoch` 由仓库根内任一文件事件推进，`MetadataEpoch` 由 HEAD/refs/index 事件推进；任一推进或写操作完成都使状态缓存失效。只读消费方在代次变化后不得命中旧条目。
- Commit/Explorer 在自身去抖窗口后的首次读取使用权威 `status`；`status_cached` 只服务 gutter、树着色等容忍 500ms 陈旧的只读面。写路径（rollback/unstage/delete/runtime 写）一律使用权威 `status` 并携带 revision 校验；缓存不得参与写决策。
- 缓存失效与事件风暴合并：watcher 只推进代次，不逐事件 spawn；下一次读取统一重建。

HEAD：

```rust
pub struct HeadGeneration {
    pub oid: String,
    pub ref_name: Option<String>, // detached 时 None
}

pub struct HeadFile {
    pub bytes: Vec<u8>,
    pub total_size: Option<u64>,
    pub truncated: bool,
}

impl GitServices {
    pub fn head_generation(&self, repo: &RepositoryRoot) -> Result<HeadGeneration, GitError>;
    pub fn head_file(&self, repo: &RepositoryRoot, relative: &str, max_bytes: usize)
        -> Result<HeadFile, GitError>;
    pub fn head_fingerprint(&self, repo: &RepositoryRoot, relative: &str)
        -> Result<HeadFingerprint, GitError>;
}
```

- HEAD 代次解析 `rev-parse --verify HEAD` + 当前引用名；unborn 仓库返回 `GitError::NotInHead` 语义的分类，不伪造空 oid。
- HEAD 文件读取顺序：先判定 HEAD 是否存在（unborn），再 `cat-file -e HEAD:<relative>` 预检；预检通过后的读取失败也走同一分类。权限/损坏/不存在分别映射。
- `HeadFingerprint = { generation, blob_oid: Option<String>, size: u64 }` 服务 GitShow 过期检测：最近一次成功读取后指纹变化即置过期，不自动刷新正文。
- HEAD 缓存按 `(仓库根, relative, generation, max_bytes 档位)` 键，容量有界；代次变化自动失效；读取失败不缓存。
- 大文件截断只发生在服务层读取上限；`total_size` 来自 Git 对象大小，供 GitShow 分级；Diff 的 HEAD 侧读取按面板阈值决定上限。

**Commit 写步骤**

- 所有步骤先按 `expected_revision` 重跑权威 `status` 并重新校验目标状态与路径；不一致返回 `GitError::Stale`，不执行任何磁盘/索引写入。
- modified/deleted/conflict 回滚：`git checkout HEAD -- <path>`，索引与工作区同时回到 HEAD 字节。
- renamed 回滚：先恢复旧路径，成功后才删除新路径；旧路径恢复失败不得删除新路径；新路径删除失败按部分成功返回。
- added 删除：先 `unstage`，成功后才把工作区文件移入回收站；取消暂存失败时磁盘文件保持不变。
- untracked 删除：直接回收站/永久删除回退。
- 符号链接只操作链接项；所有写操作经 `RepoWriteGate` 按仓库串行，完成后推进两个代次并静默刷新。
- 不自动 `git init`、不修改 Git 配置、不执行网络命令。

### 外部修改检测

```rust
pub struct FileStamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub file_id: Option<FileId>, // 卷序列号 + 文件索引；不可得时为 None
}

pub enum DiskVersion {
    Unchanged,
    Modified(FileStamp),
    Removed,
    Replaced { old: FileStamp, new: FileStamp },
}

impl FileServices {
    pub fn stamp(&self, path: &PathRef) -> Result<FileStamp, FsError>;
    pub fn compare(&self, path: &PathRef, baseline: &FileStamp) -> Result<DiskVersion, FsError>;
    pub fn write_atomic_checked(&self, path: &PathRef, bytes: &[u8], expected: Option<&FileStamp>)
        -> Result<FileStamp, FsError>;
}
```

- 基线 = 打开/上次保存成功后的 `FileStamp`（大小、修改时间、文件标识）。文件标识不可得时退化为大小+时间；读取失败时编辑器不建立基线，聚焦/激活复核重试。
- 保存前比较；`write_atomic_checked` 在临时文件写完、替换前重新解析真实路径并再比较一次；重解析点/符号链接逃逸或不一致均中止并返回错误，临时文件清理，磁盘不产生半写。
- 保存成功后返回新 stamp，调用方更新基线；同时发出 `FileMutation`（见下），Explorer/Commit 据此先清除该文件状态，再在 300ms 合并窗口后重算。
- 外部变化来源：监听事件优先；监听不可用或事件溢出时由窗口聚焦/面板激活触发全量复核。编辑器目标时延 1s 内。
- 重命名映射：监听给出 `Renamed { from, to }` 时，文档身份更新并把基线迁移到新路径；新路径越出允许根时进入“已移出允许根”状态，禁用原地保存。
- 同一路径在多个项目中打开：各面板独立基线；一侧保存后另一侧按外部修改处理。

### 原子保存与回收站

```rust
pub struct FileMutation {
    pub path: PathBuf,
    pub kind: MutationKind, // Saved | Created | Deleted | Renamed
    pub origin: MutationOrigin, // App(AppFileSession) | External
}

pub enum RecycleOutcome { Recycled, PermanentlyDeleted }

impl FileServices {
    pub fn write_atomic(&self, path: &PathRef, bytes: &[u8], expect: Option<&FileStamp>)
        -> Result<FileStamp, FsError>;
    pub fn delete(&self, path: &PathRef, mode: DeleteMode) -> Result<DeleteOutcome, FsError>;
    pub fn delete_many(&self, paths: &[PathRef], mode: DeleteMode) -> Vec<DeleteResult>;
    pub fn create_file(&self, path: &PathRef) -> Result<FileStamp, FsError>;
    pub fn create_dir(&self, path: &PathRef) -> Result<(), FsError>;
    pub fn rename(&self, from: &PathRef, to_name: &str) -> Result<PathRef, FsError>;
    pub fn open_in_system(&self, path: &PathRef) -> Result<(), FsError>;
}
```

原子写步骤：

1. 校验父目录存在且位于允许根；父目录不存在时由调用方决定是否创建（新建文件流程创建；编辑器保存不隐式创建父目录）。
2. 同目录创建唯一临时文件，写入、flush、sync。
3. 复制目标文件 ACL 与只读属性（存在时）；保留扩展属性不在首版承诺。
4. 替换前重新经 `PathGuard` 解析目标真实路径，拒绝重解析点与符号链接逃逸；若有 `expect`，再比较 stamp，不一致则删除临时文件并返回 `ExternalModified`。
5. `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` 原子替换目标路径。
6. 返回新 stamp，并登记自触发抑制。

- 网络文件系统不支持原子替换：返回 `AtomicWriteUnsupported`，不回退普通写。
- 目标为符号链接且 `LinkPolicy::Follow`：身份与写入目标都是真实目标；`LinkPolicy::Entry` 的删除/重命名只作用于链接本身。
- 回收站：`DeleteMode::RecyclePreferred` 先走 `SHFileOperationW + FOF_ALLOWUNDO`；卷不支持或系统拒绝时返回 `RecycleUnavailable`，由调用方显示确认后以 `DeleteMode::Permanent` 重试。`DeleteMode::Permanent` 只能在此回退或明确的永久删除语义下使用。
- 批量删除/粘贴替换逐项执行，不要求原子性；结果聚合为 `Vec<DeleteResult>`，部分失败保留成功项。替换文件夹按“先移走旧目标，再写入”执行，失败保留可恢复状态。
- 符号链接、目录联接的删除只作用于链接项；递归删除不跟随链接目标。

### 树内复制与移动

```rust
pub enum TransferMode { Copy, Move }
pub enum ConflictPolicy { Replace, Skip, Cancel }

pub struct TransferRequest {
    pub sources: Vec<PathRef>,
    pub target_dir: PathRef,
    pub mode: TransferMode,
    pub conflict: ConflictPolicy,
    pub allow_symlink_copy: bool,
}

pub enum TransferResult {
    Completed { source: PathBuf, destination: PathBuf },
    Skipped { source: PathBuf, reason: TransferSkip },
    Failed { source: PathBuf, error: FsError },
}

pub enum TransferSkip { IntoSelf, AlreadyThere, SymlinkUnsupported }
```

- 源集合由视图按顶层项去重后传入；服务不接受任意外部路径。
- 目标为源自身或源的后代：该项失败并返回 `TransferSkip::IntoSelf`，其他项继续。
- 目标与源父目录相同：复制与移动都跳过该项并返回 `TransferSkip::AlreadyThere`。
- 复制递归处理文件夹内容；移动优先同卷原子改名，跨卷回退为复制后删除源；失败项保留源与剪切态。
- 冲突按不区分大小写比较；`Replace` 先按删除规则把目标移入回收站（不可用时要求调用方已确认永久删除），再写入；替换文件夹整体移走旧目标，不合并目录内容；`Cancel` 中止整个粘贴且零修改。
- 符号链接与目录联接按链接项本身处理；无法保持链接语义时跳过并报告，不递归复制目标内容。
- 批量逐项执行、不要求原子性；结果聚合，部分失败不回滚成功项。替换单个文件必须原子完成，失败时目标保持原样。
- 操作完成后由调用方刷新受影响目录与 Git 状态；服务返回实际处理数量与失败原因。

### 消费方 Port 与假实现

```rust
pub type WatchSink = Box<dyn Fn(WatchEvent) + Send + Sync>;

pub struct ExplorerGitState {
    pub status: Arc<StatusSnapshot>,
    pub ignored: Arc<IgnoredIndex>, // 按已加载路径批量查询；目录自身是否忽略
}

pub enum SaveError {
    Fs(FsError),
    ExternalModified(FileStamp),
    ReadOnly,
    AtomicWriteUnsupported,
}

pub struct RollbackRequest {
    pub project_id: ProjectId,
    pub repo: RepositoryRoot,
    pub path: String,
    pub original: Option<String>, // renamed 的旧路径
    pub expected_revision: String,
}

pub struct DeleteEntryRequest {
    pub project_id: ProjectId,
    pub repo: RepositoryRoot,
    pub path: String,
    pub state: CommitEntryState, // Added | Untracked
    pub expected_revision: String,
}

pub struct MutationReceipt {
    pub path: String,
    pub outcome: MutationOutcome, // Completed | PartiallyCompleted { done, failed }
}

pub enum CommitError { Git(GitError), Fs(FsError), Partial(MutationReceipt) }

// Explorer / Agent 文件树
pub trait ExplorerBackend: Send + Sync {
    fn root(&self, project: ProjectId) -> ProjectPathAuthority;
    fn list_dir(&self, dir: &PathRef, cursor: Option<DirCursor>) -> Result<DirPage, FsError>;
    fn git_state(&self, project: ProjectId) -> Result<ExplorerGitState, GitError>;
    fn classify_ignored(&self, project: ProjectId, paths: &[PathBuf])
        -> Result<Arc<IgnoredIndex>, GitError>;
    fn watch(&self, project: ProjectId, sink: WatchSink) -> Result<WatchLease, WatchError>;
    fn create(&self, parent: &PathRef, name: &str, kind: CreateKind) -> Result<PathRef, FsError>;
    fn rename(&self, path: &PathRef, new_name: &str) -> Result<PathRef, FsError>;
    fn delete(&self, paths: &[PathRef], confirmed_permanent: bool) -> Vec<DeleteResult>;
    fn transfer(&self, req: TransferRequest) -> Vec<TransferResult>;
    fn copy_path_text(&self, path: &PathRef, relative_to: &PathRef) -> String;
}

// Commit
pub trait CommitBackend: Send + Sync {
    fn repository_root(&self, project_root: &PathRef) -> Result<RepositoryRoot, GitError>;
    fn status(&self, repo: &RepositoryRoot) -> Result<Arc<StatusSnapshot>, GitError>;
    fn ignored(&self, repo: &RepositoryRoot) -> Result<Vec<IgnoredEntry>, GitError>;
    fn watch(&self, repo: &RepositoryRoot, sink: WatchSink) -> Result<WatchLease, WatchError>;
    fn rollback(&self, req: RollbackRequest) -> Result<MutationReceipt, CommitError>;
    fn delete_entry(&self, req: DeleteEntryRequest) -> Result<MutationReceipt, CommitError>;
}

// Editor / Diff 工作区侧
pub trait DocumentBackend: Send + Sync {
    fn open(&self, project: ProjectId, path: &Path, policy: LinkPolicy)
        -> Result<DocumentHandle, FsError>;
    fn read(&self, handle: &DocumentHandle, max_bytes: usize) -> Result<DocumentBytes, FsError>;
    fn recheck(&self, handle: &DocumentHandle) -> Result<DiskVersion, FsError>;
    fn save(&self, handle: &DocumentHandle, bytes: &[u8]) -> Result<FileStamp, SaveError>;
    fn watch(&self, handle: &DocumentHandle, sink: WatchSink) -> Result<WatchLease, WatchError>;
}

// Diff HEAD 侧 / GitShow
pub trait HeadBackend: Send + Sync {
    fn head_file(&self, repo: &RepositoryRoot, relative: &str, max_bytes: usize)
        -> Result<HeadFile, GitError>;
    fn head_fingerprint(&self, repo: &RepositoryRoot, relative: &str)
        -> Result<HeadFingerprint, GitError>;
    fn watch_metadata(&self, repo: &RepositoryRoot, sink: WatchSink) -> Result<WatchLease, WatchError>;
}
```

- Diff 不新增独立后端：工作区侧用 `DocumentBackend`，HEAD 侧用 `HeadBackend`；重命名条目用 `HeadBackend(head_path)` + `DocumentBackend(worktree_path)` 组合。
- GitShow 只用 `HeadBackend` + `HeadFingerprint`；不读取工作区。
- 所有 port 的类型不含 GPUI 类型，便于纯函数测试与虚拟窗口外测试。

假实现（`test_support`）：

- `FakePlatformFs`：内存文件树、可脚本化错误（权限/占用/磁盘满/原子替换不支持）、可控 stamp 与文件标识、自触发抑制记录、回收站结果。
- `FakeWatch`：手动推送 `Changed/Renamed/RescanRequired`、共享租约计数、可模拟不可用与丢事件。
- `FakeGitProcess`：按 argv 模式串返回脚本化 stdout/stderr/退出码；覆盖 porcelain v2 解析、HEAD 分类、超时、锁冲突。
- `FakeClock`：控制 TTL、去抖与最大延迟。
- `FakeExplorerBackend/FakeCommitBackend/FakeDocumentBackend/FakeHeadBackend`：面向上层视图的确定性替身，默认返回固定快照并记录调用。

### 错误映射

| 服务错误 | UI 类别 | 展示/恢复 |
|---|---|---|
| `RootUnavailable` | 数据根不可访问 | 路径 + 系统原因；重试 |
| `NotRepository` | 非 Git 仓库 | 提示可在终端运行 `git init` 后重试；不自动初始化 |
| `Unavailable` | 未找到 Git | 提示安装 Git 并加入 PATH；重试 |
| `AccessDenied` | 权限不足 | 路径 + 原因；重试 |
| `Corrupt` | 仓库数据损坏 | 索引/仓库错误摘要；重试 |
| `Command` | 其他读取失败 | 命令类别 + stderr 摘要；重试 |
| `Timeout` | 操作超时 | 路径/仓库 + 超时；重试 |
| `Stale` | 文件状态已变化 | 提示刷新后重试；静默刷新 |
| `ExternalModified` | 文件已被外部修改 | 编辑器进入保存冲突；不覆盖 |
| `AtomicWriteUnsupported` | 保存失败 | 原子替换不受支持；保留原文件 |
| `RecycleUnavailable` | 回收站不可用 | 二次确认后永久删除 |
| `PathNotAllowed` | 路径越界 | 显示允许根与限制；不提供绕过 |
| `NotFound` | 文件不存在 | 提示并刷新 |
| `Busy` | 文件被占用 | 提示稍后重试；不自动重试 |

### 复用、改造与新建

| 现有模块 | 处置 | 说明 |
|---|---|---|
| `slterm_app/src/git/mod.rs` | 改造并迁移 | 唯一子进程构造口、porcelain v2 解析、HEAD 分类、TTL 缓存、rollback/unstage 语义直接复用；移入 app 服务层并补 ignored、权威读、写门与类型化错误 |
| `slterm_app/src/git_worktree.rs` | 改造 | 事务创建/回滚逻辑保留；改为经 `GitExecutor` 与同一仓库写门执行，错误码收敛到 Runtime v2 |
| `slterm_app/src/atomic_file.rs` | 改造并迁移 | 原子替换、锁与备份原语复用；平台替换抽到 `PlatformFs`，增加替换前版本校验与 ACL 保留 |
| `slterm_app/src/platform/file_operations.rs` | 直接复用 | 回收站与系统打开的系统调用保留，包进 `trash.rs`/`open_in_system` |
| `slterm_app/src/platform/process.rs` | 直接复用 | `hidden_command` 供 Git 子进程 |
| `slterm_app/src/app_shell/fs_watch.rs` | 重写 | 现有树根+父目录监听只服务旧壳；改为租约、策略、重命名映射、元数据过滤与溢出复核 |
| `slterm_app/src/side_panel/enumerate.rs` | 部分复用 | 排序/截断概念与 `.git` 过滤保留；改为逐层按需读取，不再整棵平铺 |
| `slterm_app/src/side_panel/gitignore.rs` | 部分复用 | 仓库根文件系统探测保留；忽略清单改由 Commit 的 `ls-files` 提供，树视图不自行解析 |
| `slterm_app/src/side_panel/vcs.rs` | 改造 | 改为消费 `CommitBackend::status`；分支/统计/历史展示保留在视图层，Git 解析不再留在面板 |
| 编辑器/文件树现有直接 `std::fs` 调用 | 改造 | 全部改经 `DocumentBackend`/`ExplorerBackend`；禁止绕过允许根 |
| 新 `PathAuthority`/`AccessPolicy`/`PathRef` | 新建 | 路径权威与真实路径校验单点 |
| 新 `WatchService` | 新建 | 租约式监听与策略化去抖 |
| 新外部修改基线/保存冲突 | 新建 | `FileStamp`、`DiskVersion`、替换前二次校验 |
| 新仓库写门 `RepoWriteGate` | 新建 | Commit 回滚/删除、runtime `git.*` 写、worktree 事务按仓库串行 |
| 新消费方 port 与假实现 | 新建 | Explorer/Commit/Document/Head 四组接口 |

### 验收与测试接缝

- 路径：归一化/路径段边界/UNC/盘符根/长路径/大小写/末尾分隔符表驱动；真实路径穿越（junction 指向根外）拒绝；`C:/foo` 与 `C:/foobar` 不误判。
- 允许根：项目根写、仓库根只读、Agent 根读写、仓库根外拒绝；Commit 破坏性操作在项目根外禁用；显式另存通道只对编辑器生效。
- 监听：尾部去抖与最大延迟；重命名映射；溢出触发全量复核；租约共享与释放；不可用降级。
- Git：porcelain v2 解析（含 rename/conflict/unknown）、ignored 目录条目、HEAD unborn/不存在/权限/损坏分类、写前 revision 校验、`--no-optional-locks` 出现在每条命令、并发读共享一次子进程、写后缓存失效。
- 外部修改：修改/删除/替换/重命名；保存前与替换前两次检查；自触发抑制只忽略一次；干净文档自动重载由编辑器用例覆盖，服务只断言版本结果。
- 原子保存/回收站：故障注入（临时文件创建失败、替换失败、备份失败、回收站不可用回退）；真实回收站与网络盘登记人工验收豁免。
- 假实现：四组 port 各带确定性夹具；视图测试不触真实磁盘与真实 Git（Git 解析用临时仓库交叉验证）。
