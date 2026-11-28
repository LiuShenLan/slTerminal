use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use gpui::{Entity, Focusable as _, Modifiers, TestAppContext};
use slterm_document::{RepoPath, RepositoryBinding};
use slterm_path::{CanonicalPath, PathGrant};

use super::super::diff::DiffOpenRequest;
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

/// 普通打开只复用 Editor/Markdown 身份，不得成对去重 Diff 或 GitShow 面板。
#[gpui::test]
fn ordinary_open_keeps_diff_and_gitshow_panels(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let shared =
        CanonicalPath::new(dunce::canonicalize(repository.join("shared.md")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, shared.clone());
    draw_frame(&mut window);
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == &shared).then_some(id))
            .unwrap()
    });

    window
        .update(|window, cx| {
            root.update(cx, |root, cx| {
                root.open_diff(
                    DiffOpenRequest {
                        project_id: project_id(PROJECT_A),
                        target_group: Some(group_id(GROUP_A)),
                        document_id: document,
                        binding: binding.clone(),
                        path: RepoPath::from_str("shared.md").unwrap(),
                        focus: false,
                        target_line: None,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    let gitshow = open(&root, &mut window, &binding, "shared.md");
    draw_frame(&mut window);

    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, shared);
    draw_frame(&mut window);

    let (markdown, diff, gitshow_panels, active) = root.read_with(cx, |root, cx| {
        let project = root.projects[&project_id(PROJECT_A)].read(cx);
        let mut markdown = 0;
        let mut diff = 0;
        let mut gitshow_panels = 0;
        for panel in project.panels.values() {
            match &panel.content {
                PanelContent::Markdown(_) => markdown += 1,
                PanelContent::Diff(_) => diff += 1,
                PanelContent::GitShow(_) => gitshow_panels += 1,
                _ => {}
            }
        }
        let active = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group(group_id(GROUP_A))
            .unwrap()
            .active()
            .clone();
        (markdown, diff, gitshow_panels, active)
    });
    assert_eq!(
        (markdown, diff, gitshow_panels),
        (1, 1, 1),
        "普通打开必须新增/复用 Markdown，同时保留 Diff 与 GitShow"
    );
    assert!(
        matches!(&active, ViewKey::Markdown { .. }),
        "激活必须是 Markdown"
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group(group_id(GROUP_A))
            .unwrap()
            .views()
            .iter()
            .filter(|view| matches!(view, ViewKey::GitShow { .. }))
            .count()),
        1,
        "GitShow 面板身份不得被普通打开替换: {gitshow:?}"
    );
}

/// 固定版本对象路径；测试用真实对象文件模拟不可用与恢复。
fn fixed_object(repository: &Path, path: &str) -> PathBuf {
    let output = git(repository, &["rev-parse", &format!("HEAD:{path}")]);
    let object = std::str::from_utf8(&output.stdout).unwrap().trim();
    repository
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..])
}

/// 初次读取失败必须保留面板、显示重试，并在重试时只重读同一固定版本。
#[gpui::test]
fn gitshow_initial_error_retries_the_same_fixed_version(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = repository();
    let object = fixed_object(&repository, "first.txt");
    let preserved = object.with_extension("preserved");
    std::fs::rename(&object, &preserved).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let panel = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    let retry = window
        .debug_bounds("gitshow-retry")
        .expect("初次读取失败必须渲染重试入口");
    let view = view(&root, panel, cx);
    assert!(view.read_with(cx, |view, _| view.snapshot().is_none()));

    std::fs::rename(&preserved, &object).unwrap();
    window.simulate_click(retry.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    view.read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot().unwrap().bytes.as_ref(),
            b"fixed first\n",
            "重试必须读取创建时固定版本"
        );
    });
    assert!(
        window.debug_bounds("gitshow-retry").is_none(),
        "成功重试后不得残留错误态"
    );
}

/// 手动刷新失败保留最后一次成功内容，并可从横幅重试。
#[gpui::test]
fn gitshow_refresh_failure_keeps_loaded_content_and_retries(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let panel = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    let view = view(&root, panel, cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.snapshot().unwrap().bytes.as_ref(), b"fixed first\n");
    });

    // 固定版本对象不可用后刷新：内容保留，错误横幅可见。
    let object = fixed_object(&repository, "first.txt");
    let preserved = object.with_extension("preserved");
    std::fs::rename(&object, &preserved).unwrap();
    let refresh = window
        .debug_bounds("gitshow-refresh")
        .expect("GitShow 必须提供手动刷新入口");
    window.simulate_click(refresh.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot().unwrap().bytes.as_ref(),
            b"fixed first\n",
            "刷新失败不得丢弃已加载内容"
        );
    });
    assert!(
        window.debug_bounds("gitshow-error-banner").is_some(),
        "刷新失败必须显示可见错误"
    );

    // 对象恢复后横幅重试成功。
    std::fs::rename(&preserved, &object).unwrap();
    let retry = window
        .debug_bounds("gitshow-retry")
        .expect("刷新失败横幅必须提供重试");
    window.simulate_click(retry.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("gitshow-error-banner").is_none(),
        "成功重试后必须清除错误横幅"
    );
    view.read_with(cx, |view, _| {
        assert_eq!(view.snapshot().unwrap().bytes.as_ref(), b"fixed first\n");
    });
}

/// 手动刷新只重读同一固定版本，不跟随 HEAD 前进。
#[gpui::test]
fn gitshow_refresh_reads_the_same_fixed_version(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let panel = open(&root, &mut window, &binding, "first.txt");
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    std::fs::write(repository.join("first.txt"), b"later head\n").unwrap();
    git(&repository, &["add", "--", "first.txt"]);
    git(&repository, &["commit", "--quiet", "-m", "later"]);

    let refresh = window
        .debug_bounds("gitshow-refresh")
        .expect("GitShow 必须提供手动刷新入口");
    window.simulate_click(refresh.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    view(&root, panel, cx).read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot().unwrap().bytes.as_ref(),
            b"fixed first\n",
            "刷新不得前进到新 HEAD"
        );
    });
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
