//! 侧栏骨架:ActivityBar + 上下双槽内容区(05 篇 D05-2 已裁双槽形态)。
//!
//! 槽位状态机照旧栈侧栏 R1-R9 纯函数语义移植:
//! - R1/R2(`toggle_view_pure`):点击未打开视图 = 打开并隐式替换同槽旧视图;
//!   点击已打开视图 = 关闭该槽。
//! - R3-R5(`derive_layout`):双空 hidden / 仅上 single-top / 仅下
//!   single-bottom / 双开 split。
//! - R9(`reconcile_views`):持久化恢复时把未注册或错槽的 active view 置空;
//!   「缺失注册 id 补按钮」由封闭集每帧重建按钮列表机械保证,无需追加状态。
//!
//! 本步只落骨架与占位:视图内容归 03/07 篇,图标归 06 篇;按钮换区排序
//! (旧栈 R6-R8)归后续步骤。`SidebarState` 的四字段是 session schema 的
//! 持久化面(D05-3 已裁归 session),既有字段追加一律走 `serde(default)`。
//!
//! 视图形状收缩为封闭集 [`SideViewId`]:按钮列表由 [`side_view_defs`] 每帧
//! 重建,槽位归属由 [`SideViewId::zone`] 单点裁定(本步无拖拽换区)。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    div, px,
};
use gpui_component::{ActiveTheme as _, Icon, IconName};
use serde::{Deserialize, Serialize};

use super::SltermWorkspace;

/// ActivityBar 固定宽度(旧栈同值 46)。
pub(crate) const ACTIVITY_BAR_SIZE: f32 = 46.0;
/// 侧栏 tab 行高度(旧壳纵向列表同档 30)。
const TAB_ROW_HEIGHT: f32 = 30.0;
/// 相邻 tab 行间距。
const TAB_ROW_GAP: f32 = 2.0;
/// 宽度拖拽热区宽度(内容区右缘竖条)。
pub(crate) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 4.0;
/// 侧栏内容区默认宽度。
pub(crate) const SIDEBAR_WIDTH_DEFAULT: f32 = 250.0;
/// 侧栏内容区最小宽度(旧栈同值)。
pub(super) const SIDEBAR_WIDTH_MIN: f32 = 160.0;
/// 侧栏内容区最大宽度(旧栈同值)。
pub(super) const SIDEBAR_WIDTH_MAX: f32 = 500.0;

/// 侧栏半区标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SidebarZone {
    Top,
    Bottom,
}

impl SidebarZone {
    /// 调试/测试选择器用短名。
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

/// 双槽布局推导结果(R3-R5)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SidebarLayout {
    Hidden,
    SingleTop,
    SingleBottom,
    Split,
}

/// 侧栏视图封闭集(旧栈注册序:nav/explorer/commit/agentFiles)。
///
/// 内容归属:nav 数据源归 D05-2 所在篇,explorer/commit 归 07 篇,
/// agentFiles 归 03 篇。图标归 06 篇,本步只登记 id + title。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SideViewId {
    Nav,
    Explorer,
    Commit,
    AgentFiles,
}

impl SideViewId {
    /// 封闭集全集,注册序。R9 对齐与按钮渲染共用这一处真值源。
    pub(super) const ALL: [SideViewId; 4] = [
        SideViewId::Nav,
        SideViewId::Explorer,
        SideViewId::Commit,
        SideViewId::AgentFiles,
    ];

    /// 持久化/调试短名(与 serde snake_case 同形)。
    pub(super) const fn id_str(self) -> &'static str {
        match self {
            Self::Nav => "nav",
            Self::Explorer => "explorer",
            Self::Commit => "commit",
            Self::AgentFiles => "agent_files",
        }
    }

    /// 从持久化短名解析;未注册 id 返回 None(R9 的输入侧防线)。
    ///
    /// 兼容旧产品面 camelCase 的 `agentFiles` 写法:会话文件可手工编辑,
    /// 未知/旧写法一律退回未打开而不是整份会话拒收。
    pub(super) fn from_id(id: &str) -> Option<Self> {
        match id {
            "nav" => Some(Self::Nav),
            "explorer" => Some(Self::Explorer),
            "commit" => Some(Self::Commit),
            "agent_files" | "agentFiles" => Some(Self::AgentFiles),
            _ => None,
        }
    }

    /// 显示名(侧栏槽标题/按钮占位文案)。
    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Nav => "导航树",
            Self::Explorer => "文件浏览器",
            Self::Commit => "Commit",
            Self::AgentFiles => "Agent 全局文件",
        }
    }

    /// 槽位归属单点。本步无按钮拖拽换区(旧栈 R6-R8 归后续),固定:
    /// 上槽 = nav/explorer/commit(旧栈 DEFAULT_ZONES 上区),下槽 = agentFiles。
    pub(super) const fn zone(self) -> SidebarZone {
        match self {
            Self::AgentFiles => SidebarZone::Bottom,
            Self::Nav | Self::Explorer | Self::Commit => SidebarZone::Top,
        }
    }

    /// 内容占位文案——真实视图归对应分片,本步只提示归属。
    pub(super) const fn content_owner(self) -> &'static str {
        match self {
            Self::Nav => "视图内容归 05 篇(导航树数据源)",
            Self::Explorer | Self::Commit => "视图内容归 07 篇",
            Self::AgentFiles => "视图内容归 03 篇",
        }
    }
}

/// side view 注册项骨架:id + title;icon 归 06 篇。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SideViewDef {
    pub id: SideViewId,
    pub title: &'static str,
}

/// 封闭集定义列表(注册序 = 旧栈 sideViewDefs 注册序)。
pub(super) fn side_view_defs() -> [SideViewDef; 4] {
    SideViewId::ALL.map(|id| SideViewDef {
        id,
        title: id.title(),
    })
}

/// 侧栏骨架态:collapsed/width/双槽 active view。
///
/// D05-3 已裁归 session:字段直接进 session schema 跟现场恢复,不是设置域
/// 偏好。宽度用 f32 逻辑像素,载入时 clamp 到旧栈区间。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct SidebarState {
    pub collapsed: bool,
    pub active_view_top: Option<SideViewId>,
    pub active_view_bottom: Option<SideViewId>,
    pub width: f32,
}

impl Default for SidebarState {
    /// 旧栈 DEFAULT_OPEN:上槽默认打开导航树,下槽空,侧栏展开。
    fn default() -> Self {
        Self {
            collapsed: false,
            active_view_top: Some(SideViewId::Nav),
            active_view_bottom: None,
            width: SIDEBAR_WIDTH_DEFAULT,
        }
    }
}

impl<'de> Deserialize<'de> for SidebarState {
    /// 缺失字段回默认值、未注册 id 回未打开、宽度钳制(R9 对齐)。类型级
    /// 损坏(如数字落入字符串槽)仍按会话解析失败处理,不在此吞掉。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(default)]
        struct RawSidebar {
            collapsed: bool,
            active_view_top: Option<String>,
            active_view_bottom: Option<String>,
            width: f32,
        }

        impl Default for RawSidebar {
            fn default() -> Self {
                let defaults = SidebarState::default();
                Self {
                    collapsed: defaults.collapsed,
                    active_view_top: defaults.active_view_top.map(|id| id.id_str().to_owned()),
                    active_view_bottom: None,
                    width: defaults.width,
                }
            }
        }

        let raw = RawSidebar::deserialize(deserializer)?;
        let state = SidebarState {
            collapsed: raw.collapsed,
            active_view_top: raw.active_view_top.as_deref().and_then(SideViewId::from_id),
            active_view_bottom: raw
                .active_view_bottom
                .as_deref()
                .and_then(SideViewId::from_id),
            width: clamp_width(raw.width),
        };
        Ok(reconcile_views(&state, &SideViewId::ALL))
    }
}

impl SidebarState {
    /// 指定槽位当前视图。
    pub(super) fn active_in(&self, zone: SidebarZone) -> Option<SideViewId> {
        match zone {
            SidebarZone::Top => self.active_view_top,
            SidebarZone::Bottom => self.active_view_bottom,
        }
    }
}

/// 启动几何用的侧栏总宽:collapsed 时只剩 ActivityBar(内容区与拖拽热区
/// 不占布局),与首帧渲染同源——恢复的宽度/开合态不能等首帧后再改网格。
pub(crate) fn chrome_width(state: &SidebarState) -> f32 {
    if state.collapsed {
        ACTIVITY_BAR_SIZE
    } else {
        ACTIVITY_BAR_SIZE + state.width + SIDEBAR_RESIZE_HANDLE_WIDTH
    }
}

/// 将宽度钳到旧栈区间;非有限数回退最小宽度(旧栈 clamp 同语义)。
pub(super) fn clamp_width(width: f32) -> f32 {
    if !width.is_finite() {
        return SIDEBAR_WIDTH_MIN;
    }
    width.clamp(SIDEBAR_WIDTH_MIN, SIDEBAR_WIDTH_MAX)
}

/// R1/R2:点击视图开关。打开动作同时解除 collapsed——点了按钮就是要看内容。
/// 返回新状态,不修改入参(纯函数)。
pub(super) fn toggle_view_pure(state: &SidebarState, id: SideViewId) -> SidebarState {
    let zone = id.zone();
    let mut next = *state;
    let slot = match zone {
        SidebarZone::Top => &mut next.active_view_top,
        SidebarZone::Bottom => &mut next.active_view_bottom,
    };
    *slot = if *slot == Some(id) { None } else { Some(id) };
    if next.active_in(zone).is_some() {
        next.collapsed = false;
    }
    next
}

/// R3-R5:由双槽打开状态纯推导展示形态。collapsed 不参与推导——
/// collapsed 是独立的整区开合开关,不改变槽位内容。
pub(super) fn derive_layout(state: &SidebarState) -> SidebarLayout {
    match (
        state.active_view_top.is_some(),
        state.active_view_bottom.is_some(),
    ) {
        (false, false) => SidebarLayout::Hidden,
        (true, false) => SidebarLayout::SingleTop,
        (false, true) => SidebarLayout::SingleBottom,
        (true, true) => SidebarLayout::Split,
    }
}

/// R9:按注册表对齐 active view。
///
/// 旧栈在载入时过滤未注册 id、把缺失注册 id 追加到上区末尾;本形态按钮
/// 列表每帧由封闭集重建(不存在持久化的按钮归属),故对齐只剩 active view
/// 校验一项:视图必须已注册且处于其固定槽位,否则置空。
pub(super) fn reconcile_views(state: &SidebarState, registered: &[SideViewId]) -> SidebarState {
    let valid = |zone: SidebarZone, view: Option<SideViewId>| {
        view.filter(|id| registered.contains(id) && id.zone() == zone)
    };
    SidebarState {
        collapsed: state.collapsed,
        active_view_top: valid(SidebarZone::Top, state.active_view_top),
        active_view_bottom: valid(SidebarZone::Bottom, state.active_view_bottom),
        width: clamp_width(state.width),
    }
}

/// 进行中的侧栏宽度拖拽:指针原始位移与起始宽度解耦,只改内容区宽度,
/// 不重排终端网格。
pub(super) struct SidebarResizeDrag {
    pub start_x: f32,
    pub start_width: f32,
}

/// 指针位置 → 新宽度(带旧栈区间钳制)。
pub(super) fn dragged_width(drag: &SidebarResizeDrag, pointer_x: f32) -> f32 {
    clamp_width(drag.start_width + pointer_x - drag.start_x)
}

impl SltermWorkspace {
    /// ActivityBar:封闭集按钮区。按钮图标归 06 篇,本步用标题首字占位,
    /// active 态 = 选中底色 + 左侧指示条(旧栈 ActivityBar 同形)。
    pub(super) fn render_activity_bar(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = self.sidebar;
        div()
            .id("sidebar-activity-bar")
            .debug_selector(|| "sidebar-activity-bar".to_owned())
            .w(px(ACTIVITY_BAR_SIZE))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .py(px(6.0))
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .children(side_view_defs().into_iter().map(|def| {
                let active = state.active_in(def.id.zone()) == Some(def.id);
                let hover_bg = cx.theme().list_hover;
                let active_bg = cx.theme().sidebar_accent;
                let idle_fg = cx.theme().muted_foreground;
                let active_fg = cx.theme().sidebar_accent_foreground;
                let accent = cx.theme().primary;
                div()
                    .id(SharedString::from(format!(
                        "sidebar-view-btn-{}",
                        def.id.id_str()
                    )))
                    .debug_selector(move || format!("sidebar-view-btn-{}", def.id.id_str()))
                    .w(px(34.0))
                    .h(px(34.0))
                    .mb(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .when(active, |button| {
                        button
                            .bg(active_bg)
                            .text_color(active_fg)
                            .border_l_2()
                            .border_color(accent)
                    })
                    .when(!active, |button| button.text_color(idle_fg))
                    .hover(move |style| style.bg(hover_bg))
                    .child(def.title.chars().next().unwrap_or('?').to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_sidebar_view(def.id, cx);
                    }))
            }))
            // 底部配置钮:设置 tab 的唯一常驻入口(05 篇 M5.3b 登记项)。
            .child(div().flex_1())
            .child(
                div()
                    .id("sidebar-settings-btn")
                    .debug_selector(|| "sidebar-settings-btn".to_owned())
                    .w(px(34.0))
                    .h(px(34.0))
                    .mb(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .hover({
                        let hover_bg = cx.theme().list_hover;
                        move |button| button.bg(hover_bg)
                    })
                    .child(Icon::new(IconName::Settings).size(px(16.0)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings_tab(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// 侧栏列:顶部常驻 tab 列表 + 其下双槽内容区。
    ///
    /// tab 列表不随双槽全空而消失——它是切换 tab 与拖拽停靠的入口;
    /// collapsed 才整列隐藏(由 workspace 根判定)。
    pub(super) fn render_sidebar_content(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        let layout = derive_layout(&self.sidebar);
        let views_open = layout != SidebarLayout::Hidden && !self.sidebar.collapsed;
        let top_open = self.sidebar.active_view_top.is_some();
        let bottom_open = self.sidebar.active_view_bottom.is_some();
        // 视图内容需要 `&mut self`(文件树轮询/路径栏);先求值再进链式
        // 闭包,避免多个闭包同时借 self。
        let top_view = top_open.then(|| self.render_view_slot(SidebarZone::Top, cx));
        let bottom_view = bottom_open.then(|| self.render_view_slot(SidebarZone::Bottom, cx));
        let tabs = self.render_tab_list(cx);
        div()
            .id("sidebar-column")
            .debug_selector(|| "sidebar-column".to_owned())
            .size_full()
            .flex()
            .flex_col()
            // 待命阶段的指针跟踪(激活后由根部罩层独占)。
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.update_tab_drag(event, window, cx);
            }))
            .child(tabs)
            .when(views_open, |column| {
                column.child(
                    div()
                        .id("sidebar-area")
                        .debug_selector(|| "sidebar-area".to_owned())
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .flex()
                        .flex_col()
                        .bg(cx.theme().sidebar)
                        .children(top_view)
                        .when(top_open && bottom_open, |area| {
                            area.child(
                                div()
                                    .h(px(1.0))
                                    .w_full()
                                    .flex_shrink_0()
                                    .bg(cx.theme().sidebar_border),
                            )
                        })
                        .children(bottom_view),
                )
            })
            .into_any_element()
    }

    /// 侧栏 tab 列表(05 篇:左侧垂直 tab 侧边栏)。
    ///
    /// 行是 tab 拖拽的源:按下进入待命态,越过阈值后拖进终端区停靠。
    /// 徽章/新建钮/折叠槽与右键菜单归后续步骤。
    pub(super) fn render_tab_list(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let active = self.active;
        let active_bg = cx.theme().sidebar_accent;
        let active_fg = cx.theme().sidebar_accent_foreground;
        let muted = cx.theme().muted_foreground;
        let hover_bg = cx.theme().list_hover;
        let rows = self.tabs.iter().enumerate().map(|(ix, entry)| {
            let title = self.tab_title(ix, cx);
            let is_active = ix == active;
            let tab_id = entry.id;
            div()
                .id(SharedString::from(format!("sidebar-tab-{ix}")))
                .debug_selector(move || format!("sidebar-tab-{ix}"))
                .h(px(TAB_ROW_HEIGHT))
                .w_full()
                .flex_shrink_0()
                .px(px(8.0))
                .flex()
                .items_center()
                .rounded(px(6.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .when(is_active, |row| row.bg(active_bg).text_color(active_fg))
                .when(!is_active, |row| {
                    row.text_color(muted).hover(move |style| style.bg(hover_bg))
                })
                .child(div().min_w_0().truncate().child(title))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_tab(ix, window, cx);
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, _| {
                        this.begin_tab_drag(tab_id, event.position);
                    }),
                )
        });
        div()
            .id("sidebar-tabs")
            .debug_selector(|| "sidebar-tabs".to_owned())
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(TAB_ROW_GAP))
            .py(px(6.0))
            .children(rows)
            .into_any_element()
    }

    /// 单槽内容挂载点。explorer = 文件树(07 篇 M8.4);其余视图保持
    /// M5.3b 占位(内容归对应分片)。
    fn render_view_slot(&mut self, zone: SidebarZone, cx: &mut Context<'_, Self>) -> AnyElement {
        let Some(id) = self.sidebar.active_in(zone) else {
            return div().into_any_element();
        };
        let selector = format!("sidebar-slot-{}-{}", zone.as_str(), id.id_str());
        let body = if id == SideViewId::Explorer {
            self.render_file_tree(cx)
        } else {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(30.0))
                        .px(px(10.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .text_size(px(12.0))
                        .text_color(cx.theme().sidebar_foreground)
                        .child(id.title()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px(px(10.0))
                        .py(px(8.0))
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(id.content_owner()),
                )
                .into_any_element()
        };
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector)
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .child(body)
            .into_any_element()
    }

    /// explorer 槽当前可见(内容渲染/跟随 cwd 的单一判据)。
    pub(super) fn explorer_open(&self) -> bool {
        !self.sidebar.collapsed
            && self
                .sidebar
                .active_in(SideViewId::Explorer.zone())
                .is_some_and(|id| id == SideViewId::Explorer)
    }

    /// 宽度拖拽热区:内容区右缘 4px 竖条。
    pub(super) fn render_sidebar_resize_handle(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        div()
            .id("sidebar-resize-handle")
            .debug_selector(|| "sidebar-resize-handle".to_owned())
            .w(px(SIDEBAR_RESIZE_HANDLE_WIDTH))
            .h_full()
            .flex_shrink_0()
            .cursor_col_resize()
            .bg(cx.theme().sidebar_border)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.sidebar_resize = Some(SidebarResizeDrag {
                        start_x: f32::from(event.position.x),
                        start_width: this.sidebar.width,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// 拖拽期间的全窗指针罩层:松手/失配即收尾。
    pub(super) fn render_sidebar_resize_overlay(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        self.sidebar_resize.as_ref().map(|_| {
            div()
                .absolute()
                .inset_0()
                .occlude()
                .cursor_col_resize()
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.update_sidebar_resize(event, cx);
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.finish_sidebar_resize(cx);
                    }),
                )
                .into_any_element()
        })
    }

    fn toggle_sidebar_view(&mut self, id: SideViewId, cx: &mut Context<'_, Self>) {
        self.sidebar = toggle_view_pure(&self.sidebar, id);
        cx.notify();
    }

    fn update_sidebar_resize(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.finish_sidebar_resize(cx);
            return;
        }
        let Some(drag) = self.sidebar_resize.as_ref() else {
            return;
        };
        let width = dragged_width(drag, f32::from(event.position.x));
        if (self.sidebar.width - width).abs() > f32::EPSILON {
            self.sidebar.width = width;
            cx.notify();
        }
    }

    fn finish_sidebar_resize(&mut self, cx: &mut Context<'_, Self>) {
        if self.sidebar_resize.take().is_some() {
            cx.notify();
        }
    }

    /// 侧栏整区开合(策略动作 ctrl-shift-b)。
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<'_, Self>) {
        self.sidebar.collapsed = !self.sidebar.collapsed;
        cx.notify();
    }
}

#[cfg(test)]
mod sidebar_state_tests {
    use super::*;

    fn state(top: Option<SideViewId>, bottom: Option<SideViewId>) -> SidebarState {
        SidebarState {
            collapsed: false,
            active_view_top: top,
            active_view_bottom: bottom,
            width: SIDEBAR_WIDTH_DEFAULT,
        }
    }

    /// R1:关闭的视图点击后打开。
    #[test]
    fn closed_view_opens_in_its_slot() {
        let next = toggle_view_pure(&state(None, None), SideViewId::Nav);
        assert_eq!(next.active_view_top, Some(SideViewId::Nav));
        assert_eq!(next.active_view_bottom, None);
    }

    /// R2:已打开的视图点击后关闭。
    #[test]
    fn open_view_closes_on_second_click() {
        let next = toggle_view_pure(&state(Some(SideViewId::Nav), None), SideViewId::Nav);
        assert_eq!(next.active_view_top, None);
    }

    /// R1 覆盖:同槽打开新视图隐式关闭旧视图,无历史记忆。
    #[test]
    fn opening_a_sibling_replaces_the_slot_without_history() {
        let next = toggle_view_pure(&state(Some(SideViewId::Nav), None), SideViewId::Explorer);
        assert_eq!(next.active_view_top, Some(SideViewId::Explorer));
    }

    /// 双槽独立:下槽视图开关不影响上槽。
    #[test]
    fn bottom_slot_toggles_independently_of_the_top() {
        let next = toggle_view_pure(&state(Some(SideViewId::Nav), None), SideViewId::AgentFiles);
        assert_eq!(next.active_view_top, Some(SideViewId::Nav));
        assert_eq!(next.active_view_bottom, Some(SideViewId::AgentFiles));

        let next = toggle_view_pure(&next, SideViewId::AgentFiles);
        assert_eq!(next.active_view_top, Some(SideViewId::Nav));
        assert_eq!(next.active_view_bottom, None);
    }

    /// 打开动作解除 collapsed;关闭动作保留开合态。
    #[test]
    fn opening_a_view_expands_a_collapsed_sidebar() {
        let mut collapsed = state(None, None);
        collapsed.collapsed = true;
        let opened = toggle_view_pure(&collapsed, SideViewId::Nav);
        assert!(!opened.collapsed);
        assert_eq!(opened.active_view_top, Some(SideViewId::Nav));
    }

    /// R3-R5:双槽布局纯推导。
    #[test]
    fn layout_is_derived_from_both_slots() {
        assert_eq!(derive_layout(&state(None, None)), SidebarLayout::Hidden);
        assert_eq!(
            derive_layout(&state(Some(SideViewId::Nav), None)),
            SidebarLayout::SingleTop
        );
        assert_eq!(
            derive_layout(&state(None, Some(SideViewId::AgentFiles))),
            SidebarLayout::SingleBottom
        );
        assert_eq!(
            derive_layout(&state(Some(SideViewId::Nav), Some(SideViewId::AgentFiles))),
            SidebarLayout::Split
        );
    }

    /// collapsed 是独立开合开关,不改变槽位推导。
    #[test]
    fn collapse_does_not_change_slot_derivation() {
        let mut collapsed = state(Some(SideViewId::Nav), None);
        collapsed.collapsed = true;
        assert_eq!(derive_layout(&collapsed), SidebarLayout::SingleTop);
    }

    /// R9:未注册 active view 置空。
    #[test]
    fn reconcile_nulls_unregistered_views() {
        let mut dirty = state(Some(SideViewId::Nav), Some(SideViewId::AgentFiles));
        dirty.width = 1.0;
        let reconciled = reconcile_views(&dirty, &[SideViewId::Explorer]);
        assert_eq!(reconciled.active_view_top, None);
        assert_eq!(reconciled.active_view_bottom, None);
        assert_eq!(reconciled.width, SIDEBAR_WIDTH_MIN, "宽度同步钳制");
    }

    /// R9:注册但槽位不符的 active view 置空(上槽不可能显示下槽视图)。
    #[test]
    fn reconcile_nulls_views_persisted_in_the_wrong_slot() {
        let wrong = state(Some(SideViewId::AgentFiles), Some(SideViewId::Nav));
        let reconciled = reconcile_views(&wrong, &SideViewId::ALL);
        assert_eq!(reconciled.active_view_top, None);
        assert_eq!(reconciled.active_view_bottom, None);
    }

    /// R9:注册且槽位正确时原样保留。
    #[test]
    fn reconcile_keeps_registered_views_in_their_slots() {
        let clean = state(Some(SideViewId::Nav), Some(SideViewId::AgentFiles));
        assert_eq!(reconcile_views(&clean, &SideViewId::ALL), clean);
    }

    /// 封闭集解析:已注册短名可用,未知 id 回 None,旧 camelCase 兼容。
    #[test]
    fn side_view_ids_parse_closed_set_names_only() {
        for id in SideViewId::ALL {
            assert_eq!(SideViewId::from_id(id.id_str()), Some(id));
        }
        assert_eq!(
            SideViewId::from_id("agentFiles"),
            Some(SideViewId::AgentFiles)
        );
        assert_eq!(SideViewId::from_id("obsolete"), None);
    }

    /// 场景序列 S1/S2:开→替换→关;双槽开→单开。
    #[test]
    fn scenario_open_replace_close_sequence() {
        let mut current = state(None, None);
        current = toggle_view_pure(&current, SideViewId::Nav);
        assert_eq!(derive_layout(&current), SidebarLayout::SingleTop);
        current = toggle_view_pure(&current, SideViewId::Explorer);
        assert_eq!(current.active_view_top, Some(SideViewId::Explorer));
        assert_eq!(derive_layout(&current), SidebarLayout::SingleTop);
        current = toggle_view_pure(&current, SideViewId::AgentFiles);
        assert_eq!(derive_layout(&current), SidebarLayout::Split);
        current = toggle_view_pure(&current, SideViewId::AgentFiles);
        assert_eq!(derive_layout(&current), SidebarLayout::SingleTop);
        current = toggle_view_pure(&current, SideViewId::Explorer);
        assert_eq!(derive_layout(&current), SidebarLayout::Hidden);
    }

    /// 宽度拖拽纯净换算 + 旧栈区间钳制。
    #[test]
    fn dragged_width_tracks_pointer_and_clamps_to_old_bounds() {
        let drag = SidebarResizeDrag {
            start_x: 300.0,
            start_width: 250.0,
        };
        assert_eq!(dragged_width(&drag, 340.0), 290.0);
        assert_eq!(dragged_width(&drag, -10_000.0), SIDEBAR_WIDTH_MIN);
        assert_eq!(dragged_width(&drag, 10_000.0), SIDEBAR_WIDTH_MAX);
        assert_eq!(clamp_width(f32::NAN), SIDEBAR_WIDTH_MIN);
    }
}
