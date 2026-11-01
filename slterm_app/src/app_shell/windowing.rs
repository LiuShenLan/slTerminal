//! 开窗链：窗口选项、AppRoot 初始项目与首帧曝光。
//!
//! 单窗单实例；本模块不读取旧 session 或自行恢复布局。

pub(super) mod startup_geometry;

use std::sync::mpsc;

use gpui::{App, AppContext as _, Bounds, Size, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, TitleBar};

use super::SltermShellEvent;
use super::app_root::{AppRoot, AppRootPersistence};
use crate::brand;
use crate::platform::startup::first_frame;
use slterm_path::CanonicalPath;
use slterm_workspace::PanelIdAllocator;

/// 窗口最小尺寸(旧产品面同值)。
const MIN_WINDOW_SIZE: (f32, f32) = (760.0, 540.0);

/// 把首选尺寸放进显示器可见区(居中);无显示器信息时交给调用方的
/// `Bounds::centered` 兜底。纯函数,回放用例直接断言。
fn fit_to_display(
    preferred: Size<gpui::Pixels>,
    visible: Option<Bounds<gpui::Pixels>>,
) -> Option<Bounds<gpui::Pixels>> {
    let visible = visible?;
    let fitted = preferred.min(&visible.size);
    Some(Bounds::new(
        gpui::point(
            visible.origin.x + (visible.size.width - fitted.width) / 2.0,
            visible.origin.y + (visible.size.height - fitted.height) / 2.0,
        ),
        fitted,
    ))
}

/// 窗口定形唯一函数。新根尚未恢复几何，统一使用终端网格基准尺寸。
pub(crate) fn workspace_window_options(cx: &App, focus: bool) -> WindowOptions {
    let fallback =
        startup_geometry::preferred_size(cx).unwrap_or_else(|| size(px(1080.0), px(720.0)));
    let preferred = fallback.max(&size(px(MIN_WINDOW_SIZE.0), px(MIN_WINDOW_SIZE.1)));
    let bounds = fit_to_display(
        preferred,
        cx.primary_display().map(|display| display.visible_bounds()),
    )
    .unwrap_or_else(|| Bounds::centered(None, preferred, cx));
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(MIN_WINDOW_SIZE.0), px(MIN_WINDOW_SIZE.1)).min(&bounds.size)),
        titlebar: Some(TitleBar::title_bar_options()),
        app_id: Some(brand::WINDOWS_APP_ID.to_owned()),
        // 首帧即平台 backdrop:Mica/MicaAlt/Acrylic(22H2+)
        // 走系统 backdrop 通道,其中 Acrylic 由壳侧改写为 TRANSIENTWINDOW;
        // Aero 与旧系统 Acrylic 走 Blurred;None 也保留透明交换链——GPUI
        // Windows 渲染器按枚举选清屏 alpha,Opaque 会盖死透明度与材质。
        window_background: crate::app_shell::wallpaper::initial_background_appearance(cx),
        focus,
        ..Default::default()
    }
}

/// 静默启动的窗口形态:不显示、不抢焦点。
///
/// 隐藏窗口不进入 present-then-show 链——该链只为「即将曝光」的窗口
/// 防首帧黑窗;托盘「显示」时 GPUI `activate_window` 消费首窗 placement
/// 完成曝光与前台化(facade 同路径)。
fn apply_start_hidden(options: &mut WindowOptions, start_hidden: bool) {
    if start_hidden {
        options.show = false;
        options.focus = false;
    }
}

fn should_open_initial_project(
    regular_start: bool,
    has_restored_projects: bool,
    recovery_failed: bool,
) -> bool {
    if regular_start {
        !recovery_failed && !has_restored_projects
    } else {
        true
    }
}

/// 启动链调一次的开窗入口：只消费进入 GPUI 前冻结的 Shell 与项目根。
pub(crate) fn open_initial_window(
    cx: &mut App,
    shell_rx: mpsc::Receiver<SltermShellEvent>,
    initial_root: CanonicalPath,
    shell: CanonicalPath,
    persistence: AppRootPersistence,
    start_hidden: bool,
    regular_start: bool,
) {
    let mut options = workspace_window_options(cx, !start_hidden);
    apply_start_hidden(&mut options, start_hidden);
    // 首帧黑窗/半构窗口不曝光:先同步画一帧再 activate(first_frame 因果链)。
    // 静默窗口不曝光,defer_show 自然返回 false,不进 present-then-show。
    let deferred_show = first_frame::defer_show(&mut options);
    let result = cx.open_window(options, move |window, cx| {
        window.set_window_title(brand::NAME);
        // HWND 发布单点:任务栏闪烁/进度经 facade 查询。
        crate::windowing::focus::publish_window(window);
        let root = cx.new(|cx| AppRoot::new(PanelIdAllocator::new(1), shell, persistence, cx));
        root.update(cx, |root, cx| {
            root.observe_focus(window, cx);
            root.restore_active_project(window, cx);
            let should_open_initial = should_open_initial_project(
                regular_start,
                !root.snapshot().registry().projects().is_empty(),
                root.project_snapshot_recovery_failed(),
            );
            if should_open_initial
                && let Err(error) = root.open_initial_project(initial_root, window, cx)
            {
                log::error!("初始项目启动失败: {error}");
                super::try_write_stderr(format_args!("初始项目启动失败: {error}"));
            }
            root.start_shell_event_pump(shell_rx, window.window_handle(), cx);
        });
        cx.new(|cx| Root::new(root, window, cx))
    });
    match result {
        Ok(handle) => {
            // 视效落窗:应用原生 AccentPolicy/DWMSBT 并启动
            // 壁纸加载。只改背景外观与绘制,不 show/activate——静默窗口的
            // `apply_start_hidden` 形态不被覆盖。
            crate::app_shell::wallpaper::refresh(cx);
            if deferred_show {
                first_frame::present_then_show(handle.into(), cx);
            }
        }
        Err(error) => {
            log::error!("failed to open main window: {error}");
            super::try_write_stderr(format_args!("failed to open main window: {error}"));
        }
    }
}

#[cfg(test)]
mod windowing_tests {
    use super::*;

    /// 首选尺寸大于显示器时收进可见区。
    #[test]
    fn preferred_window_size_fits_the_display() {
        let display = Bounds::new(gpui::point(px(0.0), px(0.0)), size(px(1000.0), px(600.0)));
        let bounds = fit_to_display(size(px(4000.0), px(3000.0)), Some(display)).unwrap();
        assert_eq!(bounds.size, display.size, "超屏尺寸收到显示器内");
    }

    /// 常规启动恢复既有项目，不额外打开 cwd；显式目录仍进入既有打开链。
    #[test]
    fn regular_start_does_not_open_cwd_over_restored_projects() {
        assert!(!should_open_initial_project(true, true, false));
        assert!(should_open_initial_project(true, false, false));
        assert!(should_open_initial_project(false, true, false));
        assert!(should_open_initial_project(false, false, false));
        assert!(!should_open_initial_project(true, false, true));
        assert!(should_open_initial_project(false, false, true));
    }

    /// 静默启动只改 `show`/`focus` 两轴,且不进入 present-then-show 链;
    /// 常规启动保持可视 + 聚焦形态(首帧纪律不变)。
    ///
    /// 缝合点:视效初始化只写背景外观,不得回写 `show`/`focus`。
    /// 这里把透明背景提前放进 options,验证 `apply_start_hidden` 的
    /// show/focus=false 仍是最终形态。
    #[test]
    fn start_hidden_hides_the_window_and_skips_first_frame_present() {
        let mut hidden = WindowOptions {
            show: true,
            focus: true,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        apply_start_hidden(&mut hidden, true);
        assert!(!hidden.show, "静默启动不得显示窗口");
        assert!(!hidden.focus, "静默启动不得抢焦点");
        assert_eq!(
            hidden.window_background,
            gpui::WindowBackgroundAppearance::Transparent,
            "视效背景外观不得被静默启动改写"
        );
        assert!(
            !first_frame::defer_show(&mut hidden),
            "未曝光窗口不进入 present-then-show"
        );

        let mut visible = WindowOptions {
            show: true,
            focus: true,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        apply_start_hidden(&mut visible, false);
        assert!(visible.show && visible.focus, "常规启动形态不变");
        assert_eq!(
            visible.window_background,
            gpui::WindowBackgroundAppearance::Transparent
        );
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod input_lifecycle_tests {
    use std::{cell::Cell, ops::Range, rc::Rc};

    use gpui::{
        App, Bounds, Context, Entity, FocusHandle, InputHandler, IntoElement, ParentElement as _,
        Pixels, Point, Render, Styled as _, TestAppContext, UTF16Selection, Window, canvas, div,
    };

    struct InputView {
        focus_handle: FocusHandle,
        live_handlers: Rc<Cell<usize>>,
    }

    struct TrackedInputHandler {
        // 与 ElementInputHandler 相同，持有视图强句柄。
        _view: Entity<InputView>,
        live_handlers: Rc<Cell<usize>>,
    }

    impl Drop for TrackedInputHandler {
        fn drop(&mut self) {
            self.live_handlers
                .set(self.live_handlers.get().checked_sub(1).unwrap());
        }
    }

    impl Render for InputView {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
            let focus_handle = self.focus_handle.clone();
            let live_handlers = self.live_handlers.clone();
            let view = cx.entity().clone();
            div().size_full().child(canvas(
                |_, _, _| {},
                move |_, _, window, cx| {
                    assert!(focus_handle.is_focused(window), "测试必须先聚焦输入视图");
                    live_handlers.set(live_handlers.get() + 1);
                    window.handle_input(
                        &focus_handle,
                        TrackedInputHandler {
                            _view: view.clone(),
                            live_handlers: live_handlers.clone(),
                        },
                        cx,
                    );
                },
            ))
        }
    }

    impl InputHandler for TrackedInputHandler {
        fn selected_text_range(
            &mut self,
            _ignore_disabled_input: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<UTF16Selection> {
            None
        }

        fn marked_text_range(
            &mut self,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<Range<usize>> {
            None
        }

        fn text_for_range(
            &mut self,
            _range: Range<usize>,
            _adjusted_range: &mut Option<Range<usize>>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<String> {
            None
        }

        fn replace_text_in_range(
            &mut self,
            _range: Option<Range<usize>>,
            _text: &str,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            _range: Option<Range<usize>>,
            _text: &str,
            _selected_range: Option<Range<usize>>,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn unmark_text(&mut self, _window: &mut Window, _cx: &mut App) {}

        fn bounds_for_range(
            &mut self,
            _range: Range<usize>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<Bounds<Pixels>> {
            None
        }

        fn character_index_for_point(
            &mut self,
            _point: Point<Pixels>,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Option<usize> {
            None
        }
    }

    /// 关窗标记返回时必须已释放输入处理器；不能等整个 Window 或平台
    /// 原生窗口稍后析构，否则真实 Windows 退出会先把强句柄报成泄漏。
    #[gpui::test]
    fn remove_window_releases_focused_input_handler(cx: &mut TestAppContext) {
        let live_handlers = Rc::new(Cell::new(0));
        let window = cx.add_window(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            InputView {
                focus_handle,
                live_handlers: live_handlers.clone(),
            }
        });
        let weak = window.root(cx).unwrap().downgrade();
        cx.run_until_parked();
        assert!(live_handlers.get() > 0, "测试未注册输入处理器");

        window
            .update(cx, |_, window, _| {
                window.remove_window();
                assert_eq!(
                    live_handlers.get(),
                    0,
                    "remove_window 返回时仍持有输入处理器"
                );
            })
            .unwrap();

        weak.assert_released();
    }
}
