//! 设置中心页面行为：导航分组与过滤、快捷键目录、后台任务与恢复默认。

use slterm_settings::json::Value;

use super::nav::open_root_with_windows;
use super::*;

pub(super) fn open_settings(window: &mut VisualTestContext) {
    window.simulate_keystrokes("ctrl-,");
    draw_frame(window);
}

pub(super) fn focus_workspace(root: &Entity<AppRoot>, window: &mut VisualTestContext) {
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

/// 导出设置：后台选择目标并写出可解析的版本化信封。
#[gpui::test]
fn settings_export_writes_versioned_envelope(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("settings-export.json");
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(target.clone()));
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    root.update(cx, |root, cx| {
        root.apply_settings_field("theme", Value::String("Nord".to_owned()), cx);
    });
    draw_frame(&mut window);

    let export = window.debug_bounds("settings-export").unwrap();
    window.simulate_click(export.center(), Modifiers::default());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }

    let bytes = std::fs::read(&target).expect("导出目标必须写盘");
    let parsed = crate::settings_transfer::parse_export(&bytes).expect("导出信封必须可解析");
    assert_eq!(
        parsed.version,
        crate::settings_transfer::SETTINGS_EXPORT_VERSION
    );
    assert_eq!(
        parsed.settings.get("theme"),
        Some(&Value::String("Nord".to_owned())),
        "导出必须包含当前生效的非敏感字段"
    );
}

/// 导入设置：预检后先预览，确认写盘成功才应用运行值。
#[gpui::test]
fn settings_import_previews_and_applies_after_write(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("settings-import.json");
    let export = crate::settings_transfer::SettingsExport {
        version: crate::settings_transfer::SETTINGS_EXPORT_VERSION,
        settings: std::collections::BTreeMap::from([(
            "theme".to_owned(),
            Value::String("Nord".to_owned()),
        )]),
    };
    std::fs::write(&source, crate::settings_transfer::export_json(&export)).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_open_file_target(Some(source));
    let (root, mut window) = open_root_with_windows(cx, AppRootPersistence::empty(None), windows);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);

    let import = window.debug_bounds("settings-import").unwrap();
    window.simulate_click(import.center(), Modifiers::default());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-import-preview").is_some(),
        "确认前必须显示导入预览"
    );
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings.values().persisted_value("theme"),
            None,
            "确认前不得改变运行值"
        );
    });

    let confirm = window.debug_bounds("settings-import-confirm").unwrap();
    window.simulate_click(confirm.center(), Modifiers::default());
    for _ in 0..6 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    draw_frame(&mut window);

    root.read_with(cx, |root, _| {
        assert_eq!(
            root.settings.values().persisted_value("theme"),
            Some(&Value::String("Nord".to_owned())),
            "写盘成功后必须应用导入值"
        );
    });
    assert!(
        window.debug_bounds("settings-import-preview").is_none(),
        "确认后预览必须收合"
    );
}

/// 任务周期：合法整数提交并落段，非法值保留草稿与错误标记。
#[gpui::test]
fn settings_task_interval_accepts_valid_and_rejects_out_of_range(cx: &mut TestAppContext) {
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

    let settings = root.read_with(cx, |root, _| {
        root.settings_overlay.as_ref().unwrap().view.clone()
    });
    window.update(|window, cx| {
        settings.update(cx, |view, cx| {
            view.set_task_interval_for_test(
                crate::background_tasks::PLAN_BALANCE,
                "120",
                window,
                cx,
            );
        });
    });
    assert_eq!(
        crate::background_tasks::interval_sec(crate::background_tasks::PLAN_BALANCE),
        120
    );

    window.update(|window, cx| {
        settings.update(cx, |view, cx| {
            view.set_task_interval_for_test(crate::background_tasks::PLAN_BALANCE, "5", window, cx);
        });
    });
    assert_eq!(
        crate::background_tasks::interval_sec(crate::background_tasks::PLAN_BALANCE),
        120,
        "越界周期不得生效"
    );
    settings.read_with(cx, |view, cx| {
        assert!(
            view.task_interval_errors
                .contains(crate::background_tasks::PLAN_BALANCE)
        );
        assert_eq!(
            view.task_inputs
                .get(crate::background_tasks::PLAN_BALANCE)
                .unwrap()
                .read(cx)
                .value()
                .to_string(),
            "5",
            "非法草稿必须保留"
        );
    });
}

/// 设置「应用」页提供加密备份入口，类别与口令面可见。
#[gpui::test]
fn settings_application_page_offers_backup_dialog(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-open-backup").is_none());

    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-backup").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("backup-dialog").is_some());
    assert!(window.debug_bounds("backup-category-settings").is_some());
    assert!(window.debug_bounds("backup-category-workspace").is_some());
    assert!(window.debug_bounds("backup-export").is_some());

    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("backup-dialog").is_none());
}

/// 加密备份导出：短口令拒绝，合法口令写出可解封归档。
#[gpui::test]
fn backup_export_rejects_short_passphrase_and_writes_archive(cx: &mut TestAppContext) {
    use crate::persistence::workspace_writer::WorkspaceWriter;
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("workspace.sltmbk");
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(target.clone()));
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window) = open_root_with_windows(cx, persistence, windows);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-backup").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.set_backup_passphrase_for_test("short", window, cx);
        });
    });
    let export = window.debug_bounds("backup-export").unwrap();
    window.simulate_click(export.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(!target.exists(), "短口令不得写盘");
    assert!(window.debug_bounds("backup-dialog").is_some());

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.set_backup_passphrase_for_test("correct horse", window, cx);
        });
    });
    let export = window.debug_bounds("backup-export").unwrap();
    window.simulate_click(export.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if target.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let bytes = std::fs::read(&target).expect("备份目标必须写盘");
    let archive = crate::encrypted_backup::open(&bytes, "correct horse").expect("归档可解封");
    assert!(
        archive
            .manifest
            .categories
            .contains(&crate::encrypted_backup::BackupCategory::Settings)
    );
    assert!(
        archive
            .manifest
            .categories
            .contains(&crate::encrypted_backup::BackupCategory::Workspace)
    );
    let workspace_entry = archive
        .entries
        .iter()
        .find(|entry| entry.name == crate::encrypted_backup::workspace::WORKSPACE_ENTRY_NAME)
        .expect("工作区条目必须存在");
    let workspace =
        crate::encrypted_backup::workspace::decode_workspace(&workspace_entry.bytes).unwrap();
    assert_eq!(
        workspace.source_machine,
        crate::platform::machine::machine_key()
    );
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("backup-dialog").is_none(),
        "成功后关闭对话框"
    );
}

/// 加密备份恢复：选择文件、口令预览、登记维护请求并提示重启生效。
#[gpui::test]
fn backup_restore_previews_and_stages_maintenance_request(cx: &mut TestAppContext) {
    use crate::encrypted_backup::{BackupArchive, BackupCategory, BackupEntry, BackupManifest};
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriter;
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    {
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run(
                "epoch-backup-ui",
                RunMarker {
                    run_id: "run-backup-ui".to_owned(),
                    process_id: 3,
                    process_created_at_ms: 5,
                },
            )
            .unwrap();
    }
    let archive = BackupArchive {
        manifest: BackupManifest {
            version: 1,
            categories: vec![BackupCategory::Settings],
        },
        entries: vec![BackupEntry {
            category: BackupCategory::Settings,
            name: "settings.json".to_owned(),
            bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
        }],
    };
    let packet = crate::encrypted_backup::seal(&archive, "correct horse").unwrap();
    let target = temp.path().join("restore.sltmbk");
    std::fs::write(&target, &packet).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_open_file_target(Some(target));
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window) = open_root_with_windows(cx, persistence, windows);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-backup").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);

    let restore = window.debug_bounds("backup-restore").unwrap();
    window.simulate_click(restore.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("backup-restore-confirm").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.set_backup_passphrase_for_test("correct horse", window, cx);
        });
    });
    let confirm = window.debug_bounds("backup-restore-confirm").unwrap();
    window.simulate_click(confirm.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("backup-restore-preview").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("backup-restore-preview").is_some(),
        "口令正确后必须显示恢复预览"
    );

    let confirm = window.debug_bounds("backup-restore-confirm").unwrap();
    window.simulate_click(confirm.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("backup-restore-staged").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("backup-restore-staged").is_some(),
        "登记成功必须提示下次启动生效"
    );
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    let request = observer
        .unfinished_maintenance_requests()
        .unwrap()
        .into_iter()
        .find(|request| request.kind == "backup.restore")
        .expect("必须登记维护请求");
    assert_eq!(
        request.status,
        crate::persistence::maintenance_request::MaintenanceRequestStatus::Prepared
    );
    drop(observer);
    drop(writer);
}

/// 已完成恢复的恢复点在对话框中可见并可登记撤回请求。
#[gpui::test]
fn backup_undo_lists_applied_point_and_stages_request(cx: &mut TestAppContext) {
    use crate::encrypted_backup::maintenance::{
        BACKUP_RESTORE_KIND, BackupRestoreRecoverer, stage_restore,
    };
    use crate::encrypted_backup::{
        BackupArchive, BackupCategory, BackupEntry, BackupManifest, BackupSelection,
    };
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::startup::{
        JournalRecovererRegistry, MaintenanceOutcome, MaintenanceRequestRecoverer,
        MaintenanceRequestRecovererRegistry, MaintenanceRunner,
    };
    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriter;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    {
        std::fs::create_dir_all(paths.config_root()).unwrap();
        std::fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .begin_run(
                "epoch-undo-ui",
                RunMarker {
                    run_id: "run-undo-ui".to_owned(),
                    process_id: 7,
                    process_created_at_ms: 13,
                },
            )
            .unwrap();
        let archive = BackupArchive {
            manifest: BackupManifest {
                version: 1,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![BackupEntry {
                category: BackupCategory::Settings,
                name: "settings.json".to_owned(),
                bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
            }],
        };
        stage_restore(
            &mut store,
            &paths,
            &archive,
            BackupSelection::from_categories([BackupCategory::Settings]),
        )
        .unwrap();
        let recoverer = BackupRestoreRecoverer::new(
            paths.clone(),
            Arc::new(slterm_workspace::ViewStateRegistry::new(&[]).unwrap()),
        );
        let journals = JournalRecovererRegistry::default();
        let requests = MaintenanceRequestRecovererRegistry::from([(
            BACKUP_RESTORE_KIND,
            &recoverer as &dyn MaintenanceRequestRecoverer,
        )]);
        let epoch = store.metadata().unwrap().unwrap().store_epoch;
        assert_eq!(
            MaintenanceRunner::new(&mut store, &epoch, &journals, &requests)
                .run()
                .unwrap(),
            MaintenanceOutcome::Ready
        );
    }
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _state) = open_root_with_persistence(cx, persistence);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-backup").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("backup-recovery-point-0").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    let undo = window
        .debug_bounds("backup-undo-0")
        .expect("已应用恢复点必须可撤回");
    window.simulate_click(undo.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("backup-undo-staged").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("backup-undo-staged").is_some(),
        "撤回登记成功必须提示下次启动生效"
    );
    let observer = WorkspaceDatabase::open(&paths).unwrap();
    assert!(
        observer
            .unfinished_maintenance_requests()
            .unwrap()
            .iter()
            .any(|request| request.kind == "backup.undo"
                && request.status
                    == crate::persistence::maintenance_request::MaintenanceRequestStatus::Prepared),
        "必须登记撤回维护请求"
    );
    drop(observer);
    drop(writer);
}

/// 跨机工作区备份：恢复后根缺失进入重定位，未授权前不产生运行态。
#[test]
fn backup_cross_machine_workspace_requires_relocation_before_runtime() {
    use crate::encrypted_backup::workspace::{
        WORKSPACE_DTO_VERSION, WorkspaceBackup, WorkspaceGeometryBackup, WorkspaceProjectBackup,
        restore_workspace,
    };
    use crate::persistence::workspace::WorkspaceDatabase;
    use slterm_workspace::{ProjectId, ProjectLayout, ViewStateRegistry};
    use std::str::FromStr as _;

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let missing = temp.path().join("missing-root");
    let payload = WorkspaceBackup {
        version: WORKSPACE_DTO_VERSION,
        source_machine: "other-machine".to_owned(),
        active_project: Some(project_id.to_string()),
        projects: vec![WorkspaceProjectBackup {
            id: project_id.to_string(),
            name: "from-other-machine".to_owned(),
            root: missing.to_str().unwrap().to_owned(),
            layout: ProjectLayout::empty(project_id)
                .to_canonical_json()
                .unwrap(),
            next_terminal_number: 0,
        }],
        terminals: Vec::new(),
        documents: Vec::new(),
        view_states: Vec::new(),
        sidebar: None,
        window_geometry: Some(WorkspaceGeometryBackup {
            machine_key: "other-machine".to_owned(),
            geometry: crate::windowing::geometry::WindowGeometry {
                x: 1.0,
                y: 2.0,
                width: 800.0,
                height: 600.0,
                maximized: false,
                display: None,
            },
        }),
    };
    let registry = ViewStateRegistry::new(&[]).unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    restore_workspace(&mut store, &payload, "this-machine", &registry).unwrap();
    assert!(
        store.window_geometry("this-machine").unwrap().is_none(),
        "跨机几何不得应用"
    );
    let mut snapshot = store.load_project_snapshot().unwrap();
    super::super::root_status::probe_with_system_resolver(&mut snapshot).unwrap();
    assert_eq!(
        snapshot.registry().projects()[0].root_status(),
        slterm_workspace::ProjectRootStatus::RelocationRequired,
        "恢复的跨机根缺失必须先进入重定位"
    );
}

/// 设置「应用」页提供诊断导出入口，完整路径默认关闭。
#[gpui::test]
fn settings_application_page_offers_diagnostics_dialog(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-open-diagnostics").is_none());
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-diagnostics").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("diagnostics-dialog").is_some());
    assert!(window.debug_bounds("diagnostics-full-paths").is_some());
    assert!(window.debug_bounds("diagnostics-export").is_some());
    window.simulate_keystrokes("escape");
    draw_frame(&mut window);
    assert!(window.debug_bounds("diagnostics-dialog").is_none());
}

/// 诊断导出：默认不含完整路径；显式勾选后才进入 full-paths 类别。
#[gpui::test]
fn diagnostics_export_respects_full_path_selection(cx: &mut TestAppContext) {
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use std::sync::Arc;

    let temp = tempfile::tempdir().unwrap();
    let plain = temp.path().join("diagnostics-plain.json");
    let full = temp.path().join("diagnostics-full.json");
    let windows = Arc::new(FakeWindowsIntegration::default());
    windows.set_save_target(Some(plain.clone()));
    let (root, mut window) =
        open_root_with_windows(cx, AppRootPersistence::empty(None), windows.clone());
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-diagnostics").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);
    let export = window.debug_bounds("diagnostics-export").unwrap();
    window.simulate_click(export.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if plain.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&plain).expect("诊断文件必须写盘")).unwrap();
    let sections = report["sections"].as_array().unwrap();
    assert!(sections.iter().any(|value| value == "environment"));
    assert!(sections.iter().any(|value| value == "app-log"));
    assert!(
        !sections.iter().any(|value| value == "full-paths"),
        "默认导出不得包含完整路径类别"
    );
    assert!(report.get("full_paths").is_none());

    // 重新打开并显式勾选完整路径，目标换成第二个文件。
    windows.set_save_target(Some(full.clone()));
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-diagnostics").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    draw_frame(&mut window);
    let toggle = window.debug_bounds("diagnostics-full-paths").unwrap();
    window.simulate_click(toggle.center(), Modifiers::default());
    draw_frame(&mut window);
    let export = window.debug_bounds("diagnostics-export").unwrap();
    window.simulate_click(export.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if full.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&full).expect("完整路径诊断必须写盘")).unwrap();
    let sections = report["sections"].as_array().unwrap();
    assert!(
        sections.iter().any(|value| value == "full-paths"),
        "显式勾选后必须声明完整路径类别"
    );
    assert!(
        !report["full_paths"].as_array().unwrap().is_empty(),
        "完整路径类别不得为空"
    );
}

/// 清理：预览所有者候选，执行后删除旧日志与更新残留。
#[gpui::test]
fn cleanup_previews_and_removes_owned_candidates(cx: &mut TestAppContext) {
    let local = crate::platform::dirs::local_dir().to_owned();
    std::fs::create_dir_all(local.join("updates")).unwrap();
    let stale_log = local.join(format!(
        "slterm-{}.log",
        std::process::id().saturating_add(1_000_000)
    ));
    std::fs::write(&stale_log, b"old log").unwrap();
    let stale_part = local.join("updates").join("slterm-cleanup-test.part");
    std::fs::write(&stale_part, b"partial").unwrap();

    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    let open = window.debug_bounds("settings-open-cleanup").unwrap();
    window.simulate_click(open.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("cleanup-candidates").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("cleanup-candidates").is_some(),
        "候选预览必须显示"
    );
    let execute = window.debug_bounds("cleanup-execute").unwrap();
    window.simulate_click(execute.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if window.debug_bounds("cleanup-result").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("cleanup-result").is_some(),
        "执行后必须显示逐项结果汇总"
    );
    assert!(!stale_log.exists(), "旧日志必须被清理");
    assert!(!stale_part.exists(), "更新下载残留必须被清理");
}

/// 应用页展示 Runtime 可用性与非敏感端点。
#[gpui::test]
fn settings_application_page_shows_runtime_status_and_endpoint(cx: &mut TestAppContext) {
    use slterm_runtime_protocol::DiscoveryRecord;

    let paths = crate::platform::dirs::app_paths();
    let endpoint_path = paths.local_root().join("local-control-endpoint.json");
    let record = DiscoveryRecord {
        protocol_version: 2,
        endpoint: "127.0.0.1:4242".parse().unwrap(),
        run_id: "run-runtime-panel".to_owned(),
        process_id: 1,
        process_created_at_ms: 2,
    };
    std::fs::write(&endpoint_path, serde_json::to_vec(&record).unwrap()).unwrap();

    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    for _ in 0..80 {
        window.run_until_parked();
        cx.run_until_parked();
        if root.read_with(cx, |root, _| root.runtime_status.loaded) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-runtime").is_some());
    assert!(window.debug_bounds("settings-runtime-endpoint").is_some());
    root.read_with(cx, |root, _| {
        assert!(root.runtime_status.loaded);
        assert!(!root.runtime_status.available, "测试根没有控制面 client");
        assert_eq!(
            root.runtime_status.endpoint.as_deref(),
            Some("127.0.0.1:4242")
        );
    });
    let _ = std::fs::remove_file(&endpoint_path);
}
