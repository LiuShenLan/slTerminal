//! 活跃项目的面板循环、关闭后的布局落点与历史归属。

use gpui::{App, Context, Window};
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelId, ProjectId, ProjectSnapshot};

/// 历史 cwd → 词法最深项目根；只比较路径组件，不访问文件系统。
///
/// 目录暂时不可访问不改变归属；空、相对或无匹配 cwd 返回 None（未归属）。
pub(super) fn deepest_project_for_cwd<'a>(
    roots: impl Iterator<Item = (ProjectId, &'a CanonicalPath)>,
    cwd: &CanonicalPath,
) -> Option<ProjectId> {
    roots
        .filter(|(_, root)| cwd.as_path().starts_with(root.as_path()))
        .max_by_key(|(_, root)| root.as_path().components().count())
        .map(|(project, _)| project)
}

/// 历史恢复的落点：保存 cwd 失效时只在已确定项目内回退项目根。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RestoreCwd {
    Saved(CanonicalPath),
    /// 保存 cwd 不可用，已回退到项目根；调用方必须提示而非静默。
    ProjectRootFallback(CanonicalPath),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RestoreCwdError {
    /// 保存 cwd 与项目根都不可用：明确失败，不改用其他目录。
    Unavailable,
}

/// 纯判定：cwd 有效用 cwd；cwd 失效且项目根有效回退项目根；否则失败。
pub(super) fn resolve_restore_cwd(
    saved: &CanonicalPath,
    saved_available: bool,
    project_root: &CanonicalPath,
    project_root_available: bool,
) -> Result<RestoreCwd, RestoreCwdError> {
    if saved_available {
        return Ok(RestoreCwd::Saved(saved.clone()));
    }
    if project_root_available {
        return Ok(RestoreCwd::ProjectRootFallback(project_root.clone()));
    }
    Err(RestoreCwdError::Unavailable)
}

/// Shell 就绪等待上限；注入只尝试一次，不因超时重复建终端。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestoreShell {
    PowerShell,
    Cmd,
    Other,
}

/// Shell 就绪等待只由真实程序名分类；未知程序不冒充 PowerShell。
pub(crate) fn restore_shell_from_program(program: Option<&str>) -> RestoreShell {
    let Some(program) = program else {
        return RestoreShell::Other;
    };
    let Some(stem) = std::path::Path::new(program).file_stem() else {
        return RestoreShell::Other;
    };
    let stem = stem.to_string_lossy();
    if stem.eq_ignore_ascii_case("cmd") {
        RestoreShell::Cmd
    } else if stem.eq_ignore_ascii_case("pwsh") || stem.eq_ignore_ascii_case("powershell") {
        RestoreShell::PowerShell
    } else {
        RestoreShell::Other
    }
}

pub(crate) fn restore_injection_timeout(shell: RestoreShell) -> std::time::Duration {
    match shell {
        RestoreShell::PowerShell | RestoreShell::Other => std::time::Duration::from_secs(10),
        RestoreShell::Cmd => std::time::Duration::from_millis(500),
    }
}

/// 历史恢复的项目落点：未归属时必须先展示目标根并取得交互确认。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RestoreProject {
    /// 已词法归属既有项目，直接复用。
    Reuse(ProjectId),
    /// 未归属：确认前不得创建项目；Runtime 无隐式创建分支。
    RequiresConfirmation(CanonicalPath),
}

pub(super) fn resolve_restore_project(
    attributed: Option<ProjectId>,
    unowned_root: &CanonicalPath,
) -> RestoreProject {
    match attributed {
        Some(project) => RestoreProject::Reuse(project),
        None => RestoreProject::RequiresConfirmation(unowned_root.clone()),
    }
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

    /// 导航瞬态不进入持久状态；未到期隔离批次重启后按原期限恢复。
    #[test]
    fn navigation_transient_and_quarantine_restore() {
        use crate::history_quarantine::{
            BatchStatus, HistoryQuarantine, PersistedBatch, QuarantineBatchId,
        };
        use slterm_sidebar::{SideViewState, side_view_state_schema};
        use slterm_workspace::{SideViewId, ViewStateRegistry};
        use std::time::{Duration, SystemTime};

        let registry = ViewStateRegistry::new(&[side_view_state_schema()]).unwrap();
        let encoded = SideViewState::new(SideViewId::Nav)
            .to_view_state(&registry)
            .unwrap();
        let keys = encoded
            .state()
            .as_object()
            .expect("持久状态必须是对象")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            keys,
            std::collections::BTreeSet::from(["projects", "scroll", "unassigned_expanded"]),
            "搜索、选中与错误等瞬态不得进入持久状态"
        );

        let id = QuarantineBatchId(uuid::Uuid::from_bytes([
            0x01, 0x9b, 0x76, 0xda, 0xa8, 0x01, 0x70, 0x00, 0x80, 0, 0, 0, 0, 0, 0, 0,
        ]));
        let completed = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let restored = HistoryQuarantine::from_persisted(&[PersistedBatch {
            id,
            status: BatchStatus::Completed,
            completed_at: Some(completed),
        }]);
        assert!(
            restored.undoable(id, completed + Duration::from_secs(4)),
            "未到期隔离批次按规则恢复为可撤销"
        );
        assert!(!restored.undoable(id, completed + Duration::from_secs(5)));
    }

    /// 未归属历史必须先确认目标根；已归属直接复用，不隐式创建项目。
    #[test]
    fn navigation_unowned_restore_requires_project_confirmation() {
        use super::{RestoreProject, resolve_restore_project};

        let project = project("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a");
        let root = path("C:/work/unknown");
        assert_eq!(
            resolve_restore_project(Some(project), &root),
            RestoreProject::Reuse(project),
            "已归属历史直接复用既有项目"
        );
        assert_eq!(
            resolve_restore_project(None, &root),
            RestoreProject::RequiresConfirmation(root.clone()),
            "未归属历史必须先确认目标根"
        );
    }

    /// cwd 失效回退项目根、项目根失效才失败；注入超时按 Shell 类型且只一次。
    #[test]
    fn navigation_restore_fallback_and_injection_retry() {
        use super::{
            RestoreCwd, RestoreCwdError, RestoreShell, resolve_restore_cwd,
            restore_injection_timeout, restore_shell_from_program,
        };
        use std::time::Duration;

        let saved = path("C:/work/agent");
        let project = path("C:/work");
        assert_eq!(
            resolve_restore_cwd(&saved, true, &project, true),
            Ok(RestoreCwd::Saved(saved.clone()))
        );
        assert_eq!(
            resolve_restore_cwd(&saved, false, &project, true),
            Ok(RestoreCwd::ProjectRootFallback(project.clone())),
            "cwd 失效必须回退已确定项目根"
        );
        assert_eq!(
            resolve_restore_cwd(&saved, false, &project, false),
            Err(RestoreCwdError::Unavailable),
            "项目根也失效时必须明确失败"
        );
        assert_eq!(
            restore_injection_timeout(RestoreShell::PowerShell),
            Duration::from_secs(10)
        );
        assert_eq!(
            restore_injection_timeout(RestoreShell::Cmd),
            Duration::from_millis(500)
        );
        assert_eq!(
            restore_injection_timeout(RestoreShell::Other),
            Duration::from_secs(10),
            "未知程序按上限等待，不得过早注入"
        );
        assert_eq!(
            restore_shell_from_program(Some("C:/Program Files/PowerShell/7/pwsh.exe")),
            RestoreShell::PowerShell
        );
        assert_eq!(
            restore_shell_from_program(Some("C:/Windows/System32/cmd.EXE")),
            RestoreShell::Cmd
        );
        assert_eq!(
            restore_shell_from_program(Some("C:/msys64/usr/bin/bash.exe")),
            RestoreShell::Other
        );
        assert_eq!(restore_shell_from_program(None), RestoreShell::Other);
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
