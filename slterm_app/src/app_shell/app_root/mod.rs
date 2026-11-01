//! 项目工作区运行根：唯一持有项目快照与全部项目面板实例。

mod commands;
mod events;
mod focus_manager;
pub(crate) mod keymap;
mod panel_host;
mod project_view;
mod render;
mod side_view;
mod side_view_host;
mod sidebar;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use std::collections::BTreeMap;

use gpui::{Context, Entity, FocusHandle, Pixels, Point, Subscription, Window};
use gpui_component::menu::PopupMenu;
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelId, PanelIdAllocator, ProjectId, ProjectSnapshot, SideViewId};

use focus_manager::{FocusManager, FocusRegistration, FocusSubscriptions};
use side_view::SideView;
pub(super) use side_view::SideViewPersistence;
use side_view_host::SideViewHost;

pub(crate) use commands::TerminalPanelLauncher;

gpui::actions!(
    slterm_app_root,
    [
        NewTerminal,
        SplitRight,
        SplitDown,
        CloseActiveTerminal,
        ToggleSidebar,
        ActivateNav,
        ActivateExplorer,
        ActivateCommit,
        ActivateAgentFiles,
        FocusActivityBar,
        FocusNextSideButton,
        FocusPreviousSideButton,
        FocusFirstSideButton,
        FocusLastSideButton,
        ActivateSideButton,
        OpenSideButtonMenu
    ]
);

/// 活动栏按钮的焦点句柄与弱注册；按钮生命周期独立于 SideView 实例。
struct SideButtonFocus {
    handle: FocusHandle,
    registration: FocusRegistration,
}

/// 当前按钮菜单与其锚点；菜单实体只由 AppRoot 持有。
struct SideButtonMenu {
    view: SideViewId,
    position: Option<Point<Pixels>>,
    menu: Entity<PopupMenu>,
    registration: FocusRegistration,
    _dismiss: Subscription,
}

/// 单窗口运行根；`projects` 只是强实体引用表，不复制项目注册信息。
pub(crate) struct AppRoot {
    snapshot: ProjectSnapshot,
    projects: BTreeMap<ProjectId, Entity<ProjectView>>,
    panel_ids: PanelIdAllocator,
    panel_subscriptions: BTreeMap<PanelId, Subscription>,
    panel_focus: BTreeMap<PanelId, FocusRegistration>,
    workspace_focus: FocusHandle,
    focus_manager: FocusManager,
    focus_subscriptions: Option<FocusSubscriptions>,
    side_views: SideViewHost<Entity<SideView>>,
    side_view_focus: BTreeMap<SideViewId, FocusRegistration>,
    side_buttons: BTreeMap<SideViewId, SideButtonFocus>,
    side_button_menu: Option<SideButtonMenu>,
    side_view_persistence: SideViewPersistence,
    launcher: Box<dyn TerminalPanelLauncher>,
    shell: CanonicalPath,
}

impl AppRoot {
    pub(crate) fn new(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        persistence: SideViewPersistence,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::with_launcher(
            panel_ids,
            shell,
            Box::new(commands::LocalTerminalLauncher),
            persistence,
            cx,
        )
    }

    fn with_launcher(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        launcher: Box<dyn TerminalPanelLauncher>,
        mut persistence: SideViewPersistence,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let workspace_focus = cx.focus_handle();
        let mut focus_manager = FocusManager::new(&workspace_focus);
        let sidebar_state = persistence.take_sidebar_state();
        let side_views = SideViewHost::new(sidebar_state, persistence.controller(cx));
        let parent = focus_manager.workspace_token();
        let mut side_buttons = BTreeMap::new();
        for view in SideViewId::ALL {
            let handle = cx.focus_handle().tab_stop(true);
            let registration = focus_manager
                .register(
                    &handle,
                    parent,
                    None,
                    side_views.state().button_is_visible(view),
                )
                .expect("活动栏按钮焦点注册必须唯一");
            side_buttons.insert(
                view,
                SideButtonFocus {
                    handle,
                    registration,
                },
            );
        }
        let mut side_view_focus = BTreeMap::new();
        for view in SideViewId::ALL {
            let Some(instance) = side_views.instance(view) else {
                continue;
            };
            let handle = instance
                .read(cx)
                .focus_handle
                .clone()
                .expect("生产 SideView 必须持有焦点句柄");
            let registration = focus_manager
                .register(&handle, parent, None, side_views.state().is_visible())
                .expect("SideView 焦点注册必须唯一");
            side_view_focus.insert(view, registration);
        }
        Self {
            snapshot: ProjectSnapshot::default(),
            projects: BTreeMap::new(),
            panel_ids,
            panel_subscriptions: BTreeMap::new(),
            panel_focus: BTreeMap::new(),
            workspace_focus,
            focus_manager,
            focus_subscriptions: None,
            side_views,
            side_view_focus,
            side_buttons,
            side_button_menu: None,
            side_view_persistence: persistence,
            launcher,
            shell,
        }
    }

    pub(crate) fn observe_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus_subscriptions = Some(FocusSubscriptions::new(
            cx,
            window,
            |root, window, cx| root.sync_focus(window, cx),
            |root, window, cx| root.recover_focus(window, cx),
        ));
    }

    #[allow(dead_code)] // 生产快照保存与窗口切换时消费。
    pub(crate) const fn snapshot(&self) -> &ProjectSnapshot {
        &self.snapshot
    }
}

use project_view::ProjectView;
