//! 设置页 AgentRoot 管理：列表、改名、能力开关、删除与新增的重复拒绝。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::Modifiers;
use slterm_settings::AgentRoot;

use super::*;
use crate::platform::windows_integration::FakeWindowsIntegration;

fn root_id(seed: u64) -> slterm_settings::AgentRootId {
    let mut bytes = *uuid::Uuid::now_v7().as_bytes();
    bytes[15] ^= seed as u8;
    slterm_settings::AgentRootId::from_bytes(bytes).unwrap()
}

fn custom_root(seed: u64, name: &str, path: &std::path::Path) -> AgentRoot {
    AgentRoot::custom(
        root_id(seed),
        name.to_owned(),
        path.to_string_lossy().into_owned(),
    )
    .unwrap()
}

fn open_agent_roots(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
    super::settings_pages::focus_workspace(root, window);
    super::settings_pages::open_settings(window);
    draw_frame(window);
    let page = window.debug_bounds("settings-page-agent-basic").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(window);
    let open = window.debug_bounds("settings-open-agent-roots").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(window);
}

fn wait_until(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    cx: &TestAppContext,
    mut predicate: impl FnMut(&AppRoot) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        if root.read_with(cx, |root, _| predicate(root)) {
            return;
        }
        assert!(Instant::now() < deadline, "等待 AgentRoot 状态超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Agent 基础页提供根管理入口并列出已登记自定义根，Esc 关闭。
#[gpui::test]
fn agent_basic_page_opens_agent_roots_dialog(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    let custom_entry = custom_root(1, "Custom", &custom);
    let (root_entity, mut window, _state) = open_root(cx);
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_entry]);
    });
    open_agent_roots(&root_entity, &mut window);

    assert!(window.debug_bounds("agent-roots-dialog").is_some());
    assert!(window.debug_bounds("agent-root-row-0").is_some());
    assert!(window.debug_bounds("agent-root-add").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("agent-roots-dialog").is_none());
}

/// 改名经统一设置写通道落盘后才采纳运行值。
#[gpui::test]
fn agent_roots_dialog_renames_root_after_save(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    let (root_entity, mut window, _state) = open_root(cx);
    let saved = Arc::new(Mutex::new(Vec::<Vec<AgentRoot>>::new()));
    let captured = Arc::clone(&saved);
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_root(1, "Before", &custom)]);
        root.set_agent_roots_save_for_test(Arc::new(move |roots| {
            captured.lock().unwrap().push(roots.to_vec());
            Ok(())
        }));
    });
    open_agent_roots(&root_entity, &mut window);

    let rename = window.debug_bounds("agent-root-rename-0").unwrap();
    window.simulate_click(rename.center(), Modifiers::default());
    draw_frame(&mut window);
    window.update(|window, cx| {
        root_entity.update(cx, |root, cx| {
            root.set_agent_root_rename_for_test("After", window, cx);
        });
    });
    let save = window.debug_bounds("agent-root-rename-save").unwrap();
    window.simulate_click(save.center(), Modifiers::default());
    wait_until(&root_entity, &mut window, cx, |root| {
        root.custom_agent_roots
            .first()
            .is_some_and(|root| root.display_name() == "After")
    });
    let saves = saved.lock().unwrap();
    assert_eq!(
        saves.last().unwrap()[0].display_name(),
        "After",
        "落盘候选必须携带新名称"
    );
}

/// 能力开关写盘成功后推进授权代次。
#[gpui::test]
fn agent_roots_dialog_capability_toggle_advances_authorization(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    let (root_entity, mut window, _state) = open_root(cx);
    let saved = Arc::new(Mutex::new(Vec::<Vec<AgentRoot>>::new()));
    let captured = Arc::clone(&saved);
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_root(1, "Custom", &custom)]);
        root.set_agent_roots_save_for_test(Arc::new(move |roots| {
            captured.lock().unwrap().push(roots.to_vec());
            Ok(())
        }));
    });
    open_agent_roots(&root_entity, &mut window);
    let before = crate::app_shell::authorization::current_generation();

    let toggle = window.debug_bounds("agent-root-write-0").unwrap();
    window.simulate_click(toggle.center(), Modifiers::default());
    wait_until(&root_entity, &mut window, cx, |root| {
        root.custom_agent_roots
            .first()
            .is_some_and(|root| root.capabilities().write)
    });
    assert!(
        saved.lock().unwrap().last().unwrap()[0]
            .capabilities()
            .write
    );
    assert!(
        crate::app_shell::authorization::current_generation() > before,
        "能力变化必须推进授权代次"
    );
}

/// 删除根后列表与落盘候选同步移除。
#[gpui::test]
fn agent_roots_dialog_deletes_root(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    let (root_entity, mut window, _state) = open_root(cx);
    let saved = Arc::new(Mutex::new(Vec::<Vec<AgentRoot>>::new()));
    let captured = Arc::clone(&saved);
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_root(1, "Custom", &custom)]);
        root.set_agent_roots_save_for_test(Arc::new(move |roots| {
            captured.lock().unwrap().push(roots.to_vec());
            Ok(())
        }));
    });
    open_agent_roots(&root_entity, &mut window);

    let delete = window.debug_bounds("agent-root-delete-0").unwrap();
    window.simulate_click(delete.center(), Modifiers::default());
    wait_until(&root_entity, &mut window, cx, |root| {
        root.custom_agent_roots.is_empty()
    });
    assert!(saved.lock().unwrap().last().unwrap().is_empty());
    draw_frame(&mut window);
    assert!(window.debug_bounds("agent-root-row-0").is_none());
}

/// 自定义根的运行时清单从根管理对话框编辑并按根身份持久。
#[gpui::test]
fn agent_roots_dialog_edits_custom_root_runtime_files(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("custom");
    std::fs::create_dir(&custom).unwrap();
    let custom_entry = custom_root(1, "Custom", &custom);
    let custom_id = custom_entry.id();
    let (root_entity, mut window, _state) = open_root(cx);
    let saved = Arc::new(Mutex::new(Vec::<Vec<AgentRoot>>::new()));
    let captured = Arc::clone(&saved);
    let (_, config_hook) = super::agent_files_ops::saved_configs();
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_entry]);
        root.set_agent_roots_save_for_test(Arc::new(move |roots| {
            captured.lock().unwrap().push(roots.to_vec());
            Ok(())
        }));
        root.set_agent_files_save_for_test(config_hook);
    });
    open_agent_roots(&root_entity, &mut window);

    let runtime = window.debug_bounds("agent-root-runtime-0").unwrap();
    window.simulate_click(runtime.center(), Modifiers::default());
    draw_frame(&mut window);
    window.update(|window, cx| {
        root_entity.update(cx, |root, cx| {
            root.set_agent_root_runtime_input_for_test("settings.json", window, cx);
        });
    });
    click_selector(&mut window, "agent-root-runtime-add");
    wait_until(&root_entity, &mut window, cx, |root| {
        root.agent_files_config.runtime_names(custom_id) == ["settings.json"]
    });
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("agent-root-runtime-chip-0").is_some(),
        "新增运行时清单项必须出现在编辑面"
    );
}

fn click_selector(window: &mut VisualTestContext, selector: &'static str) {
    let bounds = window.debug_bounds(selector).unwrap();
    window.simulate_click(bounds.center(), Modifiers::default());
    draw_frame(window);
}

/// 新增根经目录选择与真实查重；重复目录拒绝且不写盘。
#[gpui::test]
fn agent_roots_dialog_adds_root_and_rejects_duplicate(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    for directory in [&first, &second] {
        std::fs::create_dir(directory).unwrap();
    }
    let windows = Arc::new(FakeWindowsIntegration::default());
    let (root_entity, mut window) =
        super::nav::open_root_with_windows(cx, AppRootPersistence::empty(None), windows.clone());
    let saved = Arc::new(Mutex::new(Vec::<Vec<AgentRoot>>::new()));
    let captured = Arc::clone(&saved);
    root_entity.update(cx, |root, _| {
        root.replace_agent_global_dirs_for_test(Vec::new());
        root.replace_custom_agent_roots_for_test(vec![custom_root(1, "Existing", &first)]);
        root.set_agent_roots_save_for_test(Arc::new(move |roots| {
            captured.lock().unwrap().push(roots.to_vec());
            Ok(())
        }));
    });
    open_agent_roots(&root_entity, &mut window);

    windows.set_folder_target(Some(second));
    let add = window.debug_bounds("agent-root-add").unwrap();
    window.simulate_click(add.center(), Modifiers::default());
    wait_until(&root_entity, &mut window, cx, |root| {
        root.custom_agent_roots.len() == 2
    });
    assert_eq!(saved.lock().unwrap().last().unwrap().len(), 2);

    windows.set_folder_target(Some(first));
    let add = window.debug_bounds("agent-root-add").unwrap();
    window.simulate_click(add.center(), Modifiers::default());
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("agent-roots-error").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "等待重复目录提示超时");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        root_entity.read_with(cx, |root, _| root.custom_agent_roots.len()),
        2,
        "重复目录不得新增根"
    );
    assert_eq!(saved.lock().unwrap().len(), 1, "重复候选不得写盘");
}
