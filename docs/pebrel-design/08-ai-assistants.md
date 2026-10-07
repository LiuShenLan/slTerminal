# 08 AI 辅助详细设计

> pebrel-design 分片 08/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/08-ai-assistants.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **补全类型(候选、方言快照、语义源接口、呈现条目)** 的锚点归属篇(01 篇类型锚点表已登记):`Suggestion`/`SemanticSuggestion` 候选族、`ShellSyntax`/`CommandContext` 方言快照、`Source`/`Context` 语义源描述接口、`SltermCompletionItem`(呈现条目)唯一定义在本篇;他篇(05/06/07 等)只许 `use` 或经 facade 传参,禁重定义。`CompletionStyle` 本体归 06 篇键域枚举(`CompletionStyleName`),本篇以 `pub use` 再导出——与 pebrel `display/state.rs` 的 `pub use nebula_settings::CompletionStyleName as CompletionStyle` 同形态。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

三个能力面在单进程 GPUI 模块化单体中的终态,全部**面向所有 AI CLI 调优**的 Windows 原生定位收敛:

- **补全** = `slterm_completions`(core crate,照抄 `pebrel-completions`,nushell nu-cli 血统的独立轻量补全库)+ app 层补全域(`slterm_app::completion`,Session/Request 分层、fish 式本地历史、frecency 目录史、有界 git 发现、静态 scripts 发现、suggest_engine 自由函数)+ GPUI 自绘 ghost/popup 双形态。补全**不上按键热路径**:按键只更新输入快照与缓存键,候选在后台算。
- **AI assistant**(默认关闭)= 错误恢复的 LLM 建议条:OSC 133;D 非零退出 → 便宜门链 → 终端网格输出尾部(≤2000 字符、出境前打码)→ 后台请求 → pane chrome 底部建议条,Ctrl+. 只贴入不执行。隐私闸门三件套(默认关 + `redact_secrets` + 字符上限)出厂写死成测试。
- **数学渲染** = app 层自研 TeX 子集引擎(`slterm_app::math`,parser→validate→IR→layout→compile→rasterizer/bitmap/cache/font/spacing 十模块,后端无关)+ 终端覆盖层(`terminal_math`,paint pass 替换可见定界符跨度,终端 cell/滚动/选区/复制不动)+ markdown 数学双管线(自建文档模型阅读器 + file_editor 的 gpui-component TextView 数学薄桥),两条管线共用 `compile_formula` 单点、`MathLayoutCache` 与内嵌字体单例。
- **runtime skills** = 托管 Skill 投放机制(SHA-256 指纹 + marker 三态 + 原子写),挂进 config_guard `heal_all`(归 03 篇挂点);runtime skill 这一件资产归 04 篇,其余 skill 资产与投放策略归本篇(边界归改造节 5)。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_completions/src/`(lib/matcher/completer/file/directory/static_completion/command_context/command_search/semantic/options/span/suggestion) | pebrel `nebula-completions/` 照抄改名(砍 color feature 与 unix 小分支) | 补全引擎:三算法匹配、四方言纯字面量解析、git/scripts/common 语义源纯描述 |
| `slterm_app/src/completion.rs` + `completion/`(paths/project_scripts.rs + project_scripts/、metadata.rs 收敛版) | pebrel `nebula_app/src/completion.rs` 等照抄改造 | Session/Request 分层、路径来源复用、静态 scripts 发现、执行元数据(收敛本地单态) |
| `slterm_app/src/history.rs` | pebrel `nebula_history.rs` 照抄改造(本地池单态化) | fish 式作用域历史(jsonl 追加 + BTreeMap 前缀 range) |
| `slterm_app/src/directory_history.rs` | pebrel 照抄 | frecency 目录智能 |
| `slterm_app/src/git_completion.rs` + `git_completion/tracking.rs` | pebrel 照抄 | 有界本地 git 发现(3s/1MB 双预算、Snapshot TTL + generation) |
| `slterm_app/src/suggest.rs`(display 域下沉) | pebrel `display/suggest_engine.rs` + `display/command_completion.rs` + `display/completion.rs` 收敛 | ghost 余量 + 弹窗双形态计算核心、命令位置 ghost 源、提交纪律 |
| `slterm_app/src/ai_assistant.rs` | pebrel 照抄改造(GPUI 单进程形态) | 触发链判定纯函数族、建议条状态机、四协议族请求 |
| `slterm_app/src/ai_providers.rs` + `provider_test.rs` | pebrel 照抄 | 十三家 provider 元数据/凭据引用体系、连通性测试十态 |
| `slterm_app/src/math/`(十模块) | pebrel `nebula_app/src/math/` 照抄 | 后端无关 TeX 子集引擎 + 有界栅格化 + 4MB LRU |
| `slterm_app/src/terminal_math.rs` + `terminal_math/scan.rs` | pebrel `display/terminal_math*` 照抄 | 终端覆盖层:有界扫描、LineProjection、CoverageMask、绘制计划 |
| `slterm_app/src/markdown/` + `markdown_view.rs` | pebrel `nebula_app/src/markdown/` + `display/markdown_view.rs` 照抄 | 自建文档模型(数学片段)+ 阅读器数学适配(measure/fit) |
| `slterm_app/src/gpui_shell/math_view.rs` | pebrel 照抄 | gpui-component TextView 数学钩子的引擎侧接线(薄桥的 app 半) |
| `slterm_app/src/skills.rs` + `ai_hook/local/runtime_skills.rs` | pebrel 照抄归位(eg 03 安装器家族) | 托管 Skill 投放机制 + skill 资产登记归本篇(除 runtime skill eg 04) |
| `slterm_app/src/scientific_corpus.rs` | pebrel 裁剪(数学部分) | 回归语料:数学 case 集(分子/化学条目砍) |

裁剪不迁:`completion/connections.rs` 整支、`semantic.rs::Source::SshHosts`/`WslDistributions`、`metadata.rs::Execution::Ssh`、`nebula_history.rs` 的 WSL/SSH 池与文件、`chemistry.rs` 整支、`suggest_engine.rs` 的 `SuggestEnv::{Wsl,Ssh,Shell}` + `POSIX_COMMANDS` + `pending_remote_dir` + `remote_dirs` 代际项、`CompletionStyle` legacy 通道(`spawn_fix_request` 的 winit `EventLoopProxy` 形态/`spawn_test`/winit 事件变体)、runtime skills legacy 迁移层、历史/目录史 legacy 迁移面(import_legacy_history/unsaved_seed)、引擎与应用层 unix/macOS 分支、`nebula-completions` 的 `color` feature、Lua 配置接口(全局裁定)。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。
2. **语义源纯描述纪律**(pebrel `nebula-completions/src/semantic.rs` 模块头注释「describes sources, never performs I/O」逐字随迁):`Source`/`Context` 只描述候选来自哪里、值如何编辑,绝不调包管理器、绝不执行项目代码、绝不发起网络 I/O。git 的真实 I/O 在 `git_completion`(有界预算),scripts 的真实读盘在 `project_scripts`(只读 package.json),语义层本身保持纯函数可单测。
3. **command_context 只切词、绝不执行**(spec 采纳点 7):`$` / `` ` `` / `%` / `;|&<>()` 等命中即拒解析;行长 4096、词数 64 上限;这是「补全可安全回写」的根——候选经 `candidate()` 的 per-shell 引用与转义合同(单引号 doubling、双引号反引号转义、CMD 尾部反斜杠拒写、PowerShell 参数分裂整词引用)产出,界外不猜。
4. **隐私闸门三件套不可配置掉**:AI assistant 默认 enabled=false、`redact_secrets` 打码(40+ 连续 base64/hex → `[redacted]`)、输出尾部 ≤2000 字符——三件出厂默认写死成测试,任何设置项不得关闭。
5. **凭据红线**:provider 库只存引用字段(`api_key_set`/`api_key_hint`),明文 token 经 `credential_target` 进 Windows 凭据管理器,`Zeroizing` 全程包裹(含 Bearer 头拼接串),日志不插值;token 不出凭据域的类型层边界 eg 10 篇裁定(本篇只保证 ai_providers 侧红线完整,`plan_balance` 的 settings.json 自读通道归 10 篇,不共享存储归 10 篇 D 裁定)。
6. **引擎后端无关纪律**(pebrel `math/mod.rs` 头注释随迁):`math/` 十模块不依赖窗口、OpenGL 或主题类型;光学补偿只在 `compile_formula` 单点生效(缓存键/fit/draw 全持名义字号);`MIN_READABLE_MATH_PX` 两管线共用,低于即回退源码。
7. **终端仍是真值**:数学覆盖层不改 cell/滚动/选区/复制语义,只在 paint pass 替换可见定界符跨度;覆盖层坐标经 `LineProjection` 与 02 篇 reflow 重排对齐(同一裁决,位图/coverage/文字不落三套坐标)。
8. **类型锚点纪律**:本篇唯一定义补全候选/方言快照/语义源/呈现条目类型;跨领域类型只消费:`Grid`/`Term`/`RenderSnapshot`/`OscEvent`(02)、`AgentKind`(03)、Runtime API 信封(04)、`PaneId`/`TabId`/`TerminalPane`/`terminal_registry`(05)、`CompletionStyleName`/`RuntimeSettings`/主题 token(06)、`TextFileView` 域(07)、凭据三原语归 10。`pebrel` 侧 pane id 裸 `u64` 处本篇以 05 篇 `PaneId` newtype 承接(改动理由同 03/05 篇:防 id 混算笔误,不改语义)。
9. **补全不上按键热路径**:`Request::calculate` 在后台跑,取消/失效代次全链路检查;取消语义在文件遍历、历史扫描、git 子进程间统一为 `Cancellation`(`Arc<AtomicBool>` 令牌)。
10. **出厂交互默认照抄**:CompletionStyle 默认 Inline;ghost 开关 `ghost: bool` 归 06 键;AI assistant 默认关;建议条 Ctrl+. 只贴不执行;Esc/typing 关闭弹窗的 `suppressed_line` 语义照抄。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致,改名/收敛点已标;`pub(crate)` 可见性照 pebrel 形态(应用层内部件),`slterm_completions` 的公开面为 crate 边界契约。契约数值(上限/预算/TTL)首发取 pebrel 实测值照抄,与 04 篇 D04-2 同纪律。

### 补全引擎(`slterm_completions`,本篇锚点,照抄)

```rust
// span.rs —— 输入内字节区间,替换/高亮的锚
pub struct Span { pub start: usize, pub end: usize }

// suggestion.rs —— 候选核心值类型(本篇锚点)
pub struct Suggestion {
    pub value: String,                  // 接受时写入的文本
    pub display_override: Option<String>,
    pub description: Option<String>,
    pub extra: Option<String>,
    pub append_whitespace: bool,
    pub match_indices: Option<Vec<usize>>,  // 弹窗高亮位(字节索引,渲染层转列)
    pub span: Span,
}
impl Suggestion { pub fn display_value(&self) -> &str; }   // override 优先
pub enum SuggestionKind { Command(String), Value(String), CellPath, Directory, File, Flag, Module, Operator, Variable }
pub struct SemanticSuggestion { pub suggestion: Suggestion, pub kind: Option<SuggestionKind> }

// options.rs —— 匹配配置族
pub enum MatchAlgorithm { Prefix, Substring, Fuzzy }
pub enum CompletionSort { #[default] Alphabetical, Smart }
pub struct CompletionOptions { pub case_sensitive: bool, pub match_algorithm: MatchAlgorithm,
                               pub sort: CompletionSort, pub match_description: bool }

// matcher.rs —— 三算法匹配引擎(CandidateMatcher<'a, T> 泛型收集器)
impl<T> CandidateMatcher<'_, T> {
    pub fn new(needle: impl AsRef<str>, options: &CompletionOptions, should_sort: bool) -> Self;  // 引号剥离(终端回写形态)
    pub fn literal(needle: &str, options: &CompletionOptions, should_sort: bool) -> Self;         // 解码值直配(引用属于值本身)
    pub fn add(&mut self, haystack: impl AsRef<str>, item: T) -> bool;
    pub fn check_match(&mut self, haystack: &str) -> Option<Vec<usize>>;
    pub fn results(self) -> Vec<(T, Vec<usize>)>;    // Smart/Alphabetical 双排序
}
impl CandidateMatcher<'_, SemanticSuggestion> {
    pub fn add_suggestion(&mut self, sugg: SemanticSuggestion) -> bool;
    pub fn suggestion_results(self) -> Vec<SemanticSuggestion>;
}
// Prefix/Substring → unscored 折叠匹配(IgnoreCaseExt 的 unicase 折叠)
// Fuzzy → nucleo-matcher(prefer_prefix + Smart normalization);grapheme 对齐的 match_indices
// 是弹窗高亮与排序质量的全部来源。

// completer.rs —— 可插拔来源合同
pub trait Completer {
    fn fetch(&mut self, cwd: &str, prefix: impl AsRef<str>, span: Span, offset: usize,
             options: &CompletionOptions) -> Vec<SemanticSuggestion>;
}

// command_context.rs —— 纯字面量 shell 语法解析(方言快照,本篇锚点)
pub enum ShellSyntax { Posix, PowerShell, Cmd, Literal }   // Literal = 未知 shell 通用字面量
impl ShellSyntax { pub fn for_program(program: &str) -> Self; }   // 按程序名判定,与平台无关
pub struct CommandContext { /* arguments/home_arguments/following/full_word/target:Word/syntax */ }
impl CommandContext {
    pub fn parse(line: &str, cursor: usize, syntax: ShellSyntax) -> Option<Self>;  // 行长 4096/词数 64 上限;展开命中即 None
    pub fn following_arguments(&self) -> &[String];
    pub fn prefix(&self) -> &str;            // 目标词(光标处编辑单位)
    pub fn expands_home(&self) -> bool;      // "~/..." 由路径来源展开
    pub fn candidate(&self, value: &str) -> Option<Suggestion>;  // 匹配与引用分离的编辑合同
}

// file.rs —— 递归路径补全(协作式取消)
pub struct FileSuggestion { /* value/path/style 收敛后无 style 字段(color feature 砍) */ }
pub fn escape_path(path: &str) -> Option<String>;
pub fn expand_home(partial: &str) -> Option<String>;
pub fn surround_remove(partial: &str) -> String;
// color feature 砍除后签名收敛:use_ls_colors/ls_colors_env 两参数随 feature 一并删
pub fn complete_item(want_directory: bool, span: Span, partial: &str, cwds: &[impl AsRef<str>],
                     options: &CompletionOptions) -> Vec<FileSuggestion>;
pub fn complete_item_with_cancel(want_directory: bool, span: Span, partial: &str,
    cwds: &[impl AsRef<str>], options: &CompletionOptions, cancelled: &dyn Fn() -> bool) -> Vec<FileSuggestion>;
pub fn complete_literal_with_cancel(want_directory: bool, span: Span, partial: &str,
    cwds: &[impl AsRef<str>], options: &CompletionOptions, cancelled: &dyn Fn() -> bool) -> Vec<FileSuggestion>;
// exact-match 单命中直钻(隐藏兄弟条目)、n-dots 展开/折叠、home 展开、Windows 盘符前缀分量
// escape_path 转义合同;literal 变体匹配与引用分离。取消即弃部分结果(目录条目间检查)。

// directory.rs / static_completion.rs —— 两个内建源
pub struct DirectoryCompletion;      // 目录角色判定 + 隐藏文件后置排序
pub struct StaticCompletion { pub fn new(options: Cow<'static, [String]>) -> Self;
                              pub fn from_static(options: &'static [&'static str]) -> Self; }

// command_search.rs —— 词级模糊搜索(弹窗历史搜索质量核心)
pub struct CommandQuery { /* terms:空格分词、词序无关、标点字面(shell flag 永不变成搜索算符) */ }
impl CommandQuery {
    pub fn new(query: &str) -> Self;
    pub fn score(&mut self, text: &str) -> Option<u32>;           // exact/prefix/word-prefix/substring/fuzzy 四级 tier 加权
    pub fn score_fields(&mut self, fields: &[&str]) -> Option<u32>; // 多字段优先级
}

// semantic.rs —— 语义源纯描述层(模块头注释「describes sources, never performs I/O」即纪律)
pub enum TrackingMode { Direct, Inherit, Disabled }
pub enum Source {
    Words(&'static [&'static str]),
    Branches { include_busy: bool },
    Revisions { include_busy: bool },
    RevisionsAndPaths { include_busy: bool },
    Tracking { mode: TrackingMode, infer_name: bool },
    Paths { directories_only: bool },
    Remotes, RemoteBranches,
    PushRefs { destination: bool },
    Workspaces, ProjectScripts,
    Options,
    // SshHosts / WslDistributions 两变体随远程整砍(spec 不采纳点 1/2)
}
pub struct Context { /* input:CommandContext / source / directories / attached / preserved_tail / value_prefix */ }
impl Context {
    pub fn parse(line: &str, cursor: usize, syntax: ShellSyntax) -> Option<Self>;  // git/npm/pnpm/yarn/common 命令分派
    pub fn input(&self) -> &CommandContext;
    pub fn value_prefix(&self) -> &str;
    pub fn candidate(&self, value: &str) -> Option<Suggestion>;   // attached/preserved_tail 值编辑合同
    pub fn guesses_branches(&self, configured: bool) -> bool;
    pub fn candidates<'a>(&self, values: impl IntoIterator<Item = &'a str>) -> Vec<Suggestion>;  // fuzzy + 前缀提权 + 256 截断
    pub fn static_candidates(&self) -> Vec<Suggestion>;
}
// semantic/git.rs:git 参数角色全家——switch/checkout/merge/rebase 选项表
//   (OptionSpec 五属性 names/value/include_busy/terminal/optional_value)、push/pull/fetch 网络族
//   (git_network:remote 位置/--multiple/--delete/refspec 尾保留/PushRefs{destination} 双侧)、
//   --track=direct|inherit 与 --guess/--no-guess/--no-track 相互作用、checkout "--" 后分支/路径二态。
// semantic/scripts.rs:PackageManager{Npm,Pnpm,Yarn} + ProjectSelection
//   { manager, selectors, all, include_root, allow_missing }——只产出描述,不调包管理器。
// semantic/common.rs:POSIX/PowerShell/Cmd 三套别名 + resolve() 短选项连写与附值解析、Cmd "/" 选项前缀。

// lib.rs —— 模块契约(照抄):matcher/completer/file/directory/static_completion/
//   command_context/command_search/semantic/options/span/suggestion
```

### 补全应用层(`slterm_app`,照抄 + 收敛)

```rust
// completion.rs —— Session/Request 分层(注释「来源缓存跟随 pane」即纪律)
#[derive(Clone, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);
impl Cancellation { pub(crate) fn is_cancelled(&self) -> bool; pub(crate) fn cancel(&self); }

pub(crate) struct Session {              // 按 pane 持有(git/scripts 来源 Cache;connections 成员随远程砍)
    git: Arc<crate::git_completion::Cache>,
    scripts: Arc<project_scripts::Cache>,
}
impl Session {
    pub(crate) fn invalidate(&self);     // 提交命令事件触发归壳归 02 事件链
    pub(crate) fn request_at(&self, cwd: String, env: SuggestEnv, line: String, cursor: usize,
        style: CompletionStyle, execution: Option<&PaneExecContext>,   // PaneExecContext eg 04 篇锚点
        syntax_override: Option<ShellSyntax>) -> Request;
}
pub(crate) struct Request { /* cwd/env/line/cursor/style + git/scripts Option<(Arc<Cache>, Execution)>
                               + semantic + syntax;后台只收到输入快照与已确认执行环境 */ }
impl Request {
    pub(crate) fn calculate(mut self, cancellation: &Cancellation) -> Candidates;
    // 取消/失效代次全链路检查;git/scripts 的 Execution::prepare() 失败即降级该来源
}

// suggest.rs(suggest_engine 收敛版)—— 双形态计算核心
pub(crate) struct Candidates {
    pub suggestion: String,                        // Inline ghost 余量
    pub suggestion_edit: Option<SltermCompletionItem>,
    pub completion_items: Vec<SltermCompletionItem>,   // Popup 列表
}
pub(crate) struct SuggestSources<'a> {
    pub history: &'a Mutex<SltermHistory>,         // Borrowed/Shared 双形态收敛为单持有(eg 改造节 2)
    pub directories: &'a DirectoryHistory,
    pub commands: &'a Arc<Mutex<Vec<String>>>,     // PATH 探针结果(eg command_completion)
    pub enabled: bool, pub style: CompletionStyle, // CompletionStyle归 06 篇再导出
}
pub(crate) struct SuggestState {                 // 原 NebulaPaneState 补全面收敛归本篇
    suggestion_key: String,                       // cwd+env+line+style+命令代际 缓存键
    suppressed_line: Option<String>,              // Esc 关闭语义:行不变不重开
    popup_requested: bool,                        // Hybrid 模式的弹窗请求位(eg 06 active_style)
}
pub(crate) fn calculate(sources: &SuggestSources<'_>, input: &Input<'_>, syntax: ShellSyntax,
                        cancelled: &dyn Fn() -> bool) -> Candidates;
// Inline 三级回退:历史 hint → 命令位置 → 路径;Popup 多候选合并(历史 8 条 + 命令 + 路径)
pub(crate) fn suggest_collect(sources: &SuggestSources<'_>, input: &Input<'_>, syntax: ShellSyntax,
                              result: &mut Candidates, cancelled: &dyn Fn() -> bool);
// POPUP_LIMIT=256(「8 是视口行数不是数据上限」注释承载的教训)、LABEL_MAX=44 + elide_left 左省略、
//   精确命令补空格。
pub(crate) fn suggestion_key(input: &Input<'_>, style: CompletionStyle, command_generation: usize) -> String;
// 键 = cwd+env+line+style+命令代际;远端目录代际项随 remote_dirs 砍归改造节 2。
pub(crate) fn semantic_candidates_at(line: &str, cursor: usize, style: CompletionStyle,
    candidates: Vec<SemanticSuggestion>) -> Candidates;
// 语义 byte span → 终端行尾编辑合同:只替换分歧尾部、已闭合引文接受、UTF-8 安全
//   (replace_chars / replace_after_chars)。

// 呈现条目(本篇锚点;pebrel display/state.rs 改名随迁)
pub enum SltermCompletionKind { History, Command, Dir, File }   // 弹窗右侧 tag + 图标四态
pub struct SltermCompletionItem {
    pub label: String,               // 展示文本(可能左省略)
    pub insert: String,              // 接受时写入 PTY 的字符(用户已打部分的余量)
    pub replace_chars: usize,        // 光标前替换字符数
    pub replace_after_chars: usize,  // 光标后替换字符数(原生行编辑器证明的尾部)
    pub kind: SltermCompletionKind,
}
pub use slterm_settings::CompletionStyleName as CompletionStyle;   // 本体归 06 篇(与 pebrel 同形态)

// command_completion.rs —— 命令位置 ghost 源(改名随迁)
pub(crate) const SLTERM_GHOST_MAX: usize = 96;    // 幽灵余量上限(防长路径溢进 chrome)
pub(crate) fn extract_program(line: &str) -> Option<String>;  // 程序身份归一:小写、剥路径与
//   .exe/.cmd/.bat/.ps1/.com——AI assistant 判定表(INTERACTIVE)与侧栏图标共用同一份身份(eg 03)。
pub(crate) fn command_hint<'a>(commands: &'a [String], prefix: &str) -> Option<&'a str>;
pub(crate) fn command_hints<'a>(commands: &'a [String], prefix: &str, limit: usize) -> Vec<&'a str>;
pub(crate) fn is_command_position(line: &str) -> bool;
pub(crate) fn path_wants_directory(line: &str) -> bool;   // cd/chdir/pushd/sl/set-location

// 提交纪律(display/completion.rs::nebula_commit_line 收敛归壳)
// Enter 提交历史时 screen_line(网格读回,Windows 下唯一能看见 tab 补全的来源)优先于
// 按键重建的 line_buf——suggest_skip 注释「laudeclaude」拼接垃圾的教训;宁可不记也不记脏。

// history.rs —— fish 式本地历史(本地池单态化,eg 改造节 2)
pub enum HistoryScope { #[default] Local }        // Wsl(String)/Ssh(String) 两变体砍;normalized() 塌缩
pub struct SltermHistory { /* pools: 单池;原 HashMap<HistoryScope, HistoryIndex> 形态保留以备未来远程 */ }
impl SltermHistory {
    pub fn load() -> Self;                        // 只读 history.jsonl(01 篇 D 节归位归本篇)
    pub fn record(&mut self, scope: &HistoryScope, cmd: &str, cwd: &str);  // 连续重复去重;jsonl 追加
    pub fn hint(&self, scope: &HistoryScope, prefix: &str) -> Option<&str>;        // BTreeMap 前缀 range 取最新余量(Inline ghost)
    pub fn search_with_cancel(&self, scope: &HistoryScope, text: &str, limit: usize,
                              cancelled: &dyn Fn() -> bool) -> Vec<&str>;           // CommandQuery 评分,popup 8 条上限
}
// HISTORY_MAX 5000 条池上限;parse_record / record_category_file 分池校验纪律保留(单池语义下塌缩)。

// directory_history.rs —— frecency 目录智能
pub(crate) struct DirectoryHistory { /* rank × 四档时间衰减(小时/天/周/外);只记本机终端真实启动/上报目录 */ }
pub(crate) fn global() -> DirectoryHistory;
impl DirectoryHistory {
    pub(crate) fn record(&self, path: &str) -> bool;
    pub(crate) fn hint(&self, line: &str, cwd: &str) -> Option<String>;   // cd 补全与目录幽灵提示源
    pub(crate) fn hint_with_cancel(&self, line: &str, cwd: &str, cancelled: &dyn Fn() -> bool) -> Option<String>;
    pub(crate) fn score(&self, path: &Path) -> Option<f64>;
    pub(crate) fn search(&self, query: &str, limit: usize) -> Vec<PathBuf>;
}
// MAX_ENTRIES 2048 / MAX_TOTAL_RANK 10000 封顶 / DATABASE_VERSION 1 / 跨进程文件锁 + 原子落盘归 05/06 共有件。

// git_completion.rs —— 有界本地 git 发现
pub(crate) struct Cache(Mutex<(u64, Option<Snapshot>)>);   // generation 代次失效:提交命令后旧请求不得回填
impl Cache { pub(crate) fn invalidate(&self); }
pub(crate) fn complete_available(cache: &Cache, execution: &Execution, cwd: &str,
    context: &Context, cancelled: &dyn Fn() -> bool) -> Option<Vec<Suggestion>>;
// 3s 总预算 + 1MB 输出预算双上限;for-each-ref 一次取全 + config --null --get-regexp 哨兵两次读取;
// Snapshot 2s TTL;busy/worktree/current/commit-ish/symbolic/direct_tracking 引用语义;
// tracking 上游推断归 git_completion/tracking.rs;RevisionsAndPaths 的 guesses 文件重名磁盘核查。
// is_host() guest 分支随 Execution::Ssh 砍折叠(不采纳点 4)。

// completion/project_scripts.rs —— 静态 scripts 发现(绝不执行项目代码)
pub(super) struct Cache(Mutex<(u64, Option<Snapshot>)>);   // 2s TTL + 代次纪律
// 只读 package.json 的 scripts 键:1MB 字节上限、向上找最近项目即停、损坏/缺 scripts 不借父项目、
//   workspace 选择器展开归 project_scripts/workspace.rs(read/finish_catalog/patterns)。
// read_names 的「must-not-run」测试断言随迁(eg 测试点清单)。

// completion/metadata.rs —— 执行元数据(收敛本地单态)
pub(crate) enum Execution {                      // Ssh 变体砍(不采纳点 4);tokio 冻结/prepare 塌缩为本地直通
    Process { context: PaneExecContext, scope: SuggestEnv },
}
impl Execution { pub(super) fn prepare(&mut self) -> bool; pub fn key(&self) -> String; }
```

### AI assistant(`slterm_app::ai_assistant`,照抄 + GPUI 单进程改造)

```rust
// 建议条数据(底部 pane chrome 消费归壳归 05)
pub struct AiFix { pub command: String, pub explain: String, pub danger: bool }
pub enum AiFixState {                        // 每 pane 一条生命周期
    Pending { seq: u64 },                    // 请求在飞;条显 analyzing
    Ready { seq: u64, fix: AiFix },          // 等 Ctrl+. / Esc / typing
}
impl AiFixState { pub fn seq(&self) -> u64; }   // seq 防过期响应落位

// AssistantConfig —— 独立配置文件(slterm_assistant.txt;形态归 D08-1)
pub struct AssistantConfig { pub enabled: bool, pub base_url: String, pub model: String,
                             pub ignored_exit_codes: Vec<i32> }     // 默认 enabled=false
impl AssistantConfig { pub fn load() -> Self; }  // 每次失败命令读一次(亚毫秒,不缓存)

// 防误触规则表(spec 的规则表);program = extract_program(last_committed),与命令位置源共用身份
pub fn should_suggest(exit_code: i32, command: &str, program: Option<&str>, ignored: &[i32]) -> bool;
// USER_ABORT_CODES: 130/137/143 + Windows STATUS_CONTROL_C_EXIT(-1073741510)——「用户停了它」不是「它失败了」
// BARE_TOOLS 单词调用:裸 git/npm/pnpm/yarn/pip/pip3/cargo/dotnet/go/docker/kubectl 是帮助屏
// INTERACTIVE 表:vim/nvim/vi/nano/less/more/top/htop/btop/ssh/claude/codex/gemini/aider/
//   lazygit/yazi/ranger/fzf/gh;--help/-h/-? 与 help 豁免;ignored_exit_codes 用户自定义。

pub const COOLDOWN: Duration = Duration::from_secs(5);   // 同 pane 两次触发最短间隔
pub fn is_dangerous(command: &str) -> bool;   // 本地词表 21 条与模型 danger 按位 OR:
// 模型判意图、词表判字面,单方可被骗。字面表照抄 pebrel PATTERNS(rm -rf / rm -fr /
// git reset --hard / git push --force 与 -f / git clean -fd / remove-item -recurse /
// rd 与 del 的递归强删形态 / format / mkfs / dd if= / shutdown / taskkill 强杀形态 /
// kill -9 / chmod -r 777 / 重定向写盘设备 / drop table / truncate table),用例逐条钉死。

pub fn redact_secrets(text: &str) -> String;  // 40+ 连续 base64/hex 字符(含 +/=/-/_ 拼接形态)
                                              // 替换 [redacted];出境前唯一闸。
pub fn parse_fix(content: &str) -> Option<AiFix>;  // 抗 prose/fence 噪声提取 JSON:
// 首个 { 至末个 } 切片;空 command 视为刻意沉默;单命令行约束(含换行即拒);
// danger = 模型标志 OR is_dangerous。

pub struct FixRequest { pub pane: PaneId, pub seq: u64, pub command: String,
    pub exit_code: i32, pub cwd: String, pub branch: String,
    pub output_tail: String }                 // 网格尾部,ANSI-free,已过 redact_secrets
// SYSTEM_PROMPT 照抄:fix failed shell commands / Reply with ONLY a JSON ...

pub(crate) fn spawn_fix_request(cfg: AssistantConfig, req: FixRequest,
    deliver: impl Fn(AiFixState) + Send + 'static);   // GPUI 单进程形态归改造节 8:
// winit EventLoopProxy 通道/spawn_test/EventType::AiFixReady 变体全砍(不采纳点 7),
// 后台线程算完经 GPUI executor 投递回 pane归属线程。
fn send_model_request(target: &RequestTarget, user: &str) -> Option<String>;  // 四协议族归下
fn fallback_api_key() -> Option<Zeroizing<String>>;   // SLTERM_AI_KEY → OPENAI_API_KEY 存在即用
```

### ai_providers(`slterm_app::ai_providers` + `provider_test.rs`,照抄)

```rust
pub enum ProviderKind { OpenAi, Anthropic, Google, Ollama, OpenRouter, Qwen, DeepSeek,
                        Kimi, Zhipu, Doubao, Mimo, AzureOpenAi, Custom }   // 十三家,不裁剪
impl ProviderKind {
    pub const PRESETS: [Self; 13];
    pub fn label(self) -> &'static str;
    pub fn default_base_url(self) -> &'static str;    // Azure 为 {resource}.openai.azure.com 部署路径形态
    pub fn default_model(self) -> &'static str;
    pub fn requires_api_key(self) -> bool;            // Ollama 免 key
    pub fn uses_openai_protocol(self) -> bool;        // Anthropic/Google 两族例外
}
pub struct AiProvider { pub id: String, pub name: String, #[serde(default)] pub note: String,
    #[serde(default)] pub website_url: String, pub kind: ProviderKind, pub base_url: String,
    pub model: String, #[serde(default)] pub enabled: bool,
    #[serde(default)] pub api_key_set: bool,          // 凭据只存引用,明文永不入 store
    #[serde(default)] pub api_key_hint: String,       // 尾四掩码(•••• + 末四位,user-facing fingerprint)
    #[serde(default)] pub full_url: bool, ... }       // full_url 直通形态
pub struct ProviderStore { /* providers 数组 + active_id;normalize 预设补全与 active_id 修复 */ }
pub struct ProviderMetadataDraft { /* 设置页草稿归 06;共享清洗归 apply_metadata_draft */ }
pub fn apply_metadata_draft(provider: &mut AiProvider, draft: ProviderMetadataDraft);
pub fn store_path() -> PathBuf;                        // providers.json(01 篇 D 节归位归本篇)
pub fn load() -> ProviderStore; pub fn save(store: &ProviderStore) -> io::Result<()>;
pub fn preset_id(kind: ProviderKind) -> String; pub fn next_custom_id(store: &ProviderStore) -> String;
pub fn active_enabled(store: &ProviderStore) -> Option<&AiProvider>;
pub fn credential_target(id: &str) -> String;         // Slterm/AI/<id>(归 10 凭据域消费归 01 表归位)
pub fn api_key_hint(key: &str) -> String;
pub fn save_api_key(id: &str, key: &str) -> io::Result<String>;
pub fn store_provider_api_key(provider: &mut AiProvider, key: &str) -> io::Result<()>;  // 先写凭据后回写引用
pub fn delete_api_key(id: &str) -> io::Result<()>;
pub fn remove_provider(store: &mut ProviderStore, id: &str) -> io::Result<()>;  // 先删凭据再删元数据,不留孤儿密钥
pub fn load_api_key(id: &str) -> io::Result<Option<Vec<u8>>>;   // 出域即 Zeroizing,日志不插值
pub fn test_provider(provider: &AiProvider) -> ProviderTestResult;   // 12s 超时连通性测试归设置页归 06
// 四协议族归 ai_assistant::send_model_request:OpenAI chat/completions(Authorization Bearer)/
// Anthropic /messages(x-api-key + anthropic-version)/ Google :generateContent(x-goog-api-key)/
// Azure OpenAI(api-key + 部署路径);full_url 直通;30s 全局超时;响应按协议族取文本字段。

// provider_test.rs —— 语义化结果枚举(十态族,UI 边界转本地化归 06)
pub enum ProviderTestOutcome {
    Success { status: u16 }, InvalidEndpoint, MissingModel, MissingApiKey,
    CredentialReadFailed, InvalidCredentialEncoding, Timeout, HostNotFound,
    ConnectionFailed, Io { kind: String }, Tls, RequestFailed,
    AuthFailed { status: u16 }, EndpointNotFound { status: u16 },
    RateLimited { status: u16 }, HttpStatus { status: u16 }, StartFailed { error: String },
}
impl ProviderTestOutcome { pub const fn is_success(&self) -> bool; }
```

### 数学引擎(`slterm_app::math`,照抄;头注释纪律随迁)

```rust
// mod.rs —— 光学常数族(决策记录,注释逐字随迁)
pub(crate) const MIN_READABLE_MATH_PX: f32 = 6.0;   // 低于即回退源码;两管线共用的判定底线
pub(crate) const OPTICAL_SCALE: f32 = 1.21;         // KaTeX 对 Latin Modern 同族补偿的同源数值
                                                    // (x-height 0.431 em vs 编程字体 0.53-0.56);只在 compile_formula 单点生效
pub(crate) const MIN_SCRIPT_SCALE: f32 = 0.8;       // 刻意偏离 LaTeX 0.7 的终端理由:20px 下 0.7em
                                                    // 分子只剩 14px;0.85 会把根号顶上字形变体阈值(注释实测上限)
pub(crate) const MIN_SCRIPT_SCRIPT_SCALE: f32 = 0.65;
pub(crate) fn pixels_per_point(scale_factor: f32) -> f32;   // scale × 96/72.27,两壳统一公式

// validate.rs —— 宏展开前的线性预算扫描器
pub(crate) const DEFAULT_LIMITS: MathLimits = MathLimits { max_source_bytes: 16*1024,
    max_depth: 64, max_events: 8192, max_nodes: 4096, max_matrix_cells: 1024,
    max_children: 1024, max_ops: 8192 };
pub(crate) struct MathLimits { /* 八项预算字段如上 */ }
pub(crate) enum MathErrorKind { /* 分类:超预算/黑名单命令/解析/字体 归实现 */ }
pub(crate) struct MathError { pub(crate) kind: MathErrorKind, pub(crate) message: String, ... }
pub(crate) struct ValidationStats { /* 展开前统计归实现 */ }
pub(crate) fn validate(source: &str, limits: MathLimits) -> Result<ValidationStats, MathError>;
// FORBIDDEN_COMMANDS 黑名单(宏展开前:\def/\csname/\input/\directlua/catcode 等动态控制序列
//   与外部资源命令——\input{private.tex} 必须报错);任何宏展开晚于这一层。

// parser.rs —— pulldown-latex =0.7.1 事件流 + 自建有界 arena
pub(crate) fn parse_formula(source: &str) -> ...;
// normalize_ascii_math_arrows / substitute_unsupported_presentation 机制照抄;
// 「original source 不重写」的分流合同归 compile 节准绳,具体替换表是实现细节随实现走。

// compile.rs —— 单点编译入口与传输规范化
pub(crate) fn compile_formula(source: Arc<str>, font_size_px: f32, display: bool,
                              ppp: f32) -> Option<Arc<MathLayout>>;   // 光学补偿唯一生效点
pub(crate) fn compile_formula_source(source: &str, ...) -> Option<Arc<MathLayout>>;  // 原始路径
pub(super) fn normalize_formula_source(source: &str) -> Cow<'_, str>;  // HTML 实体有限轮解码/CRLF 归一/
pub(super) fn brace_unbraced_fraction_arguments(source: &str) -> Option<String>;      // 行断裂痕/分式参数规范化
// 三条测试钉死:original source 不重写、不猜缺失行断、不静默替换箭头。

// font.rs —— Latin Modern Math 零拷贝访问层(换字体 = 换这一个文件)
pub(crate) static FONT_BYTES: &[u8] = include_bytes!(...);   // ~716KiB 内嵌;资产归 01 篇 licenses/ 随迁归 12
pub(crate) struct MathFont;                                 // OnceLock Face 进程级单解析
pub(crate) struct GlyphMetrics { ... }
pub(crate) enum MathConstant { /* 轴高/分式/上下标/根号/极限全组(数值照抄) */ }
pub(crate) struct StretchPart { ... } pub(crate) enum StretchGlyph { ... }   // 字形组装 1024 上限

// layout.rs —— OpenType MATH 盒布局与后端无关绘制指令
pub(crate) struct MathMetrics { ... }
pub(crate) struct MathGlyphOp { /* glyph id + x/y + size */ }
pub(crate) struct MathRuleOp { /* 线段的 x/y/w/h */ }
pub(crate) struct MathTextOp { /* 紧凑数学字体缺字的跨平台文本兜底——「\text 里的空格曾
                                 吃掉整条流水线公式」教训的修复形态 */ }
pub(crate) struct MathLayout { pub(crate) width/height/depth/ops: Vec<Op> /* Glyph/Rule/Text 三族 */ }
pub(crate) fn layout_formula(ir: &Ir, font: &MathFont, font_size_px: f32, ppp: f32) -> Option<MathLayout>;

// rasterizer.rs + bitmap.rs —— 有界 CPU 栅格化与合成
pub(crate) struct RasterizedMathGlyph { ... }
pub(crate) struct MathGlyphRasterizer { ... }   // glyph 512px 维度上限;coverage gamma 0.75
                                                // (无 DirectWrite 子像素对比度的灰度笔画补偿)
// 位图 8192px/24MB 双上限;required_bytes 预检与实配一致纪律归 bitmap.rs。

// cache.rs —— 固定 4MB LRU(防内存膨胀的唯一闸)
pub(crate) const LAYOUT_CACHE_BUDGET: usize = 4 * 1024 * 1024;
pub(crate) struct FormulaCacheKey { /* 公式编号 + 字号 bits + display */ }
pub(crate) struct MathLayoutCache { ... }       // allocated_bytes 计费,预算驱逐
impl MathLayoutCache { pub(crate) fn get(&mut self, key) -> Option<&MathLayout>;
                        pub(crate) fn get_or_insert_with(...); }
```

### 终端覆盖层(`slterm_app::terminal_math`,照抄;终端仍是真值)

```rust
pub(crate) use scan::scan_visible;
// 有界扫描契约值(全部随迁):
// MAX_VISIBLE_FORMULAS 64 / MAX_PERSISTED_FORMULAS 2048 / PERSISTED_FORMULA_BUDGET 1MB /
// MAX_HISTORY_FORMULA_ROWS 512(视口锚点孤儿闭定界符回溯历史补全——AI TUI 硬折行恢复路径)/
// BARE_PAREN_SEARCH_ROWS 8 / BARE_BRACKET_SEARCH_ROWS 24 / BARE_SEARCH_CELL_BUDGET 4k cell /
// MAX_ABSORBED_BLANK_ROWS 2(+margin)/ HEIGHT_OVERRUN_TOLERANCE 0.12(行高溢出容差缩放)
// 定界符意图分级:$$ / \[ / \( / 裸括号,意图自证的 O(1) 预过滤(一屏几千个未闭合
// 左括号不得把单帧扫描变成网格平方级);ANSI 背景与 TUI reasoning 样式放弃覆盖。

pub(crate) struct TerminalMathState { /* formulas 持久化集 + persisted_bytes + LineProjection */ }
pub(crate) type LayoutResolver =
    Arc<dyn Fn(Arc<str>, f32, f32, bool) -> Option<Arc<MathLayout>> + Send + Sync>;
// 引擎侧的编译句柄注入归 math_view / markdown_view 两消费面,覆盖层不直接依赖引擎模块归实现归本篇。
impl TerminalMathState {
    pub(crate) fn set_layout_resolver(&mut self, resolver: LayoutResolver);
    pub(crate) fn update_projection(&mut self, overlays: &[FormulaOverlay],
                                    prepared: &[Option<PreparedFormula>], reflow_inline: bool);
}
pub(crate) struct FormulaOverlay { /* spans/charge(源字节计费)/display/intent/neighbours 邻行标记 */ }
pub(crate) struct PreparedFormula { /* compact_cells 等绘制就绪数据 */ }
pub(crate) struct LineProjection { spans: Vec<ProjectionSpan> }   // reflow 重排后覆盖坐标重建;
// 选区单元格原子化——公式跨度是原子,左半/右半选中映射源边界而不是发明 TeX 内部光标;
// 调用方必须和投影采用同一裁决,否则位图、coverage 与后续文字会落在三套坐标上。
pub(crate) struct CoverageMask { ... }        // 覆盖区文字抑制
pub(crate) struct OverlayDrawPlan { ... }
pub(crate) fn prepare_overlays(state: &mut TerminalMathState, overlays: &[FormulaOverlay]) -> Vec<Option<PreparedFormula>>;
pub(crate) fn plan_overlay_draw(prepared: &[Option<PreparedFormula>], ...) -> Vec<OverlayDrawPlan>;
pub(crate) fn draw_overlays(plans: &[OverlayDrawPlan], mask: &CoverageMask, ...);   // 壳 paint pass 调用归 M3+
// scan.rs(scan_visible / scan_grid / find_formula 族):TextGrid 为扫描输入适配器——
//   由 02 篇快照同一锁窗口构造(eg 缝合点 1),不改 cell/滚动/选区/复制语义。
```

### markdown 数学管线(`slterm_app::markdown` + `markdown_view`,照抄)

```rust
// markdown/mod.rs —— 自包含文档模型(flat block 序列;引擎全在 app 侧)
pub enum MathMode { Inline, Display }
pub struct MathSource { pub source: String, pub mode: MathMode }
impl MathSource { pub fn as_str(&self) -> &str; ... }
pub enum FormattedTextLine { ..., DisplayMath(MathSource), ... }   // 数学行块
pub enum FragmentContent { ..., Math(MathSource), ... }           // 行内数学片段
pub struct FormattedTextFragment { pub content: FragmentContent, pub styles: FormattedTextStyles }
pub fn parse_markdown(...) -> FormattedText;    // pulldown-cmark GFM math_flow/math_text 构造开启;
                                                // quoted display math 折叠与 math fence 规范化归 parser.rs

// display/markdown_view.rs —— 阅读器数学适配(双管线之一;复用 MathLayoutCache)
struct MathRun { source: MathSource, layout: Option<Arc<MathLayout>>, scale: f32 }
fn measure_math(cache: &mut MathLayoutCache, source: &MathSource, base_font_size: f32, ...)
    -> Option<MathRun>;          // 公式参与行宽测量
fn fit_math_run(run: &mut MathRun, max_width: f32);   // 过宽缩放;低于 MIN_READABLE_MATH_PX 回退源码
// 「markdown 与终端定位公式的方式不同,但都不允许独立规范化/解析/排版」的边界注释随迁。

// gpui_shell/math_view.rs —— 薄桥的引擎侧(TextView 钩子归 gpui-component fork归 D08-3)
pub fn register(cx: &mut App);   // gpui_component::text::set_math_renderer(cx, |spec, window, cx| ...)
//                                引擎/栅格化/绘制全住本篇,fork 只有接线层归改造节 6。
pub(crate) fn paint_formula_image(...); pub(crate) struct MathAssets { ... }
```

### runtime skills(eg 03 锚点,本篇只给机制签名与资产边界归改造节 5)

```rust
// ai_hook/local/runtime_skills.rs(eg 03 安装器家族归位归 03;签名照抄)
const RUNTIME_SKILL_MD: &str = include_str!("../../../../docs/skills/slterm-runtime/SKILL.md");   // 归 04
const RUNTIME_SKILL_OPENAI_YAML: &str = include_str!("../../../../docs/skills/slterm-runtime/agents/openai.yaml");
const RUNTIME_SKILL_MARKER: &str = ".slterm-managed";            // legacy 双 marker 不迁归 D 节归 03
pub(super) enum ManagedSkillInstall { Installed, Current, Conflict }   // 指纹 ≠ marker 即 Conflict,永不覆盖用户 Skill
pub(super) enum ManagedSkillRemoval { ... }
pub(super) fn runtime_skill_candidates() -> Vec<(&'static str, PathBuf)>;  // codex ~/.agents/skills + claude 配置目录
pub(super) fn ensure_runtime_skills() -> Vec<(&'static str, PathBuf, io::Result<ManagedSkillInstall>)>;
// SHA-256 指纹(SKILL.md + openai.yaml 双文件哈希)+ 原子写 + 先内容后 marker(崩溃不产生半套误认)
```

### 回归语料(`scientific_corpus.rs`,数学部分)

```rust
pub(crate) const DOCUMENTS: [(&str, usize, usize); /* 数学部分保留 */];  // 分子/化学条目随化学砍归改造节 1
pub(crate) struct Case { pub(crate) source: String, pub(crate) molecule: bool,  // molecule 字段随化学砍
    pub(crate) display: bool, pub(crate) fallback: bool, pub(crate) preserve: bool, pub(crate) line: usize }
pub(crate) fn read_document(name: &str) -> String;   // ≤128KB 上限断言(语料防腐)
pub(crate) fn cases(source: &str) -> Vec<Case>;      // 期望标记注释(pebrel-test: source-fallback / preserve-source)
// 编译 + 栅格化 + 像素非空断言 + preserve 语义断言(不发明缺失等号、逗号不变成不可见间距)
//   + 覆盖数下限断言(防语料缩水)+ resize 重复的 LRU 有界测试归测试点清单归 11。
```

## 数据流与状态机

### 补全请求链(按键 → 快照 → 后台计算 → 双形态呈现)

```
按键/粘贴归壳输入层(eg 02 Msg::Input 提交路径归 M3)
  → 壳更新行缓冲与 cursor,取 Input 快照 { line, cursor, cwd, env=SuggestEnv::Local }
  → suggest_update(sources, state, line_override)
      ① suggestion_key = cwd+env+line+style+命令代际(远端目录代际项已砍归改造节 2)
      ② key == state.suggestion_key → 命中缓存直接呈现;Esc 关闭的 suppressed_line 语义:
         键保留、行不变不重开
      ③ miss → Session::request_at(cwd, env, line, cursor, style, execution, syntax_override)
         ——方言是输入事实:local 时按 execution.shell_program 经 ShellSyntax::for_program
         判定,未证明方言只接受 Literal(四方言合同归本篇锚点)
      ④ Request{calculate} 后台跑(任何一门取消/失效代次即弃)
         - semantic = SemanticContext::parse(line, cursor, syntax) → 纯描述 Source
         - 需要本机 git I/O 的 Source 才复制启动环境(git/scripts Option 化纪律)
         - git 来源:complete_available(cache, execution, cwd, ctx, cancel)
           (Snapshot 2s TTL;generation 失效;3s/1MB 预算;取消在子进程间检查)
         - scripts 来源:project_scripts Cache(2s TTL;只读 package.json)
         - 历史/命令位置/路径来源:本进程直算,取消令牌全链路
      ⑤ Candidates → Inline:三级回退(历史 hint → 命令位置 → 路径)写 ghost 余量
         Popup:suggest_collect 多候选合并(历史 8 条 + 命令 + 路径,256 上限,
         44 字符左省略)→ 弹窗列表归壳绘制归改造节 4
      ⑥ 语义候选投射:semantic_candidates_at 把 byte span 换成行尾编辑合同
         (只替换分歧尾部 / replace_chars / replace_after_chars)
提交命令(Enter)归 02 事件链 → nebula_commit_line:screen_line(网格读回)优先于
  按键重建 line_buf → record_command 入历史(连续重复去重)+ record_directory
  → Session::invalidate(git/scripts 代次 +1,旧后台请求不得回填)
```

硬规则:补全来源缓存按 pane 挂(`TerminalPane` 持 Session归 05 篇);「PTY 进程映射归 terminal_registry、补全来源缓存归 Session,两者以 PaneId 关联」的纪律归 05 篇索引;界面不决定来源适用条件(pebrel 注释「来源缓存跟随 pane」逐字随迁)。

### AI assistant 触发状态机(便宜门链,跑在每次命令失败上,不能吵)

```
Event::CommandDone{exit_code}(eg 02 篇锚点;133;D 首参,裸第三方 133;D 为 None 静默)
  → Some(code) 且非 0:
      门 1:同 pane ai_fix_cooldown.elapsed() >= COOLDOWN(5s)
      门 2:AssistantConfig::load().enabled(默认 false——首门即关,后续成本为零)
      门 3:should_suggest(code, last_committed, extract_program(last_committed), ignored)
           规则表归本篇锚点(USER_ABORT_CODES/BARE_TOOLS/INTERACTIVE/help 豁免)
      任一门不过 → 安静返回
  → 全过:cooldown = now;seq = 全局单调 +1
  → FixRequest { command, exit_code, cwd, branch,
                 output_tail = redact_secrets(grid_output_tail(24 行, 2000 字符)) }
      grid_output_tail:光标处向上 24 行、尾部 2000 字符封顶、跳 WIDE_CHAR_SPACER
      (CJK 双胞空格污染)、去首尾空行——shell 侧集成看不见渲染上下文,终端网格是权威源(eg 02)
  → pane.ai_fix = Pending{seq};后台线程 spawn_fix_request(四协议族归 ai_providers 节)
  → 响应到达:seq 与 pane 现态比对,过期即丢;parse_fix 抗噪提取
  → pane.ai_fix = Ready{seq, fix}(danger = 模型标志 OR is_dangerous)
  → 建议条归壳绘制归改造节 4:Ctrl+. 只贴入不执行(发送 = Msg::Input 归 02);
    Esc/typing 关闭;Pending 时误按 Ctrl+. 不丢请求(pebrel event.rs 注释语义)
```

### provider 凭据流(引用字段出境,明文不出凭据域 eg 10)

```
设置页归 06 编辑 ProviderMetadataDraft → apply_metadata_draft 共享清洗归本篇
  → save:providers.json 原子写(eg 05/06 共有件);normalize 预设补全与 active_id 修复
首次配 key:store_provider_api_key(provider, key)
  → save_api_key → credential_target(Slterm/AI/<id>)归 10 凭据管理器归 01 表归位归本篇消费
  → 回写 api_key_set=true + api_key_hint(尾四掩码)
请求出境:ai_assistant::request_target
  → active_enabled 取启用 provider → load_api_key(出域即 Zeroizing)
  → send_model_request 按 kind 组四协议族头(Bearer/x-api-key + anthropic-version/
    x-goog-api-key/Azure api-key);全程含头拼接串 Zeroizing;日志只记状态不插值归 10
删除:remove_provider 先删凭据再删元数据(不留孤儿密钥)
兜底:无启用 provider 时 fallback_api_key(SLTERM_AI_KEY → OPENAI_API_KEY,Zeroizing)
测试:test_provider 12s 超时 → ProviderTestOutcome 十态归设置页归 06 消费归本篇枚举
```

### 数学双管线(引擎单点,两消费面)

```
管线 A:终端覆盖层(paint pass归 M3+ 壳)
  02 快照同一锁窗口构造 TextGrid(eg 缝合点 1)
    → scan_visible(&mut TerminalMathState, grid, ...) → Vec<FormulaOverlay>
       (有界扫描契约值归关键类型节;意图分级 O(1) 预过滤;孤儿闭定界符回溯
        MAX_HISTORY_FORMULA_ROWS 512 封顶)
    → prepare_overlays → Vec<Option<PreparedFormula>>(LayoutResolver 注入 eg math_view)
    → LineProjection::update_projection(reflow_inline) —— 与 reflow 重排对齐归 02 事件
    → CoverageMask::build(覆盖区文字抑制)+ plan_overlay_draw → draw_overlays
       壳在 GPUI paint pass 应用;选区经 LineProjection 原子化归 02 选区语义

管线 B:markdown 阅读器(file_editor 渲染 eg 07 / 阅读归本篇 markdown_view)
  parse_markdown(GFM math 构造)→ MathSource 片段
    → measure_math(MathLayoutCache, ...)参与行宽测量 → fit_math_run 过宽缩放
       低于 MIN_READABLE_MATH_PX 回退源码
  管线 B':file_editor 的 TextView 渲染归 07,eg gpui-component fork 数学钩子归 D08-3
    → set_math_renderer 回调归 math_view::register 接线 → 引擎/栅格化/绘制归本篇

共享面:compile_formula(光学补偿单点)/ MathLayoutCache(4MB LRU)/ MathFont 单例
  / MIN_READABLE_MATH_PX 底线——两管线判定一致;任何管线不得独立规范化/解析/排版归边界 6
```

### runtime skills 投放状态机(eg 03 config_guard heal_all 归位归 03)

```
config_guard heal_all(eg 03 锚点) → ensure_runtime_skills
  → runtime_skill_candidates():codex ~/.agents/skills/slterm-runtime + claude 配置目录归 04 资产
  → 逐目标 ensure_runtime_skill:
      目标存在且非空(用户同名 Skill)→ 指纹对账
        指纹 == 本次内容 → Current(跳过)
        指纹 ≠ marker   → Conflict(永不覆盖,保留用户编辑归归 03 managed_files 同族)
        无 marker(用户自建) → 存在即让位归归 03 安装纪律归位归 04
      空目录/自有       → 原子写:先 SKILL.md + openai.yaml 内容,后 .slterm-managed marker
        (崩溃不产生半套误认;双文件联合 SHA-256 归归 03 指纹家族归归 04)
  → 卸载:remove_runtime_skill 只删自己文件,目录空才清归归 03 卸载纪律归归 04
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;每条对应 spec 分片 08 一个采纳点,一行)

**补全引擎(采纳点 1-12)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 1 | `nebula-completions/src/lib.rs` 模块骨架 | `slterm_completions` 归 00-roadmap M1 依赖序 | nushell nu-cli 血统独立引擎,前端只消费最终候选,零改动移植归 core |
| 2 | `matcher.rs` `CandidateMatcher` 三算法 | 全量 | Prefix/Substring 折叠 + Fuzzy nucleo-matcher——弹窗高亮与排序质量全部来源 |
| 3 | `options.rs` `CompletionOptions` 族 | 全量 | 匹配/排序配置族照抄归本篇锚点 |
| 4 | `completer.rs` `Completer::fetch` | 全量 | 可插拔来源的合同归本篇锚点 |
| 5 | `file.rs` `complete_item*`/`expand_ndots`/`escape_path` | color feature 砍归改造节 8 | exact-match 直钻 + 协作式取消 + Windows 盘符分量,回写工程底座 |
| 6 | `directory.rs`/`static_completion.rs` 两内建源 | 全量 | 隐藏文件后置排序细节照抄 |
| 7 | `command_context.rs` `CommandContext`/`ShellSyntax` 四方言 | 本篇锚点 | 只切词绝不执行;per-shell 引用转义合同是回写安全的根归边界 3 |
| 8 | `semantic.rs` `Source` 枚举 + `Context::parse` | 砍 SshHosts/WslDistributions 归改造节 2 | 「describes sources, never performs I/O」模块头注释即纪律归边界 2 |
| 9 | `semantic/git.rs` 参数角色全家 | 全量 | 选项表五属性 + 网络族 refspec 尾保留,纯函数可单测归边界 2 |
| 10 | `semantic/scripts.rs` 位置解析 | 全量 | 只产出 ProjectSelection 描述,不调包管理器不执行项目代码归边界 2 |
| 11 | `semantic/common.rs` 三套别名 + `resolve()` | 砍 SSH/WSL 选项归改造节 2 | Cmd `/` 选项前缀形态照抄 |
| 12 | `command_search.rs` `CommandQuery` | 全量 | 词序无关 + 四级 tier 加权,弹窗历史搜索质量核心归边界 |

**补全应用层(采纳点 13-24)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 13 | `completion.rs` `Session`/`Request` | connections 成员砍归改造节 2 | 补全不上按键热路径的工程基础归数据流节 |
| 14 | `completion.rs` `Cancellation` | 全量 | 文件遍历/历史扫描/git 子进程间统一取消形态归边界 9 |
| 15 | `nebula_history.rs` `NebulaHistory` 本地池 | 单池化归改造节 2;文件归 01 表归本篇消费 | jsonl + BTreeMap range 的 hint/search 双通道是 ghost/popup 数据源归数据流节 |
| 16 | `directory_history.rs` `DirectoryHistory` | 全量 | rank × 四档衰减;「不引入 shell 特定命令」纪律归数据流节 |
| 17 | `git_completion.rs` 有界发现 + `tracking.rs` | is_host guest 分支折叠归改造节 2 | 3s/1MB 双预算 + TTL + generation 代次,数据发现与语义描述分离形态归边界 2 |
| 18 | `completion/project_scripts.rs` + `workspace.rs` | 全量 | 静态读 package.json;must-not-run 断言随迁归测试点清单 |
| 19 | `display/command_completion.rs` PATH 探针 + `extract_program` | 全量改 SLTERM_GHOST_MAX 归本篇 | 程序身份归一与 AI assistant 判定表、侧栏图标归 03 共用同一份归缝合点 2 |
| 20 | `display/suggest_engine.rs` `calculate`/`suggest_collect`/`suggestion_key`/`semantic_candidates_at` | 自由函数下沉归 suggest.rs归改造节 4 | Inline 三级回退 + Popup 多候选合并;「8 是视口行数不是数据上限」注释承载教训归数据流节 |
| 21 | `CompletionStyle` 三态 | 本体归 06 再导出归本篇 | Inline/Popup/Hybrid 语义照抄归边界 10 |
| 22 | `completion/paths.rs` 路径来源复用 | 全量 | 语义与裸行共用遍历;shell 转义只由 command_context 负责的边界归边界 3 |
| 23 | `display/completion.rs` `nebula_commit_line` 提交纪律 | 归壳 eg 02 事件链归 M3+ | screen_line 优先于按键重建;「laudeclaude」拼接垃圾教训归数据流节 |
| 24 | `display/suggest_engine.rs` `SuggestEnv` 环境抽象 | 收敛 Local 单归改造节 2 | 「走错机器不是补不出来,是补出另一台机器上的路径」注释保留本地形态归改造节 2 |

**AI assistant(采纳点 25-35)**

| # | pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- | --- |
| 25 | `ai_assistant.rs` 触发链 + `event.rs::maybe_request_ai_fix` | GPUI 形态归改造节 8 | 便宜门链跑在每次命令失败上,任何一门不过安静归数据流节 |
| 26 | `should_suggest` 规则表 + 三词表 | 全量归本篇锚点 | 「用户停了它」不是「它失败了」;规则表出厂钉死归测试点清单 |
| 27 | `event.rs::grid_output_tail` | 消费 Grid归 02 篇归本篇 | 光标上 24 行 + 2000 字符封顶 + 跳 WIDE_CHAR_SPACER;终端网格是权威源归数据流节 |
| 28 | `redact_secrets` | 全量归本篇锚点 | 40+ base64/hex 打码——隐私闸门三件套之一,出厂默认写死归边界 4 |
| 29 | `is_dangerous` 词表 + `parse_fix` 合同 | 全量归本篇锚点 | 模型判意图、词表判字面,按位 OR 单方不可被骗归关键类型节 |

| 30 | `AssistantConfig` + 独立配置文件 | 形态归 D08-1归本篇 | 默认关;独立于设置主文件的理由注释随迁归改造节 9 |
| 31 | `ai_providers.rs` 元数据/凭据体系整体 | 凭据域归 10归本篇红线归边界 5 | 引用字段 + 尾四掩码 + 先删凭据再删元数据归数据流节 |
| 32 | `ProviderKind` 十三家枚举全家 | 全量归本篇锚点 | slTerminal 面向 27 家 AI CLI 调优,provider 面不裁剪归边界 |

| 33 | `ai_assistant.rs::send_model_request` 四协议族 | 全量归本篇 | Bearer / x-api-key / x-goog-api-key / Azure api-key；全局 30s 超时照抄 |

| 34 | `ai_assistant.rs::fallback_api_key` 环境变量回退链 | 归本篇（01 表 C 节未列，属本域新增映射） | `NEBULA_AI_KEY`→`SLTERM_AI_KEY`；`OPENAI_API_KEY` 直通不动 |

| 35 | 建议条 UI 与 `Ctrl+.` 接线形态 | 全量归本篇；键位细节归 11 篇 | paste-only 语义（不自动执行）照抄；展示组件 GPUI 呈现形态见改造节 4 |

### 数学渲染

| # | 照抄对象（pebrel 符号 + 文件路径） | 改造归属 | 备注（预算/常量/纪律随迁） |
|---|-------------------------------------|----------|------------------------------|
| 36 | 数学引擎 parser/validate 层（`nebula_app/src/math/`） | 全量照抄 | `pulldown-latex` 锁 0.7.1；`FORBIDDEN_COMMANDS` 宏展开前黑名单照抄 |
| 37 | `compile_formula` / `compile_formula_source` / `normalize_formula_source` | 全量照抄 | IR→layout 编译链与源归一化不变 |
| 38 | layout 层五值类型 + optical 常数族 | 全量照抄 | `OPTICAL_SCALE`/`MIN_SCRIPT_SCALE`/`MIN_SCRIPT_SCRIPT_SCALE`/`MIN_READABLE_MATH_PX` 等照抄 |
| 39 | `FONT_BYTES` 字体内嵌 | 全量照抄 | LatinModernMath.otf 约 716KiB `include_bytes!`；二进制资产随迁打包链 |

| 40 | rasterizer/bitmap 层 | 全量照抄 | glyph 512px 上限、gamma 0.75、bitmap 8192px/24MB 预算照抄 |
| 41 | `MathLayoutCache` LRU | 全量照抄 | `LAYOUT_CACHE_BUDGET` 4MB；`FormulaCacheKey` 键形态照抄 |
| 42 | `MathLimits` 八道预算 + `DEFAULT_LIMITS` | 全量照抄 | 16KB/64/8192/4096/1024/1024/8192 契约值随迁为设计决策 |
| 43 | terminal_math 状态与扫描（`display/terminal_math/scan`） | 全量照抄 | `TerminalMathState` 与扫描常量族照抄；可见公式 64/持久 2048/1MB 等预算不变 |

| 44 | overlay 三件套 `prepare_overlays`/`plan_overlay_draw`/`draw_overlays` + `LayoutResolver` | 全量照抄；呈现细节归 11 篇 | paint-pass 替换可见定界符 span；终端仍是真值纪律见边界节 |
| 45 | 同裁定规则 `LineProjection` + `CoverageMask` | 全量照抄 | 终端/覆盖层同裁定不变；`MAX_ABSORBED_BLANK_ROWS` 等常数照抄 |
| 46 | markdown 自含文档模型（`markdown/mod.rs`、`markdown/parser.rs`） | 全量照抄 | `MathMode`/`MathSource`/`FormattedTextLine::DisplayMath`/`FragmentContent::Math` 照抄 |
| 47 | markdown 排版适配 `MathRun`/`measure_math`/`fit_math_run` | 全量照抄 | 自含排版度量，不依赖宿主编辑器形态 |

| 48 | TextView 数学钩子薄桥（`gpui_shell/math_view.rs`） | 归 D08-3 决策 | `register`/`set_math_renderer` 薄桥照抄；fork 补丁集随迁范围待定 |
| 49 | scientific_corpus 语料（`ai_hook/local/scientific_corpus.rs`） | 照抄；正式归置归 11/12 篇 | `DOCUMENTS`/`Case`/`read_document` 回归语料随迁，落位登记为开放问题 |

### runtime skills 与 slTerminal 现状对照

| # | 照抄对象（pebrel 符号 + 文件路径） | 改造归属 | 备注 |
|---|-------------------------------------|----------|------|
| 50 | runtime skills 投放状态机（`ai_hook/local/runtime_skills.rs`） | 全量照抄 | SHA-256 指纹（SKILL.md+openai.yaml）、三态 marker、先内容后 marker 原子写、双目标（codex/claude）全保留 |
| 51 | slTerminal 现状对照 | 不适用 | slTerminal 无对应实现，全部新建；与 04 篇 SKILL.md 边界约定见改造节 5 |

### 改名映射引用

本片不重定义改名映射，全部消费 01 篇单点表。本片涉及的映射条目：

- crate 族：`nebula-completions`→`slterm_completions`、应用层 `nebula_*` 模块→`slterm_*` 模块（01 表 A 节）。
- 类型/函数符号：`NebulaCompletionKind`→`SltermCompletionKind`、`NebulaCompletionItem`→`SltermCompletionItem`、`NEBULA_GHOST_MAX`→`SLTERM_GHOST_MAX`（01 表 B 节）。
- 存储文件名：`pebrel_history.jsonl`→`history.jsonl`、`pebrel_providers.json`→`providers.json`（01 表 C 节）。
- 环境变量：`NEBULA_AI_KEY`→`SLTERM_AI_KEY`；`OPENAI_API_KEY` 为生态直通变量，不动（01 表 C 节）。

**与 spec 分片 08 的命名偏差**：spec 分片 08 采纳明细中示例文件名为 `slterm_history.jsonl`/`slterm_providers.jsonl`，与 01 表 C 节的 `history.jsonl`/`providers.json` 不一致。按「01 表为唯一权威」原则，本片一律采用 01 表命名，偏差回改登记为开放问题 4。

**本域新增映射**（01 表未列、由本片登记）：`NEBULA_AI_KEY`→`SLTERM_AI_KEY` 属环境变量域新增条目，若 01 表 C 节后续增补本片条目则收敛回单点表。

### 缝合点

| 缝合对象 | 本片侧接口 | 对侧归属 |
|----------|------------|----------|
| 终端网格与滚动（渲染快照输入） | `grid_output_tail` 提取、`RenderSnapshot` 消费 | 02 篇 |
| OSC 133;D 退出码事件 | AI 触发闸门入口事件 | 03 篇（事件归集） |
| hook 失败边通道 | 触发链默认接入 `assistant_answer` 投影 | 03 篇（`config_guard` 钩）+ 本片改造节 9 |
| AI CLI 语义元数据 | `command_hint`/`extract_program` 消费 AgentKind | 03 篇 |
| runtime skills 边界 | SKILL.md 内容边界 + `openai.yaml` 指纹 | 04 篇 |
| 面板/标签标识 | `PaneId`/`TabId` 穿透补全会话与建议条 | 05 篇 |
| 设置接线 | `CompletionStyleName`/`RuntimeSettings` 消费 | 06 篇 |
| markdown 编辑器 | `parse_markdown`/`measure_math` 供 07 篇编辑器调用 | 07 篇 |
| 凭据域 | `credential_target`/`save_api_key` 等仅引用字段 | 10 篇（凭据细则唯一归属） |
| GPUI 呈现与键位 | ghost/popup/建议条组件与 `Ctrl+.` 接线 | 11 篇 |
| 测试基建 | L1 测试迁移与豁免登记 | 12 篇 |

## 改造 / 移植 / 新建设计

### 1. 化学/生物渲染消亡清单

pebrel 的科学渲染面除数学外还含化学式与生物序列两个子族（各自 parser/渲染器/字体资产）。spec 分片 08 采纳明细中该两族不采纳，本片登记消亡清单：

- 化学渲染：分子式/反应式 parser、专用渲染器、化学字体资产——不迁。
- 生物渲染：序列（碱基/氨基酸）解析与高亮渲染——不迁。
- 数学引擎中与科学语料共用的 infra（validate 预算、rasterizer、cache）保留，仅裁剪语料入口。
- scientific_corpus 语料集中化学/生物条目一并消亡，仅保留数学条目回归语料（照抄清单 #49）。

### 2. WSL/SSH 历史池砍除后补全源收敛

pebrel 补全应用层的 fish-style 历史与目录历史按连接形态分池（local/wsl/ssh）。slTerminal 单进程本机形态下连接分池失去意义，收敛为单池：

- `HistoryScope` 仅保留 `Local` 变体；远程作用域枚举整体消亡。
- `SltermHistory` 单例（jsonl + `BTreeMap` 前缀范围索引），`HISTORY_MAX` 5000 不变。
- `DirectoryHistory` frecency 单例（`MAX_ENTRIES` 2048、`MAX_TOTAL_RANK` 10000 不变）。
- `SuggestEnv` 收敛为单态（无远端 env 快照）；`Execution` 收敛为本地进程形态，「静态只读、绝不执行」纪律（project_scripts 1MB/2s TTL）原样保留。
- git_completion 有界发现（3s/1MB 预算、2s TTL、代际失效）原样保留。

### 3. 九种额外语言砍除对补全方言的影响

spec 分片 08 不采纳九种额外 CLI 语言的一等支持。对本片的影响：

- `ShellSyntax` 保持 Posix/PowerShell/Cmd/Literal 四方言，不新增方言变体。
- 27 家 AI CLI 的命令语义经由 `CommandQuery` 词级模糊分层与 `command_hint` 语义源覆盖，不走方言扩展。
- Posix 方言在纯 Windows 形态下的去留归开放问题 1；砍除则 `ShellSyntax` 收敛为三方言 + Literal。

### 4. ghost / popup / 建议条的 GPUI 呈现形态

pebrel 中 ghost 文本与补全 popup 的绘制经 GPU 渲染层内联呈现。slTerminal 保持三态交互（`CompletionStyle` Inline/Popup/Hybrid）不变，呈现层约束为 GPUI 组件形态：

- ghost：行内幻影文本，随 `suppressed_line` Esc 语义隐藏；`SLTERM_GHOST_MAX` 96 字符预算不变。
- popup：候选浮层，`POPUP_LIMIT` 256、`LABEL_MAX` 44 `elide_left` 截断不变。
- 建议条：AI 修复建议以底栏条呈现，`Ctrl+.` paste-only 接线不变。
- 三者的组件实现、焦点/键位裁决归 11 篇；本片只承诺 `semantic_candidates_at` 字节 span→edit 契约与 screen_line commit 纪律作为组件输入。
- 绘制输入统一消费 `RenderSnapshot`（02 篇锚点），本片不自定义网格形态。

### 5. runtime skills 投放与 04 篇 SKILL.md 边界

- 本片侧：投放机制全保留——SHA-256 指纹（SKILL.md + openai.yaml）、三态 marker（Installed/Current/Conflict）、先写内容后写 marker 的原子序、单一 `.slterm-managed` 标记（旧式多标记裁掉）、codex 与 claude 双目标目录。
- 04 篇侧：SKILL.md 内容形态与边界归 04 篇；本片仅消费其内容产物做指纹。
- 对侧调用：heal_all 经 03 篇 `config_guard` 钩挂接，投放时机不新增独立触发器。

### 6. 数学渲染与 07 篇 markdown 编辑器缝合

- 本片提供双管线共享的引擎面（parser→IR→layout→rasterizer→cache）与 markdown 自含文档模型（`MathMode`/`MathSource`/`MathRun`）。
- 07 篇 markdown 编辑器组件消费 `parse_markdown`/`measure_math`/`fit_math_run`，不重新实现排版度量。
- TextView 数学钩子薄桥（`math_view::register`/`set_math_renderer`）随迁范围归 D08-3；若 fork 不随迁，则 07 篇编辑器改走本片自含模型直渲染，接口已足以支撑。
- terminal_math overlay（管线 A）与 markdown 数学（管线 B）共享引擎但状态独立，互不穿透。

### 7. ai_providers 与 10 篇合流

- 本片保留：provider 元数据面——`ProviderKind` 十三家、`PRESETS`、`AiProvider` 引用字段（`api_key_set`/`api_key_hint` 尾四掩码）、`normalize`、`test_provider`/`ProviderTestOutcome` 十七态。
- 10 篇独有：凭据域细则——`credential_target`（`Slterm/AI/<id>`）、Windows Credential Manager 读写、`Zeroizing` 纪律、凭据不出凭据域的类型级隔离。
- 本片经 10 篇暴露的凭据接口调用，不直接触碰凭据存储；`remove_provider` 先删凭据再删元数据的顺序纪律经 10 篇接口兑现。
- 发送协议四族（OpenAI Bearer/Anthropic x-api-key/Google x-goog-api-key/Azure api-key）归本片 `send_model_request`；10 篇不重复实现。
- `fallback_api_key` 环境变量回退链（`SLTERM_AI_KEY`→`OPENAI_API_KEY`）归本片；凭据文件读写路径 eg 10 篇。

### 8. color feature 签名收敛

terminal_math 的 overlay 绘制消费带色 span。收敛约定：

- 颜色形态一律消费 02 篇 `RenderSnapshot` 的 color 表达，本片不定义新颜色类型。
- `draw_overlays` 的绘制参数签名直接以 02 篇快照类型入参；pebrel 侧若有私有 color 包装，随快照类型一并收敛。
- 配色单点纪律（主题 token）归 06 篇；本片组件不硬编码颜色。

### 9. AssistantConfig 改造 + legacy 通道砍除

- 默认关（`enabled=false`）与独立于设置主文件的形态照抄 pebrel；其终态落位（独立文件 vs 并入 06 篇设置树）归 D08-1。
- legacy 通道砍除：pebrel 若存在旧版配置路径/环境变量回退读取（除 `SLTERM_AI_KEY`/`OPENAI_API_KEY` 生态直通变量外），一律不迁；启动只读当前形态。
- hook 失败边并入 AI 触发链：03 篇 `config_guard` 钩失败边默认取 `assistant_answer` 投影进入本片触发闸门（闸门后于 COOLDOWN 与 `AssistantConfig::load`），归 D08-2。
- `should_suggest` 规则表、危险词表（21 条 OR 模型危险标记）、`USER_ABORT_CODES`、`BARE_TOOLS`/`INTERACTIVE` 表原样照抄，不裁剪。

### 10. 测试迁移登记

- 本片域内 pebrel L1 测试（补全三算法、frecency、有界发现、should_suggest、redact_secrets、parse_fix、危险词表、math IR/layout/rasterizer/cache、scan 有界、LineProjection、provider 预设/掩码、runtime skills 指纹）随 crate 迁移，具体豁免与落位归 12 篇。
- 红线：测试与文档仅允许假值占位符（`sk-test` 形态），真实凭据禁入 git 追踪文件（SEC-18，根 CLAUDE.md 纪律）。
- 禁名（`test_` 前缀、规格编号前缀）归 01 篇命名纪律，本片不重复定义。

## 测试点清单

本片域内须覆盖的测试点（层级归 12 篇分配，本片只列点）：

| 测试点 | 断言要点 |
|--------|----------|
| 补全三算法 | Prefix/Substring 不评分大小写折叠；Fuzzy 走 nucleo-matcher；`CandidateMatcher` 分桶序稳定 |
| frecency 历史 | `DirectoryHistory` 排名衰减与 `MAX_ENTRIES` 上限；`SltermHistory` 前缀范围命中 |
| git_completion 有界发现 | 3s/1MB 预算截断、2s TTL 命中、代际失效触发 |
| project_scripts 静态纪律 | 1MB 截断 + TTL；断言只读不执行 |
| `should_suggest` 规则表 | 各规则逐条真/假例 |
| `redact_secrets` | base64/hex 40+ 形态全部替换为 `[redacted]`，无误伤 |
| `parse_fix` 抗噪 | 非 JSON 包裹提取、空/坏输入不 panic |
| 危险词表 | 21 条逐条命中 + 模型危险标记 OR 语义 |
| 触发闸门 | COOLDOWN 5s、默认关、`USER_ABORT_CODES` 全集 |
| math IR/layout | 光学常数族契约值、script 缩放下限 |
| rasterizer | 512px glyph 上限、gamma 0.75、bitmap 24MB 预算 |
| `MathLayoutCache` | 4MB LRU 逐出、键归一化等价 |
| scan 有界 | 可见公式 64 / 持久 2048 / 1MB / 行预算全部截断路径 |
| `LineProjection` | 终端/覆盖层同裁定等价性 |
| corpus 回归 | scientific_corpus 数学条目逐例渲染不回归 |
| provider 预设 | 十三家 `PRESETS` 字段完整性；`api_key_hint` 尾四掩码 |
| runtime skills 指纹 | SKILL.md/openai.yaml 变更触发 Conflict；先内容后 marker 原子序 |
| `fallback_api_key` | `SLTERM_AI_KEY`→`OPENAI_API_KEY` 回退序 |

禁名纪律（`test_` 前缀、规格编号前缀）与测试命名归 01 篇，本片不重复定义。

## 阶段归属与出口标准

- crate 壳 `slterm_completions` 依赖序迁入归 00-roadmap M1（只建壳，不装填）。
- 本片领域完形归 M9，按下列子步拆分；每步出口标准机器可验：

| 子步 | 内容 | 出口标准（机器可验） |
|------|------|------------------------|
| M9.1 | 补全引擎 + 应用层（源收敛后单态形态） | `cargo test` 本片补全用例全绿；`ShellSyntax` 四方言编译通过；源检索无 remote/ssh 池残留符号 |
| M9.2 | AI assistant 触发链 + provider 元数据面 | `should_suggest`/闸门/`redact_secrets`/`parse_fix` 用例绿；十三家 `PRESETS` 完整性断言绿；凭据读写经 10 篇接口的 mock 断言 |
| M9.3 | 触发链合流（D08-2 落地） | 03 篇 `config_guard` 失败边进入闸门的集成用例绿 |
| M9.4 | 数学引擎 + terminal_math overlay | math IR/layout/rasterizer/cache 用例绿；scan 有界与 `LineProjection` 同裁定用例绿；corpus 回归零 diff |
| M9.5 | markdown 数学 + runtime skills + 建议条呈现接线 | markdown 管线用例绿；runtime skills 指纹/原子序用例绿；`Ctrl+.` paste-only 与三态交互接线经 11 篇用例绿 |

- 每子步出口前须过静态门禁四件（tsc/eslint/clippy/rustfmt 中适用者）与全量 L1 回归；命名禁名与凭据红线检查随门禁。

## 待沉淀决策

> **[待沉淀 D08-1] AssistantConfig 终态形态（独立文件 vs 并入 06 篇设置树）**
> 难逆：存储形态一旦分流，回并需迁移既有用户文件；意外：双源配置漂移——用户在一处改 enabled 另一处不知；真实权衡：独立文件便于 AI 辅助面独立演进/整体禁用，并入设置树则单点持久化、与 `CompletionStyleName` 等设置同一读写路径。默认照抄 pebrel 独立文件形态，M9.2 前须拍板，拍板前不实现设置树接线。

> **[待沉淀 D08-2] hook 失败边默认并入 AI 触发链**
> 难逆：默认接入后改默认关会破坏已依赖该行为的用户预期；意外：钩失败频发场景下建议条噪声放大，用户可能把钩故障归因于 AI 辅助；真实权衡：失败即提示降低排障门槛 vs 噪声与误归因。M9.3 落地时拍板默认开/关，拍板前仅实现通道不接线。

> **[待沉淀 D08-3] gpui-component fork 补丁集随迁范围**
> 难逆：随迁即长期背负 fork 维护与上游漂移成本，不随迁后补成本高；意外：补丁 3（TextView GitHub 化）可能引入本片不需要的行为面；真实权衡：TextView 数学钩子直用 vs 本片自含文档模型已足以支撑 markdown 渲染。M9.4 前拍板，若随迁需登记补丁清单与回采纪律。

## 开放问题

1. **Posix 方言去留**：WSL/SSH 砍除后，`ShellSyntax::Posix` 在纯 Windows 形态下服务对象仅剩 Git Bash/MSYS 类场景；保留与否影响 `complete_item` 分方言路径的测试基数。归本片后续评审。
2. **数学语料正式归置**：scientific_corpus 数学条目的最终落位（测试资产目录 vs 独立 crate）归 11/12 篇评审时定。
3. **pwsh PSReadLine Prediction 与 ghost 并存**：pwsh 自带预测补全与 ghost 幻影文本在视觉与键位上的裁决归 02/03 篇联评；本片承诺 `suppressed_line` 语义可供其复用。
4. **spec 分片 08 命名偏差回改**：spec 分片 08 示例文件名与 01 表 C 节不一致（见「改名映射引用」节），回改 spec 分片归 M11 文档统一性 pass。
