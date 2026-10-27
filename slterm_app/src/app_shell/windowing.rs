//! 开窗链:窗口选项(116×30 反推尺寸/标题栏/app_id)与启动变体分派。
//!
//! 单窗单实例(产品定位):`open_initial_window` 只在启动链调一次;
//! 窗口角色单形态(QuickTerminal 归 09)。会话恢复决策在开窗前由
//! `session_recovery::prepare_startup` 产出 `StartupPlan`;本模块只把它
//! 应用到窗口定形(回放)并透传给 workspace。

pub(super) mod startup_geometry;

use std::path::PathBuf;
use std::sync::mpsc;

use gpui::{App, AppContext as _, Bounds, Size, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, TitleBar};

use super::SltermShellEvent;
use super::workspace::SltermWorkspace;
use super::workspace::session_recovery::StartupPlan;
use super::workspace::sidebar::{self, SidebarState};
use crate::brand;
use crate::cli::Program;
use crate::platform::startup::first_frame;
use crate::session::WindowState;

/// 首窗的启动形态(新终端 / 新终端并执行命令两变体)。
pub(crate) enum WorkspaceStartup {
    /// 新终端(可带显式 cwd)。
    NewTerminal { cwd: Option<PathBuf> },
    /// 新终端并执行启动命令(`-e` 尾捕获)。
    LaunchTerminal {
        cwd: Option<PathBuf>,
        command: Program,
    },
}

/// 窗口最小尺寸(旧产品面同值)。
const MIN_WINDOW_SIZE: (f32, f32) = (760.0, 540.0);

/// 回放尺寸:保存值优先,过小或无保存值回退启动基准尺寸。
/// 返回 `(尺寸, 最大化态)`。
fn restored_size(
    saved: Option<WindowState>,
    fallback: Size<gpui::Pixels>,
) -> (Size<gpui::Pixels>, bool) {
    let floor = size(px(MIN_WINDOW_SIZE.0), px(MIN_WINDOW_SIZE.1));
    match saved {
        Some(saved) if saved.width > 0 && saved.height > 0 => (
            size(px(saved.width as f32), px(saved.height as f32)).max(&floor),
            saved.maximized,
        ),
        _ => (fallback.max(&floor), false),
    }
}

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

/// 窗口定形唯一函数(会话恢复回放 = 定形前覆盖 preferred_size,
/// 不改调用序)。单形态 = Regular。
pub(crate) fn workspace_window_options(
    cx: &App,
    focus: bool,
    saved: Option<WindowState>,
    sidebar: &SidebarState,
) -> WindowOptions {
    let fallback = startup_geometry::preferred_size(cx, sidebar::chrome_width(sidebar))
        .unwrap_or_else(|| size(px(1080.0), px(720.0)));
    let (preferred, maximized) = restored_size(saved, fallback);
    let bounds = fit_to_display(
        preferred,
        cx.primary_display().map(|display| display.visible_bounds()),
    )
    .unwrap_or_else(|| Bounds::centered(None, preferred, cx));
    WindowOptions {
        window_bounds: Some(if maximized {
            // 最大化窗口的 bounds 是还原尺寸,回放后系统自己铺满显示器。
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }),
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

/// 启动链调一次的开窗入口:命令行 `-e` → LaunchTerminal;有 cwd →
/// NewTerminal;否则消费启动恢复计划(见 `session_recovery`)。
#[allow(clippy::too_many_arguments)] // 启动链参数逐项显式(单一调用点),不折叠中间结构体。
pub(crate) fn open_initial_window(
    cx: &mut App,
    shell_rx: mpsc::Receiver<SltermShellEvent>,
    ai_events: mpsc::Receiver<crate::ai_hook::AiHookEvent>,
    view_state_writer: Option<crate::persistence::workspace_writer::WorkspaceWriterClient>,
    initial_cwd: Option<PathBuf>,
    initial_command: Option<Program>,
    plan: StartupPlan,
    start_hidden: bool,
) {
    let startup = match initial_command {
        Some(command) => WorkspaceStartup::LaunchTerminal {
            cwd: initial_cwd,
            command,
        },
        None => WorkspaceStartup::NewTerminal { cwd: initial_cwd },
    };
    let mut options = workspace_window_options(cx, !start_hidden, plan.window, &plan.sidebar);
    apply_start_hidden(&mut options, start_hidden);
    // 首帧黑窗/半构窗口不曝光:先同步画一帧再 activate(first_frame 因果链)。
    // 静默窗口不曝光,defer_show 自然返回 false,不进 present-then-show。
    let deferred_show = first_frame::defer_show(&mut options);
    let result = cx.open_window(options, move |window, cx| {
        window.set_window_title(brand::NAME);
        // HWND 发布单点:任务栏闪烁/进度经 facade 查询。
        crate::windowing::focus::publish_window(window);
        let workspace = cx.new(|cx| {
            SltermWorkspace::new(
                window,
                shell_rx,
                Some(ai_events),
                view_state_writer,
                startup,
                plan,
                cx,
            )
        });
        cx.new(|cx| Root::new(workspace, window, cx))
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

    /// 保存的尺寸/最大化态在窗口定形前决定 bounds 形态。
    #[test]
    fn saved_window_state_replays_size_and_maximized_flag() {
        let fallback = size(px(1080.0), px(720.0));
        let (preferred, maximized) = restored_size(
            Some(WindowState {
                width: 1280,
                height: 720,
                maximized: true,
            }),
            fallback,
        );
        assert_eq!(preferred, size(px(1280.0), px(720.0)));
        assert!(maximized, "回放最大化态");
        let display = Bounds::new(gpui::point(px(0.0), px(0.0)), size(px(1920.0), px(1080.0)));
        let bounds = fit_to_display(preferred, Some(display)).unwrap();
        assert_eq!(bounds.size, preferred);
        assert_eq!(bounds.origin.x, px(320.0), "在显示器内居中");
        assert_eq!(bounds.origin.y, px(180.0));

        let (preferred, maximized) = restored_size(None, fallback);
        assert_eq!(preferred, fallback);
        assert!(!maximized);
    }

    /// 保存值过小钳到最小尺寸;大于显示器则收进可见区。
    #[test]
    fn replayed_window_state_clamps_to_floor_and_display() {
        let fallback = size(px(1080.0), px(720.0));
        let (tiny, _) = restored_size(
            Some(WindowState {
                width: 120,
                height: 90,
                maximized: false,
            }),
            fallback,
        );
        assert_eq!(tiny, size(px(760.0), px(540.0)));

        let display = Bounds::new(gpui::point(px(0.0), px(0.0)), size(px(1000.0), px(600.0)));
        let bounds = fit_to_display(size(px(4000.0), px(3000.0)), Some(display)).unwrap();
        assert_eq!(bounds.size, display.size, "超屏尺寸收到显示器内");
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

    /// 启动命令必须原样保留程序与参数(逐字,不经 shell 二次解析)。
    #[test]
    fn explicit_program_is_kept_with_its_arguments_and_directory() {
        let command = Program::WithArgs {
            program: "shell.exe".into(),
            args: vec!["--literal=two words".into()],
        };
        let startup = match Some(command) {
            Some(command) => WorkspaceStartup::LaunchTerminal {
                cwd: Some(PathBuf::from("C:/work area")),
                command,
            },
            None => unreachable!(),
        };
        let WorkspaceStartup::LaunchTerminal { cwd, command } = startup else {
            panic!("explicit launch was discarded");
        };
        assert_eq!(cwd, Some(PathBuf::from("C:/work area")));
        assert_eq!(command.program(), "shell.exe");
        assert_eq!(command.args(), &["--literal=two words".to_owned()]);
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
