//! 首帧 present-then-show(照上游 platform/startup/first_frame.rs 随迁):
//! 常规启动窗口先同步画完一帧,再对用户可见。
//!
//! 因果链:GPUI 在 `App::open_window` 里构建场景,但其 Windows 后端通常在场景
//! 存在之前就把 HWND show 出来;next-frame 回调先于呈现执行,回调与 `App::defer`
//! 都不是绘制屏障——不显式同步一帧,用户会看到首帧黑窗/半构窗口。

use gpui::{AnyWindowHandle, App, WindowOptions};

/// 请求延迟出窗:保留激活意图(GPUI 在 activate_window 前保留原始 placement),
/// 返回是否发生了延迟。静默/后台路径不进这里(show/focus 已关)。
pub(crate) fn defer_show(options: &mut WindowOptions) -> bool {
    if !cfg!(windows) || !options.show || !options.focus {
        return false;
    }
    options.show = false;
    true
}

/// WM_PAINT 同步一帧后才 activate:隐藏窗口的首帧绘制/呈现完成前不曝光。
pub(crate) fn present_then_show(handle: AnyWindowHandle, cx: &mut App) {
    cx.spawn(async move |cx| {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_PAINT};

        let hwnd = handle.update(cx, |_, window, _| {
            let native = HasWindowHandle::window_handle(window).ok()?;
            let RawWindowHandle::Win32(native) = native.as_raw() else {
                return None;
            };
            Some(native.hwnd.get())
        });
        // 虚拟窗口(测试)无原生句柄:拿不到即放弃延迟出窗,窗口按原样呈现。
        let Ok(Some(hwnd)) = hwnd else { return };

        // SAFETY: 一次性任务跑在 GPUI 前台线程;句柄取自上面的活 GPUI 窗口,
        // SendMessage 期间不持有任何 App/Window 借用:WM_PAINT 会重入 GPUI 的
        // 帧回调。钉版 GPUI Windows 即使隐藏也同步绘制/呈现 WM_PAINT;显式投递
        // 避免依赖 Windows 对隐藏窗口的 WM_PAINT 生成。无定时器、无轮询。
        unsafe { SendMessageW(hwnd as _, WM_PAINT, 0, 0) };

        // paint 处理器返回后才走 GPUI activate,消费其 pending placement;
        // 裸 ShowWindow 会让那份 placement 滞留过期。
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    })
    .detach();
}

#[cfg(test)]
mod first_frame_tests {
    use super::*;

    #[test]
    fn only_visible_focused_windows_defer_their_initial_show() {
        for (show, focus) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut options = WindowOptions {
                show,
                focus,
                ..Default::default()
            };
            let deferred = defer_show(&mut options);
            assert_eq!(deferred, cfg!(windows) && show && focus);
            assert_eq!(options.show, show && !deferred);
            assert_eq!(options.focus, focus);
        }
    }
}
