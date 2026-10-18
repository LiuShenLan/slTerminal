# git 执行体

## 决策与红线

- **D07-1 已裁:git 子进程统一**。旧栈 `src-tauri/src/git/` 的 git2 封装不移植;本模块以
  git CLI 子进程重实现操作面(上游子进程形态),消费面自始单一数据源。
- **每命令 `--no-optional-locks`**(07 篇边界条 7,不可谈判):读操作不得触发 index 锁或
  后台 gc;flag 必须位于子命令之前。
- **argv 不经 shell**,Windows 一律挂 `CREATE_NO_WINDOW`;路径参数是纯 argv 元素。
- **HEAD 不存在错误契约**:`UnbornBranch`(尚无提交)与「文件不在 HEAD tree」统一归
  `GitError::NotInHead`,消息含「HEAD 中不存在」;UI 只做占位文案,不解析具体错误。

## 步骤接缝

- M8.1 只落只读面:仓库根解析 + HEAD 文件字节(gitshow 数据源)。
- M8.4 起 `command` 对 `side_panel` 开放:文件树过滤索引与 git 状态快照共用
  同一子进程构造口(无第二执行体)。
- M8.5 接 status/diff/hunks/rollback/unstage 四方法与 04 篇
  `verify_repository`/`verify_revision` 核对;旧 git2 语义资产(LRU 仓库缓存 BE-09、
  不扫 ignored CP-008、rename 双检、rollback 三方字节一致)在该步转对照用例。
