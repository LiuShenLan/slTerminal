# git 执行体

> 以 git CLI 子进程实现的单一 git 执行体；文件树、Commit、gutter、diff 与 Runtime 共用。

## 决策与红线

- **子进程统一**：旧栈的 git2 类型化封装不移植；本模块以 git CLI 子进程重实现操作面，消费面自始单一数据源。原因：子进程形态与上游同构、跨版本稳定，双轨会永久维护两套状态解析。边界：禁止在面板侧 spawn git 或维护第二份 porcelain 解析。
- **每命令 `--no-optional-locks`**：读操作不得触发 index 锁或后台 gc；flag 必须位于子命令之前。不可谈判。
- **argv 不经 shell**：Windows 一律挂 `CREATE_NO_WINDOW`；路径参数是纯 argv 元素。
- **HEAD 错误分类契约**：`head_file` 先 `verify_head`（unborn 判定）再 `cat-file -e HEAD:<relative>` 预检，`git show` 在预检通过后失败也走同一分类臂；`classify_git_failure` 按 stderr（大小写不敏感）分出权限（`GitError::AccessDenied`）与损坏（`GitError::Corrupt`），识别不出归 `GitError::NotInHead`。UI 只做占位文案，不解析具体错误。
- **只读短 TTL 缓存边界**：`status_cached`/`repository_root_cached` 仅服务只读消费面（gutter、文件树 VCS），TTL 500 毫秒；同 root 的连续/并发读取共享一次子进程。写路径（rollback、runtime revision 校验）必须走未缓存的 `status`/`file_status_uncached`，不得吃缓存。状态缓存键 = 调用方传入的 root 原样（进程级单槽）；仓库根缓存键 = 起始目录（容量上限 16，过期与超限清理，解析失败不缓存）。
- **未跟踪展开**：共享 `status` 带 `--untracked-files=all`，快照含单文件未跟踪条目；单文件状态由 `file_status_from_snapshot` 从快照派生，不为路径单独 spawn status。
- **操作面**：status/diff/hunks/rollback/unstage；porcelain v2 解析在本模块单源（`parse_status`），`revision` = `sha256(status stdout)` 64 hex；写操作由 runtime 执行器重跑 status 比对（防陈旧），rollback = `git checkout HEAD -- path`（索引与工作区同时回到 HEAD 字节），untracked 回滚删除工作区新文件，unstage = `git restore --staged`（unborn HEAD 回退 `git rm --cached`）。
- **旧 git2 语义资产**：LRU 仓库缓存、不扫 ignored、rename 双检、rollback 三方字节一致已转对照用例保留。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：porcelain v2 解析、HEAD 错误分类、只读缓存 TTL 与写路径不吃缓存、未跟踪展开、rollback/unstage 字节语义、旧 git2 对照用例。
- 测试隔离使用临时仓库；不得在真实用户仓库上执行写操作。

## 操作

- 新增 git 能力：只在本模块增加，经 `command` 构造口执行；同步补解析与错误分类用例。
- 写操作：由 runtime 执行器重跑 status 比对后再执行。
