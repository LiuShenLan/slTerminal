//! B55 壁纸设置 UI(L4 虚拟窗口):路径/fit/alignment/cover/opacity 写入、
//! 拖动预览不落盘、取消与解码失败零写入。

use super::*;
use image::RgbaImage;

/// 写一张真实 PNG 作为壁纸夹具。
fn write_wallpaper(path: &std::path::Path) {
    RgbaImage::from_pixel(64, 32, image::Rgba([10, 20, 30, 255]))
        .save(path)
        .unwrap();
}

/// 路径/fit/alignment/cover/opacity 五项设置键全部落盘;清除写回 null。
#[gpui::test]
fn wallpaper_controls_persist_path_fit_alignment_cover_and_opacity(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    let image_path = directory.path().join("wallpaper.png");
    write_wallpaper(&image_path);
    let image_string = image_path.to_string_lossy().into_owned();

    // 路径选择:对话框 → 后台解码 → 写 background_image。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| pane.pick_wallpaper_for_test(window, cx));
    });
    assert!(window.did_prompt_for_paths(), "选择壁纸必须打开文件选择器");
    window.simulate_path_prompt_response(|_| Some(vec![image_path.clone()]));
    window.run_until_parked();
    cx.update(|cx| {
        assert!(
            pane.read(cx).wallpaper_preview_ready_for_test(),
            "解码成功后必须持有缩略图"
        );
        assert!(pane.read(cx).wallpaper_error_for_test().is_none());
    });

    // 其余四项经与控件同一提交入口落盘。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_wallpaper_fit_for_test("uniform", window, cx);
            pane.commit_wallpaper_alignment_for_test("bottom-right", window, cx);
            pane.toggle_for_test("background_image_cover_chrome", true, window, cx);
            pane.commit_wallpaper_opacity_for_test(40.0, window, cx);
        });
    });
    window.run_until_parked();

    let tree = slterm_settings::json::parse(&std::fs::read(&settings_path).unwrap()).unwrap();
    let appearance = tree.get("appearance").expect("壁纸键归 appearance 段");
    assert_eq!(
        appearance
            .get("background_image")
            .and_then(|value| value.as_str()),
        Some(image_string.as_str()),
        "壁纸路径必须落盘"
    );
    assert_eq!(
        appearance
            .get("background_image_fit")
            .and_then(|value| value.as_str()),
        Some("uniform")
    );
    assert_eq!(
        appearance
            .get("background_image_alignment")
            .and_then(|value| value.as_str()),
        Some("bottom-right")
    );
    assert_eq!(
        appearance
            .get("background_image_cover_chrome")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let opacity = appearance
        .get("background_image_opacity")
        .and_then(|value| value.as_f64())
        .expect("壁纸不透明度必须落盘");
    assert!((opacity - 0.4).abs() < 1e-6, "opacity = {opacity}");

    // 清除:设置键写回 null,预览退场。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| pane.clear_wallpaper_for_test(window, cx));
    });
    window.run_until_parked();
    let tree = slterm_settings::json::parse(&std::fs::read(&settings_path).unwrap()).unwrap();
    assert_eq!(
        tree.get("appearance")
            .and_then(|section| section.get("background_image"))
            .and_then(|value| value.as_str()),
        None,
        "清除必须写回 null"
    );
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert!(pane.runtime_for_test().background_image.is_none());
        assert!(!pane.wallpaper_preview_ready_for_test());
    });
}

/// 不透明度拖动只改内存/视效,松手才写盘。
#[gpui::test]
fn wallpaper_opacity_preview_stays_in_memory_until_release(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());

    cx.update(|cx| {
        pane.update(cx, |pane, cx| {
            pane.preview_wallpaper_opacity_for_test(35.0, cx);
        });
        assert!(!settings_path.exists(), "预览阶段绝不落盘");
        assert!(
            (pane.read(cx).runtime_for_test().background_image_opacity - 0.35).abs() < 1e-6,
            "预览必须即时更新运行时值"
        );
    });

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.commit_wallpaper_opacity_for_test(35.0, window, cx);
        });
    });
    window.run_until_parked();
    assert!(settings_path.exists(), "松手提交必须落盘");
    let tree = slterm_settings::json::parse(&std::fs::read(&settings_path).unwrap()).unwrap();
    let opacity = tree
        .get("appearance")
        .and_then(|section| section.get("background_image_opacity"))
        .and_then(|value| value.as_f64())
        .expect("松手后必须写入 background_image_opacity");
    assert!((opacity - 0.35).abs() < 1e-6, "opacity = {opacity}");
}

/// 取消文件选择:零写入、运行时保持无壁纸。
#[gpui::test]
fn cancelling_the_wallpaper_dialog_writes_nothing(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| pane.pick_wallpaper_for_test(window, cx));
    });
    assert!(window.did_prompt_for_paths());
    window.simulate_path_prompt_response(|_| None);
    window.run_until_parked();

    assert!(!settings_path.exists(), "取消选择不得创建设置文件");
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert!(pane.runtime_for_test().background_image.is_none());
        assert!(!pane.wallpaper_preview_ready_for_test());
        assert!(pane.wallpaper_error_for_test().is_none());
    });
}

/// 不存在/不可解码的路径:给可读错误且不写设置。
#[gpui::test]
fn invalid_wallpaper_path_reports_an_error_without_persisting(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let settings_path = directory.path().join("settings.json");
    let pane = open_settings_at(cx, &workspace, &mut window, settings_path.clone());
    let missing = directory.path().join("missing.png");

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| pane.pick_wallpaper_for_test(window, cx));
    });
    window.simulate_path_prompt_response(|_| Some(vec![missing]));
    window.run_until_parked();

    assert!(!settings_path.exists(), "解码失败不得写设置");
    cx.update(|cx| {
        let pane = pane.read(cx);
        assert!(
            pane.wallpaper_error_for_test().is_some(),
            "失败必须留下可读错误"
        );
        assert!(pane.runtime_for_test().background_image.is_none());
        assert!(!pane.wallpaper_preview_ready_for_test());
    });

    // 损坏文件(非图片字节)同样拒绝并保留错误。
    let corrupt = directory.path().join("corrupt.png");
    std::fs::write(&corrupt, b"not an image").unwrap();
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| pane.pick_wallpaper_for_test(window, cx));
    });
    window.simulate_path_prompt_response(|_| Some(vec![corrupt]));
    window.run_until_parked();
    assert!(!settings_path.exists(), "损坏图片不得写设置");
    cx.update(|cx| {
        assert!(pane.read(cx).wallpaper_error_for_test().is_some());
        assert!(pane.read(cx).runtime_for_test().background_image.is_none());
    });
}
