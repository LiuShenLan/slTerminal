//! 设置页主题库：内置与自定义主题的选择、导入、改名与删除前置条件。

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::Modifiers;

use super::*;
use crate::platform::windows_integration::FakeWindowsIntegration;
use crate::theme_library::ThemeLibraryStore;

fn theme_fixture() -> (tempfile::TempDir, std::path::PathBuf, ThemeLibraryStore) {
    let directory = tempfile::tempdir().unwrap();
    let themes = directory.path().join("themes");
    let store = ThemeLibraryStore::new(&themes);
    (directory, themes, store)
}

fn import_theme(store: &ThemeLibraryStore, name: &str, background: [u8; 3]) -> (String, String) {
    let mut definition =
        slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
    definition.name = name.to_owned();
    definition.terminal.background = background;
    let document = store
        .import(&crate::theme_library::from_definition(&definition).unwrap())
        .unwrap();
    (
        document.theme_id().unwrap().to_string(),
        document.name().to_owned(),
    )
}

fn pump_until(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    mut predicate: impl FnMut(&AppRoot, &gpui::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        let reached = root.read_with(cx, |root, app| predicate(root, app));
        if reached {
            return;
        }
        assert!(Instant::now() < deadline, "等待主题库状态超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn open_theme_library(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    super::settings_pages::focus_workspace(root, window);
    super::settings_pages::open_settings(window);
    draw_frame(window);
    let page = window.debug_bounds("settings-page-appearance").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(window);
    let open = window.debug_bounds("settings-open-theme-library").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(window);
}

fn rows(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
) -> Vec<crate::app_shell::app_root::settings_theme_library::ThemeLibraryRowView> {
    root.read_with(cx, |root, _| {
        root.theme_library_rows_for_test()
            .expect("主题库对话框必须存在")
    })
}

/// 外观页提供主题库入口；列表同时包含内置与自定义主题，Esc 关闭。
#[gpui::test]
fn settings_appearance_page_offers_theme_library_with_builtin_and_custom_rows(
    cx: &mut TestAppContext,
) {
    let (_directory, themes, store) = theme_fixture();
    let (_id, name) = import_theme(&store, "Custom One", [0x11, 0x22, 0x33]);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == name))
    });

    let rows = rows(&root, cx);
    assert!(
        rows.iter()
            .any(|row| row.builtin && row.name == slterm_settings::ThemeName::Linear.prompt_name()),
        "内置主题必须列出"
    );
    assert!(
        rows.iter()
            .any(|row| !row.builtin && row.name == name && row.revision == 1),
        "自定义主题行携带库内修订"
    );
    assert!(window.debug_bounds("theme-library-import").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("theme-library-dialog").is_none());
}

/// 点击自定义主题行即写入 custom_theme 并热应用其调色板。
#[gpui::test]
fn theme_library_selects_custom_theme_and_marks_it_active(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let (id, name) = import_theme(&store, "Selected", [0x44, 0x55, 0x66]);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == name))
    });
    let index = rows(&root, cx)
        .iter()
        .position(|row| row.name == name)
        .unwrap();
    let row = window
        .debug_bounds(Box::leak(
            format!("theme-library-row-{index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(row.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, app| {
        root.settings.runtime().custom_theme.as_deref() == Some(id.as_str())
            && app
                .global::<crate::app_shell::config::Settings>()
                .palette
                .background
                == crate::gpui_terminal::colors::from_rgb8([0x44, 0x55, 0x66])
    });
    assert!(
        rows(&root, cx).iter().any(|row| row.id == id && row.active),
        "选中行必须标记为当前主题"
    );
}

/// 当前自定义主题必须先另选才能删除。
#[gpui::test]
fn theme_library_rejects_deleting_the_active_custom_theme(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let (id, name) = import_theme(&store, "Protected", [0x77, 0x88, 0x99]);
    let path = store.path_for(id.parse().unwrap()).unwrap();
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == name))
    });
    let index = rows(&root, cx)
        .iter()
        .position(|row| row.name == name)
        .unwrap();
    let row = window
        .debug_bounds(Box::leak(
            format!("theme-library-row-{index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(row.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, _| {
        root.settings.runtime().custom_theme.as_deref() == Some(id.as_str())
    });

    let delete = window
        .debug_bounds(Box::leak(
            format!("theme-library-delete-{index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(delete.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("theme-library-error").is_some(),
        "删除当前主题必须显示封闭错误文案"
    );
    assert!(path.exists(), "拒绝删除时不得改动主题文件");
}

/// 改名经库内冲突与修订规则落盘，列表刷新为新名称。
#[gpui::test]
fn theme_library_renames_custom_theme_and_refreshes_rows(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let (id, name) = import_theme(&store, "Before", [0x11, 0x22, 0x33]);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == name))
    });
    let index = rows(&root, cx)
        .iter()
        .position(|row| row.name == name)
        .unwrap();
    let rename = window
        .debug_bounds(Box::leak(
            format!("theme-library-rename-{index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(rename.center(), Modifiers::default());
    draw_frame(&mut window);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.set_theme_library_rename_for_test("After", window, cx);
        });
    });
    let save = window.debug_bounds("theme-library-rename-save").unwrap();
    window.simulate_click(save.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_rows_for_test()
            .is_some_and(|rows| rows.iter().any(|row| row.id == id && row.name == "After"))
    });
    assert_eq!(store.load(&id).unwrap().name(), "After");
    assert_eq!(store.load(&id).unwrap().revision(), 2);
}

/// 非当前自定义主题可删除，删除后列表与文件同步消失。
#[gpui::test]
fn theme_library_deletes_inactive_custom_theme(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let (active_id, active_name) = import_theme(&store, "Active", [0x11, 0x22, 0x33]);
    let (_id, doomed_name) = import_theme(&store, "Doomed", [0x44, 0x55, 0x66]);
    let doomed_path = store
        .list()
        .unwrap()
        .custom
        .into_iter()
        .find(|document| document.name() == doomed_name)
        .map(|document| store.path_for(document.theme_id().unwrap()).unwrap())
        .unwrap();
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == doomed_name))
    });
    let rows = rows(&root, cx);
    let active_index = rows.iter().position(|row| row.name == active_name).unwrap();
    let row = window
        .debug_bounds(Box::leak(
            format!("theme-library-row-{active_index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(row.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, _| {
        root.settings.runtime().custom_theme.as_deref() == Some(active_id.as_str())
    });
    let doomed_index = rows.iter().position(|row| row.name == doomed_name).unwrap();
    let delete = window
        .debug_bounds(Box::leak(
            format!("theme-library-delete-{doomed_index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(delete.center(), Modifiers::default());
    draw_frame(&mut window);
    // 首次点击只进入确认态：零删除、零加载
    assert_eq!(
        root.read_with(cx, |root, _| root.theme_library_delete_confirm_for_test()),
        Some(rows[doomed_index].id.clone()),
        "首次点击必须记录待确认主题"
    );
    assert!(doomed_path.exists(), "首次点击不得删除主题文件");
    assert!(
        root.read_with(cx, |root, _| root.theme_library_loading_for_test()) == Some(false),
        "首次点击不得启动删除任务"
    );
    window.simulate_click(delete.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().all(|row| row.name != doomed_name))
    });
    assert!(!doomed_path.exists(), "删除必须移除主题文件");
    assert!(
        root.read_with(cx, |root, _| root.theme_library_delete_confirm_for_test())
            .is_none(),
        "删除完成后必须清除确认态"
    );
}

/// 选择其他主题会离开删除确认语境，避免跨行沿用二次确认。
#[gpui::test]
fn theme_library_delete_confirmation_resets_on_selection_change(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let (active_id, active_name) = import_theme(&store, "Active", [0x11, 0x22, 0x33]);
    let (_id, doomed_name) = import_theme(&store, "Doomed", [0x44, 0x55, 0x66]);
    let (root, mut window, _state) = open_root(cx);
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
            && root
                .theme_library_rows_for_test()
                .is_some_and(|rows| rows.iter().any(|row| row.name == doomed_name))
    });
    let rows = rows(&root, cx);
    let doomed_index = rows.iter().position(|row| row.name == doomed_name).unwrap();
    let delete = window
        .debug_bounds(Box::leak(
            format!("theme-library-delete-{doomed_index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(delete.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(
        root.read_with(cx, |root, _| root.theme_library_delete_confirm_for_test())
            .is_some(),
        "首次点击必须进入确认态"
    );
    let active_index = rows.iter().position(|row| row.name == active_name).unwrap();
    let active_row = window
        .debug_bounds(Box::leak(
            format!("theme-library-row-{active_index}").into_boxed_str(),
        ))
        .unwrap();
    window.simulate_click(active_row.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, _| {
        root.settings.runtime().custom_theme.as_deref() == Some(active_id.as_str())
    });
    assert!(
        root.read_with(cx, |root, _| root.theme_library_delete_confirm_for_test())
            .is_none(),
        "选择其他主题必须清除删除确认态"
    );
}

/// 导入合法文件即入库并选中；非法文件零写入并保留对话框错误。
#[gpui::test]
fn theme_library_imports_picked_file_and_rejects_invalid_json(cx: &mut TestAppContext) {
    let (_directory, themes, store) = theme_fixture();
    let source_directory = tempfile::tempdir().unwrap();
    let mut definition =
        slterm_settings::ThemeDefinition::from_builtin(slterm_settings::ThemeName::Nord);
    definition.name = "Imported".to_owned();
    definition.terminal.background = [0x22, 0x33, 0x44];
    let valid = source_directory.path().join("valid.json");
    std::fs::write(
        &valid,
        crate::theme_library::from_definition(&definition)
            .unwrap()
            .to_json_bytes()
            .unwrap(),
    )
    .unwrap();
    let invalid = source_directory.path().join("invalid.json");
    std::fs::write(&invalid, b"{ not json").unwrap();

    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_open_file_target(Some(valid));
    let (root, mut window) =
        super::nav::open_root_with_windows(cx, AppRootPersistence::empty(None), windows.clone());
    root.update(cx, |root, _| {
        root.theme_root_for_test = Some(themes.clone())
    });
    open_theme_library(&root, &mut window);
    pump_until(&root, &mut window, cx, |root, _| {
        root.theme_library_loading_for_test() == Some(false)
    });
    let import = window.debug_bounds("theme-library-import").unwrap();
    window.simulate_click(import.center(), Modifiers::default());
    pump_until(&root, &mut window, cx, |root, app| {
        root.theme_library_rows_for_test()
            .is_some_and(|rows| rows.iter().any(|row| row.name == "Imported"))
            && root.settings.runtime().custom_theme.is_some()
            && app
                .global::<crate::app_shell::config::Settings>()
                .palette
                .background
                == crate::gpui_terminal::colors::from_rgb8([0x22, 0x33, 0x44])
    });
    assert_eq!(store.list().unwrap().custom.len(), 1, "导入只新增一份主题");

    windows.set_open_file_target(Some(invalid));
    let import = window.debug_bounds("theme-library-import").unwrap();
    window.simulate_click(import.center(), Modifiers::default());
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("theme-library-error").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "等待导入失败提示超时");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(store.list().unwrap().custom.len(), 1, "非法导入零写入");
}
