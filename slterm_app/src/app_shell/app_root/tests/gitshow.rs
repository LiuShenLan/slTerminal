use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use gpui::{Entity, Focusable as _, TestAppContext};
use slterm_document::{RepoPath, RepositoryBinding};
use slterm_path::{CanonicalPath, PathGrant};

use super::super::git_show::{GitShowOpenRequest, GitShowView};
use super::super::panel_host::PanelContent;
use super::*;

pub(super) fn git(repository: &Path, args: &[&str]) -> std::process::Output {
    let output = crate::git::command(repository).args(args).output().unwrap();
    assert!(output.status.success(), "测试 Git 命令必须成功");
    output
}

fn canonical(path: &Path) -> CanonicalPath {
    CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap()
}

pub(super) fn repository() -> (tempfile::TempDir, PathBuf, RepositoryBinding) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    git(&repository, &["config", "core.autocrlf", "false"]);
    git(&repository, &["config", "user.name", "slTerminal Test"]);
    git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("first.txt"), b"fixed first\n").unwrap();
    std::fs::write(repository.join("second.txt"), b"fixed second\n").unwrap();
    std::fs::write(repository.join("shared.md"), b"# Shared\n\nbody\n").unwrap();
    git(
        &repository,
        &["add", "--", "first.txt", "second.txt", "shared.md"],
    );
    git(&repository, &["commit", "--quiet", "-m", "initial"]);
    let grants = vec![PathGrant::repo_root(canonical(&repository))];
    let binding = crate::git::GitFacade::default()
        .repository_binding(&canonical(&repository), &grants, 0)
        .unwrap();
    (temp, repository, binding)
}

fn open(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    binding: &RepositoryBinding,
    path: &str,
) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_gitshow(
                GitShowOpenRequest {
                    project_id: project_id(PROJECT_A),
                    target_group: Some(group_id(GROUP_A)),
                    binding: binding.clone(),
                    path: RepoPath::from_str(path).unwrap(),
                    focus: true,
                    activate_project: false,
                },
                window,
                cx,
            )
        })
    });
    result.unwrap()
}

fn view(root: &Entity<AppRoot>, panel: PanelId, cx: &TestAppContext) -> Entity<GitShowView> {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .get(&panel)
            .and_then(|panel| match &panel.content {
                PanelContent::GitShow(view) => Some(view.clone()),
                _ => None,
            })
            .unwrap()
    })
}

#[gpui::test]
fn gitshow_duplicate_view_focuses(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let first = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);
    let first_view = view(&root, first, cx);
    first_view.read_with(cx, |view, _| {
        assert_eq!(view.snapshot().unwrap().bytes.as_ref(), b"fixed first\n");
    });

    let second = open(&root, &mut window, &binding, "second.txt");
    draw_frame(&mut window);
    assert_ne!(first, second);
    let object = git(&repository, &["rev-parse", "HEAD:first.txt"]);
    let object = std::str::from_utf8(&object.stdout).unwrap().trim();
    let object_path = repository
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..]);
    std::fs::rename(&object_path, object_path.with_extension("preserved")).unwrap();

    let reopened = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);

    assert_eq!(reopened, first);
    assert_eq!(view(&root, first, cx), first_view);
    assert!(!first_view.read_with(cx, |view, _| view.is_error_for_test()));
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .layout()
                .group(group_id(GROUP_A))
                .unwrap()
                .views()
                .len()
        }),
        2
    );
    let focused = window.update(|window, cx| {
        let focus = first_view.read(cx).focus_handle(cx);
        window.focused(cx).is_some_and(|actual| actual == focus)
    });
    assert!(focused);
}

#[gpui::test]
fn gitshow_view_state_restore(cx: &mut TestAppContext) {
    let (temp, repository, binding) = repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let panel = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);
    let original = view(&root, panel, cx);
    window.update(|window, cx| {
        original.update(cx, |view, cx| view.set_transients_for_test(window, cx));
    });
    let key = ViewKey::GitShow {
        project_id: project_id(PROJECT_A),
        repository_binding: Box::new(binding.clone()),
        commit_id: binding.head().clone(),
        repo_path: RepoPath::from_str("first.txt").unwrap(),
    };
    let registry = super::super::side_view::workspace_view_state_registry();
    assert!(
        slterm_workspace::ViewState::new(
            &key,
            serde_json::json!({"search":"transient","scroll_line":3}),
            &registry,
        )
        .is_err()
    );
    let state = slterm_workspace::ViewState::new(&key, serde_json::json!({}), &registry).unwrap();
    runtime.client().save_view_state(&key, &state).unwrap();
    super::close::flush_when_ready(&runtime.client());

    window.update(|window, _| window.remove_window());
    drop(root);
    drop(window);
    cx.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    std::fs::write(repository.join("first.txt"), b"later head\n").unwrap();
    git(&repository, &["add", "--", "first.txt"]);
    git(&repository, &["commit", "--quiet", "-m", "later"]);

    let persisted =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persisted);
    draw_frame(&mut window);

    let restored_panel = root.read_with(cx, |root, cx| {
        root.panel_for_view(project_id(PROJECT_A), &key, cx)
            .unwrap()
    });
    let restored = view(&root, restored_panel, cx);
    restored.read_with(cx, |view, _| {
        assert_eq!(view.snapshot().unwrap().bytes.as_ref(), b"fixed first\n");
    });
    assert_eq!(
        restored.read_with(cx, |view, app| view.transients_for_test(app)),
        (0, 0..0)
    );
    let focused = window.update(|window, cx| {
        let focus = restored.read(cx).focus_handle(cx);
        window.focused(cx).is_some_and(|actual| actual == focus)
    });
    assert!(!focused);
}
