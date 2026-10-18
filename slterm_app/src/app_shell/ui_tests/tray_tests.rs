//! 09 篇 M10.3:托盘 1 Hz 快照、attention 聚合与回窗路径。
//!
//! 真实托盘交互(Shell_NotifyIconW/右键菜单点击)按 11 篇豁免走构建产物
//! 人工 smoke;这里驱动生产同一条快照推送函数与壳事件入口,断言「推送
//! 什么、何时不推、动作落到哪个 pane」。

use std::time::Duration;

use gpui::TestAppContext;

use super::{NewTerminalTab, open_workspace, open_workspace_with_ai};
use crate::ai_hook::{AiHookEvent, parse_envelope};
use crate::app_shell::SltermShellEvent;
use crate::app_shell::config::Settings;
use crate::tray::{reset_for_test, take_updates_for_test};

fn hook_event(pane: u64, payload: &str) -> AiHookEvent {
    parse_envelope(format!("slterm-hook/1 source=claude pane={pane}\n{payload}").as_bytes())
        .expect("测试信封应可解析")
}

/// 1 Hz 快照:attention 翻转后首拍推送一次,状态不变的第二拍不再推送
/// (去抖在 `tray::update` 内完成,1 Hz 调用方无需自行判重)。
#[gpui::test]
fn tray_snapshot_tracks_attention_and_is_debounced_at_one_hz(cx: &mut TestAppContext) {
    reset_for_test();
    let (workspace, window, ai_tx) = open_workspace_with_ai(cx);
    // 构造期首帧快照(空清单)不产生推送;清掉用例间残留的启用态记录。
    reset_for_test();

    let pane = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_id().expect("首个 pane")
    });
    ai_tx
        .send(hook_event(
            pane.0,
            r#"{"hook_event_name":"SessionStart","session_id":"tray-1"}"#,
        ))
        .unwrap();
    ai_tx
        .send(hook_event(
            pane.0,
            r#"{"hook_event_name":"PermissionRequest","session_id":"tray-1"}"#,
        ))
        .unwrap();
    // Hook 泵 75ms 一批;随后 1 Hz 看门狗拍发出托盘快照。
    window.executor().advance_clock(Duration::from_millis(80));
    window.run_until_parked();
    window.executor().advance_clock(Duration::from_secs(1));
    window.run_until_parked();

    let pushes = take_updates_for_test();
    let expected_language = cx.update(|cx| cx.global::<Settings>().language);
    assert_eq!(pushes.len(), 1, "attention 翻转后首拍必须推送一次");
    let (agents, language) = &pushes[0];
    assert_eq!(*language, expected_language);
    assert_eq!(agents.len(), 1, "只有已识别 AI CLI 的 pane 进托盘菜单");
    assert_eq!(agents[0].pane, pane);
    assert!(agents[0].needs_attention, "权限确认必须点亮橙点");
    assert!(
        agents[0].label.starts_with("claude"),
        "菜单文案以程序名开头: {}",
        agents[0].label
    );

    // 状态不变的第二拍:内容判等直接返回,不得重复 PostMessage。
    window.executor().advance_clock(Duration::from_secs(1));
    window.run_until_parked();
    assert!(
        take_updates_for_test().is_empty(),
        "1 Hz 快照内容不变时不得重复推送"
    );
}

/// 托盘回窗路径:左键/菜单项 → 聚焦来源 pane;「退出」→ 关窗同语义的
/// 退出请求(测试构建记录请求,虚拟窗口不支持真实移除)。
#[gpui::test]
fn tray_focus_and_quit_events_reach_the_workspace(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let first = workspace.read_with(cx, |workspace, _| {
        workspace.first_pane_id().expect("首个 pane")
    });

    // 第二个 tab 处于活跃态;托盘点回第一个 tab 的 pane。
    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.active_tab_index()),
        1
    );

    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            // 托盘 Focus 命令在壳侧的翻译结果(见 app_shell::tray_command_event)。
            workspace.handle_shell_event(
                SltermShellEvent::NotificationFocus(Some(first)),
                window,
                cx,
            );
        });
    });
    assert_eq!(
        workspace.read_with(cx, |workspace, _| workspace.active_tab_index()),
        0,
        "托盘点击必须切回来源 pane 所在 tab"
    );

    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.handle_shell_event(SltermShellEvent::TrayQuit, window, cx);
        });
    });
    assert!(
        workspace.read_with(cx, |workspace, _| workspace.tray_quit_requested()),
        "托盘退出必须走关窗同一条退出链"
    );
}
