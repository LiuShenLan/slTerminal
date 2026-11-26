//! 终端创建:冻结的启动上下文与启动资源。

use std::cell::RefCell;
use std::rc::Rc;

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
    title_locked: bool,
    cwd_fallback: bool,
    exec_context: Option<crate::completion::PaneExecContext>,
}

/// 已注册到固定 PTY 代次的会话与首帧。
struct BoundSession {
    binding: crate::gpui_terminal::registry::TerminalBinding,
    events: super::super::event_mailbox::EventReceiver,
    frame: slterm_terminal::render::TerminalFrame,
}

/// 已成功启动、但尚未挂成 GPUI 实体的终端。
pub(crate) struct LaunchedTerminal {
    prepared: PreparedTerminal,
    spawned: Box<BoundSession>,
}

/// 恢复启动的显示几何与目录回退授权；普通新建没有此上下文。
pub(crate) struct RestoreLaunchOptions {
    pub(crate) spawn_grid: (u16, u16),
    pub(crate) recovery_root: Option<std::path::PathBuf>,
}

/// 新 PTY 代次回装到既有 View 的全部展示与恢复状态。
pub(crate) struct ReconnectView {
    pub(crate) events: super::super::event_mailbox::EventReceiver,
    pub(crate) frame: slterm_terminal::render::TerminalFrame,
    pub(crate) exec_context: Option<crate::completion::PaneExecContext>,
    pub(crate) cwd_fallback: bool,
}

/// 恢复构造的会话来源；Exited 只恢复状态，不创建 PTY。
enum SessionStartup {
    Spawn(Result<Box<BoundSession>, crate::gpui_terminal::registry::RegistryError>),
    Failed(String),
    Exited,
}

impl TerminalView {
    fn bind_spawned(
        registry: &Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
        terminal_id: slterm_workspace::TerminalId,
        spawned: session::SpawnedSession,
    ) -> Result<Box<BoundSession>, crate::gpui_terminal::registry::RegistryError> {
        let (session, events) = spawned;
        let target = registry
            .borrow_mut()
            .launch(terminal_id, || Ok::<_, std::io::Error>(session))?;
        let binding = crate::gpui_terminal::registry::TerminalBinding::new(registry, target);
        let frame = registry.borrow().capture_frame(target, 0)?;
        Ok(Box::new(BoundSession {
            binding,
            events,
            frame,
        }))
    }

    /// 只执行一次可失败启动；失败时不存在视图、订阅或占位面板。
    pub(crate) fn launch(
        pane_id: PaneId,
        terminal_id: slterm_workspace::TerminalId,
        registry: &Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &Window,
        cx: &App,
    ) -> std::io::Result<LaunchedTerminal> {
        if registry.borrow().target(terminal_id).is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "终端已有存活 worker",
            ));
        }
        // 存活额度与重复身份必须先于真实 PTY 创建判定：拒绝路径不得
        // 解析 shell、创建 ConPTY 或在 GPUI 前台 join 刚 spawn 的 worker。
        registry
            .borrow()
            .preflight_launch(terminal_id)
            .map_err(std::io::Error::other)?;
        let (prepared, spawned) =
            Self::prepare(pane_id, spawn_grid, launch, window, cx, true, None);
        let spawned = spawned.expect("启动构造必须执行 PTY 创建")?;
        let bound =
            Self::bind_spawned(registry, terminal_id, spawned).map_err(std::io::Error::other)?;
        Ok(LaunchedTerminal {
            prepared,
            spawned: bound,
        })
    }

    /// 复用已经成功的启动结果构造实体，不再次 spawn。
    pub(crate) fn from_launched(
        launched: LaunchedTerminal,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_prepared(
            launched.prepared,
            SessionStartup::Spawn(Ok(launched.spawned)),
            window,
            cx,
        )
    }

    /// 恢复 Launch 面板；失败保留在既有面板内，不复用显式新建的空面板拒绝语义。
    pub(crate) fn restore_launch(
        pane_id: PaneId,
        terminal_id: slterm_workspace::TerminalId,
        registry: &Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
        launch: TerminalLaunch,
        options: RestoreLaunchOptions,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        // 额度预检必须先于真实 PTY 创建；拒绝时保留恢复面板与错误，
        // 不解析 shell、不创建 ConPTY。
        let startup = match registry.borrow().preflight_launch(terminal_id) {
            Ok(()) => {
                let (prepared, spawned) = Self::prepare(
                    pane_id,
                    options.spawn_grid,
                    launch,
                    window,
                    cx,
                    true,
                    options.recovery_root,
                );
                let spawned = spawned.expect("恢复 Launch 必须执行一次 PTY 创建");
                let startup = match spawned {
                    Ok(spawned) => match Self::bind_spawned(registry, terminal_id, spawned) {
                        Ok(bound) => SessionStartup::Spawn(Ok(bound)),
                        Err(error) => SessionStartup::Spawn(Err(error)),
                    },
                    Err(error) => SessionStartup::Spawn(Err(
                        crate::gpui_terminal::registry::RegistryError::Launch(error),
                    )),
                };
                return Self::from_prepared(prepared, startup, window, cx);
            }
            Err(error) => SessionStartup::Spawn(Err(error)),
        };
        let (prepared, spawned) =
            Self::prepare(pane_id, options.spawn_grid, launch, window, cx, false, None);
        debug_assert!(spawned.is_none(), "额度拒绝不得创建 PTY");
        Self::from_prepared(prepared, startup, window, cx)
    }

    /// 恢复无法在组装阶段启动的元数据；保留错误，不做有损 Shell 转换。
    pub(crate) fn restore_error(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        error: String,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let (prepared, spawned) =
            Self::prepare(pane_id, spawn_grid, launch, window, cx, false, None);
        debug_assert!(spawned.is_none(), "错误恢复不得创建 PTY");
        Self::from_prepared(prepared, SessionStartup::Failed(error), window, cx)
    }

    /// 恢复 Exited 面板；不创建会话，也不伪造已持久化的输出或退出码。
    pub(crate) fn restore_exited(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let (prepared, spawned) =
            Self::prepare(pane_id, spawn_grid, launch, window, cx, false, None);
        debug_assert!(spawned.is_none(), "Exited 恢复不得创建 PTY");
        Self::from_prepared(prepared, SessionStartup::Exited, window, cx)
    }

    fn prepare(
        pane_id: PaneId,
        spawn_grid: (u16, u16),
        launch: TerminalLaunch,
        window: &Window,
        cx: &App,
        spawn: bool,
        recovery_root: Option<std::path::PathBuf>,
    ) -> (
        PreparedTerminal,
        Option<std::io::Result<session::SpawnedSession>>,
    ) {
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
        let (initial_cwd, initial_title, title_locked, cwd_fallback, spawned, exec_context) =
            match launch {
                TerminalLaunch::Local {
                    cwd,
                    shell,
                    shell_name,
                    explicit_command,
                } => {
                    let mut initial_cwd = cwd
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let mut cwd_fallback = false;
                    // 无设置 shell 键:launch 未指定即 None,core 探测链接手。
                    // 只有恢复上下文允许在真实目录错误后回退项目根。
                    let (spawned, exec_context) = if spawn {
                        match session::local_options(shell, explicit_command, pane_id, cwd) {
                            Ok(options) => match session::spawn_local(
                                initial,
                                term_config,
                                options,
                                recovery_root.as_deref(),
                                None,
                            ) {
                                Ok(local) => {
                                    if let Some(cwd) = local.cwd {
                                        initial_cwd = cwd.to_string_lossy().into_owned();
                                    }
                                    cwd_fallback = local.cwd_fallback;
                                    (Some(Ok(local.spawned)), local.exec_context)
                                }
                                Err(error) => (Some(Err(error)), None),
                            },
                            Err(error) => (Some(Err(error)), None),
                        }
                    } else {
                        (None, None)
                    };
                    let title_locked = shell_name.is_some();
                    let title = shell_name.unwrap_or_else(|| String::from("shell"));
                    (
                        initial_cwd,
                        title,
                        title_locked,
                        cwd_fallback,
                        spawned,
                        exec_context,
                    )
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
                title_locked,
                cwd_fallback,
                exec_context,
            },
            spawned,
        )
    }

    fn from_prepared(
        prepared: PreparedTerminal,
        startup: SessionStartup,
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
            title_locked,
            cwd_fallback,
            exec_context,
        } = prepared;
        let (binding, session_task, frame, error, exited) = match startup {
            SessionStartup::Spawn(Ok(bound)) => {
                let BoundSession {
                    binding,
                    events,
                    frame,
                } = *bound;
                let target = binding.target();
                let task = crate::gpui_terminal::session_pump::attach(target, events, cx);
                (Some(binding), Some(task), Some(Arc::new(frame)), None, None)
            }
            SessionStartup::Spawn(Err(error)) => (
                None,
                None,
                None,
                Some(format!("PTY 启动失败: {error}")),
                None,
            ),
            SessionStartup::Failed(error) => (None, None, None, Some(error), None),
            SessionStartup::Exited => (None, None, None, None, Some("进程已退出".to_owned())),
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

        let shell_line = crate::gpui_terminal::shell_line::ShellLineTracker::new(
            exec_context
                .as_ref()
                .and_then(crate::completion::PaneExecContext::shell_program),
        );
        let mut view = Self {
            pane_id,
            binding,
            _session_task: session_task,
            frame,
            selection_cleared: false,
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
            shell_line,
            completion_session: None,
            suggestion_task: None,
            completion_style,
            ghost_enabled,
            marked_text: None,
            ime_bounds: Bounds::default(),
            title: initial_title,
            title_locked,
            cwd_fallback,
            cwd: initial_cwd,
            branch: String::new(),
            running_program: None,
            error,
            exited,
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
            pending_paste_task: None,
            pending_resume: None,
            terminal_search: None,
            terminal_search_task: None,
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
        if view.binding.is_some()
            && let Err(error) =
                view.send_worker_command(Msg::SetColorScheme(view.palette.is_dark()))
        {
            log::warn!("终端初始配色命令发送失败: {error}");
        }
        view.restart_cursor_blink(cx);
        view
    }
}

impl TerminalView {
    /// 重连成功后切换到新代次；旧订阅随任务替换取消。
    pub(crate) fn install_reconnect(
        &mut self,
        registry: &Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
        target: crate::gpui_terminal::registry::TerminalTarget,
        reconnect: ReconnectView,
        cx: &mut Context<'_, Self>,
    ) {
        let ReconnectView {
            events,
            frame,
            exec_context,
            cwd_fallback,
        } = reconnect;
        self.binding = Some(crate::gpui_terminal::registry::TerminalBinding::new(
            registry, target,
        ));
        self._session_task = Some(crate::gpui_terminal::session_pump::attach(
            target, events, cx,
        ));
        self.frame = Some(Arc::new(frame));
        self.exec_context = exec_context;
        self.cwd_fallback = cwd_fallback;
        self.selection_cleared = false;
        self.suggest = crate::suggest::SuggestState::default();
        self.shell_line = crate::gpui_terminal::shell_line::ShellLineTracker::new(
            self.exec_context
                .as_ref()
                .and_then(crate::completion::PaneExecContext::shell_program),
        );
        self.suggestion_task = None;
        self.terminal_search = None;
        self.terminal_search_task = None;
        self.pending_paste_task = None;
        self.pending_resume = None;
        self.marked_text = None;
        self.running_program = None;
        self.error = None;
        self.exited = None;
        self.pending_resize = None;
        self.resize_epoch = self.resize_epoch.wrapping_add(1);
        self.grid_synced = true;
        self.spawn_at = std::time::Instant::now();
        self.selecting = false;
        self.selection_scroll_active = false;
        self.restart_cursor_blink(cx);
        cx.notify();
    }

    pub(crate) fn reconnect_failed(&mut self, error: String, cx: &mut Context<'_, Self>) {
        self.error = Some(error);
        cx.notify();
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod startup_tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::str::FromStr as _;

    use gpui::{AppContext as _, Context, IntoElement, Render, TestAppContext, Window, div};

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

    #[gpui::test]
    fn custom_title_survives_cli_title_and_reset_signals(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let (_root, window) = cx.add_window_view(|_window, _cx| StartupHarness);
        let terminal = window.update(|window, cx| {
            cx.new(|cx| {
                TerminalView::restore_error(
                    PaneId(1),
                    (80, 24),
                    TerminalLaunch::Local {
                        cwd: None,
                        shell: None,
                        shell_name: Some("我的终端".to_owned()),
                        explicit_command: false,
                    },
                    "测试禁用真实 PTY".to_owned(),
                    window,
                    cx,
                )
            })
        });
        window.update(|_, cx| {
            terminal.update(cx, |view, cx| {
                view.feed_event_for_test(TermEvent::Title("Claude Code".to_owned()), cx);
                view.feed_event_for_test(TermEvent::ResetTitle, cx);
            });
        });
        terminal.read_with(cx, |view, _| assert_eq!(view.title, "我的终端"));
    }

    /// 存活额度拒绝必须先于真实 PTY 创建：额满时错误必须是上限错误，
    /// 而不是"PTY 启动失败"（后者证明工厂已经被执行）。
    #[gpui::test]
    fn launch_capacity_is_checked_before_spawning_a_real_pty(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let mut registry = crate::gpui_terminal::registry::TerminalRegistry::default();
        for index in 0..slterm_terminal::tty::MAX_PTY_SESSIONS {
            let terminal_id = slterm_workspace::TerminalId::from_str(&format!(
                "018f2f7c-8f5a-7c3d-9e2b-{index:012x}"
            ))
            .unwrap();
            registry
                .launch(terminal_id, || {
                    Ok::<_, std::io::Error>(crate::gpui_terminal::session::test_session().0)
                })
                .unwrap();
        }
        let registry = Rc::new(RefCell::new(registry));
        let rejected_id = slterm_workspace::TerminalId::from_str(&format!(
            "018f2f7c-8f5a-7c3d-9e2b-{:012x}",
            slterm_terminal::tty::MAX_PTY_SESSIONS
        ))
        .unwrap();
        let (_root, _window) = cx.add_window_view(|window, cx| {
            let result = TerminalView::launch(
                PaneId(1),
                rejected_id,
                &registry,
                (80, 24),
                missing_shell_launch(),
                window,
                cx,
            );
            let error = result.err().expect("额满时必须拒绝创建");
            assert!(
                error.to_string().contains("上限"),
                "额度拒绝必须先于 PTY 创建；实际错误：{error}"
            );
            StartupHarness
        });
    }

    /// 显式创建失败必须向上返回错误，而不是伪装成可挂载的终端。
    #[gpui::test]
    fn terminal_launch_failure_is_returned(cx: &mut TestAppContext) {
        let parent_env = session::ParentEnvGuard::set(None, Some("slterm-test-parent-colorterm"));
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Settings::default());
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let failed = Rc::new(Cell::new(false));
        let registry = Rc::new(RefCell::new(
            crate::gpui_terminal::registry::TerminalRegistry::default(),
        ));
        let terminal_id =
            slterm_workspace::TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e06").unwrap();
        let observed = Rc::clone(&failed);
        let (_root, _window) = cx.add_window_view(|window, cx| {
            let result = TerminalView::launch(
                PaneId(1),
                terminal_id,
                &registry,
                (80, 24),
                missing_shell_launch(),
                window,
                cx,
            );
            observed.set(result.is_err());
            StartupHarness
        });
        assert!(failed.get(), "启动失败应返回 Err，而非无会话视图");
        parent_env.assert_unchanged();
    }
}
