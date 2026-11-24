#[gpui::test]
fn diff_hidden_head_refresh_coalesced(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n"
    );

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    for (label, text) in [("second", "second head\n"), ("third", "third head\n")] {
        std::fs::write(repository.join("first.txt"), text).unwrap();
        super::gitshow::git(&repository, &["add", "--", "first.txt"]);
        super::gitshow::git(&repository, &["commit", "--quiet", "-m", label]);
        window.update(|_, cx| {
            root.update(cx, |root, cx| root.sync_project_visibility(cx));
        });
    }
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n",
        "隐藏期间不得刷新固定 HEAD"
    );

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
            root.sync_project_visibility(cx);
        });
    });
    draw_frame(&mut window);

    let head =
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.head_bytes_for_test().to_vec());
    let generation = root.read_with(cx, |root, _| {
        root.tasks.view_refresh_generation_for_test(
            super::super::task_service::ViewTaskOwner::Panel(panel),
        )
    });
    assert_eq!(
        (head, generation),
        (b"third head\n".to_vec(), Some(2)),
        "重新可见只合并复核一次，并采用原仓库最新 HEAD"
    );

    let editor = ViewKey::Editor {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(editor, window, cx).unwrap();
        });
    });
    std::fs::write(repository.join("first.txt"), "fourth head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "fourth"]);
    window.update(|_, cx| {
        root.update(cx, |root, cx| root.sync_project_visibility(cx));
    });
    assert_eq!(
        diff_view(&root, panel, cx)
            .read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"third head\n",
        "同组非 active 的 Diff 同样隐藏"
    );

    let diff_key = ViewKey::Diff {
        project_id: project_id(PROJECT_A),
        document_id: document,
    };
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_view(diff_key, window, cx).unwrap();
        });
    });
    draw_frame(&mut window);
    let head =
        diff_view(&root, panel, cx).read_with(cx, |view, _| view.head_bytes_for_test().to_vec());
    let generation = root.read_with(cx, |root, _| {
        root.tasks.view_refresh_generation_for_test(
            super::super::task_service::ViewTaskOwner::Panel(panel),
        )
    });
    assert_eq!((head, generation), (b"fourth head\n".to_vec(), Some(3)));
    let refreshed = diff_view(&root, panel, cx)
        .read_with(cx, |view, _| view.view_state_for_test().binding().unwrap());
    assert_eq!(
        refreshed.repository_binding(),
        &current_binding(&repository),
        "复核成功必须推进唯一 ViewState 中的提交身份"
    );
}

#[gpui::test]
fn diff_stale_head_completion_preserves_current_refresh(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    let diff = diff_view(&root, panel, cx);
    draw_frame(&mut window);

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    std::fs::write(repository.join("first.txt"), "second head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "second"]);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    let owner = super::super::task_service::ViewTaskOwner::Panel(panel);
    let stale = root.read_with(cx, |root, _| {
        root.tasks
            .current_view_refresh_ticket_for_test(owner)
            .unwrap()
    });

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_B), window, cx)
                .unwrap();
        });
    });
    std::fs::write(repository.join("first.txt"), "third head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "third"]);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    let (current, task_identity) = root.read_with(cx, |root, _| {
        (
            root.tasks
                .current_view_refresh_ticket_for_test(owner)
                .unwrap(),
            root.tasks
                .task_identity(super::super::task_service::TaskId::DiffHead(panel))
                .unwrap(),
        )
    });
    assert_ne!(stale.generation(), current.generation());

    window.update(|_, cx| {
        root.update(cx, |root, cx| {
            root.finish_diff_head(
                panel,
                diff.downgrade(),
                stale,
                Err(crate::git::GitError::Cancelled),
                cx,
            );
        });
    });
    root.read_with(cx, |root, _| {
        assert!(
            root.tasks
                .is_registered(super::super::task_service::TaskId::DiffHead(panel)),
            "stale 回执不得移除新任务"
        );
        assert_eq!(
            root.tasks
                .task_identity(super::super::task_service::TaskId::DiffHead(panel)),
            Some(task_identity)
        );
        assert_eq!(
            root.tasks.current_view_refresh_ticket_for_test(owner),
            Some(current)
        );
    });
    assert_eq!(
        diff.read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"fixed first\n",
        "stale 错误不得覆盖当前固定正文"
    );

    draw_frame(&mut window);
    assert_eq!(
        diff.read_with(cx, |view, _| view.head_bytes_for_test())
            .as_ref(),
        b"third head\n"
    );
}

#[gpui::test]
fn diff_close_cancels_refresh_and_releases_ticket(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_all_views_from_request(group_id(GROUP_A), BTreeMap::new(), window, cx)
                .unwrap();
        });
    });
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert!(
            !root
                .tasks
                .is_registered(super::super::task_service::TaskId::DiffHead(panel))
        );
        assert_eq!(
            root.tasks.view_refresh_generation_for_test(
                super::super::task_service::ViewTaskOwner::Panel(panel),
            ),
            None,
            "Panel 销毁必须释放可见性代次"
        );
    });
}

#[gpui::test]
fn diff_does_not_rebind_after_relocation(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    let panel = open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);

    add_project(
        &root,
        &mut window,
        PROJECT_B,
        _temp.path().to_str().unwrap(),
    );
    let moved = _temp.path().join("moved");
    std::fs::rename(&repository, &moved).unwrap();
    std::fs::create_dir(&repository).unwrap();
    super::gitshow::git(&repository, &["init", "--quiet"]);
    super::gitshow::git(&repository, &["config", "core.autocrlf", "false"]);
    super::gitshow::git(&repository, &["config", "user.name", "slTerminal Test"]);
    super::gitshow::git(
        &repository,
        &["config", "user.email", "slterm-test@example.invalid"],
    );
    std::fs::write(repository.join("first.txt"), "imposter head\n").unwrap();
    super::gitshow::git(&repository, &["add", "--", "first.txt"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "imposter"]);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_A), window, cx)
                .unwrap();
        });
    });
    draw_frame(&mut window);

    let diff = diff_view(&root, panel, cx);
    let (head, current_binding) = diff.read_with(cx, |view, _| {
        (
            view.head_bytes_option_for_test(),
            view.view_state_for_test().binding().unwrap(),
        )
    });
    assert_eq!(
        head.as_deref(),
        Some(b"fixed first\n".as_slice()),
        "刷新失败不得丢弃原固定快照或采用重定位路径的新仓库"
    );
    assert_eq!(current_binding.repository_binding().head(), binding.head());
}

#[gpui::test]
fn diff_open_respects_snapshot_commit_barrier(cx: &mut TestAppContext) {
    let (temp, repository, binding) = super::gitshow::repository();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file);
    let document = root.read_with(cx, |root, _| root.snapshot().documents().next().unwrap().0);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    super::close::flush_when_ready(&runtime.client());
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let database_lock = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    database_lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let accepted = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(accepted.is_ok());

    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_diff(
                DiffOpenRequest {
                    project_id: project_id(PROJECT_A),
                    target_group: Some(group_id(GROUP_A)),
                    document_id: document,
                    binding,
                    path: RepoPath::from_str("first.txt").unwrap(),
                    focus: true,
                    target_line: None,
                },
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        rejected,
        Err(super::super::commands::AppRootError::ProjectSnapshotCommitInFlight)
    ));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    database_lock.execute_batch("ROLLBACK").unwrap();
    window.run_until_parked();
    super::close::flush_when_ready(&runtime.client());
}

