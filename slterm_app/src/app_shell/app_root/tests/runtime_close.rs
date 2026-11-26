//! Runtime app.focus 与 app.close：非交互关闭的显式策略与决定错误。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use slterm_runtime_protocol::{
    CloseDocumentsPolicy, CloseHooksPolicy, DomainErrorCode, JsonRpcError,
};

use super::*;

fn dispatch_runtime_mutation(
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.run_until_parked();
        match response.try_recv() {
            Ok(reply) => return reply,
            Err(mpsc::TryRecvError::Empty) => {
                assert!(Instant::now() < deadline, "写意图回执超时");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::TryRecvError::Disconnected) => panic!("回执通道断开"),
        }
    }
}

/// 插入脏 Hooks 草稿；Project／Local 层需要项目身份。
fn insert_dirty_hooks_draft(
    root: &Entity<AppRoot>,
    cx: &mut TestAppContext,
    layer: crate::ai_hook::draft_host::HooksDraftLayer,
) -> crate::ai_hook::draft_host::HooksDraftKey {
    use crate::ai_hook::draft_host::{HooksDraft, HooksDraftKey};
    let panel = PanelIdAllocator::new(91).allocate().unwrap();
    let key = HooksDraftKey {
        panel,
        agent: slterm_agents::AgentKind::Claude,
        layer,
    };
    root.update(cx, |root, _| {
        let mut draft = HooksDraft::loaded(None, "{}".to_owned(), serde_json::json!({}));
        draft.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"edited"}]}]}"#);
        root.hook_drafts.insert(key, draft);
    });
    key
}

fn dirty_document(
    cx: &mut TestAppContext,
) -> (
    tempfile::TempDir,
    Entity<AppRoot>,
    VisualTestContext,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("project");
    std::fs::create_dir(&repository).unwrap();
    let file = repository.join("notes.txt");
    std::fs::write(&file, "old\n").unwrap();
    let canonical = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, canonical);
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
                "draft\n",
                window,
                cx,
            );
        });
    });
    (temp, root, window, file)
}

#[gpui::test]
fn runtime_close_requires_decision(cx: &mut TestAppContext) {
    let (_temp, root, mut window, file) = dirty_document(cx);

    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppFocus,
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("app.focus 必须被接受"),
    }

    assert!(
        root.read_with(cx, |root, _| root.documents.ids().any(|id| {
            root.documents
                .session(id)
                .is_some_and(|session| session.is_dirty())
        })),
        "前置：必须有脏 Document"
    );
    // 脏 Document 没有策略：决定错误且不得开始关闭或写入。
    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: None,
            hooks: None,
        },
    ) else {
        panic!("缺少决定必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::DecisionRequired)
    );
    assert!(!root.read_with(cx, |root, _| root.close_in_progress));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");

    // Discard：关闭继续，磁盘保持原内容。
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: Some(CloseDocumentsPolicy::Discard),
            hooks: None,
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式策略必须被接受"),
    }
    window.run_until_parked();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");
    assert!(root.read_with(cx, |root, _| root.close_completed));
}

#[gpui::test]
fn runtime_app_close_save_policy_without_writer_is_structurally_rejected(cx: &mut TestAppContext) {
    let (_temp, root, mut window, file) = dirty_document(cx);

    // 无工作区 writer：不能证明关闭意图与保存已持久，明确 NotReady 且不关闭。
    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: Some(CloseDocumentsPolicy::Save),
            hooks: None,
        },
    ) else {
        panic!("无法保存时不得接受关闭");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::NotReady)
    );
    assert!(!root.read_with(cx, |root, _| root.close_in_progress));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "old
"
    );
}

fn editor_view(root: &Entity<AppRoot>, cx: &mut TestAppContext) -> ViewKey {
    root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .find(|view| matches!(view, ViewKey::Editor { .. }))
            .unwrap()
    })
}

fn view_exists(root: &Entity<AppRoot>, cx: &mut TestAppContext, view: &ViewKey) -> bool {
    root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .contains(view)
    })
}

#[gpui::test]
fn runtime_panel_close_requires_decision_and_honors_policies(cx: &mut TestAppContext) {
    let (_temp, root, mut window, file) = dirty_document(cx);
    let view = editor_view(&root, cx);

    // 无策略：决定错误，面板与正文保持。
    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelClose {
            view: view.clone(),
            documents: None,
        },
    ) else {
        panic!("缺少决定必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::DecisionRequired)
    );
    assert!(view_exists(&root, cx, &view));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");

    // Discard：关闭面板且不写盘。
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelClose {
            view: view.clone(),
            documents: Some(CloseDocumentsPolicy::Discard),
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式策略必须被接受"),
    }
    window.run_until_parked();
    assert!(!view_exists(&root, cx, &view));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");
}

#[gpui::test]
fn runtime_panel_close_save_policy_persists_before_destroying(cx: &mut TestAppContext) {
    let (_temp, root, mut window, file) = dirty_document(cx);
    let view = editor_view(&root, cx);

    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelClose {
            view: view.clone(),
            documents: Some(CloseDocumentsPolicy::Save),
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式保存策略必须被接受"),
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while view_exists(&root, cx, &view) {
        assert!(Instant::now() < deadline, "保存后关闭必须完成");
        window.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "draft\n");
}

fn root_with_writer(
    cx: &mut TestAppContext,
) -> (
    tempfile::TempDir,
    AppPaths,
    WorkspaceWriterRuntime,
    Entity<AppRoot>,
    VisualTestContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let (events, _event_rx) = std::sync::mpsc::sync_channel(64);
    let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
    let persistence =
        AppRootPersistence::load_with_previous_run(&paths, Some(runtime.client()), None);
    let (root, window, _state) = open_root_with_persistence(cx, persistence);
    (temp, paths, runtime, root, window)
}

fn wait_project_removed(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &mut TestAppContext,
    project: ProjectId,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let (present, state) = root.read_with(cx, |root, _| {
            (
                root.snapshot.registry().project(project).is_some(),
                root.project_closes.get(&project).map(|close| {
                    (
                        format!("{:?}", close.coordinator.phase()),
                        close.persisted.commit_status,
                        close.outcome_unknown,
                    )
                }),
            )
        });
        if !present && state.is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "项目移除必须完成: present={present} state={state:?}"
        );
        if !cx.executor().tick() {
            std::thread::sleep(Duration::from_millis(5));
        }
        window.run_until_parked();
    }
}

#[gpui::test]
fn runtime_project_remove_requires_document_decision(cx: &mut TestAppContext) {
    let (_temp, root, mut window, file) = dirty_document(cx);

    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::ProjectRemove {
            project_id: project_id(PROJECT_A),
            documents: None,
            hooks: None,
        },
    ) else {
        panic!("缺少决定必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::DecisionRequired)
    );
    assert!(root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .is_some()
    }));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");
}

#[gpui::test]
fn runtime_project_remove_honors_discard_and_save_policies(cx: &mut TestAppContext) {
    let (temp, _paths, runtime, root, mut window) = root_with_writer(cx);
    let project_a = temp.path().join("project-a");
    std::fs::create_dir(&project_a).unwrap();
    let discard_file = project_a.join("discard.txt");
    std::fs::write(&discard_file, "old\n").unwrap();
    add_project(&root, &mut window, PROJECT_A, project_a.to_str().unwrap());
    let canonical = CanonicalPath::new(dunce::canonicalize(&discard_file).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, canonical);
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
                "dropped\n",
                window,
                cx,
            );
        });
    });

    super::close::flush_when_ready(&runtime.client());
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::ProjectRemove {
            project_id: project_id(PROJECT_A),
            documents: Some(CloseDocumentsPolicy::Discard),
            hooks: None,
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式策略必须被接受"),
    }
    wait_project_removed(&root, &mut window, cx, project_id(PROJECT_A));
    assert_eq!(std::fs::read_to_string(&discard_file).unwrap(), "old\n");

    // Save：先真实写盘，再移除项目。
    let project_b = temp.path().join("project-b");
    std::fs::create_dir(&project_b).unwrap();
    let save_file = project_b.join("save.txt");
    std::fs::write(&save_file, "old\n").unwrap();
    add_project(&root, &mut window, PROJECT_B, project_b.to_str().unwrap());
    let canonical = CanonicalPath::new(dunce::canonicalize(&save_file).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_B, GROUP_B, canonical);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_B)]
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
                "kept\n",
                window,
                cx,
            );
        });
    });
    super::close::flush_when_ready(&runtime.client());
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::ProjectRemove {
            project_id: project_id(PROJECT_B),
            documents: Some(CloseDocumentsPolicy::Save),
            hooks: None,
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式保存策略必须被接受"),
    }
    wait_project_removed(&root, &mut window, cx, project_id(PROJECT_B));
    assert_eq!(std::fs::read_to_string(&save_file).unwrap(), "kept\n");

    drop(root);
    cx.update(|_| {});
    runtime
        .shutdown(ControlPlaneCleanup::Succeeded)
        .expect("writer 必须正常关闭");
}

#[gpui::test]
fn integration_runtime_noninteractive_close(cx: &mut TestAppContext) {
    let (_temp, _paths, runtime, root, mut window) = root_with_writer(cx);
    let project = project_id(PROJECT_A);
    let project_root = _temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let file = project_root.join("notes.txt");
    std::fs::write(&file, "old\n").unwrap();
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );
    let canonical = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, canonical);
    let view = editor_view(&root, cx);
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project]
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
                "draft\n",
                window,
                cx,
            );
        });
    });

    // 三个非交互关闭作用域都必须先要显式决定。
    for mutation in [
        crate::local_control::RuntimeMutation::PanelClose {
            view: view.clone(),
            documents: None,
        },
        crate::local_control::RuntimeMutation::ProjectRemove {
            project_id: project,
            documents: None,
            hooks: None,
        },
        crate::local_control::RuntimeMutation::AppClose {
            documents: None,
            hooks: None,
        },
    ] {
        let Err(error) = dispatch_runtime_mutation(&root, &mut window, mutation) else {
            panic!("缺少决定必须拒绝");
        };
        assert_eq!(
            error.data.as_ref().map(|data| data.code),
            Some(DomainErrorCode::DecisionRequired)
        );
    }
    assert!(view_exists(&root, cx, &view));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");

    // 显式策略后应用退出真实推进；未保存的草稿保持原内容。
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: Some(CloseDocumentsPolicy::Discard),
            hooks: None,
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("显式策略必须被接受"),
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.read_with(cx, |root, _| root.close_completed) {
        assert!(Instant::now() < deadline, "应用关闭必须完成");
        if !cx.executor().tick() {
            std::thread::sleep(Duration::from_millis(5));
        }
        window.run_until_parked();
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "old\n");
    runtime
        .shutdown(ControlPlaneCleanup::Succeeded)
        .expect("writer 必须正常关闭");
}

/// Runtime 关闭：决定不齐备时 Hooks 草稿必须保持现场，齐备后才按策略丢弃。
#[gpui::test]
fn runtime_hooks_discard_waits_for_complete_document_decisions(cx: &mut TestAppContext) {
    use crate::ai_hook::draft_host::HooksDraftLayer;

    let (_temp, root, mut window, _file) = dirty_document(cx);
    let key = insert_dirty_hooks_draft(&root, cx, HooksDraftLayer::User);

    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: None,
            hooks: Some(CloseHooksPolicy::Discard),
        },
    ) else {
        panic!("缺少文档决定必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::DecisionRequired)
    );
    root.read_with(cx, |root, _| {
        let draft = root.hook_drafts.get(key).expect("失败的关闭不得丢弃草稿");
        assert!(draft.dirty(), "草稿必须保持未提交状态");
    });

    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::AppClose {
            documents: Some(CloseDocumentsPolicy::Discard),
            hooks: Some(CloseHooksPolicy::Discard),
        },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("决定齐备后必须接受关闭"),
    }
    assert!(
        root.read_with(cx, |root, _| root.hook_drafts.get(key).is_none()),
        "关闭登记成功后必须丢弃草稿"
    );
}

/// Runtime 项目移除：文档决定缺失时同样不得提前丢弃项目草稿。
#[gpui::test]
fn runtime_project_hooks_discard_waits_for_document_decisions(cx: &mut TestAppContext) {
    use crate::ai_hook::draft_host::HooksDraftLayer;

    let (_temp, root, mut window, _file) = dirty_document(cx);
    let key = insert_dirty_hooks_draft(&root, cx, HooksDraftLayer::Project(project_id(PROJECT_A)));

    let Err(error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::ProjectRemove {
            project_id: project_id(PROJECT_A),
            documents: None,
            hooks: Some(CloseHooksPolicy::Discard),
        },
    ) else {
        panic!("缺少文档决定必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::DecisionRequired)
    );
    assert!(
        root.read_with(cx, |root, _| root.hook_drafts.get(key).is_some()),
        "失败的移除不得丢弃草稿"
    );
}
