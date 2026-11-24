//! AppRoot 的项目与终端命令。

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::rc::Rc;

use gpui::{App, AppContext as _, Context, Entity, FocusHandle, Window};
use slterm_path::CanonicalPath;
use slterm_sidebar::SidebarError;
use slterm_split::SplitDirection;
use slterm_workspace::{
    AddProjectOutcome, LayoutError, LogicalSize, NewTerminalSpec, PanelGroupId, PanelId,
    PanelIdExhausted, ProjectCommandError, ProjectId, ProjectRegistryError, ProjectSnapshot,
    ProjectSnapshotError, RunIntent, SplitTerminalSpec, TerminalId, TerminalRecord, ViewKey,
};
use uuid::Uuid;

use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::registry::{RegistryError, TerminalRegistry, TerminalTarget};
use crate::gpui_terminal::session::SharedTerminal;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};

use super::command_registry::{
    CommandId, CommandOutcome, CommandParameters, CommandSource, CommandTarget,
};
use super::focus_manager::{FocusError, FocusToken};
use super::panel_host::{PanelContent, PanelHost};
use super::task_service::{CancellationToken, TaskId};
use super::{AppRoot, ProjectView};

pub(crate) struct ReconnectLaunch {
    pub(crate) session: crate::gpui_terminal::session::TerminalSession,
    pub(crate) events: crate::gpui_terminal::event_mailbox::EventReceiver,
    pub(crate) exec_context: Option<crate::completion::PaneExecContext>,
    pub(crate) cwd_fallback: bool,
}

/// 终端面板启动适配器；测试可替换为无真实 PTY 的实现。
pub(crate) trait TerminalPanelLauncher {
    fn launch(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        terminals: &Rc<RefCell<TerminalRegistry>>,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<Entity<TerminalView>>;

    fn reconnect(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        recovery_root: &CanonicalPath,
        terminal: Option<SharedTerminal>,
        window_size: slterm_terminal::event::WindowSize,
        term_config: slterm_terminal::term::Config,
    ) -> io::Result<ReconnectLaunch>;

    /// 恢复既有面板；Launch 失败与 Exited 都保留实体，不重复逻辑记录。
    fn restore(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        terminals: &Rc<RefCell<TerminalRegistry>>,
        recovery_root: &CanonicalPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TerminalView>;
}

pub(crate) struct LocalTerminalLauncher;

fn local_launch(record: &TerminalRecord) -> io::Result<TerminalLaunch> {
    let shell = record
        .shell()
        .as_path()
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Shell 路径必须为有效 Unicode"))?
        .to_owned();
    Ok(launch_metadata(
        record,
        Some(slterm_terminal::tty::Shell::new(shell, Vec::new())),
    ))
}

fn fallback_launch(record: &TerminalRecord) -> TerminalLaunch {
    launch_metadata(record, None)
}

fn launch_metadata(
    record: &TerminalRecord,
    shell: Option<slterm_terminal::tty::Shell>,
) -> TerminalLaunch {
    TerminalLaunch::Local {
        cwd: Some(record.cwd().as_path().to_path_buf()),
        shell,
        shell_name: record.custom_title().map(str::to_owned),
        explicit_command: false,
    }
}

impl TerminalPanelLauncher for LocalTerminalLauncher {
    fn launch(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        terminals: &Rc<RefCell<TerminalRegistry>>,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<Entity<TerminalView>> {
        let launch = local_launch(record)?;
        let launched = TerminalView::launch(
            PaneId(panel_id.sequence()),
            record.id(),
            terminals,
            (
                TerminalView::DEFAULT_GRID_COLUMNS,
                TerminalView::DEFAULT_GRID_LINES,
            ),
            launch,
            window,
            cx,
        )?;
        Ok(cx.new(|cx| TerminalView::from_launched(launched, window, cx)))
    }

    fn reconnect(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        recovery_root: &CanonicalPath,
        terminal: Option<SharedTerminal>,
        window_size: slterm_terminal::event::WindowSize,
        term_config: slterm_terminal::term::Config,
    ) -> io::Result<ReconnectLaunch> {
        let launch = local_launch(record)?;
        let TerminalLaunch::Local {
            cwd,
            shell,
            explicit_command,
            ..
        } = launch;
        let options = crate::gpui_terminal::session::local_options(
            shell,
            explicit_command,
            PaneId(panel_id.sequence()),
            cwd,
        )?;
        let local = crate::gpui_terminal::session::spawn_local(
            window_size,
            term_config,
            options,
            Some(recovery_root.as_path()),
            terminal,
        )?;
        let (session, events) = local.spawned;
        Ok(ReconnectLaunch {
            session,
            events,
            exec_context: local.exec_context,
            cwd_fallback: local.cwd_fallback,
        })
    }

    fn restore(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        terminals: &Rc<RefCell<TerminalRegistry>>,
        recovery_root: &CanonicalPath,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TerminalView> {
        let pane_id = PaneId(panel_id.sequence());
        let grid = (
            TerminalView::DEFAULT_GRID_COLUMNS,
            TerminalView::DEFAULT_GRID_LINES,
        );
        match record.run_intent() {
            RunIntent::Exited => {
                let launch = local_launch(record).unwrap_or_else(|_| fallback_launch(record));
                cx.new(|cx| TerminalView::restore_exited(pane_id, grid, launch, window, cx))
            }
            RunIntent::Launch => match local_launch(record) {
                Ok(launch) => cx.new(|cx| {
                    TerminalView::restore_launch(
                        pane_id,
                        record.id(),
                        terminals,
                        launch,
                        crate::gpui_terminal::view::RestoreLaunchOptions {
                            spawn_grid: grid,
                            recovery_root: Some(recovery_root.as_path().to_path_buf()),
                        },
                        window,
                        cx,
                    )
                }),
                Err(error) => {
                    let launch = fallback_launch(record);
                    cx.new(|cx| {
                        TerminalView::restore_error(
                            pane_id,
                            grid,
                            launch,
                            format!("PTY 启动失败: {error}"),
                            window,
                            cx,
                        )
                    })
                }
            },
        }
    }
}

pub(super) fn take_terminal_worker(
    registry: &Rc<RefCell<TerminalRegistry>>,
    terminal_id: TerminalId,
) -> Option<crate::gpui_terminal::session::TerminalSession> {
    let target = registry.borrow().target(terminal_id)?;
    registry.borrow_mut().remove(target).ok()
}

/// 项目运行命令错误；失败时正式快照与实例表保持不变。
#[derive(Debug)]
pub(crate) enum AppRootError {
    Project(ProjectCommandError),
    Registry(ProjectRegistryError),
    Snapshot(ProjectSnapshotError),
    Layout(LayoutError),
    PanelIdExhausted,
    Launch(io::Error),
    TerminalReconnect(RegistryError),
    ProjectPath(io::Error),
    DocumentOpen(crate::app_shell::documents::DocumentOpenError),
    DocumentClose(slterm_workspace::DocumentCloseError),
    Focus(FocusError),
    ProjectNotActive,
    NewTerminalTargetUnauthorized,
    OpenFileTargetUnauthorized,
    MissingPanel,
    NoLayoutActions,
    UnexpectedParameters,
    Closing,
    ProjectSnapshotCommitInFlight,
    ProjectSnapshotRecoveryFailed,
    TerminalCloseRunning,
    TerminalCloseState,
    TerminalCloseOutcomeUnknown,
    ProjectCloseRunning,
    ProjectCloseState,
    ProjectCloseOutcomeUnknown,
    WorkspaceEpochUnavailable,
    /// 目标项目仍有未提交 Hooks 草稿：须先保存、丢弃或取消。
    HooksDraftPending,
    TerminalCloseWriter(crate::persistence::workspace_writer::WorkspaceWriterError),
    ProjectCloseWriter(crate::persistence::workspace_writer::WorkspaceWriterError),
    Sidebar(SidebarError),
}

impl fmt::Display for AppRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(error) => write!(formatter, "项目快照命令失败: {error}"),
            Self::Registry(error) => write!(formatter, "项目注册命令失败: {error:?}"),
            Self::Snapshot(error) => write!(formatter, "项目快照不一致: {error}"),
            Self::Layout(error) => write!(formatter, "布局命令失败: {error:?}"),
            Self::PanelIdExhausted => formatter.write_str("面板 ID 序列已耗尽"),
            Self::Launch(error) => write!(formatter, "终端启动失败: {error}"),
            Self::TerminalReconnect(error) => write!(formatter, "终端重连失败: {error}"),
            Self::ProjectPath(error) => write!(formatter, "项目根真实路径解析失败: {error}"),
            Self::DocumentOpen(error) => write!(formatter, "文档打开失败: {error}"),
            Self::DocumentClose(error) => write!(formatter, "文档关闭守卫失败: {error}"),
            Self::Focus(error) => write!(formatter, "焦点目标不可用: {error:?}"),
            Self::ProjectNotActive => formatter.write_str("只能在活跃项目中显式创建终端"),
            Self::NewTerminalTargetUnauthorized => formatter.write_str("新终端目标项目未获授权"),
            Self::OpenFileTargetUnauthorized => {
                formatter.write_str("打开文件没有覆盖它的既有项目根")
            }
            Self::MissingPanel => formatter.write_str("终端运行面板不存在"),
            Self::NoLayoutActions => formatter.write_str("当前组没有可用布局操作"),
            Self::UnexpectedParameters => formatter.write_str("命令参数与目标命令不匹配"),
            Self::Closing => formatter.write_str("应用正在关闭，拒绝新命令"),
            Self::ProjectSnapshotCommitInFlight => {
                formatter.write_str("关闭快照正在提交，暂拒显式快照命令")
            }
            Self::ProjectSnapshotRecoveryFailed => {
                formatter.write_str("项目快照恢复失败，拒绝关闭写入")
            }
            Self::TerminalCloseRunning => formatter.write_str("终端正在关闭，拒绝重复请求"),
            Self::TerminalCloseState => formatter.write_str("终端关闭状态转换无效"),
            Self::TerminalCloseOutcomeUnknown => {
                formatter.write_str("终端关闭结果未知，已停止自动重试")
            }
            Self::ProjectCloseRunning => formatter.write_str("项目正在关闭，拒绝重复或并发请求"),
            Self::ProjectCloseState => formatter.write_str("项目关闭状态转换无效"),
            Self::ProjectCloseOutcomeUnknown => {
                formatter.write_str("项目关闭结果未知，已停止自动重试")
            }
            Self::WorkspaceEpochUnavailable => {
                formatter.write_str("工作区当前世代不可用，拒绝关闭")
            }
            Self::HooksDraftPending => {
                formatter.write_str("项目仍有未提交的 Hooks 草稿，请先保存或丢弃")
            }
            Self::TerminalCloseWriter(error) => write!(formatter, "终端关闭写请求失败: {error}"),
            Self::ProjectCloseWriter(error) => write!(formatter, "项目关闭写请求失败: {error}"),
            Self::Sidebar(error) => write!(formatter, "侧栏命令失败: {error}"),
        }
    }
}

impl std::error::Error for AppRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(error) => Some(error),
            Self::Snapshot(error) => Some(error),
            Self::Launch(error) | Self::ProjectPath(error) => Some(error),
            Self::TerminalReconnect(error) => Some(error),
            Self::DocumentOpen(error) => Some(error),
            Self::TerminalCloseWriter(error) | Self::ProjectCloseWriter(error) => Some(error),
            Self::Sidebar(error) => Some(error),
            Self::Registry(_)
            | Self::Layout(_)
            | Self::PanelIdExhausted
            | Self::Focus(_)
            | Self::ProjectNotActive
            | Self::NewTerminalTargetUnauthorized
            | Self::OpenFileTargetUnauthorized
            | Self::MissingPanel
            | Self::NoLayoutActions
            | Self::UnexpectedParameters
            | Self::Closing
            | Self::ProjectSnapshotCommitInFlight
            | Self::ProjectSnapshotRecoveryFailed
            | Self::DocumentClose(_)
            | Self::TerminalCloseRunning
            | Self::TerminalCloseState
            | Self::TerminalCloseOutcomeUnknown
            | Self::ProjectCloseRunning
            | Self::ProjectCloseState
            | Self::ProjectCloseOutcomeUnknown
            | Self::WorkspaceEpochUnavailable
            | Self::HooksDraftPending => None,
        }
    }
}

impl From<ProjectCommandError> for AppRootError {
    fn from(error: ProjectCommandError) -> Self {
        Self::Project(error)
    }
}

impl From<ProjectRegistryError> for AppRootError {
    fn from(error: ProjectRegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<ProjectSnapshotError> for AppRootError {
    fn from(error: ProjectSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<LayoutError> for AppRootError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<slterm_workspace::DocumentCloseError> for AppRootError {
    fn from(error: slterm_workspace::DocumentCloseError) -> Self {
        Self::DocumentClose(error)
    }
}

impl From<PanelIdExhausted> for AppRootError {
    fn from(_: PanelIdExhausted) -> Self {
        Self::PanelIdExhausted
    }
}

impl From<io::Error> for AppRootError {
    fn from(error: io::Error) -> Self {
        Self::Launch(error)
    }
}

impl From<FocusError> for AppRootError {
    fn from(error: FocusError) -> Self {
        Self::Focus(error)
    }
}

impl From<SidebarError> for AppRootError {
    fn from(error: SidebarError) -> Self {
        Self::Sidebar(error)
    }
}

impl AppRoot {
    /// 首帧前只恢复活跃项目；隐藏项目保留记录，首次激活时再恢复实例。
    pub(crate) fn restore_active_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let active = self.snapshot.registry().active();
        self.focus_manager.restore_active_project(active);
        if let Some(project_id) = active {
            self.restore_project_panels(project_id, window, cx);
        }
        self.sync_project_visibility(cx);
        cx.notify();
    }

    pub(super) fn restore_project_panels(
        &mut self,
        project_id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(project) = self.snapshot.registry().project(project_id) else {
            return;
        };
        // 跨机或根重定位未完成：重新授权前不启动 PTY、不读正文。
        if project.root_status() != slterm_workspace::ProjectRootStatus::Ready {
            log::info!("项目根未就绪，跳过面板恢复: {project_id:?}");
            return;
        }
        let views = project.layout().view_keys();
        for view in views {
            match view {
                ViewKey::Terminal {
                    project_id: view_project,
                    terminal_id,
                } => {
                    if view_project != project_id
                        || self.terminal_panel(project_id, terminal_id, cx).is_some()
                    {
                        continue;
                    }
                    let Some(record) = self.snapshot.terminal(terminal_id).cloned() else {
                        log::warn!("恢复布局引用的终端记录缺失");
                        continue;
                    };
                    if let Err(error) = self.restore_terminal_panel(&record, window, cx) {
                        log::warn!("恢复终端面板失败: {error}");
                    }
                }
                ViewKey::Editor { document_id, .. }
                | ViewKey::Markdown { document_id, .. }
                | ViewKey::Diff { document_id, .. } => {
                    if self.panel_for_view(project_id, &view, cx).is_ok() {
                        continue;
                    }
                    let result = if self
                        .documents
                        .session(document_id)
                        .is_some_and(|session| session.paged().is_some())
                    {
                        self.restore_recovery_panel(
                            project_id,
                            view,
                            super::document_recovery::DocumentRecoveryKind::Paged,
                            cx,
                        )
                    } else if self.documents.session(document_id).is_some() {
                        if matches!(&view, ViewKey::Diff { .. }) {
                            self.restore_diff_panel(project_id, &view, document_id, window, cx)
                        } else {
                            self.restore_document_panel(project_id, view, document_id, window, cx)
                        }
                    } else if self
                        .side_view_persistence
                        .document_requires_authorization(document_id)
                    {
                        self.restore_recovery_panel(
                            project_id,
                            view,
                            super::document_recovery::DocumentRecoveryKind::Authorization,
                            cx,
                        )
                    } else if self
                        .side_view_persistence
                        .document_identity_was_replaced(document_id)
                    {
                        self.restore_recovery_panel(
                            project_id,
                            view,
                            super::document_recovery::DocumentRecoveryKind::Replaced,
                            cx,
                        )
                    } else {
                        continue;
                    };
                    if let Err(error) = result {
                        log::warn!("恢复 Document 面板失败: {error}");
                    }
                }
                ViewKey::Settings { .. } => {
                    if self.panel_for_view(project_id, &view, cx).is_ok() {
                        continue;
                    }
                    let view_states = self.side_view_persistence.settings_view_states();
                    let settings_key = view.clone();
                    let result = self.install_panel(
                        super::documents::DocumentPanelTarget {
                            project_id,
                            view: view.clone(),
                            focus: false,
                            activate_project: false,
                        },
                        window,
                        cx,
                        move |_, cx| {
                            let settings = cx.new(|cx| {
                                super::settings_overlay::SettingsView::with_state(
                                    cx,
                                    settings_key,
                                    Some(view_states),
                                    false,
                                )
                            });
                            let focus = settings.read(cx).focus_handle.clone();
                            (super::panel_host::PanelContent::Settings(settings), focus)
                        },
                    );
                    if let Err(error) = result {
                        log::warn!("恢复设置面板失败: {error}");
                    }
                }
                ViewKey::GitShow {
                    ref repository_binding,
                    ref repo_path,
                    ..
                } => {
                    if self.panel_for_view(project_id, &view, cx).is_ok() {
                        continue;
                    }
                    if let Err(error) = self.open_gitshow(
                        super::git_show::GitShowOpenRequest {
                            project_id,
                            target_group: None,
                            binding: (**repository_binding).clone(),
                            path: repo_path.clone(),
                            focus: false,
                            activate_project: false,
                        },
                        window,
                        cx,
                    ) {
                        log::warn!("恢复 GitShow 面板失败: {error}");
                    }
                }
                _ => {}
            }
        }
        let default_view = self
            .snapshot
            .registry()
            .project(project_id)
            .and_then(|project| {
                let group_id = project.layout().group_ids().into_iter().next()?;
                project.layout().group(group_id).map(|group| group.active())
            })
            .cloned();
        let default_panel = match default_view {
            Some(ViewKey::Terminal { terminal_id, .. }) => self
                .terminal_panel(project_id, terminal_id, cx)
                .map(|(panel_id, _)| panel_id),
            Some(
                view @ (ViewKey::Editor { .. }
                | ViewKey::Markdown { .. }
                | ViewKey::Diff { .. }
                | ViewKey::GitShow { .. }),
            ) => self.panel_for_view(project_id, &view, cx).ok(),
            _ => None,
        };
        if let Some(project) = self.projects.get(&project_id)
            && project.read(cx).focused_panel.is_none()
        {
            project.update(cx, |project, _| project.focused_panel = default_panel);
        }
    }

    fn restore_diff_panel(
        &mut self,
        project_id: ProjectId,
        view: &ViewKey,
        document_id: slterm_document::DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let state = self
            .side_view_persistence
            .editor_view_states()
            .diff_state(view)
            .ok_or_else(|| {
                log::warn!("恢复 Diff 面板失败: 缺少固定绑定");
                AppRootError::MissingPanel
            })?;
        let fixed = state.binding().map_err(|error| {
            log::warn!("恢复 Diff 固定绑定失败: {error}");
            AppRootError::MissingPanel
        })?;
        self.open_diff(
            super::diff::DiffOpenRequest {
                project_id,
                target_group: None,
                document_id,
                binding: fixed.repository_binding().clone(),
                path: fixed.head_path().clone(),
                focus: false,
                target_line: None,
            },
            window,
            cx,
        )
    }

    fn restore_terminal_panel(
        &mut self,
        record: &TerminalRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let project_id = record.project_id();
        let terminal_id = record.id();
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;
        let panel_id = self.panel_ids.allocate()?;
        let parent = self.project_focus_token(project_id, cx)?;
        let recovery_root = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .root()
            .clone();
        let terminal = self.launcher.restore(
            panel_id,
            record,
            &self.terminals,
            &recovery_root,
            window,
            cx,
        );
        let pane_id = PaneId(panel_id.sequence());
        self.agents.register(pane_id);
        if let Some(agent) = record.agent() {
            self.agents.seed_session(pane_id, agent.clone());
        }
        terminal.update(cx, |terminal, cx| {
            terminal.attach_completion_session(Rc::new(crate::completion::Session::new(
                self.learning.clone(),
            )));
            if record.run_intent() == RunIntent::Launch {
                terminal.schedule_agent_resume(record.agent(), cx);
            }
        });
        let visible = self.snapshot.registry().active() == Some(project_id);
        let focus_registration = match self.focus_manager.register(
            &terminal.read(cx).focus_handle,
            parent,
            Some(project_id),
            visible,
        ) {
            Ok(registration) => registration,
            Err(error) => {
                self.agents.remove(pane_id);
                self.retire_terminal_worker(terminal_id, cx);
                return Err(error.into());
            }
        };
        let subscription = cx.subscribe_in(&terminal, window, move |root, _, event, window, cx| {
            root.on_terminal_event(panel_id, event, window, cx);
        });
        let cwd_fallback = terminal.read(cx).used_cwd_fallback();
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    },
                    content: PanelContent::Terminal(terminal),
                },
            );
        });
        self.panel_focus.insert(panel_id, focus_registration);
        self.panel_subscriptions.insert(panel_id, subscription);
        if cwd_fallback {
            super::events::notify_terminal_cwd_fallback(cx);
        }
        Ok(())
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn open_project(
        &mut self,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<ProjectId, AppRootError> {
        let requested = ProjectId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        let outcome = self.add_project(requested, root, window, cx)?;
        Ok(match outcome {
            AddProjectOutcome::Added(project_id) | AddProjectOutcome::Existing(project_id) => {
                project_id
            }
        })
    }

    pub(super) fn add_project(
        &mut self,
        id: ProjectId,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<AddProjectOutcome, AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let mut candidate = self.snapshot.clone();
        let outcome = candidate.registry_mut().add_project(id, root)?;
        candidate.validate()?;
        let active_id = match outcome {
            AddProjectOutcome::Added(project_id) | AddProjectOutcome::Existing(project_id) => {
                project_id
            }
        };
        let new_project = if matches!(outcome, AddProjectOutcome::Added(project_id) if project_id == id)
        {
            let project = cx.new(ProjectView::new);
            let parent = self.focus_manager.workspace_token();
            let registration = self.focus_manager.register(
                &project.read(cx).focus_handle,
                parent,
                Some(id),
                true,
            )?;
            Some((project, registration))
        } else {
            None
        };
        let preflight_token = match &new_project {
            Some((_, registration)) => registration.token(),
            None => self.project_default_focus(active_id, cx)?,
        };
        self.focus_manager
            .ensure_project_activation_allowed(active_id, preflight_token)?;
        self.restore_project_panels(active_id, window, cx);
        let activation_default =
            self.activation_default_after_restore(active_id, preflight_token, cx);
        self.sync_focus_visibility(Some(active_id), cx);
        self.focus_manager
            .activate_project(active_id, activation_default, window, cx)?;
        if let Some((project, registration)) = new_project {
            project.update(cx, |project, _| {
                project.focus_registration = Some(registration);
            });
            self.projects.insert(id, project);
        }
        self.commit_project_snapshot(candidate, cx);
        self.sync_project_visibility(cx);
        cx.notify();
        Ok(outcome)
    }

    pub(crate) fn activate_project(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let mut candidate = self.snapshot.clone();
        candidate.registry_mut().activate_project(id)?;
        candidate.validate()?;
        if !self.projects.contains_key(&id) {
            return Err(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ));
        }
        let preflight_token = self.project_default_focus(id, cx)?;
        self.focus_manager
            .ensure_project_activation_allowed(id, preflight_token)?;
        self.restore_project_panels(id, window, cx);
        let activation_default = self.activation_default_after_restore(id, preflight_token, cx);
        self.sync_focus_visibility(Some(id), cx);
        self.focus_manager
            .activate_project(id, activation_default, window, cx)?;
        self.commit_project_snapshot(candidate, cx);
        self.sync_project_visibility(cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn new_terminal(
        &mut self,
        shell: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let outcome = self.execute_command(
            CommandId::NewTerminal,
            CommandSource::Application,
            CommandTarget::Focused,
            Some(CommandParameters::NewTerminal {
                shell,
                project_root: None,
                cwd: None,
            }),
            window,
            cx,
        )?;
        let CommandOutcome::TerminalPanel(panel_id) = outcome else {
            unreachable!("新建终端命令必须返回面板身份");
        };
        Ok(panel_id)
    }

    pub(super) fn new_terminal_from_request(
        &mut self,
        shell: CanonicalPath,
        project_root: Option<CanonicalPath>,
        cwd: Option<CanonicalPath>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.new_terminal_project(project_root.as_ref(), cwd.as_ref())?;
        if self.snapshot.registry().active() == Some(project_id) {
            let target_group = match self.focused_group(project_id, cx)? {
                Some(group_id) => group_id,
                None => PanelGroupId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7"),
            };
            let terminal_id =
                TerminalId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
            return self.new_terminal_with_identity(
                NewTerminalSpec {
                    project_id,
                    terminal_id,
                    target_group,
                    shell,
                    cwd,
                    custom_title: None,
                },
                window,
                cx,
            );
        }
        self.new_hidden_terminal_from_request(project_id, shell, cwd, window, cx)
    }

    fn new_terminal_project(
        &self,
        project_root: Option<&CanonicalPath>,
        cwd: Option<&CanonicalPath>,
    ) -> Result<ProjectId, AppRootError> {
        if let (Some(root), Some(cwd)) = (project_root, cwd)
            && !cwd.as_path().starts_with(root.as_path())
        {
            return Err(AppRootError::NewTerminalTargetUnauthorized);
        }
        if let Some(root) = project_root {
            return self
                .snapshot
                .registry()
                .projects()
                .iter()
                .find(|project| project.root() == root)
                .map(|project| project.id())
                .ok_or(AppRootError::NewTerminalTargetUnauthorized);
        }
        if let Some(cwd) = cwd {
            return self
                .snapshot
                .registry()
                .projects()
                .iter()
                .filter(|project| cwd.as_path().starts_with(project.root().as_path()))
                .max_by_key(|project| project.root().as_path().components().count())
                .map(|project| project.id())
                .ok_or(AppRootError::NewTerminalTargetUnauthorized);
        }
        self.active_project()
    }

    fn new_hidden_terminal_from_request(
        &mut self,
        project_id: ProjectId,
        shell: CanonicalPath,
        cwd: Option<CanonicalPath>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let mut candidate = self.snapshot.clone();
        candidate.registry_mut().activate_project(project_id)?;
        candidate.validate()?;
        let target_group = match self.hidden_target_group(project_id, cx)? {
            Some(group_id) => group_id,
            None => PanelGroupId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7"),
        };
        let terminal_id = TerminalId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        let (candidate, record) = candidate.with_new_terminal(NewTerminalSpec {
            project_id,
            terminal_id,
            target_group,
            shell,
            cwd,
            custom_title: None,
        })?;
        self.launch_candidate(candidate, record, true, window, cx)
    }

    /// 隐藏项目优先复用焦点记忆；未恢复面板时按布局顺序选择既有组。
    pub(super) fn hidden_target_group(
        &self,
        project_id: ProjectId,
        cx: &App,
    ) -> Result<Option<PanelGroupId>, AppRootError> {
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let layout = project.layout();
        if layout.tree().is_none() {
            return Ok(None);
        }
        let remembered_group = self
            .focused_panel(project_id, cx)
            .and_then(|panel_id| {
                self.projects
                    .get(&project_id)?
                    .read(cx)
                    .panels
                    .get(&panel_id)
                    .map(|panel| panel.view_key.clone())
            })
            .and_then(|view_key| {
                layout.group_ids().into_iter().find(|group_id| {
                    layout
                        .group(*group_id)
                        .is_some_and(|group| group.views().contains(&view_key))
                })
            });
        Ok(remembered_group.or_else(|| layout.group_ids().into_iter().next()))
    }

    pub(super) fn new_terminal_with_identity(
        &mut self,
        spec: NewTerminalSpec,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.ensure_project_snapshot_command_allowed()?;
        self.require_active_project(spec.project_id)?;
        let (candidate, record) = self.snapshot.with_new_terminal(spec)?;
        self.launch_candidate(candidate, record, false, window, cx)
    }

    pub(super) fn split_terminal_from_request(
        &mut self,
        direction: SplitDirection,
        ratio: f32,
        viewport: LogicalSize,
        shell: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let target_group = self
            .focused_group(project_id, cx)?
            .ok_or(AppRootError::MissingPanel)?;
        let terminal_id = TerminalId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        let new_group_id =
            PanelGroupId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        self.split_terminal_with_identity(
            SplitTerminalSpec {
                new_terminal: NewTerminalSpec {
                    project_id,
                    terminal_id,
                    target_group,
                    shell,
                    cwd: None,
                    custom_title: None,
                },
                new_group_id,
                direction,
                ratio,
                viewport,
            },
            window,
            cx,
        )
    }

    pub(super) fn split_terminal_with_identity(
        &mut self,
        spec: SplitTerminalSpec,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.ensure_project_snapshot_command_allowed()?;
        self.require_active_project(spec.new_terminal.project_id)?;
        let (candidate, record) = self.snapshot.with_split_terminal(spec)?;
        self.launch_candidate(candidate, record, false, window, cx)
    }

    pub(super) fn ensure_not_closing(&self) -> Result<(), AppRootError> {
        if self.close_in_progress || self.application_close.is_some() {
            Err(AppRootError::Closing)
        } else {
            Ok(())
        }
    }

    pub(super) fn terminal_close_store_epoch(&self) -> Result<Option<String>, AppRootError> {
        if self.project_writer.is_none() {
            return Ok(None);
        }
        self.store_epoch
            .clone()
            .filter(|epoch| !epoch.is_empty())
            .map(Some)
            .ok_or(AppRootError::WorkspaceEpochUnavailable)
    }

    pub(super) fn ensure_project_snapshot_command_allowed(&self) -> Result<(), AppRootError> {
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
            Err(AppRootError::ProjectSnapshotCommitInFlight)
        } else {
            Ok(())
        }
    }

    /// 项目快照命令与局部关闭共享启动恢复屏障，但显式目录打开不受此限制。
    pub(super) fn ensure_project_snapshot_recovery_allowed(&self) -> Result<(), AppRootError> {
        if self.project_snapshot_recovery_failed {
            Err(AppRootError::ProjectSnapshotRecoveryFailed)
        } else {
            Ok(())
        }
    }

    pub(super) fn active_project(&self) -> Result<ProjectId, AppRootError> {
        self.snapshot
            .registry()
            .active()
            .ok_or(AppRootError::ProjectNotActive)
    }

    /// 非空布局以项目聚焦面板定位目标组；空布局由调用方生成首组身份。
    pub(super) fn focused_group(
        &self,
        project_id: ProjectId,
        cx: &App,
    ) -> Result<Option<PanelGroupId>, AppRootError> {
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let layout = project.layout();
        if layout.tree().is_none() {
            return Ok(None);
        }
        let panel_id = self
            .focused_panel(project_id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let view_key = self
            .projects
            .get(&project_id)
            .and_then(|project| {
                project
                    .read(cx)
                    .panels
                    .get(&panel_id)
                    .map(|panel| panel.view_key.clone())
            })
            .ok_or(AppRootError::MissingPanel)?;
        layout
            .group_ids()
            .into_iter()
            .find(|group_id| {
                layout
                    .group(*group_id)
                    .is_some_and(|group| group.views().contains(&view_key))
            })
            .map(Some)
            .ok_or(AppRootError::MissingPanel)
    }

    fn require_active_project(&self, project_id: ProjectId) -> Result<(), AppRootError> {
        if self.snapshot.registry().active() == Some(project_id) {
            Ok(())
        } else {
            Err(AppRootError::ProjectNotActive)
        }
    }

    fn retire_terminal_worker(&mut self, terminal_id: TerminalId, cx: &mut Context<'_, Self>) {
        let Some(target) = self.terminals.borrow().target(terminal_id) else {
            return;
        };
        let Some(worker) = take_terminal_worker(&self.terminals, terminal_id) else {
            return;
        };
        self.reclaim_terminal_worker(target, worker, cx);
    }

    /// 按已退休的完整目标回收；连续重连的不同代次拥有不同任务身份。
    fn reclaim_terminal_worker(
        &mut self,
        target: TerminalTarget,
        worker: crate::gpui_terminal::session::TerminalSession,
        cx: &mut Context<'_, Self>,
    ) {
        let task_id = TaskId::TerminalReclaim(target);
        debug_assert!(
            !self.tasks.is_registered(task_id),
            "已退休代次的回收任务不得重复"
        );
        self.tasks
            .register_with(task_id, CancellationToken::new(), || {
                let background = cx.background_executor().spawn(async move {
                    if worker.shutdown().is_err() {
                        log::warn!("终端 worker 回收线程异常终止");
                    }
                });
                cx.spawn(async move |root, cx| {
                    background.await;
                    let _ = root.update(cx, |root, _| root.tasks.complete(task_id));
                })
            })
            .expect("回收任务预检后必须可登记");
    }

    /// 启动、项目激活、实例登记与实际聚焦全部成功后才提交候选快照。
    fn launch_candidate(
        &mut self,
        candidate: ProjectSnapshot,
        record: TerminalRecord,
        activate_project: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let project_id = record.project_id();
        let terminal_id = record.id();
        if self.terminal_panel(project_id, terminal_id, cx).is_some() {
            return Err(AppRootError::MissingPanel);
        }
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;
        let panel_id = self.panel_ids.allocate()?;
        let parent = self.project_focus_token(project_id, cx)?;
        let terminal = self
            .launcher
            .launch(panel_id, &record, &self.terminals, window, cx)?;
        let pane_id = PaneId(panel_id.sequence());
        self.agents.register(pane_id);
        terminal.update(cx, |terminal, _| {
            terminal.attach_completion_session(Rc::new(crate::completion::Session::new(
                self.learning.clone(),
            )));
        });
        let focus_registration = match self.focus_manager.register(
            &terminal.read(cx).focus_handle,
            parent,
            Some(project_id),
            !activate_project,
        ) {
            Ok(registration) => registration,
            Err(error) => {
                self.agents.remove(pane_id);
                self.retire_terminal_worker(terminal_id, cx);
                return Err(error.into());
            }
        };
        let focus_token = focus_registration.token();
        if activate_project {
            if let Err(error) = self
                .focus_manager
                .ensure_project_activation_allowed(project_id, focus_token)
            {
                let _ = self.focus_manager.unregister(focus_token);
                self.agents.remove(pane_id);
                self.retire_terminal_worker(terminal_id, cx);
                return Err(error.into());
            }
            let previous_active = self.snapshot.registry().active();
            let previous_focus = window.focused(cx);
            self.sync_focus_visibility(Some(project_id), cx);
            let focused = self
                .focus_manager
                .present_nonmodal(focus_token)
                .and_then(|_| {
                    self.focus_manager
                        .activate_project(project_id, focus_token, window, cx)
                })
                .and_then(|_| self.focus_manager.request_focus(focus_token, window, cx));
            if let Err(error) = focused {
                self.restore_focus_after_failed_activation(
                    previous_active,
                    previous_focus,
                    window,
                    cx,
                );
                let _ = self.focus_manager.unregister(focus_token);
                self.agents.remove(pane_id);
                self.retire_terminal_worker(terminal_id, cx);
                return Err(error.into());
            }
        } else if let Err(error) = self.focus_manager.request_focus(focus_token, window, cx) {
            let _ = self.focus_manager.unregister(focus_token);
            self.agents.remove(pane_id);
            self.retire_terminal_worker(terminal_id, cx);
            return Err(error.into());
        }
        let subscription = cx.subscribe_in(&terminal, window, move |root, _, event, window, cx| {
            root.on_terminal_event(panel_id, event, window, cx);
        });
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    },
                    content: PanelContent::Terminal(terminal),
                },
            );
            project.focused_panel = Some(panel_id);
        });
        self.panel_focus.insert(panel_id, focus_registration);
        self.panel_subscriptions.insert(panel_id, subscription);
        self.commit_project_snapshot(candidate, cx);
        if activate_project {
            // 新终端已原子提交；其余隐藏记录按既有恢复语义补齐实例。
            self.restore_project_panels(project_id, window, cx);
        }
        self.sync_project_visibility(cx);
        cx.notify();
        Ok(panel_id)
    }

    pub(super) fn restore_focus_after_failed_activation(
        &mut self,
        active: Option<ProjectId>,
        focused: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.sync_focus_visibility(active, cx);
        self.focus_manager.restore_active_project(active);
        if let Some(focused) = focused {
            focused.focus(window, cx);
        } else {
            window.blur();
        }
        self.focus_manager.sync_window_focus(window, cx);
    }

    /// 只读真实 GPUI 焦点，不推进投影、代次或普通写；供关闭重试保留目标语义。
    fn actual_focused_terminal(&self, window: &Window, cx: &App) -> Option<TerminalId> {
        let focused = window.focused(cx)?;
        self.projects.values().find_map(|project| {
            project.read(cx).panels.values().find_map(|panel| {
                let terminal = panel.terminal()?;
                if terminal.read(cx).focus_handle != focused {
                    return None;
                }
                match panel.view_key {
                    ViewKey::Terminal { terminal_id, .. } => Some(terminal_id),
                    _ => None,
                }
            })
        })
    }

    /// 失败关闭直到 Completed 都占据用户重试队列；重试中的 Running 仍阻止后续目标。
    pub(super) fn oldest_unresolved_terminal_close_failure(&self) -> Option<TerminalId> {
        let single = self
            .terminal_closes
            .iter()
            .filter(|(_, close)| close.unresolved_failure)
            .map(|(id, close)| (close.initiation_sequence, *id));
        single
            .chain(self.oldest_unresolved_batch_failure())
            .min_by_key(|(sequence, _)| *sequence)
            .map(|(_, id)| id)
    }

    pub(super) fn reconnect_focused_terminal_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_recovery_allowed()?;
        self.ensure_project_snapshot_command_allowed()?;
        let terminal_id = self
            .actual_focused_terminal(window, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let record = self
            .snapshot
            .terminal(terminal_id)
            .cloned()
            .ok_or(AppRootError::MissingPanel)?;
        if record.run_intent() != RunIntent::Exited {
            return Err(AppRootError::TerminalReconnect(
                RegistryError::ProcessStillRunning,
            ));
        }
        // 在产生新 PTY 前完成全部快照与目标预检。
        let candidate = self
            .snapshot
            .with_terminal_run_intent(terminal_id, RunIntent::Launch)?;
        let project_id = record.project_id();
        let recovery_root = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .root()
            .clone();
        let (panel_id, view) = self
            .terminal_panel(project_id, terminal_id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let previous_target = self.terminals.borrow().target(terminal_id);
        let window_size = view.read(cx).pty_window_size();
        let term_config = cx
            .try_global::<crate::app_shell::config::Settings>()
            .map_or_else(slterm_terminal::term::Config::default, |settings| {
                settings.term_config()
            });
        let mut exec_context = None;
        let mut cwd_fallback = false;
        let replacement = if previous_target.is_some() {
            let launcher = &mut self.launcher;
            self.terminals
                .borrow_mut()
                .reconnect(terminal_id, |terminal| {
                    let launched = launcher.reconnect(
                        panel_id,
                        &record,
                        &recovery_root,
                        Some(terminal),
                        window_size,
                        term_config.clone(),
                    )?;
                    exec_context = launched.exec_context;
                    cwd_fallback = launched.cwd_fallback;
                    Ok((launched.session, launched.events))
                })
                .map(|replacement| {
                    let (target, previous, events, frame) = replacement.into_parts();
                    (target, Some(previous), events, frame)
                })
        } else {
            // 持久化 Exited 没有旧 worker：仍走同一 Registry 容量与启动闸门，
            // 但不伪造回滚或要求一个不存在的旧代次。
            let launcher = &mut self.launcher;
            let mut launched_events = None;
            let launched = self.terminals.borrow_mut().launch(terminal_id, || {
                let launched = launcher.reconnect(
                    panel_id,
                    &record,
                    &recovery_root,
                    None,
                    window_size,
                    term_config,
                )?;
                exec_context = launched.exec_context;
                cwd_fallback = launched.cwd_fallback;
                launched_events = Some(launched.events);
                Ok(launched.session)
            });
            launched.map(|target| {
                let events = launched_events.expect("成功启动必须返回事件流");
                let frame = self
                    .terminals
                    .borrow()
                    .capture_frame(target, 0)
                    .expect("刚登记的 worker 必须可捕获首帧");
                (target, None, events, frame)
            })
        };
        let (target, previous, events, frame) = match replacement {
            Ok(replacement) => replacement,
            Err(error) => {
                view.update(cx, |view, cx| {
                    view.reconnect_failed(format!("终端重连失败: {error}"), cx);
                });
                return Err(AppRootError::TerminalReconnect(error));
            }
        };
        view.update(cx, |view, cx| {
            view.install_reconnect(
                &self.terminals,
                target,
                crate::gpui_terminal::view::ReconnectView {
                    events,
                    frame,
                    exec_context,
                    cwd_fallback,
                },
                cx,
            );
            view.schedule_agent_resume(record.agent(), cx);
        });
        if let (Some(previous_target), Some(previous)) = (previous_target, previous) {
            self.reclaim_terminal_worker(previous_target, previous, cx);
        }
        if cwd_fallback {
            super::events::notify_terminal_cwd_fallback(cx);
        }
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn close_focused_terminal_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        self.ensure_not_closing()?;
        if let Some(terminal_id) = self.oldest_unresolved_terminal_close_failure() {
            return self.close_terminal(terminal_id, window, cx);
        }
        if let Some(terminal_id) = self.actual_focused_terminal(window, cx)
            && self.terminal_closes.contains_key(&terminal_id)
        {
            return self.close_terminal(terminal_id, window, cx);
        }
        self.ensure_project_snapshot_recovery_allowed()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.terminal_close_store_epoch()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let panel_id = self
            .focused_panel(project_id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let terminal_id = self
            .terminal_identity(panel_id, cx)
            .filter(|terminal_id| {
                self.snapshot
                    .terminal(*terminal_id)
                    .is_some_and(|record| record.project_id() == project_id)
            })
            .ok_or(AppRootError::MissingPanel)?;
        self.close_terminal(terminal_id, window, cx)
    }

    pub(crate) fn close_terminal(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        if let Some(batch_id) = self.terminal_close_batch_members.get(&id).copied() {
            return self.close_batch_member(batch_id, id, cx);
        }
        if self.project_writer.is_none() {
            self.close_terminal_in_memory(id, window, cx)
        } else {
            self.close_terminal_persisted(id, window, cx)
        }
    }

    pub(crate) fn activate_terminal(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let project_id = self
            .snapshot
            .terminal(id)
            .map(TerminalRecord::project_id)
            .ok_or(AppRootError::MissingPanel)?;
        self.activate_view(
            ViewKey::Terminal {
                project_id,
                terminal_id: id,
            },
            window,
            cx,
        )
    }

    pub(crate) fn activate_view(
        &mut self,
        view: ViewKey,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let candidate = self.snapshot.with_active_view(&view)?;
        let project_id = view.project_id().ok_or(AppRootError::MissingPanel)?;
        let panel_id = self.panel_for_view(project_id, &view, cx)?;
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;
        let focus_token = self
            .panel_focus
            .get(&panel_id)
            .map(super::focus_manager::FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)?;
        self.focus_manager.request_focus(focus_token, window, cx)?;
        self.commit_project_snapshot(candidate, cx);
        project.update(cx, |project, _| project.focused_panel = Some(panel_id));
        cx.notify();
        Ok(())
    }

    pub(crate) fn focus_notification_pane(
        &mut self,
        pane: PaneId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (project_id, terminal_id) = self
            .projects
            .iter()
            .find_map(|(project_id, project)| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .find(|panel| panel.id.sequence() == pane.0)
                    .and_then(|panel| match panel.view_key {
                        ViewKey::Terminal { terminal_id, .. } => Some(terminal_id),
                        _ => None,
                    })
                    .map(|terminal_id| (*project_id, terminal_id))
            })
            .ok_or(AppRootError::MissingPanel)?;
        self.activate_project(project_id, window, cx)?;
        self.activate_terminal(terminal_id, window, cx)
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn panel_count(&self, project_id: ProjectId, cx: &App) -> usize {
        self.projects
            .get(&project_id)
            .map(|project| project.read(cx).panels.len())
            .unwrap_or(0)
    }

    pub(crate) fn focused_panel(&self, project_id: ProjectId, cx: &App) -> Option<PanelId> {
        self.projects
            .get(&project_id)
            .and_then(|project| project.read(cx).focused_panel)
    }

    pub(crate) fn terminal_panel(
        &self,
        project_id: ProjectId,
        terminal_id: TerminalId,
        cx: &App,
    ) -> Option<(PanelId, Entity<TerminalView>)> {
        self.projects.get(&project_id).and_then(|project| {
            project.read(cx).panels.iter().find_map(|(id, panel)| {
                if panel.view_key
                    == (ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    })
                {
                    panel.terminal().map(|terminal| (*id, terminal.clone()))
                } else {
                    None
                }
            })
        })
    }

    /// 同步输出、焦点与视图复核可见性；所有项目的会话泵与实体继续持有。
    pub(super) fn sync_project_visibility(&mut self, cx: &mut App) {
        let active = self.snapshot.registry().active();
        self.sync_focus_visibility(active, cx);
        let mut diff_refreshes = Vec::new();
        for (project_id, project) in &self.projects {
            let visible = active == Some(*project_id);
            let terminals = project
                .read(cx)
                .panels
                .values()
                .filter_map(|panel| panel.terminal().cloned())
                .collect::<Vec<_>>();
            for terminal in terminals {
                terminal.update(cx, |terminal, cx| {
                    terminal.set_output_visible(visible, cx);
                });
            }
            let diffs = project
                .read(cx)
                .panels
                .iter()
                .filter_map(|(panel_id, panel)| match &panel.content {
                    PanelContent::Diff(view) => {
                        Some((*panel_id, panel.view_key.clone(), view.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (panel_id, view_key, view) in diffs {
                let view_visible = visible
                    && self
                        .snapshot
                        .registry()
                        .project(*project_id)
                        .is_some_and(|project| {
                            let layout = project.layout();
                            layout.group_ids().into_iter().any(|group_id| {
                                layout
                                    .group(group_id)
                                    .is_some_and(|group| group.active() == &view_key)
                            })
                        });
                let owner = super::task_service::ViewTaskOwner::Panel(panel_id);
                let ticket = self.tasks.set_view_visible(owner, view_visible);
                if !view_visible {
                    self.tasks
                        .cancel(super::task_service::TaskId::DiffHead(panel_id));
                }
                if let Some(ticket) = ticket {
                    diff_refreshes.push((panel_id, view, ticket));
                }
            }
        }
        for (panel_id, view, ticket) in diff_refreshes {
            self.schedule_diff_head(panel_id, view, ticket, true, cx);
        }
    }

    /// 还焦前先开放目标项目节点，并关闭所有隐藏项目的焦点资格。
    pub(super) fn sync_focus_visibility(&mut self, active: Option<ProjectId>, cx: &App) {
        let mut focus_visibility = Vec::new();
        for (project_id, project) in &self.projects {
            let visible = active == Some(*project_id);
            let project = project.read(cx);
            if let Some(registration) = &project.focus_registration {
                focus_visibility.push((registration.token(), visible));
            }
            for panel_id in project.panels.keys() {
                if let Some(registration) = self.panel_focus.get(panel_id) {
                    focus_visibility.push((registration.token(), visible));
                }
            }
        }
        for ((project_id, _), divider) in &self.split_dividers {
            focus_visibility.push((divider.registration.token(), active == Some(*project_id)));
        }
        for (token, visible) in focus_visibility {
            let result = if visible {
                self.focus_manager.present_nonmodal(token)
            } else {
                self.focus_manager.set_visible(token, false)
            };
            if let Err(error) = result {
                log::warn!("焦点可见性同步失败: {error:?}");
            }
        }
    }

    pub(super) fn project_focus_token(
        &self,
        project_id: ProjectId,
        cx: &App,
    ) -> Result<FocusToken, AppRootError> {
        self.projects
            .get(&project_id)
            .and_then(|project| project.read(cx).focus_registration.as_ref())
            .map(super::focus_manager::FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)
    }

    pub(super) fn project_default_focus(
        &self,
        project_id: ProjectId,
        cx: &App,
    ) -> Result<FocusToken, AppRootError> {
        match self.focused_panel(project_id, cx) {
            Some(panel_id) => self
                .panel_focus
                .get(&panel_id)
                .map(super::focus_manager::FocusRegistration::token)
                .ok_or(AppRootError::MissingPanel),
            None => self.project_focus_token(project_id, cx),
        }
    }

    /// 恢复可能补齐布局 active 面板；重算失败时沿用已预检 token，不新增失败窗口。
    fn activation_default_after_restore(
        &self,
        project_id: ProjectId,
        preflight_token: FocusToken,
        cx: &App,
    ) -> FocusToken {
        self.project_default_focus(project_id, cx)
            .ok()
            .filter(|default| {
                self.focus_manager
                    .ensure_project_activation_allowed(project_id, *default)
                    .is_ok()
            })
            .unwrap_or(preflight_token)
    }

    /// 把真实 GPUI 焦点同步到 FocusManager、项目瞬时焦点和布局激活项。
    pub(super) fn sync_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus_manager.sync_window_focus(window, cx);
        self.sync_focus_projection(window, cx);
    }

    pub(super) fn recover_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.focus_manager.recover_focus_lost(window, cx);
        self.sync_focus_projection(window, cx);
    }

    fn sync_document_autosave_focus(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.sync_document_autosave_enabled(cx);
        let focused = window.focused(cx).map(|handle| handle.downgrade());
        let current = self.projects.values().find_map(|project| {
            project.read(cx).panels.values().find_map(|panel| {
                let document_id = match panel.view_key {
                    ViewKey::Editor { document_id, .. }
                    | ViewKey::Markdown { document_id, .. }
                    | ViewKey::Diff { document_id, .. } => document_id,
                    _ => return None,
                };
                (Some(panel.focus_handle(cx).downgrade()) == focused)
                    .then_some((document_id, panel.view_key.clone()))
            })
        });
        if current == self.focused_document_view {
            return;
        }
        if let Some((document_id, view)) = &current {
            self.document_autosave.on_focus(*document_id, view.clone());
        }
        if let Some((document_id, view)) = self.focused_document_view.take() {
            let ready = self
                .document_autosave
                .on_blur(document_id, view, &self.documents);
            for document_id in ready {
                self.save_document(document_id, cx);
            }
        }
        self.focused_document_view = current;
    }

    fn sync_focus_projection(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() {
            return;
        }
        self.sync_document_autosave_focus(window, cx);
        let focused = window.focused(cx).map(|handle| handle.downgrade());
        let Some(project_id) = self.snapshot.registry().active() else {
            return;
        };
        let focused_panel = self.projects.get(&project_id).and_then(|project| {
            project
                .read(cx)
                .panels
                .iter()
                .find_map(|(panel_id, panel)| {
                    let handle = panel.focus_handle(cx).downgrade();
                    (Some(&handle) == focused.as_ref()).then(|| (*panel_id, panel.view_key.clone()))
                })
        });
        let Some((panel_id, view)) = focused_panel else {
            return;
        };
        if self.focused_panel(project_id, cx) == Some(panel_id) {
            return;
        }
        match self.snapshot.with_active_view(&view) {
            Ok(candidate) => {
                self.commit_project_snapshot(candidate, cx);
                if let Some(project) = self.projects.get(&project_id) {
                    project.update(cx, |project, _| project.focused_panel = Some(panel_id));
                }
                cx.notify();
            }
            Err(error) => log::warn!("实际焦点同步失败: {error}"),
        }
    }

    fn terminal_identity(&self, panel_id: PanelId, cx: &App) -> Option<TerminalId> {
        self.projects.values().find_map(|project| {
            let project = project.read(cx);
            let panel = project.panels.get(&panel_id)?;
            match panel.view_key {
                ViewKey::Terminal { terminal_id, .. } => Some(terminal_id),
                _ => None,
            }
        })
    }

    #[allow(dead_code)] // 状态徽标归 Agent 展示接线；当前恢复测试直接核验仲裁事实。
    pub(crate) fn terminal_agent_status(
        &self,
        terminal_id: TerminalId,
        cx: &App,
    ) -> crate::ai_agents::AgentStatus {
        let Some(record) = self.snapshot.terminal(terminal_id) else {
            return crate::ai_agents::AgentStatus::Unknown;
        };
        self.terminal_panel(record.project_id(), terminal_id, cx)
            .and_then(|(panel_id, _)| {
                self.agents
                    .host(PaneId(panel_id.sequence()))
                    .map(|host| host.status())
            })
            .unwrap_or(crate::ai_agents::AgentStatus::Unknown)
    }

    fn sync_agent_command(&mut self, panel_id: PanelId, started: bool, cx: &mut Context<'_, Self>) {
        let Some(terminal_id) = self.terminal_identity(panel_id, cx) else {
            return;
        };
        let Some(project_id) = self
            .snapshot
            .terminal(terminal_id)
            .map(|record| record.project_id())
        else {
            return;
        };
        let pane_id = PaneId(panel_id.sequence());
        if started {
            let program = self
                .terminal_panel(project_id, terminal_id, cx)
                .and_then(|(_, terminal)| terminal.read(cx).running_program.clone());
            self.agents.command_started(pane_id, program.as_deref());
        } else {
            self.agents.command_finished(pane_id);
        }
        let agent = self.agents.session_agent(pane_id);
        let current = self
            .snapshot
            .terminal(terminal_id)
            .and_then(|record| record.agent());
        if current == agent.as_ref() {
            cx.notify();
            return;
        }
        match self.snapshot.with_terminal_agent(terminal_id, agent) {
            Ok(candidate) => {
                self.commit_project_snapshot(candidate, cx);
                cx.notify();
            }
            Err(error) => log::warn!("终端 Agent 身份提交失败: {error}"),
        }
    }

    pub(crate) fn on_terminal_event(
        &mut self,
        panel_id: PanelId,
        event: &TerminalViewEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            TerminalViewEvent::TitleChanged => cx.notify(),
            TerminalViewEvent::Exited => {
                let Some(terminal_id) = self.terminal_identity(panel_id, cx) else {
                    return;
                };
                self.agents.process_exited(PaneId(panel_id.sequence()));
                match self
                    .snapshot
                    .with_terminal_run_intent(terminal_id, RunIntent::Exited)
                {
                    Ok(candidate) => {
                        self.commit_project_snapshot(candidate, cx);
                        cx.notify();
                    }
                    Err(error) => log::warn!("终端退出状态提交失败: {error}"),
                }
            }
            TerminalViewEvent::CommandStarted => self.sync_agent_command(panel_id, true, cx),
            TerminalViewEvent::CommandFinished => self.sync_agent_command(panel_id, false, cx),
            TerminalViewEvent::UserInput(_)
            | TerminalViewEvent::CommandSubmitted(_)
            | TerminalViewEvent::ProgressChanged(_) => {}
        }
    }
}
