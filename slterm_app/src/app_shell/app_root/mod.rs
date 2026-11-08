//! 项目工作区运行根：唯一持有项目快照与全部项目面板实例。

pub(crate) mod close_coordinator;
mod command_registry;
mod commands;
mod events;
mod focus_manager;
pub(crate) mod keymap;
mod panel_host;
mod project_close;
mod project_view;
mod render;
mod side_view;
mod side_view_host;
mod sidebar;
mod task_service;
mod terminal_close;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use std::collections::BTreeMap;

use gpui::{AppContext as _, Context, Entity, FocusHandle, Pixels, Point, Subscription, Window};
use gpui_component::menu::PopupMenu;
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelId, PanelIdAllocator, ProjectId, ProjectSnapshot, SideViewId};

use crate::persistence::workspace_writer::WorkspaceWriterClient;

use focus_manager::{FocusManager, FocusRegistration, FocusSubscriptions};
pub(super) use side_view::AppRootPersistence;
use side_view::SideView;
use side_view_host::SideViewHost;
use task_service::TaskService;

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
        OpenSideButtonMenu,
        AdjustSidebarWidthLeft,
        AdjustSidebarWidthRight,
        AdjustSidebarWidthLeftLarge,
        AdjustSidebarWidthRightLarge,
        ResetSidebarWidth,
        AdjustSidebarRatioUp,
        AdjustSidebarRatioDown,
        AdjustSidebarRatioUpLarge,
        AdjustSidebarRatioDownLarge,
        ResetSidebarRatio
    ]
);

/// 活动栏按钮的焦点句柄与弱注册；按钮生命周期独立于 SideView 实例。
struct SideButtonFocus {
    handle: FocusHandle,
    registration: FocusRegistration,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SidebarDivider {
    Width,
    Ratio,
}

/// 同一时刻仅保留一个侧栏布局手势，锚定按下时的指针和用户值。
#[derive(Clone, Copy)]
enum SidebarDrag {
    Width { start_x: f32, start_width: f32 },
    Ratio { start_y: f32, start_ratio: f32 },
}

/// 分隔线焦点独立于 SideView 实例，显隐资格由实际槽位布局决定。
struct SideDividerFocus {
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
    snapshot_generation: u64,
    project_writer: Option<WorkspaceWriterClient>,
    store_epoch: Option<String>,
    pending_final_project_snapshot: Option<ProjectSnapshot>,
    project_snapshot_recovery_failed: bool,
    close_in_progress: bool,
    close_completed: bool,
    terminal_closes: BTreeMap<slterm_workspace::TerminalId, terminal_close::LocalTerminalClose>,
    next_terminal_close_sequence: u64,
    project_closes: BTreeMap<ProjectId, project_close::LocalProjectClose>,
    projects: BTreeMap<ProjectId, Entity<ProjectView>>,
    panel_ids: PanelIdAllocator,
    panel_subscriptions: BTreeMap<PanelId, Subscription>,
    panel_focus: BTreeMap<PanelId, FocusRegistration>,
    workspace_focus: FocusHandle,
    tasks: TaskService,
    focus_manager: FocusManager,
    focus_subscriptions: Option<FocusSubscriptions>,
    side_views: SideViewHost<Entity<SideView>>,
    side_view_focus: BTreeMap<SideViewId, FocusRegistration>,
    side_buttons: BTreeMap<SideViewId, SideButtonFocus>,
    side_dividers: BTreeMap<SidebarDivider, SideDividerFocus>,
    sidebar_drag: Option<SidebarDrag>,
    side_button_menu: Option<SideButtonMenu>,
    side_view_persistence: AppRootPersistence,
    launcher: Box<dyn TerminalPanelLauncher>,
    shell: CanonicalPath,
}

impl AppRoot {
    pub(crate) fn new(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        persistence: AppRootPersistence,
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
        mut persistence: AppRootPersistence,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let workspace_focus = cx.focus_handle();
        let mut focus_manager = FocusManager::new(&workspace_focus);
        let project_writer = persistence.writer_client();
        let store_epoch = persistence.store_epoch();
        let project_snapshot_recovery_failed = persistence.project_snapshot_recovery_failed();
        let snapshot = persistence.take_project_snapshot();
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
        let mut side_dividers = BTreeMap::new();
        for divider in [SidebarDivider::Width, SidebarDivider::Ratio] {
            let eligible = sidebar::divider_is_visible(side_views.state(), divider);
            let handle = cx.focus_handle().tab_stop(true);
            let registration = focus_manager
                .register(&handle, parent, None, eligible)
                .expect("侧栏分隔线焦点注册必须唯一");
            side_dividers.insert(
                divider,
                SideDividerFocus {
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
        let mut projects = BTreeMap::new();
        let active = snapshot.registry().active();
        for project_id in snapshot
            .registry()
            .projects()
            .iter()
            .map(|project| project.id())
        {
            let project = cx.new(ProjectView::new);
            let registration = focus_manager
                .register(
                    &project.read(cx).focus_handle,
                    parent,
                    Some(project_id),
                    active == Some(project_id),
                )
                .expect("恢复项目焦点注册必须唯一");
            project.update(cx, |project, _| {
                project.focus_registration = Some(registration);
            });
            projects.insert(project_id, project);
        }
        Self {
            snapshot,
            snapshot_generation: 0,
            project_writer,
            store_epoch,
            pending_final_project_snapshot: None,
            project_snapshot_recovery_failed,
            close_in_progress: false,
            close_completed: false,
            terminal_closes: BTreeMap::new(),
            next_terminal_close_sequence: 0,
            project_closes: BTreeMap::new(),
            projects,
            panel_ids,
            panel_subscriptions: BTreeMap::new(),
            panel_focus: BTreeMap::new(),
            workspace_focus,
            tasks: TaskService::new(),
            focus_manager,
            focus_subscriptions: None,
            side_views,
            side_view_focus,
            side_buttons,
            side_dividers,
            sidebar_drag: None,
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

    /// 快照只在候选提交后进入既有写队列；关闭屏障期间仅更新内存并延期发布。
    fn commit_project_snapshot(&mut self, snapshot: ProjectSnapshot) {
        self.snapshot_generation = self.snapshot_generation.wrapping_add(1);
        self.snapshot = snapshot;
        if self.project_snapshot_recovery_failed {
            return;
        }
        if !self.project_closes.is_empty()
            || self.terminal_closes.values().any(|close| {
                matches!(
                    close.coordinator.phase(),
                    slterm_workspace::ClosePhase::Running(
                        slterm_workspace::CloseStage::CommitState
                    ) | slterm_workspace::ClosePhase::Failed(
                        slterm_workspace::CloseStage::CommitState
                    )
                )
            })
        {
            self.pending_final_project_snapshot = Some(self.snapshot.clone());
            return;
        }
        self.publish_project_snapshot();
    }

    /// 已知初始提交失败后恢复最新普通写；未知结果继续保持写屏障。
    pub(super) fn resume_deferred_project_snapshot(&mut self) {
        if self.pending_final_project_snapshot.is_some()
            && self.ensure_project_snapshot_command_allowed().is_ok()
        {
            self.publish_project_snapshot();
        }
    }

    fn publish_project_snapshot(&mut self) {
        let Some(writer) = &self.project_writer else {
            return;
        };
        match writer.save_project_snapshot(&self.snapshot) {
            Ok(()) => self.pending_final_project_snapshot = None,
            Err(error) => {
                self.pending_final_project_snapshot = Some(self.snapshot.clone());
                log::warn!("could not queue project snapshot: {error}");
            }
        }
    }

    pub(crate) const fn project_snapshot_recovery_failed(&self) -> bool {
        self.project_snapshot_recovery_failed
    }

    pub(crate) const fn snapshot(&self) -> &ProjectSnapshot {
        &self.snapshot
    }
}

impl Drop for AppRoot {
    fn drop(&mut self) {
        if self.project_snapshot_recovery_failed
            || self.ensure_project_snapshot_command_allowed().is_err()
        {
            return;
        }
        let (Some(writer), Some(snapshot)) =
            (&self.project_writer, &self.pending_final_project_snapshot)
        else {
            return;
        };
        if let Err(error) = writer.save_final_project_snapshot(snapshot) {
            log::warn!("could not queue final project snapshot: {error}");
        }
    }
}

use project_view::ProjectView;
