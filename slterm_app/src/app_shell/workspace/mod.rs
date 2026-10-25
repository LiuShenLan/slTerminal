//! 工作区:tabs/pane 生命周期的唯一属主。
//!
//! ## 分屏 pane 生命周期合同
//!
//! 1. 每个 Terminal tab 持有 `panes`(实体属主,无序按 id 查)+ `tree`
//!    (`slterm_split` 布局树,叶 = pane id)+ `focused`;**panes 的 id 集合
//!    == 树的叶集合**,贯穿新建 tab / split / close 全路径。
//! 2. pane id 全 workspace 唯一、终生不复用;分配权威 = 本模块单调计数器
//!    `next_pane` / `next_tab`(newtype 不持全局态)。
//! 3. 关 pane 的结局由 `RemoveOutcome` 三态裁定:`WasRoot` 关整个 tab、
//!    `Collapsed(id)` 兄弟收编且焦点交幸存子树首叶;壳不自行散判。被摘
//!    pane 立即显式 `shutdown`;建树挂不上时立即回收 pane,不留孤儿 PTY。
//! 4. 布局持久化只经 session schema(快照/恢复已接线,见
//!    `session_persistence`/`session_recovery`);本模块运行时树与持久化
//!    树的转换只走 `session_restore` 一对纯函数。
//!
//! ## 壳层职责
//!
//! 壳层只做四件事:按**提交比例**渲染递归、canvas 探针回写节点/pane 矩形、
//! 分隔条拖拽 overlay、松手提交。分屏语义(比例/树变换)全由 `slterm_split`
//! 裁定,壳层不重算。渲染只读提交比例,`preview_ratio`/`dragging` 只喂
//! overlay 与分隔条跟随——拖拽期间终端网格零重排(TTY 尺寸链交界:
//! 结构性变化经 `TerminalView::mark_structural_resize` 同步下发,不进尾沿
//! 去抖)。

/// 壳事件、更新交互与关闭守卫。
mod control;
/// 文件打开路由(`openable_in_app` → Image/Document/Code)。
mod documents;
/// 侧栏文件树渲染 + 路径栏 + 拖放。
mod file_tree;
/// 键位 JSON 段消费 + EDITABLE_ACTIONS + 保留键红线。
/// 暴露给设置页键位编辑(EDITABLE_ACTIONS/RESERVED_ROWS/clear/reset)。
pub(crate) mod keyboard_bindings;
/// 后台泵与系统关窗(自动保存 / AI hook / 壳事件)。
mod pumps;
/// 终端 tab 与分屏树渲染(壳层职责下沉)。
mod render;
pub(crate) mod session_persistence;
pub(crate) mod session_recovery;
pub(crate) mod sidebar;
/// 侧栏三视图渲染(导航树 / Commit / Agent 全局文件)。
mod sidebar_views;
mod split_drag;
mod tab_drag;
pub(crate) mod title_bar;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels,
    PromptLevel, Render, Styled as _, Subscription, Window, div, px,
};
use gpui_component::WindowExt as _;
use slterm_settings::Rgb;
use slterm_split::{PaneId, RemoveOutcome, SplitDirection, SplitNav, SplitTree, TabId, nav_target};

use super::SltermShellEvent;
use super::file_editor::SaveFile;
use super::settings_pane::{SettingsPane, SettingsPaneEvent};
use super::windowing::{WorkspaceStartup, startup_geometry};
use crate::agent_host::AgentHub;
use crate::ai_hook::AiHookEvent;
use crate::app_shell::config::Settings;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};
use crate::i18n::Message;
use crate::session::LaunchSession;
use render::to_split_rect;
use session_persistence::SessionPersistence;
use session_recovery::StartupPlan;
use sidebar::{SidebarResizeDrag, SidebarSplitDrag, SidebarState};
use split_drag::SplitDrag;
use tab_drag::TabDrag;

gpui::actions!(
    slterm_workspace,
    [
        SplitRight,
        SplitDown,
        ToggleZoom,
        FocusPaneLeft,
        FocusPaneRight,
        FocusPaneUp,
        FocusPaneDown,
        CloseActivePane,
        NewTerminalTab,
        SelectNextTab,
        SelectPreviousTab,
        CloseActiveTab,
        ToggleSidebar,
        OpenSettings,
    ]
);

/// 一个终端 pane:视图实体 + 宿主订阅。id 全 workspace 唯一、终生不复用
/// (hook 的 `SLTERM_PANE_ID` 环境值与此同源)。
pub struct TerminalPane {
    pub id: PaneId,
    /// 用户重命名(持久化归 05 session 叶;无重命名 UI,恒 None)。
    pub custom_name: Option<String>,
    /// 本 pane 创建时冻结的启动身份;快照逐叶落盘。
    pub launch: Option<LaunchSession>,
    pub view: Entity<TerminalView>,
    /// 补全来源缓存按 pane 持有；缓存本体不复制，消费经视图的同源句柄。
    #[allow(dead_code)]
    pub(crate) completion_session: Rc<crate::completion::Session>,
    _subscription: Subscription,
}

/// 唯一合法 tab 类型集(面板封闭的 Rust 落点)。本阶段 Terminal +
/// Settings + Document/Code/Image 三个文件 tab。
///
/// Settings 是**单例 tab**:再次打开只激活既有实例,不新建;不进会话
/// 快照(快照只收 Terminal tab)。Document/Code/Image 同样
/// 不进快照——纯视图 drop,无进程/PTY 生命周期。
pub enum WorkspaceTab {
    Terminal {
        /// 实体属主,无序按 id 查。见模块头生命周期合同。
        panes: Vec<TerminalPane>,
        /// 叶 = pane id。
        tree: SplitTree<PaneId>,
        /// 键盘焦点与 split/close 作用对象。
        focused: PaneId,
        /// 聚焦 pane 满卡(内存态,快照不含);任何结构性操作先解除。
        zoomed: bool,
    },
    Settings {
        /// 设置页实体(单例;写通道与渲染态在其内部)。
        view: Entity<SettingsPane>,
        /// 宿主事件订阅(Changed/Close);随 tab 一起释放。
        _subscription: Subscription,
    },
    /// Markdown 文档 tab（渲染态以只读文本占位）。
    Document {
        view: Entity<super::doc_tabs::DocTabView>,
    },
    /// 源码 tab（普通文件可编辑，gitshow 只读臂）。
    Code {
        view: Entity<super::code_tab::CodeTabView>,
    },
    /// 只读图片 tab（后台解码）。
    Image {
        view: Entity<super::doc_tabs::ImageTabView>,
    },
}

/// tab 级展示态元数据(持久化四字段形态)。
#[derive(Default)]
pub struct TabMeta {
    /// 用户命名;标题栏展示名第一优先源。
    pub custom_name: Option<String>,
    /// 用户色标;侧栏色标消费归后续步骤。
    pub color: Option<Rgb>,
    /// 建 tab 时冻结的启动身份;快照与恢复消费。
    pub launch: Option<LaunchSession>,
}

/// tab + 展示态元数据同槽存放(单 Vec 消下同标耦合)。
pub struct TabEntry {
    /// 稳定身份:跨排序/活跃切换不变;消费面 = 侧栏/dock/节点矩形缓存键。
    pub id: TabId,
    pub tab: WorkspaceTab,
    pub meta: TabMeta,
}

/// 壳内状态权威:直持 pane 实体与展示态,无投影层。
pub struct SltermWorkspace {
    tabs: Vec<TabEntry>,
    /// 位置索引(活跃定位);TabId 为稳定身份。
    active: usize,
    /// 单调分配器(PaneId/TabId 唯一性权威)。多窗扩展时按窗口分段。
    next_pane: u64,
    next_tab: u64,
    /// spawn 网格:开窗定形后由 `prepare_initial_grid` 反推,首个 pane 的
    /// 出生几何(启动零 resize 锚)。
    initial_grid: (u16, u16),
    /// PaneBoundsStore:canvas 探针 prepaint 回写(键 = pane id)。
    pane_bounds: PaneBoundsStore,
    /// SplitBoundsStore:canvas 探针 prepaint 回写(键 = (稳定 TabId, 节点路径))。
    /// 下标会随关 tab/重排漂移,稳定身份才是键。
    split_bounds: SplitBoundsStore,
    /// 进行中的分隔条拖拽;`None` = 无手势。
    split_drag: Option<SplitDrag>,
    /// 进行中的 tab 拖拽(含未过阈值的待命态);`None` = 无手势。
    tab_drag: Option<TabDrag>,
    /// 侧栏骨架态:开合/宽度/双槽 active view。
    sidebar: SidebarState,
    /// 进行中的侧栏宽度拖拽;`None` = 无手势。
    sidebar_resize: Option<SidebarResizeDrag>,
    /// 进行中的双槽分隔条拖拽;`None` = 无手势。与 `sidebar_resize` 互斥。
    sidebar_split_drag: Option<SidebarSplitDrag>,
    /// 侧栏双槽内容区上一帧矩形(canvas prepaint 回写;分隔条拖拽的
    /// 指针 y → 比例换算读取点)。
    sidebar_split_bounds: SidebarSplitBoundsStore,
    /// 虚拟窗口用例的三钮命令记录(生产不编译;平台窗口 API 在测试
    /// 平台是 unimplemented,行为断言只能落在命令分派端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    window_commands: Vec<title_bar::TitleBarCommand>,
    /// 虚拟窗口用例的托盘「退出」记录(生产不编译;测试直接调用
    /// `remove_window` 会绕过关窗钩子,断言只能落在请求端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    tray_quit_requested: bool,
    /// 每 pane 的 agent 宿主(AgentActivity 归属表)。
    agent_hub: AgentHub,
    /// 后台任务订阅:字段只为持有订阅生命周期;归零即停轮询。
    background_tasks: Vec<crate::background_tasks::TaskSubscription>,
    /// 任务栏进度的进程级单显示路由:只认活跃 tab 聚焦 pane。
    taskbar: crate::taskbar::TaskbarState,
    /// 单窗口会话写盘状态机(latest/saved 双快照、无变化跳过、提权隔离)。
    session_persistence: SessionPersistence,
    /// 文件 tab 的变更观察(键 = 稳定 TabId):编辑器键入/保存完成时刷新
    /// 侧栏与窗口标题的脏标记。关闭 tab 时随条目一并释放。
    file_observations: HashMap<TabId, Subscription>,
    /// code tab 事件订阅(gutter 点击 → 打开差异对照面)。
    file_tab_events: HashMap<TabId, Subscription>,
    /// 脏关闭确认在途的 tab:同一时刻只允许一个确认框。
    pending_file_close: Option<TabId>,
    /// 整窗关闭确认已放行:异步回调 `remove_window` 前落位,防重入守卫。
    window_close_confirmed: bool,
    /// 忙碌 pane 关闭确认已放行(防异步回调重入弹框)。
    pane_close_confirmed: Option<PaneId>,
    /// 更新弹窗/安装授权单槽;整个更新交互(含下载与二次确认)期间
    /// 为 true,重复事件直接丢弃。
    update_prompt_open: bool,
    /// 测试专用:更新安装执行替身(生产恒走 handoff)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    update_install_runner: Option<control::UpdateInstallRunner>,
    /// 测试专用:安装成功后的退出请求(虚拟窗口不可直调 remove_window)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    update_install_exit_requested: bool,
    /// 侧栏文件树视图状态(explorer 槽内容)。
    file_tree: file_tree::FileTreeState,
    /// AgentFiles 槽的进程内目录缓存:首次渲染加载,禁止每帧 27 次
    /// `is_dir`;显式刷新经 `refresh_agent_dirs`。测试构建预置空表,避免
    /// UI 用例读真实 USERPROFILE(替身经 `set_agent_dirs_for_test` 注入)。
    agent_dirs_cache: Option<Vec<crate::agent_dirs::AgentGlobalDir>>,
    /// 设置页当前读写路径(热重绑重读 keybind 段时与设置页同一份文件);
    /// `None` = 生产默认数据目录,测试经 `open_settings_tab_at` 注入 tempdir。
    settings_path_override: Option<PathBuf>,
    /// 测试专用:Commit 视图注入快照(渲染 selector 的确定性数据源)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    commit_snapshot_for_test: Option<std::sync::Arc<crate::side_panel::GitInfo>>,
    /// 外部文件修改监听(树根递归 + 打开文件父目录)。事件驱动刷新
    /// gutter/目录树,不做定时目录重扫;watcher 不可用时为 None。
    fs_watch: Option<super::fs_watch::FsWatch>,
    /// 窗口激活态订阅:通知漏斗前台门控的唯一写入源。
    /// 字段本身只为持有订阅生命周期。
    _window_activation: Subscription,
}

/// 键:pane id(方向导航要拿所有叶子的屏幕矩形算最近邻)。
type PaneBoundsStore = Rc<RefCell<HashMap<PaneId, Bounds<Pixels>>>>;
/// 键:(稳定 TabId, 分屏节点路径)。路径语义同 `slterm_split::Divider`。
///
/// 用 TabId 而非下标:关 tab 会让后续 tab 的下标整体前移,旧下标键会被
/// 幸存 tab 误读(拖分隔条读到别的 tab 的上一帧矩形);TabId 终生不复用,
/// 重排/关闭都不改键。关闭 tab 时仍显式清键,避免缓存无界增长。
type SplitBoundsStore = Rc<RefCell<HashMap<(TabId, Vec<bool>), Bounds<Pixels>>>>;
/// 侧栏双槽内容区矩形(单值;分隔条拖拽只在双槽打开时存在)。
type SidebarSplitBoundsStore = Rc<RefCell<Option<Bounds<Pixels>>>>;

/// 启动身份冻结:实际 shell 命令 + 参数 + 出生目录进 `LaunchSession::Profile`
/// (Profile 内嵌完整命令,导出工作区时可移植,session schema 契约)。
fn launch_session_of(
    shell: &slterm_terminal::tty::Shell,
    cwd: Option<&std::path::Path>,
) -> LaunchSession {
    LaunchSession::Profile {
        name: shell_display_name(shell.program()),
        command: shell.program().to_owned(),
        args: shell.args().to_vec(),
        cwd: cwd.map(|path| path.to_string_lossy().into_owned()),
        shell_id: None,
    }
}

/// 展示短名:prog 路径取文件主干(`pwsh.exe` → `pwsh`),无主干回退全名。
fn shell_display_name(program: &str) -> String {
    std::path::Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| program.to_owned())
}

/// 运行时启动参数里已冻结的 shell → 持久化启动身份。`shell=None`(生产
/// 分屏走 core 探测链)没有可冻结的命令,快照落 `None`,恢复回默认启动。
fn launch_session_from_terminal_launch(launch: &TerminalLaunch) -> Option<LaunchSession> {
    let TerminalLaunch::Local {
        cwd,
        shell: Some(shell),
        ..
    } = launch
    else {
        return None;
    };
    Some(launch_session_of(shell, cwd.as_deref()))
}

impl SltermWorkspace {
    pub fn new(
        window: &mut Window,
        shell_rx: mpsc::Receiver<SltermShellEvent>,
        ai_events: Option<mpsc::Receiver<AiHookEvent>>,
        startup: WorkspaceStartup,
        plan: StartupPlan,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 侧栏宽度先于 spawn 网格入账:恢复的宽度若在首帧后才生效,会先按
        // 默认宽度触发一次 ConPTY resize(/回放顺序)。
        let sidebar = sidebar::reconcile_views(&plan.sidebar, &sidebar::SideViewId::ALL);
        // 开窗 resize 异步落地前,先按目标几何谓出 spawn 网格(启动零 resize)。
        let initial_grid = startup_geometry::prepare_initial_grid(
            window,
            cx,
            sidebar::chrome_width(&sidebar),
            true,
        );
        // 通知漏斗前台门控:初始态取开窗激活态,此后由窗口
        // 激活态订阅持续刷新;漏斗据此丢弃聚焦期间的打扰。
        crate::notify::set_foreground(window.is_window_active());
        let window_activation = cx.observe_window_activation(window, |_workspace, window, _cx| {
            crate::notify::set_foreground(window.is_window_active());
        });
        let mut workspace = SltermWorkspace {
            tabs: Vec::new(),
            active: 0,
            next_pane: 1,
            next_tab: 1,
            initial_grid,
            pane_bounds: Rc::new(RefCell::new(HashMap::new())),
            split_bounds: Rc::new(RefCell::new(HashMap::new())),
            split_drag: None,
            tab_drag: None,
            sidebar,
            sidebar_resize: None,
            sidebar_split_drag: None,
            sidebar_split_bounds: Rc::new(RefCell::new(None)),
            #[cfg(all(test, feature = "gpui-test-support"))]
            window_commands: Vec::new(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            tray_quit_requested: false,
            agent_hub: AgentHub::new(),
            background_tasks: Vec::new(),
            taskbar: crate::taskbar::TaskbarState::new(),
            session_persistence: Self::new_session_persistence(),
            file_observations: HashMap::new(),
            file_tab_events: HashMap::new(),
            pending_file_close: None,
            window_close_confirmed: false,
            pane_close_confirmed: None,
            update_prompt_open: false,
            #[cfg(all(test, feature = "gpui-test-support"))]
            update_install_runner: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            update_install_exit_requested: false,
            file_tree: file_tree::FileTreeState::new(window, cx),
            agent_dirs_cache: Self::initial_agent_dirs_cache(),
            settings_path_override: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            commit_snapshot_for_test: None,
            fs_watch: super::fs_watch::FsWatch::new(),
            _window_activation: window_activation,
        };
        // 搜索框变更订阅要 workspace 实体已存在;文件树其余状态由构造给出。
        workspace.install_file_tree_events(window, cx);
        let restored = plan
            .session
            .as_ref()
            .is_some_and(|session| workspace.try_restore_session(session, window, cx));
        if !restored {
            match startup {
                WorkspaceStartup::NewTerminal { cwd } => {
                    workspace.add_terminal_tab(cwd, None, false, window, cx);
                }
                WorkspaceStartup::LaunchTerminal { cwd, command } => {
                    // `-e` 语义:命令本体即 pane 的启动进程(不是默认 shell
                    // 里再注入一条命令)——冻结进 launch,会话恢复照此重建。
                    // 显式命令通道:core 跳过三家白名单并做裸名 PATH 路径化
                    // (豁免论证见 `Options::explicit_command`)。
                    let name = command.program().to_owned();
                    let shell = slterm_terminal::tty::Shell::new(
                        command.program().to_owned(),
                        command.args().to_vec(),
                    );
                    workspace.add_terminal_tab(cwd, Some((shell, name)), true, window, cx);
                }
            }
        }
        // 恢复结果一次性汇总:
        // 恢复成功按标签数提示(崩溃现场多一句来源说明);断路器隔离时给
        // 出挪走的现场路径。启动期用户虽在前台,但日志不可见,故走直发
        // 通道而非漏斗(前台门控会丢弃这条非打扰信息)。
        let restored_tabs = restored.then_some(workspace.tabs.len());
        let language = cx
            .try_global::<crate::app_shell::config::Settings>()
            .map(|settings| settings.language)
            .unwrap_or(crate::i18n::UiLanguage::EnUs);
        if let Some(text) = session_recovery::recovery_notice(
            restored_tabs,
            plan.crashed,
            plan.quarantined.as_deref(),
            language,
        ) {
            // /统一走通知漏斗;启动汇总经 `plan_startup` 豁免前台
            // 门控,不再使用直发通道(直发通道保留给更新提示)。
            crate::notify::deliver_startup_summary(
                &crate::notify::Notification::RecoverySummary { text },
                cx,
            );
        }
        // 测试构建不启用会话注册表写入(真实数据目录隔离);生产启用。
        if !cfg!(all(test, feature = "gpui-test-support")) {
            workspace.agent_hub.enable_session_registry();
        }
        workspace.start_shell_event_pump(shell_rx, window.window_handle(), cx);
        workspace.start_ai_hook_pump(ai_events, cx);
        workspace.start_agent_screen_watchdog(cx);
        workspace.start_autosave_pump(cx);
        workspace.install_window_close_save(window, cx);
        workspace.sync_window_title(window, cx);
        // 托盘首帧快照:起跑即对齐,不等首秒看门狗。
        workspace.publish_tray_snapshot(cx);
        workspace
    }

    /// AgentFiles 缓存初值:生产懒加载(NULL = 首帧读目录表);测试构建
    /// 预置空表,避免 UI 用例触碰真实 USERPROFILE。
    fn initial_agent_dirs_cache() -> Option<Vec<crate::agent_dirs::AgentGlobalDir>> {
        #[cfg(all(test, feature = "gpui-test-support"))]
        {
            Some(Vec::new())
        }
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        {
            None
        }
    }

    /// 生产 = 真实写盘;测试 = 空写。虚拟时钟推进 1s 会驱动自动保存泵,
    /// 测试写真实 `%APPDATA%` 违反隔离红线;需要观察快照的用例经
    /// `set_session_writer_for_test` 注入捕获器。
    fn new_session_persistence() -> SessionPersistence {
        if cfg!(all(test, feature = "gpui-test-support")) {
            SessionPersistence::with_writer(Rc::new(|_| Ok(())), true)
        } else {
            SessionPersistence::default()
        }
    }

    /// 建一个 Terminal tab(cwd + 可选的显式 shell 身份),并聚焦其 pane。
    ///
    /// 启动身份在**建 tab 这一刻**冻结进 `TabMeta.launch`(问题 1):
    /// 默认 shell 也只在本刻探测一次,设置随后变更只影响下一次建 tab,
    /// 冷恢复按各 tab 自己的 launch 重建,不会把混合工作区抹成同一种 shell。
    fn add_terminal_tab(
        &mut self,
        cwd: Option<std::path::PathBuf>,
        shell: Option<(slterm_terminal::tty::Shell, String)>,
        explicit_command: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let (shell, shell_name) = match shell {
            Some((shell, name)) => (Some(shell), Some(name)),
            // 默认路径:探测结果冻结进 meta 并显式交给 spawn;展示名保持
            // 现状(None → 终端标题 "shell"),不让启动身份冻结改动 UI。
            None => (Self::default_shell(), None),
        };
        let meta_launch = shell
            .as_ref()
            .map(|shell| launch_session_of(shell, cwd.as_deref()));
        let launch = TerminalLaunch::Local {
            cwd,
            shell,
            shell_name,
            explicit_command,
        };
        let pane = self.new_pane(self.initial_grid, launch, window, cx);
        let focused = pane.id;
        let tab_id = TabId(self.next_tab);
        self.next_tab = self.next_tab.saturating_add(1);
        self.tabs.push(TabEntry {
            id: tab_id,
            tab: WorkspaceTab::Terminal {
                panes: vec![pane],
                tree: SplitTree::leaf(focused),
                focused,
                zoomed: false,
            },
            meta: TabMeta {
                launch: meta_launch,
                ..TabMeta::default()
            },
        });
        self.active = self.tabs.len() - 1;
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
    }

    /// 默认 shell 探测:生产走 core 的 pwsh→powershell→cmd 链,测试顶替
    /// 必败命令(真 PTY 线程跨测试存活,测试纪律)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    fn default_shell() -> Option<slterm_terminal::tty::Shell> {
        Some(slterm_terminal::tty::Shell::new(
            "slterm-test-missing-shell-executable".to_owned(),
            Vec::new(),
        ))
    }

    #[cfg(not(all(test, feature = "gpui-test-support")))]
    fn default_shell() -> Option<slterm_terminal::tty::Shell> {
        slterm_terminal::tty::resolve_shell(None).ok()
    }

    /// 新建 tab(ctrl-shift-t):cwd 继承聚焦 pane 的当前目录。
    fn new_terminal_tab(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let cwd = self
            .tabs
            .get(self.active)
            .map(|entry| &entry.tab)
            .and_then(|tab| match tab {
                WorkspaceTab::Terminal { panes, focused, .. } => panes
                    .iter()
                    .find(|pane| pane.id == *focused)
                    .and_then(|pane| {
                        let view = pane.view.read(cx);
                        (!view.cwd.is_empty()).then(|| PathBuf::from(&view.cwd))
                    }),
                WorkspaceTab::Settings { .. }
                | WorkspaceTab::Document { .. }
                | WorkspaceTab::Code { .. }
                | WorkspaceTab::Image { .. } => None,
            });
        self.add_terminal_tab(cwd, None, false, window, cx);
    }

    /// ctrl-tab / ctrl-shift-tab:相邻 tab 循环切换(上游同语义)。
    fn select_adjacent_tab(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tabs.len() < 2 {
            return;
        }
        let len = self.tabs.len();
        self.active = if forward {
            (self.active + 1) % len
        } else {
            (self.active + len - 1) % len
        };
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 点击侧栏 tab 行:切活跃 tab(同下标空操作)。
    fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.active = index;
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 打开设置 tab(单例):已存在则只激活;不存在则新建并订阅写盘/关闭
    /// 事件。生产路径用数据目录 settings.json。
    pub(crate) fn open_settings_tab(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.open_settings_tab_inner(None, None, window, cx);
    }

    /// 显式路径打开设置 tab(测试 tempdir 隔离;生产经 `open_settings_tab`)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn open_settings_tab_at(
        &mut self,
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_settings_tab_inner(Some(settings_path), None, window, cx);
    }

    /// 同 [`Self::open_settings_tab_at`],额外注入 hooks 假 home(Agent 组
    /// 联调用例:安装器只写 tempdir)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn open_settings_tab_at_with_hooks(
        &mut self,
        settings_path: PathBuf,
        hooks: (PathBuf, Option<String>, Option<PathBuf>),
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_settings_tab_inner(Some(settings_path), Some(hooks), window, cx);
    }

    fn open_settings_tab_inner(
        &mut self,
        settings_path: Option<PathBuf>,
        #[cfg_attr(not(all(test, feature = "gpui-test-support")), allow(unused_variables))]
        hooks: Option<(PathBuf, Option<String>, Option<PathBuf>)>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 设置页写盘路径即 keybind 热重绑的重读路径。
        self.settings_path_override = settings_path.clone();
        if let Some(index) = self
            .tabs
            .iter()
            .position(|entry| matches!(entry.tab, WorkspaceTab::Settings { .. }))
        {
            self.active = index;
            self.focus_active(window, cx);
            self.sync_window_title(window, cx);
            cx.notify();
            return;
        }
        #[cfg(all(test, feature = "gpui-test-support"))]
        let view = match (settings_path, hooks) {
            (Some(path), Some((config_dir, helper, executable))) => cx.new(|cx| {
                SettingsPane::new_at_with_claude_hooks(
                    path, config_dir, helper, executable, window, cx,
                )
            }),
            (Some(path), None) => cx.new(|cx| SettingsPane::new_at(path, window, cx)),
            (None, _) => cx.new(|cx| SettingsPane::new(window, cx)),
        };
        #[cfg(not(all(test, feature = "gpui-test-support")))]
        let view = match settings_path {
            Some(path) => cx.new(|cx| SettingsPane::new_at(path, window, cx)),
            None => cx.new(|cx| SettingsPane::new(window, cx)),
        };
        let subscription = cx.subscribe_in(&view, window, Self::on_settings_event);
        let tab_id = TabId(self.next_tab);
        self.next_tab = self.next_tab.saturating_add(1);
        self.tabs.push(TabEntry {
            id: tab_id,
            tab: WorkspaceTab::Settings {
                view,
                _subscription: subscription,
            },
            meta: TabMeta::default(),
        });
        self.active = self.tabs.len() - 1;
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 设置页事件:写盘成功 = 刷新窗口(终端消费字段由全局 Settings 承载);
    /// 关闭 = 摘除设置 tab。
    fn on_settings_event(
        &mut self,
        view: &Entity<SettingsPane>,
        event: &SettingsPaneEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            SettingsPaneEvent::Changed => {
                // keybind 段可能随本次写盘变化;重读并重建 keymap。
                self.reload_user_keybinds(cx);
                // 全局 `Settings` 已由设置页写入;这里把已消费字段扇出到
                // 每个终端视图(字体/字号/连字/选中复制/调色板热应用)。
                let settings = cx.global::<crate::app_shell::config::Settings>().clone();
                self.apply_settings_to_terminals(&settings, cx);
                // 视效与 token 同一帧热应用(模糊/透明度/壁纸)。
                crate::app_shell::wallpaper::refresh(cx);
                self.sync_window_title(window, cx);
                cx.refresh_windows();
                cx.notify();
            }
            // 工作区导出入口。前台只拍内存快照;写盘走后台执行器,
            // 成功/失败都只发通知,不改变设置 tab 的既有 Changed/Close 行为。
            SettingsPaneEvent::ExportWorkspace { path } => {
                let language = cx.global::<Settings>().language;
                let path = path.clone();
                let handle = window.window_handle();
                let task = self.export_workspace_task(&path, window, cx);
                cx.spawn(async move |_this, cx| {
                    let result = task.await;
                    let _ = handle.update(cx, |_, window, cx| match result {
                        Ok(()) => {
                            let text = language.format(
                                Message::SettingsAdvancedExportWorkspaceDone,
                                &[("path", &path.display().to_string())],
                            );
                            window.push_notification(
                                gpui_component::notification::Notification::success(text),
                                cx,
                            );
                        }
                        Err(error) => {
                            window.push_notification(
                                gpui_component::notification::Notification::error(
                                    language.text(Message::SettingsAdvancedExportWorkspaceFailed),
                                )
                                .message(error.to_string()),
                                cx,
                            );
                        }
                    });
                })
                .detach();
                cx.notify();
            }
            SettingsPaneEvent::Close => self.close_settings_tab(view, window, cx),
        }
    }

    /// 设置变更后重读 `keybind` 段并热重建 keymap。
    ///
    /// 与当前已应用表比较,只有准入结果真正变化才重建;重读路径与设置页
    /// 写盘路径同源(`settings_path_override`,生产回退默认数据目录)。
    fn reload_user_keybinds(&mut self, cx: &mut Context<'_, Self>) {
        let path = self
            .settings_path_override
            .clone()
            .unwrap_or_else(slterm_settings::settings_path);
        let pairs = keyboard_bindings::load_keybind_pairs_at(&path);
        let admitted = keyboard_bindings::admit_keybind_pairs(pairs).accepted;
        if keyboard_bindings::applied_user_keybinds(cx) != admitted {
            keyboard_bindings::rebuild_keymap(cx, &admitted);
        }
    }

    fn apply_settings_to_terminals(
        &self,
        settings: &crate::app_shell::config::Settings,
        cx: &mut App,
    ) {
        let views: Vec<Entity<TerminalView>> = self
            .tabs
            .iter()
            .filter_map(|entry| match &entry.tab {
                WorkspaceTab::Terminal { panes, .. } => Some(panes),
                WorkspaceTab::Settings { .. }
                | WorkspaceTab::Document { .. }
                | WorkspaceTab::Code { .. }
                | WorkspaceTab::Image { .. } => None,
            })
            .flat_map(|panes| panes.iter().map(|pane| pane.view.clone()))
            .collect();
        for view in views {
            view.update(cx, |view, cx| view.apply_settings(settings, cx));
        }
    }

    fn close_settings_tab(
        &mut self,
        view: &Entity<SettingsPane>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(index) = self.tabs.iter().position(
            |entry| matches!(&entry.tab, WorkspaceTab::Settings { view: open, .. } if open == view),
        ) else {
            return;
        };
        self.remove_tab_entry(index);
        if self.tabs.is_empty() {
            // 设置 tab 是最后一个 tab:关设置 = 关窗(单窗定位)。
            self.save_on_tabs_closed(window, cx);
            window.remove_window();
            return;
        }
        if index <= self.active {
            self.active = self.active.saturating_sub(1).min(self.tabs.len() - 1);
        }
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
    }

    /// 关闭活跃 tab(统一入口 [`Self::close_tab_at`])。
    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.close_tab_at(self.active, window, cx);
    }

    /// 统一「摘除 tab」点:条目离开 tabs 的同时清掉它的节点矩形缓存。
    /// TabId 终生不复用,残留键不会误配到别的 tab;但不清会随关闭次数
    /// 无界增长。停靠摘源 tab 同走此点。
    fn remove_tab_entry(&mut self, index: usize) -> Option<TabEntry> {
        if index >= self.tabs.len() {
            return None;
        }
        let entry = self.tabs.remove(index);
        self.split_bounds
            .borrow_mut()
            .retain(|(tab, _), _| *tab != entry.id);
        self.file_observations.remove(&entry.id);
        self.file_tab_events.remove(&entry.id);
        Some(entry)
    }

    /// 创建一个 pane 实体(分配 id、spawn 会话、挂宿主订阅、写注册表);
    /// 调用方决定它进新 tab 还是接进某棵分屏树。
    fn new_pane(
        &mut self,
        grid: (u16, u16),
        launch: TerminalLaunch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> TerminalPane {
        let pane_id = PaneId(self.next_pane);
        self.next_pane = self.next_pane.saturating_add(1);
        let launch_identity = launch_session_from_terminal_launch(&launch);
        let completion_session = Rc::new(crate::completion::Session::default());
        let session_for_view = Rc::clone(&completion_session);
        let view = cx.new(|cx| {
            let mut view = TerminalView::new(pane_id, grid, launch, window, cx);
            view.attach_completion_session(session_for_view);
            view
        });
        let subscription = cx.subscribe_in(&view, window, Self::on_terminal_event);
        // agent 宿主只在 pane 创建/关闭两处增删。
        self.agent_hub.register(pane_id);
        TerminalPane {
            id: pane_id,
            custom_name: None,
            launch: launch_identity,
            view,
            completion_session,
            _subscription: subscription,
        }
    }

    /// 分屏新 pane 的启动身份:cwd 继承聚焦 pane;默认 shell 走 core 探测链。
    ///
    /// 测试隔离:UI 测试用必败命令顶替真 shell(真 PTY 线程会跨测试存活,
    /// wake 已换线程的 executor 任务直接 fail-fast——测试纪律)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    fn split_launch(cwd: Option<std::path::PathBuf>) -> TerminalLaunch {
        TerminalLaunch::Local {
            cwd,
            shell: Some(slterm_terminal::tty::Shell::new(
                "slterm-test-missing-shell-executable".into(),
                Vec::new(),
            )),
            shell_name: None,
            explicit_command: false,
        }
    }

    #[cfg(not(all(test, feature = "gpui-test-support")))]
    fn split_launch(cwd: Option<std::path::PathBuf>) -> TerminalLaunch {
        TerminalLaunch::Local {
            cwd,
            shell: None,
            shell_name: None,
            explicit_command: false,
        }
    }

    /// 在聚焦 pane 上开分屏:新 pane 继承聚焦 pane 的 cwd,spawn 网格按
    /// 切割方向对半预估——首帧 prepaint 回写真实矩形后自动收敛。
    fn split_focused(
        &mut self,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<PaneId> {
        let active = self.active;
        let entry = self.tabs.get(active)?;
        let WorkspaceTab::Terminal { panes, focused, .. } = &entry.tab else {
            return None;
        };
        let focused = *focused;
        let anchor = panes.iter().find(|pane| pane.id == focused)?;
        let (cols, rows, cwd) = {
            let view = anchor.view.read(cx);
            (
                view.grid_cols() as u16,
                view.grid_rows() as u16,
                view.cwd.clone(),
            )
        };
        let grid = match direction {
            SplitDirection::LeftRight => ((cols / 2).max(2), rows.max(2)),
            SplitDirection::TopBottom => (cols.max(2), (rows / 2).max(2)),
        };
        let cwd = (!cwd.is_empty()).then(|| std::path::PathBuf::from(cwd));
        let launch = Self::split_launch(cwd);
        let pane = self.new_pane(grid, launch, window, cx);
        let new_id = pane.id;

        let attached = match self.tabs.get_mut(active) {
            Some(entry) => {
                if let WorkspaceTab::Terminal {
                    panes,
                    tree,
                    focused,
                    zoomed,
                } = &mut entry.tab
                {
                    // 建树挂不上 = 失败路径:立即回收 pane,不留孤儿 PTY。
                    if tree.split_leaf(*focused, new_id, direction, 0.5) {
                        panes.push(pane);
                        *focused = new_id;
                        *zoomed = false;
                        true
                    } else {
                        pane.view.read(cx).shutdown();
                        false
                    }
                } else {
                    pane.view.read(cx).shutdown();
                    false
                }
            }
            None => {
                pane.view.read(cx).shutdown();
                false
            }
        };
        if !attached {
            self.forget_pane(new_id, cx);
            return None;
        }
        self.mark_structural_resize(active, cx);
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
        Some(new_id)
    }

    /// 结构性布局变化:让 tab 内每个 pane 的下一次网格观测直接下发到 Term
    /// 与 ConPTY,不进尾沿去抖。
    ///
    /// 分屏创建/关闭、zoom 切换都属此列——几何只改一次,去抖没有可合并的
    /// 后续帧,而多等的每一毫秒都是 Term(已按新网格渲染)与 ConPTY(仍是
    /// 旧几何)不一致的窗口期:这段时间到达的字节按旧宽度进网格,等提交时
    /// 本地 reflow 与 conhost rewrap 各折一套行数,提示符与回显就此错开。
    fn mark_structural_resize(&mut self, tab_ix: usize, cx: &mut App) {
        let Some(entry) = self.tabs.get(tab_ix) else {
            return;
        };
        let WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
            return;
        };
        let views: Vec<_> = panes.iter().map(|pane| pane.view.clone()).collect();
        for view in views {
            view.update(cx, |view, _| view.mark_structural_resize());
        }
    }

    /// 视图事件白集:标题变化重绘;会话结束走关 pane 状态机;
    /// 用户输入的广播扇出归壳层（当前未接）。
    fn on_terminal_event(
        &mut self,
        view: &Entity<TerminalView>,
        event: &TerminalViewEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            TerminalViewEvent::TitleChanged => {
                // 标题栏与任务栏标题同源:活跃 tab 的展示名即时跟随 OSC 标题。
                self.sync_window_title(window, cx);
                cx.notify();
            }
            TerminalViewEvent::Exited => {
                if let Some(pane_id) = self.locate_pane(view) {
                    self.close_pane(pane_id, window, cx);
                }
            }
            TerminalViewEvent::UserInput(_) => {}
            // 历史记账在视图内完成(record_command + Session::invalidate);
            // 宿主只需在命令边界推进 agent 生命周期。
            TerminalViewEvent::CommandSubmitted(_) => {}
            TerminalViewEvent::CommandStarted => {
                if let Some(pane_id) = self.locate_pane(view) {
                    self.agent_hub.command_started(pane_id);
                }
            }
            TerminalViewEvent::CommandFinished => {
                if let Some(pane_id) = self.locate_pane(view) {
                    self.agent_hub.command_finished(pane_id);
                    // 命令回到提示符,进度主人已经收尾;程序忘发
                    // `9;4;0` 也不能把进度条留在任务栏上。
                    self.clear_taskbar_progress(pane_id, cx);
                }
            }
            // 任务栏是窗口级的,只反映活跃 tab 的聚焦 pane。
            // 后台 pane 的构建进度投到同一条按钮上只会互相覆盖,读数还不如
            // 没有;事件本身照常触发重绘。
            TerminalViewEvent::ProgressChanged(progress) => {
                if let Some(pane_id) = self.locate_pane(view)
                    && self.is_active_focused_pane(pane_id)
                {
                    self.show_taskbar_progress(pane_id, *progress, cx);
                }
                cx.notify();
            }
        }
    }

    /// 活跃 tab 的聚焦 pane 判定(任务栏进度的路由门)。
    fn is_active_focused_pane(&self, pane_id: PaneId) -> bool {
        self.tabs.get(self.active).is_some_and(|entry| {
            matches!(&entry.tab, WorkspaceTab::Terminal { focused, .. } if *focused == pane_id)
        })
    }

    /// 投递一次任务栏进度。HWND 源 = 窗口 facade 的查询点
    /// (`windowing::focus::window_hwnd`)。
    fn show_taskbar_progress(
        &mut self,
        pane_id: PaneId,
        progress: crate::taskbar::TaskProgress,
        cx: &App,
    ) {
        if let Some(progress) = self.taskbar.observe(pane_id, progress) {
            crate::taskbar::apply(
                crate::windowing::focus::window_hwnd(cx).unwrap_or(0),
                progress,
            );
        }
    }

    /// 任务栏进度的清除边:只有当前显示的来源 pane 自己收尾才恢复常态,
    /// 别的 pane 的收尾不打断正在显示的进度。
    fn clear_taskbar_progress(&mut self, pane_id: PaneId, cx: &App) {
        if let Some(progress) = self.taskbar.clear_for(pane_id) {
            crate::taskbar::apply(
                crate::windowing::focus::window_hwnd(cx).unwrap_or(0),
                progress,
            );
        }
    }

    fn locate_pane(&self, view: &Entity<TerminalView>) -> Option<PaneId> {
        self.tabs.iter().find_map(|entry| match &entry.tab {
            WorkspaceTab::Terminal { panes, .. } => panes
                .iter()
                .find(|pane| pane.view == *view)
                .map(|pane| pane.id),
            WorkspaceTab::Settings { .. }
            | WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => None,
        })
    }

    /// 忙碌 pane 的关闭确认(运行中/等输入/attention 视为破坏性操作)。
    fn confirm_close_busy_pane(
        &mut self,
        pane_id: PaneId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let language = cx.global::<Settings>().language;
        let detail = language.text(Message::TerminalCloseBusyDetail);
        let answer = window.prompt(
            PromptLevel::Warning,
            language.text(Message::TerminalCloseBusyTitle),
            Some(detail),
            &[
                language.text(Message::TerminalCloseBusyConfirm),
                language.text(Message::EditorCancel),
            ],
            cx,
        );
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            if !matches!(answer.await, Ok(0)) {
                return;
            }
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |workspace, cx| {
                    workspace.pane_close_confirmed = Some(pane_id);
                    workspace.close_pane(pane_id, window, cx);
                });
            });
        })
        .detach();
    }

    /// 关 pane 状态机:摘除 → 立即显式 shutdown → 焦点交幸存
    /// 首叶;唯一叶子关闭 = 关 tab = 无 tab 时关窗(语义保留)。
    fn close_pane(&mut self, pane_id: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        // 忙碌 pane 先确认;确认后的同一次调用经标志放行。
        if self.pane_close_confirmed.take() != Some(pane_id) && self.pane_is_busy(pane_id) {
            self.confirm_close_busy_pane(pane_id, window, cx);
            return;
        }
        let Some(tab_index) = self.tabs.iter().position(|entry| match &entry.tab {
            WorkspaceTab::Terminal { tree, .. } => tree.contains(pane_id),
            WorkspaceTab::Settings { .. }
            | WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => false,
        }) else {
            return;
        };
        let outcome = match &mut self.tabs[tab_index].tab {
            WorkspaceTab::Terminal { tree, .. } => tree.remove_leaf(pane_id),
            WorkspaceTab::Settings { .. }
            | WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => return,
        };
        match outcome {
            RemoveOutcome::NotFound => {}
            RemoveOutcome::WasRoot => {
                if let Some(pane) = self.tab_panes(tab_index).find(|pane| pane.id == pane_id) {
                    pane.view.read(cx).shutdown();
                }
                self.forget_pane(pane_id, cx);
                self.remove_tab_entry(tab_index);
                if self.tabs.is_empty() {
                    // 同 close_active_tab:最后 tab 关闭 = 空会话终局笔。
                    self.save_on_tabs_closed(window, cx);
                    window.remove_window();
                    return;
                }
                if tab_index < self.active {
                    self.active -= 1;
                } else {
                    self.active = self.active.min(self.tabs.len() - 1);
                }
                self.focus_active(window, cx);
                self.sync_window_title(window, cx);
                cx.notify();
            }
            RemoveOutcome::Collapsed(next_focus) => {
                {
                    let WorkspaceTab::Terminal {
                        panes,
                        focused,
                        zoomed,
                        ..
                    } = &mut self.tabs[tab_index].tab
                    else {
                        return;
                    };
                    if let Some(position) = panes.iter().position(|pane| pane.id == pane_id) {
                        let pane = panes.remove(position);
                        // 被摘 pane 立即显式 shutdown,不等 Drop 兜底。
                        pane.view.read(cx).shutdown();
                    }
                    if *focused == pane_id {
                        *focused = next_focus;
                    }
                    *zoomed = false;
                }
                self.forget_pane(pane_id, cx);
                self.mark_structural_resize(tab_index, cx);
                if tab_index == self.active {
                    self.focus_active(window, cx);
                    self.sync_window_title(window, cx);
                }
                cx.notify();
            }
        }
    }

    /// agent 宿主与视图缓存的统一摘除点。
    fn forget_pane(&mut self, pane_id: PaneId, cx: &App) {
        // pane 关闭是任务栏进度的清除边之一(关 pane/关 tab/
        // PTY 退出/恢复失败同路);只清自己名下正在显示的条。
        self.clear_taskbar_progress(pane_id, cx);
        self.agent_hub.remove(pane_id);
        self.pane_bounds.borrow_mut().remove(&pane_id);
    }

    fn tab_panes(&self, tab_index: usize) -> impl Iterator<Item = &TerminalPane> {
        self.tabs
            .get(tab_index)
            .map(|entry| match &entry.tab {
                WorkspaceTab::Terminal { panes, .. } => panes.as_slice(),
                WorkspaceTab::Settings { .. }
                | WorkspaceTab::Document { .. }
                | WorkspaceTab::Code { .. }
                | WorkspaceTab::Image { .. } => &[],
            })
            .unwrap_or(&[])
            .iter()
    }

    /// 聚焦另一个 pane(点击上报或方向导航落点)。点同一个 pane 不解除
    /// zoom(满卡里点击终端选字不应退出满卡);点别的 pane 先解除 zoom,
    /// 属结构性几何变化,走同步下发。
    fn focus_pane(
        &mut self,
        tab_ix: usize,
        pane_id: PaneId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let was_zoomed = {
            let Some(entry) = self.tabs.get_mut(tab_ix) else {
                return;
            };
            let WorkspaceTab::Terminal {
                panes,
                focused,
                zoomed,
                ..
            } = &mut entry.tab
            else {
                return;
            };
            if !panes.iter().any(|pane| pane.id == pane_id) || *focused == pane_id {
                return;
            }
            let was_zoomed = *zoomed;
            *focused = pane_id;
            *zoomed = false;
            was_zoomed
        };
        if tab_ix == self.active {
            if was_zoomed {
                self.mark_structural_resize(tab_ix, cx);
            }
            self.focus_active(window, cx);
            self.sync_window_title(window, cx);
        }
        cx.notify();
    }

    /// 方向导航:按上一帧 pane 矩形找目标方向最近邻(非对齐轴 4 倍漂移
    /// 惩罚归 `slterm_split::nav_target`)。缩放态先退出缩放再导航,pane
    /// 矩形读缩放前记录的上一帧。
    fn navigate_pane(&mut self, nav: SplitNav, window: &mut Window, cx: &mut Context<'_, Self>) {
        let active = self.active;
        let (focused_id, rects, was_zoomed) = {
            let Some(entry) = self.tabs.get(active) else {
                return;
            };
            let WorkspaceTab::Terminal {
                panes,
                focused,
                zoomed,
                ..
            } = &entry.tab
            else {
                return;
            };
            if panes.len() < 2 {
                return;
            }
            let bounds = self.pane_bounds.borrow();
            let rects: Vec<(PaneId, slterm_split::Rect)> = panes
                .iter()
                .filter_map(|pane| {
                    bounds
                        .get(&pane.id)
                        .map(|bounds| (pane.id, to_split_rect(bounds)))
                })
                .collect();
            (*focused, rects, *zoomed)
        };
        if was_zoomed {
            if let Some(WorkspaceTab::Terminal { zoomed, .. }) =
                self.tabs.get_mut(active).map(|entry| &mut entry.tab)
            {
                *zoomed = false;
            }
            self.mark_structural_resize(active, cx);
        }
        if let Some(target) = nav_target(&rects, focused_id, nav) {
            self.focus_pane(active, target, window, cx);
        } else {
            cx.notify();
        }
    }

    /// 聚焦 pane 满卡缩放开关(zoom 状态机;单 pane 无缩放语义)。
    /// 几何从半卡跳整卡属一次性结构变化,走同步下发。
    fn toggle_zoom(&mut self, cx: &mut Context<'_, Self>) {
        let active = self.active;
        let toggled = match self.tabs.get_mut(active).map(|entry| &mut entry.tab) {
            Some(WorkspaceTab::Terminal { panes, zoomed, .. }) if panes.len() > 1 => {
                *zoomed = !*zoomed;
                true
            }
            _ => false,
        };
        if toggled {
            self.mark_structural_resize(active, cx);
            cx.notify();
        }
    }

    /// 测试专用:活跃 tab 的 (panes id, 树叶, focused, zoomed) 快照。
    /// 「panes == 树叶」不变式的断言端。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_tab_snapshot(&self) -> Option<(Vec<PaneId>, Vec<PaneId>, PaneId, bool)> {
        let entry = self.tabs.get(self.active)?;
        let WorkspaceTab::Terminal {
            panes,
            tree,
            focused,
            zoomed,
        } = &entry.tab
        else {
            return None;
        };
        Some((
            panes.iter().map(|pane| pane.id).collect(),
            tree.leaves(),
            *focused,
            *zoomed,
        ))
    }

    /// 测试专用:任一 tab 的 (panes id, 树叶) 快照(停靠回退路径的断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn tab_snapshot(&self, index: usize) -> Option<(Vec<PaneId>, Vec<PaneId>)> {
        let entry = self.tabs.get(index)?;
        let WorkspaceTab::Terminal { panes, tree, .. } = &entry.tab else {
            return None;
        };
        Some((panes.iter().map(|pane| pane.id).collect(), tree.leaves()))
    }

    /// 测试专用:第 index 个 tab 的稳定身份。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn tab_id_at(&self, index: usize) -> Option<TabId> {
        self.tabs.get(index).map(|entry| entry.id)
    }

    /// 测试专用:节点矩形缓存的当前键集合(TabId 键修复的回归断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn split_bounds_keys(&self) -> Vec<(TabId, Vec<bool>)> {
        let mut keys: Vec<_> = self.split_bounds.borrow().keys().cloned().collect();
        keys.sort();
        keys
    }

    /// 测试专用:重排 tab(模拟拖拽换位,验证缓存键随稳定身份走)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn move_tab_for_test(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let entry = self.tabs.remove(from);
        self.tabs.insert(to, entry);
        self.active = if self.active == from {
            to
        } else {
            let mut ix = self.active;
            if ix > from {
                ix -= 1;
            }
            if ix >= to {
                ix += 1;
            }
            ix
        };
    }

    /// 测试专用:活跃 tab 根节点的提交比例(单叶 = None)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_split_ratio(&self) -> Option<f32> {
        let WorkspaceTab::Terminal { tree, .. } = &self.tabs.get(self.active)?.tab else {
            return None;
        };
        match tree {
            SplitTree::Split { ratio, .. } => Some(*ratio),
            SplitTree::Leaf(_) => None,
        }
    }

    /// 测试专用:pane 矩形上一帧记录(canvas 探针回写端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn pane_bounds_of(&self, pane_id: PaneId) -> Option<Bounds<Pixels>> {
        self.pane_bounds.borrow().get(&pane_id).copied()
    }

    /// 测试专用:活跃 tab 首个 pane 的视图实体。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn first_pane_view(&self) -> Option<Entity<TerminalView>> {
        self.tab_panes(self.active)
            .next()
            .map(|pane| pane.view.clone())
    }

    /// 测试专用:按 pane id 取视图实体(终端事件路由的喂入口)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn pane_view(&self, pane_id: PaneId) -> Option<Entity<TerminalView>> {
        self.tabs.iter().find_map(|entry| match &entry.tab {
            WorkspaceTab::Terminal { panes, .. } => panes
                .iter()
                .find(|pane| pane.id == pane_id)
                .map(|pane| pane.view.clone()),
            WorkspaceTab::Settings { .. }
            | WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => None,
        })
    }

    /// 测试专用:spawn 网格只读(启动零 resize 锚的断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn initial_grid(&self) -> (u16, u16) {
        self.initial_grid
    }

    /// 测试专用:侧栏骨架态只读(宽度/双槽/开合的断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn sidebar_state(&self) -> &SidebarState {
        &self.sidebar
    }

    /// 测试专用:当前设置 tab 的视图实体(设置页豁免层用例的驱动端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn settings_pane_for_test(&self) -> Option<Entity<SettingsPane>> {
        self.tabs.iter().find_map(|entry| match &entry.tab {
            WorkspaceTab::Settings { view, .. } => Some(view.clone()),
            WorkspaceTab::Terminal { .. }
            | WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => None,
        })
    }

    /// 测试专用:直接播种双槽 active view(会话恢复消费前的渲染锚)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_sidebar_views_for_test(
        &mut self,
        top: Option<sidebar::SideViewId>,
        bottom: Option<sidebar::SideViewId>,
    ) {
        self.sidebar = sidebar::reconcile_views(
            &SidebarState {
                active_view_top: top,
                active_view_bottom: bottom,
                ..self.sidebar
            },
            &sidebar::SideViewId::ALL,
        );
    }

    /// 测试专用:标题栏三钮的命令记录(虚拟窗口平台 API 不可直调)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn window_commands(&self) -> &[title_bar::TitleBarCommand] {
        &self.window_commands
    }

    /// 测试专用:托盘「退出」是否已请求(虚拟窗口不可直调 remove_window)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn tray_quit_requested(&self) -> bool {
        self.tray_quit_requested
    }

    /// 测试专用:更新弹窗/安装授权是否在途(单实例闸门断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn update_prompt_open_for_test(&self) -> bool {
        self.update_prompt_open
    }

    /// 测试专用:注入更新安装执行替身(避免真实 helper/安装目录)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_update_install_runner_for_test(
        &mut self,
        runner: control::UpdateInstallRunner,
    ) {
        self.update_install_runner = Some(runner);
    }

    /// 测试专用:安装成功后的退出请求是否已记录。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn update_install_exit_requested(&self) -> bool {
        self.update_install_exit_requested
    }

    /// 测试专用:活跃 tab 展示名。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_tab_title_for_test(&self, cx: &App) -> String {
        self.active_tab_title(cx)
    }

    /// 测试专用:写入活跃 tab 自定义名(标题栏展示名的第一优先源)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_active_tab_custom_name_for_test(&mut self, name: Option<String>) {
        if let Some(entry) = self.tabs.get_mut(self.active) {
            entry.meta.custom_name = name;
        }
    }

    /// 测试专用:活跃 tab 冻结的启动身份。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_tab_launch(&self) -> Option<LaunchSession> {
        self.tabs
            .get(self.active)
            .and_then(|entry| entry.meta.launch.clone())
    }

    /// 测试专用:tab 数量与活跃下标(新建/切换/关闭动作的断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn active_tab_index(&self) -> usize {
        self.active
    }

    /// 测试专用:活跃 tab 首个 pane id(hook 泵路由断言的锚点)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn first_pane_id(&self) -> Option<PaneId> {
        self.tab_panes(self.active).next().map(|pane| pane.id)
    }

    /// 测试专用:单 pane 的 agent 仲裁状态(事件泵端到端断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn agent_status(&self, pane_id: PaneId) -> Option<crate::ai_agents::AgentStatus> {
        self.agent_hub.host(pane_id).map(|host| host.status())
    }

    /// 键盘焦点交活跃 tab 的聚焦 pane。
    fn focus_active(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(entry) = self.tabs.get(self.active) else {
            return;
        };
        match &entry.tab {
            WorkspaceTab::Terminal { panes, focused, .. } => {
                if let Some(pane) = panes.iter().find(|pane| pane.id == *focused) {
                    let handle: FocusHandle = pane.view.read(cx).focus_handle.clone();
                    window.focus(&handle, cx);
                }
            }
            WorkspaceTab::Settings { view, .. } => {
                let handle = view.read(cx).focus_handle.clone();
                window.focus(&handle, cx);
            }
            WorkspaceTab::Document { .. }
            | WorkspaceTab::Code { .. }
            | WorkspaceTab::Image { .. } => {
                // 文件 tab 的焦点分派归 documents.rs(图片臂无可编辑面,
                // 返回 None 即不迁移焦点)。
                if let Some(handle) = self.file_tab_focus_handle(&entry.tab, cx) {
                    window.focus(&handle, cx);
                }
            }
        }
    }

    /// 焦点 pane:信封缺 `SLTERM_PANE_ID` 时的唯一回退目标。
    fn focused_pane(&self) -> Option<PaneId> {
        self.tabs
            .get(self.active)
            .and_then(|entry| match &entry.tab {
                WorkspaceTab::Terminal { focused, .. } => Some(*focused),
                WorkspaceTab::Settings { .. }
                | WorkspaceTab::Document { .. }
                | WorkspaceTab::Code { .. }
                | WorkspaceTab::Image { .. } => None,
            })
    }
}

impl Render for SltermWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // 每帧对账 watcher 兴趣(树根 + 打开的可编辑文件)。放在渲染根
        // 而非 explorer 槽内:explorer 关闭时 gutter 外部刷新仍须工作。
        self.sync_fs_watch(cx);
        // 只有活跃文件 tab 物化预览资产;切走即释放块槽/图片。
        self.sync_file_render_activity(window, cx);
        let content = match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Terminal { .. }) => Some(self.render_terminal_tab(self.active, cx)),
            Some(WorkspaceTab::Settings { view, .. }) => Some(view.clone().into_any_element()),
            // 文件 tab 的渲染分派(含各变体外层选择器)归 documents.rs。
            Some(
                tab @ (WorkspaceTab::Document { .. }
                | WorkspaceTab::Code { .. }
                | WorkspaceTab::Image { .. }),
            ) => self.render_file_tab(tab),
            None => None,
        };
        // 侧栏组件:ActivityBar + tab 列表/双槽内容区 + 宽度拖拽热区。
        // collapsed 才整列隐藏;双槽全空时 tab 列表仍常驻(切 tab 与拖拽
        // 停靠的入口不能随视图槽一起消失)。ActivityBar 始终常驻。
        let sidebar_content = self.render_sidebar_content(cx);
        let sidebar_visible = !self.sidebar.collapsed;
        let activity_bar = self.render_activity_bar(cx);
        let resize_handle = sidebar_visible.then(|| self.render_sidebar_resize_handle(cx));
        let sidebar_width = self.sidebar.width;
        let title_bar = self.render_title_bar(window, cx);
        // 分隔条拖拽 overlay 与全窗指针罩层(同 tab_drag 的捕获模式:罩层
        // 独占指针,移动喂预览、松开提交/关闭)。overlay 在内容之上、罩层
        // 之下;顺序即绘制序。
        let split_overlay = self.split_drag_visual(cx);
        let sidebar_resize_overlay = self.render_sidebar_resize_overlay(cx);
        let sidebar_split_overlay = self.render_sidebar_split_overlay(cx);
        let drag_direction = self.split_drag.as_ref().map(|drag| drag.direction);
        // tab 拖拽:预览矩形只提示落区,树与网格不动;激活后全窗罩层独占
        // 指针(与分隔条拖拽同套路),指针划出侧栏/划到终端上都不丢手势。
        let tab_drag_preview = self.tab_drag_preview(cx);
        let tab_drag_active = self.tab_drag.as_ref().is_some_and(|drag| drag.active);
        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            // 待命阶段(未过阈值)的指针跟踪;激活后由根部罩层接管。
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.tab_drag.as_ref().is_some_and(|drag| !drag.active) {
                    this.update_tab_drag(event, window, cx);
                }
            }))
            // 未过阈值的松手 = 普通点击(保留行 on_click 的激活语义);
            // 已激活的松手走兜底提交(罩层通常已先接到)。
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.release_tab_drag(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.split_focused(SplitDirection::LeftRight, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                this.split_focused(SplitDirection::TopBottom, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleZoom, _, cx| {
                this.toggle_zoom(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusPaneLeft, window, cx| {
                this.navigate_pane(SplitNav::Left, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusPaneRight, window, cx| {
                this.navigate_pane(SplitNav::Right, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusPaneUp, window, cx| {
                this.navigate_pane(SplitNav::Up, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusPaneDown, window, cx| {
                this.navigate_pane(SplitNav::Down, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseActivePane, window, cx| {
                if let Some(pane_id) = this.focused_pane() {
                    this.close_pane(pane_id, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewTerminalTab, window, cx| {
                this.new_terminal_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectNextTab, window, cx| {
                this.select_adjacent_tab(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousTab, window, cx| {
                this.select_adjacent_tab(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseActiveTab, window, cx| {
                this.close_active_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings_tab(window, cx);
            }))
            // Ctrl+S:活跃可编辑源码 tab 的保存入口(保存链)。
            .on_action(cx.listener(|this, _: &SaveFile, _, cx| {
                this.save_active_file(cx);
            }))
            // 整窗壁纸垫在 chrome 之下(与系统 Mica/Aero 同一
            // 层次语义);非整窗模式此层不绘制,由终端内容区的卡层承担。
            .child(crate::app_shell::wallpaper::window_layer(cx))
            // 自绘标题栏:活跃 tab 展示名 + 三钮 + 拖拽区。
            .child(title_bar)
            // 主行:ActivityBar(46px) + 侧栏内容区(可开合/拖宽) + 终端主区。
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .flex()
                    .flex_row()
                    .child(activity_bar)
                    .when(sidebar_visible, |row| {
                        row.child(
                            div()
                                .w(px(sidebar_width))
                                .h_full()
                                .flex_shrink_0()
                                .min_w_0()
                                .child(sidebar_content),
                        )
                        .child(resize_handle.unwrap_or_else(|| div().into_any_element()))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            // 窗口不透明度已在壳底色层(Root token)生效;终端
                            // 默认格保持透明,由该层透出,避免同一区域二次
                            // alpha 叠加。壁纸在底色之上、单元格之下;整窗
                            // 模式此层不绘制。
                            .child(crate::app_shell::wallpaper::card_layer(cx))
                            .children(content),
                    ),
            )
            .children(sidebar_resize_overlay)
            .children(sidebar_split_overlay)
            // 文件树右键菜单挂 workspace 根,不受抽屉裁剪影响。
            .children(self.render_file_tree_context_menu(cx))
            .children(split_overlay)
            .children(tab_drag_preview)
            .when(tab_drag_active, |root| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                            this.update_tab_drag(event, window, cx);
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.finish_tab_drag(window, cx);
                            }),
                        ),
                )
            })
            .when_some(drag_direction, |root, direction| {
                root.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .map(|mask| match direction {
                            SplitDirection::LeftRight => mask.cursor_col_resize(),
                            SplitDirection::TopBottom => mask.cursor_row_resize(),
                        })
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                            this.update_split_drag(event, window, cx);
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseUpEvent, window, cx| {
                                this.finish_split_drag(event.position, window, cx);
                            }),
                        ),
                )
            })
    }
}

/// 测试专用:注入用户 keybind 表到 GPUI keymap(启动路径为文件读取)。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) fn apply_user_keybinds_for_test(cx: &mut App, pairs: &[(String, String)]) {
    keyboard_bindings::bind_user_keybinds(cx, pairs);
}

/// 工作区级键绑定(键位收口)。
///
/// 静态表与 `keyboard_bindings::default_workspace_bindings` 同源;用户
/// `keybind` 段在其后追加(行序即优先级)。`CloseActiveTab` 暂无默认键位——
/// 与上游 ctrl-shift-w 的整 tab 关闭语义冲突,入口归菜单裁定。
/// `Ctrl+S` 只在 `FileEditor` 上下文注册(无 context 会吞终端焦点按键)。
/// 在 `gpui_component::init` 之后调用一次。
pub fn init(cx: &mut App) {
    // 编辑器动作(撤销统一/ToggleSource/渲染态键盘)绑定在 FileEditor
    // 上下文,只对文件视图生效。
    super::file_editor::init(cx);
    cx.bind_keys(keyboard_bindings::default_workspace_bindings());
    // 内置/默认绑定全部就位后、用户表安装前捕获快照——热重绑以它
    // 为回放基底(clear_key_bindings 会连组件内置绑定一起清掉)。
    keyboard_bindings::capture_keymap_base(cx);
    // 用户 keybind 段:启动读一次;保留键/坏行由准入门拒绝。
    keyboard_bindings::install_user_keybinds(cx);
}

#[cfg(test)]
mod workspace_tests {
    use super::render::to_split_rect;
    use super::*;

    /// 满卡路径与分屏递归共用同一坐标换算:GPUI Bounds → slterm_split Rect
    /// 保持左上原点与宽高逐字段一致。
    #[test]
    fn bounds_to_split_rect_keeps_origin_and_size() {
        let bounds = Bounds::new(
            gpui::point(px(10.0), px(20.0)),
            gpui::size(px(300.0), px(200.0)),
        );
        let rect = to_split_rect(&bounds);
        assert_eq!((rect.x, rect.y, rect.w, rect.h), (10.0, 20.0, 300.0, 200.0));
    }
}
