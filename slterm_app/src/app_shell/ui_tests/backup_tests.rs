//! 加密备份导出 UI:整单成功/失败语义、前台校验、取消零写入与忙态门。
//!
//! 用例走真实设置页控件与路径选择器;导出内容经 `encrypted_backup::open`
//! 用同一口令回读。`collect` 的生产根是进程级测试临时数据目录,
//! 本文件写入的 `settings.json` 由 [`DataDirSettingsGuard`] 原样恢复。

use std::path::PathBuf;

use gpui::{TestAppContext, VisualTestContext};
use gpui_component::Root;

use super::{click_selector, draw_frame, open_settings_at, open_workspace};
use crate::app_shell::settings_pane::SettingsPane;
use crate::app_shell::settings_pane::SettingsStatus;
use crate::app_shell::workspace::SltermWorkspace;
use crate::encrypted_backup::{BackupCategory, BackupSelection, open};

const PASSPHRASE: &str = "correct horse";

/// 当前窗口通知数量(失败提示的可观测面)。
fn notification_count(window: &mut VisualTestContext) -> usize {
    window.update(|window, cx| {
        window
            .root::<Root>()
            .flatten()
            .map(|root| {
                let list = root.read(cx).notification.clone();
                list.read(cx).notifications().len()
            })
            .unwrap_or(0)
    })
}

/// 在进程级测试数据目录布置/恢复 `settings.json`,避免跨用例污染。
struct DataDirSettingsGuard {
    path: PathBuf,
    previous: Option<Vec<u8>>,
}

impl DataDirSettingsGuard {
    fn write(bytes: &[u8]) -> Self {
        let path = crate::platform::dirs::data_dir().join("settings.json");
        let previous = std::fs::read(&path).ok();
        std::fs::write(&path, bytes).expect("测试数据目录可写");
        Self { path, previous }
    }
}

impl Drop for DataDirSettingsGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(bytes) => {
                let _ = std::fs::write(&self.path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// 打开设置页并切到「高级」页(加密备份分组所在页)。
fn open_advanced_page(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    settings_path: PathBuf,
) -> gpui::Entity<SettingsPane> {
    let pane = open_settings_at(cx, workspace, window, settings_path);
    window.refresh().unwrap();
    draw_frame(window);
    click_selector(window, "settings-nav-advanced");
    window.refresh().unwrap();
    draw_frame(window);
    pane
}

/// 成功路径:整包可解封、状态为非错误成功态、忙态复位。
#[gpui::test]
fn backup_export_writes_an_openable_archive_and_reports_success(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_advanced_page(cx, &workspace, &mut window, settings_path);
    let _guard = DataDirSettingsGuard::write(br#"{"appearance":{"theme":"Nord"}}"#);
    let output = directory.path().join("slterm-backup.slterm-backup");

    // 高级页必须渲染出备份控件。
    assert!(
        window.debug_bounds("settings-backup-export").is_some(),
        "加密备份导出入口必须绘制"
    );
    assert!(window.debug_bounds("settings-backup-passphrase").is_some());
    for selector in [
        "settings-backup-category-settings",
        "settings-backup-category-session",
        "settings-backup-category-themes",
        "settings-backup-category-command_history",
        "settings-backup-category-directory_history",
        "settings-backup-category-providers",
    ] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "类别开关必须绘制: {selector}"
        );
    }

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_passphrase_for_test(PASSPHRASE, window, cx);
            pane.start_backup_export_for_test(output.clone(), window, cx);
        });
    });
    window.run_until_parked();

    assert!(output.exists(), "导出必须生成目标文件");
    let packet = std::fs::read(&output).unwrap();
    let archive = open(&packet, PASSPHRASE).expect("同一口令必须能解封");
    assert_eq!(
        archive.manifest.categories,
        vec![BackupCategory::Settings],
        "默认选择仅设置"
    );
    assert_eq!(archive.entries.len(), 1, "settings.json 必须入包");
    cx.update(|cx| {
        let pane = pane.read(cx);
        match pane.status_for_test() {
            Some(SettingsStatus::BackupExported { path }) => assert_eq!(path, &output),
            other => panic!("应报导出成功: {other:?}"),
        }
        assert!(!pane.backup_busy_for_test(), "成功后忙态必须复位");
    });
}

/// 整单失败:`collect` 解析失败即中止,状态为错误态 + 失败通知,零写入。
#[gpui::test]
fn backup_export_reports_whole_archive_failure_without_writing(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_advanced_page(cx, &workspace, &mut window, settings_path);
    let _guard = DataDirSettingsGuard::write(b"{ not json");
    let output = directory.path().join("slterm-backup.slterm-backup");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_passphrase_for_test(PASSPHRASE, window, cx);
            pane.start_backup_export_for_test(output.clone(), window, cx);
        });
    });
    window.run_until_parked();

    assert!(!output.exists(), "整单失败不得留下目标文件");
    cx.update(|cx| {
        let pane = pane.read(cx);
        let status = pane.status_for_test().expect("失败必须给出状态条");
        assert!(status.is_error(), "整单失败必须是错误态: {status:?}");
        match status {
            SettingsStatus::BackupFailed(error) => assert!(!error.is_empty()),
            other => panic!("应为 BackupFailed: {other:?}"),
        }
        assert!(!pane.backup_busy_for_test(), "失败后忙态必须复位");
    });
    assert!(
        notification_count(&mut window) >= 1,
        "整单失败必须给出「备份未完成」窗口通知"
    );
}

/// 前台校验:未选类别 / 口令过短都在本地拒绝,不弹对话框、不写文件。
#[gpui::test]
fn backup_export_validates_before_opening_the_dialog(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_advanced_page(cx, &workspace, &mut window, settings_path.clone());

    // 未选任何类别。
    let empty_message = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_selection_for_test(BackupSelection::from_categories([]));
            pane.export_backup_for_test(window, cx);
            match pane.status_for_test() {
                Some(SettingsStatus::BackupFailed(message)) => message.clone(),
                other => panic!("空选择应报校验失败: {other:?}"),
            }
        })
    });
    assert!(!window.did_prompt_for_new_path(), "校验失败不得打开对话框");

    // 口令过短。
    let short_message = window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_selection_for_test(BackupSelection::default());
            pane.set_backup_passphrase_for_test("short", window, cx);
            pane.export_backup_for_test(window, cx);
            match pane.status_for_test() {
                Some(SettingsStatus::BackupFailed(message)) => message.clone(),
                other => panic!("短口令应报校验失败: {other:?}"),
            }
        })
    });
    assert!(!window.did_prompt_for_new_path(), "短口令不得打开对话框");
    assert_ne!(empty_message, short_message, "两类校验文案必须区分");
    assert!(!pane.read_with(cx, |pane, _| pane.backup_busy_for_test()));
    let names: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(names.is_empty(), "校验失败不得产生任何文件: {names:?}");
    assert!(!settings_path.exists(), "校验失败不得写设置");
}

/// 取消路径选择:零写入、忙态复位、状态不变。
#[gpui::test]
fn cancelling_backup_export_writes_nothing(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_advanced_page(cx, &workspace, &mut window, settings_path);

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_passphrase_for_test(PASSPHRASE, window, cx);
            pane.export_backup_for_test(window, cx);
        });
    });
    assert!(window.did_prompt_for_new_path(), "导出必须打开另存对话框");

    window.simulate_new_path_selection(|_| None);
    window.run_until_parked();

    cx.update(|cx| {
        let pane = pane.read(cx);
        assert!(!pane.backup_busy_for_test(), "取消后忙态必须复位");
        assert!(pane.status_for_test().is_none(), "取消不得改变状态条");
    });
    let names: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(names.is_empty(), "取消不得产生任何文件: {names:?}");
}

/// 忙态重入:在途导出期间再次请求被忙态门拒绝,不产生第二个任务/文件。
#[gpui::test]
fn backup_export_ignores_reentry_while_busy(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_advanced_page(cx, &workspace, &mut window, settings_path);
    let _guard = DataDirSettingsGuard::write(br#"{"appearance":{"theme":"Nord"}}"#);
    let first = directory.path().join("first.slterm-backup");
    let second = directory.path().join("second.slterm-backup");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_passphrase_for_test(PASSPHRASE, window, cx);
            pane.start_backup_export_for_test(first.clone(), window, cx);
            assert!(pane.backup_busy_for_test(), "首个导出必须进入忙态");
            // 在途再次调用:忙态门拒绝,不得产生第二个输出。
            pane.start_backup_export_for_test(second.clone(), window, cx);
        });
    });
    window.run_until_parked();

    assert!(first.exists(), "首个导出必须完成");
    assert!(!second.exists(), "忙态重入不得产生第二个备份");
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert!(!pane.backup_busy_for_test());
        assert!(
            matches!(
                pane.status_for_test(),
                Some(SettingsStatus::BackupExported { path }) if path == &first
            ),
            "状态应指向首个导出"
        );
    });

    // 按钮入口同门:忙态时点击不打开路径对话框。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_backup_busy_for_test(true);
            pane.export_backup_for_test(window, cx);
            pane.set_backup_busy_for_test(false);
        });
    });
    assert!(
        !window.did_prompt_for_new_path(),
        "忙态下按钮入口不得再开对话框"
    );
}
