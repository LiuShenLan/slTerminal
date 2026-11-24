//! Runtime Agent 资源：持久会话身份与运行期仲裁状态派生。

use std::sync::mpsc;

use slterm_agents::AgentSession;
use slterm_runtime_protocol::{AgentResource, DomainErrorCode, JsonRpcError};

use super::*;

fn dispatch_agents(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
) -> Result<Vec<AgentResource>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(SltermShellEvent::RuntimeAgents { completion }, window, cx);
        });
    });
    response
        .try_recv()
        .expect("AppRoot 必须返回 Agent 资源回执")
}

#[gpui::test]
fn runtime_agents_derive_from_session_identity_and_status(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);

    assert!(
        dispatch_agents(&root, &mut window)
            .expect("Agent 列表必须成功")
            .is_empty(),
        "纯 Shell 不产生 Agent 关联行"
    );

    root.update(cx, |root, _| {
        root.agents.seed_session(
            PaneId(panel.sequence()),
            AgentSession {
                source: "claude".to_owned(),
                session_id: Some("session-a".to_owned()),
                session_file: None,
            },
        );
    });
    let agents = dispatch_agents(&root, &mut window).expect("Agent 列表必须成功");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_id, "claude:session-a");
    assert_eq!(agents[0].terminal_id, terminal_id(TERMINAL_A).to_string());
    assert_eq!(agents[0].project_id, project_id(PROJECT_A).to_string());
    assert_eq!(agents[0].kind, "claude");
    assert_eq!(agents[0].status, "unknown");
    assert_eq!(agents[0].native_session_id.as_deref(), Some("session-a"));

    root.update(cx, |root, _| root.close_in_progress = true);
    let error = dispatch_agents(&root, &mut window).unwrap_err();
    assert_eq!(
        error.data.as_ref().map(|data| data.code),
        Some(DomainErrorCode::Closing)
    );
}
