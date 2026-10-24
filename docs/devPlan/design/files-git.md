# 文件操作、Git 与差异

> 规定 Explorer／Commit 共用的文件树、PathPolicy 执行计划、GitFacade、固定版本读取和 Diff 核心。

## 决策与红线

### 文件树与 watcher

- `slterm_sidebar` 提供根作用域的 FileTreeModel：分批目录读取、稳定排序、展开、选择、过滤和可见行投影。Explorer 与 Agent 文件树复用该模型和 FileOperationService，不各自实现路径操作。
- Explorer 树根固定为活跃 ProjectRoot；Agent 文件树固定为 AgentRoot。项目或根代次变化立即取消旧读取并丢弃旧结果，不能短暂混入新根内容。
- `.git` 文件或目录始终隐藏，其他点号条目正常显示；Git 忽略状态只作展示，不把 RepoRoot 读能力扩大为 ProjectRoot 写能力。
- WatcherService 按根和可见性拥有 watcher；事件按 200 毫秒合并，连续事件最长 1,000 毫秒内至少复核一次。不可见时暂停纯界面监听，重新可见合并复核一次，不以高频轮询伪装 watcher。
- 读取、排序、过滤和文件操作后台执行；目录按需分批，未展开目录不递归预读。根不可访问、子目录失败和部分批次失败分别显示，不伪装空目录。

### FileOperationService

每个写操作先生成不可变 `OperationPlan`：

```text
OperationPlan {
    kind, root_id, authorization_generation,
    lexical_path, canonical_path, file_identity,
    expected_fingerprint, destination?, recycle_policy,
    document_revision?, operation_id,
}
```

- 执行前重新解析父目录、最终目标、链接／目录联接、真实文件身份、授权代次和预期指纹；任一变化即中止或要求重新确认。不得沿用树加载时的旧授权。
- 新建文件为 0 字节，新建目录为空目录；名称校验、同目录大小写冲突和 Windows 保留名在执行前再次检查。重命名只改位置，不重建 Document 或 Panel。
- 删除优先 Windows 回收站；卷不支持时明确披露永久删除并再次确认。符号链接和目录联接只处理链接项本身，不传递到未授权目标。
- 批量操作逐项执行、可部分成功且不回滚；成功项移除，失败项保留并汇总。目录删除确认必须明确内容范围，目标已外部删除可按该项完成处理。
- 内部文件剪贴板是应用级会话状态，与系统文本剪贴板分离；复制允许跨根但每次粘贴分别校验读／写能力，剪切只允许同一根内。禁止目录粘贴到自身或后代，多操作串行并重新校验。
- 名称冲突一次聚合提供替换、跳过、取消，默认跳过；替换先把既有目标移入回收站，目录不合并。文件替换采用原子写或等效可恢复步骤，不能将部分成功报成全部成功。
- 普通单文件写入不增加全局日志；多步骤且崩溃后需判定成败的粘贴、替换、删除和回滚使用 ManagedOperationJournal。

### GitFacade 与状态事实

- 保留单一 GitFacade 和系统 `git.exe`，不引入 git2／gix。所有调用使用直接 argv、`--literal-pathspecs`、`GIT_TERMINAL_PROMPT=0` 和有界 stdout／stderr；不经过 Shell 字符串拼接，不自动 `git init`、修改 Git 配置或访问网络。
- 每仓库写操作串行，读取按仓库与代次合并；进程启动、超时、退出码和输出截断都形成结构化结果。RepositoryBinding 复核失败只报绑定不可用，不改绑。
- 状态使用 porcelain v2 NUL 分隔机器输出，保留 staged／unstaged、rename、conflict、ignored 和 unknown 原始事实。状态 revision 由规范状态字节散列形成，不能只依赖 watcher 时间。
- Commit 数据根是实际 RepoRoot，项目根可为其子目录；Explorer 仍只展示项目根内条目。读取失败保留旧列表并标记可能过期，不把失败当空列表。
- 未跟踪目录展开为文件，嵌套仓库和子模块只显示顶层条目；重命名只显示新路径一行并保留旧路径事实。未知状态使用原始状态码，不伪造为修改或删除。
- `CommitModel` 只保留当前 RepoRoot 代次的一份列表，分组为 Changes、Unversioned Files 和可选 ignored；不支持多选、批量回滚／删除、暂存、提交、推拉、分支、合并、stash、历史浏览或网络 Git。过滤只匹配文件名与完整相对路径，折叠、ignored 开关和滚动按 ViewKey 保存。
- 打开映射固定为：added／untracked／存在工作区文件的 unknown → 普通 Document；deleted → 固定版本 GitShow；modified／renamed／conflict → 共享 Document Diff；不存在的 unknown 与 ignored 不打开。所有落点使用活跃项目聚焦组，不跨项目聚焦。

### 回滚与删除

通用回滚只执行：

```text
git restore --worktree -- <path>
```

- 默认从当前索引恢复工作区，不传 HEAD、不改索引。modified／deleted 只在有未暂存变化时可用；纯 staged、staged rename、conflict、ignored 和未知形态禁用，无法区分时默认禁用。
- 执行前必须完成打开脏 Document 的保存／丢弃／取消守卫，再重读状态、真实路径、Document revision 和授权；状态改变即中止并刷新。
- 未暂存 rename 只有在索引仍保存原路径时提供：从索引恢复旧路径并清除新路径工作区条目，不改索引；新路径清除失败按部分成功报告。
- added 不走回滚。staged added 删除在用户明确确认后先执行 `git reset -q -- <path>`，成功后才进入回收站流程；取消暂存失败文件不动，删除失败显示部分成功。
- 所有破坏操作串行、可审计，结束后无论成功失败都刷新真实现状；不通过修改索引掩盖残留。

### Diff 核心

- `DiffBinding { repository_binding, head_path }` 固定创建时仓库与 HEAD 旧路径。DiffInput 是不可变快照，包含 HEAD 内容、工作区内容、空白模式、TextStateId、HEAD 对象身份和计算代次。
- 工作区侧直接引用共享 Document；大于 10 MiB 的脏工作区复制为独立只读快照，源 Document 继续可编辑，不强制保存或转只读。冲突对比使用磁盘与当前缓冲快照，不读取 HEAD。
- 使用 `similar` Myers 行差分，设置 2 秒硬预算。该库 deadline 到期可能返回近似结果，因此返回时发现已到期必须丢弃，不展示近似差异；过期任务由 revision 丢弃，worker 最迟在硬预算退出。
- 差异计算只接收有界快照；任一侧超过 50 MiB 或无法在安全预算内取得完整输入时返回差异不可用，正文仍可分页阅读，不建立无界内存副本。
- 空白模式只影响行键：行尾模式忽略行尾空格／Tab，全部模式忽略所有空格／Tab；CRLF／LF 和文件末尾换行不进入行键，原始文本保留。
- 差异操作先形成 hunk，再配对为 `Equal／Delete／Insert／Replace` 行对；空白占位和行内虚拟删除行只由视图生成，不写入 Document。
- 每个视图只缓存最新输入与结果，键包含 HEAD 对象身份、TextStateId、空白模式和格式参数。保存、重载、HEAD 刷新和可见性变化按代次重算。
- Diff 任一侧大于 10 MiB 时该侧分页只读；前 8 KiB 含 NUL 按二进制处理，不显示文本差异。Editor Git 行标复用同一差分核心。

### GitShow 固定版本

- GitShow 创建时捕获 RepositoryBinding、CommitId 和 RepoPath；内容只来自该提交的 blob，不读取工作区、索引或后续 HEAD。手动刷新只重读同一对象。
- 先经 `ls-tree` 等有界查询解析固定路径的 blob OID，再以 `cat-file` 流式读取并限制字节数；不使用会跟随工作区或过滤器的 show 输出。
- 固定版本不超过 1,000,000 字节常规只读，1,000,001–10,000,000 字节提示，大于 10,000,000 字节分页简化只读；前 8 KiB 含 NUL 按二进制处理。
- 二进制系统打开只在当前用户可写的系统临时位置生成带原扩展名的只读中转文件，关闭与退出清理，失败项 24 小时后按过期临时材料清理。
- 读取请求携带 ViewKey、对象身份和代次；对象不可用保留面板与错误，不回退工作区或新 HEAD。

## 验收与测试

- 使用真实临时 Git 仓库覆盖 staged＋unstaged、纯 staged、deleted、unstaged／staged rename、conflict、unknown、ignored、嵌套仓库和子模块。
- 回滚测试断言索引 blob 前后不变、工作区恢复当前索引而非 HEAD；staged added 删除分别注入取消暂存和回收站失败。
- 路径测试覆盖符号链接、目录联接、硬链接、大小写、跨根复制／剪切、替换、ACL 和执行前状态变化；批量部分成功必须如实汇总。
- Diff 属性测试覆盖行对、hunk、空白模式、CRLF、末尾换行和随机编辑；deadline 测试证明近似结果被丢弃而非展示。
- GitShow 测试在 HEAD 前进、根重定位、对象删除和大文件／二进制情况下验证固定版本与错误语义。
- GPUI 测试验证 Editor、Diff、Commit 共用 Document，行标定位差异，快照不自动刷新且不可保存。

## 操作

- 新增 Git 命令必须经 GitFacade argv 白名单、路径重解析、输出上限和仓库串行规则；不得在视图层调用 `git.exe`。
- 新增文件操作先定义 OperationPlan、回收站策略、部分成功结果和 Journal 恢复器，再接入菜单。
- 修改 Diff 行键或空白规则必须同步行对、缓存键、Editor 行标和属性测试。
