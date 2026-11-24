//! Commit 视图：状态分组、共享 Document Diff 与固定版本打开。

use super::*;

use super::super::commit::CommitGroup;
use super::super::panel_host::PanelContent;

fn open_commit(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
) -> Entity<super::super::side_view::SideView> {
    window.simulate_keystrokes("ctrl-shift-g");
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
    root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Commit)
            .expect("Commit 必须已打开")
            .clone()
    })
}

fn activate_commit_row(
    commit: &Entity<super::super::side_view::SideView>,
    path: &CanonicalPath,
    window: &mut VisualTestContext,
) {
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_activate_row(path, window, cx));
    });
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
}

#[gpui::test]
fn commit_opens_shared_document_diff(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::write(repository.join("first.txt"), "changed\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "dirty draft\n",
                window,
                cx,
            );
        });
    });

    let commit = open_commit(&root, &mut window, cx);
    activate_commit_row(&commit, &file, &mut window);

    let diff = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .expect("modified 行必须打开 Diff")
    });
    diff.read_with(cx, |view, app| {
        assert_eq!(
            view.shared_identity_for_test(),
            document,
            "必须复用共享 Document"
        );
        assert_eq!(view.shared_text_for_test(app), "dirty draft\n");
        assert_eq!(view.head_bytes_for_test().as_ref(), b"fixed first\n");
    });
    assert_eq!(
        root.read_with(cx, |root, _| root.documents.len()),
        1,
        "不得为 Diff 另建第二份 Document"
    );
}

#[gpui::test]
fn commit_gitshow_fixed_version(cx: &mut TestAppContext) {
    let (_temp, repository, _binding) = super::gitshow::repository();
    std::fs::remove_file(repository.join("first.txt")).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let file = CanonicalPath::new(repo_root.as_path().join("first.txt")).unwrap();
    let commit = open_commit(&root, &mut window, cx);
    activate_commit_row(&commit, &file, &mut window);

    let gitshow = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                PanelContent::GitShow(view) => Some(view.clone()),
                _ => None,
            })
            .expect("deleted 行必须打开 GitShow")
    });
    gitshow.read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot().unwrap().bytes.as_ref(),
            b"fixed first\n",
            "固定版本必须来自创建时的 HEAD"
        );
    });

    // HEAD 前进后固定面板不得改写为工作区或新提交内容。
    std::fs::write(repository.join("second.txt"), "second moved\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "second.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "move head"]);
    activate_commit_row(&commit, &file, &mut window);
    gitshow.read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot().unwrap().bytes.as_ref(),
            b"fixed first\n",
            "HEAD 移动不得改变已打开固定版本"
        );
    });
}

#[gpui::test]
fn commit_view_state_restore(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (_repo_temp, repository, _binding) = super::gitshow::repository();
    for index in 0..60 {
        std::fs::write(repository.join(format!("filler-{index:02}.txt")), "x\n").unwrap();
    }
    super::gitshow::git(&repository, &["add", "--", "filler-00.txt"]);
    for name in (0..60).map(|index| format!("filler-{index:02}.txt")) {
        super::gitshow::git(&repository, &["add", "--", name.as_str()]);
    }
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "fillers"]);
    for index in 0..60 {
        std::fs::write(
            repository.join(format!("filler-{index:02}.txt")),
            "changed\n",
        )
        .unwrap();
    }
    std::fs::write(repository.join("first.txt"), "changed\n").unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|_, cx| {
        commit.update(cx, |view, cx| {
            view.commit_set_show_ignored(true, cx);
            view.commit_toggle_group(CommitGroup::Unversioned, cx);
            view.commit_set_filter("filler", cx);
        });
    });
    draw_frame(&mut window);
    let max_offset = commit.read_with(cx, |view, _| {
        view.commit_scroll_handle().unwrap().max_offset().y.as_f32()
    });
    window.update(|_, cx| {
        commit.update(cx, |view, _| {
            view.commit_scroll_handle()
                .unwrap()
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-40.0)));
        });
    });

    let (before_close, before_visible) = commit.read_with(cx, |view, _| {
        (
            view.commit_scroll_handle().unwrap().offset().y.as_f32(),
            view.commit_visible_row_count_for_test(),
        )
    });
    let status = super::gitshow::git(&repository, &["status", "--porcelain"]);
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(
        before_visible >= 60,
        "close_before visible={before_visible} status={status_text}"
    );
    assert!(
        (-before_close - 40.0).abs() < 1.0,
        "close_before offset={before_close} max={max_offset}"
    );
    window.simulate_keystrokes("ctrl-shift-n");
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let key = ViewKey::SideView {
        side_view_id: slterm_workspace::SideViewId::Commit,
    };
    let stored = store
        .view_state(
            &key,
            &super::super::side_view::workspace_view_state_registry(),
        )
        .unwrap()
        .expect("Commit ViewState 必须已持久化");
    assert_eq!(
        stored.state.state()["scroll"].as_f64(),
        Some(40.0),
        "checkpoint 必须写入滚动"
    );
    let commit = open_commit(&root, &mut window, cx);
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);
    let after_open = commit.read_with(cx, |view, _| {
        view.commit_scroll_handle().unwrap().offset().y.as_f32()
    });
    assert!(
        (-after_open - 40.0).abs() < 1.0,
        "after_open offset={after_open}"
    );
    commit.read_with(cx, |view, _| {
        assert!(view.commit_show_ignored(), "忽略开关必须恢复");
        assert!(
            view.commit_group_collapsed(CommitGroup::Unversioned),
            "分组折叠必须恢复"
        );
        assert!(
            (-view.commit_scroll_handle().unwrap().offset().y.as_f32() - 40.0).abs() < 1.0,
            "滚动位置必须恢复"
        );
        assert!(view.commit_filter().is_empty(), "过滤必须清除");
    });
}

fn staged_and_unstaged_repository(temp: &tempfile::TempDir) -> (std::path::PathBuf, CanonicalPath) {
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    super::gitshow::git(&repository, &["init", "--quiet"]);
    super::gitshow::git(&repository, &["config", "core.autocrlf", "false"]);
    super::gitshow::git(&repository, &["config", "user.name", "slTerminal Test"]);
    super::gitshow::git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("tracked.txt"), "base\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "tracked.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "base"]);
    std::fs::write(repository.join("tracked.txt"), "staged\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "tracked.txt"]);
    std::fs::write(repository.join("tracked.txt"), "worktree\n").unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("tracked.txt")).unwrap()).unwrap();
    (repository, file)
}

#[gpui::test]
fn commit_rollback_preserves_staged_changes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, file) = staged_and_unstaged_repository(&temp);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "staged\n",
        "回滚必须只放弃未暂存变化"
    );
    let staged = super::gitshow::git(&repository, &["diff", "--cached", "--stat"]);
    assert!(!staged.stdout.is_empty(), "staged 差异必须保留");
    let unstaged = super::gitshow::git(&repository, &["diff", "--stat"]);
    assert!(unstaged.stdout.is_empty(), "未暂存差异必须被放弃");
}

#[gpui::test]
fn commit_rollback_document_guard(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, file) = staged_and_unstaged_repository(&temp);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "dirty draft\n",
                window,
                cx,
            );
        });
    });
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "worktree\n",
        "脏 Document 未取得决定前不得执行 Git 回滚"
    );
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "dirty draft\n",
        "脏正文必须保留"
    );
    assert!(root.read_with(cx, |root, _| {
        root.documents.session(document).unwrap().is_dirty()
    }));
}

#[gpui::test]
fn commit_rechecks_state_before_rollback(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, file) = staged_and_unstaged_repository(&temp);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    // 列表快照仍显示未暂存修改；执行前外部把文件删除，形态不再是 modified。
    std::fs::remove_file(repository.join("tracked.txt")).unwrap();
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    assert!(
        !repository.join("tracked.txt").exists(),
        "写前重检发现形态变化必须中止恢复"
    );
}

#[gpui::test]
fn commit_deleted_restores_index_version(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    super::gitshow::git(&repository, &["init", "--quiet"]);
    super::gitshow::git(&repository, &["config", "core.autocrlf", "false"]);
    super::gitshow::git(&repository, &["config", "user.name", "slTerminal Test"]);
    super::gitshow::git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("gone.txt"), "indexed\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "gone.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "base"]);
    std::fs::remove_file(repository.join("gone.txt")).unwrap();
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let file = CanonicalPath::new(repo_root.as_path().join("gone.txt")).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("gone.txt")).unwrap(),
        "indexed\n",
        "删除回滚必须从索引恢复"
    );
}

#[gpui::test]
fn commit_rollback_capability_union(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, _file) = staged_and_unstaged_repository(&temp);
    let project = repository.join("nested/project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("inside.txt"), "inside\n").unwrap();
    let outside =
        CanonicalPath::new(dunce::canonicalize(repository.join("tracked.txt")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&outside, window, cx));
    });
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "worktree\n",
        "项目根外目标没有写能力，不得回滚"
    );
}
