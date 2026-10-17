//! 工作区:tabs/pane 生命周期的唯一属主(05 篇三件套锚点)。
//!
//! 不变式(照上游):Workspace 是 pane 实体的唯一属主,生命周期与回收全走
//! 这里;`terminal_registry` 只是模块级索引,写入点唯一 = `new_pane`/
//! 关 pane 两处。
//!
//! M3 雏形:恒单窗口、单 Terminal tab、单 pane。字段语义按 05 篇终态锚定
//! (含 M3 恒空的 `split_bounds` 机制先立),分屏/多 tab 的渲染递归归 M5。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement as _,
    Pixels, Render, Styled as _, Subscription, Window, div,
};
use gpui_component::TitleBar;
use slterm_split::{PaneId, RemoveOutcome, SplitTree, TabId};

use super::SltermShellEvent;
use super::windowing::{WorkspaceStartup, startup_geometry};
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};
use crate::terminal_registry::{self, PaneSessionMeta};

/// 一个终端 pane:视图实体 + 宿主订阅。id 全 workspace 唯一、终生不复用
/// (03 篇 hook 的 `SLTERM_PANE_ID` 环境值与此同源)。
pub struct TerminalPane {
    pub id: PaneId,
    /// 用户重命名(持久化归 05 session 叶;M3 无重命名 UI,恒 None)。
    #[allow(dead_code)]
    pub custom_name: Option<String>,
    pub view: Entity<TerminalView>,
    _subscription: Subscription,
}

/// 唯一合法 tab 类型集(面板封闭的 Rust 落点)。M3 只有 Terminal 变体;
/// Settings/Document/Code 等变体归 06/07 篇按片落地。
pub enum WorkspaceTab {
    Terminal {
        /// 实体属主,无序按 id 查。
        panes: Vec<TerminalPane>,
        /// 叶 = pane id。
        tree: SplitTree<PaneId>,
        /// 键盘焦点与 split/close 作用对象。
        focused: PaneId,
        /// 聚焦 pane 满卡(内存态,快照不含)。
        #[allow(dead_code)] // M3 单叶恒 false;zoom 操作面归 M5。
        zoomed: bool,
    },
}

/// tab + 展示态元数据同槽存放(单 Vec 消下同标耦合)。
pub struct TabEntry {
    #[allow(dead_code)] // 稳定身份:M3 单 tab 只经 active 下标访问,TabId 消费面归 M5。
    pub id: TabId,
    pub tab: WorkspaceTab,
}

/// 壳内状态权威雏形:直持 pane 实体与展示态,无投影层(13 篇改造节 2)。
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
    pane_bounds: Rc<RefCell<HashMap<PaneId, Bounds<Pixels>>>>,
    /// SplitBoundsStore:M3 恒空,机制先立(05 篇终态键 = (tab 下标, 节点路径))。
    #[allow(dead_code)]
    split_bounds: SplitBoundsMap,
}

/// SplitBoundsStore 容器形态(05 篇契约):键 = (tab 下标, 分屏节点路径)。
type SplitBoundsMap = Rc<RefCell<HashMap<(usize, Vec<bool>), Bounds<Pixels>>>>;

impl SltermWorkspace {
    pub fn new(
        window: &mut Window,
        shell_rx: mpsc::Receiver<SltermShellEvent>,
        startup: WorkspaceStartup,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 开窗 resize 异步落地前,先按目标几何谓出 spawn 网格(启动零 resize)。
        let initial_grid = startup_geometry::prepare_initial_grid(
            window,
            cx,
            startup_geometry::M3_SIDEBAR_WIDTH,
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
        };
        match startup {
            WorkspaceStartup::NewTerminal { cwd } => {
                workspace.add_terminal_tab(cwd, None, false, window, cx);
            }
            WorkspaceStartup::LaunchTerminal { cwd, command } => {
                // `-e` 语义:命令本体即 pane 的启动进程(不是默认 shell 里
                // 再注入一条命令)——冻结进 launch,会话恢复照此重建。显式
                // 命令通道:core 跳过三家白名单并做裸名 PATH 路径化(SEC-01
                // 豁免论证见 `Options::explicit_command`)。
                let name = command.program().to_owned();
                let shell = slterm_terminal::tty::Shell::new(
                    command.program().to_owned(),
                    command.args().to_vec(),
                );
                workspace.add_terminal_tab(cwd, Some((shell, name)), true, window, cx);
            }
        }
        workspace.start_shell_event_pump(shell_rx, cx);
        workspace
    }

    /// 建一个 Terminal tab(cwd + 可选的显式 shell 身份),并聚焦其 pane。
    fn add_terminal_tab(
        &mut self,
        cwd: Option<std::path::PathBuf>,
        shell: Option<(slterm_terminal::tty::Shell, String)>,
        explicit_command: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let launch = match shell {
            Some((shell, name)) => TerminalLaunch::Local {
                cwd,
                shell: Some(shell),
                shell_name: Some(name),
                explicit_command,
            },
            None => TerminalLaunch::Local {
                cwd,
                shell: None,
                shell_name: None,
                explicit_command,
            },
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
        });
        self.active = self.tabs.len() - 1;
        self.focus_active(window, cx);
    }

    /// 创建一个 pane 实体(分配 id、spawn 会话、挂宿主订阅、写注册表);
    /// 调用方决定它进新 tab 还是接进某棵分屏树(M5)。
    fn new_pane(
        &mut self,
        grid: (u16, u16),
        launch: TerminalLaunch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> TerminalPane {
        let pane_id = PaneId(self.next_pane);
        self.next_pane = self.next_pane.saturating_add(1);
        let view = cx.new(|cx| TerminalView::new(pane_id, grid, launch, window, cx));
        let subscription = cx.subscribe_in(&view, window, Self::on_terminal_event);
        // 注册表写入点 1/2(spawn 侧;另一侧 = close_pane)。
        terminal_registry::register(pane_id, PaneSessionMeta::default());
        TerminalPane {
            id: pane_id,
            custom_name: None,
            view,
            _subscription: subscription,
        }
    }

    /// 视图事件白集(M3):标题变化重绘;会话结束走关 pane 状态机;
    /// 用户输入的广播扇出归 M5。
    fn on_terminal_event(
        &mut self,
        view: &Entity<TerminalView>,
        event: &TerminalViewEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            TerminalViewEvent::TitleChanged => {
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
        })
    }

    /// 关 pane 状态机(05 篇锚定):摘除 → 立即显式 shutdown → 焦点交幸存
    /// 首叶;唯一叶子关闭 = 关 tab = 无 tab 时关窗(`LastWindowClosed` 退进程)。
    fn close_pane(&mut self, pane_id: PaneId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(tab_index) = self.tabs.iter().position(|entry| match &entry.tab {
            WorkspaceTab::Terminal { tree, .. } => tree.contains(pane_id),
        }) else {
            return;
        };
        let outcome = match &mut self.tabs[tab_index].tab {
            WorkspaceTab::Terminal { tree, .. } => tree.remove_leaf(pane_id),
        };
        match outcome {
            RemoveOutcome::NotFound => {}
            RemoveOutcome::WasRoot => {
                if let Some(pane) = self.tab_panes(tab_index).find(|pane| pane.id == pane_id) {
                    pane.view.read(cx).shutdown();
                }
                terminal_registry::remove(pane_id);
                self.pane_bounds.borrow_mut().remove(&pane_id);
                self.tabs.remove(tab_index);
                if self.tabs.is_empty() {
                    window.remove_window();
                } else {
                    self.active = self.active.min(self.tabs.len() - 1);
                    cx.notify();
                }
            }
            RemoveOutcome::Collapsed(next_focus) => {
                // M3 单变体:多 tab 形态归 M5,届时恢复 if let 分派。
                let WorkspaceTab::Terminal {
                    panes,
                    focused,
                    zoomed,
                    ..
                } = &mut self.tabs[tab_index].tab;
                {
                    if let Some(position) = panes.iter().position(|pane| pane.id == pane_id) {
                        let pane = panes.remove(position);
                        // 被摘 pane 立即显式 shutdown,不等 Drop 兜底。
                        pane.view.read(cx).shutdown();
                    }
                    *focused = next_focus;
                    *zoomed = false;
                }
                terminal_registry::remove(pane_id);
                self.pane_bounds.borrow_mut().remove(&pane_id);
                cx.notify();
            }
        }
    }

    fn tab_panes(&self, tab_index: usize) -> impl Iterator<Item = &TerminalPane> {
        self.tabs
            .get(tab_index)
            .map(|entry| match &entry.tab {
                WorkspaceTab::Terminal { panes, .. } => panes.as_slice(),
            })
            .unwrap_or(&[])
            .iter()
    }

    /// 测试专用:活跃 tab 首个 pane 的视图实体(M3 恒单 pane,渲染/几何
    /// 断言经此句柄读视图)。属 05 篇终态的「宿主读 pane」最小面。
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
        }
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
}

impl Render for SltermWorkspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let content = match self.tabs.get(self.active).map(|entry| &entry.tab) {
            Some(WorkspaceTab::Terminal { panes, focused, .. }) => {
                // zoomed 或单叶 → 聚焦 pane 满卡直渲(M3 恒此臂);分屏递归
                // 渲染归 M5(render_split_node + SplitBounds 探针)。
                let pane = panes
                    .iter()
                    .find(|pane| pane.id == *focused)
                    .or_else(|| panes.first());
                pane.map(|pane| {
                    let store = self.pane_bounds.clone();
                    let pane_id = pane.id;
                    div().size_full().relative().child(pane.view.clone()).child(
                        // canvas 探针:prepaint 回写 pane 矩形(命中换算/
                        // 拖拽归 M5 读上一帧记录)。absolute + inset_0 不
                        // 参与布局,纯观测。
                        gpui::canvas(
                            move |bounds, _, _| {
                                store.borrow_mut().insert(pane_id, bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    )
                })
            }
            None => None,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            // 标题栏系统底座(13 篇缝合点 11):gpui-component TitleBar 承载
            // 窗口标题与三钮;自绘标题栏演化归 05/09。
            .child(TitleBar::new())
            .children(content)
    }
}

/// 工作区级键绑定:M3 不注册(应用族键位 ctrl+shift+t/w/... 的 action 与
/// 分发归 M3.3/M5;终端视图内已按白名单放行冒泡)。
pub fn init(_cx: &mut App) {}
