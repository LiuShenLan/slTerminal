# 07 文件与编辑详细设计

> pebrel-design 分片 07/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/07-files-editor.md`(spec 分片,含采纳点编号 1–44);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本片消费锚点:`PaneId`/`TabId`/`WorkspaceTab`/`TabEntry`/`TabMeta` 归 05 篇,本片只许 `use`;`Rgb`/`ReviewedPalette`/`ThemeUiColors`/`RuntimeSettings`/设置注册表归 06 篇;`AppError`/`brand` 归 01 篇。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。
>
> 事实修正登记:spec 分片 07 采纳点 41 称 slTerminal 自有「git 命令封装」为子进程形态;实测 `src-tauri/src/git/` 为 **git2(libgit2 vendored 静态链接)封装**,非子进程。本片改造节按此事实重述;执行体统一裁定 D07-1 已裁 = git 子进程统一(git2 不移植,语义资产转测试夹具保留)。

## 目标形态

文件与编辑在 GPUI 单进程壳内重建为**一个编辑器内核 + 一个文件树模型 + 一条打开路由 + 两个新建项(git gutter / 通用 diff)**:

- **编辑器内核** `slterm_app::gpui_shell::file_editor/`(pebrel 照抄改名):`TextFileView` 双模态(markdown 默认渲染态、显式切源码)为产品形态基准;live_edit 投影内核 + 结构化块事务 + input_rules + EditHistory 统一撤销构成编辑内核;大文件语义 = 上限截断 + truncated 提示横幅。
- **tab 路由** `slterm_app::gpui_shell/code_tab.rs` + `doc_tabs.rs`(照抄改名):普通文件 = 可保存文本编辑器;Git 冲突 = 三栏合并器;图片 = ImageTabView;`openable_in_app` 为双击路由单判定。
- **文件树** 模型 `slterm_app/src/side_panel/`(pebrel `display/side_panel/` 照抄改名,脱离旧 display 壳;旧壳渲染 `render.rs`/`notice.rs` 不迁)+ 渲染 `gpui_shell/workspace/file_tree.rs` + `file_tree/path_bar.rs`。
- **新建项**:`git gutter`(编辑器行标,pebrel 无对应物)与 `通用 diff 对照面`(旧 diff 面板语义重定)签名级设计见改造节。
- **键位**:`settings.json` 的 `keybind` JSON 段(结构归 06 篇 D06-1 键形态终裁),pebrel `keybind_pairs` 语义(行序即优先级、整表替换、恢复默认精确收回)照抄。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_app/src/text_document.rs` | pebrel `nebula_app/src/text_document.rs` 照抄 | `TextSnapshot`(bom/crlf/truncated/invalid_encoding/read_only)+ `SaveError` + `MAX_BYTES`——编码/行尾/快照比对单点 |
| `slterm_app/src/document_io.rs` | pebrel `nebula_app/src/document_io.rs` 照抄 | `read_prefix` 有界前缀读取(内存上限先于解码) |
| `slterm_app/src/gpui_shell/file_editor/` | pebrel 同名目录照抄改名 | 双模态编辑器全家(下表) |
| `slterm_app/src/gpui_shell/code_tab.rs` | pebrel 照抄改名 | `CodeTabView` 普通文件 + `MergeEditor` 三栏合并 |
| `slterm_app/src/gpui_shell/doc_tabs.rs` | pebrel 照抄改名 | `ImageTabView` + `openable_in_app` |
| `slterm_app/src/gpui_shell/diff_tab.rs` | **新建** | 通用 diff 对照面(改造节 4) |
| `slterm_app/src/side_panel/` | pebrel `display/side_panel/` 照抄改名 | 文件树模型:enumerate/gitignore/vcs/search/FileRow/SidePanel |
| `slterm_app/src/gpui_shell/workspace/file_tree.rs` + `file_tree/path_bar.rs` | pebrel 照抄改名 | 文件树渲染 + `PathEditor` + `FileTreeContextMenu` |
| `slterm_app/src/gpui_shell/file_drop.rs` | pebrel 照抄改名 | `FileTreeDrag` 载荷 + `FileDragGhost` |
| `slterm_app/src/git/` | slTerminal `src-tauri/src/git/` 语义资产转测试 + 操作面子进程重实现 | git 子进程统一(D07-1 已裁):status/diff/hunks/HEAD/rollback/unstage 经 git CLI,git2 不移植(改造节 2) |
| `slterm_app/src/gpui_shell/editor_theme.rs` | **新建** | 编辑器配色 token 装配(消费 06 篇语义槽,改造节 8) |
| `slterm_app/src/gpui_shell/file_editor/gutter.rs` | **新建** | git gutter(改造节 3) |

file_editor 内部职责照 pebrel 原分层不变:`mod.rs`(TextFileView 壳/状态机)/`document.rs`(load/save 编排)/`edit_history.rs`/`outline.rs`+`outline_view.rs`/`details.rs`+`info.rs`(大纲/详情侧栏)/`preview.rs`/`activity.rs`(虚拟化与资产释放)/`live_edit.rs`/`live_commands.rs`/`live_navigation.rs`/`live_selection.rs`(WYSIWYG 内核)/`inline_edit.rs`/`inline_selection.rs`(源位置投影)/`block_structure.rs`/`block_inline.rs`/`structure_view.rs`/`structure_inline_view.rs`/`structure_commands.rs`(结构化块)/`input_rules.rs`/`code_actions.rs`/`images.rs`/`image_cache.rs`/`chrome.rs`/`reader_presentation.rs`/`source.rs`(Local 单态)。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本片涉及 WebView / IPC / Tauri / Dockview / xterm.js / vitest / CM6 / iframe / Dockview 的词一律只在「消亡 / 映射 / 来源」语境。
2. **类型锚点纪律**(01 篇表):本片唯一定义文件/编辑域内部类型(`TextSnapshot`/`Document`/`EditHistory`/`Outline`/`Projection`/`BlockStructure` 族/`MergeEditor` 族/`FileRow`/`SidePanel`/`PathEditor`/`FileTreeDrag`/`GutterMark`/`DiffTabView` 等);跨领域类型只消费不定义:`PaneId`/`TabId`/`WorkspaceTab`/`TabEntry`/`TabMeta`(05)、`Rgb`/`ReviewedPalette`/`ThemeUiColors`/`RuntimeSettings`/`SettingsKeyDef`(06)、`AppError`/`brand`(01)。`WorkspaceTab::Document`/`Image`/`Code` 三变体的**渲染分支、关闭语义、持久化语义**归本片(05 篇缝合点 5 已登记),变体定义权在 05 禁改动。
3. **面板封闭三件套**(05 篇):本片新增的两类 tab(git gutter 是编辑器内嵌条不是 tab,通用 diff 是)——`DiffTabView` 若进 `WorkspaceTab` 需回 05 篇加变体;默认设计为 `WorkspaceTab::Code` 的只读臂(左/右只读 InputState),不新增变体,05 篇零改动;若 D07-2 裁定独立形态再回 05 登记。
4. **配色单点**(06 篇):编辑器/gutter/diff/文件树的一切颜色经 `editor_theme.rs` 装配的语义槽消费(06 篇 `ReviewedPalette`/`ThemeUiColors`),禁硬编码颜色;文件图标色归 06 篇主题 token 体系,本片不建色表。
5. **截断语义不可回退**(spec 采纳点 34,已定):可编辑域上限 = pebrel `MAX_BYTES`(8MB),超限 = 截断 + `truncated` 只读 + 提示横幅;slTerminal 旧分块只读浏览(`largeFileViewer`)/1MB 警告弹窗/四层防线整体消亡,不回填兼容。
6. **只读 contract 类型层强制**:`TextSnapshot.read_only`(truncated/invalid_encoding/只读文件)在 `encode` 入口拒绝(`SaveError::ReadOnly`);gitshow/html/diff 左栏等「只读查看」一律经 `decode_prefix(.., read_only=true, ..)` 载入,不靠 UI 层自觉。
7. **git 纪律不碰可选锁**(spec 采纳点 41,已定):一切 git 读操作不得触发 index 锁或后台 gc——执行体 = git 子进程统一(D07-1 已裁),每命令 `--no-optional-locks` 照抄 pebrel;git2 形态不迁,`GIT_OPTIONAL_LOCKS=0` 等价臂随之消亡。纪律本身不可谈判。
8. **文件树 ignored 语义调和**(两仓纪律并存,改造节 6):pebrel `FileRow.ignored` = ignored 条目仍在树中、降色渲染;slTerminal 旧「status 不扫 ignored」= 状态图不含被忽略文件。新世界两纪律同时成立,互不取消。
9. **SSH/WSL/SVN/亮主题整面不迁**(spec 不采纳点 1/2/12 与采纳点 41 末句):`DocumentSource` 收敛 `Local(PathBuf)` 单态;`path_bar`/`enumerate` 的 WSL guest 分支、`vcs.rs` SVN 臂、`code_actions.rs` `CodeUiColors` 亮角色表全砍。

## 关键类型与签名

> 均为草稿级签名;照抄部分与 pebrel 一致(裁剪点已注)。锚点类型他篇只许 `use`。

### 文本快照与有界读取(本片内部锚点,照抄)

```rust
// slterm_app/src/text_document.rs —— 编码/行尾/快照比对单点(pebrel 同名文件照抄)
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;   // 可编辑域上限(契约值,spec 采纳点 34)

pub(crate) struct TextSnapshot {
    pub text: String,
    pub bytes: Arc<[u8]>,            // 载入原文逐字节副本——保存前比对的权威
    pub bom: bool,                   // UTF-8 BOM 保留往返
    pub crlf: bool,                  // CRLF 行尾保留往返
    pub truncated: bool,             // 超限截断 → 强制只读
    pub invalid_encoding: bool,      // 非法 UTF-8/NUL → 强制只读
    pub read_only: bool,
}
pub(crate) enum SaveError { Changed, ReadOnly, Io(std::io::Error) }

impl TextSnapshot {
    fn decode(bytes: Vec<u8>, read_only: bool) -> Self;          // 超限截断+非法编码判定
    fn decode_prefix(bytes: Vec<u8>, read_only: bool, truncated: bool) -> Self;
    fn encode(&self, text: &str) -> Result<Vec<u8>, SaveError>;  // read_only → ReadOnly 拒绝
    fn verify(&self, current: &[u8]) -> Result<(), SaveError>;   // 磁盘已变 → Changed 拒绝覆盖
}

// slterm_app/src/document_io.rs
pub(crate) fn read_prefix(path: &Path, max_bytes: usize) -> io::Result<(Vec<u8>, bool)>; // (bytes, truncated)
```

CRLF 检测面吸收 slTerminal 旧 `src-tauri/src/fs/` 语义资产(spec 采纳点 40):64KB 样本判行尾并入 `decode` 的 `crlf` 判定,旧 keyset 分页/256KB 分块/沙箱校验随 IPC 消亡不迁。

### 编辑器壳(TextFileView,照抄锚点)

```rust
// slterm_app/src/gpui_shell/file_editor/mod.rs
actions!(file_editor, [SaveFile, ToggleSource, FinishBlockEdit, BoldSelection, ItalicSelection]);

pub enum TextFileEvent {
    Changed,
    DetailsRequested,
    SelectionContextMenuRequested { position: Point<Pixels>, text: String },
    ReaderFocusChanged { focused: bool },
}

pub struct TextFileView {
    pub path: PathBuf,
    pub title: String,
    input: Entity<InputState>,              // 源码态原生输入(gpui-component)
    history: EditHistory,                   // 跨表示统一撤销(spec 采纳点 4)
    source: DocumentSource,                 // Local(PathBuf) 单态(Remote 随 SSH 砍)
    document: Option<LoadedDocument>,       // TextSnapshot + Document
    operation: Option<DocumentOperation>,   // 后台保存/重载任务在途守卫
    dirty: bool, saving: bool, loading: bool,
    notice: Option<(Message, Option<String>)>,  // truncated/保存失败等提示横幅(i18n typed ID eg 06)
    markdown: bool, preview: bool,          // 双模态:markdown 默认 preview=true
    live_edit: Option<LiveEdit>,            // 聚焦块的 WYSIWYG 输入实体
    render_active: bool, preview_stale: bool,
    show_details: bool, details_hosted: bool, info: bool,   // 右侧栏大纲/信息双节
    outline: Outline,                       // 四索引同源(spec 采纳点 5)
    blocks: Rc<RefCell<Vec<Option<Entity<TextViewState>>>>>,        // 虚拟化块槽
    inline_views: Rc<RefCell<BTreeMap<(usize, usize), WeakEntity<TextViewState>>>>, // 内联格式视图
    preview_images: Entity<DocumentImageCache>,
    scroll: ListState,                      // 预览虚拟化列表状态
    details_width: f32, collapsed_headings: HashSet<usize>, selected_heading: Option<usize>,
    revision: u64, content_revision: u64, preview_task: Option<Task<()>>,
    // ……(余字段照抄,略)
}

impl TextFileView {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self;
    fn new_with_source(source: DocumentSource, window: &mut Window, cx: &mut Context<Self>) -> Self;
    pub fn is_dirty(&self) -> bool;        // tab 标题脏标记「•」数据源
    pub fn tab_title(&self) -> String;
    pub fn save(&mut self, cx: &mut Context<Self>) -> Task<bool>;   // 快照比对→后台写盘
    fn request_reload(&mut self, window: &mut Window, cx: &mut Context<Self>); // 外部修改→脏确认弹窗
    fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<Self>); // 渲染态↔源码态
    fn apply_outline(&mut self, outline: Outline, cx: &mut Context<Self>);     // 防抖重排落点
    fn schedule_preview(&mut self, cx: &mut Context<Self>);                    // 250ms 防抖
}
```

### 编辑内核(照抄锚点)

```rust
// edit_history.rs —— 源码态与渲染态共享一份撤销历史(spec 采纳点 4)
pub(super) struct EditHistory { /* undo/redo 栈,条目上限 8MB(MAX_HISTORY_BYTES) */ }
impl EditHistory {
    pub fn reset(&mut self, text: &str);
    pub fn barrier(&mut self);                     // 撤销组边界(连续输入合并的单位)
    pub fn record_rope(&mut self, text: &Rope);    // rope 差量提取,不物化全文
    pub fn record(&mut self, text: &str);
    pub fn travel(&mut self, redo: bool) -> Option<(String, Range<usize>)>; // (全文,变更 span)
}

// outline.rs —— mdast 一次解析,四索引同源(spec 采纳点 5)
pub(super) struct Heading { pub label: String, pub depth: u8, pub row: u32,
                            pub block: usize, pub parent: Option<usize>, pub indent: usize }
pub(super) struct Outline {
    pub headings: Vec<Heading>,            // 大纲导航索引
    pub blocks: Vec<String>,               // 预览块清单(虚拟化行源)
    pub limited: bool,                     // 预览上限截断标志
    pub source_ranges: Vec<Range<usize>>,  // 块→源码 span(导航/编辑同源)
    pub structures: Vec<Option<Arc<BlockStructure>>>, // 块→结构化部件
}
impl Outline {
    pub(super) fn prepare(source: &str, base: Option<&Path>) -> Self; // parse + 图片重写基址
    pub(super) fn edit_block_at(&mut self, offset: usize) -> usize;
    pub(super) fn visible_headings<'a>(&'a self, collapsed: &HashSet<usize>) -> Vec<...>;
    pub(super) fn source_span(&self, block: usize) -> Option<Range<usize>>;
    pub(super) fn replace_block(&mut self, block: usize, next: String);
}
// 上限契约(照抄):MAX_PREVIEW_BYTES = 512KB(预览域)/ MAX_BLOCK_BYTES = 32KB(单块)

// inline_edit.rs —— 源位置投影内核(spec 采纳点 7)
pub(super) struct Marks { /* bold/italic/code/link 行内格式标记集合 */ }
pub(super) struct Projection { /* 可见文本 ↔ 源码 span 双向映射;未改叶子与链接目标逐字节不动 */ }
impl Projection {
    pub fn new(source: &str) -> Self;
    pub fn with_reveal(source: &str, reveal: Option<Range<usize>>) -> Self; // 行内源码显影(spec 采纳点 8)
    pub fn accept(&mut self, text: &str, source: &str) -> bool;  // IME 组合期不重排输入实体
    pub fn marks(&self) -> impl Iterator<Item = (Range<usize>, Marks)> + '_;
    pub fn visible_offset(&self, offset: usize) -> usize;        // 源→可见
    pub fn source_offset(&self, offset: usize) -> usize;         // 可见→源
    pub fn toggle_mark(&self, selection: Range<usize>, marker: &str) -> Option<String>;
}
pub(super) fn changed_span(before: &str, after: &str) -> (Range<usize>, Range<usize>); // 差分定位

// live_edit.rs / live_navigation.rs / live_selection.rs / inline_selection.rs
pub(super) struct LiveEdit { projection: Projection, input: Entity<InputState>,
                             decorations: TextDecorationCollection }
pub(super) fn decorations(projection: &Projection, cx: &App) -> Vec<TextDecoration>; // 格式标记留源码、可见态只投影
pub(super) fn move_live_edge(...);          // 方向键跨输入边界不离开格式化文档
pub(super) fn sync_live_selection(...);     // 行内源码可见性跟随光标(不加撤销项)
pub(super) fn inline_selected_text(...) -> Option<String>; // 相邻内联视图共享段落复制

// block_structure.rs / block_inline.rs —— 容器拥有布局,输入只改内容 span(spec 采纳点 9)
pub(super) enum PartKind { Rich, /* 数学/代码部件归 08 渲染,本片只锚编辑结构 */ }
pub(super) struct EditPart { pub range: Range<usize>, pub kind: PartKind, /* …… */ }
pub(super) struct ListItem { /* checkbox/序号/内容 span */ }
pub(super) enum StructureNode { Inline(Vec<InlineRun>), Table { rows: Vec<Vec<usize>>, span: Range<usize> },
                                List(Vec<ListItem>), Literal(usize) /* 病理块回退 */ }
pub(super) struct BlockStructure { pub root: StructureNode, pub parts: Vec<EditPart> }
impl BlockStructure {
    pub fn from_node(node: &Node, source: &str, offset: usize) -> Option<Self>; // mdast 节点→部件
    pub fn part_at(&self, offset: usize) -> Option<usize>;
    pub fn replace_part(&mut self, part: usize, length: usize);  // 编辑后部件 span 重排
    pub fn table(&self, part: usize) -> Option<(&[Vec<usize>], &Range<usize>)>;
    pub fn list_item(&self, part: usize) -> Option<&ListItem>;
}
pub(super) struct InlineRun { /* 同格式/同对象内联段;嵌入对象切分 run */ }
pub(super) fn contains_object(node: &Node) -> bool;
pub(super) fn build(...) -> StructureNode;   // 病理块回退字面量 + 单输入有界

// structure_commands.rs —— 结构编辑 = 普通编辑,共享文档事务/撤销/保存(spec 采纳点 10)
pub(super) fn table_cell_source(source: &str) -> String;   // 表格单元格→源文本往返
fn commit_structure_edit(&mut self, range: Range<usize>, replacement: &str,
                         cursor: Option<usize>, window, cx); // 全部结构化命令的统一落点
fn toggle_list_check(&mut self, ...); fn navigate_table(&mut self, ...) -> bool;
fn continue_list(&mut self, ...) -> bool;  fn indent_list(&mut self, ..., outdent: bool);

// input_rules.rs —— 已提交打字/粘贴引入结构(spec 采纳点 11)
fn block_prefix(text: &str) -> bool;        // `- `/`# `/围栏/分割线/`$$` 前缀判定
fn enter_block(text: &str) -> Option<(String, usize)>; // 前缀→块源 + 光标位
fn apply_input_rule(&mut self, ...);        // IME 组合更新不触发
fn insert_block_on_enter(&mut self, ...);
```

### 图片压帧与缓存(照抄锚点,spec 采纳点 13)

```rust
// images.rs
pub(super) fn rewrite_doc_images(text: &str, base: Option<&Path>) -> String;   // 文档级图片 URL 重写
fn flatten_local_image_url(url: &str, base: Option<&Path>) -> String;
// 本地 GIF/动图 WebP → 压单帧 PNG 进缓存;http(s)/data: 原样透传;静态 PNG/JPEG 不重读
// 因果链:不压则 gpui 播 GIF 且多图共用 element id 越界 panic(压帧是正确性措施非优化)

// image_cache.rs
const IMAGE_BYTES: usize = 64 * 1024 * 1024;    // 文档解码图 LRU 字节上限
const IMAGE_ENTRIES: usize = 128;               // 条目上限
pub(super) struct DocumentImageCache { /* generation 守卫 + ready/pending 双相 */ }
// 逐出同时释放像素缓冲与图集条目;非活跃文档 set_active(false)(activity.rs)
```

### chrome / 几何(照抄;数值为 pebrel 现值,实现期按 slTerminal 视觉重定——spec 采纳点 14)

```rust
// reader_presentation.rs —— 阅读器几何常量族单点,消费方零字面量
pub(super) const PAGE_WIDTH: f32 = 720.0;
pub(super) const OUTLINE_WIDTH: f32 = 248.0;
pub(super) const DETAILS_MIN_WIDTH: f32 = 216.0;
pub(super) const DETAILS_MAX_WIDTH: f32 = 360.0;
pub(super) const DETAILS_RESIZE_HIT_WIDTH: f32 = 8.0;
pub(super) fn clamp_details_width(width: f32) -> f32;
pub(super) fn heading_size(depth: u8) -> f32;

// chrome.rs / details.rs / info.rs
fn render_toolbar(...) -> impl IntoElement;   // 面包屑工具条
pub(in crate::gpui_shell) struct DocumentDetails { section: DocumentSection, /* 宽度拖拽钳制 */ }
pub(in crate::gpui_shell) enum DocumentSection { Outline, Info }
fn render_info_content(...) -> AnyElement;    // 字符/行数/编码/行尾/修改时间统计
```

### tab 容器与路由(照抄锚点)

```rust
// code_tab.rs
pub struct CodeTabView {
    pub path: PathBuf, pub title: String,
    input: Entity<InputState>,                 // 合并结果中栏;普通文件缓冲区由 TextFileView 持有
    merge: Option<MergeEditor>,
    file: Option<Entity<TextFileView>>,        // 普通文件 = 可保存文本编辑器(共享编辑内核)
    notice: Option<String>, lines: usize,
}
impl CodeTabView {
    pub fn new(path: PathBuf, window, cx) -> Self;
    pub fn new_git_merge(location: GitLocation, relative_path: &str, window, cx) -> Self; // 三栏合并
    pub fn matches_git_merge(&self, location: &GitLocation, relative_path: &str) -> bool; // 冲突重开命中
    pub fn reload(&mut self, ...); pub fn reload_git_merge(&mut self, ...);
}
struct MergeEditor { key: MergeKey, ours: Entity<InputState>, theirs: Entity<InputState>,
                     ours_text: String, theirs_text: String,
                     ours_missing: bool, theirs_missing: bool, state: MergeState }
// 三栏:左 ours / 中可编辑结果 / 右 theirs,外侧只读;应用 = 写回工作树 + git add(spec 采纳点 16)
// git 侧载一律 --no-optional-locks(code_tab.rs 内两处 git 调用照抄)

pub fn viewable_file(path: &Path) -> bool;   // 扩展名→tree-sitter 语言映射表判定
fn language_for_extension(extension: &str) -> Option<&'static str>; // 全量映射表单点

// doc_tabs.rs —— 双击路由单判定(spec 采纳点 19)
pub fn openable_in_app(path: &Path) -> bool {
    image_viewer::viewable_file(path) || markdown_view::viewable_file(path) || code_tab::viewable_file(path)
}
// slTerminal fileViewers 注册表值集并入本判定集(策略链机制不采纳,spec 采纳点 38)

// doc_tabs.rs ImageTabView + display/image_viewer.rs ImageView(spec 采纳点 18)
pub struct ImageTabView { /* path + geometry */ }
impl ImageTabView { pub fn new(path: PathBuf, cx) -> Self; pub fn reload(&mut self, cx); fn spawn_decode(&mut self, cx); }
pub struct ImageView { /* zoom/pan 几何状态机,绘制不依赖上一帧 bounds(当帧 area 换算,采纳点 20) */ }
impl ImageView {
    pub fn open(path: PathBuf) -> Self; pub fn reload(&mut self);
    pub fn zoom_by(&mut self, steps: f32, anchor: (f32, f32), area: (f32,f32,f32,f32)) -> bool;
    pub fn begin_drag(&mut self, point: (f32, f32), area: (f32,f32,f32,f32)) -> bool;
    pub fn drag_to(&mut self, point: (f32, f32), area: (f32,f32,f32,f32)) -> bool;
    pub fn render_rect(&self, area: (f32,f32,f32,f32)) -> (f32, f32, f32, f32);
}
// 缩放契约值(照抄):MIN_ZOOM 0.25× / MAX_ZOOM 8.0× / ZOOM_PER_STEP 1.18;后台 BGRA 解码;重开同路径 reload
```

### 文件树模型与渲染(照抄锚点)

```rust
// side_panel/mod.rs —— 模型/渲染分层:模型随 fs 通知存活,渲染只按行号取(spec 采纳点 21)
pub struct FileRow {
    pub path: PathBuf,
    pub name: String, pub depth: usize, pub is_dir: bool, pub expanded: bool,
    pub is_parent: bool,      // 合成 `..` 导航行,禁与真实条目混判
    pub ignored: bool,        // gitignore 命中:留在树序,降色渲染
    // guest_path 随 WSL 砍
}
pub struct GitInfo { pub vcs: VcsKind, pub branch: String, pub plus: u64, pub minus: u64,
                     pub staged: ..., pub files: ... }   // git status 快照;SVN 臂砍
pub struct SidePanel {
    pub open: bool, pub view: PanelView, pub git_view: GitPanelView,
    root: Option<PathBuf>, followed_cwd: Option<PathBuf>, custom_root: Option<PathBuf>,
    rows: Vec<FileRow>,            // 平铺可见行(渲染唯一数据源)
    tree_rows: Vec<FileRow>,       // 未过滤快照;清搜索即恢复,不重新走盘
    expanded: HashSet<PathBuf>,    // 展开态跨刷新存活
    git: Option<Arc<GitInfo>>,
    scroll: usize, search: String, search_focus: bool,
    // 后台快照 worker:needs_refresh 重臂 + generation 防串台;文件名索引随快照存活
}
pub fn file_rows(&self) -> &[FileRow];   // 渲染分层边界:渲染只读本切片
pub fn drop_text_for_path(path: &str) -> Option<Vec<u8>>;      // eg 02 篇消费(spec 采纳点 43 交界)
pub fn drop_text_for_paths(paths: &[String], shell: PathQuote) -> Option<String>;

// side_panel/{enumerate,gitignore,search,vcs}.rs 公开面(照抄,裁剪已注)
enumerate: 目录枚举 + 过滤索引(WSL find 族砍);gitignore: git_repository_root/gitignore_entry
           + `--no-optional-locks check-ignore` 过滤索引;search: FileSearchOptions 流式文件名索引;
vcs: git status 快照(子进程,--no-optional-locks;SVN/tortoise 臂砍)

// gpui_shell/workspace/file_tree.rs —— 渲染与交互(spec 采纳点 21/23)
pub(super) struct FileTreeContextMenu { /* 挂 workspace 根:不受抽屉裁剪与推出动画影响 */ }
fn render_file_tree_row(&self, visible_ix: usize, cx) -> AnyElement; // 行渲染只按行号取模型
fn on_file_tree_search_event(&mut self, ...);
// 行几何水洗形态(参考,实现期重定——采纳点 22):ROW_PITCH/ROW_WASH_H/ROW_WASH_INSET/DRAWER_TEXT_INSET

// file_tree/path_bar.rs —— 路径栏导航(spec 采纳点 24)
pub(in crate::gpui_shell::workspace) struct PathEditor { /* 只拥有草稿;导航经文件树模型单入口 */ }
fn resolve_directory(origin: &..., text: &str) -> Option<PathBuf>; // `~` 展开 + 规范化
fn display_path(path: &Path) -> String;   // UNC \\?\ 前缀规范化显示(WSL guest 分支砍)

// display/file_operations.rs + file_dialog.rs(仅 Windows 分支,spec 采纳点 25/26)
pub(crate) fn send_to_recycle_bin(path: &Path) -> Result<(), String>; // SHFileOperationW + FOF_ALLOWUNDO
struct FileFilter { name: &str, extensions: &[&str], patterns: &[&str] }  // 过滤器族单点;私钥过滤器随 SSH 砍

// gpui_shell/file_drop.rs(spec 采纳点 27)
pub struct FileTreeDrag { /* 载荷纯数据:宿主路径 + 写入 PTY 的路径原文 + 名称 */ }
pub struct FileDragGhost;  // 拖动预览独立实体(源行可能已因树刷新消失)

// text_preview.rs(参考,spec 采纳点 28)
pub(crate) fn multiline_source(source: &str) -> Cow<str>;   // 32 行/4KB 双上限
pub(crate) fn single_line_label(source: &str) -> Cow<str>;  // 单行表面不喂换行给单行 API
```

### 键位(照抄语义,JSON 化载体——spec 采纳点 29–32)

```rust
// settings.json keybind 段(eg 06 篇登记结构;D06-1 段形态终裁):
//   "keybind": [ { "combo": "ctrl+shift+c", "action": "copy" }, … ]
// 行序即优先级;整表替换 = 删旧数组按序追加,其余内容原样保留(persist_keybinds 语义归 06 写通道)

// slterm_app 侧消费形态(语义照抄 nebula_settings keybind_pairs 族)
pub struct KeybindEntry { pub combo: String, pub action: String }
pub fn keybind_pairs() -> Vec<(String, String)>;            // JSON 段 → 对表
pub fn keybind_pairs_from_json(value: &serde_json::Value) -> Vec<(String, String)>; // 纯函数,单测锁定
pub fn persist_keybinds(pairs: &[(String, String)]) -> std::io::Result<()>;   // 整表替换归 06 写通道
pub fn apply_keybinds(existing: &serde_json::Value, pairs: &[(String, String)]) -> serde_json::Value;

// gpui_shell/workspace/keyboard_bindings.rs(spec 采纳点 30)
pub(super) const STATIC_DEFAULT_COMBOS: &[&str];  // 静态默认镜像表:区分「恢复默认」与「Unbind 精确收回」
pub(super) fn gpui_binding_combo(combo: &str) -> String; // 存储格式 → GPUI 绑定串规范化
// (大小写/修饰键序/digitN/plus-minus 转义;解析归 gpui::Keystroke)

// display/keymap.rs(spec 采纳点 31)
pub(crate) const EDITABLE_ACTIONS: &[(Action, &str, &str)]; // 可重绑动作集中常量化,顺序=展示顺序
pub(crate) fn editable_row_count() -> usize;
pub(crate) fn display_stored_combo(combo: &str) -> String;
// 数字系动作与 AI 贴入键只读展示;slTerminal COMMAND_CATALOG 两表实现期对账合并
```

### 新建项签名(git gutter / 通用 diff,改造节 3/4 详述)

```rust
// slterm_app/src/gpui_shell/file_editor/gutter.rs(新建)
pub enum GutterMark { Added, Modified, Deleted, Renamed, Untracked, Conflicted } // 值集与 vcs 子进程状态值集对账(D07-1 已裁子进程统一,实现期收敛归一)
pub struct GitGutter { marks: Vec<(u32, GutterMark)>, revision: u64 } // 按行号升序
impl TextFileView { fn sync_git_gutter(&mut self, file_status: FileStatus, cx); }

// slterm_app/src/gpui_shell/diff_tab.rs(新建)
pub enum DiffSource { HeadToWorktree { path: PathBuf }, BlobPair { left: DiffBlob, right: DiffBlob } }
pub struct DiffHunk { pub old_start: u32, pub old_lines: u32, pub new_start: u32, pub new_lines: u32 }
pub struct DiffTabView {
    left: Entity<InputState>,   // 只读(decode read_only=true 语义)
    right: Entity<InputState>,  // 只读
    hunks: Vec<DiffHunk>,
    active_hunk: Option<usize>,
}
```

## 数据流与状态机

### 打开路由(双击 → tab)

```
file_tree 双击 / openable_in_app 调用
  → openable_in_app(path) 三判定短路:图片 → markdown → 可映射源码
  ├─ 已开同路径 tab? → 激活重开臂(图片 reload / 编辑器保留草稿与撤销历史,spec 采纳点 1)
  ├─ ImageTabView::new(后台 BGRA 解码)
  ├─ markdown 且 ≤512KB → WorkspaceTab::Document(TextFileView 渲染态打开)
  ├─ language_for_extension 命中 → WorkspaceTab::Code(TextFileView 源码态)
  │    (git 冲突路径归 CodeTabView::new_git_merge,见下)
  └─ 未命中 → ShellExecute 交系统处理器(spec 采纳点 19)
```

路由判定值集(扩展名→打开方式)是 fileViewers 注册表的产品等价物:实现期与 slTerminal 旧注册表值集对账合并,**单判定函数单源**,策略链机制不采纳(spec 采纳点 38)。

### 编辑器生命周期状态机(dirty/saving/loading 三态合同,spec 采纳点 1)

```
loading →(Document::load + Outline::prepare 完成)→ clean
clean --编辑(record/record_rope)→ dirty(tab 标题「•」)
dirty --Ctrl+S / SaveFile--> saving ─┬─ verify 通过 → 写盘 → clean(快照换新)
                                     ├─ SaveError::Changed → notice「磁盘已变」+ 保持 dirty
                                     ├─ SaveError::ReadOnly → notice 只读 + 保持 dirty
                                     └─ Io → notice 写盘失败(失败可感知,eg 06 三纪律同形)
saving 在途(operation.is_some())→ 再触发 save 为 no-op;外部 reload 请求同理守卫
```

### 保存链(快照比对单点)

```
TextFileView::save
  → Document::save(path, text)          // 后台 Task
    → TextSnapshot::verify(磁盘现字节)   // 与载入快照逐字节比对
      ├─ 不等 → SaveError::Changed(拒绝覆盖,用户显式确认后走 request_reload → 重载 → 再编辑 → 存)
      ├─ read_only → SaveError::ReadOnly
      └─ 通过 → TextSnapshot::encode(按 bom/crlf 元数据回写)→ 原子写
```

### 外部修改流(Rust 直连,无 IPC/轮询——spec 优化方向)

```
fs notify(文档所在目录 watcher,归 02/09 notify 资产移植归位)
  → 磁盘 mtime/字节 ≠ 载入快照
  ├─ !dirty → 静默 request_reload(保留滚动/大纲展开态)
  └─ dirty  → 确认弹窗:丢弃草稿重载 / 保留草稿(继续编辑,下次保存走 Changed 分支)
```

### 双模态与 live_edit 状态机

```
preview(渲染态,markdown 默认)
  --ToggleSource--> 源码态(全文 InputState,编辑即普通文本)
  --预览块聚焦--> live_edit:聚焦块换稳定原生输入(LiveEdit),
                 Projection 双向映射,格式标记留源码(格式经 decorations 投影可见)
  --FinishBlockEdit / 失焦--> 提交 → commit_structure_edit 等价事务 → 重算 Outline → schedule_preview
  --IME 组合期--> 不重排输入实体(Projection::accept 守卫)
```

### 统一撤销历史流

```
一切编辑(源码键入/粘贴/live_edit 提交/结构化命令/输入规则转换)
  → EditHistory::record(_rope)   // rope 差量,8MB 上限
  → barrier 划分撤销组(连续输入合并为一项)
Ctrl+Z / Ctrl+Y → travel(false/true) → (全文, 变更 span) → 回写输入 + Outline 重算
输入实体寿命不决定可撤销性——渲染态提交与源码态键入同栈(spec 采纳点 4 语义)
```

### 预览虚拟化与资产释放(照抄因果:note 2026-09-19-render-resource-lifetimes)

```
apply_outline → schedule_preview(250ms 防抖,preview_task 在途即取消重排)
  → blocks 槽按 ListState 视口物化(屏外块 None,不物化 DOM/元素)
tab 切走(set_render_active(false))
  → 清内联视图/图片激活/滚动状态;DocumentImageCache::set_active(false)
  → 草稿与 EditHistory 存活(重开同路径恢复现场)
逐出:LRU 满 → 同时释放像素缓冲与图集条目(虚拟行不得污染进程级常驻资产图)
```

### 文件树快照流(模型/渲染分层,spec 采纳点 21)

```
聚焦 pane cwd 变化 / 树开合 / fs notify / 显式刷新 / gitignore 变更
  → needs_refresh 重臂 → 后台快照 worker(generation 守卫防串台)
    → enumerate 目录枚举 + gitignore 过滤索引(--no-optional-locks check-ignore)
    → vcs git status 快照(非 work tree 内则 git=None)
    → 平铺 FileRow(展开集存活;ignored 降色位;合成 `..` 行)
  → sync:generation 匹配才落 rows/tree_rows;期间新请求再武装下一轮
搜索:非空 query → 文件名索引流式过滤,rows 换平铺结果集;清空 → tree_rows 立即恢复(不走盘)
渲染:file_rows() 切片 → render_file_tree_row(visible_ix) 按行号取(模型渲染零穿透)
```

### 三栏合并状态机(spec 采纳点 16)

```
new_git_merge(location, relative_path)
  → 后台取 ours/theirs(--no-optional-locks;缺失侧 ours_missing/theirs_missing 守卫)
  → MergeState 机:载入 → 编辑中栏(可编辑 InputState,外侧只读)→ apply
apply → 中栏结果写回工作树 + git add → tab 转普通 CodeTabView(后续走常规保存链)
matches_git_merge 命中 → 重开冲突不另开浮窗,命中既有 tab
```

### 键位加载流(spec 采纳点 29/30 语义,JSON 化)

```
启动 / settings.json keybind 段变更(归 06 写通道通知链)
  → keybind_pairs()(JSON → Vec<(combo, action)>,行序即优先级)
  → gpui_binding_combo 逐条规范化 → gpui::KeyBinding::new(combo, action, Some("FileEditor"/上下文))
  → bind_keys 全量重绑(整表替换:删旧按序追加;STATIC_DEFAULT_COMBOS 镜像供「恢复默认/Unbind 收回」)
保留键红线:Ctrl+C 中断透传(终端上下文)与 Ctrl+Shift+C 复制归保留键保护,不可被用户表覆盖
  (slTerminal shortcuts reserved 四纪律归 32 照抄关系裁定归位归 06 注册表登记归 07 动作面)
```

### git gutter 数据流(新建,改造节 3)

```
文档打开/保存/外部 reload 完成 + repo 状态刷新(side_panel git 快照或 git 模块状态图,同一子进程执行体,D07-1 已裁)
  → 取本文件 status → GutterMark 映射(未跟踪/新增/修改/删除/重命名/冲突)
  → modified 粒度细化:diff hunks 首行号(可选路径,D07-2 关联;文件级为零成本基线)
  → GitGutter{marks 按行升序} → revision bump → 编辑器行条重渲(色取 06 语义槽)
点击 mark → 跳转首变更行 / 打开 DiffTabView(HEAD↔工作区)
```

### 通用 diff 数据流(新建,改造节 4)

```
DiffSource::HeadToWorktree{path}
  → git_file_at_head(HEAD 字节,只读)‖ 工作区现字节(read_prefix 有界)
  → compute_diff_hunks → DiffHunk 集
  → DiffTabView 双只读栏 + hunk 条导航(active_hunk 跳转 + 双栏同步滚动)
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;每条对应 spec 分片 07 一个采纳点,一行)

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 1 | `gpui_shell/file_editor/mod.rs` `TextFileView`/`new_with_source`/`save`/`request_reload`/`toggle_preview`/`tab_title` | `slterm_app::gpui_shell::file_editor` 全量 | 双模态+三态合同+脏标记+草稿保留全套产品语义,替代 CM6 编辑器面板(采纳点 33) |
| 2 | `gpui_shell/file_editor/document.rs` `Document`/`Document::load`/`Document::save`/`SaveError`(re-export `text_document::{MAX_BYTES, SaveError}`) | 同上全量 | 保存前快照比对+只读禁写是防覆盖/防只读破坏的合同,旧栈保存链路无此严格度 |
| 3 | `nebula_app/src/document_io.rs` `read_prefix` | `slterm_app/src/document_io.rs` | 「大文件读多少」合同单点:内存上限先于解码生效 |
| 4 | `file_editor/edit_history.rs` `EditHistory`/`record`/`record_rope`/`changed_rope_span` | 全量 | 跨表示统一撤销(输入实体寿命不决定可撤销性),CM6 分立历史无对应物 |
| 5 | `file_editor/outline.rs` `Outline`/`Outline::prepare`/`edit_block_at`/`visible_headings` | 全量 | 大纲/块/源码 span/结构四索引同一份 AST,「大纲即渲染块清单」 |
| 6 | `file_editor/preview.rs` `render_markdown_preview`/`activity.rs` `set_render_active`/`mod.rs` `apply_outline`/`schedule_preview` | 全量 | ListState 虚拟化+250ms 防抖+切走释放资产但草稿历史存活——恒挂载白名单问题的原生答案 |
| 7 | `file_editor/live_edit.rs` `LiveEdit`/`decorations`/`inline_edit.rs` `Projection`/`Marks`/`changed_span` | 全量 | WYSIWYG 投影内核:聚焦块原生输入、格式标记留源码、IME 稳定——本片核心资产 |
| 8 | `live_navigation.rs` `move_live_edge`/`live_selection.rs` `sync_live_selection`/`inline_selection.rs` `inline_selected_text` | 全量 | 跨输入边界导航+行内源码跟随+段落共享复制,WYSIWYG 交互完成度三件套 |
| 9 | `block_structure.rs` `StructureNode`/`EditPart`/`PartKind`/`block_inline.rs` `InlineRun`/`contains_object`/`build` | 全量 | 容器拥有布局、输入只改内容 span;病理块回退字面量+单输入有界 |
| 10 | `structure_view.rs` `render_structured_block`/`structure_inline_view.rs` `render_inline_parts`/`structure_commands.rs` `commit_structure_edit`/`table_cell_source` | 全量 | 结构编辑=普通编辑的统一事务语义,共享撤销/保存 |
| 11 | `input_rules.rs` `block_prefix`/`enter_block` | 全量 | 已提交打字引入结构、IME 组合不触发;数学编辑结构归本片,渲染归 08(交界 42) |
| 12 | `code_actions.rs` `CodeLanguage`/`CodeSpec` | 全量(亮角色表砍,边界条 9) | 块级语言选择+复制动作+复制反馈 |
| 13 | `images.rs` `rewrite_doc_images`/`flatten_local_image_url`/`image_cache.rs` `DocumentImageCache` | 全量 | 压帧是正确性措施(共用 element id panic 链),文档级 LRU 64MB/128 项+逐出双释放是缓存纪律 |
| 14 | `reader_presentation.rs` `PAGE_WIDTH`/`OUTLINE_WIDTH`/`DETAILS_MIN_WIDTH`/`clamp_details_width`/`heading_size` | 数值实现期重定 | 几何常量族单点,消费方零字面量 |
| 15 | `chrome.rs` `render_toolbar`/`details.rs` `DocumentDetails`/`DocumentSection`/`info.rs` `render_info_content` | 全量(几何参考 14) | 工具条/大纲信息双节侧栏/详情宽度钳制/文件统计产品语义 |
| 16 | `gpui_shell/code_tab.rs` `CodeTabView`/`new_git_merge`/`MergeEditor`/`matches_git_merge` | 全量 | 普通文件可保存编辑器+冲突三栏合并写回工作树+git add,替代 diff 面板冲突面(36) |
| 17 | `code_tab.rs` `language_for_extension`/`viewable_file`/`display/image_viewer.rs` `viewable_file` | 值集实现期对账 | 扩展名→语言映射表单点;fileViewers 值集并入(38) |
| 18 | `gpui_shell/doc_tabs.rs` `ImageTabView`/`spawn_decode`/`reload`/`image_viewer.rs` `ImageView`/`zoom_by`/`begin_drag` | 全量 | 0.25×–8×/步进 1.18/锚点缩放/拖拽平移/后台解码/重开 reload 产品语义 |
| 19 | `doc_tabs.rs` `openable_in_app` | 全量 | 双击路由单判定 = fileViewers 注册表的产品等价物(机制不采纳) |
| 20 | `doc_tabs.rs` `area_tuple`/`on_scroll`/`on_mouse_down` | 参考 | 事件换算用当帧 area,绘制不依赖上一帧 bounds |
| 21 | `display/side_panel/mod.rs` `file_rows`/`PanelView`/`workspace/file_tree.rs` `render_file_tree_row`/`on_file_tree_search_event`/`enumerate.rs`/`gitignore.rs`/`vcs.rs`/`search.rs` | `slterm_app/src/side_panel/` 改名脱壳 | 模型渲染完全分层+行平铺,useFileTree 全部前端状态的 Rust 模型化落点(37) |
| 22 | `workspace/file_tree.rs` `ROW_PITCH`/`ROW_WASH_H`/`ROW_WASH_INSET`/`DRAWER_TEXT_INSET` | 参考,数值重定 | 行缝来自水洗矮于行距的连续列表读感 |
| 23 | `workspace/file_tree.rs` `FileTreeContextMenu` | 全量 | 菜单挂 workspace 根,不受抽屉裁剪/推出动画影响 |
| 24 | `workspace/file_tree/path_bar.rs` `PathEditor`/`resolve_directory`/`display_path` | 全量(WSL 分支砍) | 路径框只拥有草稿、导航单入口、`~` 展开、UNC 规范化;slTerminal 无路径栏,新增能力 |
| 25 | `display/file_operations.rs` `send_to_recycle_bin` | 全量(非 Windows 回退砍) | SHFileOperationW+FOF_ALLOWUNDO,删除对齐系统回收站 |
| 26 | `display/file_dialog.rs` `FileFilter` 过滤器族 | 仅 Windows 分支(私钥过滤器砍) | 系统打开/保存对话框+过滤器单点;新增能力 |
| 27 | `gpui_shell/file_drop.rs` `FileTreeDrag`/`FileDragGhost` | 全量 | 载荷纯数据+幽灵预览独立实体;写 PTY 规则归 02(交界 43) |
| 28 | `nebula_app/src/text_preview.rs` `multiline_source`/`single_line_label` | 参考 | 单行表面不喂换行给字体单行 API;多行 32 行/4KB 双上限 |
| 29 | `nebula_settings/src/lib.rs` `keybind_pairs`/`keybind_pairs_from_text`/`persist_keybinds`/`apply_keybinds` | 载体 JSON 化(eg 06) | 行序即优先级+整表替换语义照抄,txt 行协议不迁 |
| 30 | `workspace/keyboard_bindings.rs` `STATIC_DEFAULT_COMBOS`/`gpui_binding_combo` | 全量 | 恢复默认与 Unbind 精确收回的镜像表+combo 规范化归 gpui::Keystroke |
| 31 | `display/keymap.rs` `EDITABLE_ACTIONS`/`editable_row_count`/`display_stored_combo` | 全量,两表实现对账 | 可重绑动作集中常量化;COMMAND_CATALOG 同构合并归实现期 |
| 32 | slTerminal `src/features/shortcuts/` 四纪律 ↔ pebrel 键位模型 | 映射归位(机制归 GPUI key_context) | Command/Keybinding 分离+覆盖层+目录单点+保留键保护同构;Ctrl+C/Ctrl+Shift+C 红线映射保留键 |
| 33 | slTerminal `src/panels/editor/`(CM6 栈) | 能力→file_editor+code_tab(消亡清单归改造节 1) | 打开/保存/外部改动/语言/换行/字号逐项落位;repaintGuard 随 WebView2 GLYPH bug 消亡 |
| 34 | slTerminal `src/panels/editor/largeFileViewer/` | 语义重定:截断+truncated 提示 | 分块只读浏览整体消亡;可编辑域 8MB,1MB 警告弹窗不迁移 |
| 35 | slTerminal `src/panels/docViewer/`+`src-tauri/src/preview.rs`+`panels/html`/`markdown` | iframe 沙箱全灭,markdown→双模态;html→源码只读 | 预览协议域/消息桥/注入脚本/缩放滚动运行时无 WebView 对应物;SEC 预览红线防护对象不复存在 |
| 36 | slTerminal `src/panels/gitshow/`+`src/panels/diff/` | gitshow→只读 Code tab;diff→通用 diff 新建(改造节 4) | HEAD 只读=decode read_only 语义;占位对齐/滚动同步技艺随 CM6 消亡 |
| 37 | slTerminal `src/features/explorer/` | 能力并入 file_tree(改造节 6) | CRUD/git 着色/fs-event/展开态保留全数为 Rust 模型行为;FileIcon 六色盘归 06 token |
| 38 | slTerminal `src/features/fileViewers/FileViewerRegistry.ts` | 值集并入 `viewable_file`(19),机制消亡 | 策略链/_reset 注册表契约的 WebView 载体消亡 |
| 39 | slTerminal `src/features/navTree/`+`src/features/agentFiles/` | 挂 workspace 侧栏归 05/03,本片只登记树组件复用 | pebrel 无项目/页面模型,GPUI 重建归对应篇 |
| 40 | slTerminal `src-tauri/src/fs/` 命令层 | 消亡;语义资产并入(改造节 1) | 单进程 std::fs 直连;CRLF 检测并 text_document;分页/分块/沙箱随 IPC 消亡 |
| 41 | slTerminal `src-tauri/src/git/` ↔ pebrel `side_panel/vcs.rs` 纪律 | 模块落位归改造节 2(D07-1 已裁子进程统一) | git2 实测修正(见篇头登记);--no-optional-locks 纪律不可谈判(边界条 7) |
| 42 | `input_rules.rs` `enter_block`/`block_structure.rs` `PartKind::Math`/`structure_view.rs` 数学分支 | 本片编辑结构归本片;渲染归 08 | 本片消费 08 渲染结果,不重复裁定 |
| 43 | `file_drop.rs` 消费点 `drop_text_for_path` | 载荷照抄;写 PTY 规则归 02 | 控制字符拒绝/空白加引号/尾随空格归终端输入分片 |
| 44 | `workspace/quick_jump.rs` `rows` 文件条目投影 | 数据源登记归命令面板篇 | file_tree 是其数据源之一,本片不建 palette |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威,本篇不另立映射。直接消费点:`nebula_app` → `slterm_app`(A 节,file_editor/code_tab/doc_tabs/file_drop/side_panel 全部落其下);`pebrel_settings.txt` 键位行 → `settings.json` `keybind` 段(D 节 + 06 篇 D06-1 段形态终裁);`NEBULA_UNFOCUSED_SPLIT_DIM` 等环境面归对应篇。编辑器域标识符(`TextFileView`/`Outline`/`Projection` 等)无品牌,**= 不变**;`DocumentSource::Remote`/`RemoteLocation` 不迁(SSH 砍);`VcsKind::Svn*` 与 tortoise 臂不迁;`CodeUiColors` 亮角色不迁。

### 缝合点

1. **05 篇(布局)**:本片持有 `WorkspaceTab::Document`/`Image`/`Code` 三变体的渲染分支/关闭语义(纯视图 drop,无进程)/持久化语义(不进 session,05 边界条 6);`TabMeta` 展示字段演化(脏标记「•」/标题派生归 `TabPresentation`,05 开放问题 4 的承接);文件树作为 side view 挂 05 篇侧栏骨架(`SideViewId` 封闭集归 05 篇 D05-2 收敛,本片只供视图内容);单 tab 导出臂与 05 篇 `export_workspace` 共用 session schema;tab 右键菜单归 05 开放问题 2 默认(M8 与本片联定)。
2. **06 篇(设置/主题)**:编辑器配色 token 装配归本片 `editor_theme.rs`,槽值单源在 06(改造节 8);`keybind` JSON 段结构归 06 篇 D06-1 终裁,本片锚定段内数组形态与整表替换语义;编辑器相关设置键(自动换行/字体归 D07-3)经 06 注册表登记;编辑器 UI 文案走 06 篇 i18n typed ID(`Message::Editor*`)。
3. **08 篇(AI 辅助)**:数学块编辑结构(`$$` 入口/part span 切分)归本片;公式编译/栅格化/位图上屏归 08;本片消费其渲染 element,不重复裁定(spec 采纳点 42)。
4. **02 篇(终端)**:文件拖放写 PTY 规则(控制字符/引号/尾随空格归 `drop_text_for_path` 消费点,采纳点 43);终端 pane 是拖放目标实体,落点归属 02 渲染域。
5. **03 篇(AI CLI)**:`send_to_chat` 本地选区引用工作流归 03(spec 不采纳点 9 的本地臂)。
6. **11 篇(测试)**:本片全部测试点的虚拟窗口/夹具/门禁登记归 11;L2/L3 旧用例按类别重生归 11 篇定类。
7. **命令面板篇(44)**:file_tree 作为 quick_jump 文件条目数据源,投影归命令面板分片。

## 改造 / 移植 / 新建设计

### 1. slTerminal 前端编辑器/预览能力消亡映射的落地

| 旧能力(slTerminal 位置) | 新落点 | 落地要点 |
| --- | --- | --- |
| 打开/保存/外部改动检测(`EditorPanel`/`useCodeMirror` 的外部修改三模式) | `TextFileView` + `Document` + fs notify 直连 | 三模式弹窗语义由「是否 dirty」二分承接(数据流节);无轮询心跳、无保存回声抑制链——单进程无 IPC 延迟问题 |
| 语言热切换(Compartment) | `language_for_extension` 映射表 + `code_actions` 语言选择 | 映射值两仓对账合并(开放问题 1);代码块语言选择只影响渲染高亮,不回写源 |
| git gutter(`gitGutter.ts` 状态槽) | **新建 gutter.rs**(改造节 3) | pebrel 无对应物;GPUI 世界编辑器行条自建,语义槽取色归 06 |
| 自动换行 / Ctrl+S 保存 / Ctrl+滚轮字号(`keyboard.ts`) | `actions!(file_editor, …)` + `EDITABLE_ACTIONS` 表 | 进可重绑动作集,combo 默认照 pebrel 键位族;字号设置键归 D07-3 |
| `repaintGuard`(WebView2 GLYPH 防复发) | 消亡 | 防护对象随 WebView 消亡,e2e 防复发锚一并退役 |
| 大文件四层防线 + `LargeFileViewer` | 截断 + truncated 横幅 | `read_prefix` 有界读 + `TextSnapshot.truncated` 只读 + notice;可编辑域 8MB(spec 采纳点 34 已定) |
| docViewer 三态/缩放/滚动运行时 | file_editor 双模态 + `ImageView` 缩放 | markdown edit/split/preview 三态被双模态+详情侧栏覆盖并超出;html = `WorkspaceTab::Code` 只读臂(decode read_only=true) |
| gitshow(HEAD 只读) | `CodeTabView` 只读臂 | 经 `git_file_at_head` 取 HEAD 字节,`decode_prefix(read_only=true)` 载入 |
| diff 面板(HEAD↔工作区双栏) | **新建 DiffTabView**(改造节 4) | 占位对齐/滚动同步 CM6 技艺消亡;新双栏=两只读 InputState + hunk 导航 |
| explorer CRUD/git 着色/增量刷新/展开态 | file_tree 模型行为 | 全数并入 `SidePanel` 模型(照抄点 21 已覆盖);`FileTreeExplorer` 共享树组件 = 树组件 GPUI 复用,宿主=侧栏与后续视图 |
| fileViewers 注册表 | `openable_in_app` 值集 | 策略链机制消亡,值集并入(照抄点 19/38) |

`src-tauri/src/fs/` 语义资产(spec 采纳点 40):CRLF 64KB 样本检测并入 `text_document.rs::decode` 的 `crlf` 判定;keyset 分页/256KB 分块/10MB 上限/沙箱校验全随 IPC 消亡。「项目根外访问」产品语义由 `PathEditor` 任意路径导航替代,越出项目根允许(与 pebrel 一致)。

### 2. git 执行体模块落位(事实修正 + D07-1 已裁:子进程统一)

**事实**:spec 采纳点 41 称「git 子进程封装保留」;实测 slTerminal `src-tauri/src/git/` 为 git2(libgit2 vendored,静态链接,无系统 git 依赖)封装,操作集 = `git_status`/`git_diff`+`compute_diff_hunks`/`git_file_at_head`/`git_rollback`/`git_unstage`,配套决策(LRU 仓库缓存 BE-09、不扫 ignored CP-008、rename 双检、`rollback` 不用 checkout API 的三方字节一致、HEAD 不存在错误契约)在其模块 CLAUDE.md 有完整因果档案。pebrel 侧(`side_panel/vcs.rs`/`code_tab.rs`)为**子进程**封装,`--no-optional-locks` 为每命令 flag。

**落位**(D07-1 已裁 = git 子进程统一,git2 不移植):

- `slterm_app/src/git/` —— 操作集(status/diff hunks/HEAD/rollback/unstage)以 git CLI 子进程重实现,与 pebrel 照抄面同一执行体;`--no-optional-locks` 每命令 flag 全面照抄(边界条 7)。`validate_path_within_root` 沙箱语义消亡,`get_or_open_repo` 的 project_root 校验臂改「无校验单进程直连」。
- pebrel `side_panel/vcs.rs` 照抄进 `slterm_app/src/side_panel/vcs.rs` —— 文件树 git 快照(子进程)随模型照抄落地,与 `src/git/` 共用一套子进程封装,无第二执行体。
- **git2 语义资产转测试保留**:LRU 仓库缓存 BE-09、不扫 ignored CP-008、rename 双检、rollback 三方字节一致、HEAD 不存在错误契约五份因果档案转为行为规格,落成 L1 对照用例与临时仓库夹具(子进程实现须复现同等行为);git2/libgit2 依赖不进 workspace。

git2 依赖剔除的收益面:vendored 静态链接体积与构建时长消亡;两套锁定纪律(git2 面 `GIT_OPTIONAL_LOCKS=0` 等价臂)与两份状态解析(git2 Status vs `git status --porcelain`)的维护面消亡;消费面(file_tree 着色/gutter/diff/merge)自始单一数据源,无接线后跨面返工。

### 3. 新建:git gutter(签名级)

```rust
// slterm_app/src/gpui_shell/file_editor/gutter.rs
pub enum GutterMark { Untracked, Added, Modified, Deleted, Renamed, Conflicted }
// 值集与 file tree vcs 状态值集实现对账(同一子进程执行体,实现期收敛归一)

pub struct GitGutter { marks: Vec<(u32, GutterMark)>, revision: u64 }

impl TextFileView {
    fn sync_git_gutter(&mut self, cx);        // 三触发:文档打开/保存落盘/git 快照刷新(经 06 通知链或 side_panel 快照订阅,子进程执行体单源)
    fn render_git_gutter(&self, cx) -> impl IntoElement; // 行条渲染:色取 editor_theme 槽(green/yellow/red/blue/muted 语义六色),禁硬编码
    fn on_gutter_click(&mut self, line: u32, window, cx); // Modified → 开 DiffTabView::HeadToWorktree;Conflicted → CodeTabView::new_git_merge 命中臂
}
```

粒度基线 = 文件级(status→整文件单标记,hunks 零成本);hunk 首行级为可选增强(diff hunks→首个变更行号),实现期按性能实测定,值集与渲染合同不因粒度变。**只读文档(truncated/gitshow/diff)不挂 gutter**——无编辑域即无行标意义。

### 4. 新建:通用 diff 对照面(签名级)

```rust
// slterm_app/src/gpui_shell/diff_tab.rs
pub enum DiffSource {
    HeadToWorktree { path: PathBuf },                    // gitshow/diff 面板/gutter 点击三入口
    BlobPair { left: DiffBlob, right: DiffBlob },        // 预留:两任意文本源对照(纯内存,不走 git)
}
pub struct DiffBlob { pub label: String, pub bytes: Vec<u8> }  // 经 decode(read_only=true) 有界载入

pub struct DiffHunk { pub old_start: u32, pub old_lines: u32, pub new_start: u32, pub new_lines: u32 }
// 来源:git 模块 compute_diff_hunks(子进程实现,D07-1 已裁)或等价纯函数(两文本直接 Myers)

pub struct DiffTabView {
    left: Entity<InputState>,      // 只读:TextSnapshot read_only 语义,encode 层禁写
    right: Entity<InputState>,     // 只读:同上
    hunks: Vec<DiffHunk>,
    active_hunk: Option<usize>,
    // 渲染:双栏等宽 + hunk 侧条(上一/下一导航,active 跳转双栏同步滚动)
}
impl DiffTabView {
    pub fn open(source: DiffSource, cx) -> Self;   // hunk 计算后台 Task,在途守卫
    pub fn next_hunk(&mut self, cx); pub fn prev_hunk(&mut self, cx);
}
```

形态默认 = **双栏对照**(左 HEAD/右工作区,旧 diff 面板的产品惯性,占位对齐技艺消亡后以行号对等替代);unified 单栏视图为 D07-2 备选。落 `WorkspaceTab::Code` 只读臂(边界条 3 默认,不新增变体)。旧 diff 面板的「工作区侧可编辑」能力消亡——合并/回滚动作由 gutter 菜单与三栏合并器承接。

### 5. gitshow 只读臂落地

`git_file_at_head`(git 子进程,D07-1 已裁)→ HEAD 字节;`decode_prefix(bytes, read_only=true, truncated)` → `CodeTabView` 只读渲染(标题 = `<path> @ HEAD`)。HEAD 不存在错误契约(UnbornBranch/不在 HEAD tree,消息含「HEAD 中不存在」)随迁,UI 占位文案归 06 i18n。

### 6. file_tree:explorer 能力并入 + ignored 语义调和

- **并入清单**:`useFileTree` 的状态面(展开集/选中/滚动)已有 `SidePanel` 对应字段;CRUD(新建/重命名/删除)挂 `FileTreeContextMenu` 归 05 骨架槽位(菜单内容归本片动作面);键盘导航(上下/展开/回车打开)经 `EDITABLE_ACTIONS` 登记。
- **ignored 调和**(边界条 8):树枚举照 pebrel 全量列目录 + `gitignore.rs` 过滤索引置 `FileRow.ignored`(降色渲染);git 状态快照照 slTerminal 纪律不扫 ignored(CP-008 因果随迁:50K+ 忽略文件数秒 I/O 阻塞史)。即「看得见但无状态色」。
- **虚拟化**:slTerminal 旧虚拟化行 = pebrel `ListState` 行渲染,能力等价无缺口;`fs-event 增量刷新` = side_panel 后台快照 worker 的 `needs_refresh` 流,语义保留。

### 7. path_bar / 回收站 / 拖放新增能力

三者均为 slTerminal 旧栈没有的能力,照抄落位零改造:`PathEditor`(草稿/单入口导航/`~`/`\\?\` 规范化)、`send_to_recycle_bin`(FOF_ALLOWUNDO)、`FileTreeDrag`+`FileDragGhost`(纯数据载荷+独立幽灵)。系统文件对话框(`FileFilter` 族)为编辑器「另存为/打开」的宿主件,SSH 私钥过滤器砍。

### 8. 编辑器主题槽装配(06 篇缝合落地)

```rust
// slterm_app/src/gpui_shell/editor_theme.rs(新建)——消费侧唯一装配点
pub struct EditorThemeTokens {
    pub background: Rgb8,       // ReviewedPalette::code_background()(selected alpha 合成,照抄)
    pub foreground: Rgb8,       // ReviewedPalette.foreground
    pub muted: Rgb8,            // 行号/次要 chrome
    pub accent: Rgb8,           // 光标行/选中边
    pub gutter_added: Rgb8, pub gutter_modified: Rgb8, pub gutter_deleted: Rgb8, // 语义六色 green/yellow/red
    pub diff_added_bg: Rgb8, pub diff_removed_bg: Rgb8,  // 语义六色派生(derive 不合成第二色带归 06 纪律)
    pub link: Rgb8,             // 预览链接 = accent 或语义 blue 槽
}
pub fn editor_theme_tokens(palette: ReviewedPalette, ui: &ThemeUiColors) -> EditorThemeTokens;
```

装配时机 = 06 篇 `Settings::load_with_runtime` 热应用链的订阅方(主题变更 → 重建 tokens → 编辑器/gutter/diff 三消费方重渲)。**纪律**:本片渲染代码只引用 `EditorThemeTokens` 字段,禁裸颜色字面量;消费测试守卫归 11 篇门禁登记。

### 9. 大文件截断落地(可机验面)

`read_prefix(path, MAX_BYTES)` → `truncated=true` → `decode_prefix(.., read_only=true, ..)` → `TextFileView` notice 横幅(`Message::EditorTruncated` 归 06 i18n 键集)+ 只读合同。可编辑域上限 8MB 为契约值(spec 采纳点 34),测试锁定:`MAX_BYTES` 边界文件可开可读不可存。

### 10. 键位 JSON 化落地(spec 采纳点 29–32 总装)

- **段结构**:`settings.json` 顶层 `keybind` 键,值为 `[{"combo": "…", "action": "…"}]` 数组(段嵌套与否归 06 篇 D06-1;本篇锚定段内元素形态与语义)。
- **语义照抄**:数组行序即优先级;`persist_keybinds` = 整表替换(删旧数组按序追加,文件其余段原样保留——归 06 写通道浅合并天然成立);`keybind_pairs_from_json` 宽容解析(元素缺字段/类型不符 → 跳过该行,不炸整表)。
- **规范化**:`gpui_binding_combo`(大小写/修饰键序/digitN/plus-minus 转义)→ `gpui::Keystroke`;`STATIC_DEFAULT_COMBOS` 镜像表支持「恢复默认」与「Unbind 精确收回」两动作的分歧语义。
- **保留键红线**:Ctrl+C(终端中断透传)与 Ctrl+Shift+C(复制,产品定位约束)在 `EDITABLE_ACTIONS` 外保留登记,用户表覆盖尝试被拒绝(notice 告知,eg 06 键集);slTerminal 旧 `reserved.ts` 保留键集与 pebrel 对照合并归实现期对账。
- **EDITABLE_ACTIONS 合并**:slTerminal `COMMAND_CATALOG` 与 pebrel `EDITABLE_ACTIONS` 两表实现对账——编辑器域动作(SaveFile/ToggleSource/换行/字号)进本片表;跨域动作(终端/AI/布局)归对应篇,目录单点归 06 注册表吸收形态。

## 测试点清单

> 引测试一律例名;pebrel 侧承重内嵌用例随迁改名(pebrel `file_editor/*_tests.rs`/`tests.rs` 全量用例名照抄,替换 slTerminal L2/L3 散点用例);虚拟窗口/夹具/门禁归 11 篇。bugfix 防复发:旧 CM6/explorer 行为对照用例按「老代码行为锚」补回归。

| 测试 | 层级 | 机制 |
| --- | --- | --- |
| `TextSnapshot` 往返:BOM/CRLF/Unicode 保留;`verify` Changed/ReadOnly/Io 三分支 | L1 text_document | 迁移 pebrel 内嵌例(`both_backends_preserve_bom_crlf_and_unicode` 等) |
| `decode` 上限:>8MB 截断 + truncated/read_only 联锁;`decode_prefix` 有界前缀仍只读 | L1 | 迁移语义新建(契约值边界) |
| `read_prefix` UTF-8 边界截断(不中缝)、truncated 标志 | L1 document_io | 迁移 |
| `EditHistory`:record/travel 往返、barrier 撤销组、8MB 上限丢弃最旧、record_rope 差分不物化全文 | L1 edit_history | 迁移 pebrel 用例 |
| `Outline` 四索引同源:同 AST 出 headings/blocks/source_ranges/structures;512KB 预览上限/32KB 块上限 | L1 outline | 迁移(mod.rs/outline.rs 内嵌例) |
| `Projection`:源↔可见双向 offset 往返;`changed_span` 差分定位;未改叶子与链接目标逐字节不动;IME 组合期 accept 守卫 | L1 inline_edit | 迁移 pebrel 内嵌例 |
| `toggle_mark` 加粗/斜体往返(bold/italic marker);行内源码显影 `with_reveal` | L1 | 迁移(mod.rs 内嵌例) |
| `BlockStructure`:表格/列表/代码部件 span 切分;`replace_part` 后部件重排;病理块回退字面量+单输入有界(`excessive_objects_have_one_literal_part_and_no_partial_allocation` 等) | L1 block_structure/block_inline | 迁移 |
| `InlineRun`:相同对象不同源 span、嵌套 mark 保留、静帧预览(`identical_objects_have_distinct_source_spans_and_keep_nested_marks` 等) | L1 | 迁移 |
| `input_rules`:`block_prefix` 全族(列表/标题/围栏/分割线/`$$`)完整标记判定;`enter_block` 光标位;IME 组合不触发(`markdown_prefixes_require_complete_markers_and_keep_fence_language` 等) | L1 input_rules | 迁移 |
| 结构化命令:`commit_structure_edit` 共享撤销/保存;`table_cell_source` 往返;`toggle_list_check`/`continue_list`/`indent_list` | L1 structure_commands | 迁移 pebrel structure/focus/live 内嵌例族 |
| `DocumentImageCache`:LRU 64MB/128 项逐出、逐出双释放(像素+图集)、`set_active(false)` 非活跃释放(`ready.used_bytes` 断言族) | L1 image_cache | 迁移 |
| `images`:`rewrite_doc_images` 本地 GIF/WebP 压帧、http(s)/data: 透传、静态 PNG 不重读 | L1 | 迁移 |
| `ImageView` 几何:0.25×–8× 钳制、1.18 步进、锚点缩放、drag_to/render_rect(area 换算不依赖上帧) | L1 image_viewer | 迁移 pebrel 内嵌例 |
| 保存链防复发(旧栈对照):外部修改三模式 → 「dirty 确认/静默 reload」二分;保存撞外部修改 → Changed 拒绝 | L1 | 新建,锚旧 EditorPanel 三模式行为 |
| `openable_in_app` 判定族:图片/markdown/可映射源码三分支 + 未命中交系统 | L1 doc_tabs | 迁移 + 新建(旧 fileViewers 值集并入回归) |
| `viewable_file`/`language_for_extension`:映射值集与 slTerminal 旧注册表对账合并(全键往返) | L1 code_tab | 新建(对账回归) |
| `MergeEditor`:三栏载入(含单侧缺失守卫)、中栏编辑、apply 写回 + git add、matches_git_merge 命中 | L1 code_tab(临时 git 仓库) | 迁移 pebrel code_tab 内嵌例(真实 git 命令,SPAWN_LOCK 串行 eg 02) |
| file_tree 行模型:枚举平铺/展开集跨刷新/合成 `..` 行不与真实条目混判/ignored 降色位 | L1 side_panel | 迁移 pebrel side_panel 内嵌例 |
| gitignore 过滤索引:`check-ignore` 输出解析、仓库根上溯 | L1 gitignore | 迁移 |
| 搜索过滤:非空 query 平铺结果集、清空即恢复 tree_rows 不走盘、文件名索引 generation 防串台 | L1 search/side_panel | 迁移 |
| vcs 快照:git status 解析(branch/plus/minus/staged)、非 work tree 内 git=None、子进程一律 `--no-optional-locks`(grep 断言 eg 11 登记) | L1 vcs | 迁移 |
| `send_to_recycle_bin`:FOF_ALLOWUNDO 标志位(注入封装断言)+ 错误码映射 | L1 file_operations | 迁移语义新建 |
| `PathEditor`:草稿不入导航、resolve `~` 展开、UNC `\\?\` 规范化显示(WSL 分支无对象) | L1 path_bar | 迁移 pebrel 内嵌例 |
| `FileTreeDrag` 载荷纯数据(宿主路径/PTY 原文/名称)+ `FileDragGhost` 独立于源行 | L1 file_drop | 迁移 |
| git gutter 新建:status→GutterMark 映射、marks 行升序、revision bump 触发重渲、gutter 点击开 diff/merge 臂 | L1 gutter(注入状态源) | 新建 |
| 通用 diff 新建:hunks 计算(HEAD↔工作区注入 git 子进程夹具)、双栏只读合同(encode 拒绝)、hunk 导航跳转 | L1 diff_tab | 新建 |
| 键位:`keybind_pairs_from_json` 宽容解析(坏行跳过)/整表替换/行序即优先级/`gpui_binding_combo` 规范化往返/恢复默认 vs Unbind 精确收回/保留键覆盖拒绝 | L1 keys/keyboard_bindings | 迁移 pebrel `keybind_pairs_parse_and_rewrite` 等改写 JSON 载体 |
| 截断横幅:`MAX_BYTES` 边界文件打开 → truncated notice + 只读(encode 拒绝) | L1 file_editor | 新建(契约值边界) |
| 配色单点守卫:本片渲染代码 grep 禁裸颜色字面量(eg 11 门禁登记) | L1 + 门禁归 01/11 | 新建 |
| UI 关键路径:双模态打开 → 渲染态切源码 → 编辑 → 撤销 → 保存 → 脏标记/tab 标题 | L3 虚拟窗口(eg 11) | 新建 |
| UI 关键路径:文件树渲染/搜索/右键菜单/拖放进终端 + 图片 tab 缩放拖拽 | L3 虚拟窗口 | 迁移 pebrel ui 用例语义 |

## 阶段归属与出口标准

本片主体归 **M8 文件与编辑**(00-roadmap:file_editor 双模态/file_tree/tab 路由/path_bar/回收站;git gutter/通用 diff 新建项),依赖 M5(tab 骨架)/M6(设置键/主题槽/写通道)/M2(git 环境);细分步与可机验出口:

**M8.1 tab 路由 + 只读臂点亮**:`openable_in_app` 三判定 + `WorkspaceTab::Code`/`Document`/`Image` 渲染分支 + gitshow 只读臂 + 图片 tab。
出口 = 路由判定族用例全绿;只读合同用例绿(encode 拒绝);`cargo check`/`cargo test` 全绿;禁名门禁过。

**M8.2 编辑器内核**:text_document/document_io/Document/EditHistory/Outline/live_edit 全家/inline 投影/块结构/输入规则,源码态可编辑可存。
出口 = 内核用例全绿(撤销/outline/投影/块事务/输入规则/图片缓存);保存链 Changed/ReadOnly/Io 用例绿;`cargo test` 全绿。

**M8.3 预览与 chrome**:双模态渲染态 + 虚拟化 + 详情侧栏 + 工具条 + 几何常量族 + 图片 tab 缩放。
出口 = 虚拟化/防抖/资产释放用例绿;L3 双模态关键路径过;`cargo test` 全绿。

**M8.4 file_tree + path_bar + 回收站 + 拖放**:side_panel 模型族 + file_tree 渲染 + PathEditor + send_to_recycle_bin + FileTreeDrag。
出口 = 行模型/gitignore/搜索/vcs 快照用例绿;explorer 并入行为回归(CRUD/展开态/增量刷新)绿;L3 文件树关键路径过。

**M8.5 新建项 + 键位收口**:gutter.rs/DiffTabView/editor_theme.rs/keybind JSON 段/EDITABLE_ACTIONS 合并/保留键红线。
出口 = gutter/diff 新建用例绿;键位 JSON 化用例全绿;截断横幅用例绿;L3 UI 关键路径过;`cargo check`/`cargo test`/门禁三件套全绿——对齐 00-roadmap M8 出口(编辑器内核测试(撤销/outline/虚拟化)全绿;UI 测试过)。

绿灯语义:M8.1–M8.4 期间 gutter/diff 缺席是过渡期预期态(00-roadmap 逐领域点亮,不设功能兜底);D07-2 须在 M8.5 前收敛,逾期按默认(节 4 双栏形态)执行并登记(D07-1 已裁子进程统一,改造节 2)。

## 待沉淀决策

> [待沉淀] **D07-1 · git 执行体统一:git2 移植 vs pebrel 子进程照抄 vs 双轨并存终态化**。真实权衡:git2 = 类型化 API + slTerminal 既有投资(LRU 仓库缓存 BE-09、rename 合并语义、rollback 三方字节一致、HEAD 错误契约,因果档案完整)且 vendored 静态链接无系统 git 依赖;子进程 = pebrel 照抄面(file_tree vcs/code_tab merge)零改造、`--no-optional-locks` 按命令粒度、文本协议跨版本稳;双轨并存 = 照抄落地最快的既成事实,代价是两套锁定纪律(子进程 flag + `GIT_OPTIONAL_LOCKS=0`)与两份状态解析(git2 Status vs `git status --porcelain`)的永久维护面。意外因素:spec 采纳点 41 称「git 子进程封装」与实测 git2 不符(篇头事实修正),「照抄并入」的原始裁定基于错误前提,双轨是修正后的新默认而非原裁定。难逆点:gutter/diff/树着色/合并四处消费面接线后,执行体合并 = 跨面返工 + 行为差异(status 值集/hunk 语义)重测。截止:M8.1 前定(默认:双轨并存 + facade 收敛消费面,统一归一留待执行体差异实证后评审)。

> [待沉淀] **D07-2 · 通用 diff 对照形态:双栏对照 vs unified 单栏**。真实权衡:双栏 = 旧 diff 面板产品惯性、左右语义对等清晰(HEAD↔工作区天然有向),代价是窄屏信息密度与 hunk 对齐实现成本;unified = 单流可读性高、git diff 文本直觉一致,代价是左右对照的结构性弱化、与旧面板心智断裂。意外因素:旧面板的占位对齐/滚动同步技艺随 CM6 消亡,两形态都无法继承旧实现,选择自由度大于常规「照抄 or 新建」。难逆点:DiffTabView 的 InputState 布局与导航合同落地后,形态切换 = UI 重写 + L3 用例重写;gutter 点击/命令面板入口的返回值形态随之定死。截止:M8.5 前定(默认:双栏,改造节 4)。

> [待沉淀] **D07-3 · 编辑器字体/字号键域:与终端共享 font 键 vs 独立 editor 键族**。真实权衡:共享 = pebrel 单键形态照抄、设置面最简、跨 surface 一致;独立 = slTerminal 旧 `fontSize` 段的 `{terminal, editor}` 双值产品面保留(旧用户存在编辑器字号 ≠ 终端字号的配置习惯),代价是 06 注册表新增键族 + 迁移映射表扩面(D06 旧八键迁移的 fontSize 段归并路径随之分叉)。意外因素:06 篇 D06-1 键形态(平铺 vs 段嵌套)未定,独立键族的落盘形态依赖其终裁;Ctrl+滚轮字号缩放的设置持久化键归属随之分叉。难逆点:键域发布后 = 数据义务(pebrel 迁移链前车之鉴),合并/拆分都是迁移。截止:M6.1 键域冻结前与 06 篇联定(默认:共享 font_family/font_size 单键,旧 editor 值迁移时一次性并入并登记映射)。

## 开放问题

1. **扩展名归属冲突的对账细则**:html/jsx/svg 等同时被「源码可映射」与「系统处理器」声称的扩展名,`viewable_file` 值集与 slTerminal 旧 fileViewers 注册表对账时由谁让位(旧值集锚 or pebrel 映射表锚)归实现期对账清单定;图片格式判定两表已一致,冲突集中在文本类。
2. **gutter 值集与 vcs 状态值集的收敛时点**:`GutterMark` 六值与 file_tree 状态值集在 D07-1 双轨期允许各自演化,统一归一时点 = D07-1 收敛日;期间两表差异在实现对账清单登记,防语义漂移。
3. **编辑器右键/选区菜单的命令归属**:`SelectionContextMenuRequested` 的菜单项(复制/剪切/粘贴/在终端打开/send_to_chat)归本片动作面还是 04 篇命令目录单点,归 M8.5 与 04 联调按改动面最小定(两处测试锚等价归 06 开放问题 4 同法)。
4. **文件图标资源形态**:slTerminal `FileIcon` 六色盘归 06 token 体系已登记,图标载体(GPUI svg 内嵌/字体图标/位图)pebrel 侧为旧壳 icons.rs(不迁),新载体选型归 M8.4 实现期,本片锚「色归 token、形归实现」。
5. **预览代码块语法高亮来源**:gpui-component `MarkdownExtensions` 的高亮能力边界与 tree-sitter 映射表的接合形态(pebrel `code_actions.rs` 语言值是高亮选择而非解析器注册)归 M8.3 实现期验证;若 gpui-component 高亮不覆盖映射值集,降档方案 = 朴素渲染 + 语言标签,不回填 CM6 式高亮栈。


