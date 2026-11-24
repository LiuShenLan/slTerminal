//! Runtime Git 状态目标：前台只解析项目与仓库入口。

use std::sync::mpsc;

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};

use super::*;

fn dispatch_git_status(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project_id: Option<ProjectId>,
) -> Result<crate::app_shell::RuntimeGitTarget, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeGitStatus {
                    project_id,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回 Git 目标回执")
}

fn dispatch_git_diff(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    path: &str,
) -> Result<crate::app_shell::RuntimeGitDiffTarget, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeGitDiff {
                    project_id: None,
                    path: path.to_owned(),
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response
        .try_recv()
        .expect("AppRoot 必须返回 Git diff 目标回执")
}

#[gpui::test]
fn runtime_git_diff_target_validates_repository_relative_path(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);

    let Err(error) = dispatch_git_diff(&root, &mut window, "src/main.rs") else {
        panic!("无活跃项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let target = dispatch_git_diff(&root, &mut window, "src/main.rs").expect("相对路径必须解析");
    assert_eq!(target.project_id, project_id(PROJECT_A));
    assert_eq!(target.path, "src/main.rs");

    for invalid in [
        "",
        " ",
        "../escape.txt",
        "a/../b.txt",
        "/absolute",
        "a\\\\b",
        "./a",
    ] {
        let Err(error) = dispatch_git_diff(&root, &mut window, invalid) else {
            panic!("非法仓库相对路径必须拒绝: {invalid:?}");
        };
        assert_eq!(
            error.data.as_ref().map(|data| data.code),
            Some(DomainErrorCode::InvalidRequest),
            "{invalid:?}"
        );
    }
}

#[gpui::test]
fn runtime_git_target_resolves_project_root_and_rejects_unknown(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);

    let Err(error) = dispatch_git_status(&root, &mut window, None) else {
        panic!("无活跃项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let target =
        dispatch_git_status(&root, &mut window, None).expect("活跃项目必须解析出 Git 目标");
    assert_eq!(target.project_id, project_id(PROJECT_A));
    assert_eq!(target.root, path("D:/work/a"));

    let target = dispatch_git_status(&root, &mut window, Some(project_id(PROJECT_A)))
        .expect("显式项目必须解析");
    assert_eq!(target.project_id, project_id(PROJECT_A));

    let Err(error) = dispatch_git_status(&root, &mut window, Some(project_id(PROJECT_B))) else {
        panic!("未知项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let Err(error) = dispatch_git_status(&root, &mut window, None) else {
        panic!("关闭中必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}

fn dispatch_git_write(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    mutation: crate::local_control::RuntimeMutation,
) -> Result<crate::app_shell::RuntimeMutationReply, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeMutation {
                    mutation,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回写意图回执")
}

#[gpui::test]
fn runtime_git_write_target_resolves_project_and_validates_path(cx: &mut TestAppContext) {
    use crate::app_shell::RuntimeMutationReply;
    use crate::local_control::RuntimeMutation;
    use slterm_runtime_protocol::{GitDeleteExpected, GitRollbackExpected};

    let (root, mut window, _state) = open_root(cx);
    let rollback = |project_id, path: &str| RuntimeMutation::GitRollback {
        project_id,
        path: path.to_owned(),
        expected: GitRollbackExpected::Modified,
    };

    let Err(error) = dispatch_git_write(&root, &mut window, rollback(None, "src/main.rs")) else {
        panic!("无活跃项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let reply = dispatch_git_write(&root, &mut window, rollback(None, "src/main.rs"))
        .expect("活跃项目必须解析出 Git 写目标");
    let RuntimeMutationReply::Git(target) = reply else {
        panic!("Git 写意图必须返回后台目标");
    };
    assert_eq!(target.project_id, project_id(PROJECT_A));
    assert_eq!(target.root, path("D:/work/a"));

    for invalid in ["", " ", "../escape.txt", "a/../b.txt", "/absolute", "a\\b"] {
        let Err(error) = dispatch_git_write(&root, &mut window, rollback(None, invalid)) else {
            panic!("非法仓库相对路径必须拒绝: {invalid:?}");
        };
        assert_eq!(
            error.data.as_ref().map(|data| data.code),
            Some(DomainErrorCode::InvalidRequest),
            "{invalid:?}"
        );
    }

    let Err(error) = dispatch_git_write(
        &root,
        &mut window,
        RuntimeMutation::GitDelete {
            project_id: Some(project_id(PROJECT_B)),
            path: "notes.txt".to_owned(),
            expected: GitDeleteExpected::Untracked,
            permanent: false,
        },
    ) else {
        panic!("未知项目必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let Err(error) = dispatch_git_write(&root, &mut window, rollback(None, "src/main.rs")) else {
        panic!("关闭中必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}

fn dispatch_git_open(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project_id: ProjectId,
    binding: slterm_document::RepositoryBinding,
    path: CanonicalPath,
    kind: crate::app_shell::RuntimeGitOpenKind,
) -> Result<(), JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::RuntimeGitOpen {
                    project_id,
                    binding: Box::new(binding),
                    path,
                    kind,
                    completion,
                },
                window,
                cx,
            );
        });
    });
    response.try_recv().expect("AppRoot 必须返回 Git 打开回执")
}

fn capture_binding(repository: &std::path::Path) -> slterm_document::RepositoryBinding {
    let root = CanonicalPath::new(dunce::canonicalize(repository).unwrap()).unwrap();
    let grants = crate::app_shell::app_root::status_grants(&root).unwrap();
    crate::git::GitFacade::default()
        .repository_binding(&root, &grants, 0)
        .unwrap()
}

fn has_panel(
    root: &Entity<AppRoot>,
    cx: &mut TestAppContext,
    matches: impl Fn(&crate::app_shell::app_root::panel_host::PanelContent) -> bool,
) -> bool {
    root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .any(|panel| matches(&panel.content))
    })
}

#[gpui::test]
fn runtime_git_open_reuses_the_commit_open_chain(cx: &mut TestAppContext) {
    use crate::app_shell::RuntimeGitOpenKind;
    use crate::app_shell::app_root::panel_host::PanelContent;

    // 临时仓库：跟踪文件先修改，再从未暂存删除。
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    let git = |args: &[&str]| super::gitshow::git(&repository, args);
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "test@example.invalid"]);
    git(&["config", "user.name", "test"]);
    std::fs::write(repository.join("tracked.txt"), "one\n").unwrap();
    git(&["add", "--", "tracked.txt"]);
    git(&["commit", "--quiet", "-m", "base"]);
    std::fs::write(repository.join("tracked.txt"), "one\ntwo\n").unwrap();

    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let project = project_id(PROJECT_A);
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("tracked.txt")).unwrap()).unwrap();

    // 未暂存修改映射为共享 Diff。
    dispatch_git_open(
        &root,
        &mut window,
        project,
        capture_binding(&repository),
        file.clone(),
        RuntimeGitOpenKind::Diff,
    )
    .expect("未暂存修改必须打开 Diff");
    draw_frame(&mut window);
    window.run_until_parked();
    assert!(
        has_panel(&root, cx, |content| matches!(
            content,
            PanelContent::Diff(_)
        )),
        "Diff 打开链必须真正产生 Diff 面板"
    );

    // 未暂存删除映射为固定版本。
    std::fs::remove_file(repository.join("tracked.txt")).unwrap();
    dispatch_git_open(
        &root,
        &mut window,
        project,
        capture_binding(&repository),
        file.clone(),
        RuntimeGitOpenKind::Deleted,
    )
    .expect("未暂存删除必须打开固定版本");
    window.run_until_parked();
    assert!(
        has_panel(&root, cx, |content| matches!(
            content,
            PanelContent::GitShow(_)
        )),
        "删除条目必须打开 GitShow 固定版本"
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let Err(error) = dispatch_git_open(
        &root,
        &mut window,
        project,
        capture_binding(&repository),
        CanonicalPath::new(repository.join("tracked.txt")).unwrap(),
        RuntimeGitOpenKind::Deleted,
    ) else {
        panic!("关闭中必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
