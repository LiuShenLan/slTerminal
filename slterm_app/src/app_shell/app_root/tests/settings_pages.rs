//! 设置中心页面行为：导航分组与过滤、快捷键目录、后台任务与恢复默认。

use slterm_settings::json::Value;

use super::*;

fn open_settings(window: &mut VisualTestContext) {
    window.simulate_keystrokes("ctrl-,");
    draw_frame(window);
}

fn focus_workspace(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    window.update(|window, cx| {
        let focus = root.read(cx).workspace_focus.clone();
        focus.focus(window, cx);
    });
}

/// 导航按全局／Agent 分组，过滤只影响导航且 Esc 先清空查询。
#[gpui::test]
fn settings_navigation_groups_and_filters_pages(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-nav-group-global").is_some(),
        "全局组标题必须渲染"
    );
    assert!(
        window.debug_bounds("settings-nav-group-agent").is_some(),
        "Agent 组标题必须渲染"
    );
    assert!(window.debug_bounds("settings-page-shortcuts").is_some());

    let settings = root.read_with(cx, |root, _| {
        root.settings_overlay.as_ref().unwrap().view.clone()
    });
    settings.update(cx, |view, cx| {
        view.set_nav_query_for_test("hooks", cx);
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-page-hooks").is_some());
    assert!(
        window.debug_bounds("settings-page-appearance").is_none(),
        "不匹配的全局页必须被过滤"
    );
    assert!(
        window.debug_bounds("settings-nav-group-global").is_none(),
        "无匹配页的组标题必须隐藏"
    );
    assert!(window.debug_bounds("settings-nav-group-agent").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.settings_overlay.is_some()),
        "Esc 清空搜索时不得关闭浮层"
    );
    assert!(
        settings
            .read_with(cx, |view, _| view.nav_query_for_test())
            .is_empty()
    );
    assert!(window.debug_bounds("settings-page-appearance").is_some());
}

/// 后台任务页：任务行来自注册表，启停立即生效并写入外部段。
#[gpui::test]
fn settings_background_tasks_toggle_applies_and_persists(cx: &mut TestAppContext) {
    crate::background_tasks::reset_for_test();
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window
        .debug_bounds("settings-page-background-tasks")
        .unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-background-tasks").is_some());
    assert!(
        window
            .debug_bounds("settings-task-toggle-planBalance")
            .is_some(),
        "注册表任务必须逐行渲染"
    );

    let before = crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE);
    let toggle = window
        .debug_bounds("settings-task-toggle-planBalance")
        .unwrap();
    window.simulate_click(toggle.center(), Modifiers::default());
    draw_frame(&mut window);

    assert_eq!(
        crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE),
        !before,
        "启停必须立即热应用"
    );
    root.read_with(cx, |root, _| {
        let segment = root
            .settings
            .values()
            .external_value(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT)
            .expect("外部段必须写入");
        let plan = segment
            .get(crate::background_tasks::PLAN_BALANCE)
            .expect("目标任务必须存在");
        assert_eq!(
            plan.get("enabled").and_then(|value| value.as_bool()),
            Some(!before)
        );
    });
}

/// 页级恢复默认：本页字段回到缺省，其他页不受影响。
#[gpui::test]
fn settings_page_reset_restores_field_defaults(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    root.update(cx, |root, cx| {
        root.apply_settings_field("theme", Value::String("Nord".to_owned()), cx);
        root.apply_settings_field("font_size", Value::Number(18.0), cx);
    });
    draw_frame(&mut window);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings.values().persisted_value("theme"),
            Some(&Value::String("Nord".to_owned()))
        );
    });

    let reset = window.debug_bounds("settings-reset-page").unwrap();
    window.simulate_click(reset.center(), Modifiers::default());
    draw_frame(&mut window);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings.values().persisted_value("theme"),
            Some(&Value::Null),
            "恢复本页默认必须把字段写回缺省"
        );
        assert_eq!(
            root.settings.values().persisted_value("font_size"),
            Some(&Value::Null)
        );
    });
}

/// 快捷键页只读渲染统一命令目录，按上下文分组。
#[gpui::test]
fn settings_shortcuts_page_lists_catalog_bindings(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let page = window.debug_bounds("settings-page-shortcuts").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);

    assert!(window.debug_bounds("settings-shortcuts").is_some());
    for group in ["global", "terminal", "layout", "sidebar"] {
        assert!(
            window
                .debug_bounds(Box::leak(
                    format!("settings-shortcut-group-{group}").into_boxed_str()
                ))
                .is_some(),
            "快捷键分组 {group} 必须渲染"
        );
        assert!(
            window
                .debug_bounds(Box::leak(
                    format!("settings-shortcut-{group}-0").into_boxed_str()
                ))
                .is_some(),
            "快捷键分组 {group} 必须至少有一条绑定"
        );
    }
}

/// 恢复所有设置默认：字段页与后台任务段一起回到缺省。
#[gpui::test]
fn settings_restore_all_resets_fields_and_tasks(cx: &mut TestAppContext) {
    crate::background_tasks::reset_for_test();
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    root.update(cx, |root, cx| {
        root.apply_settings_field("theme", Value::String("Nord".to_owned()), cx);
        root.apply_background_task_field(
            crate::background_tasks::PLAN_BALANCE,
            Some(false),
            Some(120),
            cx,
        );
    });
    draw_frame(&mut window);
    assert!(!crate::background_tasks::enabled(
        crate::background_tasks::PLAN_BALANCE
    ));

    let restore = window.debug_bounds("settings-restore-all").unwrap();
    window.simulate_click(restore.center(), Modifiers::default());
    draw_frame(&mut window);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings.values().persisted_value("theme"),
            Some(&Value::Null)
        );
        assert_eq!(
            root.settings
                .values()
                .external_value(slterm_settings::keys::BACKGROUND_TASKS_SEGMENT),
            Some(&Value::Null),
            "后台任务段必须回到缺省"
        );
    });
    assert!(
        crate::background_tasks::enabled(crate::background_tasks::PLAN_BALANCE),
        "恢复默认后任务必须回到默认启用"
    );
    assert_eq!(
        crate::background_tasks::interval_sec(crate::background_tasks::PLAN_BALANCE),
        60
    );
}
