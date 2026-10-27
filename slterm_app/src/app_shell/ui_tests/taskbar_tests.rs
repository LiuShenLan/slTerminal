//! OSC 9;4 → 任务栏的投递与清除路径。
//!
//! 真实 COM 面按测试豁免登记走构建产物人工 smoke;这里断言「哪些形态被
//! 投递」与「清除边恰好恢复常态」——测试构建的 `taskbar::apply` 只记录调用
//! 序列,不触 COM(虚拟窗口也没有原生 HWND)。

use crate::gpui_terminal::PaneId;
use gpui::{Entity, TestAppContext, VisualTestContext};
use slterm_terminal::event::Event as TermEvent;

use super::{CloseActivePane, SplitRight, open_workspace};
use crate::ai_hook::{AiHookEvent, parse_envelope};
use crate::app_shell::workspace::SltermWorkspace;
use crate::taskbar::{TaskProgress, take_applied_for_test};

/// 经视图事件白集喂一条 OSC 9;4(生产入口 = 会话泵;这里绕开 PTY 线程)。
fn feed_progress(
    workspace: &Entity<SltermWorkspace>,
    cx: &mut TestAppContext,
    pane: PaneId,
    state: u8,
    value: Option<u8>,
) {
    let view = workspace
        .read_with(cx, |workspace, _| workspace.pane_view(pane))
        .unwrap_or_else(|| panic!("pane {} 的视图必须存在", pane.0));
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.feed_event_for_test(TermEvent::Progress { state, value }, cx);
        });
    });
}

/// 走生产同一条 batch 入口处理已接纳的 hook 事件。
fn process_lifecycle(
    workspace: &Entity<SltermWorkspace>,
    cx: &mut TestAppContext,
    event: AiHookEvent,
) {
    cx.update(|cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.process_agent_lifecycle_events(&[event], cx);
        });
    });
}

fn hook_event(payload: &str) -> AiHookEvent {
    parse_envelope(format!("slterm-hook/1 source=claude pane=1\n{payload}").as_bytes())
        .expect("测试信封应可解析")
}

/// 聚焦 pane 的 OSC 9;4 上任务栏;同形态去重;OSC 0 与规范外码清除;
/// 清除后重发同值必须重新投递(视图不持有进度镜像)。
#[gpui::test]
fn focused_pane_progress_drives_the_taskbar_and_clear_codes_reset_it(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    take_applied_for_test();

    feed_progress(&workspace, cx, PaneId(1), 1, Some(42));
    assert_eq!(take_applied_for_test(), vec![(0, TaskProgress::Value(42))]);

    // 同 pane 同形态:不重复过 COM(`cargo build` 每秒会发好几次)。
    feed_progress(&workspace, cx, PaneId(1), 1, Some(42));
    assert!(take_applied_for_test().is_empty());

    // OSC 0 = 程序自报清除。
    feed_progress(&workspace, cx, PaneId(1), 0, None);
    assert_eq!(take_applied_for_test(), vec![(0, TaskProgress::None)]);

    // 清除后重发同值仍要投递:视图若缓存进度镜像会错误去重。
    feed_progress(&workspace, cx, PaneId(1), 1, Some(42));
    assert_eq!(take_applied_for_test(), vec![(0, TaskProgress::Value(42))]);

    // 规范外码(实测 9;4;5;0)归清除:进度条不能停在最后状态上。
    feed_progress(&workspace, cx, PaneId(1), 5, Some(0));
    assert_eq!(take_applied_for_test(), vec![(0, TaskProgress::None)]);

    // 不确定/错误/暂停三态原样过 COM(五态映射的消费端验证)。
    feed_progress(&workspace, cx, PaneId(1), 3, None);
    feed_progress(&workspace, cx, PaneId(1), 2, None);
    feed_progress(&workspace, cx, PaneId(1), 4, Some(90));
    assert_eq!(
        take_applied_for_test(),
        vec![
            (0, TaskProgress::Indeterminate),
            (0, TaskProgress::Error(None)),
            (0, TaskProgress::Paused(Some(90))),
        ]
    );
}

/// 任务栏是窗口级的:后台 pane 的进度不上任务栏,聚焦 pane 才上。
#[gpui::test]
fn background_pane_progress_never_reaches_the_taskbar(cx: &mut TestAppContext) {
    let (workspace, mut window): (Entity<SltermWorkspace>, VisualTestContext) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    take_applied_for_test();

    // 分屏后焦点在新 pane(2);pane 1 变成后台 pane。
    feed_progress(&workspace, cx, PaneId(1), 1, Some(10));
    assert!(
        take_applied_for_test().is_empty(),
        "后台 pane 不得覆盖任务栏"
    );

    feed_progress(&workspace, cx, PaneId(2), 1, Some(20));
    assert_eq!(take_applied_for_test(), vec![(0, TaskProgress::Value(20))]);
}

/// 回合结束(TurnDone)与 agent 退出(SessionEnd)都恢复任务栏常态。
#[gpui::test]
fn turn_done_and_agent_exit_clear_the_taskbar_progress(cx: &mut TestAppContext) {
    let (workspace, _window) = open_workspace(cx);
    let turn_done = hook_event(r#"{"hook_event_name":"Stop","session_id":"s-1"}"#);
    let session_end = hook_event(r#"{"hook_event_name":"SessionEnd","session_id":"s-1"}"#);

    feed_progress(&workspace, cx, PaneId(1), 1, Some(50));
    take_applied_for_test();
    process_lifecycle(&workspace, cx, turn_done);
    assert_eq!(
        take_applied_for_test(),
        vec![(0, TaskProgress::None)],
        "TurnDone 必须清进度"
    );

    feed_progress(&workspace, cx, PaneId(1), 3, None);
    take_applied_for_test();
    process_lifecycle(&workspace, cx, session_end);
    assert_eq!(
        take_applied_for_test(),
        vec![(0, TaskProgress::None)],
        "agent 退出必须清进度"
    );
}

/// pane 关闭(分屏收编路径)清掉它名下的进度条。
#[gpui::test]
fn closing_the_source_pane_clears_the_taskbar_progress(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();

    feed_progress(&workspace, cx, PaneId(2), 1, Some(70));
    take_applied_for_test();

    window.dispatch_action(CloseActivePane);
    window.run_until_parked();
    assert_eq!(
        take_applied_for_test(),
        vec![(0, TaskProgress::None)],
        "关闭来源 pane 必须清进度"
    );
}

/// 别的 pane 的清除边不得打断正在显示的进度。
#[gpui::test]
fn other_panes_clear_edges_do_not_touch_the_displayed_progress(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();

    // 焦点在 pane 2,它拥有任务栏进度。
    feed_progress(&workspace, cx, PaneId(2), 1, Some(60));
    take_applied_for_test();

    // pane 1 的命令收尾(133;D)不是进度主人,不得清掉 pane 2 的条。
    let view = workspace
        .read_with(cx, |workspace, _| workspace.pane_view(PaneId(1)))
        .expect("pane 1 视图");
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.feed_event_for_test(
                slterm_terminal::event::Event::CommandDone { exit_code: Some(0) },
                cx,
            );
        });
    });
    assert!(take_applied_for_test().is_empty());
}
