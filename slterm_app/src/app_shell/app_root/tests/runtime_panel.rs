//! Runtime panel.open_file 与 panel.save：复用 Document 打开与保存链。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use slterm_runtime_protocol::{DomainErrorCode, JsonRpcError};

use super::*;

fn dispatch_runtime_mutation(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    mutation: crate::local_control::RuntimeMutation,
) -> mpsc::Receiver<Result<crate::app_shell::RuntimeMutationReply, JsonRpcError>> {
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
    response
}

fn wait_reply(
    root: &Entity<AppRoot>,
    receiver: &mpsc::Receiver<Result<crate::app_shell::RuntimeMutationReply, JsonRpcError>>,
    window: &mut VisualTestContext,
    label: &str,
) -> Result<crate::app_shell::RuntimeMutationReply, JsonRpcError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.update(|_, cx| {
            root.update(cx, |root, cx| root.poll_snapshot_replies(cx));
        });
        window.run_until_parked();
        match receiver.try_recv() {
            Ok(reply) => return reply,
            Err(mpsc::TryRecvError::Empty) => {
                assert!(Instant::now() < deadline, "{label} 回执超时");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::TryRecvError::Disconnected) => panic!("{label} 回执通道断开"),
        }
    }
}

fn expect_done(reply: Result<crate::app_shell::RuntimeMutationReply, JsonRpcError>) {
    match reply {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("必须收到完成回执"),
    }
}

#[gpui::test]
fn runtime_panel_open_file_reuses_the_document_open_chain(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("project");
    std::fs::create_dir(&repository).unwrap();
    let file = repository.join("notes.txt");
    std::fs::write(&file, "hello\n").unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());

    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelOpenFile {
            path: dunce::canonicalize(&file)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
        },
    );
    expect_done(wait_reply(&root, &receiver, &mut window, "open_file"));
    assert!(
        root.read_with(cx, |root, cx| {
            root.projects[&project_id(PROJECT_A)]
                .read(cx)
                .panels
                .values()
                .any(|panel| panel.editor().is_some())
        }),
        "打开回执后 Editor 面板必须存在"
    );

    // 相对路径在规范路径校验处拒绝。
    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelOpenFile {
            path: "notes.txt".to_owned(),
        },
    );
    let Err(error) = wait_reply(&root, &receiver, &mut window, "invalid open_file") else {
        panic!("相对路径必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    // 项目根外路径不得借 Runtime 获得打开能力。
    let outside = temp.path().join("outside.txt");
    std::fs::write(&outside, "outside\n").unwrap();
    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelOpenFile {
            path: dunce::canonicalize(&outside)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
        },
    );
    let Err(error) = wait_reply(&root, &receiver, &mut window, "unauthorized open_file") else {
        panic!("项目根外路径必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Unauthorized)
    );
}

#[gpui::test]
fn runtime_panel_save_waits_for_the_document_save_chain(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("project");
    std::fs::create_dir(&repository).unwrap();
    let file = repository.join("notes.txt");
    std::fs::write(&file, "old\n").unwrap();
    let canonical = CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, repository.to_str().unwrap());
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, canonical.clone());
    let (document_id, editor) = root.read_with(cx, |root, cx| {
        let document_id = root
            .snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .find_map(|view| match view {
                ViewKey::Editor { document_id, .. } => Some(document_id),
                _ => None,
            })
            .unwrap();
        let editor = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap();
        (document_id, editor)
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            let length = view.input_text_for_test().len();
            gpui::EntityInputHandler::replace_text_in_range(
                view,
                Some(0..length),
                "saved draft\n",
                window,
                cx,
            );
        });
    });

    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelSave {
            view: ViewKey::Editor {
                project_id: project_id(PROJECT_A),
                document_id,
            },
        },
    );
    // 回执必须等到真实保存落定：完成时磁盘与脏态都已经更新。
    expect_done(wait_reply(&root, &receiver, &mut window, "panel.save"));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "saved draft\n",
        "保存回执必须晚于真实写盘"
    );
    assert!(!root.read_with(cx, |root, _| {
        root.documents.session(document_id).unwrap().is_dirty()
    }));

    // 非文本视图不提供保存。
    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelSave {
            view: ViewKey::Settings {
                project_id: project_id(PROJECT_A),
            },
        },
    );
    let Err(error) = wait_reply(&root, &receiver, &mut window, "invalid save") else {
        panic!("非文本视图必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::InvalidRequest)
    );

    root.update(cx, |root, _| root.close_in_progress = true);
    let receiver = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelSave {
            view: ViewKey::Editor {
                project_id: project_id(PROJECT_A),
                document_id,
            },
        },
    );
    let Err(error) = wait_reply(&root, &receiver, &mut window, "closing save") else {
        panic!("关闭中必须拒绝");
    };
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
