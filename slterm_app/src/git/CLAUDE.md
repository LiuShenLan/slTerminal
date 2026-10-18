# git 执行体

## 决策与红线

- **D07-1 已裁:git 子进程统一**。旧栈 `src-tauri/src/git/` 的 git2 封装不移植;本模块以
  git CLI 子进程重实现操作面(上游子进程形态),消费面自始单一数据源。
- **每命令 `--no-optional-locks`**(边界条 7,不可谈判):读操作不得触发 index 锁或
  后台 gc;flag 必须位于子命令之前。
- **argv 不经 shell**,Windows 一律挂 `CREATE_NO_WINDOW`;路径参数是纯 argv 元素。
- **HEAD 错误分类契约**:`head_file` 先 `verify_head`(unborn 判定)再
  `cat-file -e HEAD:<relative>` 预检,`git show` 在预检通过后失败也走同一分类臂;
  `classify_git_failure` 按 stderr(大小写不敏感)分出权限
  (`GitError::AccessDenied`)与损坏(`GitError::Corrupt`),识别不出归
  `GitError::NotInHead`(消息含「HEAD 中不存在」)。UI 只做占位文案,不解析具体错误。
- **只读短 TTL 缓存边界**:`status_cached`/`repository_root_cached` 仅服务只读消费面
  (gutter、文件树 vcs),TTL 500ms;同 root 的连续/并发读取共享一次子进程。写路径
  (rollback、runtime revision 校验)必须走未缓存的 `status`/`file_status_uncached`,
  不得吃缓存。状态缓存键 = 调用方传入的 root 原样(进程级单槽);仓库根缓存键 =
  起始目录(容量上限 16,过期与超限清理,解析失败不缓存)。
- **未跟踪展开**:共享 `status` 带 `--untracked-files=all`,快照含单文件未跟踪条目;
  单文件状态由 `file_status_from_snapshot` 从快照派生,不为路径单独 spawn status。

## 步骤接缝

- M8.1 只落只读面:仓库根解析 + HEAD 文件字节(gitshow 数据源)。
- M8.4 起 `command` 对 `side_panel` 开放:文件树过滤索引与 git 状态快照共用
  同一子进程构造口(无第二执行体)。
- M8.5 已接 status/diff/hunks/rollback/unstage 四方法:porcelain v2 解析在本模块
  单源(`parse_status`),`revision` = `sha256(status stdout)` 64 hex;写操作由
  runtime 执行器重跑 status 比对(防陈旧),rollback = `git checkout HEAD -- path`
  (索引与工作区同时回到 HEAD 字节),untracked 回滚删除工作区新文件,unstage =
  `git restore --staged`(unborn HEAD 回退 `git rm --cached`)。旧 git2 语义资产
  (LRU 仓库缓存 BE-09、不扫 ignored CP-008、rename 双检、rollback 三方字节一致)
  已转对照用例。
