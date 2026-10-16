//! 终端创建:冻结的启动上下文与启动资源。

use super::*;

impl TerminalView {
    /// `spawn_grid`:PTY 出生网格(宿主已把窗口定形到该几何)。首帧布局
    /// 与之相同则零下发;见 `set_layout` 的启动稳定闸。
    pub fn new(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 字体、调色板与终端启动配置来自全局 Settings(bootstrap 装载)。
        // M3 Settings 白名单外项取模块常量(06 篇设置键回填点):
        // 字宽模式恒 Compact、偏移恒 (0,0)、行高倍率恒 None、shell 恒 None
        // (走 core pwsh→powershell→cmd 探测链)。
        let (families, font_size, palette, term_config, copy_on_select, ligatures) = match cx
            .try_global::<Settings>(
        ) {
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
            ),
            None => (
                std::array::from_fn(|_| crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned()),
                px(15.0),
                Arc::new(Palette::default()),
                slterm_terminal::term::Config::default(),
                false,
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
        let (initial_cwd, initial_title, spawned) = match launch {
            TerminalLaunch::Local {
                cwd,
                shell,
                shell_name,
            } => {
                let initial_cwd = cwd
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // M3 无设置 shell 键:launch 未指定即 None,core 探测链接手。
                let options = session::local_options(shell, pane_id, cwd);
                let spawned = session::spawn(initial, term_config, options);
                let title = shell_name.unwrap_or_else(|| String::from("shell"));
                (initial_cwd, title, spawned)
            }
        };
        let (session, error) = match spawned {
            Ok((session, rx)) => {
                crate::gpui_terminal::session_pump::attach(rx, cx);
                (Some(session), None)
            }
            Err(err) => (None, Some(format!("PTY 启动失败: {err}"))),
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
            font_offset_x: M3_FONT_OFFSET.0,
            font_offset_y: M3_FONT_OFFSET.1,
            line_height_multiplier: None,
            palette,
            marked_text: None,
            ime_bounds: Bounds::default(),
            title: initial_title,
            cwd: initial_cwd,
            error,
            exited: None,
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
