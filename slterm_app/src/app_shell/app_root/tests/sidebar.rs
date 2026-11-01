//! 侧栏组织、按钮菜单与焦点生命周期测试。

use super::super::SidebarDivider;
use super::*;
use gpui::{MouseDownEvent, MouseUpEvent};
use slterm_sidebar::SidebarState;

fn assert_side_button_focused(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    view: SideViewId,
) {
    let expected = root.read_with(cx, |root, _| root.side_buttons[&view].handle.clone());
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(expected),
        "焦点必须位于 {view:?} 活动栏按钮"
    );
}

fn focus_side_button(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    view: SideViewId,
) {
    let button = root.read_with(cx, |root, _| root.side_buttons[&view].handle.clone());
    window.update(|window, cx| button.focus(window, cx));
    draw_frame(window);
    assert_side_button_focused(root, window, cx, view);
}

fn focus_sidebar_divider(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    divider: SidebarDivider,
) {
    let handle = root.read_with(cx, |root, _| root.side_dividers[&divider].handle.clone());
    window.update(|window, cx| handle.focus(window, cx));
    draw_frame(window);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(handle),
        "焦点必须位于 {divider:?} 分隔线"
    );
}

fn double_click(window: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    window.simulate_event(MouseDownEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    window.simulate_event(MouseUpEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
    });
}

fn open_side_button_menu(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    view: SideViewId,
) {
    focus_side_button(root, window, cx, view);
    window.simulate_keystrokes("shift-f10");
    draw_frame(window);
    assert!(window.debug_bounds("app-root-side-button-menu").is_some());
}

fn choose_side_button_menu_item(window: &mut VisualTestContext, clickable_index: usize) {
    for _ in 0..=clickable_index {
        window.simulate_keystrokes("down");
    }
    window.simulate_keystrokes("enter");
    draw_frame(window);
    assert!(window.debug_bounds("app-root-side-button-menu").is_none());
}

#[gpui::test]
fn toggle_sidebar_shortcut_preserves_visible_view_lifecycle(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-side-view-nav").is_some());
    let store = WorkspaceDatabase::open(&paths).unwrap();

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-side-view-nav").is_none());
    writer.flush().unwrap();
    let mut hidden = slterm_sidebar::SidebarState::default();
    hidden.toggle_sidebar();
    assert_eq!(
        store
            .sidebar_state()
            .unwrap()
            .map(|recovery| recovery.state),
        Some(hidden)
    );

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-side-view-nav").is_some());
    writer.flush().unwrap();
    assert_eq!(
        store
            .sidebar_state()
            .unwrap()
            .map(|recovery| recovery.state),
        Some(slterm_sidebar::SidebarState::default())
    );
}

#[gpui::test]
fn production_sidebar_restores_organization_before_first_frame(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.set_width(333);
    sidebar.toggle_sidebar();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);

    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    assert!(window.update(|window, cx| window.focused(cx)).is_none());
    assert!(window.debug_bounds("app-root-side-view-nav").is_none());
    root.read_with(cx, |root, _| {
        assert_eq!(root.side_views.state(), &sidebar);
    });
}

#[gpui::test]
fn sidebar_width_divider_keyboard_adjusts_resets_and_persists(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Width);

    for (key, expected) in [
        ("right", 266),
        ("shift-right", 330),
        ("left", 314),
        ("shift-left", 250),
        ("right", 266),
        ("enter", 250),
    ] {
        window.simulate_keystrokes(key);
        draw_frame(&mut window);
        assert_eq!(
            root.read_with(cx, |root, _| root.side_views.state().width()),
            expected,
            "{key} 必须按宽度分隔线合同调整"
        );
    }

    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(
        store.sidebar_state().unwrap().unwrap().state.width(),
        SidebarState::DEFAULT_WIDTH
    );
}

#[gpui::test]
fn sidebar_ratio_divider_keyboard_adjusts_resets_and_persists(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Ratio);

    for (key, expected) in [
        ("down", 0.52),
        ("shift-down", 0.62),
        ("up", 0.60),
        ("shift-up", 0.50),
        ("down", 0.52),
        ("enter", 0.50),
    ] {
        window.simulate_keystrokes(key);
        draw_frame(&mut window);
        let actual = root.read_with(cx, |root, _| root.side_views.state().ratio());
        assert!(
            (actual - expected).abs() <= 0.000_001,
            "{key} 必须按比例分隔线合同调整，实际为 {actual}"
        );
    }

    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(
        store.sidebar_state().unwrap().unwrap().state.ratio(),
        SidebarState::DEFAULT_RATIO
    );
}

#[gpui::test]
fn sidebar_width_divider_drag_and_double_click_reset(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-width-divider")
        .unwrap();
    let start = divider.center();

    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = gpui::point(start.x + gpui::px(40.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        290
    );
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.sidebar_state().unwrap().unwrap().state.width(), 290);
    drop(store);

    window.simulate_mouse_move(
        gpui::point(target.x + gpui::px(40.0), target.y),
        None,
        Modifiers::default(),
    );
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        290,
        "松开后不得延续布局手势"
    );
    let divider = window
        .debug_bounds("app-root-sidebar-width-divider")
        .unwrap();
    double_click(&mut window, divider.center());
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        SidebarState::DEFAULT_WIDTH
    );
}

#[gpui::test]
fn sidebar_ratio_divider_drag_and_double_click_reset(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-ratio-divider")
        .unwrap();
    let start = divider.center();
    let viewport_height = window.update(|window, _| window.viewport_size().height);

    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = gpui::point(start.x, start.y + viewport_height * 0.1);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let ratio = root.read_with(cx, |root, _| root.side_views.state().ratio());
    assert!((ratio - 0.6).abs() <= 0.000_001, "实际比例为 {ratio}");
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    let divider = window
        .debug_bounds("app-root-sidebar-ratio-divider")
        .unwrap();
    double_click(&mut window, divider.center());
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().ratio()),
        SidebarState::DEFAULT_RATIO
    );
}

#[gpui::test]
fn sidebar_layout_change_cancels_active_drag(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-width-divider")
        .unwrap();
    let start = divider.center();
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    let target = gpui::point(start.x + gpui::px(40.0), start.y);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| root.sidebar_drag.is_some()));

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);

    assert!(
        root.read_with(cx, |root, _| root.sidebar_drag.is_none()),
        "布局手势涉及的侧栏隐藏后必须释放全窗覆盖层"
    );
}

#[gpui::test]
fn sidebar_temporary_compression_does_not_overwrite(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.set_width(333);
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(500.0), gpui::px(800.0))),
    );
    draw_frame(&mut window);

    let nav = window.debug_bounds("app-root-side-view-nav").unwrap();
    assert!(
        (f32::from(nav.size.width) - 254.0).abs() <= 1.0,
        "窄窗须给主区保留 200 像素，实际侧栏宽为 {}",
        f32::from(nav.size.width)
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        333,
        "临时压缩不得改写用户宽度"
    );
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.sidebar_state().unwrap().unwrap().state.width(), 333);
}

#[gpui::test]
fn sidebar_dual_slots_keep_minimum_height_without_overwriting_ratio(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    sidebar.set_ratio(0.9).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(1000.0), gpui::px(600.0))),
    );
    draw_frame(&mut window);

    let upper = window.debug_bounds("app-root-side-view-nav").unwrap();
    let lower = window.debug_bounds("app-root-side-view-explorer").unwrap();
    assert!(f32::from(upper.size.height) >= 120.0);
    assert!(
        (f32::from(lower.size.height) - 120.0).abs() <= 1.0,
        "下槽必须保留 120 像素，实际为 {}",
        f32::from(lower.size.height)
    );
    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().ratio()),
        0.9,
        "视口夹紧不得改写用户比例"
    );
}

#[gpui::test]
fn sidebar_compressed_width_keyboard_uses_visible_width(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.set_width(333);
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(500.0), gpui::px(800.0))),
    );
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Width);

    window.simulate_keystrokes("right");
    draw_frame(&mut window);

    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        270,
        "压缩后的键盘调整必须从可见宽度起算"
    );
}

#[gpui::test]
fn sidebar_clamped_ratio_keyboard_uses_visible_ratio(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    sidebar.set_ratio(0.9).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(1000.0), gpui::px(600.0))),
    );
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Ratio);

    window.simulate_keystrokes("up");
    draw_frame(&mut window);

    let expected = 478.0 / 600.0 - 0.02;
    let actual = root.read_with(cx, |root, _| root.side_views.state().ratio());
    assert!(
        (actual - expected).abs() <= 0.000_001,
        "夹紧后的键盘调整必须从可见比例 {expected} 起算，实际为 {actual}"
    );
}

#[gpui::test]
fn sidebar_compressed_width_drag_uses_visible_width(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.set_width(333);
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(500.0), gpui::px(800.0))),
    );
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-width-divider")
        .unwrap();
    let start = divider.center();
    let target = gpui::point(start.x + gpui::px(40.0), start.y);

    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    assert_eq!(
        root.read_with(cx, |root, _| root.side_views.state().width()),
        294,
        "压缩后的拖动必须从可见宽度 254 起算"
    );
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.sidebar_state().unwrap().unwrap().state.width(), 294);
}

#[gpui::test]
fn sidebar_clamped_ratio_drag_uses_visible_ratio(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    sidebar.set_ratio(0.9).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(1000.0), gpui::px(600.0))),
    );
    draw_frame(&mut window);
    let divider = window
        .debug_bounds("app-root-sidebar-ratio-divider")
        .unwrap();
    let start = divider.center();
    let target = gpui::point(start.x, start.y - gpui::px(60.0));

    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_move(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);
    window.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    let expected = 478.0 / 600.0 - 0.1;
    let actual = root.read_with(cx, |root, _| root.side_views.state().ratio());
    assert!(
        (actual - expected).abs() <= 0.000_001,
        "夹紧后的拖动必须从可见比例起算，期望 {expected}，实际为 {actual}"
    );
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let persisted = store.sidebar_state().unwrap().unwrap().state.ratio();
    assert!((persisted - expected).abs() <= 0.000_001);
}

#[gpui::test]
fn sidebar_dual_slots_scroll_when_window_is_short(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (_root, mut window, _) = open_root_with_persistence_and_size(
        cx,
        persistence,
        Some(gpui::size(gpui::px(1000.0), gpui::px(200.0))),
    );
    draw_frame(&mut window);

    let upper = window.debug_bounds("app-root-side-view-nav").unwrap();
    let lower = window.debug_bounds("app-root-side-view-explorer").unwrap();
    assert!(f32::from(upper.size.height) >= 120.0);
    assert!(f32::from(lower.size.height) >= 120.0);
    let old_lower_y = lower.origin.y;
    window.simulate_event(gpui::ScrollWheelEvent {
        position: upper.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-40.0))),
        ..Default::default()
    });
    draw_frame(&mut window);
    let lower = window.debug_bounds("app-root-side-view-explorer").unwrap();
    assert!(
        lower.origin.y < old_lower_y,
        "高度不足时整个侧栏必须可纵向滚动，滚动前 {old_lower_y:?}，滚动后 {:?}",
        lower.origin.y
    );
}

#[gpui::test]
fn hidden_width_divider_returns_focus_to_open_view_button(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Width);

    window.update(|window, cx| {
        root.update(cx, |root, cx| root.toggle_sidebar(window, cx));
    });
    draw_frame(&mut window);

    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);
}

#[gpui::test]
fn removed_ratio_divider_returns_focus_to_closed_view_button(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Ratio);

    let button = window
        .debug_bounds("app-root-side-button-explorer")
        .unwrap();
    window.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
}

#[test]
fn activity_bar_focus_indicator_uses_accent_border() {
    let accent = gpui::hsla(0.1, 0.2, 0.3, 1.0);
    assert_eq!(
        super::super::sidebar::side_button_focus_style(accent),
        StyleRefinement::default().border_1().border_color(accent)
    );
}

#[gpui::test]
fn activity_bar_lower_group_is_anchored_to_bottom(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    let upper = window.debug_bounds("app-root-side-button-nav").unwrap();
    let lower = window
        .debug_bounds("app-root-side-button-explorer")
        .unwrap();
    let viewport = window.update(|window, _| window.viewport_size());
    assert!(lower.origin.y > upper.origin.y);
    assert!(
        (f32::from(viewport.height) - f32::from(lower.bottom()) - 8.0).abs() <= 1.0,
        "下按钮组必须贴齐活动栏底部并保留 8 像素内边距"
    );

    focus_side_button(&root, &mut window, cx, SideViewId::AgentFiles);
    window.simulate_keystrokes("down");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
    window.simulate_keystrokes("up");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::AgentFiles);
}

#[gpui::test]
fn activity_bar_tab_and_arrow_keys_follow_visual_order(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));

    window.simulate_keystrokes("tab");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);
    for (keys, view) in [
        ("down", SideViewId::Explorer),
        ("down", SideViewId::Commit),
        ("end", SideViewId::AgentFiles),
        ("up", SideViewId::Commit),
        ("home", SideViewId::Nav),
        ("up", SideViewId::Nav),
    ] {
        window.simulate_keystrokes(keys);
        draw_frame(&mut window);
        assert_side_button_focused(&root, &mut window, cx, view);
    }
}

#[gpui::test]
fn activity_bar_enter_and_space_activate_buttons(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    window.simulate_keystrokes("tab");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);

    window.simulate_keystrokes("enter");
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Nav).is_none()
    }));
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);

    window.simulate_keystrokes("space");
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Nav).is_some()
    }));
    let nav_focus = root.read_with(cx, |root, cx| {
        root.side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .read(cx)
            .focus_handle
            .clone()
            .unwrap()
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(nav_focus),
        "重新打开后焦点必须进入新 SideView"
    );
}

#[gpui::test]
fn shift_f10_opens_focused_side_button_menu(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    window.simulate_keystrokes("tab");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);

    window.simulate_keystrokes("shift-f10");
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("app-root-side-button-menu").is_some(),
        "Shift+F10 必须打开当前活动栏按钮菜单"
    );

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-side-button-menu").is_none());
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);
}

#[gpui::test]
fn side_button_menu_reorders_within_group(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let upper = slterm_sidebar::SideSlot::Upper;
    let scenarios = [
        (
            0,
            vec![
                SideViewId::Explorer,
                SideViewId::Nav,
                SideViewId::Commit,
                SideViewId::AgentFiles,
            ],
        ),
        (
            1,
            vec![
                SideViewId::Nav,
                SideViewId::Explorer,
                SideViewId::Commit,
                SideViewId::AgentFiles,
            ],
        ),
        (
            3,
            vec![
                SideViewId::Nav,
                SideViewId::Commit,
                SideViewId::AgentFiles,
                SideViewId::Explorer,
            ],
        ),
        (
            2,
            vec![
                SideViewId::Explorer,
                SideViewId::Nav,
                SideViewId::Commit,
                SideViewId::AgentFiles,
            ],
        ),
    ];
    for (item, expected) in scenarios {
        open_side_button_menu(&root, &mut window, cx, SideViewId::Explorer);
        choose_side_button_menu_item(&mut window, item);
        root.read_with(cx, |root, _| {
            assert_eq!(root.side_views.state().buttons(upper), expected);
            assert_eq!(
                root.side_views.state().open_view(upper),
                Some(SideViewId::Nav),
                "同组排序不得改变槽位内容"
            );
        });
        assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
    }
}

#[gpui::test]
fn side_button_menu_swaps_open_views_without_recreating(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let original = root.read_with(cx, |root, _| {
        (
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id(),
            root.side_views
                .instance(SideViewId::Explorer)
                .unwrap()
                .entity_id(),
        )
    });

    open_side_button_menu(&root, &mut window, cx, SideViewId::Nav);
    choose_side_button_menu_item(&mut window, 4);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.side_views
                .state()
                .open_view(slterm_sidebar::SideSlot::Upper),
            Some(SideViewId::Explorer)
        );
        assert_eq!(
            root.side_views
                .state()
                .open_view(slterm_sidebar::SideSlot::Lower),
            Some(SideViewId::Nav)
        );
        assert_eq!(
            (
                root.side_views
                    .instance(SideViewId::Nav)
                    .unwrap()
                    .entity_id(),
                root.side_views
                    .instance(SideViewId::Explorer)
                    .unwrap()
                    .entity_id(),
            ),
            original,
            "已打开视图跨槽交换必须保留双方实例"
        );
    });
}

#[gpui::test]
fn side_button_menu_moves_unopened_view_to_other_slot(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);

    open_side_button_menu(&root, &mut window, cx, SideViewId::Explorer);
    choose_side_button_menu_item(&mut window, 4);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.side_views.state().button_slot(SideViewId::Explorer),
            slterm_sidebar::SideSlot::Lower
        );
        assert_eq!(
            root.side_views.state().open_slot(SideViewId::Explorer),
            None
        );
        assert_eq!(
            root.side_views
                .state()
                .open_view(slterm_sidebar::SideSlot::Upper),
            Some(SideViewId::Nav),
            "未打开视图换区不得改变槽位内容"
        );
        assert!(root.side_views.instance(SideViewId::Explorer).is_none());
    });
}

#[gpui::test]
fn right_click_opens_same_side_button_menu(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let bounds = window
        .debug_bounds("app-root-side-button-explorer")
        .unwrap();

    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-side-button-menu").is_some());
    choose_side_button_menu_item(&mut window, 0);
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.side_views
                .state()
                .buttons(slterm_sidebar::SideSlot::Upper),
            [
                SideViewId::Explorer,
                SideViewId::Nav,
                SideViewId::Commit,
                SideViewId::AgentFiles
            ]
        );
    });
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
}

#[gpui::test]
fn clicking_menu_item_does_not_activate_trigger_button(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let button = window
        .debug_bounds("app-root-side-button-explorer")
        .unwrap();
    window.simulate_mouse_down(button.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(button.center(), MouseButton::Right, Modifiers::default());
    draw_frame(&mut window);
    let menu = window.debug_bounds("app-root-side-button-menu").unwrap();
    let first_item = gpui::point(
        menu.origin.x + menu.size.width / 2.0,
        menu.origin.y + gpui::px(18.0),
    );

    window.simulate_mouse_down(first_item, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(first_item, MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-side-button-menu").is_none());
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.side_views
                .state()
                .buttons(slterm_sidebar::SideSlot::Upper),
            [
                SideViewId::Explorer,
                SideViewId::Nav,
                SideViewId::Commit,
                SideViewId::AgentFiles
            ]
        );
        assert_eq!(
            root.side_views
                .state()
                .open_view(slterm_sidebar::SideSlot::Upper),
            Some(SideViewId::Nav),
            "菜单项目鼠标事件不得冒泡激活其按钮"
        );
        assert!(root.side_views.instance(SideViewId::Explorer).is_none());
    });
}

#[gpui::test]
fn side_button_menu_hides_button_without_closing_view(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let original = root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .entity_id()
    });

    open_side_button_menu(&root, &mut window, cx, SideViewId::Nav);
    choose_side_button_menu_item(&mut window, 5);

    assert!(window.debug_bounds("app-root-side-button-nav").is_none());
    assert!(window.debug_bounds("app-root-side-view-nav").is_some());
    root.read_with(cx, |root, _| {
        assert!(!root.side_views.state().button_is_visible(SideViewId::Nav));
        assert_eq!(
            root.side_views.state().open_slot(SideViewId::Nav),
            Some(slterm_sidebar::SideSlot::Upper)
        );
        assert_eq!(
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id(),
            original,
            "隐藏按钮不得销毁已打开视图"
        );
    });
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
}

#[gpui::test]
fn side_button_menu_hiding_last_button_focuses_previous(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);

    open_side_button_menu(&root, &mut window, cx, SideViewId::AgentFiles);
    choose_side_button_menu_item(&mut window, 5);

    assert_side_button_focused(&root, &mut window, cx, SideViewId::Commit);
}

#[gpui::test]
fn side_button_menu_hiding_only_button_focuses_workspace(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    for view in [
        SideViewId::Explorer,
        SideViewId::Commit,
        SideViewId::AgentFiles,
    ] {
        sidebar.set_button_hidden(view, true);
    }
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());

    open_side_button_menu(&root, &mut window, cx, SideViewId::Nav);
    choose_side_button_menu_item(&mut window, 5);

    assert!(root.read_with(cx, |root, _| {
        root.side_views.state().visible_buttons().is_empty()
    }));
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(workspace),
        "隐藏唯一可见按钮后必须还焦 workspace"
    );
}

#[gpui::test]
fn side_button_menu_reset_order_preserves_other_organization(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.reorder_button(SideViewId::Nav, slterm_sidebar::ButtonMovement::Bottom);
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    sidebar.set_button_hidden(SideViewId::Commit, true);
    sidebar.set_width(333);
    sidebar.set_ratio(0.7).unwrap();
    sidebar.toggle_sidebar();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    open_side_button_menu(&root, &mut window, cx, SideViewId::Nav);
    choose_side_button_menu_item(&mut window, 6);

    root.read_with(cx, |root, _| {
        let state = root.side_views.state();
        assert_eq!(
            state.buttons(slterm_sidebar::SideSlot::Upper),
            [SideViewId::Nav, SideViewId::Commit, SideViewId::AgentFiles]
        );
        assert_eq!(
            state.buttons(slterm_sidebar::SideSlot::Lower),
            [SideViewId::Explorer]
        );
        assert!(!state.button_is_visible(SideViewId::Commit));
        assert_eq!(
            state.open_view(slterm_sidebar::SideSlot::Upper),
            Some(SideViewId::Nav)
        );
        assert_eq!(
            state.open_view(slterm_sidebar::SideSlot::Lower),
            Some(SideViewId::Explorer)
        );
        assert_eq!(state.width(), 333);
        assert_eq!(state.ratio(), 0.7);
        assert!(!state.visible_intent());
    });
}

#[gpui::test]
fn side_button_menu_organization_edit_persists(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    open_side_button_menu(&root, &mut window, cx, SideViewId::Explorer);
    choose_side_button_menu_item(&mut window, 0);

    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let restored = store.sidebar_state().unwrap().unwrap().state;
    assert_eq!(
        restored.buttons(slterm_sidebar::SideSlot::Upper),
        [
            SideViewId::Explorer,
            SideViewId::Nav,
            SideViewId::Commit,
            SideViewId::AgentFiles
        ]
    );
    root.read_with(cx, |root, _| assert_eq!(root.side_views.state(), &restored));
}

#[gpui::test]
fn side_button_menu_noop_preserves_recovered_record(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .save_sidebar_state(&slterm_sidebar::SidebarState::default())
        .unwrap();
    drop(store);
    let raw_store = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    raw_store
        .execute(
            "UPDATE app_state SET sidebar_json = 'not-json' WHERE singleton = 1",
            [],
        )
        .unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);

    open_side_button_menu(&root, &mut window, cx, SideViewId::Nav);
    choose_side_button_menu_item(&mut window, 6);

    writer.flush().unwrap();
    writer.shutdown().unwrap();
    let preserved = raw_store
        .query_row(
            "SELECT sidebar_json FROM app_state WHERE singleton = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(preserved, "not-json", "无变化的重置不得覆盖恢复前记录");
}

#[gpui::test]
fn sidebar_geometry_noop_preserves_recovered_record(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .save_sidebar_state(&slterm_sidebar::SidebarState::default())
        .unwrap();
    drop(store);
    let raw_store = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    raw_store
        .execute(
            "UPDATE app_state SET sidebar_json = 'not-json' WHERE singleton = 1",
            [],
        )
        .unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    focus_sidebar_divider(&root, &mut window, cx, SidebarDivider::Width);

    window.simulate_keystrokes("enter");

    writer.flush().unwrap();
    writer.shutdown().unwrap();
    let preserved = raw_store
        .query_row(
            "SELECT sidebar_json FROM app_state WHERE singleton = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(preserved, "not-json", "无变化的几何复位不得覆盖恢复前记录");
}

#[gpui::test]
fn focus_activity_bar_prefers_open_view_then_first_visible(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));

    window.simulate_keystrokes("ctrl-shift-l");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);

    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-l");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);

    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Explorer).is_none()
    }));
    window.update(|window, cx| workspace.focus(window, cx));
    window.simulate_keystrokes("ctrl-shift-l");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Nav);
}

#[gpui::test]
fn focus_activity_bar_skips_hidden_buttons(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar.set_button_hidden(SideViewId::Nav, true);
    sidebar.close_view(SideViewId::Nav);
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));

    window.simulate_keystrokes("ctrl-shift-l");
    draw_frame(&mut window);
    assert_side_button_focused(&root, &mut window, cx, SideViewId::Explorer);
}

#[gpui::test]
fn side_view_shortcut_bindings_cover_all_registered_views(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    draw_frame(&mut window);

    for (keys, view) in [
        ("ctrl-shift-n", SideViewId::Nav),
        ("ctrl-shift-e", SideViewId::Explorer),
        ("ctrl-shift-g", SideViewId::Commit),
        ("ctrl-shift-a", SideViewId::AgentFiles),
    ] {
        window.simulate_keystrokes(keys);
        draw_frame(&mut window);
        root.read_with(cx, |root, _| {
            let expected = root.side_views.state().button_slot(view);
            assert_eq!(
                root.side_views.state().open_slot(view),
                Some(expected),
                "{keys}"
            );
        });
    }
}

#[gpui::test]
fn side_view_shortcut_focuses_closes_and_reopens_instances(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    let original = root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .entity_id()
    });

    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);
    assert_ne!(
        window.update(|window, cx| window.focused(cx)),
        Some(workspace.clone())
    );
    assert_eq!(
        root.read_with(cx, |root, _| root
            .side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .entity_id()),
        original,
        "已打开视图只需送焦，不得重建"
    );
    let nav_button = root.read_with(cx, |root, _| {
        root.side_buttons[&SideViewId::Nav].handle.clone()
    });

    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Nav).is_none()
    }));
    let returned = window.update(|window, cx| window.focused(cx));
    assert_eq!(
        returned,
        Some(nav_button),
        "关闭聚焦视图后必须还焦其可见按钮"
    );

    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    let explorer = root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Explorer)
            .unwrap()
            .entity_id()
    });
    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    window.simulate_keystrokes("ctrl-shift-e");
    draw_frame(&mut window);
    let reopened = root.read_with(cx, |root, _| {
        root.side_views
            .instance(SideViewId::Explorer)
            .unwrap()
            .entity_id()
    });
    assert_ne!(reopened, explorer, "显式关闭后重开必须创建新实例");
}

#[gpui::test]
fn toggle_sidebar_returns_focus_without_reopening_instance(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);
    let (original, button) = root.read_with(cx, |root, _| {
        (
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id(),
            root.side_buttons[&SideViewId::Nav].handle.clone(),
        )
    });

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-side-view-nav").is_none());
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(button),
        "隐藏聚焦侧栏后必须还焦可见按钮"
    );
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id()
        }),
        original
    );

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id()
        }),
        original,
        "重新显示不得重建保留实例"
    );
}

#[gpui::test]
fn clicking_side_view_focuses_and_shortcut_recognizes_it(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    let bounds = window.debug_bounds("app-root-side-view-nav").unwrap();
    window.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    let nav_focus = root.read_with(cx, |root, cx| {
        root.side_views
            .instance(SideViewId::Nav)
            .unwrap()
            .read(cx)
            .focus_handle
            .clone()
            .unwrap()
    });
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(nav_focus)
    );

    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);
    assert!(root.read_with(cx, |root, _| {
        root.side_views.instance(SideViewId::Nav).is_none()
    }));
}

#[gpui::test]
fn side_view_shortcut_restores_hidden_button_and_persists(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut initial = slterm_sidebar::SidebarState::default();
    initial.set_button_hidden(SideViewId::Nav, true);
    initial.close_view(SideViewId::Nav);
    store.save_sidebar_state(&initial).unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));
    assert!(window.debug_bounds("app-root-side-button-nav").is_none());
    assert!(window.debug_bounds("app-root-side-view-nav").is_none());

    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-side-button-nav").is_some());
    assert!(window.debug_bounds("app-root-side-view-nav").is_some());
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    let restored = store.sidebar_state().unwrap().unwrap().state;
    assert!(restored.button_is_visible(SideViewId::Nav));
    assert_eq!(
        restored.open_view(slterm_sidebar::SideSlot::Upper),
        Some(SideViewId::Nav)
    );
    root.read_with(cx, |root, _| assert_eq!(root.side_views.state(), &restored));
}

fn assert_focus_shortcut_preserves_recovered_sidebar(cx: &mut TestAppContext, raw: String) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    store
        .save_sidebar_state(&slterm_sidebar::SidebarState::default())
        .unwrap();
    let raw_store = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    raw_store
        .execute(
            "UPDATE app_state SET sidebar_json = ?1 WHERE singleton = 1",
            [&raw],
        )
        .unwrap();
    drop(store);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    let workspace = root.read_with(cx, |root, _| root.workspace_focus.clone());
    window.update(|window, cx| workspace.focus(window, cx));

    window.simulate_keystrokes("ctrl-shift-n");
    draw_frame(&mut window);
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.side_views.state().open_slot(SideViewId::Nav)
        }),
        Some(slterm_sidebar::SideSlot::Upper),
        "测试必须只触发送焦，不得改变组织状态"
    );
    writer.flush().unwrap();
    writer.shutdown().unwrap();
    let preserved = raw_store
        .query_row(
            "SELECT sidebar_json FROM app_state WHERE singleton = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(preserved, raw, "纯送焦不得覆盖启动恢复前的原始记录");
}

#[gpui::test]
fn side_view_shortcut_focus_preserves_invalid_recovery(cx: &mut TestAppContext) {
    assert_focus_shortcut_preserves_recovered_sidebar(cx, "not-json".to_owned());
}

#[gpui::test]
fn side_view_shortcut_focus_preserves_field_repair(cx: &mut TestAppContext) {
    let mut custom = slterm_sidebar::SidebarState::default();
    custom.set_width(333);
    let invalid_width = custom
        .to_canonical_json()
        .unwrap()
        .replace(r#""width":333"#, r#""width":100"#);
    assert_focus_shortcut_preserves_recovered_sidebar(cx, invalid_width);
}

#[gpui::test]
fn production_sidebar_dual_slots_preserve_instances_across_hide(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    let mut sidebar = slterm_sidebar::SidebarState::default();
    sidebar
        .move_button_to_slot(SideViewId::Explorer, slterm_sidebar::SideSlot::Lower, 0)
        .unwrap();
    sidebar.click_button(SideViewId::Explorer).unwrap();
    store.save_sidebar_state(&sidebar).unwrap();
    drop(store);
    let persistence = SideViewPersistence::load(&paths, None);
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let original = root.read_with(cx, |root, _| {
        (
            root.side_views
                .instance(SideViewId::Nav)
                .unwrap()
                .entity_id(),
            root.side_views
                .instance(SideViewId::Explorer)
                .unwrap()
                .entity_id(),
        )
    });
    draw_frame(&mut window);

    let upper = window.debug_bounds("app-root-side-view-nav").unwrap();
    let lower = window.debug_bounds("app-root-side-view-explorer").unwrap();
    let viewport = window.update(|window, _| window.viewport_size());
    assert!(upper.origin.y < lower.origin.y);
    assert!(
        (f32::from(upper.size.height) - f32::from(viewport.height) * 0.5).abs() <= 1.0,
        "上槽必须采用组织状态比例"
    );
    let expected_lower =
        f32::from(viewport.height) - f32::from(upper.size.height) - slterm_split::DIVIDER_GAP;
    assert!(
        (f32::from(lower.size.height) - expected_lower).abs() <= 1.0,
        "下槽必须接收扣除比例槽与分隔线后的空间"
    );

    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-side-view-nav").is_none());
    assert!(window.debug_bounds("app-root-side-view-explorer").is_none());
    window.simulate_keystrokes("ctrl-shift-b");
    draw_frame(&mut window);

    let restored = window.update(|_, cx| {
        root.update(cx, |root, _| {
            (
                root.side_views
                    .instance(SideViewId::Nav)
                    .unwrap()
                    .entity_id(),
                root.side_views
                    .instance(SideViewId::Explorer)
                    .unwrap()
                    .entity_id(),
            )
        })
    });
    assert_eq!(restored, original, "隐藏整个侧栏不得重建任何槽位实体");
}

#[gpui::test]
fn production_sidebar_button_replaces_visible_view(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let registry = super::super::side_view::side_view_state_registry();
    let persistence = SideViewPersistence::load(&paths, Some(writer.client()));
    let (_root, mut window, _) = open_root_with_persistence(cx, persistence);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("app-root-side-view-nav").is_some(),
        "首次使用必须呈现真实导航树实例"
    );
    let button = window
        .debug_bounds("app-root-side-button-explorer")
        .expect("活动栏必须呈现文件浏览器按钮");
    window.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::default());
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-side-view-nav").is_none());
    assert!(window.debug_bounds("app-root-side-view-explorer").is_some());
    writer.flush().unwrap();
    let store = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        store
            .view_state(
                &ViewKey::SideView {
                    side_view_id: SideViewId::Nav,
                },
                &registry,
            )
            .unwrap()
            .is_some(),
        "同槽关闭必须经生产 writer 保存被替换视图的私有状态"
    );
    let sidebar = store
        .sidebar_state()
        .unwrap()
        .expect("生产按钮必须保存侧栏组织状态");
    assert_eq!(
        sidebar.state.open_view(slterm_sidebar::SideSlot::Upper),
        Some(SideViewId::Explorer)
    );
}
