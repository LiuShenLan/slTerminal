# git 执行体

> 以 git CLI 子进程实现的单一只读 git 执行体；文件树、gutter 与补全探测共用。

## 决策与红线

- **子进程统一**：旧栈的 git2 类型化封装不移植；本模块以 git CLI 子进程重实现操作面，消费面自始单一数据源。原因：子进程形态与上游同构、跨版本稳定，双轨会永久维护两套状态解析。边界：禁止在面板侧 spawn git 或维护第二份 porcelain 解析。
- **每命令 `--no-optional-locks`**：读操作不得触发 index 锁或后台 gc；flag 必须位于子命令之前。不可谈判。
- **argv 不经 shell**：Windows 一律挂 `CREATE_NO_WINDOW`；路径参数是纯 argv 元素。
- **HEAD 错误分类契约**：`head_file` 先 `verify_head`（unborn 判定）再 `cat-file -e HEAD:<relative>` 预检，`git show` 在预检通过后失败也走同一分类臂；`classify_git_failure` 按 stderr（大小写不敏感）分出权限（`GitError::AccessDenied`）与损坏（`GitError::Corrupt`），识别不出归 `GitError::NotInHead`。UI 只做占位文案，不解析具体错误。
- **只读短 TTL 缓存边界**：`status_cached`/`repository_root_cached` 仅服务只读消费面（gutter、文件树 VCS），TTL 500 毫秒；同 root 的连续/并发读取共享一次子进程。状态缓存键 = 调用方传入的 root 原样（进程级单槽）；仓库根缓存键 = 起始目录（容量上限 16，过期与超限清理，解析失败不缓存）。
- **GitShow 固定对象读取**：`GitFacade::show` 先复核原 RepositoryBinding，再以 `ls-tree -z <固定提交>` 精确解析路径对应的 blob OID，最后只经 `cat-file blob <OID>` 读取；请求携带独立 generation。原因：HEAD 前进、工作区编辑或手动刷新不得改变固定版本，对象缺失不得回退工作区。边界：路径、提交和对象输出严格校验；面板按完整 ViewKey 复用并在后台读取，重复打开只聚焦，固定身份与布局可恢复。
- **GitShow 内容路由与系统中转**：不超过 1,000,000 字节常规只读，1,000,001–10,000,000 字节提示，大于 10,000,000 字节建立完整内存页索引并逐页严格 UTF-8 解码，前 8192 字节含 NUL 按二进制处理，非法 UTF-8 不生成伪文本。系统打开只把固定 blob 写入当前用户系统临时根下带原扩展名的只读文件，不写工作区；打开失败立即清理，句柄 Drop 覆盖关闭与正常退出，单实例启动维护只清理已超过 24 小时且属于本前缀的遗留文件。原因：固定对象不得借系统打开污染仓库，也不得把有限前缀冒充完整分页正文。边界：系统调用仍在后台执行；过期清理失败不阻断已通过持久维护的启动，存活句柄不由过期清理删除。
- **状态事实与代次缓存**：`GitFacade::status` 先分别复核项目路径与真实 RepoRoot 的 Read 授权，再使用有界、禁网的 porcelain v2 NUL 输出；`StatusSnapshot` 保存规范字节 SHA-256 revision、真实 worktree 根、staged／unstaged、rename、conflict、ignored 与未知原始状态。`--untracked-files=all` 展开普通未跟踪目录，嵌套仓库和子模块只保留顶层条目；ignored 不进入既有六值 gutter 映射。实例状态缓存 TTL 500 毫秒，`invalidate` 清槽并递增代次，在途旧代次不得写回。原因：状态散列必须来自完整机器输出，路径根、授权和缓存代次不得由 watcher 时间代替。边界：unborn 仓库可读取状态但不能建立需要 HEAD 的 RepositoryBinding；读取失败由调用方保留旧列表并标记可能过期。
- **未跟踪展开**：共享 `status` 带 `--untracked-files=all`，快照含单文件未跟踪条目；单文件状态由 `file_status_from_snapshot` 从快照派生，不为路径单独 spawn status。
- **只读操作面**：status/file status/hunks/HEAD 字节与补全 refs/remotes；porcelain v2 解析在本模块单源（`parse_status`）。补全读取只经 `completion::GitFacade` 的语义方法，具体预算与禁网红线见 `completion/CLAUDE.md`。本模块不得执行 checkout、restore、rm 等工作区或索引写操作。
- **仓库绑定捕获与复核**：`GitFacade::repository_binding` 使用有界、禁网的 `rev-parse` 同时捕获 worktree、gitdir、common dir 的真实路径／身份与完整 HEAD，并分别取得 Read 授权；项目根可以是仓库子目录，但不会因此获得父仓库元数据访问权。`revalidate_binding` 重新捕获当前事实并查询原 HEAD 对象，绑定路径或身份不符只报 `BindingUnavailable`，不改绑原值；Diff 可见性刷新复用同一当前事实捕获与原 HEAD 存在性查询，仅经 `DiffBinding::refresh_head` 推进提交并保留旧路径。原因：Diff／GitShow 不得因 HEAD 移动或根重定位静默落到另一仓库。边界：unborn 仓库不能建立需要 HEAD 的绑定；进程启动、超时、取消、退出与输出限额保持结构化分类。
- **旧 git2 语义资产**：LRU 仓库缓存、不扫 ignored、rename 双检已转对照用例保留。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：porcelain v2 解析、HEAD 错误分类、只读缓存 TTL、未跟踪展开、status/file status/hunks 与旧 git2 对照用例。
- 测试隔离使用临时仓库；不得在真实用户仓库上执行写操作。

## 操作

- 新增 git 能力：只在本模块增加，经 `command` 构造口执行；同步补解析与错误分类用例。
