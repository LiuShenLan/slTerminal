//! 窗口材质能力：按系统构建号判定原生支持、回退与不可用。
//!
//! 设置页据此禁用不可用档位并说明回退；不改变主题 token。

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
}
