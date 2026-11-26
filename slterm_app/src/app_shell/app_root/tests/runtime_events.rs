//! Runtime 事件发布：项目切换产生合法事件，关闭后抑制过期事件。

use std::sync::mpsc;

use slterm_runtime_protocol::RuntimeEvent;

use super::*;

fn install_publisher(
    root: &Entity<AppRoot>,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<RuntimeEvent> {
    let (sender, receiver) = mpsc::sync_channel(16);
    root.update(cx, |root, _| {
        root.runtime_events = Some(crate::local_control::RuntimeEventPublisher::from_callback(
            move |event| sender.try_send(event).is_ok(),
        ));
    });
    receiver
}

/// 锚：隐藏或切项目仍收合法事件，关闭/销毁后不再发布过期事件。
#[gpui::test]
fn runtime_events_visibility_and_generation(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let events = install_publisher(&root, cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    activate_project(&root, &mut window, PROJECT_A);
    // 切到当前隐藏的项目同样产生合法事件。
    activate_project(&root, &mut window, PROJECT_B);

    let mut activated = Vec::new();
    while let Ok(event) = events.try_recv() {
        if event.kind == slterm_runtime_protocol::RuntimeEventKind::ProjectActivated {
            activated.push(event);
        }
    }
    let last = activated
        .last()
        .expect("项目激活必须发布 project.activated");
    assert_eq!(last.resource, project_id(PROJECT_B).to_string());
    assert_eq!(last.revision, 0, "revision 由事件环分配");
    assert!(last.generation > 0, "事件必须携带资源代次");
    assert!(
        activated
            .iter()
            .any(|event| event.resource == project_id(PROJECT_A).to_string()),
        "切到隐藏项目前后的激活都必须发布事件"
    );
}

/// hook 事件必须来自目标 pane 的 PTY 进程树；伪造 client_pid 只被丢弃。
#[gpui::test]
fn ai_hook_events_require_pane_process_ownership(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (pane, pid) = cx.update(|cx| {
        let root = root.read(cx);
        let terminal_id = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .view_keys()
            .into_iter()
            .find_map(|view| match view {
                ViewKey::Terminal { terminal_id, .. } => Some(terminal_id),
                _ => None,
            })
            .unwrap();
        let panel_id = root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .keys()
            .copied()
            .find(|panel| root.terminal_identity(*panel, cx) == Some(terminal_id))
            .unwrap();
        let target = root.terminals.borrow().target(terminal_id).unwrap();
        let identity = root.terminals.borrow().process_identity(target).unwrap();
        (
            crate::gpui_terminal::PaneId(panel_id.sequence()),
            identity.pid().get(),
        )
    });

    let hook = |client_pid: u32| {
        let envelope = format!(
            "slterm-hook/1 source=claude pane={}
{{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"session-a\"}}",
            pane.0
        );
        let mut event = crate::ai_hook::parse_envelope(envelope.as_bytes()).expect("信封可解析");
        event.client_pid = Some(client_pid);
        event
    };
    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.on_ai_hook_events(vec![hook(u32::MAX - 1)], cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, _| root.agents.host(pane).unwrap().status()),
        crate::ai_agents::AgentStatus::Unknown,
        "伪造 pane 归属不得改写状态"
    );

    window.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.on_ai_hook_events(vec![hook(pid)], cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, _| root.agents.host(pane).unwrap().status()),
        crate::ai_agents::AgentStatus::Working,
        "来自 pane 进程树的事件必须被接受"
    );
}

#[gpui::test]
fn runtime_events_are_suppressed_after_close_starts(cx: &mut TestAppContext) {
    let (root, _window, _state) = open_root(cx);
    let events = install_publisher(&root, cx);
    root.update(cx, |root, _| {
        root.close_completed = true;
        root.publish_runtime_event(
            slterm_runtime_protocol::RuntimeEventKind::ProjectActivated,
            "late-resource".to_owned(),
        );
    });

    assert!(events.try_recv().is_err(), "关闭完成后不得再发布过期事件");
}

#[gpui::test]
fn runtime_events_are_suppressed_while_close_is_in_progress(cx: &mut TestAppContext) {
    let (root, _window, _state) = open_root(cx);
    let events = install_publisher(&root, cx);
    root.update(cx, |root, _| {
        root.close_in_progress = true;
        root.publish_runtime_event(
            slterm_runtime_protocol::RuntimeEventKind::ProjectActivated,
            "panel-a".to_owned(),
        );
    });

    assert!(events.try_recv().is_err(), "关闭进行中不得回填事件");
}
