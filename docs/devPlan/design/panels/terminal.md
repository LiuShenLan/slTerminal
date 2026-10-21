# 终端面板

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../../spec-issues.md。

> 终端面板是面板类型封闭集成员，持有唯一 PTY 会话、终端网格与终端交互；工作区壳拥有焦点、关闭守卫、布局与页签呈现；设置中心只提供终端配置控件。需求真值源为 `docs/featureSpec/panels/terminal.md`。

### 复用与重建边界

| 资产 | 处置 |
|---|---|
| `slterm_terminal`：`Term`、`Grid`、VT/OSC 解析、`EventLoop`、`tty`/ConPTY、`selection`、`vi_mode`、`render` 快照、`event` 事件族 | 保留。需要扩展：PTY 容量契约 32 → 64；OSC 52 空选择器接受；渲染快照携带 OSC 8 链接跨度；新增向回滚区追加会话分隔标记的接口；输出事件保留“已收到非空字节”的可观测信号 |
| `gpui_terminal::session`（`TerminalSession`、`EventProxy`、`spawn`）、`event_mailbox`、`session_pump` | 保留并适配：spawn 移到后台执行体；会话附加改为可替换，支持重连 |
| `gpui_terminal::keymap`、`mouse_protocol`、`cursor_painter`、`colors`、`copy_feedback` | 保留。按键分发层重写：保留编码器零特例，删除语义/整行选区与终端右键菜单 |
| `gpui_terminal::element`、`view` | 适配。`PaneId` 改 `PanelId`；字号、闪烁、回滚、OSC 策略从全局设置注入；链接命中、悬浮与打开策略外置；尺寸控制替换为本文状态机；运行期状态与 UI 覆盖层移入面板壳 |
| `gpui_terminal::{broadcast, completion, math_overlay, math_render}`、`terminal_math` | 作为范围外增强保留，迁回时挂到终端增强端口；迁移前不进入本轮验收 |
| `terminal_registry` | 重建：键从 `PaneId` 改为 `PanelId`，扩展项目索引、CLI 会话、标题、工作目录与前台进程查询；仍是 PTY 元数据唯一入口 |
| 旧 `TerminalPane`、tab 内分屏、`SltermWorkspace` 终端分支 | 废弃 |

### 模块树

```text
领域 core crate（crate 名以 architecture/overview 为准）
  panels/terminal/
    mod.rs
    params.rs          # 面板参数与编号
    session_state.rs   # 连接/运行/退出/失败状态机
    title.rs           # 标题优先级与重命名校验
    numbering.rs       # 项目级编号分配器
    cwd.rs             # 工作目录继承、校验与去抖
    cli.rs             # CLI 会话、别名、四态聚合
    paste.rs           # 粘贴守卫纯函数
    osc.rs             # OSC 8/52 策略纯函数
    restore.rs         # 历史会话恢复注入状态机
    error.rs           # TerminalError 与用户文案映射

slterm_app/src/gpui_terminal/     # 保留的渲染/输入/会话引擎
slterm_app/src/panels/terminal/
  mod.rs
  panel.rs           # TerminalPanel：模型 + 子视图 + 覆盖层
  spawn.rs           # 后台 spawn、能力预检、Shell 探测
  reconnect.rs
  links.rs           # 链接命中、悬浮与系统分派
  clipboard.rs       # OSC 52 与复制粘贴的串行通道
  rename.rs          # 页签重命名弹窗
  action_bar.rs      # 连接遮罩、退出/失败操作栏
  metadata.rs        # 标题/状态向导航树与会话行的投影
```

`TerminalView` 保留在 `gpui_terminal`，只拥有网格、输入、选区、光标与渲染；`TerminalPanel` 是新面板壳，拥有编号、标题、CLI 会话、生命周期、持久化与覆盖层。两者通过事件与只读查询衔接，避免把工作区状态塞回渲染视图。

### 关键类型

```rust
pub struct TerminalPanelParams {
    pub revision: u32,
    // 编号/自定义标题/最近 cwd 归 TerminalBook，持久化在 terminals.json；此处不复制。
}

pub struct TerminalPanelModel {
    pub id: PanelId,
    pub project_id: ProjectId,
    pub params: TerminalPanelParams,
    pub session: SessionState,
    pub cli: Option<CliSession>,
    pub title: TitleState,
    pub restore: Option<PendingRestore>,
    pub visible: bool,
    pub focused: bool,
}

pub enum SessionState {
    Connecting { overlay_visible: bool, queued_input: InputQueue },
    Running { shell_pid: u32 },
    Exited { code: Option<i32>, final_message: String },
    Failed { error: TerminalError, retryable: bool },
    Closing,
}

pub struct CliSession {
    pub agent: AgentKind,
    pub source: CliSource,            // Hook | Screen | CommandToken
    pub status: CliStatus,            // Working | Attention | Done | Error
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub last_title_update: Instant,
}

pub enum TitleState {
    Custom(String),
    Cli { title: String, agent: AgentKind },
    Restored { title: String },
    Default { number: u32 },
}

pub struct SpawnPlan {
    pub shell: ShellChoice,           // Auto | Explicit(ShellId) | Custom(PathBuf)
    pub cwd: PathBuf,
    pub size: WindowSize,
    pub scrollback: usize,
    pub restore_command: Option<ValidatedRestoreCommand>,
}

pub enum TerminalError {
    ShellUnavailable { candidates: Vec<ShellFailure> },
    CwdRejected { path: PathBuf, reason: CwdReject },
    Capacity { limit: usize },
    SpawnFailed { shell: Option<PathBuf>, reason: String },
    InputSendFailed { reason: String },
    ClipboardUnavailable { reason: String },
    LinkRejected { scheme: String },
    RestoreFailed { session_id: String, reason: String },
}
```

测试接缝：`TerminalBackend` 抽象 spawn/reconnect/shutdown；`TerminalClock` 提供去抖与超时；`ClipboardSink`、`LinkOpener`、`NotificationSink` 注入假实现。纯函数（标题、编号、粘贴守卫、OSC 策略、尺寸合并）不依赖 GPUI。

### 面板实体与 PTY 生命周期

面板组成：

- `TerminalPanel` 持有 `TerminalPanelModel`、`Entity<TerminalView>`、`Entity<TerminalActionBar>`、重命名弹窗状态、待注入恢复命令与保存队列。
- 一个面板最多一个活动 PTY；`TerminalView.session` 在面板存活期间可替换，但不会同时存在两个会话。
- 面板隐藏只改 `visible`，会话泵、PTY 读取与解析继续运行；`TerminalView` 只在可见时 `notify`。隐藏项目的输出不丢字节。

创建时序：

1. 工作区提交 `NewTerminalRequest { project, target_group, cwd_source, restore }`。
2. 面板分配编号，解析工作目录与 Shell 计划；PTY 容量预检在创建面板实体前完成，失败直接提示，不产生空面板。
3. 创建 `TerminalPanel` 与无会话的 `TerminalView`，显示连接遮罩。
4. 等待容器尺寸就绪，最长 500ms；超时以 80×24 创建。尺寸就绪后立即适配。
5. 在后台执行体调用 `TerminalBackend::spawn`；成功附加会话并启动事件泵，失败进入 `Failed`。
6. 首个非空输出到达时进入 `Running`、隐藏遮罩、按序发送缓存输入；1500ms 内无输出则遮罩转为顶部“正在连接...”状态。

```rust
trait TerminalBackend {
    fn spawn(&mut self, panel: PanelId, plan: SpawnPlan) -> Result<SessionHandle, TerminalError>;
    fn reconnect(&mut self, panel: PanelId, size: WindowSize) -> Result<SessionHandle, TerminalError>;
    fn shutdown(&mut self, panel: PanelId, deadline: Duration);
}
```

状态迁移：

| 当前 | 触发 | 目标 | 行为 |
|---|---|---|---|
| 连接中 | 首个非空输出 | 运行中 | 隐藏遮罩，冲刷输入队列 |
| 连接中 | spawn 失败 | 启动失败 | 丢弃输入队列；首次弹一次非阻塞提示；显示底部操作栏 |
| 运行中 | 子进程退出 / PTY 故障 | 已退出 | 保留最终画面；清除 CLI 状态与图标；按标题优先级恢复；显示退出码或 `?` |
| 启动失败 | 重试 | 连接中 | 合并重复请求；仅保留一次 |
| 已退出 | 重新连接 | 连接中 | 保留回滚并插入“新会话”分隔标记 |
| 任意 | 关闭面板/项目/窗口 | 结束 | 取消创建、终止子进程、丢弃待注入命令 |

行为约束：

- 连接中键入内容进入 4096 字节有界队列；会话就绪后按序发送；失败或关闭时丢弃。
- 重连使用当前容器尺寸（不可用时以 80×24 兜底）与最近已知目录；目录失效回退项目根。旧进程内存不保留。
- 退出后应用不自行创建新会话；`Enter` 在终端聚焦时触发重连，按钮聚焦时触发按钮。
- 启动失败操作栏文案包含路径与原因；自动探测模式额外提供“改用自动探测重试”。
- 连续发送失败 3 次弹一次输入发送失败提示，成功一次后计数清零。
- 关闭面板时先取消在途 spawn；已产出的子进程必须终止，不能留孤儿。窗口关闭链先冲刷设置与布局，再并发终止全部 PTY；优雅终止 2 秒，终端清理不超过 3 秒。

PTY 容量：

- 应用级上限 64。`PtyBudget::try_acquire()` 在创建面板前原子取得许可；许可随会话销毁归还。core 的 `MAX_PTY_SESSIONS` 与 `ensure_pty_capacity` 同步改为 64，并在 `spawn` 内再次校验，避免 TOCTOU。
- 布局恢复超过上限时按全局顺序连接前 64 个：项目列表顺序 → 项目布局树深度优先（左/上先于右/下）→ 面板组页签顺序；其余进入 `Failed { Capacity }`，关闭其他终端后可重试。
- PTY 并发创建上限 8；界面在全部会话就绪前保持可交互，失败态可重试。
- 窗口进入关闭流程后拒绝新建终端。

Shell 选择：

- `ShellChoice::Auto` 按 `pwsh.exe` → `powershell.exe` → `cmd.exe` 逐个尝试 spawn；某个候选 spawn 失败或在 500ms 稳定窗内立即退出，则尝试下一个；全部失败时汇总每个候选的失败原因。
- `Explicit` 缺文件或启动失败直接失败，不静默切换；提供“改用自动探测重试”。
- `Custom` 只接受可执行文件绝对路径，不提供参数注入。
- PowerShell 不跳过用户 profile；Shell 集成由 core 的启动参数注入。cmd 无命令边界，标题/状态/目录跟踪按可用事件降级。

工作目录：

- 优先级：入口显式目录 > 同项目最近聚焦终端的已知目录（校验后）> 项目根。
- `WorkingDirPolicy::validate(project_root, agent_dirs, path)` 基于解析后的真实路径做允许根包含校验，拒绝符号链接逃逸、设备路径与越界 UNC。
- Shell 内部 `cd` 不受限制；`CwdTracker` 只监听并记录 OSC 7 与标题协议目录，去抖 300ms 写入面板参数。
- 恢复时目录失效回退项目根并提示；项目根不可用时创建失败，不使用应用自身目录。

编号：

- 编号由 `TerminalBook` 按项目分配，从 0 开始；关闭不复用、不重排。
- 编号/自定义标题/最近 cwd 只写 `state/projects/<id>/terminals.json`，运行期权威在 `TerminalBook`；`layout.json` 不保存 `terminal_counter` 或 `number`。恢复时以 `terminals.json` 为准，`next_ordinal = max(next_ordinal, maxSeen + 1)`；编号重复或非法时重新分配并记录一次聚合警告。
- 历史会话恢复同样分配新编号，不继承历史编号。

历史会话恢复：

- 输入为导航树契约给出的 `RestoreSessionRequest { project, target_group, cwd, agent, session_id, title, command }`。
- `ValidatedRestoreCommand` 只接受 `AgentSession::resume_command` 产出的命令；会话 ID 注入前校验格式，禁止任意文本注入。
- 就绪判定优先 `prompt_ready`；PowerShell 系等待最长 10 秒，超时仍注入；cmd 固定等待约 0.5 秒。
- 等待期间显示“正在恢复会话...”，首个输出正常呈现；面板关闭或进程退出时取消注入。
- 失败时前台提示，后台系统通知；其他终端不受影响，不自动重试。

### 渲染、尺寸与滚动

渲染合同：

- `TerminalElement` 在 `Term` 锁内截取不可变 `RenderSnapshot`，放锁后做颜色解析与绘制；锁内不做排版、公式编译或文件 I/O。
- 解析与事件处理与可见性无关。可见终端的小段输出在下一帧呈现；突发输出按帧合并，最长 16ms 提交一次；同帧网格变更原子呈现。
- 输出洪峰时 core 暂停读取 PTY，消化后续读；任何路径不丢字节。跨块 UTF-8 由 core 解析器保证完整。
- 面板隐藏或项目不可见时只跳过 `notify`；重新可见直接呈现最新缓冲，不补放中间帧。
- 渲染器静默降级；`RenderHealthMonitor` 持续低于目标时只提示一次“终端渲染性能可能下降”。

尺寸状态机：

```rust
pub struct ResizeController {
    viewport: Option<TerminalViewport>,
    pending_cols: Option<TerminalViewport>,
    startup_deadline: Instant,
    grid_synced: bool,
}
```

- 初始创建等待容器尺寸最长 500ms；不可用以 80×24 创建，就绪后立即适配并通知 PTY。
- 行数变化立即适配；列数变化合并 100ms 内连续变化后适配一次；行列均不变不发送。
- 字号变化立即适配，不受列去抖影响；先更新本地网格，再向 PTY 发送尺寸。
- 普通屏幕与交替屏幕共用同一路径。容器尺寸为 0 时不动作；重新可见后测量并适配。
- 非法网格值丢弃；适配失败只记日志，不影响输入输出。DPI 变化重新测量并立即适配。
- 网格重排与 PTY 通知顺序：先把已接收未解析字节按旧网格解析，再以 `Msg::ResizeGrid` 让本地网格 reflow，最后 `ResizePseudoConsole`；UI 线程不得直接锁 `Term` 做 resize。
- 启动稳定闸保留：首帧几何命中 spawn 网格时零下发；500ms 宽限期后按实际视口纠正一次。

滚动缓冲：

- `Config.scrolling_history` 来自终端页设置；新建会话使用当时值，已有会话不变；默认 10000，范围 1000–100000。
- 交替屏幕内容不进入回滚。宽度变化时软换行重排，硬换行不重排。
- 切换面板/项目保留各自滚动位置；原在底部则继续跟随新输出，否则保持。
- 不提供可见滚动条与滚动锁定。滚轮默认滚动本地回滚；前台程序启用鼠标跟踪时交给程序；`Shift+滚轮` 强制本地。
- 固定键：`Shift+PageUp/PageDown` 翻页，`Shift+Home/End` 到顶/到底；交替屏幕下不拦截，交给前台程序。

字体与光标：

- 终端字号全局共享，范围 8–32，默认 14；字体家族来自外观页；缺字体回退主题等宽栈。所有终端立即同步。
- `Ctrl+滚轮` 每格 1px，越界不变化；只作用于指针所在终端，但改变全局值。
- 字号/字体变更后 2 秒去抖保存；关闭窗口前强制冲刷。编辑器字号独立。
- 光标固定竖线；闪烁由终端页开关控制，系统“减少动效”时默认关闭，用户显式开启可覆盖。IME 合成、窗口失焦或面板失焦时暂停闪烁。

### 输入、选区与剪贴板

按键分发顺序（`TerminalView::on_key_down`）：

1. IME 合成中或系统窗口快捷键：不拦截。
2. 终端固定键：`Ctrl+Shift+C` 复制；`Ctrl+Shift+V`/`Shift+Insert` 粘贴；`Ctrl+Enter` 写入 LF；`Shift+滚轮` 已由指针路径处理。
3. 增强层（vi/补全/AI 修复条，迁移后启用）按键拦截。
4. 工作区快捷键透传：`Ctrl+W`、`Ctrl+Tab`、`Ctrl+Shift+Tab`、`Ctrl+,`、`Ctrl+Shift+*` 面板/侧栏命令不编码进 PTY。
5. 前台程序键序编码：`keymap::encode_for_program`；可打印字符与 IME 提交文本走 `EntityInputHandler`。
6. `Ctrl+C` 不在快捷键层拦截，由编码器发送 0x03；发送前若 CLI 状态为“工作”则本地改为“注意”。

行为：

- `Ctrl+C` 保留为中断，重复触发状态幂等；`Ctrl+Shift+C` 无选区也消费按键但不改剪贴板。
- `Ctrl+Enter` 发送 0x0A，不发送 0x0D；接收方语义由前台程序决定。
- `F2` 仅在页签本身聚焦时进入重命名；终端输入区聚焦时原样发给前台程序；CLI 会话运行中禁止重命名。
- 输入法合成期间不触发任何快捷键；合成确认不额外发送 Enter。合成文本按一次输入计入。
- `Tab` 在终端上下文绑定先于组件库焦点遍历；`Ctrl+W` 不被终端改写。
- 鼠标单击聚焦并清除选区；拖拽建立简单选区；不提供双击/三击语义选区、`Shift` 扩展选区、终端内容右键菜单、全选与 `Ctrl+End`。
- 开始输入清除选区。软换行复制为连续文本，硬换行保留换行；复制成功显示对勾反馈。
- 粘贴：`Ctrl+Shift+V` 与 `Shift+Insert` 等价；前台支持括号粘贴时包裹 `CSI 200~`/`CSI 201~`，并移除载荷内的收尾哨兵；内容按 UTF-8 原样发送，不自动追加 Enter。
- 粘贴守卫在发送前执行：含除 `\t`、`\r`、`\n` 外的 C0 或任意 C1 控制字符时确认；超过 50 行确认并显示行数；超过 10MB 拒绝。确认框不显示剪贴板内容，取消不发送任何字节。
- 复制/粘贴失败显示非阻塞提示；日志与提示不包含剪贴板或终端内容。
- 所有终端剪贴板写入经 `ClipboardSink` 单通道串行执行，最后一次成功写入生效。

### OSC 8 链接与 OSC 52 剪贴板

OSC 8：

- core 的单元格已保留 `Hyperlink`；`RenderSnapshot` 扩展链接跨度（行、列范围、URI），渲染层据此绘制下划线与悬浮命中，不在绘制热路径锁 `Term`。
- 只有程序显式声明的 OSC 8 链接可点击；普通 URL 文本不识别。
- 悬浮显示下划线与工具提示（完整 URL + “Ctrl+单击打开”）。
- `Ctrl+单击` 打开：`http`/`https` 交系统浏览器，`mailto` 交系统邮件程序；其他 scheme 忽略并提示“不支持的链接类型”。不按 `Ctrl` 的单击只移动光标或建立选区。
- URL 长度超过 8KB 忽略；打开异步执行，失败提示“无法打开链接”。

OSC 52：

```rust
pub struct Osc52Policy {
    pub allow_unfocused_in_active_project: bool,
}
pub fn accept_store(selector: Osc52Selector, focused: bool, active_project: bool,
                    visible: bool, len: usize, policy: &Osc52Policy) -> Osc52Decision;
```

- 只写不读；core 配置固定为 `Osc52::OnlyCopy`，永不响应读取请求。
- 仅接受空选择器与 `c`；`p`、`q` 及其他选择器忽略。core 需把空选择器映射到 Clipboard。
- 默认仅当终端输入区有焦点且所属项目为活跃项目时接受；开启“允许非聚焦终端写入剪贴板”后，活跃项目内非聚焦终端可写；隐藏项目终端始终不可写。
- 单条负载超过 1MB 整体忽略，不截断；非法 base64/UTF-8 由 core 静默丢弃。
- 序列处理后从画面消费，不显示控制序列。
- 成功显示“已复制到剪贴板（终端）”；超限且终端聚焦时提示已忽略；失败提示；提示与日志不含内容。
- 多个终端的写入按到达顺序串行。

### 标题、状态与 CLI 识别

标题优先级：用户自定义标题 > 运行中的 CLI 会话标题 > 历史会话标题 > `terminal-N`。

- 未自定义时，PowerShell 命令开始且首 token 精确命中已注册 CLI 命令或用户别名（Windows 不区分大小写）→ 标题切换为该 CLI 固定标题，状态置“注意”，显示 CLI 图标。
- 命令结束 → 按优先级恢复标题，清除状态圆点与图标。
- 普通命令不改变标题与状态。cmd 无命令边界，不自动切换标题。
- 运行中 CLI 会话标题按“AI 标题 > 摘要 > 首个提示词 > 会话 ID 前 8 位”刷新，每 5 秒最多一次。
- 四态：工作（绿）、注意（黄）、完成（灰）、错误（红）；未知 CLI 或未知事件忽略。
- `Ctrl+C` 只把“工作”改为“注意”；其他状态不变。
- 会话结束只清除圆点与图标，不恢复标题；命令结束或进程退出才恢复。
- 自定义标题立即持久化；CLI 标题不得覆盖自定义标题，仅在悬停提示中显示。
- 重命名弹窗：标题“重命名终端”，预填并全选；空/纯空白显示“名称不能为空”；超过 128 字符显示“名称过长”；`Enter` 确认，`Esc`/遮罩/取消不修改；CLI 运行中禁用。
- 页签同时显示可选 CLI 图标（16px，按 `AgentKind` 取资产；无资产不显示）与状态圆点；两者有可读名称，颜色不是唯一信息。
- 标题超长单行省略；悬停显示完整标题、被自定义标题覆盖的 CLI 会话标题与编号。
- 标题、状态、图标实时投影到导航树活跃会话行；激活该行切换到项目并聚焦面板。

CLI 识别来源：

- hook 事件（`slterm_hook` → `ai_hook`）为最高置信度；屏幕规则（`ai_agents::detect`）兜底；进程身份用于恢复与状态对齐。
- `CliSessionTracker` 只消费注册表与 hook 的已知事件；未知会话不产生标题、图标或通知。
- 通知：任务完成/等待输入/权限请求/错误在窗口失焦时由工作区通知服务处理；系统 toast 由 `desktop_enabled` 门控，任务栏闪烁由 `taskbar_flash` 门控；窗口聚焦时不发；同类事件按事件键去重。终端只上报 `AgentNotification { category, agent, project, panel }`，不直接投递系统通知。

### 持久化与关闭守卫

持久化：

- 布局只保存面板类型、所属项目、面板组归属、顺序、激活态与 `revision`；不内联编号或计数器。
- 编号、自定义标题、最近已知工作目录只在 `terminals.json`，由 `TerminalBook` 持有与对账。

不保存：画面、回滚、选区、滚动位置、状态圆点、CLI 图标、运行中标题、CLI 会话元数据、待注入命令、输入队列。

保存时机：自定义标题立即；编号与工作目录去抖 300ms；字号/字体按外观域 2 秒；窗口关闭前冲刷。

状态保留：

| 状态 | 同组切面板 | 切换项目 | 关闭重开 | 重启 |
|---|---|---|---|---|
| PTY/画面/回滚 | 保留 | 保留 | 丢失 | 丢失 |
| 滚动位置 | 保留 | 保留 | 重置底部 | 重置底部 |
| 工作目录 | 保留 | 保留 | 按入口继承 | 恢复保存值，失效回项目根 |
| 自定义标题 | 保留 | 保留 | 无 | 恢复 |
| 编号 | 保留 | 保留 | 新号 | 按 `terminals.json` 恢复 |
| 标题/状态/CLI 元数据 | 保留并更新 | 保留并更新 | 无 | 不恢复 |

关闭守卫：

- 终端向工作区提供 `ForegroundProcessState { None, Running(Vec<ProcessInfo>), Unknown }`；由 shell PID 的进程树查询得出，查询失败归 `Unknown`，按“存在前台进程”处理。
- 单面板关闭、批量关闭、项目移除与窗口关闭由工作区统一确认；终端不自行弹确认。关闭即终止 PTY，回滚丢失且不可撤销。
- 删除项目终止该项目全部 PTY；窗口关闭并发终止全部 PTY，失败只记录日志，由操作系统进程树兜底。

错误映射：

| 条件 | 状态/文案 | 恢复动作 |
|---|---|---|
| 会话创建失败 | 底部“终端启动失败：〈原因〉”；首次一次提示 | 重试、关闭面板；自动模式提供改用自动探测 |
| 进程退出 | 底部“进程已退出（退出码 N/`?`）” | 重新连接、关闭面板；`Enter` 重连 |
| 输入连续失败 | 非阻塞“终端输入发送失败：〈原因〉” | 成功一次清零 |
| 目录越界/不存在/无权 | 创建失败，原因含目录与限制 | 修正目录或改用项目根 |
| 项目根不可用 | 创建失败，不使用应用目录 | 恢复项目目录或移除项目 |
| Shell 不可用 | 创建失败，原因含路径；自动模式列出全部候选 | 修改默认 Shell 或改用自动 |
| 达到 64 上限 | “会话数已达上限（64），请关闭其他终端” | 关闭其他终端后重试 |
| 恢复失败 | 前台提示或后台系统通知，含会话 ID 前 8 位与原因 | 重新恢复，不自动重试 |
| OSC 8 不支持 scheme | “不支持的链接类型” | 复制链接手动打开 |
| OSC 52 超限 | 聚焦时提示已忽略 | 程序减小负载 |

### 范围外增强的扩展点

终端增强不进入本轮规格验收，但面板结构必须预留稳定端口，迁移时不改面板模型：

```rust
pub trait TerminalEnhancement {
    fn handle_key(&mut self, key: &Keystroke, mode: TermMode, cx: &mut EnhancementCx) -> bool;
    fn decorate_snapshot(&mut self, snapshot: &mut RenderSnapshot, cx: &EnhancementCx);
    fn render_overlay(&mut self, cx: &mut EnhancementCx) -> Option<AnyElement>;
    fn enabled(&self) -> bool;
}

pub struct TerminalEnhancements {
    pub vi: Option<ViController>,          // Term::toggle_vi_mode / vi_motion
    pub completion: Option<CompletionController>, // suggest::SuggestState + completion::Session
    pub math: Option<MathOverlayController>,      // math_overlay / math_render
    pub ai_fix: Option<AiFixController>,          // ai_assistant::AiFixState
}
```

- 开关全部来自应用页“终端增强”，默认值见 `design/settings/app-page.md`。关闭时 `enabled=false`，不拦截按键、不扫描公式、不渲染覆盖层。
- vi 与补全只在终端上下文生效；全屏 TUI/交替屏幕下补全不弹出、vi 不进入。
- 数学覆盖层只消费不可变快照，不修改网格、选区或复制文本。
- AI 修复条只提交建议到输入区，不自动执行；打开前必须经过粘贴守卫同级的控制字符与危险命令检查。
- 增强层顺序固定：Vi → 补全 → AI 修复条，避免 Esc 语义互相抢占；与 `design/out-of-scope/terminal-enhancements.md` 一致。

### 测试接缝

- 纯函数：标题优先级、CLI 首 token 识别、编号分配、工作目录校验、取消/注意状态迁移、粘贴守卫、OSC 8/52 策略、尺寸合并。
- 状态机：以假 backend/假时钟覆盖连接、首个输出、1500ms 遮罩降级、退出、失败重试、重复重连合并、关闭取消、恢复注入就绪与超时。
- PTY 集成：spawn/写入/读取/resize/关闭全链、64 容量原子抢占、UTF-8 跨块、退出码、进程树终止。
- 虚拟窗口：遮罩与操作栏焦点与 `Enter` 语义、重命名校验、链接悬浮与 Ctrl+单击、粘贴确认框不泄露内容、字体变化不重启进程。
- 人工豁免：真实中文输入法、Win10 早期控制台降级、GPU/软件渲染观感。
