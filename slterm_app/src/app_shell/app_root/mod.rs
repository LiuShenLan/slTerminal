//! 项目工作区运行根：唯一持有项目快照与全部项目面板实例。

mod application_close;
mod batch_close;
pub(crate) mod close_coordinator;
mod command_registry;
mod commands;
#[allow(dead_code)] // 消费点归 Editor 行标、Diff 启动恢复与可见性接线。
mod diff;
mod document_external;
mod document_recovery;
mod documents;
mod editor_git;
mod events;
mod explorer;
mod focus_manager;
mod git_show;
pub(crate) mod keymap;
mod language_selector;
mod layout;
mod markdown_links;
mod navigation;
mod panel_host;
mod project_close;
mod project_view;
mod projects;
mod render;
mod settings_overlay;
mod side_view;
mod side_view_host;
mod sidebar;
mod tabs;
mod task_service;
mod terminal_close;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Pixels, Point, ScrollHandle, Subscription,
    Window,
};
use gpui_component::menu::PopupMenu;
use slterm_document::{DocumentPageCache, DocumentRegistry};
use slterm_path::{CanonicalPath, PathGrant};
use slterm_workspace::{
    PanelGroupId, PanelId, PanelIdAllocator, ProjectId, ProjectSnapshot, SideViewId,
};

use crate::app_shell::global_close::GlobalCloseClient;
use crate::gpui_terminal::registry::TerminalRegistry;
use crate::persistence::workspace_writer::WorkspaceWriterClient;
use crate::platform::project_paths::{ProjectPathResolver, SystemProjectPathResolver};

use focus_manager::{FocusManager, FocusRegistration, FocusSubscriptions};
pub(crate) use side_view::AppRootPersistence;
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
        AdjustSplitLeft,
        AdjustSplitRight,
        AdjustSplitUp,
        AdjustSplitDown,
        CloseActiveTerminal,
        ReconnectTerminal,
        FocusNextPanel,
        FocusPreviousPanel,
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
        OpenLayoutMenu,
        OpenSettings,
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

/// 分割路径只作运行期焦点键；布局事实仍由 ProjectLayout 独占。
struct SplitDividerFocus {
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

/// 当前布局菜单锚定组身份；菜单项只生成统一命令参数。
struct LayoutMenu {
    group: PanelGroupId,
    menu: Entity<PopupMenu>,
    registration: FocusRegistration,
    _dismiss: Subscription,
}

/// 组内完整页签列表；菜单一并进入 FocusManager 模态链。
struct TabListMenu {
    group: PanelGroupId,
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    registration: FocusRegistration,
    _dismiss: Subscription,
}

/// 生产 AppRoot 的外部服务；全局关闭 client 不可省略。
pub(crate) struct AppRootServices {
    pub(crate) persistence: AppRootPersistence,
    pub(crate) global_close: GlobalCloseClient,
    pub(crate) learning: crate::learning::LearningService,
}

/// 单窗口运行根；`projects` 只是强实体引用表，不复制项目注册信息。
pub(crate) struct AppRoot {
    snapshot: ProjectSnapshot,
    snapshot_generation: u64,
    /// 最近一次 Explorer 根同步对应的快照代次；渲染只在差异时延迟接线。
    side_view_roots_generation: Option<u64>,
    project_writer: Option<WorkspaceWriterClient>,
    store_epoch: Option<String>,
    pending_final_project_snapshot: Option<ProjectSnapshot>,
    project_snapshot_recovery_failed: bool,
    close_in_progress: bool,
    close_completed: bool,
    application_close: Option<application_close::ApplicationClose>,
    pending_application_close: Option<gpui::AnyWindowHandle>,
    recovery_global_close: Option<application_close::RecoveryGlobalClose>,
    global_close: Option<GlobalCloseClient>,
    terminal_closes: BTreeMap<slterm_workspace::TerminalId, terminal_close::LocalTerminalClose>,
    terminal_close_batches: BTreeMap<uuid::Uuid, batch_close::LocalTerminalBatchClose>,
    pending_batch_commits: std::collections::BTreeSet<uuid::Uuid>,
    terminal_close_batch_members: BTreeMap<slterm_workspace::TerminalId, uuid::Uuid>,
    next_terminal_close_sequence: u64,
    project_closes: BTreeMap<ProjectId, project_close::LocalProjectClose>,
    projects: BTreeMap<ProjectId, Entity<ProjectView>>,
    pending_project_opens: VecDeque<ProjectOpenRequest>,
    active_project_open: Option<uuid::Uuid>,
    project_open_draining: bool,
    panel_ids: PanelIdAllocator,
    documents: DocumentRegistry,
    editor_git: BTreeMap<PanelId, editor_git::EditorGitState>,
    document_session_read_grants: BTreeMap<slterm_document::DocumentId, PathGrant>,
    document_autosave: crate::app_shell::document_autosave::DocumentAutosaveCoordinator,
    focused_document_view: Option<(slterm_document::DocumentId, slterm_workspace::ViewKey)>,
    terminals: Rc<RefCell<TerminalRegistry>>,
    agents: crate::agent_host::AgentHub,
    paged_documents:
        BTreeMap<slterm_document::DocumentId, Arc<crate::app_shell::paged_document::PagedDocument>>,
    document_syntax: BTreeMap<
        (slterm_document::DocumentId, slterm_document::LanguageId),
        Rc<RefCell<crate::app_shell::text_view::DocumentSyntax>>,
    >,
    markdown_render_services: crate::app_shell::markdown::MarkdownRenderServices,
    page_cache: Rc<RefCell<DocumentPageCache>>,
    git: Arc<crate::git::GitFacade>,
    /// 真实位置解析端口；Explorer 终端 cwd 与其它打开链共用同一实现。
    path_resolver: Arc<dyn crate::platform::document_paths::DocumentPathResolver>,
    /// 文件树单项操作唯一执行体；含阻塞 I/O，只在后台调用。
    file_operations: Arc<crate::app_shell::file_operation_service::FileOperationService>,
    system_open: Arc<dyn Fn(CanonicalPath) + Send + Sync>,
    #[allow(dead_code)] // 消费点 = Document 真实后台打开。
    document_open_service: Arc<crate::app_shell::documents::DocumentOpenService>,
    #[allow(dead_code)] // 消费点 = Document 单任务 FIFO 调度。
    document_open_active: bool,
    #[allow(dead_code)] // 消费点 = Document 打开请求队列。
    pending_document_opens: VecDeque<documents::DocumentOpenRequest>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    test_document_read_hold: Option<CanonicalPath>,
    #[cfg(all(test, feature = "gpui-test-support"))]
    held_document_read: Option<(
        documents::DocumentOpenRequest,
        crate::app_shell::documents::ResolvedDocument,
    )>,
    document_external_service: Arc<crate::app_shell::document_external::DocumentExternalService>,
    pending_document_relocations: BTreeMap<slterm_document::DocumentId, CanonicalPath>,
    pending_document_refreshes: std::collections::BTreeSet<slterm_document::DocumentId>,
    active_document_relocations: BTreeMap<slterm_document::DocumentId, CanonicalPath>,
    last_document_relocation_targets: BTreeMap<slterm_document::DocumentId, CanonicalPath>,
    document_relocation_aliases: BTreeMap<std::path::PathBuf, slterm_document::DocumentId>,
    fs_watch: Option<crate::app_shell::fs_watch::FsWatchHandle>,
    pending_document_saves: std::collections::BTreeSet<slterm_document::DocumentId>,
    panel_subscriptions: BTreeMap<PanelId, Subscription>,
    panel_focus: BTreeMap<PanelId, FocusRegistration>,
    split_dividers: BTreeMap<(ProjectId, Vec<bool>), SplitDividerFocus>,
    tab_scrolls: BTreeMap<(ProjectId, PanelGroupId), ScrollHandle>,
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
    layout_menu: Option<LayoutMenu>,
    tab_list_menu: Option<TabListMenu>,
    language_selector_menu: Option<language_selector::LanguageSelectorMenu>,
    settings_overlay: Option<settings_overlay::SettingsOverlayHost>,
    side_view_persistence: AppRootPersistence,
    learning: crate::learning::LearningService,
    launcher: Box<dyn TerminalPanelLauncher>,
    project_paths: Arc<dyn ProjectPathResolver>,
    shell: CanonicalPath,
}

impl AppRoot {
    pub(crate) fn new(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        services: AppRootServices,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut root = Self::with_launcher(
            panel_ids,
            shell,
            Box::new(commands::LocalTerminalLauncher),
            services.persistence,
            Some(services.global_close),
            cx,
        );
        root.learning = services.learning;
        root
    }

    fn with_launcher(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        launcher: Box<dyn TerminalPanelLauncher>,
        mut persistence: AppRootPersistence,
        global_close: Option<GlobalCloseClient>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let workspace_focus = cx.focus_handle();
        let mut focus_manager = FocusManager::new(&workspace_focus);
        let project_writer = persistence.writer_client();
        let store_epoch = persistence.store_epoch();
        let project_snapshot_recovery_failed = persistence.project_snapshot_recovery_failed();
        let snapshot = persistence.take_project_snapshot();
        let documents = persistence.take_documents();
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
        let mut root = Self {
            snapshot,
            snapshot_generation: 0,
            side_view_roots_generation: None,
            project_writer,
            store_epoch,
            pending_final_project_snapshot: None,
            project_snapshot_recovery_failed,
            close_in_progress: false,
            close_completed: false,
            application_close: None,
            pending_application_close: None,
            recovery_global_close: None,
            global_close,
            terminal_closes: BTreeMap::new(),
            terminal_close_batches: BTreeMap::new(),
            pending_batch_commits: std::collections::BTreeSet::new(),
            terminal_close_batch_members: BTreeMap::new(),
            next_terminal_close_sequence: 0,
            project_closes: BTreeMap::new(),
            projects,
            pending_project_opens: VecDeque::new(),
            active_project_open: None,
            project_open_draining: false,
            panel_ids,
            documents,
            editor_git: BTreeMap::new(),
            document_session_read_grants: BTreeMap::new(),
            document_autosave: cx
                .try_global::<crate::app_shell::config::Settings>()
                .map(
                    crate::app_shell::document_autosave::DocumentAutosaveCoordinator::from_settings,
                )
                .unwrap_or_default(),
            focused_document_view: None,
            terminals: Rc::new(RefCell::new(TerminalRegistry::default())),
            agents: crate::agent_host::AgentHub::new(),
            paged_documents: BTreeMap::new(),
            document_syntax: BTreeMap::new(),
            markdown_render_services: crate::app_shell::markdown::MarkdownRenderServices::default(),
            page_cache: Rc::new(RefCell::new(DocumentPageCache::new())),
            git: Arc::new(crate::git::GitFacade::default()),
            path_resolver: Arc::new(crate::platform::document_paths::SystemDocumentPathResolver),
            file_operations: Arc::new(
                crate::app_shell::file_operation_service::FileOperationService::default(),
            ),
            system_open: Arc::new(|path| {
                crate::platform::file_operations::open_in_system(path.as_path());
            }),
            document_open_service: Arc::new(
                crate::app_shell::documents::DocumentOpenService::default(),
            ),
            document_open_active: false,
            pending_document_opens: VecDeque::new(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            test_document_read_hold: None,
            #[cfg(all(test, feature = "gpui-test-support"))]
            held_document_read: None,
            document_external_service: Arc::new(
                crate::app_shell::document_external::DocumentExternalService::default(),
            ),
            pending_document_relocations: BTreeMap::new(),
            pending_document_refreshes: std::collections::BTreeSet::new(),
            active_document_relocations: BTreeMap::new(),
            last_document_relocation_targets: BTreeMap::new(),
            document_relocation_aliases: BTreeMap::new(),
            fs_watch: crate::app_shell::fs_watch::FsWatchHandle::new(),
            pending_document_saves: std::collections::BTreeSet::new(),
            panel_subscriptions: BTreeMap::new(),
            panel_focus: BTreeMap::new(),
            split_dividers: BTreeMap::new(),
            tab_scrolls: BTreeMap::new(),
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
            layout_menu: None,
            tab_list_menu: None,
            language_selector_menu: None,
            settings_overlay: None,
            side_view_persistence: persistence,
            learning: crate::learning::LearningService::memory_only(),
            launcher,
            project_paths: Arc::new(SystemProjectPathResolver),
            shell,
        };
        root.reconcile_fs_watch();
        root.reconcile_split_dividers(cx);
        root.reconcile_tab_scrolls(None);
        root
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
    fn commit_project_snapshot(&mut self, snapshot: ProjectSnapshot, cx: &mut App) {
        self.snapshot_generation = self.snapshot_generation.wrapping_add(1);
        let previous = std::mem::replace(&mut self.snapshot, snapshot);
        self.reconcile_document_registry(cx);
        self.reconcile_split_dividers(cx);
        self.reconcile_tab_scrolls(Some(&previous));
        self.sync_project_visibility(cx);
        if self.project_snapshot_recovery_failed {
            return;
        }
        if !self.project_closes.is_empty()
            || self.terminal_closes.values().any(|close| {
                matches!(
                    close.coordinator.phase(),
                    slterm_workspace::ClosePhase::Running(
                        slterm_workspace::CloseStage::Save
                            | slterm_workspace::CloseStage::CommitState
                    ) | slterm_workspace::ClosePhase::Failed(
                        slterm_workspace::CloseStage::CommitState
                    )
                )
            })
            || self.terminal_close_batches.values().any(|close| {
                matches!(
                    close.coordinator.phase(),
                    slterm_workspace::ClosePhase::Running(
                        slterm_workspace::CloseStage::Save
                            | slterm_workspace::CloseStage::CommitState
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
use projects::ProjectOpenRequest;
