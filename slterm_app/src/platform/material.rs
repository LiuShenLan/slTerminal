//! 窗口材质能力：按系统构建号判定原生支持、回退与不可用。
//!
//! 设置页据此禁用不可用档位并说明回退；不改变主题 token。

use gpui::{Window, WindowBackgroundAppearance};
use slterm_settings::BlurMode;

/// 材质在当前系统的实际形态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlurSupport {
    /// 原生系统 backdrop 或系统模糊。
    Native,
    /// 旧系统上的经典模糊近似。
    BlurredFallback,
    /// 系统完全不支持，材质不生效。
    NoneFallback,
}

impl BlurSupport {
    pub(crate) fn is_selectable(self) -> bool {
        !matches!(self, Self::NoneFallback)
    }
}

/// 22H2 起提供 `DWMWA_SYSTEMBACKDROP_TYPE` 原生 Mica 与 Acrylic。
const NATIVE_BACKDROP_BUILD: u32 = 22_621;
/// 1809 起提供经典模糊（WCA AccentPolicy）。
const BLUR_BUILD: u32 = 17_763;

/// 纯函数：构建号决定档位的真实形态。
pub(crate) fn blur_support_for(mode: BlurMode, build: u32) -> BlurSupport {
    match mode {
        // Aero 的原生实现就是系统模糊，不构成回退。
        BlurMode::None | BlurMode::Aero => BlurSupport::Native,
        BlurMode::Mica | BlurMode::MicaAlt | BlurMode::Acrylic
            if build >= NATIVE_BACKDROP_BUILD =>
        {
            BlurSupport::Native
        }
        BlurMode::Mica | BlurMode::MicaAlt | BlurMode::Acrylic if build >= BLUR_BUILD => {
            BlurSupport::BlurredFallback
        }
        BlurMode::Mica | BlurMode::MicaAlt | BlurMode::Acrylic => BlurSupport::NoneFallback,
    }
}

/// 当前系统构建号决定档位形态。
pub(crate) fn blur_support(mode: BlurMode) -> BlurSupport {
    blur_support_for(mode, windows_build_number())
}

/// `GetVersionEx` 受应用兼容清单影响;RtlGetVersion 才能可靠判断公开的
/// `DWMWA_SYSTEMBACKDROP_TYPE` 是否存在。缓存结果,热切换不重复进内核。
#[cfg(windows)]
pub(crate) fn windows_build_number() -> u32 {
    static BUILD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
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

#[cfg(not(windows))]
pub(crate) fn windows_build_number() -> u32 {
    // 非 Windows 构建只供测试编译；能力判定按最新系统处理。
    u32::MAX
}

/// 把材质属性落到窗口(WCA + DWM 双通道);业务层只传档位与 GPUI 外观。
pub(crate) fn apply_window_appearance(
    window: &Window,
    blur: BlurMode,
    appearance: WindowBackgroundAppearance,
) {
    #[cfg(windows)]
    apply_accent_policy(window, blur, appearance);
    #[cfg(not(windows))]
    let _ = (window, blur, appearance);
}

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

/// 该档位是否请求原生系统 backdrop。
///
/// Acrylic 在 22H2+ 与 Mica 共用同一条系统 backdrop 通道(材质值
/// `DWMSBT_TRANSIENTWINDOW`),旧系统没有该通道,只能走 WCA 回退。
#[cfg(windows)]
fn system_backdrop_requested(
    blur: BlurMode,
    appearance: WindowBackgroundAppearance,
    build: u32,
) -> bool {
    if blur == BlurMode::Acrylic {
        return build >= 22_621;
    }
    matches!(
        appearance,
        WindowBackgroundAppearance::MicaBackdrop | WindowBackgroundAppearance::MicaAltBackdrop
    )
}

#[cfg(windows)]
fn material_plan(
    blur: BlurMode,
    appearance: WindowBackgroundAppearance,
    build: u32,
    system_available: bool,
) -> MaterialPlan {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW,
    };

    let system_requested = system_backdrop_requested(blur, appearance, build);
    // 公开 system-backdrop 属性仅存在于 22621+;旧系统连「清回 NONE」也不写。
    // Acrylic 的原生材质即 `DWMSBT_TRANSIENTWINDOW`(与 WinAppSDK
    // DesktopAcrylicController 默认视觉在 DWM 侧等价)。
    let backdrop = if build >= 22_621 {
        Some(match blur {
            BlurMode::Acrylic => DWMSBT_TRANSIENTWINDOW,
            _ => match appearance {
                WindowBackgroundAppearance::MicaBackdrop => DWMSBT_MAINWINDOW,
                WindowBackgroundAppearance::MicaAltBackdrop => DWMSBT_TABBEDWINDOW,
                _ => DWMSBT_NONE,
            },
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
/// | Acrylic (22H2+) | 失败时 state 4 | DWMSBT_TRANSIENTWINDOW |
/// | Acrylic (旧系统) | state 4 | 不写(无该属性) |
///
/// 两条通道必须互斥:同时开系统 backdrop 与 Acrylic 时 DWM 行为未定义
/// (实测 backdrop 吞 Acrylic),故每档都把另一条显式写回中性值。
///
/// Acrylic 在 22H2+ 直接走系统 backdrop `DWMSBT_TRANSIENTWINDOW`,与
/// WinAppSDK `DesktopAcrylicController` 的默认视觉在 DWM 侧等价;系统
/// backdrop 写入失败时清回 `DWMSBT_NONE` 并叠 WCA state 4 兜底。
#[cfg(windows)]
fn apply_accent_policy(window: &Window, blur: BlurMode, appearance: WindowBackgroundAppearance) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::Graphics::Dwm::{
        DWM_BB_ENABLE, DWM_BLURBEHIND, DWMSBT_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
        DwmEnableBlurBehindWindow, DwmSetWindowAttribute,
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
    let system_requested = system_backdrop_requested(blur, appearance, build);
    let system_available = system_requested && backdrop_result.is_some_and(|result| result >= 0);
    // 系统 backdrop 写入失败时显式清回 NONE,避免旧 backdrop 残留;
    // 随后的 final 计划会落下 WCA state 4 兜底,不出现透明空洞。
    if system_requested && !system_available {
        let none = DWMSBT_NONE;
        // SAFETY: hwnd 来自当前存活窗口,值在调用期间保持有效。
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &none as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
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

#[cfg(test)]
mod material_tests {
    use super::*;

    /// 22H2 起原生 Mica/MicaAlt/Acrylic；更老系统明确回退或不生效。
    #[test]
    fn blur_support_follows_the_windows_build() {
        assert_eq!(
            blur_support_for(BlurMode::Mica, 22_621),
            BlurSupport::Native
        );
        assert_eq!(
            blur_support_for(BlurMode::MicaAlt, 22_621),
            BlurSupport::Native
        );
        assert_eq!(
            blur_support_for(BlurMode::Acrylic, 22_621),
            BlurSupport::Native
        );
        assert_eq!(
            blur_support_for(BlurMode::Mica, 19_045),
            BlurSupport::BlurredFallback
        );
        assert_eq!(
            blur_support_for(BlurMode::Acrylic, 19_045),
            BlurSupport::BlurredFallback
        );
        assert_eq!(
            blur_support_for(BlurMode::Mica, 17_762),
            BlurSupport::NoneFallback
        );
        assert_eq!(
            blur_support_for(BlurMode::Acrylic, 17_762),
            BlurSupport::NoneFallback
        );
    }

    /// None 与 Aero 不依赖 22H2；Aero 的原生形态就是系统模糊。
    #[test]
    fn none_and_aero_are_always_available() {
        assert_eq!(blur_support_for(BlurMode::None, 0), BlurSupport::Native);
        assert_eq!(blur_support_for(BlurMode::Aero, 0), BlurSupport::Native);
        assert!(blur_support_for(BlurMode::None, 0).is_selectable());
        assert!(!blur_support_for(BlurMode::Mica, 0).is_selectable());
    }

    /// 原生通道计划:材质档 × build × backdrop 可用性 → 双通道互斥动作。
    #[cfg(windows)]
    #[test]
    fn material_plan_keeps_the_two_native_channels_mutually_exclusive() {
        use WindowBackgroundAppearance as Appearance;
        use windows_sys::Win32::Graphics::Dwm::{
            DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW,
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

        // Acrylic 22H2+ 原生系统 backdrop:先清旧 WCA 层,写
        // TRANSIENTWINDOW,成功时不再叠 AccentPolicy。
        let plan = material_plan(BlurMode::Acrylic, Appearance::MicaBackdrop, 22_621, true);
        assert!(plan.clear_accent_first);
        assert_eq!(plan.backdrop, Some(DWMSBT_TRANSIENTWINDOW));
        assert!(!plan.apply_accent, "系统 backdrop 成功时双通道互斥");
        assert_eq!((plan.accent_state, plan.accent_color), (4, 0x0100_0000));
        assert!(!plan.blur_behind);

        // Acrylic 22H2+ 系统 backdrop 失败:清回 NONE 后落 WCA state 4。
        let plan = material_plan(BlurMode::Acrylic, Appearance::MicaBackdrop, 22_621, false);
        assert_eq!(plan.backdrop, Some(DWMSBT_TRANSIENTWINDOW));
        assert!(plan.apply_accent);
        assert_eq!((plan.accent_state, plan.accent_color), (4, 0x0100_0000));

        // Acrylic 旧系统(19045):无 system-backdrop 属性,只走 WCA state 4。
        let plan = material_plan(BlurMode::Acrylic, Appearance::Blurred, 19_045, false);
        assert_eq!(plan.backdrop, None, "旧系统不写不存在的属性");
        assert!(plan.apply_accent);
        assert!(plan.clear_accent_first);
        assert_eq!(plan.accent_state, 4);

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
