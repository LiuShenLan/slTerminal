# 编辑器面板

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 编辑器面板拥有文件打开路由、文档缓冲、运行模式、保存与外部修改契约；Markdown 文档面板、Diff、GitShow、设置中心是独立面板类型。工作区壳拥有焦点、布局与关闭守卫。

### 打开路由与面板身份

统一入口为 `FileOpenRouter::open(OpenRequest)`；所有来源（文件浏览器双击/Enter、拖放、Commit、Markdown 链接、布局恢复、单实例移交、Agent 全局文件）走同一链路。

```rust
pub struct OpenRequest {
    pub project: ProjectId,
    pub path: PathBuf,
    pub source: OpenSource,           // Explorer | DragDrop | Commit | MarkdownLink | Restore | AgentFiles
    pub target_group: Option<PanelGroupId>,
    pub outside_root_mark: bool,
}

pub enum OpenSource { Explorer, DragDrop, Commit, MarkdownLink, Restore, AgentFiles, SingleInstance }
pub struct DocumentKey { pub project: ProjectId, pub canonical_path: PathBuf }
```

分派：

| 扩展名（不区分大小写） | 目标 |
|---|---|
| `.md`、`.markdown` | Markdown 面板 |
| `.html`、`.htm` | 编辑器源码模式，显示右上 HTML 悬浮按钮 |
| 其余、无扩展名、点开头隐藏文件 | 编辑器 |

- 目录不是打开目标；文件浏览器自行展开/折叠。
- 去重键与路径规范化归 [domain-model.md](../domain-model.md) 的去重键与路径纯函数；命中编辑器去重键时激活项目、面板组与面板并聚焦，不新建面板或缓冲。
- 每项目每路径只有一个编辑器视图与一个文档缓冲；不同项目各自独立。
- 允许打开范围：项目根、项目根所在 Git 仓库实际根、已登记 Agent 全局目录。项目根与已登记 Agent 全局目录可编辑保存；项目根外、仓库根内且不在已登记 Agent 全局目录内只读；仓库根外仅允许经 Markdown 显式链接根外只读打开。系统另存为对话框是项目根与已登记 Agent 全局目录之外的唯一写入例外。
- 无活跃项目：不创建面板，提示“请先创建项目”。
- 打开失败（不存在、无权限、路径失效）仍创建错误态面板并保留路径，提供“重试”“用系统默认程序打开”；布局恢复同样不丢弃面板。
- 布局恢复只恢复路径、`PersistedPath.read_only`（按当前允许根重算）、自动换行、语言手动覆盖，不恢复文本、脏状态、光标、滚动或搜索；恢复不抢焦点。
- 空文件打开后显示空白可编辑区域与首行光标，不显示占位文案、欢迎页或“未打开文件”引导。
- 读取超过 150ms 显示轻量加载指示；超过 10,000,000 字节显示读取进度；加载期间不显示旧文件内容。
- 页签标题由路由服务统一计算：默认文件名；同项目出现同名不同路径时改用相对项目根路径，根外文件使用规范化绝对路径；另存为、重命名、关闭同名视图后重算；Commit 来源不追加后缀，未保存只用页签圆点表达。

运行模式在打开、外部修改导致大小变化、关闭重开、应用重启、Agent 根登记变化时重新判定：

```rust
pub enum EditorRunMode {
    Editable,             // 项目根、已登记 Agent 全局目录或运行期另存为授权
    ReadOnlyLarge,        // > 50,000,000 字节
    OutsideRoot,          // 根外只读
    Detached,             // 文件已移出允许根
    Error(EditorError),
}

pub enum WriteScope {
    ProjectRoot,
    AgentRoot,            // 已登记 Agent 全局目录
    SaveAsAuthorized,     // 当前运行期的另存为授权
}
```

持久参数以 `domain-model.md` 的 `EditorParams` 为准；`PersistedPath` 的类型与只读语义不在此复制。手动只读是叠加在可编辑模式上的用户锁，不产生新模式，且仍为运行期状态、不写入布局。已登记 Agent 全局目录折叠进 `Editable`，以 `WriteScope::AgentRoot` 区分可写作用域；其可写性按当前 Agent 根登记表在运行时判定，不新增持久化字段。手动只读的可用条件只看 `EditorRunMode::Editable` 且缓冲干净，不因 `WriteScope` 不同而分叉。

### 文档缓冲与编码/行尾

```rust
pub struct EditorDocument {
    pub key: DocumentKey,              // 打开路由去重身份
    pub path: PathBuf,                 // 当前保存目标
    pub buffer: TextBuffer,            // rope + 行索引
    pub encoding: TextEncoding,        // BOM 由枚举变体表达
    pub line_ending: LineEnding,
    pub final_newline: bool,
    pub baseline: Option<DiskBaseline>,
    pub mode: EditorRunMode,
    pub write_scope: WriteScope,
    pub manual_read_only: bool,
    pub dirty: bool,
    pub history: EditHistory,
    pub external: ExternalState,
    pub search: SearchState,
    pub folds: FoldSet,
}

pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
    Local(&'static encoding_rs::Encoding), // GB18030/GBK 等
}
pub enum LineEnding { Crlf, Lf }
pub struct DiskBaseline { pub size: u64, pub modified: SystemTime, pub file_id: FileId }
```

BOM 只由 `TextEncoding` 变体表达，不设独立 `bom` 字段；载入、保存与恢复快照以该枚举为唯一真值，禁止出现编码与 BOM 互相矛盾的组合。

载入顺序：

1. 读取元数据；元数据失败直接错误态，不做大小门控。
2. 按字节数分级：`≤ 10,000,000` 可编辑；`> 10,000,000 且 ≤ 50,000,000` 可编辑并提示大文件；`> 50,000,000` 只读浏览。
3. BOM 判定（UTF-8/UTF-16 LE/BE）；无 BOM 先试合法 UTF-8；失败后对常见本地编码做启发式检测（至少 GB18030/GBK），结果显示在状态条。
4. 前 8192 字节含 NUL 判为二进制，不建立编辑缓冲。
5. 编码无法判定进入错误态，提供“以编码重新打开”“用系统默认程序打开”。
6. 行尾按首个行尾确定；结尾是否有换行按原文件保持；混合行尾保存时统一为选定行尾。

编辑：

- 可编辑模式支持键入、删除、选区、复制、粘贴、剪切、全选、撤销、重做、括号匹配、当前行高亮、行号、标识符补全、`Tab`/`Shift+Tab` 缩进。
- 缩进风格按文件首行多数推断（制表符/空格；宽度 2/4/8，默认 4）；`Enter` 延续上一行缩进。
- 任何文本修改即脏；光标、选区、滚动、搜索、换行、字号、折叠、语言选择不改变脏状态；撤销回基线自动清脏。
- 撤销历史按文档维护，面板存活期间保留；关闭/重启不保留。
- IME 组合期间不打断；组合结束按一次修改计入撤销与脏状态；CJK 按实际显示宽度排版。
- 文件大于 10,000,000 字节关闭标识符补全；语法高亮、搜索与 Git 标记保留。
- 只读浏览与根外只读在超过 50,000,000 字节时使用分页文本源，只渲染可见窗口，不整份载入；超过 200,000,000 字节关闭语法高亮并在信息条说明。根外只读信息条同时标注 `项目根外` 与文件大小。

保存链：

```rust
pub enum SaveOutcome {
    Saved { baseline: DiskBaseline },
    Conflict { disk: DiskBaseline },
    Missing,
    ReadOnly(ReadOnlyReason),
    Failed { reason: String },
}
```

- `Ctrl+S` 保存聚焦编辑器；无聚焦编辑器不派发。
- 顺序：保存前校验磁盘版本（大小、修改时间、文件标识）→ 按选定编码/BOM/行尾/末尾换行编码 → 原子替换前经 `PathGuard` 复核真实路径与重解析点 → 原子替换 → 更新基线为本次写入版本 → 清脏 → 删除恢复备份 → 重算 Git 标记 → 合并 300ms 通知侧栏与 Commit。
- 保存冲突只由写入前版本校验触发；版本不一致时不写盘。原子替换成功后不再因磁盘变化进入保存冲突；替换后重新观察磁盘，若磁盘已再变化，按外部修改处理，把观察到的最新磁盘状态作为新基线，不重复冲突。与 Diff 保存链一致。
- 不做非原子降级；原子替换失败保留原文件与脏状态。
- 保存冲突对话框：“覆盖磁盘”“另存为”“取消”，默认焦点“取消”。干净文档不进入冲突，按外部修改自动重载。选择“覆盖磁盘”后写入前再次校验磁盘版本；若版本再次变化则重新进入保存冲突，不覆盖；一致则以当前缓冲原子替换并清除脏状态。
- 另存为使用系统文件对话框，筛选器“所有文件”；成功后更新文档身份、路径、保存目标、页签标题与布局路径；授权只绑定当前文档与当前运行期，不写入布局，重启后根外路径按根外只读重新判定。目标已打开时显示“目标文件已打开”，提供“取消”“替换”；替换前对目标脏缓冲要求保存/丢弃确认。
- 保存时文件已不存在：显示“文件已不存在”，默认“另存为”，另有“保存到原路径（重新创建）”“取消”。
- 只读原因分别提示：只读浏览、根外只读、手动只读；文件已移出允许根的脱离缓冲 `Ctrl+S` 打开另存为。

### 编辑视图、语法高亮与折叠

编辑区使用自建 GPUI `EditorElement`，不再以 `gpui_component::InputState` 作为文档权威：

- 文档文本与选择以字节偏移/行列为权威；渲染按可见行虚拟化，逐行 shape；行高 = 字号 × 1.5，四舍五入到整像素。
- 可见元素：行号、当前行高亮、折叠标记、Git 标记、按需滚动条、顶部搜索面板、冲突/删除横幅、只读信息条、底部状态条、`.html/.htm` 的右上悬浮按钮。无小地图、无面板内工具栏。
- 焦点进入编辑区时显示光标；点击定位，拖拽选择，双击选词、三击选行；只读模式同样支持选择与复制。
- `Ctrl+滚轮` 仅在指针位于编辑区时调整全局编辑器字号（8–32，默认 14），立即重排，不重建缓冲；2 秒去抖保存。
- 自动换行按面板独立，默认关闭；`Alt+Z` 切换；随布局持久化；`Tab` 插入缩进不移动焦点；`Ctrl+Tab`/`Ctrl+Shift+Tab` 由工作区处理。

语法高亮：

```rust
pub trait SyntaxProvider {
    fn language_for_path(&self, path: &Path) -> LanguageId;
    fn parse(&self, doc: &TextBuffer, edits: &[TextEdit]) -> ParseTree;
    fn highlights(&self, tree: &ParseTree, range: LineRange) -> Vec<HighlightSpan>;
    fn fold_regions(&self, tree: &ParseTree) -> Vec<FoldRegion>;
    fn bracket_at(&self, doc: &TextBuffer, offset: usize) -> Option<BracketPair>;
}
```

- 扩展名与固定文件名到语言的映射表归[规格编辑器篇](../../../featureSpec/panels/editor.md) §编辑、语言高亮与脏状态；本设计不复制第二张表。
- 实现为 `LanguageRegistry`：注册映射表条目、语言别名归一（`js`/`node`/`jsx` → JavaScript 等）与未知语言纯文本回退；为每种语言注册 Tree-sitter 语法与查询。编辑器、Diff、GitShow 与 Markdown 代码块共用同一注册表，Markdown 代码块语言别名归一后查同一注册表。
- 实现选型：Tree-sitter 增量解析 + 每语言查询；高亮在后台或分帧执行，只计算可见区域与 overscan。无法解析时回退纯文本，不阻塞输入。
- 路径变化导致语言变化时热切换规则，不重建缓冲、不丢文本/脏状态/光标/撤销历史。
- 手动语言覆盖是面板参数，随布局持久化；只影响高亮，不改内容、扩展名或保存行为；关闭重开恢复“自动（按扩展名）”。

折叠：

- `FoldRegion { start_line, end_line }` 来自语法树；纯文本不显示标记。
- 快捷键 `Ctrl+Shift+[` 折叠当前块、`Ctrl+Shift+]` 展开、`Ctrl+Alt+[` 全部折叠、`Ctrl+Alt+]` 全部展开；可重绑。
- 折叠状态是面板运行期状态：切换面板/项目保留，关闭重开/重启不保留；内容变化后失效区域自动展开。
- 折叠只改显示，不改内容、不改变脏状态、不进入撤销历史；只读模式同样可用。
- Gutter 中折叠标记与 Git 标记是独立命中区；点击空白不动作。

### 查找/替换与跳转

```rust
pub struct SearchQuery {
    pub text: String,
    pub case_sensitive: bool,   // 默认 false
    pub whole_word: bool,       // 默认 false
    pub regex: bool,            // 默认 false
}
pub struct SearchState { pub query: SearchQuery, pub current: usize, pub total: usize, pub panel_open: bool }
```

- `Ctrl+F` 打开顶部搜索面板并聚焦输入；`Esc` 关闭并返回编辑区。
- 增量匹配；当前匹配滚入视口并强高亮，其余弱高亮；无结果显示“无结果”。
- 导航：`Enter`/`F3`/`Ctrl+G` 下一个；`Shift+Enter`/`Shift+F3`/`Ctrl+Shift+G` 上一个；循环。
- 替换当前匹配并跳到下一个；全部替换先确认“替换全部 N 处？”，整个操作是单个撤销步骤；正则替换支持捕获组；非法正则内联报错。
- 只读模式提供查找，不提供替换；大文件只读以流式扫描执行，显示进度与取消。
- `Ctrl+Alt+G` 跳到行，越界提示实际范围。
- 搜索状态按面板保留；关闭重开/重启重置。`Ctrl+F` 为保留键，不可重绑。

### 自动保存、崩溃恢复与外部修改

自动保存：

```rust
pub enum AutoSaveState {
    Idle,
    Scheduled { due: Instant },
    Saving,
    Suspended { reason: SuspendReason }, // ExternalConflict | SaveDialog | ReadOnly | ManualReadOnly | Detached | Error
    Failed { reason: String },
}
```

- 外观页开关，默认关闭，全局生效。
- 开启后停止输入 1 秒且仍脏则保存；焦点离开编辑器或窗口失焦立即保存；关闭面板前按关闭守卫的自动保存例外先保存一次。
- 外部冲突横幅、保存冲突对话框、各种只读、文件已移出允许根的脱离状态与错误态暂停；下一次文本修改后恢复计时。脱离状态计时器到期不写盘、不打开另存为，关闭时进入关闭守卫。
- 失败沿用“保存失败：〈原因〉”，保留脏状态；同一原因连续失败合并显示，不按周期重复弹错。
- 自动保存只作用于编辑器，不作用于 Markdown 源码态与 Diff。

崩溃恢复：

```rust
pub struct RecoveryKey { pub project: ProjectId, pub panel: PanelId }
pub struct RecoveryStore;
impl RecoveryStore {
    fn snapshot(&self, key: RecoveryKey, doc: &EditorDocument) -> Result<(), RecoveryError>;
    fn delete(&self, key: RecoveryKey) -> Result<(), RecoveryError>;
    fn scan(&self) -> Vec<RecoveryEntry>;
}
```

- 快照键为 `(ProjectId, PanelId)`；保存目标路径取自 `EditorDocument.path` 并随快照载荷保存，重命名、另存为与跨类型转换不改变键。
- 与 Markdown 共用同一 `RecoveryStore`；配额、淘汰与保留期归 `persistence.md`，此处不复制；项目移除时删除该项目全部快照。
- 只备份脏的可编辑文档（含脱离缓冲），不含干净、只读浏览、根外只读、手动只读与错误态。
- 快照内容：文本、编码（含 BOM）、行尾、保存目标、快照时间；不含凭据、终端输出、剪贴板。
- 脏缓冲自最后修改起约 2 秒写一次；持续编辑最长 30 秒刷新一次；失败提示一次并继续重试。
- 保存成功、撤销回基线、关闭文档、正常退出后删除备份。
- 启动发现备份时聚合列出项目、路径与时间，提供“恢复”“丢弃”“全部恢复”“全部丢弃”；恢复进入脏态且不写盘，磁盘不同则显示冲突横幅；未处理前不得用备份覆盖布局内容。
- 所属项目不存在时恢复到当前活跃项目；无活跃项目提示“请先创建项目”并保留备份。

外部修改：

```rust
pub enum ExternalState {
    Synced,
    Reloaded { at: Instant },
    Conflict { disk: DiskBaseline },
    Deleted,
    Detached,
    Unreadable { reason: String },
}
```

检测通道：

- 项目文件监听器覆盖项目根；另存为、Commit、根外链接打开的文件监听其父目录。
- 窗口重新获得焦点时对全部已打开文档做一次版本复核（约 1 秒节流）。
- 面板激活时复核当前文档。
- 监听事件积压/溢出时全量复核；文件事件约 300ms 合并。
- 自身保存事件按写入后版本匹配忽略一次，不保留按路径的长期抑制。

处理：

| 场景 | 行为 |
|---|---|
| 干净 + 外部修改 | 自动重载，按行锚点保持光标与滚动，显示约 2 秒“已从磁盘重新加载” |
| 脏 + 外部修改 | 顶部非模态横幅“文件已被外部修改”，提供“重载”“保留我的修改”“查看差异”；不抢焦点 |
| 脏 + 查看差异 | 以磁盘快照与当前缓冲打开 Diff 冲突快照并定位首个差异块；不改变缓冲、脏状态与横幅 |
| 干净 + 增长超过 50,000,000 | 切只读浏览，保留顶部行 |
| 脏 + 增长超过 50,000,000 | 保持可编辑，横幅提示保存后重新打开将只读 |
| 外部删除 | 横幅“文件已从磁盘删除”，提供另存为、保存到原路径、关闭；只读模式只提供重试与关闭 |
| 外部重命名（有映射） | 跟随新路径；允许根外进入脱离态；新路径已打开按“目标文件已打开” |
| 重载读取失败 | 横幅“无法读取外部修改”，提供重试、保留当前内容 |
| 保存与外部写入并发 | 保存前校验版本，变化则中止并进入保存冲突；一致则原子替换，替换后磁盘再变化按外部修改处理并以最新磁盘状态为基线，不重复冲突 |
| 只读浏览、根外只读、手动只读 + 外部修改 | 事件到达即重读；保持顶部行；失败保留最后一次成功内容并轻提示，不弹冲突横幅 |

- 外部修改使文件缩小回可编辑范围时，只读浏览自动切回可编辑；根外只读保持。
- 冲突状态属于文档；横幅在唯一视图顶部显示，页签显示冲突提示点。
- 跨类型重命名（`.md` ↔ `.html`/文本）由 `PanelKindTransition` 原子转换：同一 `PanelId`、同一面板组与顺序，携带文本、脏状态与保存目标，旧实体不经过关闭守卫；目标为 Markdown 时，脏缓冲或原形态为源码则保持源码态，否则进入渲染态；目标为编辑器时显示源码。目标路径已有同类型面板时暂停转换，显式确认替换；目标有脏修改时二次确认保存/丢弃。

### Git 行标与 gutter Diff

- 行号右侧保留固定 9px Git 标记栏；无标记也保留宽度，文本不横向跳动。
- 标记表达工作区与 HEAD 的差异，不表达暂存区：新增实心竖条、修改空心竖条、删除向上三角；形状与颜色同时区分。
- 差异块规则：纯新增逐行新增；纯删除在删除位置相邻行显示三角（文件末尾显示最后一行）；修改块按较小行数逐行修改，多出的行新增或删除。
- 触发：打开、保存成功、外部重载、撤销/重做、最后一次编辑后约 300ms 防抖。编辑过程中标记随文本位置平移，重算后与内容一致。
- 计算复用 `git::repository_root_cached` + `file_status` + `file_hunks`；非仓库/读取失败静默无标记，下次触发重试。
- 单击标记打开 Diff 面板并定位差异块；编辑器面板与缓冲不关闭；空白点击不动作。
- 只读浏览与根外只读不显示标记栏；手动只读仍显示并可打开 Diff。
- 颜色来自 `EditorThemeTokens` 语义槽。

### 拖文件、HTML 按钮、焦点与状态条

拖文件：

- 来源仅文件浏览器文件；目录不触发。外部操作系统文件拖入不支持；拖入无动作或一次性提示 `请从文件浏览器拖动文件`。
- 目标为包含至少一个编辑器面板的面板组；进入时高亮，离开/取消取消动作，非编辑器组不接受。
- 释放后走标准路由：Markdown → Markdown 面板，HTML/文本 → 编辑器，二进制 → 错误态。
- 已打开则聚焦；不替换目标组当前脏面板；不修改内容、不改变脏状态、不触发保存。

HTML 悬浮按钮：

- 仅 `.html`/`.htm` 显示，位于内容区右上；键盘可聚焦触发。
- 用系统默认程序打开磁盘版本，不保存、不提示未保存修改；脏状态保持。
- 打开失败显示非阻塞提示；路径参数单独传递，不经过命令解释器。

状态条：

- 仅成功载入的可编辑、只读浏览、根外只读、手动只读模式显示；错误态不显示。
- 左到右：语言、编码（含 BOM）、行尾、只读原因；右侧手动只读锁按钮。
- 语言项打开语言菜单；编码项打开编码菜单，选择后“以编码重新打开”，脏时先确认丢弃本地修改；行尾项切换 CRLF/LF，按一次修改计入脏状态；只读原因只读展示。
- 锁按钮仅在可编辑且缓冲干净时可用；脏时提示“保存或撤销后才能开启手动只读”。手动只读叠加在可编辑模式，不覆盖自动只读判定；关闭重开/重启重置。

焦点与键盘：

- 焦点落点为编辑区；显式打开/激活聚焦，布局恢复不抢焦。
- `Ctrl+S`、`Alt+Z`、折叠命令可重绑；`Ctrl+F` 保留；搜索导航键与撤销/重做由编辑器内部处理；`Ctrl+W` 与焦点循环由工作区处理。
- 冲突横幅出现不抢焦点；横幅按钮可 Tab 到达。
- 右键使用系统标准文本编辑菜单，不提供文件级操作。

### 状态保留

| 状态 | 同组切面板 | 切换项目 | 关闭重开 | 重启 |
|---|---|---|---|---|
| 缓冲（文本、编码、行尾、撤销、脏状态） | 保留 | 保留 | 确认后丢弃，按磁盘重读 | 按磁盘重读；异常退出走恢复备份 |
| 光标、选区、滚动 | 保留 | 保留 | 不保留 | 不保留 |
| 保存目标与路径 | 保留 | 保留 | 保留（布局路径） | 保留（布局路径） |
| 搜索、折叠 | 保留 | 保留 | 重置 | 重置 |
| 自动换行 | 保留 | 保留 | 重置为设置默认 | 保留（布局） |
| 语言手动覆盖 | 保留 | 保留 | 重置自动 | 保留（布局） |
| 手动只读 | 保留 | 保留 | 重置 | 重置 |
| 字号、主题 | 全局 | 全局 | 全局 | 全局 |
| Git 标记、状态条 | 保留并重算 | 保留 | 重算 | 重算 |
| 外部基线、冲突横幅 | 保留 | 保留 | 重建/清除 | 重建/清除 |

### 模块树与复用/重建

```text
领域 core crate
  panels/recovery.rs   # 编辑器与 Markdown 共用的崩溃快照存储；键 (ProjectId, PanelId)
  panels/editor/
    params.rs
    document.rs        # 模式、可写作用域、编码、行尾、基线、脏状态
    buffer.rs          # rope + 行索引 + 编辑事务
    encoding.rs        # BOM/UTF-16/本地编码探测与往返
    search.rs          # 查询、匹配、替换事务
    fold.rs
    gutter.rs          # 行标形状与差异块到行标的投影
    external.rs        # 外部修改状态机
    open.rs            # DocumentKey、打开范围判定

slterm_app/src/panels/editor/
  panel.rs
  view.rs              # EditorView/EditorElement
  syntax/              # Tree-sitter 语言注册与查询
  status_bar.rs
  search_bar.rs
  conflict_banner.rs
  html_button.rs
  drag_target.rs
  autosave.rs
```

复用：`document_io::read_prefix`、`atomic_file::write`、`text_document` 的 BOM/行尾往返语义、`file_editor::gutter` 的状态/hunk 投影、`EditorThemeTokens`、`file_editor` 的虚拟化块槽思路、`fs_watch` 的监听与去重结构。重建：`TextFileView` 的 `InputState` 驱动外壳、双模态实时块编辑、旧 `CodeTabView` 多模式包装、8MB 截断模型、搜索/折叠/编码/自动保存/恢复缺失面。

### 测试接缝

- 编码探测与往返（BOM、UTF-16、GB18030）、BOM 仅由 `TextEncoding` 表达的单真值往返、行尾切换与混合行尾统一、末尾换行保持。
- 大小分级边界（10,000,000/50,000,001/200,000,000）、二进制 NUL、元数据失败；根外只读大文件分页与流式查找。
- 保存链：写入前版本冲突、文件不存在、只读、原子替换失败、另存为目标已打开；原子替换后磁盘再变化不重复冲突并按外部修改建立基线。
- 外部修改状态机：干净重载、脏横幅、删除、重命名、脱离、监听溢出、保存并发；只读模式静默刷新与失败保留。
- 自动保存计时与暂停条件，含脱离状态不写盘、不弹另存为；恢复备份按 `(ProjectId, PanelId)` 写入/清理、路径随载荷、重命名与跨类型转换后删除/恢复、与 Markdown 共享配额、启动聚合提示。
- 搜索增量、替换全部单撤销、非法正则；折叠区域重算；`LanguageRegistry` 代表性扩展名与固定文件名映射、别名归一、未知回退纯文本与语言热切换。
- Git 行标纯映射与去抖触发；gutter 点击请求携带行号。
- 虚拟窗口：拖文件高亮与不替换脏面板、HTML 按钮、状态条交互、搜索焦点、关闭守卫。
- 人工核验：IME、字体渲染与视觉对齐、系统默认程序打开；豁免原因、兜底层级与核验方式登记见 `.claude/docs/testing/exemptions.md`。
