//! 活跃项目的面板循环、关闭后的布局落点与历史归属。

use gpui::{App, Context, Window};
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelId, ProjectId, ProjectSnapshot};

/// 历史 cwd → 词法最深项目根；只比较路径组件，不访问文件系统。
///
/// 目录暂时不可访问不改变归属；空、相对或无匹配 cwd 返回 None（未归属）。
#[allow(dead_code)] // 消费点归导航历史视图。
pub(super) fn deepest_project_for_cwd<'a>(
    roots: impl Iterator<Item = (ProjectId, &'a CanonicalPath)>,
    cwd: &CanonicalPath,
) -> Option<ProjectId> {
    roots
        .filter(|(_, root)| cwd.as_path().starts_with(root.as_path()))
        .max_by_key(|(_, root)| root.as_path().components().count())
        .map(|(project, _)| project)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::deepest_project_for_cwd;
    use slterm_path::CanonicalPath;
    use slterm_workspace::ProjectId;

    fn project(text: &str) -> ProjectId {
        ProjectId::from_str(text).unwrap()
    }

    fn path(text: &str) -> CanonicalPath {
        CanonicalPath::new(text).unwrap()
    }

    #[test]
    fn navigation_history_lexical_deepest_match() {
        let shallow = project("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a");
        let deep = project("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b");
        let other = project("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2c");
        let roots = [
            (shallow, path("C:/work")),
            (deep, path("C:/work/deep")),
            (other, path("D:/other")),
        ];
        let lookup = |cwd: &CanonicalPath| {
            deepest_project_for_cwd(roots.iter().map(|(project, root)| (*project, root)), cwd)
        };
        assert_eq!(lookup(&path("C:/work/deep/x/y")), Some(deep));
        assert_eq!(lookup(&path("C:/work/file.txt")), Some(shallow));
        assert_eq!(lookup(&path("C:/work2/x")), None, "非组件前缀不得误归属");
        assert_eq!(lookup(&path("E:/elsewhere")), None);
        assert_eq!(
            lookup(&path("D:/other/missing/dir")),
            Some(other),
            "目录不可访问不改变词法归属"
        );
    }
}

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
