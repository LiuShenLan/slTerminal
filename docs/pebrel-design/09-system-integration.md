# 09 系统集成详细设计

> pebrel-design 分片 09/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/09-system-integration.md`(spec 分片，含采纳点编号 1-45);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **`Notification` 漏斗枚举、`TaskProgress` 五态、`GpuiTrayCommand`/`TrayAgent`、更新链 DTO(`UpdateAsset`/`UpdateCheckResult`/`UpdatePromptState`)、`VisualEffects`/`BlurMode` 值域、后台任务注册表(`TaskDef`/`TaskRuntime`)** 的锚点归属篇(01 篇类型锚点表已登记);他篇(02/03/04/05/06 等)只许 `use` 或经 facade 传参,禁重定义。消费的外部锚点:`OscEvent`(02)、`AgentKind`/`AiTurnOutcome`/`AttentionContext`(03)、`RuntimeTaskState`(04)、`PaneId`/`TabId`/`Workspace`(05)、`RuntimeSettings` 与设置键域(06)、凭据域边界归 10。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

五个系统面 + 一个收编面在单进程 GPUI 模块化单体中的终态,全部收敛于「单一漏斗 + 自有线程 + 注册表自愈 + best-effort」统一骨架——任何系统 API 失败一律落 debug 日志不弹错,任何跨线程通信一律消息/命令回调,不共享锁跨越阻塞调用:

| 面 | 终态 | 落位 |
| --- | --- | --- |
| 托盘 | 独立 Win32 隐藏窗口 + 自有消息泵常驻线程;attention 橙点双图标;右键 agent 直达清单 + 左键 focus_best 兜底;「退出」= 真退出通道 | `slterm_app/src/tray.rs` |
| 通知 | 通知中心单一漏斗 `Notification` 六变体;BEL / OSC 9 / OSC 133;D / hook 生命周期四源归一;失焦门控 + 闪烁恒发先行 + 双层节流 + 失败冷却;toast 点击回焦与托盘/移交同一条 FocusWindow 语义 | `slterm_app/src/notify.rs` + `slterm_app/src/platform/notifications{,/windows}.rs` |
| 任务栏进度 | `TaskProgress` 五态,ConEmu 码宽容映射(规范外码归清除);`ITaskbarList3` 每次自管 COM 初始化,失败静默;只投活跃 tab 聚焦 pane,agent 退出即清 | `slterm_app/src/taskbar.rs` |
| 自动更新 | GitHub Releases best-effort 检查 → 资产四重校验 → `.part` 流式下载(长度/MZ/SHA-256/512MiB 全过才原子替换)→ 事务目录两阶段 handoff(commit 才授权安装)→ restore ticket 恢复工作区;提示状态独立文件按版本生效 | `slterm_app/src/update_check.rs` / `update_proxy.rs` / `update_download.rs`(+`handoff/`、`handoff.ps1`) |
| 开机启动 | Startup 已知文件夹 `.lnk`(IShellLinkW 幂等重写);静默启动 = `silent_start && tray` 两条件合取(关窗即退定位下 `hide_window_on_close` 因子消去,改造节 1) | `slterm_app/src/platform/startup.rs` |
| 窗口特效 | 模糊走 GPUI `WindowBackgroundAppearance` 平台通道(Mica/MicaAlt 原生枚举 22H2 门控、Aero/Acrylic AccentPolicy 通道、互斥清理);透明度只透壳底色与终端默认背景;壁纸 = 底色之上单元格之下单纹理层,后台串行有界解码 | `slterm_app/src/gpui_shell/wallpaper.rs`(+`wallpaper/`) |
| DPI / 启动几何 | 多显示器整包交 GPUI 平台层;只保留「首窗创建前主屏 DPI 查询」一个 Win32 调用点 + 启动几何推导(基准字号量网格、主显 95% 上限、亚像素 round-trip 容忍) | `slterm_app/src/platform/startup.rs` + `slterm_app/src/gpui_shell/workspace/windowing/startup_geometry.rs` |
| 后台任务(收编) | slTerminal 旧双端调度器收编为 Rust 任务注册表 + GPUI executor:元数据静态切片单点、订阅生命周期(无订阅者不空转)、防重入闸门、按触发来源的失败策略 | `slterm_app/src/background_tasks.rs` |

所有节拍(托盘 1Hz、更新检查 12s 延迟线程、壁纸解码、后台任务 interval)一律挂既有 chrome 时钟 / GPUI background executor,零新增常驻 OS 定时器。OSC 52 剪贴板本片零涉及(归 02);`src-tauri/src/notify/`(fs watcher)原样保留归 07,与通知漏斗零耦合。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri 命令 / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。单窗口单实例直接裁定本篇多处语义(改造节 1)。
2. **通知与进度是纯消费端**:源信号全部来自 02 篇 OSC/BEL 解析族(`OscEvent::Notify`/`OscEvent::Progress` 原始 state 不上收窄、`CommandDone{exit_code}`)与 03 篇 hook 生命周期边(TurnDone/NeedsAttention 带上下文);本篇只做分类、策略与投递,不重定义源事件。「类别判定委托 profile」以 Rust 侧 source classification 重生:漏斗的 `program` 字段 = 壳维护的 `running_program` 进程名,来源合法值集单点在 03 篇 `AgentKind`;漏斗**不认事件文本里的关键字**。
3. **OSC 9;4 分层契约**(与 02 篇交界):解析层不收窄语义(原始 `state: u8` 原样上抛),消费层宽容映射——规范外码(含实测 `9;4;5;0`)一律归清除;把未知码当非法丢掉 = 进度条永远停在最后状态。
4. **投递纪律不可违反**:任务栏闪烁(FlashWindowEx,hwnd 直调不依赖窗口库)恒发先行,幂等、静默、shell 自动合并;toast 被节流只挡系统 toast 不挡闪烁。失焦门控由调用方执行(聚焦中不打扰,视觉 bell 覆盖该场景)。
5. **类型锚点纪律**:本篇唯一定义 Notification/TaskProgress/Tray 命令/更新 DTO/VisualEffects/后台任务注册表类型;跨领域类型只消费:`OscEvent`/`Grid`(02)、`AgentKind`/`AiTurnOutcome`/`AttentionContext`/`AiHookEvent`(03)、`RuntimeSnapshot`/`RuntimeTaskState`/信封(04)、`PaneId`/`TabId`/`WorkspaceTab`(05)、`RuntimeSettings`/设置键域(06)。pebrel 侧 pane id 裸 `u64` 处本篇以 05 篇 `PaneId` newtype 承接。
6. **更新链「下载器不信任 UI 数据」**:资产合同四重校验(名称/架构/版本/官方 URL 前缀)+ digest 字段优先、release body 内嵌 checksum 回退;落盘根目录 = slTerminal 应用数据目录(便携语义,归 01 篇 D2 决策),`update_state.json` 与用户设置正文解耦。
7. **Quick terminal 全砍**(已定,违反单窗口定位):其托盘/热键/设置模型不留残件,消亡清单见改造节 2。
8. **关窗即退出**:无 mux 驻留、无隐藏宿主;托盘「退出」与最后窗口关闭同一条退出链;静默启动两条件(`silent_start && tray`)缺一不可(无托盘的静默启动 = 不可达进程)。
9. **多平台整包砍**:toast 只留 WinRT 通道(tray_native/notify_rust/NSBundle 不迁);更新链只留 Windows 资产面(dmg/koly trailer/scoop marker 不迁);开机启动只留 Startup `.lnk`;`#[cfg(windows)]` 收敛于平台模块(平台分支收敛不变量 4)。
10. **GitHub Releases 坐标与发布核验归 12 篇**:本片消费 `RELEASES_API`/`RELEASES_PAGE`/下载前缀常量,值归 12 篇发布面登记;本片只管运行时更新链。`update-test-source` 本地假 release 演练通道永不回落公网。
11. **壁纸/特效不越界进终端渲染**:壁纸层在底色之上、单元格之下,文字与彩色单元背景永不透明;模糊材质切换显式互斥清理(GPUI 平台枚举 vs AccentPolicy 通道,DWM 双通道同开行为未定义,实测 backdrop 吞 Acrylic)。
12. **DPI 不 fork 窗口库**:winit mixed-DPI backport patch、`window_transition.rs` WinEvent hook、`display/animations.rs` 帧快照动画全不采纳;跨显示器行为以 GPUI 平台层实测为准。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致,改名/收敛点已标;契约数值(上限/预算/阈值)首发取 pebrel 实测值照抄,与 04 篇 D04-2 同纪律。`pub(crate)` 可见性照 pebrel 形态。

### 托盘(`slterm_app/src/tray.rs`,本篇锚点,照抄裁剪)

```rust
// 线程模型:Shell_NotifyIconW 回调必须挂窗口,不能挂壳窗口(壳不背 OS 消息泵职责),
// 故托盘自有线程 = 永不显示的 Win32 隐藏窗 + 独立 GetMessageW 泵。
// app→托盘:写 Mutex<Shared> + PostMessageW 摇醒;托盘→app:只发 GpuiTrayCommand 回调。
// 两方向不共享锁跨越阻塞调用;HWND 用 AtomicIsize 发布。

pub enum GpuiTrayCommand { Focus(Option<PaneId>), Quit }   // 旧壳 EventLoopProxy 臂砍

pub struct TrayAgent {           // 右键菜单一行;托盘与侧栏读同一 pane 事实源
    pub window: WindowHandle,    // 点击后 FocusWindow 的路由目标——单窗化后塌缩为唯一窗,eg 改造节 1
    pub pane: PaneId,            // 05 篇 newtype 承接 pebrel 裸 u64
    pub label: String,           // 「claude · nebula」= 程序名 + cwd 尾段
    pub needs_attention: bool,   // CLI 停在半路等用户(03 篇 attention 边)
}

#[cfg(windows)]
mod win {
    const WM_APP_TRAY: u32 = WM_APP + 1;     // Shell_NotifyIconW 回调消息
    const WM_APP_REFRESH: u32 = WM_APP + 2;  // 快照更新
    const WM_APP_SET: u32 = WM_APP + 3;      // 托盘开/关
    const WM_APP_SHUTDOWN: u32 = WM_APP + 4; // 收尾:删图标 + 退泵
    const WM_APP_ICON: u32 = WM_APP + 5;     // 图标重建（单图标态：仅启动初始化一次）
    const MENU_AGENT_BASE: usize = 1000;     // MENU_AGENT_BASE + i = 聚焦第 i 个 agent
    const MENU_SHOW: usize = 1;
    const MENU_QUIT: usize = 2;

    static HWND: AtomicIsize;
    static STATE: Mutex<Shared>;             // agents 快照 + enabled;内容相等 update 直接返回(1Hz 天然去抖)

    pub fn init_gpui(on_command: impl Fn(GpuiTrayCommand) + Send + Sync + 'static);
    pub fn set_enabled(enabled: bool);       // 关图标留线程(NIM_ADD 失败留 added=false 下次重试)
    pub fn update(agents: Vec<TrayAgent>);   // 写 STATE + post(WM_APP_REFRESH)
    pub fn shutdown();                       // post(WM_APP_SHUTDOWN):不删图标要等用户悬停才消失
    fn tray_thread(...);                     // 建隐藏窗 + 消息泵
    unsafe extern "system" fn tray_wnd_proc(hwnd, msg, wparam, lparam) -> LRESULT;
    fn build_menu(agents: &[TrayAgent], include_quit: bool) -> HMENU;  // 构建与模态展示分离:
    fn show_menu(...);                       // 回归测试不开线程验真实 HMENU;菜单期间用快照,id 映射回打开那一刻的清单
    fn focus_best();                         // 左键兜底:先等输入的 → 任意 agent → 最后裸窗
    fn send_focus(agent: Option<TrayAgent>); // 只发 GpuiTrayCommand 回调
    fn build_icons(...) / draw_attention_dot(...) / create_icon(...);
    // 预乘 BGRA HICON 像素管线:DWM 按 premultiplied 合成,直 alpha 深色任务栏泛白边;
    // 橙点选警示橙(托盘图标不知道主题 accent);负高自顶向下 DIB。
}
```

### 通知漏斗(`slterm_app/src/notify.rs`,本篇锚点,照抄裁剪)

```rust
// 「新来源 = 新变体、新输出 = deliver 里新一行」的加法扩展合同。
pub enum Notification {
    Bell { program: Option<String> },                              // BEL:AI CLI 回合完成主信号
    CommandDone { duration: Duration, program: Option<String> },   // OSC 133;C/D 跟踪命令完成
    CommandFailed { duration: Duration, program: Option<String>, exit_code: i32 }, // 非零退出(含短失败命令)
    Text { body: String, program: Option<String> },                // OSC 9 自由文本
    AiTurn { program: String, message: Option<String>, attention: bool },          // 03 篇 TurnDone/NeedsAttention
    AiTurnIssue { program: String, message: Option<String>, outcome: AiTurnOutcome }, // 03 篇,无确认成功回答
}

impl Notification {
    pub(crate) fn command_finished(program: Option<String>, duration: Duration,
        exit_code: Option<i32>, hook_seen: bool) -> Option<Self>;
    // exit_code 非零 → CommandFailed;hook_seen 确认过的短成功命令不再通知(hook 覆盖是权威)。
    fn is_attention(&self) -> bool;   // AiTurn{attention:true}:权限确认/提问,强于「回合完成」
    fn is_failure(&self) -> bool;     // AiTurnIssue / CommandFailed
    fn is_ai(&self) -> bool;
    fn toast_text(&self) -> (String, String);        // 经 clamp_toast_body 的单行预览
    fn raw_toast_text(&self) -> (String, String);    // 原文,供壳内横幅/日志
}

pub const COMMAND_NOTIFY_MIN: Duration = Duration::from_secs(10); // hook 未确认的命令最短通知时长
pub(crate) const TOAST_BODY_MAX_CHARS: usize = 160;  // 瞥读层上限:超长卡片遮关闭钮,空白折叠,幂等截断
pub(crate) fn clamp_toast_body(body: &str) -> String;

// 双层节流:全局 toast 间隔 + pane 级失败冷却(恢复事件结束旧 episode、同错冷却、
// 新错独立、跨 pane 独立)。
const TOAST_THROTTLE: Duration = Duration::from_secs(3);
const FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
pub(crate) struct PaneFailureThrottle { recent: Vec<RecentFailure> }
impl PaneFailureThrottle {
    pub(crate) const fn new() -> Self;
    pub(crate) fn accepts(&mut self, pane: PaneId, notification: &Notification,
        delivering: bool, now: Instant) -> bool;
}
pub(crate) struct PaneNotificationThrottle { recent: HashMap<(PaneId, bool, Option<u64>), Instant> }
impl PaneNotificationThrottle {
    fn accepts_request(&mut self, pane_id: PaneId, attention: bool, request: Option<u64>, now: Instant) -> bool;
}

pub(crate) type ToastActivation = Arc<dyn Fn() + Send + Sync>;
pub(crate) fn init_gpui_activation(sender: mpsc::Sender<ShellEvent>);   // 一次,壳启动早期
pub(crate) fn deliver_gpui(notification: &Notification, pane_id: PaneId);
pub(crate) fn deliver_gpui_with_choices(notification: &Notification, pane_id: PaneId,
    confirmation: Option<(u64, Vec<String>)>);  // 权限确认动作按钮直达选择(归 03 confirmation 链)
// deliver_gpui 内:PaneNotificationThrottle 判定 → toast_text → spawn_actionable_toast。
// 任务栏闪烁在壳调用点先于 deliver 恒发(eg 数据流节);legacy PROXY/deliver(window,..) 整臂砍。
```

### AUMID 注册 + WinRT toast 投递(`slterm_app/src/platform/notifications{,/windows}.rs`,照抄裁剪)

```rust
// win::AUMID = crate::brand::WINDOWS_APP_ID(01 篇锚定 com.slterminal.terminal,归 01 P-3)
pub fn prepare();                       // 幂等重写即自愈;best-effort,cached per process
pub fn show(title: &str, body: &str);   // 诊断/更新提示直发通道
pub fn notify_test() -> i32;            // 同步诊断:注册 + 发 toast 全链路逐段打印
mod win {
    pub const AUMID: &str = crate::brand::WINDOWS_APP_ID;
    pub fn ensure_aumid();              // HKCU\Software\Classes\AppUserModelId\<AUMID>
    fn set_reg_sz(subkey, name, value); // RegSetKeyValueW;免 COM/免快捷方式/免安装器/免管理员
    fn ensure_icon_file();              // IconUri 物化(appLogoOverride)
}
// windows.rs:toast 常驻 MTA worker + 有界队列(32)+ 通知对象保留 64 防 Activated premature 释放。
// Show RPC 是跨进程调用(几十 ms),永不在事件循环上跑。
struct Request { title: String, body: String, open: Option<ToastActivation>, actions: Vec<ToastAction> }
pub(super) fn enqueue(title, body, open, actions);   // try_send 满即丢(落 warn)
fn run(receiver: mpsc::Receiver<Request>);           // CoInitializeEx(MTA);live 保留 64,溢出 pop_front
fn show(request: Request) -> Result<ToastNotification>;
fn xml(request: &Request) -> String;                 // ToastGeneric + escape(控制字符过滤);eg 改造节 6 duration 槽
pub(crate) struct ToastAction { pub label: String, pub activate: ToastActivation }
```

### 任务栏进度(`slterm_app/src/taskbar.rs`,本篇锚点,照抄)

```rust
pub enum TaskProgress {     // ConEmu 9;4 状态码的五态模型
    #[default] None,         // 清除,任务栏恢复常态
    Indeterminate,           // 来回扫的条
    Value(u8),               // 正常 0..=100
    Error(Option<u8>),       // 红条,可带值也可只染色
    Paused(Option<u8>),      // 黄条
}
impl TaskProgress {
    pub fn from_osc(state: u8, value: Option<u8>) -> Self;
    // 0 清除 / 1 正常 / 2 错误 / 3 不确定 / 4 暂停；规范外码（含 5）一律 None（清除）；percent clamp 0..=100
    pub fn is_active(self) -> bool;
    #[cfg(windows)] fn taskbar_flag(self) -> i32;  // TBPFLAG
    #[cfg(windows)] fn percent(self) -> Option<u8>;
}
#[cfg(windows)]
pub fn apply(hwnd: isize, progress: TaskProgress);
// 每次调用自管 COM 生命周期:CoInitializeEx(COINIT_APARTMENTTHREADED|DISABLE_OLE1DDE),
// RPC_E_CHANGED_MODE 容忍,CoCreateInstance(CLSID_TaskbarList,IID_ITaskbarList3),
// HrInit 先调(不调部分系统后续调用全败),SetProgressState/SetProgressValue,Release + 配对 Uninitialize。
// vtable 手写最小布局;失败一律静默——任务栏是纯装饰。
```

### 自动更新检查(`slterm_app/src/update_check.rs` + `update_check/assets.rs` + `update_check/test_source.rs`,照抄裁剪)

```rust
// 启动检查全程 best-effort:断网/坏 JSON/限流全落 debug 不出横幅。
// spawn_gpui_once 序:hydrate 双证据恢复 → 缓存资产可装/失败未见过 → 提示 →
// auto_check_updates 设置闸门 → 线程延迟 12s(等首窗与首个会话安顿)再联网。
pub struct UpdateAsset {       // 资产四重校验后的可信形态(下载器不信任 UI 数据)
    pub name: String, pub version: String, pub url: String,
    pub size: Option<u64>, pub digest: Option<String>,   // SHA-256,digest 字段优先
}
pub struct UpdateCheckResult { pub current: String, pub latest: String,
    pub update_available: bool, pub asset: Option<UpdateAsset> }

struct UpdatePromptState {        // update_state.json;按版本生效互不串味
    prompted: HashSet<String>, skipped: HashSet<String>, remind_after: Option<u64>,
}
static UPDATE_STATE_LOCK: Mutex<()>;                 // + 文件锁双闸
fn update_state_path() -> PathBuf;                   // 数据根目录归 01 篇 D2
pub fn should_prompt(version: &str) -> bool;
pub fn mark_prompted(version: &str) -> Result<(), String>;
pub fn remind_later(version: &str) -> Result<(), String>;   // now + REMIND_LATER_SECS(3 天)
pub fn skip_version(version: &str) -> Result<(), String>;
pub(crate) fn is_newer(latest: &str, current: &str) -> bool;   // 手写数字版本比较:点分段数值
pub(crate) fn can_install_version(latest: &str) -> Result<bool, String>;  // 段内后缀忽略、拒绝降级
fn version_is_installable(latest: &str, current: &str, rehearsal: bool) -> bool;
fn parse_latest_release(bytes: &[u8]) -> Result<LatestRelease, String>;
fn checksum_from_release_body(body: &str, asset_name: &str) -> Option<String>;  // body 内嵌 checksum 回退
fn normalize_sha256(value: &str) -> Option<String>;
```

> assets.rs 的 release 资产精确匹配合同(名称/架构/版本/官方 URL 前缀四重校验)照抄;`windows_x64_installer_names` 由 pebrel 的三候选形态(含 legacy 品牌回退名)改造为 slTerminal 单一合同(归改造节 7)。`fetch_release_with_fallback` 的因果链见 pebrel `architecture/notes/nebula_app/update_check/2026-09-23-public-release-fallback.md`:仅 REST 403/429 时二次有界请求官方 latest release 页面(≤5 跳、终 URL 必须本仓 https + 精确数字 tag),再取 `SHA256SUMS`(10s/64KiB 预算)——嵌 token / 猜包 URL / 把 API 失败当最新 三件均被该 note 明确拒绝,口径随迁。

### 代理解析(`slterm_app/src/update_proxy.rs`,照抄)

```rust
// 自解析代理解:Win 注册表 ProxyServer 按协议取值、socks 正确归 socks5、env 回退、
// NO_PROXY、https_only、双超时——不交给 ureq 的 win-system-proxy(它把 socks 拼成 http)。
pub(crate) fn resolve(target_url: &str) -> Option<Proxy>;
pub(crate) fn agent(target_url: &str, timeout: Duration) -> ureq::Agent;
//   .timeout_global(Some(budget)) + .timeout_connect(30s) + .https_only(true)
fn raw_system_proxy(target_url: &str) -> Option<(String, Vec<String>)>;  // 注册表 ProxyServer + 例外表
fn parse_windows_proxy_server(server: &str, target_scheme: &str) -> Option<String>;
fn proxy_from_url(url: &str, no_proxy: &[String]) -> Result<Proxy, ureq::Error>;
```

### 下载与校验(`slterm_app/src/update_download.rs` + `update_download/cache.rs`,照抄裁剪)

```rust
// .part 流式 + 四重通过才原子替换:长度 / MZ 头 / SHA-256 / 512MiB 上限。
// 中断下载或错误响应不能变成可执行文件。
const RELEASE_DOWNLOAD_PREFIX: &str;           // 官方 URL 前缀归 12 篇登记
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;
static NEXT_GENERATION: AtomicU64;             // 下载会话 generation 作废链
static DOWNLOAD_SESSION: Mutex<Option<DownloadSession>>;

pub(crate) enum DownloadStatus { Absent, Downloading, Ready { .. }, InstallFailed(String), .. }
pub(crate) struct DownloadJob { asset, generation }  // begin 领取,贯穿 run/set_progress
pub(crate) fn begin(asset: &UpdateAsset) -> Result<Option<DownloadJob>, String>;
pub(crate) fn cancel(asset: &UpdateAsset);
pub(crate) fn run(job: DownloadJob, language: UiLanguage);  // 后台线程全程
pub(crate) fn hydrate();                       // 缓存与会话双证据:无网也恢复失败态/就绪态
pub(crate) fn cached_asset() -> Option<UpdateAsset>;
pub(crate) fn installation_failure_unseen(prompt_state: &Path) -> bool;
pub(crate) fn ready_path(asset: &UpdateAsset) -> Result<PathBuf, String>;
fn download_and_verify(..) / fn download_with_job(..);      // 流式写 .part,超长即中止
fn verify_download(..) / fn verify_file(path, asset) -> Result<u64, String>; // MZ+长度+SHA-256
fn set_progress(job: Option<&DownloadJob>, downloaded: u64, total: Option<u64>);
fn download_paths(asset: &UpdateAsset) -> Result<(PathBuf, PathBuf), String>;  // (最终, .part)
fn validate_asset_contract(asset: &UpdateAsset, names: &[String]) -> Result<(), String>;
```

### 两阶段 handoff(`slterm_app/src/update_download/handoff.rs` + `handoff.ps1` + `platform/update_installation.rs`,照抄裁剪)

```rust
// 事务目录文件态机:文件是持久证据,崩溃后状态可重建。
//   下载完成 → plan.json + ready.json(预备)→ 用户授权 → commit.json(安装权威);
//   否则 PreparedUpdate::Drop → cancel.json + 杀 helper;helper 无 commit.json 无安装权威。
struct Plan { asset, transaction, config_directory, guard_path, workspace snapshot.. }
pub(crate) struct PreparedUpdate { directory, plan, .. }
impl PreparedUpdate {
    pub(crate) fn commit(&self);   // 原子写 commit.json —— 唯一授权动作
}
impl Drop for PreparedUpdate {     // 未 commit 即 Drop:写 cancel.json + terminate helper
    fn drop(&mut self);
}
fn guard_base(executable: &Path) -> PathBuf;   // 旁置守卫锁:同二进制全配置共享
pub(crate) fn installation_in_progress() -> io::Result<bool>;  // 安装器不动守卫锁
pub(crate) fn prepare(asset: &UpdateAsset) -> Result<PreparedUpdate, String>;  // plan+ready
pub(crate) fn restore_ticket() -> Option<Vec<Session>>;   // 按工作区快照恢复,最多 3 次尝试
pub(crate) fn acknowledge_restore(restored_windows: usize);
pub(super) fn failed_update() -> Option<(UpdateAsset, String)>;
pub(super) fn failure_unseen(prompt_state: &Path) -> bool;
pub(crate) fn schedule(asset: &UpdateAsset) -> Result<(), String>;   // 下次启动授权安装
pub(crate) fn apply_scheduled() -> bool;   // 启动早期执行,成功后真退出 eg 改造节 1
```

> `handoff.ps1` 独立安装 helper(include_bytes! 物化,子进程跑版本验证 + 工作区快照重启 + env 恢复变量):`$env:PEBREL_UPDATE_RESTORE`/`$env:PEBREL_CONFIG_DIR`/`$env:NEBULA_CONFIG_DIR` 落地改 `SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR` 单名(01 篇 C 节);轮询 commit.json 且校验 transaction 一致,守卫锁全程旁置。`platform/update_installation.rs` 的 `canonical`/`current_process_created`(native creation identity,防回收 PID 冒名授权)/`spawn_helper`/`installation_directory`/`spawn_prepared_helper` 照抄;macOS 臂砍。

### 分发闸门(`slterm_app/src/platform/distribution.rs`,照抄裁剪)

```rust
pub(crate) enum Distribution { Direct, Scoop, Unrecognized }   // macOS bundle 臂砍
impl Distribution { pub(crate) fn externally_managed(self) -> bool }  // Scoop/Unrecognized → true
pub(crate) fn current() -> Distribution;
pub(crate) fn require_direct_update() -> Result<(), String>;
```

> `current()` 的 marker 文件探测照抄 pebrel `from_marker`/`parse_marker` 形态;slTerminal 便携 zip 形态天然 `Direct`(闸门放行),marker 文件名归 01 篇 D 节命名裁定(eg 改造节 7)。

### 开机启动与首窗 DPI(`slterm_app/src/platform/startup.rs`,照抄裁剪)

```rust
pub fn set_launch_at_login(enabled: bool) -> io::Result<()>;
fn startup_shortcut() -> io::Result<PathBuf>;
pub(crate) fn launch_at_login() -> bool;
pub(crate) fn start_hidden(settings: &RuntimeSettings) -> bool;
//   = settings.silent_start && settings.tray
//   两条件合取(hide_window_on_close 键不存在——关窗即退定位,因子消去,改造节 1);
//   无托盘的静默启动 = 不可达进程。
pub(crate) fn primary_display_scale() -> Option<f32>;
//   首窗创建前 MonitorFromPoint + GetDpiForMonitor,供初始网格尺寸一步到位。
```

### 窗口特效与壁纸(`slterm_app/src/gpui_shell/wallpaper.rs` + `wallpaper/image_loader.rs`,本篇锚点,照抄裁剪)

```rust
pub struct VisualEffects {       // gpui::Global;Drop 时 generation+1 使在途加载失效
    pub opacity: f32,
    pub blur: BlurMode,
    wallpaper: Option<Wallpaper>,
    generation: Arc<AtomicU64>,
    loading: bool,
}

pub fn refresh(cx: &mut App);            // 设置变更总入口:UI 线程零文件 I/O
pub fn window_opacity(cx: &App) -> f32;  // 终端默认背景只透这一层
pub fn chrome_surface_opacity(cx: &App) -> f32;
pub fn set_opacity_live(opacity: f32, cx: &mut App);   // live 调透明度即时预览
pub fn initial_background_appearance() -> WindowBackgroundAppearance;  // 首帧即平台 backdrop
fn effective_material(runtime: &RuntimeSettings) -> (f32, BlurMode);
// Mica/MicaAlt → GPUI 原生枚举(22H2=build 22621 门控;17763+ 回退 AccentPolicy;
// 更老 → Transparent 纯色+透明度);Aero/Acrylic → Blurred AccentPolicy 通道;
// 切换档显式清理另一通道(双通道同开 DWM 行为未定义,实测 backdrop 吞 Acrylic)。
fn background_appearance(blur: BlurMode) -> WindowBackgroundAppearance;
fn apply_window_effects(cx: &mut App);   // AppliedBlur Global 记账,先清旧层再叠新层
fn apply_windows_accent_policy(...);      // WCA 三件套:AccentPolicy/渐变color/状态位
```

壁纸渲染(`gpui_shell/wallpaper.rs` 私有 + `renderer/image_layout.rs` 公开形):

```rust
// 壁纸 = 底色之上、单元格之下的一层图;fit/alignment/cover_chrome/图自身独立透明度。
pub enum BackgroundImageFit { None, Uniform, UniformToFill, Fill }
pub enum BackgroundImageAlignment { TopLeft, Center, BottomRight, .. }   // 九宫格
fn wallpaper_rect(target_w, target_h, image_w, image_h, fit, alignment) -> Rect;
// 绘制只复用一张纹理(retire_image 旧纹理即弃);paint_wallpaper 按 cover_chrome
// 决定垫在 chrome 下还是单元格下;文字与彩色单元背景永不透明 eg 边界 11。
```

壁纸后台解码(`wallpaper/image_loader.rs`,照抄):

```rust
// 单 job 串行 + generation 过期即弃 + 按 (路径, 文件戳) 缓存;UI 线程零文件 I/O。
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;    // 文件上限
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024; // 解码预算
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;    // 绘制纹理上限
const MAX_IMAGE_EDGE: u32 = 2048;
const MAX_PREVIEW_EDGE: u32 = 512;               // 设置页预览小尺寸分支
pub(super) struct FileStamp { modified: Option<SystemTime>, bytes: u64 }
pub(super) struct Request { path, cached: Option<FileStamp>,
    generation: Arc<AtomicU64>, version: u64 }
impl Request { fn cancelled(&self) -> bool }     // generation != version
pub(super) struct LoadedImage { pixels, layout_width, layout_height, stamp }
pub(super) enum LoadError { NotFound, TooLarge, Decode(String), Cancelled, .. }
pub(super) fn load(request: Request) -> Result<Option<LoadedImage>, LoadError>;
pub(super) fn load_preview(request: Request) -> Result<Option<LoadedImage>, LoadError>;
fn bounded_dimensions(width, height) -> (u32, u32);  // 超边长/超纹理预算按比例缩
```

### 启动几何(`slterm_app/src/gpui_shell/workspace/windowing/startup_geometry.rs`,参考)

```rust
pub(super) fn preferred_size(cx: &App, sidebar_width: f32) -> Option<Size<Pixels>>;
pub(in crate::gpui_shell::workspace) fn prepare_initial_grid(...);
fn default_size(...) / fn fit_preflight_size(preferred, cx) -> Size<Pixels>;
fn same_device_size(actual, requested, scale) -> bool;
// 按基准字号量网格而非持久化缩放;主显 95% 上限;最低尺寸地板;
// 同设备像素 round-trip 亚像素容忍,容内不二次 SetWindowPos。
```

### 后台任务注册表收编(`slterm_app/src/background_tasks.rs`,本篇锚点,新建)

> slTerminal 旧双端调度器(`src/features/backgroundTasks/` 前端半 + `src-tauri/src/background_tasks/` 后端半)随 WebView 全灭,收编为单进程形态:元数据注册表契约保留(模块级单例 + 静态切片 + 注册触发),执行载体从「tauri poller + 前端 interval」改为 GPUI background executor。零新增常驻 OS 定时器(eg 目标形态)。

```rust
pub struct TaskDef {                 // 任务元数据(静态切片条目)
    pub task_id: &'static str,
    pub title: &'static str,
    pub interval_min: u64,
    pub interval_max: u64,
    pub interval_default: u64,
    pub enabled_default: bool,
    pub executor: Option<TaskExecutor>,
}

// executor 为 None 的任务由订阅方自驱（无订阅者即不轮询）。
pub type TaskExecutor = fn();  // 在 GPUI background executor 上 spawn

pub struct TaskRuntime {        // 与 TASKS 静态切片同序对齐
    pub enabled: AtomicBool,
    pub interval_sec: AtomicU64,
    pub running: AtomicBool,    // poller 循环开关
    pub subscribers: AtomicUsize,
}

pub static TASKS: &[TaskDef] = &[/* planBalance → 10 篇；sessionRefresh → 03 ai_sessions */];
pub static RUNTIMES: &[TaskRuntime] = &[/* 与 TASKS 同序 */];

pub fn find(task_id: &str) -> Option<&'static TaskDef>;
```

订阅生命周期与失败策略（契约从旧双端形态原样保留，载体替换）：

- 订阅/退订增减 `subscribers`：归零即停轮询、非零即按 `interval_sec` 续跑；`enabled` 与 `interval_sec` 写入即时生效（applyConfig 直写原子量，不重建循环）。
- 重入闸门：单次触发未跑完时到点跳过，不并发叠加。
- 逐触发失败策略：单轮 `executor` panic/Err 记日志后照常排下一轮，持久 SETTINGS_KEY 不落失败态，避免一次网络抖动把任务永久打瘫。

## 数据流与状态机

### 通知漏斗（单线程顺序管线）

```
事件源(TurnDone/NeedsAttention/命令收尾/任务栏 OSC)
   │ ① 调用点先做焦点闸门（窗口已聚焦 → 全弃）
   ▼
② FlashWindowEx 无条件先闪（节流之前就闪，pebrel causal 注释：闪烁是「恢复中」信号）
   ▼
③ 全局 TOAST_THROTTLE 3s 去抖 → ④ PaneNotificationThrottle pane 级窗口
   ▼
⑤ PaneFailureThrottle：失败归属同一 pane+程序+结局的 episode，
   episode 内弹过 1 条则 30s FAILURE_COOLDOWN 静默，恢复即结 episode
   ▼
⑥ clamp_toast_body(160) → ⑦ AUMID 注册 + MTA toast 队列（sync_channel 32，保留 64 条活通知）
```

要点：闪灼先行于一切节流；episode 语义以「进程恢复」为终点（`is_attention==false` 同 pane 到达即结），不是固定窗口。

### UpdatePromptState（按版本号的 per-version 状态机）

```
            ┌──────────────┐
            │  无该版本记录  │
            └──────┬───────┘
   检查发现新版本且  │ should_prompt（无记录 或 已到 remind_after）
   is_newer 通过    ▼
            ┌──────────────┐   用户选「稍后提醒」   ┌──────────────┐
     ┌─────▶│   prompted   │────────────────────▶│ remind_later │
     │      └──┬───┬───┬───┘  now+3d 写回         └──────┬───────┘
     │         │   │   │                                  │ remind_after 到期
     │      安装 │ 跳过 │ 启动时用户无响应（不自动写）        ▼
     │         │   │   │                        should_prompt 再真 → 回 prompted
     ▼         ▼   ▼
  进入 handoff  skip_version：本次进程生命周期内不再提示该版本
  两阶段流程    （记录落盘，下次启动 is_newer 仍会再判，pebrel 行为保留）
```

### handoff 文件机（两阶段事务）

```
下载侧(本进程)                helper 侧(handoff.ps1,提权进程)
──────────────────────────────────────────────────────────────
plan.json        ──写──▶  发现 plan.json
                          校验 + 杀本进程(等待退出) + 覆盖安装
ready.json(就绪)  ◀─写──   安装完成、待用户确认
                          启动新本进程
commit.json      ──写──▶  确认：删 plan/ready、写 commit、清理旧版本
（用户选「立即重启」）
或
cancel.json      ──写──▶  回滚：用 .old 还原、删 plan/ready、写 cancel
（用户选「稍后」或 helper 失败）
restore ticket   ◀─写──   三次尝试后仍失败 → 写 restore ticket
（≤3 次尝试后由用户确认还原路径）
```

任一阶段本进程缺席/崩溃：helper 见到孤儿 plan.json 但无 ready/commit/cancel 配对，按 cancel 路径处理（安全默认回滚）。

### DownloadJob 代际失效

```
begin(url,generation) → 登记 DOWNLOAD_SESSION
run() 循环：每 chunk 前查 generation != DOWNLOAD_SESSION → 自废退出；
            每 chunk 落 .part；累计超 512MiB → 废；
            长度不符 → 废；MZ 头不符 → 废
verify：SHA-256 全量过 → rename 去 .part，hydrate 双证据落盘
cancel()：代际 +1 + 删 .part；下次 begin 以新代际起
```

代际单调递增跨任务共享：取消旧任务不必知道其句柄，代际失配即天然失活。

### 托盘 1Hz 快照（chrome-clock 借拍，零新增定时器）

```
SltermTick(既有 1Hz 全局拍,原 NebulaTick) ──每拍──▶ tray::update(Shared 快照入 Mutex)
                                        │ PostMessageW(WM_APP_TRAY_UPDATE)
                                        ▼
                              托盘线程消息泵 GetMessage
                                        │ 取 Mutex 快照：enabled/attention/menu 态
                                        ▼
                              Shell_NotifyIconW(NIM_MODIFY 图标/提示)
                                        │
GPUI → 托盘：GpuiTrayCommand 回调枚举入队（Focus/Show/Quit/…）
托盘 → GPUI：菜单项经 MENU_AGENT_BASE+agent_idx 回馈，tray 线程只 PostMessage
```

单窗语义下所有 `Focus` 命令坍缩为「聚焦那唯一的主窗」；`MENU_SHOW` 等价聚焦，`MENU_QUIT` 走 04 的退出链（见改造节 1）。

### 壁纸代际失效（render 与 decode 同闸门）

```
apply_window_effects 改壁纸路径/适应度/对齐 → VisualEffects.generation +1
   │
   ├─→ decode 侧：image_loader Request{generation} 落后台线程
   │      decode 完成回主线程前先比对 generation；失配 → 整帧丢弃
   ├─→ render 侧：gpu 纹理绑定前比对 generation；失配 → 跳过壁纸层
   └─→ 新 decode 未回前保留旧纹理呈现（不闪空）
```

`Request::cancelled` 仅判代际失配，不做协作式中断——decode 跑完即弃，代码简单于半途取消。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

45 个采纳点逐条照抄清单。「缝合点」= 落进 slTerminal 哪个既有锚；「因果链」一句讲清为什么要它。源路径一律相对 `pebrel/` 仓根（baseline `e537d528`），目标路径相对 `slterm_app/`（本篇新建面，细节见改造节）。改名映射的唯一权威是 01 篇总表，本篇只引用不重复登记。

### 托盘（点 1–8）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 1 | `nebula_app/src/tray.rs` · `win::tray_thread`/`tray_wnd_proc`/`WM_APP_*` | 无（OS 面） | `Shell_NotifyIconW` 回调必须有宿主窗口，且不能寄生壳窗（壳不背消息泵），故独立隐藏窗 + 自有泵。 |
| 2 | `tray.rs` · `win::post`/`STATE`/`HWND`/`update` | `SltermTick` 1Hz | 单向 `PostMessageW` 摇醒 + 快照锁互不跨阻塞调用；内容相等直接返回让 1Hz 天然去抖。 |
| 3 | `tray.rs` · `win::build_icons`/`draw_attention_dot`/`create_icon` | brand 单图标 | DWM 按预乘 BGRA 合成托盘图标，直 alpha 深色任务栏泛白边；负高 DIB 自顶向下。 |
| 4 | `tray.rs` · `build_menu`/`show_menu`/`focus_best`/`send_focus`/`MENU_*` | 04 `RuntimeTaskState` 投影 | 右键列 agent 直达 + 左键 focus_best 三级兜底；菜单构建/展示分离使 HMENU 可不开线程回归测。 |
| 5 | `nebula_app/src/event.rs` 1Hz 调用点 + `window_context/agents.rs` `tray_agents` | `SltermTick` | 托盘是环境信息面，秒级延迟无感，挂既有 chrome 拍换零新增定时器。 |
| 6 | `tray.rs` · `GpuiTrayCommand`/`init_gpui`/`GPUI_COMMAND` | GPUI 主窗 | GPUI 主窗无 winit `EventLoopProxy`，托盘反向只能发命令回调枚举。 |
| 7 | `tray.rs` · `set_enabled`/`apply_enabled` | 06 `tray` 设置键 | 开/关高频往复，留线程只增删图标；`NIM_ADD` 失败留 `added=false` 下次重试自癒。 |
| 8 | `tray.rs` · `shutdown`/`remove_icon` + `gpui_shell/mod.rs` 收尾点 | 04 退出链 | 不主动删图标要等用户悬停才消失，故收尾 `WM_APP_SHUTDOWN` 必发。 |

### 通知（点 9–18）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 9 | `nebula_app/src/notify.rs` · `Notification`/`command_finished`/`COMMAND_NOTIFY_MIN` | 02 `OscEvent` 族 + 03 hook 边 | 单一漏斗枚举收全部信号源，「新来源=新变体」加法合同让 AI-CLI 钩子落地不重接骨架。 |
| 10 | `nebula_app/src/platform/notifications.rs` · `win::register_aumid`/`ensure_aumid`/`set_reg_sz`/`ensure_icon_file` | 01 P-3 AUMID `com.slterminal.terminal` | `HKCU` 写注册免 COM/快捷方式/安装器/管理员，幂等重写即自癒；身份换 slTerminal 自有 AUMID。 |
| 11 | `platform/notifications/windows.rs` · `enqueue`/`run`/`show`/`xml` | 本篇 MTA worker | toast Show 是跨进程 RPC（几十 ms）永不上事件循环；保留 64 条防 Activated 回调 premature 释放。 |
| 12 | `notify.rs` · `deliver`/`deliver_gpui` | 本篇窗口句柄 | 闪烁幂等静默、shell 自动合并，故恒发先行；toast 被节流只挡系统 toast 不挡闪烁。 |
| 13 | `notify.rs` · `deliver` 调用约定 | GPUI 焦点态 | 聚焦中不打扰，视觉 bell 已覆盖该场景，故失焦门控在调用点。 |
| 14 | `notify.rs` · `TOAST_THROTTLE`/`FAILURE_COOLDOWN`/`PaneFailureThrottle`/`PaneNotificationThrottle` | 05 `PaneId` | 全局 3s 去抖 + pane 级 30s 失败冷却；恢复结 episode，同错冷却新错独立。 |
| 15 | `notify.rs` · `TOAST_BODY_MAX_CHARS`/`clamp_toast_body`/`toast_text` | 无 | 通知是瞥读层，长回答撑破卡片会遮关闭钮，故 160 字符幂等截断。 |
| 16 | `notify.rs` · `ToastActivation`/`init_gpui_activation`/`deliver_gpui_with_choices` | 04 FocusWindow 语义 | toast 点击/托盘点击/动作按钮三条回窗路径合并同一条：还原最小化+选中 tab+聚焦 pane。 |
| 17 | `notify.rs` 模块文档 + `Notification::Bell` | 02 BEL/OSC 9/133;D | 零用户配置靠 CLI 自带信号，不向用户索要 hook 脚本编辑。 |
| 18 | `platform/notifications.rs` · `notify_test` | 本篇诊断命令 | 注册+发 toast 全链路逐段打印，是排障唯一同步诊断通道（其余 CLI 诊断面砍掉）。 |

### 任务栏进度（点 19–22）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 19 | `nebula_app/src/taskbar.rs` · `TaskProgress`/`from_osc`/`is_active`/`taskbar_flag`/`percent` | 02 `OscEvent::Progress` | 未知码（含实测 `9;4;5;0`）当非法丢弃会让进度条永停末态，故规范外码一律归清除。 |
| 20 | `taskbar.rs` · `apply` | 本篇 HWND | `ITaskbarList3` 自管 COM（容忍 `RPC_E_CHANGED_MODE`、`HrInit` 先调），任务栏纯装饰失败静默。 |
| 21 | `nebula_terminal/src/osc_cwd.rs` · `OscEvent::Progress` 注释契约 | 02 解析层 | 解析层不收窄语义、消费层宽容映射，原始 state 原样上抛。 |
| 22 | `gpui_shell/workspace/terminal_activity.rs` 调用点 + `window_context/agent_activity.rs` 清除点 | 03 agent 生命周期 | 进度事件→窗口 HWND 路由只认活动 tab 聚焦 pane，agent 退出即清不留残条。 |

### 自动更新（点 23–34）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 23 | `nebula_app/src/update_check.rs` · `spawn_gpui_once` | GPUI executor | 启动检查 best-effort，断网/坏 JSON/限流全落 debug；线程延迟 12s 等首窗与首个会话安顿。 |
| 24 | `update_check.rs` · `UpdatePromptState`/`should_prompt`/`mark_prompted`/`remind_later`/`skip_version`/`update_state_path`/`UPDATE_STATE_LOCK` | slTerm 数据目录 | 提示状态独立按版本生效，后台检查不得改写用户设置正文，文件锁双闸防并发。 |
| 25 | `update_check.rs` · `is_newer`/`can_install_version`/`version_is_installable` | 本篇 | 手写数字版本比较，点分段数值、段内后缀忽略，拒绝降级。 |
| 26 | `update_check.rs` · `parse_latest_release`/`windows_x64_installer_names`/`select_windows_x64_installer`/`checksum_from_release_body`/`normalize_sha256` + `update_check/assets.rs` `select`/`native_names` | 12 发布坐标 | 资产名/架构/版本/官方 URL 前缀四重校验，digest 字段优先、release body checksum 回退，下载器不信任 UI 数据二次收紧。 |
| 27 | `nebula_app/src/update_proxy.rs` · `resolve`/`agent`/`raw_system_proxy`/`parse_windows_proxy_server` | 本篇 ureq 客户端 | 自解析注册表 `ProxyServer` 按协议取值、socks 归 socks5；`win-system-proxy` 会把 socks 拼成 http 故不托付。 |
| 28 | `nebula_app/src/update_download.rs` · `download_and_verify`/`download_with_job`/`verify_download`/`verify_file`/`validate_asset_contract`/`download_paths` | slTerm 数据目录 | `.part` 流式 + 长度/MZ/SHA-256/512MiB 全过才原子替换，中断或错响应不能变可执行文件。 |
| 29 | `update_download.rs` · `DownloadJob`/`begin`/`cancel`/`run`/`set_progress` | 本篇 | generation 作废链让取消/换版本后旧任务进度与完成写入一律无效。 |
| 30 | `update_download.rs` · `hydrate`/`cached_asset`/`installation_failure_unseen` + `cache.rs` | slTerm 数据目录 | 缓存+会话双证据 hydrate 使启动无网也能恢复失败/就绪态；result.json mtime 晚于提示态判定「未见过」重提示。 |
| 31 | `update_download/handoff.rs` · `Plan`/`prepare`/`commit`/`Drop`/`restore_ticket`/`acknowledge_restore`/`failed_update`/`failure_unseen`/`schedule`/`apply_scheduled` | slTerm 数据目录 | 文件是持久证据，崩溃后状态可重建；restore ticket ≤3 次尝试。 |
| 32 | `update_download/handoff.ps1` + `platform/update_installation.rs` `spawn_prepared_helper`/`installation_in_progress`/`guard_base` | `SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR` | helper 独立提权子进程跑版本验证/快照重启/env 恢复；无 commit.json 无安装权威；env 名落地改 `SLTERM_*`。 |
| 33 | `platform/distribution.rs` + `update_check.rs` distribution 检查 | `slterm-distribution` 标记 | `externally_managed` 闸门：外部托管渠道直接退出自动更新；初期 zip 便携天然放行，为将来安装器留位。 |
| 34 | `update_check.rs` · `test_source` | 本篇演练通道 | 注入假 release 服务器、永不回落公网，是发布流程彩排与排障位（infra 归 11 篇）。 |

### 开机启动（点 35–36）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 35 | `nebula_app/src/platform/startup.rs` · `set_launch_at_login`/`startup_shortcut`/`launch_at_login` | 06 `launch_at_login` 键 | Startup 已知文件夹 `.lnk`（`IShellLinkW`、静默参数、工作目录 home），免注册表免管理员幂等重写。 |
| 36 | `startup.rs` `start_hidden` + `platform/capabilities.rs` `Capabilities::hide_window_on_close` | 06 `silent_start`/`tray` 键 | 静默启动 = `silent_start && tray` 两条件合取（`hide_window_on_close` 因子随关窗即退定位消去，改造节 1），无托盘静默会把应用藏成不可达进程。 |

### 窗口特效（点 37–41）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 37 | `nebula_app/src/gpui_shell/wallpaper.rs` · `background_appearance`/`initial_background_appearance`/`effective_material` | 06 `BlurMode`（D09-1） | 模糊只走 GPUI `WindowBackgroundAppearance` 平台通道，壳不自调 DWM backdrop；切档显式清另一通道。 |
| 38 | `wallpaper.rs` · `VisualEffects`/`refresh`/`refresh_surface_opacity`/`window_opacity`/`chrome_surface_opacity` | 06 `opacity` 键 | 透明度只透壳底色与终端默认背景，全窗 alpha 让文字对比度塌掉。 |
| 39 | `wallpaper.rs` `Wallpaper`/`update_wallpaper` + `renderer/image` `BackgroundImageFit`/`BackgroundImageAlignment`/`wallpaper_rect` | 06 `background_image_*` 五字段 | 壁纸 = 底色之上、单元格之下一层图，fit/alignment/cover_chrome/独立透明度。 |
| 40 | `gpui_shell/wallpaper/image_loader.rs` · `load`/`load_preview`/`Request::cancelled`/`FileStamp`/`LoadedImage` | 本篇后台线程 | 单 job 串行 + 有界（64/128MiB/2048 边）让 UI 线程零文件 I/O；generation 过期即弃。 |
| 41 | `wallpaper.rs` `set_opacity_live` + `wallpaper/preview.rs` | 06 设置页 | live 调透明度即时预览 + 预览图小尺寸分支。 |

### 显示器 / DPI（点 42–43）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 42 | `platform/startup.rs` · `primary_display_scale` | 本篇首窗创建前 | `MonitorFromPoint`+`GetDpiForMonitor` 让初始网格尺寸一步到位，多显 DPI 其余整包交 GPUI 平台层。 |
| 43 | `gpui_shell/workspace/windowing/startup_geometry.rs` · `preferred_size`/`prepare_initial_grid`/`default_size`/`fit_preflight_size`/`same_device_size` | 本篇首窗 | 按基准字号量网格而非持久化缩放，主显 95% 上限 + 最低地板 + 同设备亚像素容忍不二次 `SetWindowPos`。 |

### shell 集成交界（点 44–45）

| # | pebrel 源 · 符号 | 缝合点 | 因果链 |
|---|---|---|---|
| 44 | `nebula_app/src/platform/shell_integration.rs` Windows 分支（空操作） | 02 OSC 133 上报链 | Windows 侧自家 OSC 133 归 02 篇，本片不重复登记。 |
| 45 | `notify.rs` `command_finished` 的 `hook_seen` 参数语义 | 02/04 `CommandDone{exit_code}` | 133;D 退出码进 `CommandDone`/`CommandFailed`、133;C 建 running 边沿；hook 已确认的短成功命令不再通知。 |

### 改名映射引用（不重复登记，唯 01 篇总表为权威）

- `nebula_app` → `slterm_app` 等 crate/路径改名：引 01 篇总表 C 节。
- AUMID `com.slterminal.terminal`：引 01 篇总表 P-3（点 10 消费）。
- env `PEBREL_UPDATE_RESTORE`/`PEBREL_CONFIG_DIR`/`NEBULA_CONFIG_DIR` → `SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR`：引 01 篇总表 C-env 行（点 32 消费）。
- 分发标记文件 `slterm-distribution`：引 01 篇总表 D-section（点 33 消费，改造节 7）。
- `NebulaTick` → `SltermTick`：事件名随 01 篇 `nebula_*` → `slterm_*` 总规则。

### 缝合点登记（本片消费侧）

1. **02 篇**：消费 `OscEvent::{Bell,Notify,Progress}`、`CommandDone{exit_code}`；OSC 9;4 原始 state 原样上抛（点 21）。
2. **03 篇**：消费 `TurnDone`/`NeedsAttention`（带 `AiTurnOutcome`/`AttentionContext`/`AgentKind`）喂漏斗 `AiTurn`/`AiTurnIssue`；agent 退出清进度（点 22）。
3. **04 篇**：`RuntimeTaskState` 投影喂托盘 agent 清单（点 4/5）与 05 tab 徽标；FocusWindow 单语义（还原最小化+选中 tab+聚焦 pane）供 toast/托盘/动作按钮/ATTACH 四路共用（点 16）；**04 篇开放问题 4 的答案与「还原窗口」语义回登见改造节 1**；退出链 `RuntimeServer` Drop 承载托盘 `shutdown`（点 8）。
4. **05 篇**：消费 `PaneId`/`TabId`/`WorkspaceTab` 做 pane 级节流键与 tab 选中；壳内 toast 通知面有无归 05（本片不做，点 13 不采纳项）。
5. **06 篇**：设置键 `tray`/`silent_start`/`auto_check_updates`/`background_image_*`/`launch_at_login`/`notification_duration` 单点在 06；**BlurMode 值域对齐 06 D06-4 见改造节 3**。
6. **11 篇**：真实 toast 投递/Mica 渲染/真实 `.lnk` 登记/真实 OSC 上抛的测试豁免登记归 11；`update-test-source` 演练 infra 归 11（本片只留运行时面，点 34）。
7. **12 篇**：GitHub Releases 坐标（owner/repo、资产命名 `slTerminal-*`、官方下载 URL 前缀、SHA256SUMS 策略）归 12 篇（点 26）。

## 改造 / 移植 / 新建设计

### 1. 单窗语义重排：托盘 / 移交 / 窗口管理立面（回答 04 篇开放问题 4，回登其缝合点 5）

pebrel 的托盘与通知里藏着「多窗 + Quick terminal + mux 驻留」假设；slTerminal 单窗单实例 + 关窗即退（无 mux），逐条重排：

**焦点命令坍缩。** pebrel `focus_best` 的三级兜底（先等输入的 pane → 任意 agent pane → 裸窗口）在单窗下简化为两级：窗内若有满足条件的 pane 则聚焦之，否则聚焦主窗本身。`GpuiTrayCommand::Focus(pane)` 的 `pane` 参数在单窗下不可能是别窗的 pane——托盘右键清单来自本窗 runtime snapshot，语义天然闭合。

**「显示窗口」= 还原最小化 + 前置。** 单窗下 `MENU_SHOW` 与 `Focus(None)` 同义：`IsIconic` → `SW_RESTORE`，再 `SetForegroundWindow`。这条路径与 toast 点击激活、动作按钮、ATTACH 移交唤醒四路合并为 04 篇登记的同一 FocusWindow 语义，本篇章登记为唯一实现点，04/03 消费。

**回答 04 篇开放问题 4（单实例移交 ATTACH 的「还原窗口」在单窗下何义）：** pebrel 多窗形态里 ATTACH 命令可唤醒一个隐藏/最小化的其他窗口并还原；slTerminal 只有一扇主窗，不存在「被唤醒的其他窗」。故 ATTACH 的窗口成分坍缩为「聚焦唯一主窗（最小化则还原）」，tab.new 成分原样保留（新 tab 开在主窗内）。**此答案回登 04 篇缝合点 5**：04 的移交执行器调本篇章暴露的 `focus_window()` facade，不再自带还原逻辑。

**托盘「退出」是真退出。** 单窗下托盘 `MENU_QUIT` 不玩「关窗留驻」：直接走 04 篇退出链（等同关窗），`hide_window_on_close` 设置键在 slTerminal 不存在（关窗即退是定位约束），静默启动的 `start_hidden` 三条件里的 `hide_window_on_close` 因子替换为「关窗即退恒真」——即静默启动实际由 `silent_start && tray` 两条件决定。pebrel `Capabilities::hide_window_on_close` 在 slTerminal 恒 `false`，对应条件从公式中消去而非保留默认值。

**窗口管理 facade。** 本篇新建 `slterm_app/src/windowing/focus.rs`：

```rust
pub fn focus_window(cx: &mut App);              // IsIconic→SW_RESTORE + SetForegroundWindow
pub fn focus_pane(pane: PaneId, cx: &mut App);  // focus_window + 选中所在 tab + 窗内聚焦 pane
pub fn window_hwnd(cx: &App) -> Option<isize>;  // 任务栏进度/闪烁的 HWND 源
```

`focus_notification`（toast/托盘/按钮/ATTACH 四路）在 pebrel 位于 `workspace/windowing.rs`，slTerminal 归本篇 facade，05 篇壳重建标题栏三钮的「关闭」走同 facade（关窗即退）。

### 2. Quick terminal 死刑清单

pebrel 中以下 Quick terminal 专用面随「单窗 + 无驻留」定位整支不移植，逐条列死因，防误抄：

| pebrel 面 | 死因 |
|---|---|
| `gpui_shell` Quick terminal 专用窗型/几何 | 单窗：无第二窗型。 |
| Quick 与主窗间的会话移交/唤醒桥 | 无 Quick 即无桥。 |
| 托盘「新建 Quick terminal」菜单项 | 菜单项删除；托盘右键收敛为主窗聚焦 + agent 直达 + 退出。 |
| Quick 独立 toast/进度路由 | 进度/toast 只挂主窗 HWND（`window_hwnd`）。 |
| mux 驻留的隐藏宿主进程语义 | 关窗即退，无驻留宿主，ATTACH 只对本进程实例。 |

### 3. 模糊/透明度值域对齐 06 篇 D06-4（登记 D09-1）

06 篇 D06-4 待沉淀：BlurMode 值域留待本片定夺。本篇依据采纳点 37（整条材质通道设计照抄，无枚举则通道无意义）裁决：

**保持 pebrel 五材质枚举 `BlurMode { None, Mica, MicaAlt, Aero, Acrylic }`，默认 `None`，字符串值不变。** `BlurModeName::from_settings` 映射表照抄（`nebula_settings/src/lib.rs`）。运行时档 gate 照抄点 37：Mica/MicaAlt 需 build ≥ 22621 走原生枚举，≥ 17763 低档 Aero/Acrylic 走 `Blurred` AccentPolicy，之下纯透明；切档显式清理另一通道。

```rust
pub enum BlurMode { None, Mica, MicaAlt, Aero, Acrylic }   // 字符串序列化值同 pebrel
```

> [待沉淀] **D09-1：BlurMode/opacity 值域冻结。** 本片裁决与 06 D06-4 绑定，冻结截止 = 06 篇 M6.1 设置字段冻结点；此前 06 若改值域本片跟改，逾期不改。

透明度与 D06-4 对齐的另一面：`opacity` 只经 `VisualEffects.window_opacity` 透壳底色 + 终端默认背景（点 38），不引入全窗 alpha；该纪律在 06 的设置 schema 注释里登记。

### 4. background_tasks 收编：注册表契约保留 + 执行载体换 GPUI executor

旧双端调度器（`src/features/backgroundTasks/` 前端半 + `src-tauri/src/background_tasks/` 后端半）的 WebView/IPC 载体全灭，元数据契约原样保留（见关键类型节签名）。改造点：

- **载体替换**：poller 循环挂 GPUI background executor（`cx.background_spawn`），`TaskExecutor = fn()` 取代 `fn(tauri::AppHandle)`。`planBalance` 的 executor 体归 10 篇（余额轮询逻辑不变，载体签名变）；`sessionRefresh` 维持 `executor: None`（订阅驱动，无订阅不轮询，归 03 ai_sessions）。
- **节拍来源**：interval 到期唤醒走 GPUI executor 定时，不新增 OS 定时器句柄；托盘 1Hz 与更新 12s 延迟线程同理挂既有拍/后台执行器。
- **设置应用**：`applyConfig` 直写 `RUNTIMES` 原子量（enabled/interval_sec），不重建循环；写入立即生效。
- **持久化**：`SETTINGS_KEY` 落 slTerm 数据目录，形态不变；失败不落盘（见关键类型节失败策略）。
- **注册表契约**：模块级单例 + `TASKS` 静态切片 + side-effect import 注册 + `_reset()` 仅测试用——旧仓 13 条注册表家族契约原样继承（`_reset` 形态随 L1 测试需要保留）。

### 5. source classification 重生：CLI 身份判定单点 AgentKind::parse

旧前端通知的「类别判定委托 CLI profile」在 Rust 漏斗重生为：**漏斗不认事件文本关键字，来源标注一律问 `AgentKind::parse`（03 篇 CLI profile 注册表单点）**。判定顺序：

1. OSC 133 宿主元数据带 agent id → 直接 `AgentKind`。
2. 无元数据 → 按 pane 的启动命令查 profile 注册表。
3. 查不到 → 来源标注 `None`，toast 不标来源、托盘菜单不进该 pane。

通知的 `AiTurn`/`AiTurnIssue` 变体构造前必须拿到 `AgentKind` + `AiTurnOutcome`/`AttentionContext`（03 篇锚点），拿不到则降级为 `Text`/`Bell` 通用变体，不阻塞投递。这条消灭旧 `classifyEvent` 按文本猜来源的整条歧义面。

### 6. notification_duration 新消费面（WinRT duration 映射）

pebrel toast xml **无 duration 属性**（`windows.rs::xml` 全文核对），而 06 篇设置键 `notification_duration` 已锚定（Default = 不覆盖）。本篇新建该键的消费面：WinRT `ToastNotification` 的 `NotificationDuration` 映射——

```rust
// settings.notification_duration: Default | Short | Long
// Default → xml 不写 duration（同 pebrel 现状）；
// Short   → duration="short"；Long → duration="long"（WinRT 合法值）。
```

映射落在 MTA worker `show` 组装 xml 处；Default 形态与 pebrel 逐字节一致，非 Default 才注入属性。设置页预览 eg 06。

### 7. 自动更新链 slTerminal 化

- **资产合同单名化**：pebrel `windows_x64_installer_names` 三元组塌缩为 `slTerminal-*-setup.exe` 单名（不采纳项 11）；无 legacy 品牌回退、无双官方前缀，`LEGACY_RELEASE_DOWNLOAD_PREFIX` 不移植。具体命名归 12 篇。
- **环境变量改名**：`PEBREL_UPDATE_RESTORE`/`PEBREL_CONFIG_DIR`/`NEBULA_CONFIG_DIR` → `SLTERM_UPDATE_RESTORE`/`SLTERM_CONFIG_DIR`（01 篇 C-env 行）；`handoff.ps1` 全文检索替换 + helper 进程 env 注入同步改。
- **多平台面整删**：macOS dmg 资产/`koly` trailer 校验（不采纳项 10）、Linux 通知通道、非 Windows `show` 全删。
- **legacy 投递臂全删**：`notify.rs` 的 `PROXY`/`init_proxy`/按窗 `deliver`（不采纳项 7）与 `tray.rs` `init(EventLoopProxy)`（不采纳项 8）不移植；失焦判定改问 GPUI 焦点态。
- **应用图标变体塌缩**：`AppIconName` 变体族不采纳（不采纳项 12），托盘/toast/任务栏图标单一；物化/注册逻辑照抄，变体维度塌缩为单态，`REGISTERED` 缓存键去变体分量。
- **单进程下的「杀 helper 目标」**：pebrel helper 杀的是旧进程再启新进程；slTerminal 单进程同义，无 mux 会话可丢，restore ticket 恢复语义不变（工作区快照 = 布局序列化，05/07 篇锚点）。
- **守卫锁与安装目录探测**照抄（`guard_base`/`installation_in_progress`），同二进制全配置共享。
- **应用数据目录**：更新状态/缓存/handoff 事务目录全部落 slTerm 应用数据目录（便携语义），`update_state_path`/`cache.rs`/`handoff` 根路径单点注入。

## 测试点清单

本篇只登记本片代码面的测试点；测试基建形态归 11 篇，发布验证链归 12 篇，豁免登记归 11。

### 通知漏斗（纯函数 + 时钟注入，L1/L2 等价层按新栈重定）

- 焦点闸门：窗口聚焦时全部变体零投递。
- 闪烁先行：toast 被全局/pane 双层节流拦截时，闪烁调用仍恰好发生一次。
- 全局节流：3s 窗口内第 2 条 toast 被吞，窗口外放行。
- pane 失败冷却：同 pane+程序+结局的 episode 内 30s 只放 1 条；恢复事件到达结 episode，之后同错重新可弹；新错（异 message）立即可弹；跨 pane 互不干扰。
- `clamp_toast_body`：>160 截断加省略号、空白折叠、幂等（再截一次不变）。
- `Notification::command_finished`：`hook_seen=true` 的短成功命令（<`COMMAND_NOTIFY_MIN`）零通知；带非零退出码必成 `CommandFailed`。
- `source classification`：元数据缺失时查 profile 注册表，查不到降级 `Text`/`Bell` 不 panic。

### 任务栏进度

- `from_osc`：0/1/2/3/4 规范码映射到五态；未知码（含 `9;4;5;0` 实测形）归清除态。
- `is_active`/`taskbar_flag`/`percent`：各态的 ITaskbarList3 标志与百分比正确。
- 清除路径：agent 退出事件到达即归清除态，无残留。

### 自动更新（文件系统隔离 + 假 HTTP，全 L1 可测）

- `UpdatePromptState`：无记录/到期/未到期/remind-later 3 天/skip 单版本互不串味；`UPDATE_STATE_LOCK` 双进程抢写不坏文件。
- 版本比较：`is_newer` 点分段数值、段内后缀忽略、拒绝降级、等版本不提示。
- 资产合同：名/架构/版本/URL 前缀四重校验，digest 字段优先、release body 回退；`normalize_sha256` 大小写/空白容忍。
- 下载作废链：`begin`→`cancel` 后旧 `run` 的进度/完成写入失配失效；换版本代际失配。
- 下载校验：`.part` 流式，长度/MZ/SHA-256/512MiB 上限任一不过不原子替换。
- hydrate 双证据：无网启动可恢复就绪态/失败态；`installation_failure_unseen` 以 mtime 晚于提示态判真。
- handoff 文件机：`plan→ready→commit`/`plan→ready→cancel` 双路径；孤儿 `plan`（无配对）按 cancel 安全默认；restore ticket 第 3 次尝试后仍败写票；`commit`/`Drop` 幂等。
- 代理解析：`parse_windows_proxy_server` 按协议取值、socks→socks5、env 回退、`NO_PROXY`、https_only。
- `Distribution::externally_managed`：带标记目录 `require_direct_update` 为假。

### 托盘

- 菜单构建不开线程：`build_menu` 对快照清单产出 HMENU 结构正确，命令 id 映射回打开时刻清单。
- 状态快照：`update` 内容相等直接返回（1Hz 去抖）；attention 翻转驱动双图标选择。
- 收尾：`shutdown` 发 `WM_APP_SHUTDOWN` 删图标退泵。

### 窗口特效

- `BlurMode` 五材质字符串 round-trip；build gate 矩阵（≥22621/≥17763/之下）选通道正确；切档清理另一通道。
- 透明度：`window_opacity` 只作用壳底色与终端默认背景（文字层 alpha 恒 1）。
- 壁纸：`bounded_dimensions` 超预算等比缩；`Request::cancelled` 代际失配判真；`FileStamp` 路径+戳不等即重解码。
- 启动几何：`same_device_size` 亚像素容忍内不二次 `SetWindowPos`；主显 95% 上限与最低地板生效。

### 后台任务注册表

- 订阅生命周期：无订阅不轮询；订阅归零即停。
- 重入闸门：触发未完到点跳过不叠加。
- 失败策略：单轮失败照常排下轮，SETTINGS_KEY 不落失败态。
- `applyConfig` 写原子量立即生效，不重建循环。

### 真实面（登记豁免，11 篇汇总）

真实 toast 投递（系统抑制/AUMID 生效）、Mica 实际渲染、真实 `.lnk` 写 Startup、真实 OSC 上抛链、helper 真实提权安装——运行时逻辑全部由上述假面对偶覆盖，真实面仅 smoke。

## 阶段归属与出口标准

本片属 00 篇路线图 **M10 系统集成半程**（M10 的另一半安全面归 10 篇）。原子步与机器可验出口：

| 步 | 内容 | 机器可验出口 |
|---|---|---|
| M10.1 | 通知漏斗 + AUMID/toast worker + WinRT duration 消费 | 上述漏斗测试全绿；`notify-test` 诊断命令端到端打印每段 OK |
| M10.2 | 任务栏进度消费 + 清除路径 | OSC 9;4 五态/未知码映射测试全绿；agent 退出清进度测试绿 |
| M10.3 | 托盘线程 + 菜单 + 1Hz 推送 + 橙点 | 菜单构建不开线程测试绿；1Hz 快照去抖测试绿 |
| M10.4 | 窗口管理 facade（focus_window/focus_pane/window_hwnd） | 四路回窗（toast/托盘/按钮/ATTACH）汇到 facade 的编译期单一实现点；ATTACH 用例绿 |
| M10.5 | 自动更新链（检查→下载→handoff） | 假 release 服务器全链演练绿（infra eg 11）；文件机/作废链/代理解析测试全绿 |
| M10.6 | 开机启动 .lnk + 静默启动两条件 | `.lnk` 写读 round-trip 测试绿；两条件真值表测试绿 |
| M10.7 | 窗口特效 + 壁纸 + 启动几何 DPI | BlurMode gate 矩阵/透明度纪律/壁纸有界/几何容忍测试全绿 |
| M10.8 | background_tasks 收编 | 注册表生命周期/重入/失败策略测试绿 |

M10 安全半程（权限确认 toast 动作按钮的确认语义、guard 锁安全面）归 10 篇，本片只交付 `deliver_gpui_with_choices` 的通道面。

## 待沉淀决策

> [待沉淀] **D09-1：BlurMode/opacity 值域冻结**——本片裁决五材质枚举默认 None，与 06 D06-4 绑定；冻结截止 06 篇 M6.1 设置字段冻结点，逾期不改。

> [待沉淀] **D09-2：更新首发型态**——zip 便携期 GitHub Releases 资产究竟是「单一 setup.exe 两阶段自覆盖」还是「绿色 zip 全量替换」尚未裁定；影响点 26 资产合同与点 32 helper 是否首版即需。默认按 setup.exe 形态建（pebrel 同构），若首版定绿色 zip 则 handoff 链 M10.5 降级为整包解压替换。截止 M10.5 开工前。

## 开放问题

1. **「还原窗口」语义回登 04**：本片改造节 1 已回答 04 开放问题 4 并回登其缝合点 5；04 正式稿若改判（如保留多窗伏笔），本篇 facade 签名不变、实现回扩。