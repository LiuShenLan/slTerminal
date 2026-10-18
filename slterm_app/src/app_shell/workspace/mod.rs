//! 工作区:tabs/pane 生命周期的唯一属主(05 篇三件套锚点)。
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
//! 4. 布局持久化只经 session schema(M5.5 快照/恢复已接线,见
//!    `session_persistence`/`session_recovery`);本模块运行时树与持久化
//!    树的转换只走 `session_restore` 一对纯函数。
//!
//! ## 壳层职责(M5.3)
//!
//! 壳层只做四件事:按**提交比例**渲染递归、canvas 探针回写节点/pane 矩形、
//! 分隔条拖拽 overlay、松手提交。分屏语义(比例/树变换)全由 `slterm_split`
//! 裁定,壳层不重算。渲染只读提交比例,`preview_ratio`/`dragging` 只喂
//! overlay 与分隔条跟随——拖拽期间终端网格零重排(02 篇 TTY 尺寸链交界:
//! 结构性变化经 `TerminalView::mark_structural_resize` 同步下发,不进尾沿
//! 去抖)。

pub(crate) mod session_persistence;
pub(crate) mod session_recovery;
pub(crate) mod sidebar;
mod split_drag;
mod tab_drag;
pub(crate) mod title_bar;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _,
    Pixels, Render, SharedString, Styled as _, Subscription, Window, canvas, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use slterm_settings::Rgb;
use slterm_split::{
    DIVIDER_GAP, HIT_SLOP, PaneId, RemoveOutcome, SplitDirection, SplitNav, SplitTree, TabId,
    nav_target,
};

use super::SltermShellEvent;
use super::settings_pane::{SettingsPane, SettingsPaneEvent};
use super::windowing::{WorkspaceStartup, startup_geometry};
use crate::agent_host::AgentHub;
use crate::ai_hook::AiHookEvent;
use crate::brand;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};
use crate::session::LaunchSession;
use crate::terminal_registry::{self, PaneSessionMeta};
use session_persistence::{SaveReason, SessionPersistence};
use session_recovery::StartupPlan;
use sidebar::{SidebarResizeDrag, SidebarState};
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
/// (03 篇 hook 的 `SLTERM_PANE_ID` 环境值与此同源)。
pub struct TerminalPane {
    pub id: PaneId,
    /// 用户重命名(持久化归 05 session 叶;M5.3 无重命名 UI,恒 None)。
    pub custom_name: Option<String>,
    /// 本 pane 创建时冻结的启动身份;快照逐叶落盘(M5.5)。
    pub launch: Option<LaunchSession>,
    pub view: Entity<TerminalView>,
    _subscription: Subscription,
}

/// 唯一合法 tab 类型集(面板封闭的 Rust 落点)。本阶段 Terminal +
/// Settings 两个变体;Document/Code 归 07 篇按片落地。
///
/// Settings 是**单例 tab**:再次打开只激活既有实例,不新建;不进会话
/// 快照(05 篇合同:快照只收 Terminal tab)。
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
}

/// tab 级展示态元数据(05 篇锚点;持久化四字段形态)。
#[derive(Default)]
pub struct TabMeta {
    /// 用户命名;标题栏展示名第一优先源(M5.3b)。
    pub custom_name: Option<String>,
    /// 用户色标;侧栏色标消费归后续步骤。
    pub color: Option<Rgb>,
    /// 建 tab 时冻结的启动身份;快照与恢复(M5.5)消费。
    pub launch: Option<LaunchSession>,
}

/// tab + 展示态元数据同槽存放(单 Vec 消下同标耦合)。
pub struct TabEntry {
    /// 稳定身份:跨排序/活跃切换不变;消费面 = 侧栏/dock/节点矩形缓存键。
    pub id: TabId,
    pub tab: WorkspaceTab,
    pub meta: TabMeta,
}

/// 壳内状态权威:直持 pane 实体与展示态,无投影层(13 篇改造节 2)。
pub struct SltermWorkspace {
    tabs: Vec<TabEntry>,
    /// 位置索引(活跃定位);TabId 为稳定身份。
    active: usize,
    /// 单调分配器(PaneId/TabId 唯一性权威)。多窗扩展时按窗口分段(04 篇)。
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
    /// 侧栏骨架态(05 篇 D05-2/D05-3):开合/宽度/双槽 active view。
    sidebar: SidebarState,
    /// 进行中的侧栏宽度拖拽;`None` = 无手势。
    sidebar_resize: Option<SidebarResizeDrag>,
    /// 虚拟窗口用例的三钮命令记录(生产不编译;平台窗口 API 在测试
    /// 平台是 unimplemented,行为断言只能落在命令分派端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    window_commands: Vec<title_bar::TitleBarCommand>,
    /// 每 pane 的 agent 宿主(AgentActivity/AnswerInbox 归属表)。
    agent_hub: AgentHub,
    /// 单窗口会话写盘状态机(latest/saved 双快照、无变化跳过、提权隔离)。
    session_persistence: SessionPersistence,
}

/// 键:pane id(方向导航要拿所有叶子的屏幕矩形算最近邻)。
type PaneBoundsStore = Rc<RefCell<HashMap<PaneId, Bounds<Pixels>>>>;
/// 键:(稳定 TabId, 分屏节点路径)。路径语义同 `slterm_split::Divider`。
///
/// 用 TabId 而非下标:关 tab 会让后续 tab 的下标整体前移,旧下标键会被
/// 幸存 tab 误读(拖分隔条读到别的 tab 的上一帧矩形);TabId 终生不复用,
/// 重排/关闭都不改键。关闭 tab 时仍显式清键,避免缓存无界增长。
type SplitBoundsStore = Rc<RefCell<HashMap<(TabId, Vec<bool>), Bounds<Pixels>>>>;

/// 分屏递归的只读渲染上下文:tab 身份(下标供调试选择器、TabId 供矩形
/// 缓存键)+ pane 属主与聚焦叶。递归调用只传本结构,不逐参展开。
struct SplitRenderContext<'a> {
    tab_ix: usize,
    tab_id: TabId,
    panes: &'a [TerminalPane],
    focused: PaneId,
}

/// GPUI `Bounds` → `slterm_split::Rect`(同为窗口逻辑像素坐标系)。
fn to_split_rect(bounds: &Bounds<Pixels>) -> slterm_split::Rect {
    slterm_split::Rect::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

/// 启动身份冻结:实际 shell 命令 + 参数 + 出生目录进 `LaunchSession::Profile`
/// (Profile 内嵌完整命令,导出工作区时可移植,05 篇 schema 契约)。
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
        // 默认宽度触发一次 ConPTY resize(D05-3/D05-1 回放顺序)。
        let sidebar = sidebar::reconcile_views(&plan.sidebar, &sidebar::SideViewId::ALL);
        // 开窗 resize 异步落地前,先按目标几何谓出 spawn 网格(启动零 resize)。
        let initial_grid = startup_geometry::prepare_initial_grid(
            window,
            cx,
            sidebar::chrome_width(&sidebar),
            true,
        );
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
            #[cfg(all(test, feature = "gpui-test-support"))]
            window_commands: Vec::new(),
            agent_hub: AgentHub::new(),
            session_persistence: Self::new_session_persistence(),
        };
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
                    // (SEC-01 豁免论证见 `Options::explicit_command`)。
                    let name = command.program().to_owned();
                    let shell = slterm_terminal::tty::Shell::new(
                        command.program().to_owned(),
                        command.args().to_vec(),
                    );
                    workspace.add_terminal_tab(cwd, Some((shell, name)), true, window, cx);
                }
            }
        }
        // 测试构建不启用会话注册表写入(真实数据目录隔离);生产启用。
        if !cfg!(all(test, feature = "gpui-test-support")) {
            workspace.agent_hub.enable_session_registry();
        }
        workspace.start_shell_event_pump(shell_rx, cx);
        workspace.start_ai_hook_pump(ai_events, cx);
        workspace.start_agent_screen_watchdog(cx);
        workspace.start_autosave_pump(cx);
        workspace.install_window_close_save(window, cx);
        workspace.sync_window_title(window, cx);
        workspace
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
    /// 启动身份在**建 tab 这一刻**冻结进 `TabMeta.launch`(M5.3a 问题 1):
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
            // M3 现状(None → 终端标题 "shell"),不让启动身份冻结改动 UI。
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
    /// 必败命令(真 PTY 线程跨测试存活,11 篇测试纪律)。
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
                WorkspaceTab::Settings { .. } => None,
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
        self.open_settings_tab_inner(None, window, cx);
    }

    /// 显式路径打开设置 tab(测试 tempdir 隔离;生产经 `open_settings_tab`)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn open_settings_tab_at(
        &mut self,
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_settings_tab_inner(Some(settings_path), window, cx);
    }

    fn open_settings_tab_inner(
        &mut self,
        settings_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
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
                // 全局 `Settings` 已由设置页写入;这里把已消费字段扇出到
                // 每个终端视图(字体/字号/连字/选中复制/调色板热应用)。
                let settings = cx.global::<crate::app_shell::config::Settings>().clone();
                self.apply_settings_to_terminals(&settings, cx);
                self.sync_window_title(window, cx);
                cx.refresh_windows();
                cx.notify();
            }
            SettingsPaneEvent::Close => self.close_settings_tab(view, window, cx),
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
                WorkspaceTab::Settings { .. } => None,
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

    /// 关闭活跃 tab:逐 pane 显式 shutdown,不留孤儿 PTY;最后一个 tab
    /// 关闭 = 关窗(单窗产品定位)。设置 tab 无 pane,直接摘除。
    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(entry) = self.tabs.get(self.active) else {
            return;
        };
        if let WorkspaceTab::Terminal { panes, .. } = &entry.tab {
            let views: Vec<_> = panes.iter().map(|pane| pane.view.clone()).collect();
            let ids: Vec<_> = panes.iter().map(|pane| pane.id).collect();
            for view in &views {
                view.read(cx).shutdown();
            }
            for id in ids {
                self.forget_pane(id);
            }
        }
        self.remove_tab_entry(self.active);
        if self.tabs.is_empty() {
            // 空会话干净落盘:下次启动不恢复,也不把这次关标签误判成崩溃。
            self.save_on_tabs_closed(window, cx);
            window.remove_window();
            return;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        cx.notify();
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
        Some(entry)
    }

    /// 1 Hz 自动保存泵(05 篇数据流):节奏与 agent 看门狗同源,前台线程
    /// 拍快照、状态机判无变化/写盘。失败只 warn,终端不受影响。
    fn start_autosave_pump(&mut self, cx: &mut Context<'_, Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(1)).await;
                let Ok(()) = this.update(cx, |workspace, cx| workspace.autosave_tick(cx)) else {
                    return;
                };
            }
        })
        .detach();
    }

    /// 一拍自动保存:快照 → 状态机 → 失败 warn。首次成功快照的
    /// `boot_attempts = 0` 由 `snapshot_session` 的规范形态保证。
    fn autosave_tick(&mut self, cx: &mut Context<'_, Self>) {
        let window_state = Self::current_window_state(cx);
        let session = self.snapshot_session(window_state, cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::Checkpoint)
        {
            log::warn!("session checkpoint: {error}");
        }
    }

    /// 单窗:取第一个窗口的当前尺寸/最大化态(无窗口 = 不写 window 段)。
    fn current_window_state(cx: &mut App) -> Option<crate::session::WindowState> {
        let handle = cx.windows().first().copied()?;
        handle
            .update(cx, |_, window, _| {
                session_recovery::capture_window_state(window)
            })
            .ok()
    }

    /// 关窗终局笔:1 Hz 快照恒 false,这一笔才 `clean_exit = true`。
    fn save_on_window_close(&mut self, window: &Window, cx: &App) {
        let session =
            self.snapshot_session(Some(session_recovery::capture_window_state(window)), cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::WindowClose)
        {
            log::warn!("could not persist final terminal session: {error}");
        }
    }

    /// 最后 tab 关闭:空快照干净落盘(空会话不恢复)。
    fn save_on_tabs_closed(&mut self, window: &Window, cx: &App) {
        let session =
            self.snapshot_session(Some(session_recovery::capture_window_state(window)), cx);
        if let Err(error) = self
            .session_persistence
            .save(Some(session), SaveReason::TabsClosed)
        {
            log::warn!("could not persist empty workspace: {error}");
        }
    }

    /// 系统关窗(Alt+F4/OS 关闭请求)先落终局快照;关窗即退出。
    fn install_window_close_save(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let handle = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = handle.update(cx, |workspace, cx| {
                workspace.save_on_window_close(window, cx);
            });
            true
        });
    }

    /// 窗口标题 = 活跃 tab 展示名(任务栏可见);无 tab 时回退品牌名。
    fn sync_window_title(&self, window: &mut Window, cx: &App) {
        let title = self.active_tab_title(cx);
        window.set_window_title(&format!("{title} — {}", brand::NAME));
    }

    /// 活跃 tab 展示名:自定义名优先,否则取聚焦 pane 的既有终端标题。
    /// 07 篇 TabPresentation 落地后,这一处取值面替换为其派生结果。
    pub(super) fn active_tab_title(&self, cx: &App) -> String {
        self.tabs
            .get(self.active)
            .map(|_| self.tab_title(self.active, cx))
            .unwrap_or_else(|| brand::NAME.to_owned())
    }

    /// 单个 tab 的展示名(侧栏 tab 行与标题栏同源)。
    fn tab_title(&self, tab_ix: usize, cx: &App) -> String {
        let Some(entry) = self.tabs.get(tab_ix) else {
            return brand::NAME.to_owned();
        };
        if let Some(custom) = &entry.meta.custom_name {
            return custom.clone();
        }
        match &entry.tab {
            WorkspaceTab::Terminal { panes, focused, .. } => panes
                .iter()
                .find(|pane| pane.id == *focused)
                .map(|pane| pane.view.read(cx).title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "shell".to_owned()),
            WorkspaceTab::Settings { view, .. } => view.read(cx).title(cx).to_string(),
        }
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
        let view = cx.new(|cx| TerminalView::new(pane_id, grid, launch, window, cx));
        let subscription = cx.subscribe_in(&view, window, Self::on_terminal_event);
        // 注册表写入点 1/2(spawn 侧;另一侧 = close_pane)。
        terminal_registry::register(pane_id, PaneSessionMeta::default());
        // agent 宿主同纪律:只在 pane 创建/关闭两处增删。
        self.agent_hub.register(pane_id);
        TerminalPane {
            id: pane_id,
            custom_name: None,
            launch: launch_identity,
            view,
            _subscription: subscription,
        }
    }

    /// 分屏新 pane 的启动身份:cwd 继承聚焦 pane;默认 shell 走 core 探测链。
    ///
    /// 测试隔离:UI 测试用必败命令顶替真 shell(真 PTY 线程会跨测试存活,
    /// wake 已换线程的 executor 任务直接 fail-fast——11 篇测试纪律)。
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
            self.forget_pane(new_id);
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
    /// 用户输入的广播扇出归 M5(本次不接)。
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
        }
    }

    fn locate_pane(&self, view: &Entity<TerminalView>) -> Option<PaneId> {
        self.tabs.iter().find_map(|entry| match &entry.tab {
            WorkspaceTab::Terminal { panes, .. } => panes
                .iter()
                .find(|pane| pane.view == *view)
                .map(|pane| pane.id),
            WorkspaceTab::Settings { .. } => None,
        })
    }

    /// 关 pane 状态机(05 篇锚定):摘除 → 立即显式 shutdown → 焦点交幸存
    /// 首叶;唯一叶子关闭 = 关 tab = 无 tab 时关窗(M3 语义保留)。
    fn close_pane(&mut self, pane_id: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(tab_index) = self.tabs.iter().position(|entry| match &entry.tab {
            WorkspaceTab::Terminal { tree, .. } => tree.contains(pane_id),
            WorkspaceTab::Settings { .. } => false,
        }) else {
            return;
        };
        let outcome = match &mut self.tabs[tab_index].tab {
            WorkspaceTab::Terminal { tree, .. } => tree.remove_leaf(pane_id),
            WorkspaceTab::Settings { .. } => return,
        };
        match outcome {
            RemoveOutcome::NotFound => {}
            RemoveOutcome::WasRoot => {
                if let Some(pane) = self.tab_panes(tab_index).find(|pane| pane.id == pane_id) {
                    pane.view.read(cx).shutdown();
                }
                self.forget_pane(pane_id);
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
                self.forget_pane(pane_id);
                self.mark_structural_resize(tab_index, cx);
                if tab_index == self.active {
                    self.focus_active(window, cx);
                    self.sync_window_title(window, cx);
                }
                cx.notify();
            }
        }
    }

    /// 索引与宿主表的统一摘除点(注册表写入点 2/2 的对偶)。
    fn forget_pane(&mut self, pane_id: PaneId) {
        terminal_registry::remove(pane_id);
        self.agent_hub.remove(pane_id);
        self.pane_bounds.borrow_mut().remove(&pane_id);
    }

    fn tab_panes(&self, tab_index: usize) -> impl Iterator<Item = &TerminalPane> {
        self.tabs
            .get(tab_index)
            .map(|entry| match &entry.tab {
                WorkspaceTab::Terminal { panes, .. } => panes.as_slice(),
                WorkspaceTab::Settings { .. } => &[],
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

    /// 聚焦 pane 满卡缩放开关(05 篇 zoom 状态机;单 pane 无缩放语义)。
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

    /// 测试专用:当前设置 tab 的视图实体(设置页 L4 用例的驱动端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn settings_pane_for_test(&self) -> Option<Entity<SettingsPane>> {
        self.tabs.iter().find_map(|entry| match &entry.tab {
            WorkspaceTab::Settings { view, .. } => Some(view.clone()),
            WorkspaceTab::Terminal { .. } => None,
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
        }
    }

    /// 焦点 pane:信封缺 `SLTERM_PANE_ID` 时的唯一回退目标。
    fn focused_pane(&self) -> Option<PaneId> {
        self.tabs
            .get(self.active)
            .and_then(|entry| match &entry.tab {
                WorkspaceTab::Terminal { focused, .. } => Some(*focused),
                WorkspaceTab::Settings { .. } => None,
            })
    }

    /// AI hook 事件泵(M4.5,照上游 agents.rs 形态):管道服务线程写进程内
    /// mpsc,后台定时器每 75ms 唤醒一次,前台线程排空并与 UI 状态合并。
    /// 单批上限 64;满批说明下游有积压,退 1ms 让出渲染。
    ///
    /// 前台只做状态更新:解析与阻塞读都在管道线程,本任务只消费已解析事件。
    fn start_ai_hook_pump(
        &mut self,
        receiver: Option<mpsc::Receiver<AiHookEvent>>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(receiver) = receiver else {
            return;
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let mut backlog = false;
            loop {
                executor
                    .timer(Duration::from_millis(if backlog { 1 } else { 75 }))
                    .await;
                let mut events = Vec::new();
                let mut disconnected = false;
                while events.len() < 64 {
                    match receiver.try_recv() {
                        Ok(event) => events.push(event),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
                backlog = events.len() == 64;
                if events.is_empty() {
                    if disconnected {
                        return;
                    }
                    continue;
                }
                let Ok(()) = this.update(cx, |workspace, _cx| {
                    // 严格 pane 路由 + 仲裁在宿主内完成;UI 消费(页签徽标/
                    // attention)归 05/09 篇,M4.5 不产生视图态。
                    let focused = workspace.focused_pane();
                    workspace.agent_hub.dispatch(events, focused);
                }) else {
                    return;
                };
                if disconnected {
                    return;
                }
            }
        })
        .detach();
    }

    /// 1 Hz 屏幕看门狗(照上游形态):逐 pane 取终端网格末尾样本喂仲裁器,
    /// 给无 hook 的客户端补位、纠正丢边的 hook 状态。
    ///
    /// 节奏刻意与输出解耦:每个 `Wakeup` 上跑规则会在输出洪流下放大成本,
    /// 而屏幕证据只需要拍帧级频率;`allows_screen()` 的 pane 由宿主内判定。
    fn start_agent_screen_watchdog(&mut self, cx: &mut Context<'_, Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_secs(1)).await;
                let Ok(()) = this.update(cx, |workspace, cx| {
                    for (pane_id, screen) in workspace.screen_samples(cx) {
                        workspace.agent_hub.observe_screen(pane_id, &screen);
                    }
                }) else {
                    return;
                };
            }
        })
        .detach();
    }

    /// 每个终端 pane 的屏幕观察窗口(最近至多 24 行,与屏幕规则注意力区
    /// 窗口同源)。无会话或零几何的 pane 跳过。
    fn screen_samples(&self, cx: &App) -> Vec<(PaneId, String)> {
        let mut samples = Vec::new();
        for entry in &self.tabs {
            let WorkspaceTab::Terminal { panes, .. } = &entry.tab else {
                continue;
            };
            for pane in panes {
                if let Some(screen) = pane.view.read(cx).agent_screen_sample() {
                    samples.push((pane.id, screen));
                }
            }
        }
        samples
    }

    /// 壳事件泵(照上游 residency.rs 形态):120ms 节奏排空通道,前台线程
    /// 派发。M3 事件族均无消费者(预埋缝,见 SltermShellEvent 逐变体注释)——
    /// 泵本体先立,消费臂归各篇。
    fn start_shell_event_pump(
        &mut self,
        rx: mpsc::Receiver<SltermShellEvent>,
        cx: &mut Context<'_, Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |_this, cx| {
            loop {
                executor.timer(Duration::from_millis(120)).await;
                let mut events = Vec::new();
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
                if events.is_empty() {
                    continue;
                }
                cx.update(|_cx| {
                    for event in events {
                        match event {
                            // 预埋缝:生产者归 04 篇 M7。
                            SltermShellEvent::MuxAttach => {}
                            SltermShellEvent::RuntimeControl => {}
                            // 消费(右键「在此处打开」)归 09 篇。
                            SltermShellEvent::OpenDirectories(_) => {}
                        }
                    }
                });
            }
        })
        .detach();
    }

    /// Terminal tab 的内容区:zoom 或单叶 → 聚焦 pane 满卡直渲(M3 路径
    /// 保留为特例);否则按分屏树递归铺陈。
    fn render_terminal_tab(&self, tab_ix: usize, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let Some(entry) = self.tabs.get(tab_ix) else {
            return div().into_any_element();
        };
        let tab_id = entry.id;
        let WorkspaceTab::Terminal {
            tree,
            panes,
            focused,
            zoomed,
        } = &entry.tab
        else {
            return div().into_any_element();
        };
        if *zoomed || tree.is_leaf() {
            let pane = panes
                .iter()
                .find(|pane| pane.id == *focused)
                .or_else(|| panes.first());
            let Some(pane) = pane else {
                return div().into_any_element();
            };
            let pane_id = pane.id;
            let store = self.pane_bounds.clone();
            // 满卡路径也记录窗口坐标:方向导航与拖拽命中依赖上一帧矩形,
            // 不能只在进入分屏递归后才得到边界。
            return div()
                .size_full()
                .min_w_0()
                .min_h_0()
                .relative()
                .debug_selector(move || format!("slterm-pane-{}", pane_id.0))
                .child(pane.view.clone())
                .child(
                    canvas(
                        move |bounds, _, _| {
                            store.borrow_mut().insert(pane_id, bounds);
                        },
                        |_, _, _, _| (),
                    )
                    .absolute()
                    .inset_0(),
                )
                .into_any_element();
        }
        let mut path = Vec::new();
        let ctx = SplitRenderContext {
            tab_ix,
            tab_id,
            panes,
            focused: *focused,
        };
        self.render_split_node(&ctx, tree, &mut path, cx)
    }

    /// 递归渲染分屏树。Split 节点 = flex 容器:first 占 `relative(ratio)`、
    /// 分隔条固定 `DIVIDER_GAP`、second 吃剩余(与 `slterm_split::layout`
    /// 的切割数学等价到 ±divider×ratio ≤ 2px;比例在提交时按真实视口吸附
    /// 整格,不累积漂移)。
    ///
    /// 渲染永远读**提交比例**:拖拽预览只走 overlay,终端网格零重排。
    /// 节点视口与 pane 矩形经 canvas prepaint 回写帧记录,供拖拽换算与
    /// 方向导航消费。
    fn render_split_node(
        &self,
        ctx: &SplitRenderContext<'_>,
        node: &SplitTree<PaneId>,
        path: &mut Vec<bool>,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        match node {
            SplitTree::Leaf(id) => {
                let id = *id;
                let Some(pane) = ctx.panes.iter().find(|pane| pane.id == id) else {
                    return div().into_any_element();
                };
                let store = self.pane_bounds.clone();
                let is_focused = id == ctx.focused;
                let tab_ix = ctx.tab_ix;
                // 非聚焦 pane 压暗(05 篇开放问题 1 默认带 veil;主题开关
                // 归 06 键域)。veil 只盖终端区,不压暗分屏分隔条。
                let veil = (!is_focused).then(|| {
                    div()
                        .absolute()
                        .inset_0()
                        .bg(cx.theme().background.opacity(0.35))
                });
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .debug_selector(move || format!("slterm-pane-{}", id.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.focus_pane(tab_ix, id, window, cx);
                        }),
                    )
                    // canvas 探针量整个叶子:方向导航与 dock 命中读的是 pane
                    // 矩形。absolute + inset_0 不参与布局,纯观测。
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                store.borrow_mut().insert(id, bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        div()
                            .size_full()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            .child(pane.view.clone())
                            .children(veil),
                    )
                    .into_any_element()
            }
            SplitTree::Split {
                direction,
                ratio,
                dragging,
                first,
                second,
                ..
            } => {
                let direction = *direction;
                // 交互拖拽走轻量 overlay;这里保持提交比例,避免每次高频
                // 指针事件都重建/重排两侧终端网格。
                let show_ratio = *ratio;
                let dragging = *dragging;
                let key = (ctx.tab_id, path.clone());
                let store = self.split_bounds.clone();
                let probe = canvas(
                    move |bounds, _, _| {
                        store.borrow_mut().insert(key.clone(), bounds);
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0();

                path.push(false);
                let first_el = self.render_split_node(ctx, first, path, cx);
                path.pop();
                path.push(true);
                let second_el = self.render_split_node(ctx, second, path, cx);
                path.pop();

                let tab_ix = ctx.tab_ix;
                let tab_id = ctx.tab_id;
                let drag_path = path.clone();
                let drag_start_ratio = *ratio;
                let selector_path = drag_path.clone();
                let divider_id =
                    SharedString::from(format!("split-divider-{tab_ix}-{drag_path:?}"));
                let bar_color = if dragging {
                    cx.theme().primary
                } else {
                    cx.theme().border.opacity(0.35)
                };
                // 视觉线 DIVIDER_GAP,命中区向两侧各扩 HIT_SLOP(旧壳同参)。
                let hit = div()
                    .id(divider_id)
                    .debug_selector(move || format!("split-divider-{tab_ix}-{selector_path:?}"))
                    .absolute()
                    .map(|hit| match direction {
                        SplitDirection::LeftRight => hit
                            .top_0()
                            .bottom_0()
                            .left(px(-HIT_SLOP))
                            .w(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_col_resize(),
                        SplitDirection::TopBottom => hit
                            .left_0()
                            .right_0()
                            .top(px(-HIT_SLOP))
                            .h(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_row_resize(),
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            this.split_drag = Some(SplitDrag {
                                tab: tab_id,
                                path: drag_path.clone(),
                                direction,
                                preview_ratio: drag_start_ratio,
                                close_target: None,
                                last_notified: std::time::Instant::now(),
                            });
                            if let Some(WorkspaceTab::Terminal { tree, .. }) =
                                this.tabs.get_mut(tab_ix).map(|entry| &mut entry.tab)
                                && let Some(SplitTree::Split {
                                    preview_ratio,
                                    dragging,
                                    ..
                                }) = tree.node_mut(&drag_path)
                            {
                                *preview_ratio = None;
                                *dragging = true;
                            }
                            cx.notify();
                        }),
                    );
                let divider = div()
                    .relative()
                    .flex_shrink_0()
                    .map(|bar| match direction {
                        SplitDirection::LeftRight => bar.w(px(DIVIDER_GAP)).h_full(),
                        SplitDirection::TopBottom => bar.h(px(DIVIDER_GAP)).w_full(),
                    })
                    .bg(bar_color)
                    .child(hit);

                div()
                    .size_full()
                    .relative()
                    .flex()
                    .map(|el| match direction {
                        SplitDirection::LeftRight => el.flex_row(),
                        SplitDirection::TopBottom => el.flex_col(),
                    })
                    .child(probe)
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .map(|el| match direction {
                                SplitDirection::LeftRight => el.h_full().w(relative(show_ratio)),
                                SplitDirection::TopBottom => el.w_full().h(relative(show_ratio)),
                            })
                            .child(first_el),
                    )
                    .child(divider)
                    .child(div().flex_1().min_w_0().min_h_0().child(second_el))
                    .into_any_element()
            }
        }
    }
}

impl Render for SltermWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let content = match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Terminal { .. }) => Some(self.render_terminal_tab(self.active, cx)),
            Some(WorkspaceTab::Settings { view, .. }) => Some(view.clone().into_any_element()),
            None => None,
        };
        // 侧栏三件套:ActivityBar + tab 列表/双槽内容区 + 宽度拖拽热区。
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
            // 自绘标题栏(05 篇):活跃 tab 展示名 + 三钮 + 拖拽区。
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
                            .children(content),
                    ),
            )
            .children(sidebar_resize_overlay)
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

/// 工作区级键绑定(05 篇 M5.3;键位照抄上游:ctrl-shift-d 左右、
/// ctrl-shift-s 上下、ctrl-shift-enter 缩放、ctrl-alt+方向切聚焦、
/// ctrl-shift-w 关聚焦 pane、ctrl-shift-t 新 tab、ctrl-tab/ctrl-shift-tab
/// 切 tab、ctrl-shift-b 侧栏开合)。`CloseActiveTab` 暂无默认键位——
/// 与上游 ctrl-shift-w 的整 tab 关闭语义冲突,入口归 07/09 菜单裁定。
/// 在 `gpui_component::init` 之后调用一次。
pub fn init(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-d", SplitRight, None),
        KeyBinding::new("ctrl-shift-s", SplitDown, None),
        KeyBinding::new("ctrl-shift-enter", ToggleZoom, None),
        KeyBinding::new("ctrl-alt-left", FocusPaneLeft, None),
        KeyBinding::new("ctrl-alt-right", FocusPaneRight, None),
        KeyBinding::new("ctrl-alt-up", FocusPaneUp, None),
        KeyBinding::new("ctrl-alt-down", FocusPaneDown, None),
        KeyBinding::new("ctrl-shift-w", CloseActivePane, None),
        KeyBinding::new("ctrl-shift-t", NewTerminalTab, None),
        KeyBinding::new("ctrl-tab", SelectNextTab, None),
        KeyBinding::new("ctrl-shift-tab", SelectPreviousTab, None),
        KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
    ]);
}

#[cfg(test)]
mod workspace_tests {
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
