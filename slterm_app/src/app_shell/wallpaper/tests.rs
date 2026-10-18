//! 壁纸异步加载/代际失效/壳色纪律(gpui-test-support 门控)。

use std::path::PathBuf;
use std::sync::Arc;

use gpui::TestAppContext;
use gpui_component::{Theme, ThemeMode};
use image::RgbaImage;

use super::*;

fn visual_settings(path: Option<PathBuf>) -> Settings {
    Settings {
        background_image: path,
        background_image_opacity: 0.38,
        background_image_fit: Some("fill".into()),
        background_image_alignment: Some("center".into()),
        background_image_cover_chrome: false,
        ..Settings::default()
    }
}

fn fixture(path: &std::path::Path, color: [u8; 4]) {
    RgbaImage::from_pixel(64, 32, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// 加载期间壁纸层为空但壳色仍可算;就绪后 layout/opacity 变化复用同一
/// 纹理,不重新解码。
#[gpui::test]
fn loading_yields_and_layout_opacity_refreshes_reuse_the_same_image(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallpaper.png");
    fixture(&path, [10, 20, 30, 255]);
    let mut settings = visual_settings(Some(path));
    settings.background_image_cover_chrome = true;
    cx.update(|cx| {
        update_wallpaper(&settings, cx);
        let effects = cx.global::<VisualEffects>();
        assert!(effects.loading);
        assert!(effects.wallpaper.as_ref().unwrap().image.is_none());
        assert_eq!(chrome_surface_opacity(cx), 1.0, "就绪前按窗口不透明度");
    });
    cx.run_until_parked();
    let original = cx.update(|cx| {
        assert!(!cx.global::<VisualEffects>().loading);
        assert_eq!(chrome_surface_opacity(cx), 0.78);
        cx.global::<VisualEffects>()
            .wallpaper
            .as_ref()
            .unwrap()
            .image
            .clone()
            .unwrap()
    });
    settings.background_image_opacity = 0.75;
    settings.background_image_fit = Some("contain".into());
    settings.opacity = 0.8;
    cx.update(|cx| update_wallpaper(&settings, cx));
    cx.run_until_parked();
    cx.update(|cx| {
        let wallpaper = cx.global::<VisualEffects>().wallpaper.as_ref().unwrap();
        assert!(
            Arc::ptr_eq(&original, wallpaper.image.as_ref().unwrap()),
            "适应度/透明度变化不得重烤纹理"
        );
        assert_eq!(wallpaper.opacity, 0.75);
        assert_eq!(wallpaper.fit, BackgroundImageFit::Uniform);
        assert_eq!(chrome_surface_opacity(cx), 0.78, "整窗壁纸 scrim 上限");
        for width in 800..1000 {
            let bounds = gpui::Bounds::new(
                gpui::point(gpui::px(100.0), gpui::px(30.0)),
                gpui::size(gpui::px(width as f32), gpui::px(600.0)),
            );
            let _ = image_bounds(wallpaper, bounds, 1.5);
            assert!(Arc::ptr_eq(&original, wallpaper.image.as_ref().unwrap()));
        }
        settings.background_image_opacity = 0.0;
        update_wallpaper(&settings, cx);
        assert_eq!(chrome_surface_opacity(cx), 0.78);
        set_opacity_live(0.4, cx);
        assert_eq!(chrome_surface_opacity(cx), 0.4);
        settings.background_image_cover_chrome = false;
        update_wallpaper(&settings, cx);
        assert_eq!(
            chrome_surface_opacity(cx),
            0.8,
            "非整窗壁纸回到窗口不透明度"
        );
        settings.background_image = None;
        update_wallpaper(&settings, cx);
        assert_eq!(chrome_surface_opacity(cx), 0.8);
    });
}

/// 最新请求赢;解码期间清除壁纸不能让旧结果复活。
#[gpui::test]
fn latest_request_wins_and_clearing_during_decode_cannot_restore_an_image(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.png");
    let last = directory.path().join("last.png");
    fixture(&first, [255, 0, 0, 255]);
    fixture(&last, [0, 0, 255, 255]);
    cx.update(|cx| {
        update_wallpaper(&visual_settings(Some(first)), cx);
        update_wallpaper(&visual_settings(Some(last.clone())), cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let wallpaper = cx.global::<VisualEffects>().wallpaper.as_ref().unwrap();
        assert_eq!(wallpaper.path, last);
        assert_eq!(
            &wallpaper.image.as_ref().unwrap().as_bytes(0).unwrap()[..4],
            &[255, 0, 0, 255],
            "最后一次请求的图片胜出(路径换过但代际只保留最新)"
        );
        update_wallpaper(
            &visual_settings(Some(directory.path().join("missing.png"))),
            cx,
        );
        update_wallpaper(&visual_settings(None), cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(cx.global::<VisualEffects>().wallpaper.is_none());
        assert!(!cx.global::<VisualEffects>().loading);
    });
}

/// 卡层与整窗层共用同一锚点数学;native 尺寸按物理像素保持。
#[test]
fn card_and_chrome_share_the_window_anchor_and_preserve_native_physical_size() {
    let mut wallpaper = Wallpaper {
        path: PathBuf::new(),
        image: None,
        stamp: None,
        width: 800,
        height: 400,
        fit: BackgroundImageFit::UniformToFill,
        alignment: BackgroundImageAlignment::Center,
        cover_chrome: true,
        opacity: 0.38,
    };
    let anchor = gpui::Bounds::new(
        gpui::point(gpui::px(0.0), gpui::px(0.0)),
        gpui::size(gpui::px(600.0), gpui::px(600.0)),
    );
    assert_eq!(
        image_bounds(&wallpaper, anchor, 1.0),
        gpui::Bounds::new(
            gpui::point(gpui::px(-300.0), gpui::px(0.0)),
            gpui::size(gpui::px(1200.0), gpui::px(600.0)),
        )
    );
    wallpaper.fit = BackgroundImageFit::Uniform;
    assert_eq!(
        image_bounds(&wallpaper, anchor, 1.0),
        gpui::Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(150.0)),
            gpui::size(gpui::px(600.0), gpui::px(300.0)),
        )
    );
    wallpaper.fit = BackgroundImageFit::None;
    assert_eq!(
        image_bounds(&wallpaper, anchor, 2.0).size,
        gpui::size(gpui::px(400.0), gpui::px(200.0))
    );
    wallpaper.alignment = BackgroundImageAlignment::BottomRight;
    assert_eq!(
        image_bounds(&wallpaper, anchor, 2.0).origin,
        gpui::point(gpui::px(200.0), gpui::px(400.0))
    );
}

/// 透明度纪律:只改壳底色 alpha,文字前景恒不透明;重复应用幂等。
#[gpui::test]
fn shell_opacity_only_dims_surfaces_and_is_idempotent(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        test_install_visual_effects(cx, 0.5, BlurMode::None);
        crate::app_shell::theme::reapply_shell_opacity(cx);
        let theme = cx.global::<Theme>();
        assert!((theme.background.a - 0.5).abs() < 1e-6, "壳底色吃透明度");
        assert!((theme.tokens.background.color.a - 0.5).abs() < 1e-6);
        assert!(
            (theme.foreground.a - 1.0).abs() < 1e-6,
            "文字前景必须保持不透明"
        );
        assert!((theme.title_bar.a - 0.5).abs() < 1e-6);
        // 拖动滑块会高频重算:基线 alpha 必须先归一,不能层层相乘。
        crate::app_shell::theme::reapply_shell_opacity(cx);
        let theme = cx.global::<Theme>();
        assert!((theme.background.a - 0.5).abs() < 1e-6);
        assert!((theme.foreground.a - 1.0).abs() < 1e-6);
    });
}
