# Commit 视图

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> Commit 视图的四渲染态、变更分组与计数、ignored 分区、打开映射、右键菜单、回滚/删除、刷新与项目切换。

### 模型与模块

```text
slterm_sidebar/commit.rs
  CommitViewState / CommitReady / ChangeEntry / CommitGroup / CommitErrorKind
  分组、过滤、排序、计数纯函数

sidebar_host/views/commit/
  mod.rs        CommitView（SidebarView）
  state.rs      四态、折叠、ignored 开关、过滤、聚焦行、滚动
  data.rs       status -> CommitData 投影
  render.rs     分组标题、行、空态、错误态、行内警告
  menu.rs       右键菜单与定位/复制
  ops.rs        回滚/删除事务与串行队列
  watcher.rs    仓库监听、200ms/1000ms 去抖、可见性门控
  persist.rs    视图状态声明
```

```rust
pub enum CommitViewState {
    NoRoot,
    Loading { epoch: u64 },
    Error { kind: CommitErrorKind, detail: String },
    Ready(CommitReady),
}

pub struct CommitReady {
    pub root: PathBuf,
    pub branch: String,
    pub changes: Vec<ChangeEntry>,
    pub unversioned: Vec<ChangeEntry>,
    pub ignored: Option<Vec<ChangeEntry>>,
    pub collapsed: [bool; 3],
    pub filter: String,
    pub focus: Option<EntryKey>,
    pub scroll: ScrollAnchor,
    pub warning: Option<CommitWarning>,
}

pub enum ChangeEntry {
    Added { path, old_path: None },
    Modified { path },
    Deleted { path },
    Renamed { path, old_path },
    Conflict { path },
    Unknown { path, raw_code: char },
    Untracked { path },
    Ignored { path, is_dir: bool },
}
```

### 数据根与四态

- 数据根 = 活跃项目根所在 Git 仓库实际根；项目根可为仓库子目录；列表覆盖整个仓库工作区，相对路径相对数据根。
- 无根：无活跃项目；居中 `选择一个项目以查看变更`；头部控件禁用。
- 加载中：首次打开、数据根变化、错误重试；清空旧列表与聚焦行、滚动置顶；头部控件禁用。
- 错误：读取失败，按类别显示主文案、原因与 `重试`；不显示列表；错误类别：数据根不可访问、非 Git 仓库、未找到 Git、权限不足、仓库数据损坏、其他读取失败；不得统一归因。
- 就绪：Changes、Unversioned Files 两个分组恒显示；ignored 开关开启时追加已忽略分区；计数为当前过滤后的条目数。
- 数据根变化：立即清空列表、关闭菜单与确认框、清空过滤与聚焦行、滚动置顶、进入加载中；分组折叠、已忽略分区折叠与 ignored 开关保留。
- 自动/手动刷新与操作后刷新停留在就绪态：现有列表保留到新结果返回再原位更新；刷新失败不改错误态，显示行内警告 `刷新失败，列表可能已过期` + 原因 + `重试`；成功一次后消失。
- 错误态收到文件事件可静默重试，失败保持错误态并更新原因，不闪回加载中。

### 分组、计数与排序

- 分组顺序固定：Changes、Unversioned Files、已忽略（仅开关开启）。
- Changes 包含已跟踪的 added、modified、deleted、renamed、conflict；无法识别的已跟踪状态归 Changes，显示 `未知` 与原始状态码提示。
- Unversioned Files 仅未跟踪文件；普通未跟踪目录展开为文件行；嵌套 Git 仓库与子模块按单条目显示，不递归内部。
- renamed 只产生一行显示新路径，提示 `旧路径 → 新路径`。
- ignored 默认不显示、不计数；开启后显示 `已忽略（N）` 独立分区，按仓库返回条目展示，目录条目显示目录行且不递归展开，不计入任何变更总数。
- 标题 `Changes (N)`、`Unversioned Files (N)`、`已忽略（N）`；折叠不改变计数；英文标题不随单复数变化。
- 每行：文件图标、状态文字标签与语义色、文件名、父目录相对路径后缀（仓库根层不显示）、完整路径提示；状态为 `新增/修改/删除/重命名/冲突/未知/未跟踪/已忽略`。
- 排序按完整相对路径，不区分大小写与重音，固定 Unicode 排序规则，权重相同按原始字符串序数比较；renamed 用新路径；ignored 目录用目录路径；结果不受系统区域设置影响。
- 单行聚焦模型，无多选、无批量操作、无分页。

### 过滤与 ignored 开关

- 头部过滤框占位 `过滤文件`，名称 `过滤 Commit 文件`；匹配文件名与完整相对路径，不区分大小写子串；renamed 按新路径；不匹配状态标签或提示文本。
- 过滤同时作用于三个分区；即时生效；计数变为过滤后条目数；排序与折叠不变；底层数据、聚焦路径、滚动锚点不变；聚焦行被过滤掉时方向键从最近可见行开始。
- 无匹配时各可见分组展开显示 `无匹配文件`；过滤为空恢复全量。
- 过滤文本瞬态：切换项目/数据根变化清空；切视图、换槽、隐藏保留；重启不保留；`Ctrl+F` 聚焦并全选；`Esc` 清空并保持焦点；`Enter` 不打开面板。
- `显示已忽略` 开关默认关闭，名称 `显示已忽略文件`；开启后分区默认展开、可独立折叠；关闭立即隐藏，不改变过滤文本、其他计数、排序与折叠；开关与折叠状态随侧栏视图状态持久化。

### 打开映射与去重

| 状态 | 面板 | 标题后缀 | 内容 |
|---|---|---|---|
| added / untracked | 文件打开路由（Markdown 或编辑器） | 无 | 工作区文件 |
| deleted | GitShow | `(git delete)` | HEAD 只读 |
| modified / renamed / conflict | Diff | `(git diff)` | HEAD 与工作区；renamed 旧 HEAD 对新工作区 |
| unknown | 文件打开路由；文件不存在则提示 + 静默刷新 | 无 | 工作区文件 |
| ignored | 不打开 | 无 | 只读条目 |

- 落点为活跃项目聚焦面板组；无组时创建首个组。
- 去重键归 `domain-model.md`：文件打开路由使用项目 + 规范化真实路径；Diff 键含 `mode/source`；GitShow 使用项目 + 仓库相对路径（大小写不敏感）。Commit 的文件打开路由不追加标题后缀；Diff/GitShow 按自身规则追加 `(git diff)`/`(git delete)`，后缀不参与路由去重键。跨项目不聚焦。
- 双击或聚焦行 `Enter` 打开；单击只设焦点；ignored 不打开；unknown 无右键菜单。
- 项目根外、仓库根内条目仍显示与打开：modified/renamed/conflict 以 Diff 只读对比模式，added/untracked/unknown 以根外只读打开，deleted 以 GitShow；不提供回滚或删除。该根外只读范围与 Explorer（根固定项目根）不对称，保持不变。
- 文件消失、内容不可读或面板创建失败：非模态错误提示 + 静默刷新，不静默。

### 右键菜单与定位

- 菜单项顺序：`在文件浏览器中显示`、`复制路径`、分隔线、破坏性操作（按状态）。
- `在文件浏览器中显示`：目标存在于工作区且位于项目根内才可用；执行时文件浏览器不可见则先恢复显示（槽位由侧栏框架决定），再展开层级、选中、滚动并聚焦。renamed 定位新路径。根外禁用并提示 `文件不在项目根内，无法在文件浏览器中显示`；deleted 禁用并提示 `文件已删除，无法在文件浏览器中显示`；其他不存在提示 `文件不存在，无法在文件浏览器中显示`。
- `复制路径`：工作区条目复制绝对路径；renamed 新路径；deleted 由仓库根 + HEAD 路径拼出；ignored 可复制；成功 `已复制路径`，失败非模态错误。
- 破坏性操作仅项目根内：modified/deleted/renamed/conflict → `回滚`；added/untracked → `删除`；ignored/unknown 无；根外行只提供复制路径并禁用破坏性项，提示 `项目根外文件需在终端或外部工具中处理`。
- 菜单支持指针位置与 `Shift+F10`/菜单键，键盘方向/Home/End/Enter/Esc，关闭后焦点回原行或最近行。

### 回滚与删除

- 确认框统一由 shell 确认服务提供，危险按钮 `回滚`/`删除`，默认焦点 `取消`；正文包含完整相对路径与后果；删除说明回收站恢复路径与永久删除回退。
- 执行前重新读取状态与路径；状态或路径变化中止并提示 `文件状态已变化，请刷新后重试` + 静默刷新；目标不存在提示 `文件不存在` + 刷新。
- modified/deleted/conflict 回滚：复用 `git::rollback`（`git checkout HEAD -- path`），deleted 从 HEAD 重建；conflict 覆盖为 HEAD 内容。
- renamed 回滚：先恢复旧路径，成功后才删除新路径；旧路径恢复失败不得删除新路径；新路径删除失败按部分成功报告并刷新。
- added 删除：先取消暂存（`git::unstage`），失败不得删除磁盘文件；成功后移入回收站，回收站不可用则永久删除；移入失败按部分成功，文件以未跟踪状态留存。
- untracked 删除：直接回收站；不可用回退永久删除。
- 破坏性操作在视图内串行；执行中的行忙碌，禁止再次操作或打开菜单；其他行保持可见；合并型操作成功/失败/部分成功均静默刷新。
- 成功非模态提示含操作与完整相对路径；失败/部分成功含操作、路径、原因与已完成/未完成部分；禁止只写日志。
- 删除优先系统回收站；符号链接只操作链接本身；无应用内撤销。

### 刷新与项目切换

- 监听仓库根内工作区文件与 Git 元数据（索引、HEAD、引用）；Git 对象库与日志不单独触发；仓库根外事件忽略。
- 尾部去抖 200ms，连续事件下最长 1000ms 内至少更新一次；静默刷新保留列表、尽量保持聚焦路径与滚动锚点、折叠不变。
- 刷新触发：仓库文件事件、回滚/删除结束、手动刷新、视图重新可见、数据根变化（走加载中）；不轮询。
- 刷新按钮 24×24px，提示 `刷新 Commit 列表`；无根与加载中禁用；错误态等效重试；就绪态静默刷新；进行中忙碌并合并后续触发；视图内 `F5` 等效。
- 视图不可见时暂停监听与去抖，进行中读取可完成并更新数据；重新可见时数据根变化则清空加载，否则静默刷新一次。
- 监听不可用时就绪态显示 `自动刷新不可用，可手动刷新`；手动刷新始终可用；下次项目切换或视图重开重试监听。
- 项目切换：清空列表、过滤、聚焦行、菜单与确认框，滚动置顶，进入加载中；分组折叠、已忽略分区折叠与 ignored 开关保留；旧读取结果丢弃；不显示上一项目数据。

### 错误与性能

- 错误分类独立文案与 `重试`；就绪刷新失败行内警告；操作失败/部分成功必须刷新并可见；错误与日志不含凭据。
- 10,000 条目下连续滚动与过滤保持响应；按需构建行；只保留当前数据根一份数据；不后台轮询不可见视图；超大未跟踪目录或 ignored 目录仍满足响应目标。

### 状态矩阵与持久化

| 状态 | 切视图/换槽/隐藏 | 切换项目 | 关闭重开 | 应用重启 |
|---|---|---|---|---|
| 列表数据 | 保留；不可见暂停刷新，重新可见静默刷新 | 清空并加载 | 保留；重新可见静默刷新 | 不保留 |
| 分组折叠 | 保持 | 保持 | 保持 | 持久化 |
| 已忽略分区折叠 | 保持 | 保持 | 保持 | 持久化 |
| ignored 开关 | 保持 | 保持 | 保持 | 持久化 |
| 滚动位置 | 保持 | 重置顶部 | 保持 | 重置顶部 |
| 聚焦行 | 保持；路径消失取最近行 | 清空 | 保持 | 无 |
| 过滤文本 | 保持 | 清空 | 保持 | 不保留 |
| 右键菜单 | 关闭 | 关闭 | 不保留 | 不保留 |
| 刷新警告 | 保持；重新可见静默重试 | 清除 | 保持 | 无 |
| 错误态 | 保持；重新可见静默重试 | 清除并加载 | 保持 | 无 |
| 进行中刷新 | 可完成并更新数据 | 丢弃旧结果 | 可完成 | 无 |
| 确认框 | 模态期间禁止切换 | 取消且不执行 | 不保留 | 无 |

- 折叠、已忽略分区折叠、ignored 开关归侧栏视图状态，应用级作用域；落点见 `persistence.md` 的 `view_states` 中 `commit` 行，本节不复制 schema。滚动、过滤、聚焦行仅运行期。
- 不做每项目数据缓存；视图可见性、槽位与宽度归侧栏框架。

### 复用与重建

| 旧实现 | 处置 |
|---|---|
| `git::{status, status_cached, parse_status, StatusSnapshot, StatusEntry, rollback, unstage, classify_git_failure}` | 直接复用；新增 ignored 记录读取与有界多根缓存 |
| `platform::file_operations::send_to_recycle_bin` | 复用；补永久删除回退与“先取消暂存”顺序 |
| 旧 `sidebar_views::CommitProjection` 的冲突去重、分支统计、历史渲染 | 参考冲突去重；目标分组改为 Changes/Unversioned/Ignored，staged/unstaged 概念废弃，投影重建 |
| 旧 Commit 无 ignored、无过滤、无定位/复制、无四态 | 全部重建 |
| Git 历史 `parse_git_history` | 本视图不展示提交历史，停止复用该渲染路径 |
