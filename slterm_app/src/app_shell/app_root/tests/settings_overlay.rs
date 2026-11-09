//! 无项目设置浮层的应用级所有权与焦点测试。

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

#[gpui::test]
fn settings_overlay_explicit_close_releases_instance_and_allows_reopen(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    let (entity, focus) = root.read_with(cx, |root, cx| {
        let view = &root.settings_overlay.as_ref().unwrap().view;
        (view.downgrade(), view.read(cx).focus_handle.clone())
    });

    window.simulate_keystrokes("escape");
    window.run_until_parked();

    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_none()));
    assert!(entity.upgrade().is_none());
    assert_ne!(window.update(|window, cx| window.focused(cx)), Some(focus));

    open_settings(&mut window);
    assert!(root.read_with(cx, |root, _| root.settings_overlay.is_some()));
}

#[gpui::test]
fn app_overlay_does_not_close_on_project_activation(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    assert!(root.read_with(cx, |root, _| root.snapshot().registry().active().is_none()));

    focus_workspace(&root, &mut window);
    open_settings(&mut window);

    let (entity_id, focus, key) = root.read_with(cx, |root, cx| {
        let host = root.settings_overlay.as_ref().expect("必须创建设置浮层");
        (
            host.view.entity_id(),
            host.view.read(cx).focus_handle.clone(),
            host.view.read(cx).view_key(),
        )
    });
    assert_eq!(key, ViewKey::AppSettingsOverlay);
    assert_eq!(
        window.update(|window, cx| window.focused(cx)),
        Some(focus.clone())
    );

    open_settings(&mut window);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings_overlay.as_ref().unwrap().view.entity_id(),
            entity_id,
            "重复入口不得重置既有设置视图"
        );
    });
    assert_eq!(window.update(|window, cx| window.focused(cx)), Some(focus));

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");

    root.read_with(cx, |root, cx| {
        let host = root
            .settings_overlay
            .as_ref()
            .expect("项目激活不得销毁浮层");
        assert_eq!(host.view.entity_id(), entity_id);
        assert_eq!(host.view.read(cx).view_key(), ViewKey::AppSettingsOverlay);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_B))
        );
    });
}
