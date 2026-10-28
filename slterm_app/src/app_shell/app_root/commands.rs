//! AppRoot 的项目与终端命令。

use std::fmt;
use std::io;
use std::rc::Rc;

use gpui::{App, AppContext as _, Context, Entity, Window};
use slterm_path::CanonicalPath;
use slterm_workspace::{
    AddProjectOutcome, NewTerminalSpec, PanelId, PanelIdExhausted, ProjectCommandError, ProjectId,
    ProjectRegistryError, ProjectSnapshot, ProjectSnapshotError, RunIntent, SplitTerminalSpec,
    TerminalId, TerminalRecord, ViewKey,
};

use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};

use super::panel_host::PanelHost;
use super::{AppRoot, ProjectView};

/// 终端面板启动适配器；测试可替换为无真实 PTY 的实现。
pub(crate) trait TerminalPanelLauncher {
    fn launch(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<Entity<TerminalView>>;
}

#[allow(dead_code)] // 由 AppRoot::new 在生产窗口切换时装配。
pub(crate) struct LocalTerminalLauncher;

impl TerminalPanelLauncher for LocalTerminalLauncher {
    fn launch(
        &mut self,
        panel_id: PanelId,
        record: &TerminalRecord,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<Entity<TerminalView>> {
        let shell = record
            .shell()
            .as_path()
            .to_str()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Shell 路径必须为有效 Unicode")
            })?
            .to_owned();
        let launch = TerminalLaunch::Local {
            cwd: Some(record.cwd().as_path().to_path_buf()),
            shell: Some(slterm_terminal::tty::Shell::new(shell, Vec::new())),
            shell_name: record.custom_title().map(str::to_owned),
            explicit_command: false,
        };
        let launched = TerminalView::launch(
            PaneId(panel_id.sequence()),
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
}

/// 项目运行命令错误；失败时正式快照与实例表保持不变。
#[derive(Debug)]
pub(crate) enum AppRootError {
    Project(ProjectCommandError),
    Registry(ProjectRegistryError),
    Snapshot(ProjectSnapshotError),
    PanelIdExhausted,
    Launch(io::Error),
    ProjectNotActive,
    MissingPanel,
}

impl fmt::Display for AppRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(error) => write!(formatter, "项目快照命令失败: {error}"),
            Self::Registry(error) => write!(formatter, "项目注册命令失败: {error:?}"),
            Self::Snapshot(error) => write!(formatter, "项目快照不一致: {error}"),
            Self::PanelIdExhausted => formatter.write_str("面板 ID 序列已耗尽"),
            Self::Launch(error) => write!(formatter, "终端启动失败: {error}"),
            Self::ProjectNotActive => formatter.write_str("只能在活跃项目中显式创建终端"),
            Self::MissingPanel => formatter.write_str("终端运行面板不存在"),
        }
    }
}

impl std::error::Error for AppRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(error) => Some(error),
            Self::Snapshot(error) => Some(error),
            Self::Launch(error) => Some(error),
            Self::Registry(_)
            | Self::PanelIdExhausted
            | Self::ProjectNotActive
            | Self::MissingPanel => None,
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

#[allow(dead_code)] // 独立运行根切片，生产窗口切换时移除。
impl AppRoot {
    pub(crate) fn add_project(
        &mut self,
        id: ProjectId,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<AddProjectOutcome, AppRootError> {
        let mut candidate = self.snapshot.clone();
        let outcome = candidate.registry_mut().add_project(id, root)?;
        candidate.validate()?;
        let active_id = match outcome {
            AddProjectOutcome::Added(project_id) | AddProjectOutcome::Existing(project_id) => {
                project_id
            }
        };
        if matches!(outcome, AddProjectOutcome::Added(project_id) if project_id == id) {
            self.projects.insert(id, cx.new(|_| ProjectView::default()));
        }
        self.snapshot = candidate;
        self.sync_project_visibility(cx);
        self.focus_project(active_id, window, cx);
        cx.notify();
        Ok(outcome)
    }

    pub(crate) fn activate_project(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let mut candidate = self.snapshot.clone();
        candidate.registry_mut().activate_project(id)?;
        candidate.validate()?;
        if !self.projects.contains_key(&id) {
            return Err(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ));
        }
        self.snapshot = candidate;
        self.sync_project_visibility(cx);
        self.focus_project(id, window, cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn new_terminal(
        &mut self,
        spec: NewTerminalSpec,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.require_active_project(spec.project_id)?;
        let (candidate, record) = self.snapshot.with_new_terminal(spec)?;
        self.launch_candidate(candidate, record, window, cx)
    }

    pub(crate) fn split_terminal(
        &mut self,
        spec: SplitTerminalSpec,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.require_active_project(spec.new_terminal.project_id)?;
        let (candidate, record) = self.snapshot.with_split_terminal(spec)?;
        self.launch_candidate(candidate, record, window, cx)
    }

    fn require_active_project(&self, project_id: ProjectId) -> Result<(), AppRootError> {
        if self.snapshot.registry().active() == Some(project_id) {
            Ok(())
        } else {
            Err(AppRootError::ProjectNotActive)
        }
    }

    /// 启动与实例登记全部成功后才提交候选快照。
    fn launch_candidate(
        &mut self,
        candidate: ProjectSnapshot,
        record: TerminalRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let project_id = record.project_id();
        let terminal_id = record.id();
        if self.terminal_panel(project_id, terminal_id, cx).is_some() {
            return Err(AppRootError::MissingPanel);
        }
        let panel_id = self.panel_ids.allocate()?;
        let terminal = self.launcher.launch(panel_id, &record, window, cx)?;
        terminal.update(cx, |terminal, _| {
            terminal.attach_completion_session(Rc::new(crate::completion::Session::default()));
        });
        let subscription = cx.subscribe_in(&terminal, window, move |root, _, event, window, cx| {
            root.on_terminal_event(panel_id, event, window, cx);
        });
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    },
                    terminal,
                },
            );
            project.focused_panel = Some(panel_id);
        });
        self.panel_subscriptions.insert(panel_id, subscription);
        self.snapshot = candidate;
        self.sync_project_visibility(cx);
        self.focus_project(project_id, window, cx);
        cx.notify();
        Ok(panel_id)
    }

    pub(crate) fn close_terminal(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<TerminalRecord, AppRootError> {
        let (candidate, record) = self.snapshot.without_terminal(id)?;
        let project_id = record.project_id();
        let (panel_id, _) = self
            .terminal_panel(project_id, id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;

        self.snapshot = candidate;
        self.panel_subscriptions.remove(&panel_id);
        let panel = project.update(cx, |project, _| {
            let panel = project.panels.remove(&panel_id);
            if project.focused_panel == Some(panel_id) {
                project.focused_panel = project.panels.keys().next().copied();
            }
            panel
        });
        if let Some(panel) = panel {
            panel.terminal.read(cx).shutdown();
        }
        self.focus_project(project_id, window, cx);
        cx.notify();
        Ok(record)
    }

    pub(crate) fn activate_terminal(
        &mut self,
        id: TerminalId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let candidate = self.snapshot.with_active_terminal(id)?;
        let project_id = candidate
            .terminal(id)
            .map(TerminalRecord::project_id)
            .ok_or(AppRootError::MissingPanel)?;
        let (panel_id, _) = self
            .terminal_panel(project_id, id, cx)
            .ok_or(AppRootError::MissingPanel)?;
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(AppRootError::Registry(
                ProjectRegistryError::ProjectNotFound,
            ))?;
        self.snapshot = candidate;
        project.update(cx, |project, _| project.focused_panel = Some(panel_id));
        self.focus_project(project_id, window, cx);
        cx.notify();
        Ok(())
    }

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
                (panel.view_key
                    == ViewKey::Terminal {
                        project_id,
                        terminal_id,
                    })
                .then(|| (*id, panel.terminal.clone()))
            })
        })
    }

    /// 只改变输出重绘可见性；所有项目的会话泵与实体继续持有。
    fn sync_project_visibility(&self, cx: &mut App) {
        let active = self.snapshot.registry().active();
        for (project_id, project) in &self.projects {
            let visible = active == Some(*project_id);
            let terminals: Vec<_> = project
                .read(cx)
                .panels
                .values()
                .map(|panel| panel.terminal.clone())
                .collect();
            for terminal in terminals {
                terminal.update(cx, |terminal, cx| {
                    terminal.set_output_visible(visible, cx);
                });
            }
        }
    }

    fn focus_project(&self, project_id: ProjectId, window: &mut Window, cx: &mut App) {
        if self.snapshot.registry().active() != Some(project_id) {
            return;
        }
        let Some(panel_id) = self.focused_panel(project_id, cx) else {
            return;
        };
        let focus = project_focus(self, project_id, panel_id, cx);
        if let Some(focus) = focus {
            focus.focus(window, cx);
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
                match self
                    .snapshot
                    .with_terminal_run_intent(terminal_id, RunIntent::Exited)
                {
                    Ok(candidate) => {
                        self.snapshot = candidate;
                        cx.notify();
                    }
                    Err(error) => log::warn!("终端退出状态提交失败: {error}"),
                }
            }
            TerminalViewEvent::UserInput(_)
            | TerminalViewEvent::CommandSubmitted(_)
            | TerminalViewEvent::CommandStarted
            | TerminalViewEvent::CommandFinished
            | TerminalViewEvent::ProgressChanged(_) => {}
        }
    }
}

fn project_focus(
    root: &AppRoot,
    project_id: ProjectId,
    panel_id: PanelId,
    cx: &App,
) -> Option<gpui::FocusHandle> {
    root.projects
        .get(&project_id)?
        .read(cx)
        .panels
        .get(&panel_id)
        .map(|panel| panel.terminal.read(cx).focus_handle.clone())
}
