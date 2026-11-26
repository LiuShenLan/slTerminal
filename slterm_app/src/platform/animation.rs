//! 系统动画偏好：光标闪烁默认值跟随「减少动效」。

#[cfg(windows)]
pub(crate) fn reduced_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    let mut animations = 1_i32;
    // SAFETY: 查询系统参数，传入本函数栈上的 i32 指针；失败时不读结果。
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut animations as *mut i32).cast(),
            0,
        )
    };
    ok != 0 && animations == 0
}

#[cfg(not(windows))]
pub(crate) fn reduced_motion() -> bool {
    false
}
