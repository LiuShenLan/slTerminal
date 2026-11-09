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

/// WM_PAINT 同步一帧后才 activate；随后只发一次可服务通知。
pub(crate) fn present_then_show(
    handle: AnyWindowHandle,
    cx: &mut App,
    on_ready: impl FnOnce() + Send + 'static,
) {
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
        let Ok(hwnd) = hwnd else { return };
        let Some(hwnd) = hwnd else {
            // 仅测试虚拟窗口没有原生句柄；生产拿不到 HWND 必须保持未开放。
            #[cfg(test)]
            on_ready();
            return;
        };

        // SAFETY: 一次性任务跑在 GPUI 前台线程;句柄取自上面的活 GPUI 窗口,
        // SendMessage 期间不持有任何 App/Window 借用:WM_PAINT 会重入 GPUI 的
        // 帧回调。钉版 GPUI Windows 即使隐藏也同步绘制/呈现 WM_PAINT;显式投递
        // 避免依赖 Windows 对隐藏窗口的 WM_PAINT 生成。无定时器、无轮询。
        unsafe { SendMessageW(hwnd as _, WM_PAINT, 0, 0) };

        // paint 处理器返回后才走 GPUI activate,消费其 pending placement;
        // 裸 ShowWindow 会让那份 placement 滞留过期。
        let activated = handle.update(cx, |_, window, _| window.activate_window());
        if activated.is_ok() {
            on_ready();
        }
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

#[cfg(all(test, feature = "gpui-test-support"))]
mod readiness_tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, Window};

    use super::present_then_show;

    struct ReadyView;

    impl Render for ReadyView {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div()
        }
    }

    #[gpui::test]
    fn virtual_present_signals_readiness_exactly_once(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| ReadyView);
        let handle = window
            .update(cx, |_, window, _| window.window_handle())
            .unwrap();
        let (ready, ready_rx) = std::sync::mpsc::sync_channel(1);
        cx.update(|cx| {
            present_then_show(handle, cx, move || ready.send(()).unwrap());
        });

        cx.run_until_parked();

        assert_eq!(ready_rx.try_recv(), Ok(()));
        assert!(ready_rx.try_recv().is_err());
    }

    #[gpui::test]
    fn destroyed_window_does_not_signal_readiness(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| ReadyView);
        let handle = window
            .update(cx, |_, window, _| {
                let handle = window.window_handle();
                window.remove_window();
                handle
            })
            .unwrap();
        let (ready, ready_rx) = std::sync::mpsc::sync_channel(1);
        cx.update(|cx| {
            present_then_show(handle, cx, move || ready.send(()).unwrap());
        });

        cx.run_until_parked();

        assert!(ready_rx.try_recv().is_err());
    }
}
