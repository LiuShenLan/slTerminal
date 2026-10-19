# 侧栏文件树模型

> 文件树、搜索与 VCS 状态的数据与状态转换；渲染只经行数据消费。

## 决策与红线

- **模型/渲染分层**：本目录只持数据与状态转换；渲染只经 `file_rows()` 按行号取。禁止在模型里引入 gpui 类型。
- **单子进程执行体**：gitignore 过滤索引与 git 状态快照都走 `crate::git::command`；每命令 `--no-optional-locks` 不可谈判。目录枚举不 spawn git，仓库门控用 `gitignore::git_repository_root` 的文件系统探测。
- **状态解析单源**：`vcs.rs` 消费 `crate::git::status`/`parse_status`，不在面板侧维护第二份 porcelain 解析。
- **搜索选项**：匹配大小写/全字/正则选项由 `FileSearchOptions` 承载，改选项经 `set_file_search_options` 重排队；渲染层只读 `search_options()`。
- **WSL/SVN 整面不迁**：`guest_path`/`followed_wsl`/SVN 状态解析/远程过滤索引均已裁掉；不要以「兼容」名义回填。
- **目录内容不做定时重扫**：触发点只有 cwd/根变化、显式刷新、worker 完成补跑，以及 fs-notify 外部事件（树根递归监听 → 容量 1 有界通道 → 前台 30 毫秒合并刷新，脏路径去重）。原因：定时重扫会持续消耗磁盘与 CPU。边界：搜索目录 watch 只服务热缓存复用。
- **ignored 语义**：ignored 条目留在树序、只置降色位；git 状态快照不扫 ignored。原因：用户仍需要看到被忽略文件的位置。
- **文件名搜索内存纪律**：缓存/结果两套进程级租约（`search/memory.rs`），替换 query 不给退役任务发新预算；保留租约随结果一起交给视图。
- **快照代次**：`refresh` 推进代次，worker 产物代次不匹配即丢弃并重臂；`root` 比对是第二道防线。
- **图标载体**：内嵌 Nerd Font 字形（`icons.rs`，字形来自 `REQUIRED_FONT_FAMILY` 的 cmap）；颜色一律继承渲染行的主题 token，模型层不出现颜色。svg/位图不进本模块。
- **回收站**：Windows `SHFileOperationW` + `FOF_ALLOWUNDO`，单点在 `platform/file_operations.rs`；真实落桶不可自动化，豁免见 `.claude/docs/testing/exemptions.md`。
- **面板视图映射**：侧栏视图归属以 `SideViewId` 封闭集与单一注册点为准；未注册 id 恢复时静默丢弃。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：树模型行数据、VCS 状态单源、搜索选项重排、ignored 降色、fs-notify 合并刷新、代次失配丢弃、回收站单点。
- 外部文件变化刷新与像素级观感由 UI 用例与人工验收兜底。

## 操作

- 新增树交互：在模型层提供状态转换，渲染层只消费行数据。
- 新增文件操作：经 `platform::file_operations` 单点。
- 新增搜索能力：先扩 `FileSearchOptions` 与重排语义，再改渲染。
