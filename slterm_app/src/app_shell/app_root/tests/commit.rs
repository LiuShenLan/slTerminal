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

/// 破坏性写必须先经唯一确认面；测试通过真实点击提交选择。
fn click_commit_confirmation(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("确认面按钮必须渲染: {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(window);
    window.run_until_parked();
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
fn integration_commit_rollback(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, file) = staged_and_unstaged_repository(&temp);
    let index_before = super::gitshow::git(&repository, &["ls-files", "-s", "--", "tracked.txt"]);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "staged\n",
        "回滚必须只放弃未暂存变化"
    );
    let staged = super::gitshow::git(&repository, &["diff", "--cached", "--stat"]);
    assert!(!staged.stdout.is_empty(), "staged 差异必须保留");
    let unstaged = super::gitshow::git(&repository, &["diff", "--stat"]);
    assert!(unstaged.stdout.is_empty(), "未暂存差异必须被放弃");
    let index_after = super::gitshow::git(&repository, &["ls-files", "-s", "--", "tracked.txt"]);
    assert_eq!(
        index_before.stdout, index_after.stdout,
        "回滚不得改变索引 blob"
    );
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
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("commit-confirm-dialog").is_some(),
        "脏 Document 必须进入唯一决定面"
    );
    assert!(
        window.debug_bounds("commit-confirm-save").is_some()
            && window.debug_bounds("commit-confirm-discard").is_some(),
        "脏 Document 必须提供保存与丢弃"
    );
    assert!(
        window.debug_bounds("commit-confirm-execute").is_none(),
        "脏 Document 不得静默执行"
    );
    click_commit_confirmation(&mut window, "commit-confirm-cancel");

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "worktree\n",
        "取消后不得执行 Git 回滚"
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
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&file, window, cx));
    });
    std::fs::remove_file(repository.join("tracked.txt")).unwrap();
    click_commit_confirmation(&mut window, "commit-confirm-execute");

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
    click_commit_confirmation(&mut window, "commit-confirm-execute");

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
    click_commit_confirmation(&mut window, "commit-confirm-execute");

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "worktree\n",
        "项目根外目标没有写能力，不得回滚"
    );
}

#[gpui::test]
fn commit_unstaged_rename_rollback(cx: &mut TestAppContext) {
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
    std::fs::write(repository.join("old.txt"), "shared\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "old.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "base"]);
    std::fs::rename(repository.join("old.txt"), repository.join("new.txt")).unwrap();
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let old = CanonicalPath::new(repo_root.as_path().join("old.txt")).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&old, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");

    assert_eq!(
        std::fs::read_to_string(repository.join("old.txt")).unwrap(),
        "shared\n",
        "旧路径必须从索引恢复"
    );
    assert!(
        !repository.join("new.txt").exists(),
        "新路径工作区条目必须清除"
    );
    let status = super::gitshow::git(&repository, &["status", "--porcelain"]);
    assert!(
        status.stdout.is_empty(),
        "索引不得改变且工作区应回到干净状态"
    );
}

/// 回滚未暂存删除时，同内容镜像只在同一父目录内才可视为改名目标。
#[gpui::test]
fn unstaged_delete_rollback_never_removes_same_content_files_outside_the_parent(
    cx: &mut TestAppContext,
) {
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
    std::fs::write(
        repository.join("old.txt"),
        "shared
",
    )
    .unwrap();
    super::gitshow::git(&repository, &["add", "--", "old.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "base"]);
    std::fs::remove_file(repository.join("old.txt")).unwrap();
    std::fs::create_dir(repository.join("sub")).unwrap();
    std::fs::write(
        repository.join("sub").join("new.txt"),
        "shared
",
    )
    .unwrap();
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let old = CanonicalPath::new(repo_root.as_path().join("old.txt")).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&old, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");

    assert_eq!(
        std::fs::read_to_string(repository.join("old.txt")).unwrap(),
        "shared
",
        "旧路径必须从索引恢复"
    );
    assert!(
        repository.join("sub").join("new.txt").exists(),
        "跨目录同内容文件不是改名目标，不得被回滚删除"
    );
}

/// 删除前必须经统一写授权：项目根外的同目录镜像不得被触碰。
#[gpui::test]
fn unstaged_delete_rollback_keeps_mirrors_outside_the_write_root(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::remove_file(repository.join("base.txt")).unwrap();
    let repo_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let base = CanonicalPath::new(repo_root.as_path().join("base.txt")).unwrap();
    // 项目根 = 仓库内的子目录；仓库根不在写授权内。
    let project = repository.join("project");
    std::fs::create_dir(&project).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, project.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_rollback(&base, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");
    assert!(
        !repository.join("base.txt").exists(),
        "项目根外目标没有写能力，不得回滚"
    );
}

struct FakeRecycleBin {
    dir: std::path::PathBuf,
    fail: bool,
}

impl crate::app_shell::file_operation_service::RecycleBin for FakeRecycleBin {
    fn send_to_recycle_bin(&self, path: &std::path::Path) -> std::io::Result<()> {
        if self.fail {
            return Err(std::io::Error::other("回收站不可用"));
        }
        std::fs::create_dir_all(&self.dir)?;
        let name = path.file_name().unwrap();
        std::fs::rename(path, self.dir.join(name))
    }
}

fn delete_repository(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let repository = temp.path().join("repo");
    std::fs::create_dir(&repository).unwrap();
    super::gitshow::git(&repository, &["init", "--quiet"]);
    super::gitshow::git(&repository, &["config", "core.autocrlf", "false"]);
    super::gitshow::git(&repository, &["config", "user.name", "slTerminal Test"]);
    super::gitshow::git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("base.txt"), "base\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "base.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "base"]);
    repository
}

#[gpui::test]
fn commit_staged_added_delete_sequence(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::write(repository.join("added.txt"), "added\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "added.txt"]);
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("added.txt")).unwrap()).unwrap();
    let recycle = temp.path().join("recycle");
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_file_operations_for_test(
            crate::app_shell::file_operation_service::FileOperationService::with_recycle_bin_for_test(
                std::sync::Arc::new(FakeRecycleBin {
                    dir: recycle.clone(),
                    fail: false,
                }),
            ),
        );
    });
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_delete_row(&file, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");

    assert!(!repository.join("added.txt").exists(), "文件必须移入回收站");
    assert!(recycle.join("added.txt").exists(), "回收站必须收到文件");
    let indexed = super::gitshow::git(&repository, &["ls-files", "--", "added.txt"]);
    assert!(indexed.stdout.is_empty(), "取消暂存必须先于删除");
    let status = super::gitshow::git(&repository, &["status", "--porcelain"]);
    assert!(status.stdout.is_empty(), "删除后工作区应干净");
}

/// 删除必须先确认；取消零修改且不留下待永久确认状态。
#[gpui::test]
fn commit_delete_requires_confirmation_and_cancel_keeps_file(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::write(repository.join("untracked.txt"), "loose\n").unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("untracked.txt")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_delete_row(&file, window, cx));
    });
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("commit-confirm-dialog").is_some(),
        "删除必须先显示确认面"
    );
    assert!(
        window.debug_bounds("commit-confirm-execute").is_some(),
        "干净目标使用执行按钮"
    );
    click_commit_confirmation(&mut window, "commit-confirm-cancel");
    assert!(
        repository.join("untracked.txt").exists(),
        "取消不得删除文件"
    );
    assert!(
        commit.read_with(cx, |view, _| view.commit_pending_permanent_path().is_none()),
        "取消不得留下永久删除确认"
    );
}

/// 脏 Document 选择“不保存并继续”：只执行已确认 Git 写，正文保留。
#[gpui::test]
fn commit_rollback_dirty_document_can_continue_without_saving(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let (repository, file) = staged_and_unstaged_repository(&temp);
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
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
    draw_frame(&mut window);
    click_commit_confirmation(&mut window, "commit-confirm-discard");
    window.run_until_parked();
    draw_frame(&mut window);
    window.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "staged\n",
        "不保存并继续必须执行回滚"
    );
    assert_eq!(
        editor.read_with(cx, |view, _| view.input_text_for_test()),
        "dirty draft\n",
        "未保存正文必须仍在编辑器中"
    );
}

/// 脏 Document 选择“保存并继续”：保存回执后才执行 Git 写。
#[gpui::test]
fn commit_rollback_dirty_document_saves_before_execution(cx: &mut TestAppContext) {
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
    draw_frame(&mut window);
    click_commit_confirmation(&mut window, "commit-confirm-save");
    for _ in 0..8 {
        window.run_until_parked();
        draw_frame(&mut window);
    }

    assert_eq!(
        std::fs::read_to_string(repository.join("tracked.txt")).unwrap(),
        "staged\n",
        "保存完成后回滚必须执行并恢复索引版本"
    );
    assert!(
        root.read_with(cx, |root, _| {
            root.documents
                .session(document)
                .is_some_and(|session| !session.is_dirty())
        }),
        "保存回执后才允许继续 Git 写，文档此时已清脏"
    );
}

/// 永久删除仍走统一计划/执行体：目录不得被递归删除，形态失配不得被
/// 当成可升级的回收站失败。
#[test]
fn permanent_delete_never_bypasses_the_shared_operation_executor() {
    use crate::app_shell::app_root::git_changes::{DeleteChangeKind, GitChangeError};
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::create_dir(repository.join("loose-dir")).unwrap();
    std::fs::write(
        repository.join("loose-dir").join("file.txt"),
        "x
",
    )
    .unwrap();
    let write_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let service = crate::app_shell::file_operation_service::FileOperationService::default();
    let result = crate::app_shell::app_root::git_changes::delete_change(
        &crate::git::GitFacade::default(),
        &write_root,
        &write_root,
        PROJECT_A.parse::<ProjectId>().unwrap(),
        &service,
        "loose-dir",
        DeleteChangeKind::Untracked,
        true,
        "test",
    );
    assert_eq!(result, Err(GitChangeError::Precondition));
    assert!(
        repository.join("loose-dir").join("file.txt").exists(),
        "永久删除不得递归移除目录"
    );
}

/// 回收站失败与形态失配必须分开归类：只有前者允许升级确认。
#[test]
fn recycle_unavailable_is_distinct_from_other_delete_failures() {
    use crate::app_shell::app_root::git_changes::{DeleteChangeKind, GitChangeError};
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::write(
        repository.join("loose.txt"),
        "loose
",
    )
    .unwrap();
    let write_root = CanonicalPath::new(dunce::canonicalize(&repository).unwrap()).unwrap();
    let service =
        crate::app_shell::file_operation_service::FileOperationService::with_recycle_bin_for_test(
            std::sync::Arc::new(FakeRecycleBin {
                dir: temp.path().join("recycle"),
                fail: true,
            }),
        );
    let error = crate::app_shell::app_root::git_changes::delete_change(
        &crate::git::GitFacade::default(),
        &write_root,
        &write_root,
        PROJECT_A.parse::<ProjectId>().unwrap(),
        &service,
        "loose.txt",
        DeleteChangeKind::Untracked,
        false,
        "test",
    )
    .unwrap_err();
    assert_eq!(error, GitChangeError::RecycleUnavailable);
    assert!(repository.join("loose.txt").exists(), "失败不得删除文件");
    let error = crate::app_shell::app_root::git_changes::delete_change(
        &crate::git::GitFacade::default(),
        &write_root,
        &write_root,
        PROJECT_A.parse::<ProjectId>().unwrap(),
        &service,
        "missing.txt",
        DeleteChangeKind::Untracked,
        false,
        "test",
    )
    .unwrap_err();
    assert_eq!(error, GitChangeError::Precondition);
}

/// 永久删除只允许由「回收站不可用」这一种失败升级；其他失败保留原因，
/// 不得借第二次确认绕过身份与授权复核。
#[test]
fn permanent_retry_is_offered_only_for_recycle_unavailable() {
    use crate::app_shell::app_root::git_changes::GitChangeError;
    let ok: Result<(), GitChangeError> = Ok(());
    assert!(!super::super::commit::offers_permanent_retry(&ok, false));
    for error in [
        GitChangeError::Precondition,
        GitChangeError::Unauthorized,
        GitChangeError::Failed,
    ] {
        assert!(
            !super::super::commit::offers_permanent_retry(&Err(error), false),
            "非回收站失败不得升级为永久删除: {error:?}"
        );
    }
    assert!(super::super::commit::offers_permanent_retry(
        &Err(GitChangeError::RecycleUnavailable),
        false
    ));
    assert!(
        !super::super::commit::offers_permanent_retry(
            &Err(GitChangeError::RecycleUnavailable),
            true
        ),
        "已是永久删除时不得再次升级"
    );
}

#[gpui::test]
fn commit_untracked_permanent_delete_requires_confirmation(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = delete_repository(&temp);
    std::fs::write(repository.join("untracked.txt"), "loose\n").unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("untracked.txt")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    root.update(cx, |root, _| {
        root.replace_file_operations_for_test(
            crate::app_shell::file_operation_service::FileOperationService::with_recycle_bin_for_test(
                std::sync::Arc::new(FakeRecycleBin {
                    dir: temp.path().join("recycle"),
                    fail: true,
                }),
            ),
        );
    });
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let commit = open_commit(&root, &mut window, cx);
    window.update(|window, cx| {
        commit.update(cx, |view, cx| view.commit_delete_row(&file, window, cx));
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");
    assert!(
        repository.join("untracked.txt").exists(),
        "回收站不可用时不得静默永久删除"
    );
    let pending = window.update(|_, cx| {
        commit
            .read(cx)
            .commit_pending_permanent_path()
            .map(|path| path.as_path().to_path_buf())
    });
    assert_eq!(
        pending.as_deref(),
        Some(file.as_path()),
        "回收站失败必须记录待永久确认目标"
    );

    window.update(|window, cx| {
        commit.update(cx, |view, cx| {
            view.commit_force_delete_row(&file, window, cx);
        });
    });
    click_commit_confirmation(&mut window, "commit-confirm-execute");
    assert!(
        !repository.join("untracked.txt").exists(),
        "显式确认后永久删除"
    );
}
