//! 终端创建:冻结的启动上下文与启动资源。

use super::*;

/// 已冻结的视图配置与几何；PTY 启动结果单独传递。
struct PreparedTerminal {
    pane_id: PaneId,
    families: [String; 4],
    font_size: Pixels,
    palette: Arc<Palette>,
    copy_on_select: bool,
    ligatures: bool,
    completion_style: crate::suggest::CompletionStyle,
    ghost_enabled: bool,
    default_cursor_style: CursorStyle,
    cell_w: Pixels,
    line_h: Pixels,
    initial: WindowSize,
    initial_cwd: String,
    initial_title: String,
    exec_context: Option<crate::completion::PaneExecContext>,
}

/// 已成功启动、但尚未挂成 GPUI 实体的终端。
#[allow(dead_code)] // 独立 AppRoot 的生产窗口装配链。
pub(crate) struct LaunchedTerminal {
    prepared: PreparedTerminal,
    spawned: session::SpawnedSession,
}

impl TerminalView {
    /// 只执行一次可失败启动；失败时不存在视图、订阅或占位面板。
    #[allow(dead_code)] // 独立 AppRoot 的生产窗口装配链。
    pub(crate) fn launch(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &Window,
        cx: &App,
    ) -> std::io::Result<LaunchedTerminal> {
        let (prepared, spawned) = Self::prepare(pane_id, spawn_grid, launch, window, cx);
        Ok(LaunchedTerminal {
            prepared,
            spawned: spawned?,
        })
    }

    /// 复用已经成功的启动结果构造实体，不再次 spawn。
    #[allow(dead_code)] // 独立 AppRoot 的生产窗口装配链。
    pub(crate) fn from_launched(
        launched: LaunchedTerminal,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_prepared(launched.prepared, Ok(launched.spawned), window, cx)
    }

    /// `spawn_grid`:PTY 出生网格(宿主已把窗口定形到该几何)。首帧布局
    /// 与之相同则零下发;见 `set_layout` 的启动稳定闸。
    pub fn new(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let (prepared, spawned) = Self::prepare(pane_id, spawn_grid, launch, window, cx);
        Self::from_prepared(prepared, spawned, window, cx)
    }

    fn prepare(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &Window,
        cx: &App,
    ) -> (PreparedTerminal, std::io::Result<session::SpawnedSession>) {
        // 字体、调色板与终端启动配置来自全局 Settings(bootstrap 装载)。
        // Settings 白名单外项取模块常量(设置键回填点):
        // 字宽模式恒 Compact、偏移恒 (0,0)、行高倍率恒 None、shell 恒 None
        // (走 core pwsh→powershell→cmd 探测链)。
        let (
            families,
            font_size,
            palette,
            term_config,
            copy_on_select,
            ligatures,
            completion_style,
            ghost_enabled,
        ) = match cx.try_global::<Settings>() {
            Some(settings) => (
                [
                    settings.font_family.clone(),
                    settings.font_bold_family.clone(),
                    settings.font_italic_family.clone(),
                    settings.font_bold_italic_family.clone(),
                ],
                px(settings.font_size_px),
                Arc::new(settings.palette.clone()),
                settings.term_config(),
                settings.copy_on_select,
                settings.ligatures,
                settings.completion_style,
                settings.ghost,
            ),
            None => (
                std::array::from_fn(|_| crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned()),
                px(15.0),
                Arc::new(Palette::default()),
                slterm_terminal::term::Config::default(),
                false,
                true,
                slterm_settings::CompletionStyleName::default(),
                true,
            ),
        };
        let default_cursor_style = term_config.default_cursor_style;
        let (cell_w, line_h) = Self::cell_metrics(window, cx);
        // 像素口径与 viewport 上报一致(设备 px),避免首帧一次像素级差异。
        let scale = window.scale_factor();
        let initial = WindowSize {
            num_lines: spawn_grid.1.max(2),
            num_cols: spawn_grid.0.max(2),
            cell_width: (cell_w.as_f32() * scale).round().max(1.0) as u16,
            cell_height: (line_h.as_f32() * scale).round().max(1.0) as u16,
        };
        let (initial_cwd, initial_title, spawned, exec_context) = match launch {
            TerminalLaunch::Local {
                cwd,
                shell,
                shell_name,
                explicit_command,
            } => {
                let initial_cwd = cwd
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // 无设置 shell 键:launch 未指定即 None,core 探测链接手。
                // 显式命令(-e)的解析失败与 spawn 失败同通道呈现。
                let options = session::local_options(shell, explicit_command, pane_id, cwd);
                // 后台补全复用 PTY 创建时冻结的 shell 与环境覆盖;必须在
                // options 被 spawn 消费前留一份只读快照。
                let exec_context = options
                    .as_ref()
                    .ok()
                    .map(crate::completion::PaneExecContext::from_pty_options);
                let spawned =
                    options.and_then(|options| session::spawn(initial, term_config, options));
                let title = shell_name.unwrap_or_else(|| String::from("shell"));
                (initial_cwd, title, spawned, exec_context)
            }
        };
        (
            PreparedTerminal {
                pane_id,
                families,
                font_size,
                palette,
                copy_on_select,
                ligatures,
                completion_style,
                ghost_enabled,
                default_cursor_style,
                cell_w,
                line_h,
                initial,
                initial_cwd,
                initial_title,
                exec_context,
            },
            spawned,
        )
    }

    fn from_prepared(
        prepared: PreparedTerminal,
        spawned: std::io::Result<session::SpawnedSession>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let PreparedTerminal {
            pane_id,
            families,
            font_size,
            palette,
            copy_on_select,
            ligatures,
            completion_style,
            ghost_enabled,
            default_cursor_style,
            cell_w,
            line_h,
            initial,
            initial_cwd,
            initial_title,
            exec_context,
        } = prepared;
        let (session, error) = match spawned {
            Ok((session, rx)) => {
                crate::gpui_terminal::session_pump::attach(rx, cx);
                (Some(session), None)
            }
            Err(error) => (None, Some(format!("PTY 启动失败: {error}"))),
        };

        let focus_handle = cx.focus_handle();
        let cursor_window_active = window.is_window_active();
        let cursor_pane_focused = focus_handle.is_focused(window);
        let cursor_blink_subscriptions = [
            cx.observe_window_activation(window, |view, window, cx| {
                let active = window.is_window_active();
                if view.cursor_window_active != active {
                    view.cursor_window_active = active;
                    view.restart_cursor_blink(cx);
                }
            }),
            cx.on_focus_in(&focus_handle, window, |view, _window, cx| {
                if !view.cursor_pane_focused {
                    view.cursor_pane_focused = true;
                    view.restart_cursor_blink(cx);
                }
            }),
            cx.on_focus_out(&focus_handle, window, |view, _event, _window, cx| {
                if view.cursor_pane_focused {
                    view.cursor_pane_focused = false;
                    view.restart_cursor_blink(cx);
                }
            }),
        ];

        let mut view = Self {
            pane_id,
            session,
            exec_context,
            focus_handle,
            font: mono_font(
                &families[0],
                FontWeight::NORMAL,
                FontStyle::Normal,
                ligatures,
            ),
            font_bold: mono_font(&families[1], FontWeight::BOLD, FontStyle::Normal, ligatures),
            font_italic: mono_font(
                &families[2],
                FontWeight::NORMAL,
                FontStyle::Italic,
                ligatures,
            ),
            font_bold_italic: mono_font(
                &families[3],
                FontWeight::BOLD,
                FontStyle::Italic,
                ligatures,
            ),
            font_size,
            ligatures,
            font_offset_x: FONT_OFFSET.0,
            font_offset_y: FONT_OFFSET.1,
            line_height_multiplier: None,
            palette,
            suggest: crate::suggest::SuggestState::default(),
            suggest_line: String::new(),
            suggest_cursor: 0,
            suggest_line_trusted: true,
            completion_session: None,
            suggestion_task: None,
            completion_style,
            ghost_enabled,
            last_committed: String::new(),
            marked_text: None,
            ime_bounds: Bounds::default(),
            title: initial_title,
            cwd: initial_cwd,
            branch: String::new(),
            running_program: None,
            error,
            exited: None,
            pending_agent_command: None,
            origin: point(px(0.0), px(0.0)),
            cell_width: cell_w,
            line_height: line_h,
            cols: initial.num_cols as usize,
            rows: initial.num_lines as usize,
            window_size: initial,
            grid_synced: false,
            spawn_at: std::time::Instant::now(),
            viewports: ViewportTracker::default(),
            pending_resize: None,
            structural_resize: false,
            resize_epoch: 0,
            scroll_px: 0.0,
            selecting: false,
            selection_scroll_epoch: 0,
            selection_scroll_active: false,
            copy_on_select,
            copy_feedback: CopyFeedback::new(),
            last_report_point: None,
            cursor_visible: true,
            cursor_blink_epoch: 0,
            cursor_window_active,
            cursor_pane_focused,
            output_visible: true,
            _cursor_blink_subscriptions: cursor_blink_subscriptions,
            default_cursor_style,
        };
        // 出生即把亮暗种进 Term:`Term::color_scheme_dark` 的默认值是「暗」,
        // 若不种,第一次真实翻转会被 `set_color_scheme` 的同值去重吞掉,
        // 订阅方漏收第一条通知。
        if let Some(session) = &view.session {
            session.term.lock().set_color_scheme(view.palette.is_dark());
        }
        view.restart_cursor_blink(cx);
        view
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod startup_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};

    use super::*;

    struct StartupHarness;

    impl Render for StartupHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<'_, Self>,
        ) -> impl IntoElement {
            div()
        }
    }

    fn missing_shell_launch() -> TerminalLaunch {
        TerminalLaunch::Local {
            cwd: None,
            shell: Some(slterm_terminal::tty::Shell::new(
                "slterm-test-missing-shell-executable".into(),
                Vec::new(),
            )),
            shell_name: None,
            explicit_command: false,
        }
    }

    /// 显式创建失败必须向上返回错误，而不是伪装成可挂载的终端。
    #[gpui::test]
    fn terminal_launch_failure_is_returned(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let failed = Rc::new(Cell::new(false));
        let observed = Rc::clone(&failed);
        let (_root, _window) = cx.add_window_view(|window, cx| {
            let result =
                TerminalView::launch(PaneId(1), (80, 24), missing_shell_launch(), window, cx);
            observed.set(result.is_err());
            StartupHarness
        });
        assert!(failed.get(), "启动失败应返回 Err，而非无会话视图");
    }
}
