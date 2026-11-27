//! 托盘快照从唯一 AgentHub 与项目快照派生，并随语言一起发布。

use slterm_agents::AgentSession;

use super::*;

/// 1 Hz 节拍把真实会话与 cwd 尾段发布到托盘共享清单。
#[gpui::test]
fn tray_snapshot_publishes_accepted_agent_sessions(cx: &mut TestAppContext) {
    crate::tray::reset_for_test();
    // 测试没有常驻调度循环：订阅一次使一次性执行体通过订阅闸门。
    let _watchdog =
        crate::background_tasks::subscribe(crate::background_tasks::AGENT_SCREEN_WATCHDOG)
            .expect("看门狗任务必须已登记");
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    cx.update(|cx| {
        cx.set_global(crate::app_shell::config::Settings {
            tray: true,
            ..crate::app_shell::config::Settings::default()
        });
    });
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
    root.update(cx, |root, cx| root.poll_tray(cx));
    let updates = crate::tray::take_updates_for_test();
    let (agents, _language) = updates.last().expect("托盘必须收到快照");
    let agent = agents
        .iter()
        .find(|agent| agent.pane == PaneId(panel.sequence()))
        .expect("已登记会话必须进入托盘清单");
    assert_eq!(agent.label, "claude · a", "标签必须带 CLI 与 cwd 尾段");
    assert!(!agent.needs_attention);
}
