//! 窗口视效:背景模糊材质 / 窗口透明度 / 壁纸。
//!
//! 三条语义边界:
//! - 模糊只走 GPUI `WindowBackgroundAppearance` 平台通道;壳侧额外落
//!   Windows 原生 AccentPolicy/DWMSBT 两通道,切换档显式互斥清理。
//! - 透明度只透壳底色与终端默认背景;文字与彩色单元背景保持不透明。
//! - 壁纸 = 底色之上、单元格之下的一层图;fit/alignment/cover_chrome/
//!   图自身透明度独立于窗口 opacity。
//!
//! 解码结果按 `(路径, 文件戳)` 缓存,后台串行执行;UI 线程零文件 I/O。
//! 环境要求:本模块的 `refresh` 可安全地在首窗创建前后调用——它只读写
//! 背景外观与后台任务,**不 show/activate 任何窗口**(静默启动形态不破)。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{
    App, Bounds, Corners, IntoElement, ParentElement, Pixels, RenderImage, Styled, Window,
    WindowBackgroundAppearance, div, point, px, size,
};
use image::Frame;
use slterm_settings::BlurMode;

use crate::app_shell::config::Settings;

pub(super) mod image_layout;
mod image_loader;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use image_layout::{BackgroundImageAlignment, BackgroundImageFit, wallpaper_rect};

/// 视效全局状态。`Drop` 时推进代际,使在途加载失效。
pub(crate) struct VisualEffects {
    /// 窗口不透明度(0..=1):只作用于壳底色与终端默认背景。
    pub opacity: f32,
    /// 窗口背景材质(档位,默认 None)。
    pub blur: BlurMode,
    wallpaper: Option<Wallpaper>,
    generation: Arc<AtomicU64>,
    loading: bool,
    /// 壁纸加载循环；全局销毁即取消，迟到解码不得回写已退出状态。
    load_task: Option<gpui::Task<()>>,
}

impl gpui::Global for VisualEffects {}

impl Drop for VisualEffects {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Release);
    }
}

struct Wallpaper {
    path: PathBuf,
    image: Option<Arc<RenderImage>>,
    stamp: Option<image_loader::FileStamp>,
    width: u32,
    height: u32,
    fit: BackgroundImageFit,
    alignment: BackgroundImageAlignment,
    cover_chrome: bool,
    opacity: f32,
}

/// 设置变更总入口:更新视效全局 → 必要时后台重载壁纸 → 落窗口材质。
///
/// 只读取已入库的壳 `Settings` 快照,不读磁盘;不触碰窗口可见性。
pub(crate) fn refresh(cx: &mut App) {
    let settings = cx.try_global::<Settings>().cloned().unwrap_or_default();
    update_wallpaper(&settings, cx);
    apply_window_effects(cx);
    refresh_surface_opacity(cx);
}

fn update_wallpaper(settings: &Settings, cx: &mut App) {
    if !cx.has_global::<VisualEffects>() {
        cx.set_global(VisualEffects {
            opacity: settings.opacity.clamp(0.0, 1.0),
            blur: settings.blur,
            wallpaper: None,
            generation: Arc::new(AtomicU64::new(0)),
            loading: false,
            load_task: None,
        });
    }
    let desired = settings.background_image.clone();
    let effects = cx.global_mut::<VisualEffects>();
    effects.opacity = settings.opacity.clamp(0.0, 1.0);
    effects.blur = settings.blur;
    // 路径变化才换壁纸对象;fail/消失时旧纹理随对象一起退役。
    let path_changed = match (&effects.wallpaper, &desired) {
        (Some(wp), Some(path)) => &wp.path != path,
        (None, None) => false,
        _ => true,
    };
    let retired = if path_changed {
        let retired = effects.wallpaper.take().and_then(|wp| wp.image);
        effects.wallpaper = desired.map(|path| Wallpaper {
            path,
            image: None,
            stamp: None,
            width: 1,
            height: 1,
            fit: BackgroundImageFit::default(),
            alignment: BackgroundImageAlignment::default(),
            cover_chrome: false,
            opacity: 1.0,
        });
        retired
    } else {
        None
    };
    if let Some(wp) = effects.wallpaper.as_mut() {
        wp.fit = settings
            .background_image_fit
            .as_deref()
            .and_then(BackgroundImageFit::parse)
            .unwrap_or_default();
        wp.alignment = settings
            .background_image_alignment
            .as_deref()
            .and_then(BackgroundImageAlignment::parse)
            .unwrap_or_default();
        wp.cover_chrome = settings.background_image_cover_chrome;
        wp.opacity = settings.background_image_opacity.clamp(0.0, 1.0);
    }
    // 适应度/对齐/透明度变化也推进代际:render 与 decode 同闸门。
    effects.generation.fetch_add(1, Ordering::Release);
    retire_image(retired, cx);
    start_load(cx);
}

fn start_load(cx: &mut App) {
    let effects = cx.global_mut::<VisualEffects>();
    if effects.loading {
        return;
    }
    if effects.wallpaper.is_none() {
        return;
    }
    effects.loading = true;
    let executor = cx.background_executor().clone();
    // 单循环持有整个加载生命周期：代际过期只重取最新请求，不递归另起
    // 任务；全局销毁时任务句柄随 `VisualEffects` 一起 drop 并取消。
    let task = cx.spawn(async move |cx| {
        loop {
            let request = cx.update(|cx| {
                let effects = cx.try_global::<VisualEffects>()?;
                let wp = effects.wallpaper.as_ref()?;
                Some(image_loader::Request {
                    path: wp.path.clone(),
                    cached: wp.stamp.clone(),
                    generation: effects.generation.clone(),
                    version: effects.generation.load(Ordering::Acquire),
                })
            });
            let Some(request) = request else {
                // 壁纸已清除或全局已销毁：收掉 loading，结束循环。
                cx.update(|cx| {
                    if cx.has_global::<VisualEffects>() {
                        cx.global_mut::<VisualEffects>().loading = false;
                    }
                });
                return;
            };
            let version = request.version;
            let result = executor
                .spawn(async move { image_loader::load(request) })
                .await;
            let step = cx.update(|cx| {
                let effects = cx.global_mut::<VisualEffects>();
                if effects.generation.load(Ordering::Acquire) != version {
                    // 旧请求作废：保持 loading，下一轮按最新代际重取。
                    return (true, false, None);
                }
                effects.loading = false;
                match result {
                    Ok(Some(loaded)) => {
                        let Some(wp) = effects.wallpaper.as_mut() else {
                            return (false, false, None);
                        };
                        wp.width = loaded.layout_width;
                        wp.height = loaded.layout_height;
                        wp.stamp = Some(loaded.stamp);
                        let image = Arc::new(RenderImage::new([Frame::new(loaded.pixels)]));
                        (false, true, wp.image.replace(image))
                    }
                    // 文件戳未变 = 缓存命中,保留现纹理。
                    Ok(None) => (false, false, None),
                    Err(error) => {
                        // 解码失败降级:保留旧纹理或空壁纸,只记日志,不打断启动。
                        log::warn!("background image load failed: {error:?}");
                        (false, false, None)
                    }
                }
            });
            let (stale, applied, retired) = step;
            if stale {
                continue;
            }
            cx.update(|cx| {
                retire_image(retired, cx);
                if applied {
                    refresh_surface_opacity(cx);
                    cx.refresh_windows();
                }
            });
            return;
        }
    });
    cx.global_mut::<VisualEffects>().load_task = Some(task);
}

/// 壁纸就绪状态会异步变化(加载完成/清除),壳色 alpha 必须跟着重算。
fn refresh_surface_opacity(cx: &mut App) {
    crate::app_shell::theme::reapply_shell_opacity(cx);
}

fn retire_image(image: Option<Arc<RenderImage>>, cx: &mut App) {
    if let Some(image) = image {
        // 设置回调可能发生在窗口 update 期间;defer 等窗口归还 App 槽位后
        // 再释放纹理,并让缓存的场景回放失效。
        cx.defer(move |cx| {
            cx.drop_image(image, None);
            cx.refresh_windows();
        });
    }
}

/// 当前窗口不透明度(无全局时视为不透明)。
pub(crate) fn window_opacity(cx: &App) -> f32 {
    cx.try_global::<VisualEffects>()
        .map_or(1.0, |effects| effects.opacity.clamp(0.0, 1.0))
}

/// 壳/卡表面不透明度:整窗壁纸就绪时保留原有 scrim 上限 0.78,
/// 使文字对比度不塌;其余情况与窗口不透明度一致。
pub(crate) fn chrome_surface_opacity(cx: &App) -> f32 {
    let Some(effects) = cx.try_global::<VisualEffects>() else {
        return 1.0;
    };
    let opacity = window_opacity(cx);
    if effects
        .wallpaper
        .as_ref()
        .is_some_and(|wp| wp.cover_chrome && wp.image.is_some())
    {
        return opacity.min(0.78);
    }
    opacity
}

/// 拖不透明度滑块的快路径:只改视效全局,不读设置、不重建壁纸纹理。
/// 调用方负责紧随其后的壳色重算与 `cx.refresh_windows()`。
pub(crate) fn set_opacity_live(opacity: f32, cx: &mut App) {
    if cx.has_global::<VisualEffects>() {
        cx.global_mut::<VisualEffects>().opacity = opacity.clamp(0.0, 1.0);
    }
}

/// 拖壁纸不透明度滑块的快路径:只改视效全局中的壁纸 alpha,不读设置、
/// 不重载纹理。调用方负责 `cx.refresh_windows()`。
pub(crate) fn set_wallpaper_opacity_live(opacity: f32, cx: &mut App) {
    if cx.has_global::<VisualEffects>()
        && let Some(wallpaper) = cx.global_mut::<VisualEffects>().wallpaper.as_mut()
    {
        wallpaper.opacity = opacity.clamp(0.0, 1.0);
    }
}

/// 设置页壁纸预览结果:缩略纹理 + 原图布局尺寸(供 fit/alignment 落点)。
pub(crate) struct WallpaperPreview {
    pub image: Arc<RenderImage>,
    pub width: u32,
    pub height: u32,
}

/// 预览失败原因;设置页映射为可读文案,不裸出加载器错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WallpaperPreviewError {
    Read,
    Decode,
    TooLarge,
}

/// 设置页预览加载:后台执行器上走同一条有界解码管线(预览 512 边)。
pub(crate) fn load_preview_async(
    path: PathBuf,
    cx: &mut App,
) -> gpui::Task<Result<WallpaperPreview, WallpaperPreviewError>> {
    cx.background_executor().spawn(async move {
        let request = image_loader::Request {
            path,
            cached: None,
            // 设置页预览没有跨帧缓存/代际竞争;固定代际使请求始终有效。
            generation: Arc::new(AtomicU64::new(0)),
            version: 0,
        };
        match image_loader::load_preview(request) {
            Ok(Some(loaded)) => Ok(WallpaperPreview {
                image: Arc::new(RenderImage::new([Frame::new(loaded.pixels)])),
                width: loaded.layout_width,
                height: loaded.layout_height,
            }),
            Ok(None) => Err(WallpaperPreviewError::Read),
            Err(error) => Err(match error {
                image_loader::LoadError::TooLarge => WallpaperPreviewError::TooLarge,
                image_loader::LoadError::Read => WallpaperPreviewError::Read,
                image_loader::LoadError::Decode => WallpaperPreviewError::Decode,
            }),
        }
    })
}

/// 设置页预览盒内图片落点(fit/alignment 与运行时同一套几何)。
pub(crate) fn preview_rect(
    viewport_w: f32,
    viewport_h: f32,
    image_w: u32,
    image_h: u32,
    fit: Option<&str>,
    alignment: Option<&str>,
) -> (f32, f32, f32, f32) {
    let fit = fit.and_then(BackgroundImageFit::parse).unwrap_or_default();
    let alignment = alignment
        .and_then(BackgroundImageAlignment::parse)
        .unwrap_or_default();
    wallpaper_rect(
        viewport_w,
        viewport_h,
        image_w as f32,
        image_h as f32,
        fit,
        alignment,
    )
}

/// 开窗参数用:首帧即走平台 backdrop,不必等 `refresh` 补第二次。
///
/// Mica/MicaAlt 在 22H2(22621)走原生枚举;Acrylic 在 22H2+ 先借
/// `MicaBackdrop` 占住系统 backdrop 通道,再由壳侧改写为
/// `DWMSBT_TRANSIENTWINDOW`;1809(17763)+ 回退经典模糊;更老系统与
/// `None` 一律透明交换链——GPUI Windows 渲染器按 `Opaque` 固定
/// alpha=1 清屏,保留透明才能让绘制像素的 alpha 生效。
pub(crate) fn initial_background_appearance(cx: &App) -> WindowBackgroundAppearance {
    let blur = cx
        .try_global::<Settings>()
        .map_or(BlurMode::None, |settings| settings.blur);
    background_appearance(blur)
}

fn background_appearance(blur: BlurMode) -> WindowBackgroundAppearance {
    background_appearance_for(blur, crate::platform::material::windows_build_number())
}

/// 纯函数 gate:build 决定各档位走原生枚举还是 AccentPolicy 回退。
///
/// Acrylic 在 22H2+ 返回 `MicaBackdrop` 只是借用 GPUI 的「系统 backdrop
/// 通道」:GPUI 先写入一个系统 backdrop 态,随后
/// 平台材质层会把它改写为 `DWMSBT_TRANSIENTWINDOW`。
/// WinAppSDK `DesktopAcrylicController` 的默认 Acrylic 在 DWM 侧就是
/// 该系统 backdrop,故二者视觉等价,无需引入 WinAppSDK 依赖(本仓无
/// `Microsoft.UI.*` WinRT 面,WinRT 激活还需随包 bootstrap DLL 与运行时
/// 元数据,且公开文档不提供接口 IID,无法安全手写 COM vtable)。
/// 旧系统没有该系统 backdrop,仍以 `Blurred` 走 WCA Acrylic 回退。
fn background_appearance_for(blur: BlurMode, build: u32) -> WindowBackgroundAppearance {
    match blur {
        BlurMode::Aero => WindowBackgroundAppearance::Blurred,
        BlurMode::Acrylic if build >= 22_621 => WindowBackgroundAppearance::MicaBackdrop,
        BlurMode::Acrylic => WindowBackgroundAppearance::Blurred,
        BlurMode::Mica if build >= 22_621 => WindowBackgroundAppearance::MicaBackdrop,
        BlurMode::MicaAlt if build >= 22_621 => WindowBackgroundAppearance::MicaAltBackdrop,
        BlurMode::Mica | BlurMode::MicaAlt if build >= 17_763 => {
            WindowBackgroundAppearance::Blurred
        }
        BlurMode::Mica | BlurMode::MicaAlt | BlurMode::None => {
            WindowBackgroundAppearance::Transparent
        }
    }
}

/// 已经真正落到窗口上的模糊档位。窗口级材质是跨进程调用(WCA 两次 +
/// DWMSBT 一次);档位没变时一次都不碰,否则透明度滑块会每帧与 DWM
/// 往返,直接拖成幻灯片。
struct AppliedBlur {
    blur: BlurMode,
    windows: HashSet<gpui::WindowId>,
}

impl gpui::Global for AppliedBlur {}

/// 把窗口级材质应用到所有窗口。
///
/// 必须 `defer`:设置页开关是在某个窗口自己的 update 回调里点的,此时该
/// 窗口已被 `App::update_window` 从槽里 take 出;同一 handle 立即 update
/// 只会拿到 "window not found",把 `set_background_appearance` 一起吞掉
/// ——表现为「启动时有效、运行中开关无反应」。defer 后窗口已归还槽位。
fn apply_window_effects(cx: &mut App) {
    let blur = cx
        .try_global::<VisualEffects>()
        .map_or(BlurMode::None, |effects| effects.blur);
    let appearance = background_appearance(blur);
    cx.defer(move |cx| {
        let handles = cx.windows();
        let window_ids = handles
            .iter()
            .map(|handle| handle.window_id())
            .collect::<HashSet<_>>();
        let already_applied = cx
            .try_global::<AppliedBlur>()
            .filter(|applied| applied.blur == blur)
            .map(|applied| applied.windows.clone())
            .unwrap_or_default();
        let pending = handles
            .into_iter()
            .filter(|handle| !already_applied.contains(&handle.window_id()))
            .collect::<Vec<_>>();
        let mut applied = already_applied
            .intersection(&window_ids)
            .copied()
            .collect::<HashSet<_>>();
        for handle in pending {
            if let Err(error) = handle.update(cx, |_, window, _| {
                window.set_background_appearance(appearance);
                crate::platform::material::apply_window_appearance(window, blur, appearance);
                window.refresh();
            }) {
                log::warn!("failed to apply window visual effects: {error}");
            } else {
                applied.insert(handle.window_id());
            }
        }
        cx.set_global(AppliedBlur {
            blur,
            windows: applied,
        });
    });
}

/// 窗口材质落盘计划:把「平台 build / 材质档 / 系统 backdrop 是否可用」
/// 折算成一组无副作用的动作,供真实 Win32 调用与纯函数测试共用。
/// 壁纸层:整窗(含侧栏/标题栏)垫底,卡层(终端内容区)之上。
/// 两者互斥:同一时刻只有一个与 `cover_chrome` 匹配的层真正绘制。
pub(crate) fn card_layer(cx: &App) -> impl IntoElement {
    layer(false, cx)
}

pub(crate) fn window_layer(cx: &App) -> impl IntoElement {
    layer(true, cx)
}

fn layer(under_chrome: bool, cx: &App) -> impl IntoElement {
    let opacity = cx
        .try_global::<VisualEffects>()
        .and_then(|effects| effects.wallpaper.as_ref())
        .map_or(1.0, |wp| wp.opacity);
    // Canvas 的 style.paint 不吃元素 opacity;Div 承载作用域,不把 alpha
    // 烘进图片像素。
    div().absolute().inset_0().opacity(opacity).child(
        gpui::canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| paint_wallpaper(bounds, under_chrome, window, cx),
        )
        .size_full(),
    )
}

fn image_bounds(wp: &Wallpaper, anchor: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    let (x, y, width, height) = wallpaper_rect(
        f32::from(anchor.size.width) * scale,
        f32::from(anchor.size.height) * scale,
        wp.width as f32,
        wp.height as f32,
        wp.fit,
        wp.alignment,
    );
    Bounds::new(
        anchor.origin + point(px(x / scale), px(y / scale)),
        size(px(width / scale), px(height / scale)),
    )
}

fn paint_wallpaper(bounds: Bounds<Pixels>, under_chrome: bool, window: &mut Window, cx: &App) {
    let Some(effects) = cx.try_global::<VisualEffects>() else {
        return;
    };
    let Some(wp) = effects.wallpaper.as_ref() else {
        return;
    };
    if wp.opacity <= 0.0 || under_chrome != wp.cover_chrome {
        return;
    }
    let Some(image) = wp.image.as_ref() else {
        return;
    };
    // 整窗模式锚定视口,卡模式锚定本层 bounds(终端内容区)。
    let anchor = if wp.cover_chrome {
        Bounds::new(point(px(0.0), px(0.0)), window.viewport_size())
    } else {
        bounds
    };
    let image_bounds = image_bounds(wp, anchor, window.scale_factor().max(0.5));
    // 终端区尚无圆角卡几何(卡几何消费未接),四角恒零。
    if let Err(error) = window.paint_image(
        bounds,
        image_bounds,
        Corners::default(),
        image.clone(),
        0,
        false,
    ) {
        log::warn!("background image paint failed: {error}");
    }
}

/// 供测试构造视效全局(仅 native material 与壳色断言使用)。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) fn test_install_visual_effects(cx: &mut App, opacity: f32, blur: BlurMode) {
    cx.set_global(VisualEffects {
        opacity,
        blur,
        wallpaper: None,
        generation: Arc::new(AtomicU64::new(0)),
        loading: false,
        load_task: None,
    });
}

#[cfg(test)]
mod window_effect_tests {
    use super::*;

    /// 材质字符串 round-trip 由 `slterm-settings` 保证;这里钉
    /// 死启动 gate 矩阵:平台 build × 材质 → 背景外观。
    #[test]
    fn background_appearance_gate_matrix_follows_windows_builds() {
        use WindowBackgroundAppearance as Appearance;
        let cases = [
            (BlurMode::None, Appearance::Transparent),
            (BlurMode::Aero, Appearance::Blurred),
        ];
        for build in [0, 17_762, 17_763, 19_045, 20_000, 22_620, 22_621] {
            for (blur, expected) in cases {
                assert_eq!(
                    background_appearance_for(blur, build),
                    expected,
                    "{blur:?} @ build {build}",
                );
            }
            // Acrylic 22H2+ 借系统 backdrop 通道(GPUI 先写 MicaBackdrop,
            // 壳侧随后改写为 TRANSIENTWINDOW);旧系统回退 WCA Acrylic。
            let expected_acrylic = if build >= 22_621 {
                Appearance::MicaBackdrop
            } else {
                Appearance::Blurred
            };
            assert_eq!(
                background_appearance_for(BlurMode::Acrylic, build),
                expected_acrylic,
                "Acrylic @ build {build}",
            );
            let expected_mica = if build >= 22_621 {
                Appearance::MicaBackdrop
            } else if build >= 17_763 {
                Appearance::Blurred
            } else {
                Appearance::Transparent
            };
            assert_eq!(
                background_appearance_for(BlurMode::Mica, build),
                expected_mica,
                "Mica @ build {build}",
            );
            let expected_mica_alt = if build >= 22_621 {
                Appearance::MicaAltBackdrop
            } else if build >= 17_763 {
                Appearance::Blurred
            } else {
                Appearance::Transparent
            };
            assert_eq!(
                background_appearance_for(BlurMode::MicaAlt, build),
                expected_mica_alt,
                "MicaAlt @ build {build}",
            );
        }
    }

    /// 静默启动缝:视效模块只改背景外观与绘制,不得自带
    /// 任何曝光/聚焦原语;首窗 `show=false`/`focus=false` 由开窗链独占。
    #[test]
    fn visual_effects_never_show_or_activate_windows() {
        // 符号运行时拼出,避免本测试源码自身命中扫描。
        let forbidden = [
            ["activate", "_window("].concat(),
            ["Show", "Window("].concat(),
            ["SetForeground", "Window("].concat(),
            ["BringWindow", "ToTop("].concat(),
        ];
        let source = include_str!("wallpaper.rs");
        for symbol in forbidden {
            assert!(
                !source.contains(&symbol),
                "wallpaper.rs 不得调用 {symbol}:静默窗口形态由开窗链独占"
            );
        }
    }
}
