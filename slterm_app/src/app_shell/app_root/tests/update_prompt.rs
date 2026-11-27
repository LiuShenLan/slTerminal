//! 更新提示：检查结果经壳事件进入前台，提示只消费封闭结果。

use super::settings_pages::{focus_workspace, open_settings};
use super::*;
use crate::notify::AlertSeverity;
use crate::update_check::{RELEASE_DOWNLOAD_PREFIX, UpdateAsset, UpdateCheckResult};

fn update_result() -> UpdateCheckResult {
    let version = "9.9.9".to_owned();
    UpdateCheckResult {
        current: env!("CARGO_PKG_VERSION").to_owned(),
        latest: version.clone(),
        update_available: true,
        manual_reinstall: false,
        asset: Some(UpdateAsset {
            version: version.clone(),
            name: format!("slTerminal-{version}-x64.exe"),
            url: format!("{RELEASE_DOWNLOAD_PREFIX}v{version}/slTerminal-{version}-x64.exe"),
            size: Some(1024),
            digest: Some("a".repeat(64)),
        }),
    }
}

/// 检查结果打开提示；跳过版本关闭提示并写入抑制状态。
#[gpui::test]
fn update_available_opens_prompt_and_skip_suppresses_version(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let (root, mut window, _state) = open_root(cx);
    let result = update_result();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_update_available(result, window, cx);
        });
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-prompt").is_some());
    assert!(window.debug_bounds("update-download").is_some());
    assert!(window.debug_bounds("update-remind").is_some());

    let skip = window.debug_bounds("update-skip").unwrap();
    window.simulate_click(skip.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-prompt").is_none());
    // 状态写盘在后台；等待任务完成后再断言抑制生效。
    for _ in 0..4 {
        window.run_until_parked();
        cx.run_until_parked();
    }
    assert!(
        !crate::update_check::should_prompt("9.9.9"),
        "跳过版本后不得再次提示"
    );
}

/// 设置「应用」页提供手动检查入口。
#[gpui::test]
fn settings_application_page_offers_manual_update_check(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    focus_workspace(&root, &mut window);
    open_settings(&mut window);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-check-updates").is_none(),
        "默认页不得出现检查入口"
    );

    let page = window.debug_bounds("settings-page-application").unwrap();
    window.simulate_click(page.center(), Modifiers::default());
    draw_frame(&mut window);
    assert!(window.debug_bounds("settings-check-updates").is_some());
}

/// 手动检查结果：有更新开提示，已是最新与失败只经应用内通知。
#[gpui::test]
fn manual_update_check_result_routes_to_prompt_and_notices(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    crate::notify::reset_in_app_notices_for_test();
    let (root, mut window, _state) = open_root(cx);

    // 已是最新：Info 通知，不开提示。
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_manual_update_check(
                Ok(UpdateCheckResult {
                    current: env!("CARGO_PKG_VERSION").to_owned(),
                    latest: env!("CARGO_PKG_VERSION").to_owned(),
                    update_available: false,
                    manual_reinstall: false,
                    asset: None,
                }),
                window,
                cx,
            );
        });
    });
    root.update(cx, |root, cx| root.poll_in_app_notices(cx));
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-prompt").is_none());
    let infos = root.read_with(cx, |root, _| {
        root.in_app_notices
            .iter()
            .filter(|notice| notice.severity == AlertSeverity::Info)
            .count()
    });
    assert_eq!(infos, 1, "已是最新必须给出应用内反馈");

    // 失败：Error 通知，不开提示。
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_manual_update_check(Err("network down".to_owned()), window, cx);
        });
    });
    root.update(cx, |root, cx| root.poll_in_app_notices(cx));
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-prompt").is_none());
    let errors = root.read_with(cx, |root, _| {
        root.in_app_notices
            .iter()
            .filter(|notice| notice.severity == AlertSeverity::Error)
            .count()
    });
    assert_eq!(errors, 1, "检查失败必须给出应用内错误");

    // 有更新：手动检查绕过版本抑制，直接打开提示。
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.finish_manual_update_check(Ok(update_result()), window, cx);
        });
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-prompt").is_some());
}

/// 已就绪的缓存资产直接进入安装阶段。
#[gpui::test]
fn ready_download_shows_install_action(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let result = update_result();
    let asset = result.asset.as_ref().unwrap();
    crate::update_download::set_status_for_test(
        asset,
        crate::update_download::DownloadStatus::Ready {
            path: std::path::PathBuf::from("D:/updates/installer.exe"),
            bytes: 1024,
        },
    );
    let (root, mut window, _state) = open_root(cx);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_update_available(result, window, cx);
        });
    });
    draw_frame(&mut window);
    assert!(window.debug_bounds("update-install").is_some());
    assert!(window.debug_bounds("update-download").is_none());
}

/// 更新预约必须经统一关闭守卫：脏 Document 先取得决定，关闭未完成不得确认。
#[gpui::test]
fn update_install_waits_for_close_guard_on_dirty_document(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("note.txt");
    std::fs::write(&source, "old").unwrap();
    std::fs::create_dir_all(temp.path().join("updates")).unwrap();
    let asset = update_result().asset.unwrap();
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, temp.path().to_str().unwrap());
    super::documents::open_document(
        &root,
        &mut window,
        PROJECT_A,
        GROUP_A,
        CanonicalPath::new(&source).unwrap(),
    );
    let editor = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| panel.editor().cloned())
            .unwrap()
    });
    window.update(|window, cx| {
        editor.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, "local ", window, cx);
        });
    });
    let root_path = temp.path().to_path_buf();
    root.update(cx, |root, _| {
        root.set_update_install_arm_for_test(std::sync::Arc::new(move |asset| {
            crate::update_download::handoff::UpdateInstallGuard::arm_for_test(&root_path, asset)
        }));
    });

    let started = window
        .update(|window, cx| {
            root.update(cx, |root, cx| root.begin_update_install(asset, window, cx))
        })
        .unwrap();
    assert!(!started, "脏 Document 必须先取得关闭决定");
    assert!(
        root.read_with(cx, |root, _| root.close_decision.is_some()),
        "关闭决定面必须出现"
    );
    assert!(
        temp.path().join("updates/install-pending.json").exists(),
        "守卫必须先写待定预约"
    );
    assert!(
        !temp.path().join("updates/install-next.json").exists(),
        "关闭完成前不得留下安装授权"
    );

    window.update(|window, cx| {
        let _ = root.update(cx, |root, cx| root.cancel_close_decision(window, cx));
    });
    assert!(
        !temp.path().join("updates/install-next.json").exists(),
        "取消关闭不得确认安装"
    );
    root.update(cx, |root, _| root.cancel_update_install());
    assert!(
        !temp.path().join("updates/install-pending.json").exists(),
        "显式放弃安装必须撤销待定预约"
    );
}
