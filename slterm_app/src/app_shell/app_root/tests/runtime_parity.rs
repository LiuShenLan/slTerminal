//! 单实例移交与 Runtime 的授权、打开与命令同构。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use slterm_runtime_protocol::{DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError};

use super::*;

fn dispatch_handover(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    params: HandoverParams,
) -> Result<HandoverResponse, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::Handover { params, completion },
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
                assert!(Instant::now() < deadline, "handover 回执超时");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::TryRecvError::Disconnected) => panic!("handover 回执通道断开"),
        }
    }
}

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
            Err(mpsc::TryRecvError::Disconnected) => panic!("写意图回执通道断开"),
        }
    }
}

fn error_code(error: &JsonRpcError) -> Option<DomainErrorCode> {
    error.data.as_ref().map(|data| data.code)
}

/// 锚：单实例移交与 Runtime 共用同一授权与打开链，错误一致且不重复开面板。
#[gpui::test]
fn integration_single_instance_runtime_parity(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let outside_dir = temp.path().join("outside");
    std::fs::create_dir(&outside_dir).unwrap();
    let inside = project_root.join("notes.txt");
    std::fs::write(&inside, "inside\n").unwrap();
    let outside = outside_dir.join("secret.txt");
    std::fs::write(&outside, "outside\n").unwrap();

    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        project_root.to_str().unwrap(),
    );

    // 项目根外路径：handover 与 Runtime 都必须 Unauthorized，且不产生 Document。
    let outside_text = dunce::canonicalize(&outside)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let handover_error = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenFile {
            path: outside_text.clone(),
        },
    )
    .expect_err("handover 打开项目根外文件必须拒绝");
    let Err(runtime_error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelOpenFile { path: outside_text },
    ) else {
        panic!("Runtime 打开项目根外文件必须拒绝");
    };
    assert_eq!(
        error_code(&handover_error),
        Some(DomainErrorCode::Unauthorized)
    );
    assert_eq!(error_code(&runtime_error), error_code(&handover_error));
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().documents().count()),
        0
    );

    // 授权文件：两条路径共用打开链，重复打开只保留一个 Editor 视图。
    let inside_text = dunce::canonicalize(&inside)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    dispatch_handover(
        &root,
        &mut window,
        HandoverParams::OpenFile {
            path: inside_text.clone(),
        },
    )
    .expect("handover 必须打开项目内文件");
    match dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::PanelOpenFile { path: inside_text },
    ) {
        Ok(crate::app_shell::RuntimeMutationReply::Done(Ok(()))) => {}
        _ => panic!("Runtime 必须接受同一文件的打开"),
    }
    window.run_until_parked();
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().documents().count()),
        1,
        "同一文件不得产生第二份 Document"
    );
    let editor_views = root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .filter(|view| matches!(view, ViewKey::Editor { .. }))
            .count()
    });
    assert_eq!(editor_views, 1, "同一文件不得产生第二个 Editor 视图");

    // 未覆盖 cwd：handover 与 Runtime 新建终端都返回同一 Unauthorized。
    let outside_cwd = dunce::canonicalize(&outside_dir)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let handover_error = dispatch_handover(
        &root,
        &mut window,
        HandoverParams::NewTerminal {
            project_root: None,
            cwd: Some(outside_cwd.clone()),
        },
    )
    .expect_err("handover 越权 cwd 必须拒绝");
    let Err(runtime_error) = dispatch_runtime_mutation(
        &root,
        &mut window,
        crate::local_control::RuntimeMutation::TerminalCreate {
            project_root: None,
            cwd: Some(outside_cwd),
        },
    ) else {
        panic!("Runtime 越权 cwd 必须拒绝");
    };
    assert_eq!(
        error_code(&handover_error),
        Some(DomainErrorCode::Unauthorized)
    );
    assert_eq!(error_code(&runtime_error), error_code(&handover_error));
}
