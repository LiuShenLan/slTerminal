use std::str::FromStr as _;

use gpui::{Entity, Focusable as _, TestAppContext};
use slterm_document::{DocumentId, RepoPath, RepositoryBinding};

use super::super::diff::{DiffOpenRequest, DiffView};
use super::super::panel_host::PanelContent;
use super::*;

fn open_diff(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    document_id: DocumentId,
    binding: &RepositoryBinding,
    path: &str,
) -> PanelId {
    open_diff_at(root, window, document_id, binding, path, None)
}

fn open_diff_at(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    document_id: DocumentId,
    binding: &RepositoryBinding,
    path: &str,
    target_line: Option<usize>,
) -> PanelId {
    open_diff_in(
        root,
        window,
        DiffOpenRequest {
            project_id: project_id(PROJECT_A),
            target_group: Some(group_id(GROUP_A)),
            document_id,
            binding: binding.clone(),
            path: RepoPath::from_str(path).unwrap(),
            focus: true,
            target_line,
        },
    )
}

fn open_diff_in(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    request: DiffOpenRequest,
) -> PanelId {
    window
        .update(|window, cx| root.update(cx, |root, cx| root.open_diff(request, window, cx)))
        .unwrap()
}

fn current_binding(repository: &std::path::Path) -> RepositoryBinding {
    let root = CanonicalPath::new(dunce::canonicalize(repository).unwrap()).unwrap();
    crate::git::GitFacade::default()
        .repository_binding(&root, &[slterm_path::PathGrant::repo_root(root.clone())], 0)
        .unwrap()
}

fn persist_diff_layout(
    paths: &AppPaths,
    repository: &std::path::Path,
    document: DocumentId,
    location: slterm_path::ResolvedPath,
    binding: &RepositoryBinding,
) {
    let mut projects = slterm_workspace::ProjectRegistry::new();
    projects
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(dunce::canonicalize(repository).unwrap()).unwrap(),
        )
        .unwrap();
    let key = ViewKey::Diff {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    let layout = ProjectSnapshot::restore(projects, Vec::new())
        .unwrap()
        .with_opened_document(project_id(PROJECT_A), group_id(GROUP_A), document, location)
        .unwrap()
        .0
        .with_opened_view(project_id(PROJECT_A), group_id(GROUP_A), key.clone())
        .unwrap();
    let state = crate::app_shell::text_view::DiffViewState::new(slterm_document::DiffBinding::new(
        binding.clone(),
        "first.txt".parse().unwrap(),
    ));
    let registry = super::super::side_view::workspace_view_state_registry();
    let state =
        slterm_workspace::ViewState::new(&key, serde_json::to_value(state).unwrap(), &registry)
            .unwrap();
    let mut database = WorkspaceDatabase::open(paths).unwrap();
    database.save_project_snapshot(&layout).unwrap();
    database.save_view_state(&key, &state).unwrap();
}

fn assert_diff_recovery_placeholder(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    selector: &'static str,
) {
    let (panel_count, recovery_count) = root.read_with(cx, |root, cx| {
        let panels = &root.projects[&project_id(PROJECT_A)].read(cx).panels;
        (
            panels.len(),
            panels
                .values()
                .filter(|panel| matches!(&panel.content, PanelContent::Recovery(_)))
                .count(),
        )
    });
    assert_eq!((panel_count, recovery_count), (2, 2));
    assert!(window.debug_bounds(selector).is_some());
}

fn diff_view(root: &Entity<AppRoot>, panel: PanelId, cx: &TestAppContext) -> Entity<DiffView> {
    diff_view_in(root, panel, PROJECT_A, cx)
}

fn diff_view_in(
    root: &Entity<AppRoot>,
    panel: PanelId,
    project: &str,
    cx: &TestAppContext,
) -> Entity<DiffView> {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(project)]
            .read(cx)
            .panels
            .get(&panel)
            .and_then(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .unwrap()
    })
}

include!("diff/editor.rs");
include!("diff/navigation.rs");
include!("diff/lifecycle.rs");
include!("diff/recovery.rs");
