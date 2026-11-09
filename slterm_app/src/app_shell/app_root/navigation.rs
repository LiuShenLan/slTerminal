//! 活跃项目的面板循环与关闭后的布局落点。

use gpui::{App, Context, Window};
use slterm_workspace::{PanelId, ProjectId, ProjectSnapshot};

use super::AppRoot;
use super::commands::AppRootError;

#[derive(Clone, Copy)]
pub(super) enum PanelCycleDirection {
    Next,
    Previous,
}

impl AppRoot {
    /// 循环顺序完全来自布局的树顺序与组内顺序；激活复用既有真实面板。
    pub(super) fn cycle_panel_from_request(
        &mut self,
        direction: PanelCycleDirection,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let views = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(slterm_workspace::ProjectRegistryError::ProjectNotFound)?
            .layout()
            .view_keys();
        if views.is_empty() {
            return Err(AppRootError::MissingPanel);
        }
        let current = self.focused_panel(project_id, cx).and_then(|panel_id| {
            self.projects
                .get(&project_id)?
                .read(cx)
                .panels
                .get(&panel_id)
                .and_then(|panel| views.iter().position(|view| view == &panel.view_key))
        });
        let target = match (direction, current) {
            (PanelCycleDirection::Next, Some(index)) => (index + 1) % views.len(),
            (PanelCycleDirection::Previous, Some(0)) => views.len() - 1,
            (PanelCycleDirection::Previous, Some(index)) => index - 1,
            (PanelCycleDirection::Next, None) => 0,
            (PanelCycleDirection::Previous, None) => views.len() - 1,
        };
        self.activate_view(views[target].clone(), window, cx)
    }

    /// 同组未塌缩时采用 core 已选定的下一个／上一个；塌缩时选原相邻组 active。
    pub(super) fn preferred_close_panel(
        &self,
        project_id: ProjectId,
        closing: &[PanelId],
        candidate: &ProjectSnapshot,
        cx: &App,
    ) -> Option<PanelId> {
        let current_layout = self.snapshot.registry().project(project_id)?.layout();
        let current_groups = current_layout.group_ids();
        let current_group = closing.iter().find_map(|panel_id| {
            let view = &self
                .projects
                .get(&project_id)?
                .read(cx)
                .panels
                .get(panel_id)?
                .view_key;
            current_layout.group_ids().into_iter().find(|group_id| {
                current_layout
                    .group(*group_id)
                    .is_some_and(|group| group.views().contains(view))
            })
        })?;
        let current_index = current_groups
            .iter()
            .position(|group_id| *group_id == current_group)?;
        let candidate_layout = candidate.registry().project(project_id)?.layout();
        let target_group = if candidate_layout.group(current_group).is_some() {
            current_group
        } else {
            let remaining = candidate_layout.group_ids();
            if remaining.is_empty() {
                return None;
            }
            remaining[current_index.min(remaining.len() - 1)]
        };
        let active = candidate_layout.group(target_group)?.active();
        self.panel_for_view(project_id, active, cx)
            .ok()
            .filter(|panel_id| !closing.contains(panel_id))
    }
}
