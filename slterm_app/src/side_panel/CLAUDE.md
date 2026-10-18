# 侧栏文件树模型

## 决策与红线

- **模型/渲染分层**(07 篇照抄点 21):本目录只持数据与状态转换;渲染只经
  `file_rows()` 按行号取。禁止在模型里引入 gpui 类型。
- **单子进程执行体**(D07-1):gitignore 过滤索引与 git 状态快照都走
  `crate::git::command`;每命令 `--no-optional-locks` 不可谈判。目录枚举不
  spawn git,仓库门控用 `gitignore::git_repository_root` 的文件系统探测。
- **状态解析单源**:`vcs.rs` 消费 `crate::git::status`/`parse_status`,不在面板侧
  维护第二份 porcelain 解析。
- **搜索选项**:匹配大小写/全字/正则三钮由 `FileSearchOptions` 承载,改选项经
  `set_file_search_options` 重排队;渲染层只读 `search_options()`。
- **WSL/SVN 整面不迁**:`guest_path`/`followed_wsl`/SVN 状态解析/远程过滤
  索引均已裁掉;不要以「兼容」名义回填。
- **目录内容不做定时重扫**(上游 2026-08-21 裁定):触发点只有 cwd/根变化、
  显式刷新、worker 完成补跑,以及 fs-notify 外部事件(树根递归监听 →
  容量 1 有界通道 → 前台 30ms 合并刷新,脏路径去重)。仍禁定时目录重扫;
  搜索目录 watch 只服务热缓存复用。
- **ignored 语义调和**(边界条 8):ignored 条目留在树序、只置降色位;
  git 状态快照不扫 ignored(CP-008 因果)。
- **文件名搜索内存纪律**:缓存/结果两套进程级租约(`search/memory.rs`),
  替换 query 不给退役任务发新预算;保留租约随结果一起交给视图。
- **快照 generation**:`refresh` 推进 generation,worker 产物 generation 不匹配
  即丢弃并重臂;`root` 比对是第二道防线。

## 文档外契约

- 图标载体选型(07 篇开放问题 4 本步关闭):内嵌 Nerd Font 字形
  (`icons.rs`,字形来自 `REQUIRED_FONT_FAMILY` 的 cmap);颜色一律继承渲染行
  的主题 token,模型层不出现颜色。svg/位图不进本模块。
- 回收站:Windows `SHFileOperationW` + `FOF_ALLOWUNDO`,单点在
  `platform/file_operations.rs`;真实落桶不可自动化,豁免登记见
  `.claude/test-exemptions.md`。
- 面板视图映射:M5.3b 双槽的 `SideViewId::Explorer` 挂本模型;`Commit` 槽
  仍为占位(vcs 展示面归 M8.5)。
