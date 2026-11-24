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
        if event.kind == "project.activated" {
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

#[gpui::test]
fn runtime_events_are_suppressed_after_close_starts(cx: &mut TestAppContext) {
    let (root, _window, _state) = open_root(cx);
    let events = install_publisher(&root, cx);
    root.update(cx, |root, _| {
        root.close_completed = true;
        root.publish_runtime_event("project.activated", "late-resource".to_owned());
    });

    assert!(events.try_recv().is_err(), "关闭完成后不得再发布过期事件");
}

#[gpui::test]
fn runtime_events_are_suppressed_while_close_is_in_progress(cx: &mut TestAppContext) {
    let (root, _window, _state) = open_root(cx);
    let events = install_publisher(&root, cx);
    root.update(cx, |root, _| {
        root.close_in_progress = true;
        root.publish_runtime_event("panel.closed", "panel-a".to_owned());
    });

    assert!(events.try_recv().is_err(), "关闭进行中不得回填事件");
}
