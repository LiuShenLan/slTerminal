//! 更新提示：检查结果经壳事件进入前台，提示只消费封闭结果。

use super::*;
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
