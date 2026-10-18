//! B20/B73 侧栏三视图与 B7 键位热重绑的虚拟窗口用例。
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

/// B73 Nav:两 tab(其一左右分屏)全部可见;tab 行激活切换、pane 行聚焦。
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

/// B73 Commit:注入快照渲染分支/去重变更行/历史行;无快照走空态。
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

/// B73 AgentFiles:注入目录替身,存在/缺失目录都保留行位。
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

/// B7:设置 Changed → 重读 keybind 段 → 新组合生效、被接管动作的旧默认
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
