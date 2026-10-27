//! /侧栏三视图与键位热重绑的虚拟窗口用例。
//!
//! Commit/AgentFiles 全部走注入数据,不读真实 git 仓库与 USERPROFILE。

use std::sync::Arc;

use super::*;
use crate::agent_dirs::AgentGlobalDir;
use crate::app_shell::settings_pane::SettingsPaneEvent;
use crate::app_shell::workspace::keyboard_bindings;
use crate::side_panel::{GitCommit, GitInfo};

/// `debug_bounds` 只收 `&'static str`;动态 selector 在用例内一次性泄漏。
fn leaked(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// Nav:两 tab(其一左右分屏)全部可见;tab 行激活切换、pane 行聚焦。
#[gpui::test]
fn nav_view_lists_tabs_and_panes_and_clicking_focuses(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.dispatch_action(SplitRight);
    window.run_until_parked();
    window.dispatch_action(NewTerminalTab);
    window.run_until_parked();
    window.dispatch_action(SelectPreviousTab);
    window.run_until_parked();
    window.refresh().unwrap();
    draw_frame(&mut window);

    for selector in [
        "sidebar-nav-tab-0",
        "sidebar-nav-tab-1",
        "sidebar-nav-pane-1",
        "sidebar-nav-pane-2",
        "sidebar-nav-pane-3",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制"
        );
    }

    // tab 行 = 激活切换。
    click_selector(&mut window, "sidebar-nav-tab-1");
    draw_frame(&mut window);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 1);

    // pane 行 = 激活所属 tab 并聚焦该 pane。
    click_selector(&mut window, "sidebar-nav-pane-2");
    draw_frame(&mut window);
    assert_eq!(workspace.read_with(cx, |ws, _| ws.active_tab_index()), 0);
    let (_, _, focused, _) = workspace
        .read_with(cx, |ws, _| ws.active_tab_snapshot())
        .expect("活跃终端必须存在");
    assert_eq!(focused, PaneId(2), "点击 pane 行必须聚焦该 pane");
}

/// 生产 SideView 在同槽替换、关闭重开时保存并恢复，整栏隐藏不销毁或重复保存。
#[gpui::test]
fn sidebar_view_state_survives_replacement_reopen_and_hide(cx: &mut TestAppContext) {
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_sidebar::SideViewState;
    use slterm_workspace::{ViewKey, ViewState};

    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::{WorkspaceFlushReport, WorkspaceWriter};

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut plan = StartupPlan::fresh();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let persisted = ViewState::new(
        &key,
        json!({"projects": [], "unassigned_expanded": true, "scroll": 42.0}),
        &plan.view_state_registry,
    )
    .unwrap();
    plan.side_view_states.insert(
        SideViewId::Nav,
        SideViewState::from_view_state(SideViewId::Nav, persisted.clone()).unwrap(),
    );
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let (workspace, mut window, _ai_tx) = open_workspace_with_startup_and_writer(
        cx,
        WorkspaceStartup::LaunchTerminal {
            cwd: None,
            command: Program::Just("slterm-test-missing-shell-executable".into()),
        },
        plan.clone(),
        Some(writer.client()),
    );
    window.refresh().unwrap();
    draw_frame(&mut window);

    let (first, initial) = workspace.read_with(cx, |workspace, cx| {
        let view = workspace.side_view_for_test(SideViewId::Nav).unwrap();
        (view.entity_id(), view.read(cx).view_state().unwrap())
    });
    assert_eq!(initial, persisted);
    let updated = ViewState::new(
        &key,
        json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &plan.view_state_registry,
    )
    .unwrap();
    let nav = workspace.read_with(cx, |workspace, _| {
        workspace.side_view_for_test(SideViewId::Nav).unwrap()
    });
    nav.update(cx, |view, _| {
        view.set_state_for_test(
            SideViewState::from_view_state(SideViewId::Nav, updated.clone()).unwrap(),
        );
    });

    click_selector(&mut window, "sidebar-view-btn-explorer");
    window.run_until_parked();
    writer.flush().unwrap();
    assert_eq!(
        observer
            .view_state(&key, &plan.view_state_registry)
            .unwrap()
            .unwrap()
            .state,
        updated,
        "同槽替换必须保存旧实体修改后的最新状态"
    );

    click_selector(&mut window, "sidebar-view-btn-nav");
    window.run_until_parked();
    let (reopened, restored) = workspace.read_with(cx, |workspace, cx| {
        let view = workspace.side_view_for_test(SideViewId::Nav).unwrap();
        (view.entity_id(), view.read(cx).view_state().unwrap())
    });
    assert_ne!(first, reopened);
    assert_eq!(restored, updated, "关闭重开必须读取同一内存恢复事实");
    writer.flush().unwrap();

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    let after_hide = workspace.read_with(cx, |workspace, _| {
        workspace
            .side_view_for_test(SideViewId::Nav)
            .unwrap()
            .entity_id()
    });
    assert_eq!(after_hide, reopened, "整栏隐藏与恢复不得重建实体");
    assert_eq!(
        writer.flush().unwrap(),
        WorkspaceFlushReport::default(),
        "整栏隐藏不得产生额外保存"
    );

    click_selector(&mut window, "sidebar-view-btn-nav");
    window.run_until_parked();
    writer.flush().unwrap();
    writer.shutdown().unwrap();
    assert_eq!(
        observer
            .view_state(&key, &plan.view_state_registry)
            .unwrap()
            .unwrap()
            .state,
        updated,
        "显式关闭不得删除持久状态"
    );
}

/// 启动恢复未知只提供内存默认态，生产关闭链不得覆盖数据库中的有效记录。
#[gpui::test]
fn sidebar_unknown_recovery_does_not_overwrite_valid_record(cx: &mut TestAppContext) {
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_workspace::{ViewKey, ViewState};

    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriter;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut plan = StartupPlan::fresh();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let persisted = ViewState::new(
        &key,
        json!({"projects": [], "unassigned_expanded": true, "scroll": 42.0}),
        &plan.view_state_registry,
    )
    .unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store.save_view_state(&key, &persisted).unwrap();
    drop(store);
    plan.side_view_state_write_blocked.insert(SideViewId::Nav);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let (workspace, mut window, _ai_tx) = open_workspace_with_startup_and_writer(
        cx,
        WorkspaceStartup::LaunchTerminal {
            cwd: None,
            command: Program::Just("slterm-test-missing-shell-executable".into()),
        },
        plan.clone(),
        Some(writer.client()),
    );
    let initial = workspace.read_with(cx, |workspace, cx| {
        workspace
            .side_view_for_test(SideViewId::Nav)
            .unwrap()
            .read(cx)
            .view_state()
            .unwrap()
    });
    assert_eq!(initial.state()["scroll"], json!(0.0));

    click_selector(&mut window, "sidebar-view-btn-nav");
    window.run_until_parked();
    writer.flush().unwrap();
    writer.shutdown().unwrap();

    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let stored = observer
        .view_state(&key, &plan.view_state_registry)
        .unwrap()
        .expect("恢复未知不得删除原记录");
    assert_eq!(stored.state, persisted);
}

/// 仍有 SideView 打开时移除窗口，实体最终析构必须先于 writer shutdown 保存最新状态。
#[gpui::test]
fn sidebar_open_view_state_is_saved_during_workspace_teardown(cx: &mut TestAppContext) {
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_sidebar::SideViewState;
    use slterm_workspace::{ViewKey, ViewState};

    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriter;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let plan = StartupPlan::fresh();
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let updated = ViewState::new(
        &key,
        json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &plan.view_state_registry,
    )
    .unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let (workspace, mut window, _ai_tx) = open_workspace_with_startup_and_writer(
        cx,
        WorkspaceStartup::LaunchTerminal {
            cwd: None,
            command: Program::Just("slterm-test-missing-shell-executable".into()),
        },
        plan.clone(),
        Some(writer.client()),
    );
    let nav = workspace.read_with(cx, |workspace, _| {
        workspace.side_view_for_test(SideViewId::Nav).unwrap()
    });
    nav.update(cx, |view, _| {
        view.set_state_for_test(
            SideViewState::from_view_state(SideViewId::Nav, updated.clone()).unwrap(),
        );
    });
    let weak_nav = nav.downgrade();
    let weak_workspace = workspace.downgrade();
    drop(nav);

    window.update(|window, _| window.remove_window());
    drop(window);
    drop(workspace);
    // 强计数归零只排队释放；空 update 驱动 GPUI effect flush 执行实体析构。
    cx.update(|_| {});
    cx.run_until_parked();
    assert!(weak_workspace.upgrade().is_none(), "工作区必须随窗口释放");
    assert!(weak_nav.upgrade().is_none(), "SideView 必须随工作区释放");

    writer.shutdown().unwrap();
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let stored = observer
        .view_state(&key, &plan.view_state_registry)
        .unwrap()
        .expect("退出析构必须保存仍打开的 SideView");
    assert_eq!(stored.state, updated);
}

/// 恢复屏障经明确编辑解除后，关闭视图在 writer 队列满时仍须保存最终状态。
#[gpui::test]
fn sidebar_edited_recovery_state_survives_full_writer_queue_until_shutdown(
    cx: &mut TestAppContext,
) {
    use serde_json::json;
    use slterm_path::AppPaths;
    use slterm_sidebar::SideViewState;
    use slterm_workspace::{ViewKey, ViewState};

    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::{WorkspaceWriter, WorkspaceWriterError};

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut plan = StartupPlan::fresh();
    plan.side_view_state_write_blocked.insert(SideViewId::Nav);
    let key = ViewKey::SideView {
        side_view_id: SideViewId::Nav,
    };
    let updated = ViewState::new(
        &key,
        json!({"projects": [], "unassigned_expanded": true, "scroll": 84.0}),
        &plan.view_state_registry,
    )
    .unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let (workspace, mut window, _ai_tx) = open_workspace_with_startup_and_writer(
        cx,
        WorkspaceStartup::LaunchTerminal {
            cwd: None,
            command: Program::Just("slterm-test-missing-shell-executable".into()),
        },
        plan.clone(),
        Some(writer.client()),
    );
    let nav = workspace.read_with(cx, |workspace, _| {
        workspace.side_view_for_test(SideViewId::Nav).unwrap()
    });
    nav.update(cx, |view, _| {
        view.set_state_for_test(
            SideViewState::from_view_state(SideViewId::Nav, updated.clone()).unwrap(),
        );
    });

    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    writer.save_view_state(&key, &updated).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));
    for position in 0..128 {
        let filler = ViewState::new(
            &key,
            json!({"projects": [], "unassigned_expanded": true, "scroll": position}),
            &plan.view_state_registry,
        )
        .unwrap();
        match writer.save_view_state(&key, &filler) {
            Ok(()) => {}
            Err(WorkspaceWriterError::QueueFull) => break,
            Err(error) => panic!("填充 writer 失败: {error}"),
        }
        assert!(position < 127, "writer 必须有界");
    }
    assert!(matches!(
        writer.save_view_state(&key, &updated),
        Err(WorkspaceWriterError::QueueFull)
    ));

    // 关闭后不重开；旧强句柄使实体延迟到 workspace 退出时才真正析构。
    click_selector(&mut window, "sidebar-view-btn-nav");
    window.run_until_parked();
    let weak_nav = nav.downgrade();
    window.update(|window, _| window.remove_window());
    drop(window);
    drop(workspace);
    drop(nav);
    cx.update(|_| {});
    cx.run_until_parked();
    assert!(weak_nav.upgrade().is_none());
    fault.execute_batch("ROLLBACK").unwrap();

    writer.shutdown().unwrap();
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let stored = observer
        .view_state(&key, &plan.view_state_registry)
        .unwrap()
        .expect("最终关闭状态必须保存");
    assert_eq!(stored.state, updated);
}

/// 生产 Host 在整栏隐藏后复用真实实体，同槽替换时销毁旧实体。
#[gpui::test]
fn production_side_view_host_preserves_on_hide_and_destroys_on_replace(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    draw_frame(&mut window);
    let nav = workspace.read_with(cx, |ws, _| {
        ws.side_view_for_test(SideViewId::Nav)
            .expect("默认 Nav 必须由生产 Host 创建")
    });
    let nav_id = nav.entity_id();
    let weak_nav = nav.downgrade();
    drop(nav);

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    assert_eq!(
        workspace
            .read_with(cx, |ws, _| ws.side_view_for_test(SideViewId::Nav).unwrap())
            .entity_id(),
        nav_id,
        "整栏隐藏再显示必须复用同一真实实体"
    );

    click_selector(&mut window, "sidebar-view-btn-explorer");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        weak_nav.upgrade().is_none(),
        "同槽打开 Explorer 必须销毁真实 Nav 实体"
    );
    assert!(workspace.read_with(cx, |ws, _| ws.side_view_for_test(SideViewId::Nav).is_none()));
    assert!(workspace.read_with(cx, |ws, _| {
        ws.side_view_for_test(SideViewId::Explorer).is_some()
    }));
}

/// 默认按钮全在上组；打开 AgentFiles 必须替换 Nav，而不是沿用旧壳固定下槽。
#[gpui::test]
fn production_sidebar_rendering_uses_core_button_slots(cx: &mut TestAppContext) {
    let (_workspace, mut window) = open_workspace(cx);
    draw_frame(&mut window);
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);

    assert!(
        window
            .debug_bounds("sidebar-slot-top-agent_files")
            .is_some(),
        "AgentFiles 必须按 core 默认上槽渲染"
    );
    assert!(
        window.debug_bounds("sidebar-slot-top-nav").is_none(),
        "同槽替换后不得继续渲染 Nav"
    );
    assert!(
        window.debug_bounds("sidebar-split-divider").is_none(),
        "单槽不得沿用旧壳固定下槽形成分栏"
    );
}

/// Commit:注入快照渲染分支/去重变更行/历史行;无快照走空态。
#[gpui::test]
fn commit_view_renders_injected_snapshot_and_empty_state(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    window.refresh().unwrap();
    draw_frame(&mut window);
    click_selector(&mut window, "sidebar-view-btn-commit");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("sidebar-commit-empty").is_some(),
        "无 git 快照必须显示空态"
    );
    assert!(window.debug_bounds("sidebar-commit-branch").is_none());

    let snapshot = GitInfo {
        branch: "main".to_owned(),
        plus: 3,
        minus: 1,
        ahead: 2,
        conflicts: vec![
            ('U', "shared.txt".to_owned()),
            ('U', "conflict.txt".to_owned()),
        ],
        staged: vec![('M', "shared.txt".to_owned()), ('A', "new.txt".to_owned())],
        unstaged: vec![
            ('M', "shared.txt".to_owned()),
            ('?', "draft.txt".to_owned()),
        ],
        history: vec![GitCommit {
            short_hash: "abc1234".to_owned(),
            subject: "initial".to_owned(),
            ..GitCommit::default()
        }],
    };
    workspace.update(cx, |ws, _| {
        ws.set_commit_snapshot_for_test(Some(Arc::new(snapshot)));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);

    assert!(window.debug_bounds("sidebar-commit-branch").is_some());
    assert!(window.debug_bounds("sidebar-commit-history-0").is_some());
    // 冲突 2 + 去重后 staged 1 + unstaged 1 = 4 行,冲突路径不重复。
    for index in 0..4 {
        let selector = leaked(format!("sidebar-commit-change-{index}"));
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制"
        );
    }
    assert!(
        window
            .debug_bounds(leaked("sidebar-commit-change-4".to_owned()))
            .is_none(),
        "冲突路径不得在 staged/unstaged 重复出现"
    );
}

/// 侧栏布局按真实变更做 500ms 去抖；隐藏不取消保存，关窗立即冲刷最新终态。
#[gpui::test]
fn sidebar_layout_changes_debounce_for_500ms_and_flush_on_close(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let writes = Rc::new(RefCell::new(Vec::<crate::session::Session>::new()));
    workspace.update(cx, |ws, _| {
        let writes = Rc::clone(&writes);
        ws.set_session_writer_for_test(Rc::new(move |session| {
            writes.borrow_mut().push(session.clone());
            Ok(())
        }));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);

    window.executor().advance_clock(Duration::from_millis(600));
    window.run_until_parked();
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    window.executor().advance_clock(Duration::from_millis(300));
    window.run_until_parked();
    assert_eq!(writes.borrow().len(), 0, "首次变更的去抖窗口内不得写盘");

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    let hidden = workspace.read_with(cx, |ws, _| ws.sidebar_state().clone());
    assert!(!hidden.visible_intent());

    window.executor().advance_clock(Duration::from_millis(400));
    window.run_until_parked();
    assert_eq!(
        writes.borrow().len(),
        0,
        "1 Hz 会话泵不得提前带走尚未到期的侧栏变更"
    );
    window.executor().advance_clock(Duration::from_millis(99));
    window.run_until_parked();
    assert_eq!(writes.borrow().len(), 0, "连续变更必须重算去抖截止时刻");
    window.executor().advance_clock(Duration::from_millis(1));
    window.run_until_parked();
    assert_eq!(writes.borrow().len(), 1, "最后一次变更 500ms 后必须保存");
    {
        let writes = writes.borrow();
        let saved = writes[0].sidebar.recover().unwrap();
        assert_eq!(saved.state, hidden);
        assert!(!writes[0].clean_exit);
    }

    window.dispatch_action(ToggleSidebar);
    window.run_until_parked();
    let restored = workspace.read_with(cx, |ws, _| ws.sidebar_state().clone());
    assert!(restored.visible_intent());
    assert!(window.simulate_close(), "无脏文件的关窗必须放行");
    let writes = writes.borrow();
    assert_eq!(writes.len(), 2, "关窗不得等待下一次去抖或 1 Hz 泵");
    let final_write = writes.last().unwrap();
    assert!(final_write.clean_exit);
    assert_eq!(final_write.sidebar.recover().unwrap().state, restored);
}

/// 亚像素移动若取整后宽度不变，不得当作新变更推迟既有保存截止时刻。
#[gpui::test]
fn sidebar_width_drag_ignores_rounding_noops_for_debounce(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let writes = Rc::new(RefCell::new(Vec::<crate::session::Session>::new()));
    workspace.update(cx, |ws, _| {
        let writes = Rc::clone(&writes);
        ws.set_session_writer_for_test(Rc::new(move |session| {
            writes.borrow_mut().push(session.clone());
            Ok(())
        }));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    let handle = window
        .debug_bounds("sidebar-resize-handle")
        .expect("拖拽热区必须绘制");
    let start = handle.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(
        point(start.x + px(120.0), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw_frame(&mut window);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.sidebar_state().width()),
        370
    );

    window.executor().advance_clock(Duration::from_millis(300));
    window.run_until_parked();
    window.simulate_mouse_move(
        point(start.x + px(120.1), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    draw_frame(&mut window);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.sidebar_state().width()),
        370,
        "测试移动必须在取整后保持原宽度"
    );
    window.executor().advance_clock(Duration::from_millis(199));
    window.run_until_parked();
    assert_eq!(writes.borrow().len(), 0);
    window.executor().advance_clock(Duration::from_millis(1));
    window.run_until_parked();
    assert_eq!(
        writes.borrow().len(),
        1,
        "无实际变化的移动不得推迟最后一次真实宽度变更的保存"
    );
    assert_eq!(
        writes.borrow()[0].sidebar.recover().unwrap().state.width(),
        370
    );
    window.simulate_mouse_up(
        point(start.x + px(120.1), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
}

/// 双槽打开时渲染可拖拽分隔条;拖拽实时改比例并落在钳制区间,期间
/// 终端网格零重排;关掉一个槽后比例保留、分隔条消失。
#[gpui::test]
fn sidebar_split_divider_drags_the_slot_ratio(cx: &mut TestAppContext) {
    let mut sidebar = SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::AgentFiles, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    let (workspace, mut window) = open_workspace_with_sidebar(cx, sidebar);
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("sidebar-split-divider").is_none(),
        "单槽时没有分隔条"
    );

    // 默认上槽 nav;打开下槽 agent_files 成双槽。
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("sidebar-split-divider")
        .expect("双槽打开时必须有拖拽分隔条");
    let pane_before = workspace
        .read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)))
        .expect("终端 pane 必须有矩形");

    let start = divider.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    // 指针下移:上槽比例增大,overlay 实时生效。
    let target = point(start.x, start.y + px(100.0));
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let during = workspace.read_with(cx, |ws, _| ws.sidebar_state().ratio());
    assert!(during > 0.5, "拖拽应实时增大上槽比例: {during}");
    let pane_during = workspace
        .read_with(cx, |ws, _| ws.pane_bounds_of(PaneId(1)))
        .expect("终端 pane 矩形仍在");
    assert_eq!(
        pane_during.size, pane_before.size,
        "拖拽侧栏分隔条不得重排终端网格"
    );

    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    window.run_until_parked();
    draw_frame(&mut window);
    let ratio = workspace.read_with(cx, |ws, _| ws.sidebar_state().ratio());
    assert!(
        (0.1..=0.9).contains(&ratio),
        "比例必须落在钳制区间: {ratio}"
    );
    assert!(ratio > 0.5);

    // 关闭下槽:比例保留,分隔条消失。
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("sidebar-split-divider").is_none(),
        "单槽不显示分隔条"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.sidebar_state().ratio()),
        ratio,
        "关闭一个槽不得重置比例"
    );
}

/// AgentFiles:注入目录替身,存在/缺失目录都保留行位。
#[gpui::test]
fn agent_files_view_renders_cached_directories_with_missing_placeholder(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    workspace.update(cx, |ws, _| {
        ws.set_agent_dirs_for_test(vec![
            AgentGlobalDir {
                cli_id: "claude".to_owned(),
                path: std::path::PathBuf::from("C:/home/.claude"),
                exists: true,
            },
            AgentGlobalDir {
                cli_id: "codex".to_owned(),
                path: std::path::PathBuf::from("C:/home/.codex"),
                exists: false,
            },
        ]);
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    click_selector(&mut window, "sidebar-view-btn-agent_files");
    draw_frame(&mut window);

    for selector in ["sidebar-agent-dir-claude", "sidebar-agent-dir-codex"] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "{selector} 必须绘制(缺失目录也保留占位行)"
        );
    }
}

/// 设置 Changed → 重读 keybind 段 → 新组合生效、被接管动作的旧默认
/// 组合失效;Ctrl+C / Ctrl+Shift+C 保留键仍被准入门拒绝。
#[gpui::test]
fn settings_change_rebuilds_keymap_and_drops_overridden_default(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, path.clone());
    window.refresh().unwrap();
    draw_frame(&mut window);

    // 设置页写盘后的载荷:重绑 split_right,同时尝试占用两个保留键。
    std::fs::write(
        &path,
        r#"{"keybind":[
            {"combo":"ctrl+alt+r","action":"split_right"},
            {"combo":"ctrl+shift+c","action":"save_file"},
            {"combo":"ctrl+c","action":"new_terminal_tab"}
        ]}"#,
    )
    .unwrap();
    window.update(|_, cx| {
        pane.update(cx, |_, cx| cx.emit(SettingsPaneEvent::Changed));
    });
    window.run_until_parked();

    // 回到终端 tab:设置 tab 关闭后焦点与活跃下标收敛。
    click_selector(&mut window, "settings-back");
    window.run_until_parked();
    draw_frame(&mut window);

    let applied = cx.update(|cx| keyboard_bindings::applied_user_keybinds(cx));
    assert_eq!(
        applied,
        [("ctrl+alt+r".to_owned(), "split_right".to_owned())],
        "保留键/坏行不得进入已应用表"
    );

    // 被用户表接管的动作:旧默认组合不再触发。
    window.simulate_keystrokes("ctrl-shift-d");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_snapshot(0).unwrap().0.len()),
        1,
        "被接管动作的旧默认组合必须失效"
    );

    // 新组合生效。
    window.simulate_keystrokes("ctrl-alt-r");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_snapshot(0).unwrap().0.len()),
        2,
        "重绑后的新组合必须触达工作区动作"
    );

    // 保留键仍被拒:Ctrl+C(若误绑会新建 tab)不得改变 tab 数。
    window.simulate_keystrokes("ctrl-c");
    window.simulate_keystrokes("ctrl-shift-c");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        1,
        "保留键不得产生绑定"
    );
}
