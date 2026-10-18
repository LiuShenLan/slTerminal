//! 窗口视效:背景模糊材质 / 窗口透明度 / 壁纸(09 篇 M10.7)。
//!
//! 三条语义边界(09 篇边界 11):
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

#[cfg(windows)]
use std::sync::OnceLock;

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
    /// 窗口背景材质(D09-1 五档,默认 None)。
    pub blur: BlurMode,
    wallpaper: Option<Wallpaper>,
    generation: Arc<AtomicU64>,
    loading: bool,
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
    let Some(wp) = effects.wallpaper.as_ref() else {
        return;
    };
    let request = image_loader::Request {
        path: wp.path.clone(),
        cached: wp.stamp.clone(),
        generation: effects.generation.clone(),
        version: effects.generation.load(Ordering::Acquire),
    };
    let version = request.version;
    effects.loading = true;
    let task = cx
        .background_executor()
        .spawn(async move { image_loader::load(request) });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let stale = cx
                .try_global::<VisualEffects>()
                .is_none_or(|effects| effects.generation.load(Ordering::Acquire) != version);
            if cx.has_global::<VisualEffects>() {
                cx.global_mut::<VisualEffects>().loading = false;
            }
            if stale {
                // 旧请求作废:按最新代际重新排队(单 job 串行)。
                start_load(cx);
                return;
            }
            match result {
                Ok(Some(loaded)) => {
                    let Some(wp) = cx.global_mut::<VisualEffects>().wallpaper.as_mut() else {
                        return;
                    };
                    wp.width = loaded.layout_width;
                    wp.height = loaded.layout_height;
                    wp.stamp = Some(loaded.stamp);
                    let image = Arc::new(RenderImage::new([Frame::new(loaded.pixels)]));
                    let retired = wp.image.replace(image);
                    retire_image(retired, cx);
                    refresh_surface_opacity(cx);
                    cx.refresh_windows();
                }
                // 文件戳未变 = 缓存命中,保留现纹理。
                Ok(None) => {}
                Err(error) => {
                    // 解码失败降级:保留旧纹理或空壁纸,只记日志,不打断启动。
                    log::warn!("background image load failed: {error:?}");
                }
            }
        });
    })
    .detach();
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

/// 开窗参数用:首帧即走平台 backdrop,不必等 `refresh` 补第二次。
///
/// Mica/MicaAlt 在 22H2(22621)走原生枚举;1809(17763)+ 回退经典
/// 模糊;更老系统与 `None` 一律透明交换链——GPUI Windows 渲染器按
/// `Opaque` 固定 alpha=1 清屏,保留透明才能让绘制像素的 alpha 生效。
pub(crate) fn initial_background_appearance(cx: &App) -> WindowBackgroundAppearance {
    let blur = cx
        .try_global::<Settings>()
        .map_or(BlurMode::None, |settings| settings.blur);
    background_appearance(blur)
}

fn background_appearance(blur: BlurMode) -> WindowBackgroundAppearance {
    #[cfg(windows)]
    let build = windows_build_number();
    #[cfg(not(windows))]
    let build = u32::MAX;
    background_appearance_for(blur, build)
}

/// 纯函数 gate:build 决定 Mica 档位走原生枚举还是 AccentPolicy 回退。
fn background_appearance_for(blur: BlurMode, build: u32) -> WindowBackgroundAppearance {
    match blur {
        BlurMode::Aero | BlurMode::Acrylic => WindowBackgroundAppearance::Blurred,
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
                #[cfg(windows)]
                apply_windows_accent_policy(window, blur, appearance);
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

/// `GetVersionEx` 受应用兼容清单影响;RtlGetVersion 才能可靠判断公开的
/// `DWMWA_SYSTEMBACKDROP_TYPE` 是否存在。缓存结果,热切换不重复进内核。
#[cfg(windows)]
fn windows_build_number() -> u32 {
    static BUILD: OnceLock<u32> = OnceLock::new();
    *BUILD.get_or_init(|| {
        use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
        use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;

        // SAFETY: 输出结构按系统约定先写 Size;失败返回 0 走退化路径。
        let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
        info.dwOSVersionInfoSize = std::mem::size_of_val(&info) as u32;
        let status = unsafe { RtlGetVersion(&mut info) };
        if status == 0 { info.dwBuildNumber } else { 0 }
    })
}

/// 窗口材质落盘计划:把「平台 build / 材质档 / 系统 backdrop 是否可用」
/// 折算成一组无副作用的动作,供真实 Win32 调用与纯函数测试共用。
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MaterialPlan {
    /// 先清旧 WCA 层再写系统 backdrop/Acrylic。
    clear_accent_first: bool,
    /// `DwmEnableBlurBehindWindow` 的 enable 值(Aero 开,其余显式关)。
    blur_behind: bool,
    /// `DWMWA_SYSTEMBACKDROP_TYPE` 写入值;None = 系统无该属性,不调用。
    backdrop: Option<i32>,
    /// 最终 AccentPolicy state:0 关 / 3 Aero 模糊 / 4 Acrylic。
    accent_state: u32,
    /// AccentPolicy flags;禁用态保持标准边框绘制语义(2)。
    accent_flags: u32,
    /// AccentPolicy 渐变色调(ARGB)。
    accent_color: u32,
    /// 系统 backdrop 已生效时不再叠 AccentPolicy(双通道同开行为未定义)。
    apply_accent: bool,
}

#[cfg(windows)]
fn material_plan(
    blur: BlurMode,
    appearance: WindowBackgroundAppearance,
    build: u32,
    system_available: bool,
) -> MaterialPlan {
    use windows_sys::Win32::Graphics::Dwm::{DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW};

    let system_requested = matches!(
        appearance,
        WindowBackgroundAppearance::MicaBackdrop | WindowBackgroundAppearance::MicaAltBackdrop
    );
    // 公开 system-backdrop 属性仅存在于 22621+;旧系统连「清回 NONE」也不写。
    let backdrop = if build >= 22_621 {
        Some(match appearance {
            WindowBackgroundAppearance::MicaBackdrop => DWMSBT_MAINWINDOW,
            WindowBackgroundAppearance::MicaAltBackdrop => DWMSBT_TABBEDWINDOW,
            _ => DWMSBT_NONE,
        })
    } else {
        None
    };
    let (accent_state, accent_flags, accent_color) = match blur {
        BlurMode::Acrylic => (4, 0, 0x0100_0000),
        BlurMode::Aero => (3, 0, 0x982B_2B2B),
        // 1809..22H2 回退经典模糊;新系统原生 backdrop 调用失败时同样
        // 保留 Acrylic 兜底,避免透明空洞。
        BlurMode::Mica | BlurMode::MicaAlt
            if matches!(appearance, WindowBackgroundAppearance::Blurred)
                || (system_requested && !system_available) =>
        {
            (4, 0, 0x0100_0000)
        }
        BlurMode::Mica | BlurMode::MicaAlt | BlurMode::None => (0, 2, 0),
    };
    MaterialPlan {
        // 反过来先写 DWMSBT 时,Aero/Acrylic 的旧 AccentPolicy 会阻止 DWM
        // 接纳新材质,事后再清也不会自动重算 frame,故必须先清。
        clear_accent_first: system_requested || blur == BlurMode::Acrylic,
        blur_behind: blur == BlurMode::Aero,
        backdrop,
        accent_state,
        accent_flags,
        accent_color,
        apply_accent: !(system_requested && system_available),
    }
}

/// 显式落下 Windows 原生材质属性(WCA + DWM 双通道)。
///
/// | 档位 | AccentPolicy | SYSTEMBACKDROP |
/// |---|---|---|
/// | None | 全零 | DWMSBT_NONE |
/// | Aero | state 3 + 玻璃色调 | DWMSBT_NONE |
/// | Mica | 全零 | DWMSBT_MAINWINDOW |
/// | Mica Alt | 全零 | DWMSBT_TABBEDWINDOW |
/// | Acrylic | state 4 | DWMSBT_NONE |
///
/// 两条通道必须互斥:同时开系统 backdrop 与 Acrylic 时 DWM 行为未定义
/// (实测 backdrop 吞 Acrylic),故每档都把另一条显式写回中性值。
#[cfg(windows)]
fn apply_windows_accent_policy(
    window: &Window,
    blur: BlurMode,
    appearance: WindowBackgroundAppearance,
) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::Graphics::Dwm::{
        DWM_BB_ENABLE, DWM_BLURBEHIND, DWMWA_SYSTEMBACKDROP_TYPE, DwmEnableBlurBehindWindow,
        DwmSetWindowAttribute,
    };
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }

    #[repr(C)]
    struct WindowCompositionAttributeData {
        attribute: u32,
        data: *mut core::ffi::c_void,
        size: usize,
    }

    type SetWindowCompositionAttribute = unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HWND,
        *mut WindowCompositionAttributeData,
    ) -> BOOL;

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        // 虚拟窗口(测试)无原生句柄:平台材质面到此为止。
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let set_attribute: Option<SetWindowCompositionAttribute> = unsafe {
        let user32 = GetModuleHandleA(c"user32.dll".as_ptr() as *const u8);
        if user32.is_null() {
            None
        } else {
            GetProcAddress(
                user32,
                c"SetWindowCompositionAttribute".as_ptr() as *const u8,
            )
            .map(|procedure| std::mem::transmute(procedure))
        }
    };
    if set_attribute.is_none() {
        log::warn!("SetWindowCompositionAttribute is unavailable in user32.dll");
    }

    let apply_accent = |state: u32, flags: u32, color: u32, phase: &str| {
        let Some(set_attribute) = set_attribute else {
            return false;
        };
        let mut accent = AccentPolicy {
            state,
            flags,
            gradient_color: color,
            animation_id: 0,
        };
        let mut data = WindowCompositionAttributeData {
            attribute: 19, // WCA_ACCENT_POLICY
            data: &mut accent as *mut _ as *mut core::ffi::c_void,
            size: std::mem::size_of::<AccentPolicy>(),
        };
        // SAFETY: hwnd 来自当前存活的 GPUI 窗口,数据在调用期间保持有效。
        if unsafe { set_attribute(hwnd, &mut data) } == 0 {
            log::warn!("SetWindowCompositionAttribute({phase}) failed");
            return false;
        }
        true
    };

    let build = windows_build_number();
    let provisional = material_plan(blur, appearance, build, false);
    // 先移除旧 WCA 层;随后写入 DWM backdrop,最后按可用性决定是否叠加
    // AccentPolicy 兜底。
    if provisional.clear_accent_first {
        apply_accent(0, 2, 0, "clear-before-system-backdrop");
    }

    let blur_behind = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE,
        fEnable: i32::from(provisional.blur_behind),
        hRgnBlur: std::ptr::null_mut(),
        fTransitionOnMaximized: 0,
    };
    // 对所有档位显式 enable/disable,避免从 Aero 热切换后遗留玻璃层。
    let blur_behind_result = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur_behind) };
    let backdrop_result = provisional.backdrop.map(|backdrop| unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &backdrop as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
    });
    let system_requested = matches!(
        appearance,
        WindowBackgroundAppearance::MicaBackdrop | WindowBackgroundAppearance::MicaAltBackdrop
    );
    let system_available = system_requested && backdrop_result.is_some_and(|result| result >= 0);
    let plan = material_plan(blur, appearance, build, system_available);
    if plan.apply_accent {
        apply_accent(
            plan.accent_state,
            plan.accent_flags,
            plan.accent_color,
            "final",
        );
    }

    // 重绘 GPUI 内容不足以让 DWM 重新读取 DWMSBT;材质切换是低频操作,
    // 在 AppliedBlur 门控后刷新一次非客户区 frame。
    if backdrop_result.is_some() {
        let frame_result = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
        };
        if frame_result == 0 {
            log::warn!("failed to refresh the window frame after changing system backdrop");
        }
    }

    if let Some(backdrop_result) = backdrop_result.filter(|result| *result < 0) {
        log::warn!(
            "DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE) failed: HRESULT=0x{:08X}",
            backdrop_result as u32
        );
    }
    if blur_behind_result < 0 {
        log::warn!(
            "DwmEnableBlurBehindWindow(enable={}) failed: HRESULT=0x{:08X}",
            provisional.blur_behind,
            blur_behind_result as u32
        );
    }
}

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
    // M10.7 终端区尚无圆角卡几何(06 篇卡几何消费未接),四角恒零。
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
    });
}

#[cfg(test)]
mod window_effect_tests {
    use super::*;

    /// D09-1 五材质字符串 round-trip 由 `slterm-settings` 保证;这里钉
    /// 死启动 gate 矩阵:平台 build × 材质 → 背景外观。
    #[test]
    fn background_appearance_gate_matrix_follows_windows_builds() {
        use WindowBackgroundAppearance as Appearance;
        let cases = [
            (BlurMode::None, Appearance::Transparent),
            (BlurMode::Aero, Appearance::Blurred),
            (BlurMode::Acrylic, Appearance::Blurred),
        ];
        for build in [0, 17_762, 17_763, 20_000, 22_620, 22_621] {
            for (blur, expected) in cases {
                assert_eq!(
                    background_appearance_for(blur, build),
                    expected,
                    "{blur:?} @ build {build}",
                );
            }
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

    /// 静默启动缝(M10.6/M10.7):视效模块只改背景外观与绘制,不得自带
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

    /// 原生通道计划:材质档 × build × backdrop 可用性 → 双通道互斥动作。
    #[cfg(windows)]
    #[test]
    fn material_plan_keeps_the_two_native_channels_mutually_exclusive() {
        use WindowBackgroundAppearance as Appearance;
        use windows_sys::Win32::Graphics::Dwm::{
            DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW,
        };

        // None:两条通道都写中性值;不碰 backdrop(22621 以下)。
        for build in [17_763, 22_621] {
            let plan = material_plan(BlurMode::None, Appearance::Transparent, build, false);
            assert_eq!(plan.accent_state, 0);
            assert_eq!(plan.accent_flags, 2, "禁用态保留标准边框语义");
            assert!(!plan.blur_behind);
            assert!(plan.apply_accent);
            assert!(!plan.clear_accent_first);
            assert_eq!(plan.backdrop, (build >= 22_621).then_some(DWMSBT_NONE),);
        }

        // Aero:AccentPolicy state 3 + BlurBehind;系统 backdrop 写 NONE。
        let plan = material_plan(BlurMode::Aero, Appearance::Blurred, 22_621, false);
        assert_eq!((plan.accent_state, plan.accent_color), (3, 0x982B_2B2B));
        assert!(plan.blur_behind);
        assert!(plan.apply_accent);
        assert_eq!(plan.backdrop, Some(DWMSBT_NONE));

        // Acrylic:先清旧层再写 state 4。
        let plan = material_plan(BlurMode::Acrylic, Appearance::Blurred, 22_621, false);
        assert!(plan.clear_accent_first);
        assert_eq!((plan.accent_state, plan.accent_color), (4, 0x0100_0000));
        assert_eq!(plan.backdrop, Some(DWMSBT_NONE));

        // Mica 成功:只写 DWMSBT,AccentPolicy 保持全零且不再叠加。
        let plan = material_plan(BlurMode::Mica, Appearance::MicaBackdrop, 22_621, true);
        assert_eq!(plan.backdrop, Some(DWMSBT_MAINWINDOW));
        assert_eq!((plan.accent_state, plan.accent_flags), (0, 2));
        assert!(!plan.apply_accent, "双通道同开行为未定义");
        assert!(plan.clear_accent_first);
        assert!(!plan.blur_behind);

        let plan = material_plan(BlurMode::MicaAlt, Appearance::MicaAltBackdrop, 22_621, true);
        assert_eq!(plan.backdrop, Some(DWMSBT_TABBEDWINDOW));

        // Mica 原生调用失败:回退 Acrylic 兜底,避免透明空洞。
        let plan = material_plan(BlurMode::Mica, Appearance::MicaBackdrop, 22_621, false);
        assert_eq!((plan.accent_state, plan.accent_color), (4, 0x0100_0000));
        assert!(plan.apply_accent);

        // 1809..22H2:没有公开 backdrop,外观已回退 Blurred,走经典模糊。
        let plan = material_plan(BlurMode::Mica, Appearance::Blurred, 17_763, false);
        assert_eq!(plan.accent_state, 4);
        assert_eq!(plan.backdrop, None, "旧系统不写不存在的属性");

        // 低于 17763:纯透明,无材质动作。
        let plan = material_plan(BlurMode::Mica, Appearance::Transparent, 17_762, false);
        assert_eq!((plan.accent_state, plan.accent_flags), (0, 2));
        assert_eq!(plan.backdrop, None);
    }
}
