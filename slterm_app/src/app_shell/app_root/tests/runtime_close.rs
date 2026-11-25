//! Runtime app.focus 与 app.close：非交互关闭的显式策略与决定错误。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use slterm_runtime_protocol::{CloseDocumentsPolicy, DomainErrorCode, JsonRpcError};

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
