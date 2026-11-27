//! 恢复运行意图、Agent 注入与重连的端到端接线。

use slterm_agents::AgentSession;

use super::*;

fn restored_agent(source: &str, session_id: &str) -> Option<AgentSession> {
    Some(AgentSession {
        source: source.to_owned(),
        session_id: Some(session_id.to_owned()),
        session_file: None,
    })
}

fn open_real_root_with_persistence(
    cx: &mut TestAppContext,
    persistence: AppRootPersistence,
) -> (Entity<AppRoot>, VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::app_root::keymap::register(cx);
        crate::app_shell::text_view::init(cx);
    });
    let mut slot = None;
    let window = cx.open_window(
        cx.update(|cx| gpui::Bounds::maximized(None, cx).size),
        |window, cx| {
            let root = cx.new(|cx| {
                AppRoot::with_launcher(
                    PanelIdAllocator::new(1),
                    path("C:/Program Files/PowerShell/7/pwsh.exe"),
                    Box::new(LocalTerminalLauncher),
                    persistence,
                    None,
                    AppRootPorts {
                        runtime_events: None,
                        windows: std::sync::Arc::new(
                            crate::platform::windows_integration::FakeWindowsIntegration::default(),
                        ),
                    },
                    cx,
                )
            });
            root.update(cx, |root, cx| {
                root.observe_focus(window, cx);
                root.restore_active_project(window, cx);
            });
            slot = Some(root.clone());
            Root::new(root, window, cx)
        },
    );
    (
        slot.unwrap(),
        VisualTestContext::from_window(window.into(), cx),
    )
}

/// 跨机恢复：RelocationRequired 项目在重新授权前不启动 PTY、不恢复面板。
#[gpui::test]
fn backup_cross_machine_requires_relocation(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_a = temp.path().join("a");
    std::fs::create_dir_all(&project_a).unwrap();
    let mut snapshot = ProjectSnapshot::default();
    snapshot
        .registry_mut()
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(project_a).unwrap(),
        )
        .unwrap();
    snapshot = snapshot
        .with_new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap()
        .0;
    snapshot
        .registry_mut()
        .set_root_status(
            project_id(PROJECT_A),
            slterm_workspace::ProjectRootStatus::RelocationRequired,
        )
        .unwrap();

    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, window, state) = open_root_with_persistence(cx, persistence);
    window.run_until_parked();

    assert_eq!(state.borrow().attempts, 0, "未重新授权前不得启动任何 PTY");
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.panel_count(project_id(PROJECT_A), cx),
            0,
            "未授权项目不得恢复运行面板"
        );
        assert_eq!(
            root.snapshot().terminals().count(),
            1,
            "布局与终端记录本身必须保留"
        );
    });
}

#[gpui::test]
fn terminal_restore_real_pty_auto_and_lazy_launch(cx: &mut TestAppContext) {
    if !cfg!(windows) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let project_a = temp.path().join("a");
    let project_b = temp.path().join("b");
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    let system = std::env::var_os("SystemRoot").expect("Windows 目录必须存在");
    let shell = CanonicalPath::new(
        std::path::Path::new(&system)
            .join("System32")
            .join("cmd.exe"),
    )
    .unwrap();
    let mut snapshot = ProjectSnapshot::default();
    snapshot
        .registry_mut()
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(project_a).unwrap(),
        )
        .unwrap();
    for (project, group, terminal) in [
        (PROJECT_A, GROUP_A, TERMINAL_A),
        (PROJECT_A, GROUP_A, TERMINAL_C),
    ] {
        let spec = NewTerminalSpec {
            shell: shell.clone(),
            ..terminal_spec(project, group, terminal)
        };
        snapshot = snapshot.with_new_terminal(spec).unwrap().0;
    }
    snapshot = snapshot
        .with_terminal_run_intent(terminal_id(TERMINAL_C), RunIntent::Exited)
        .unwrap();
    snapshot
        .registry_mut()
        .add_project(
            project_id(PROJECT_B),
            CanonicalPath::new(project_b).unwrap(),
        )
        .unwrap();
    snapshot = snapshot
        .with_new_terminal(NewTerminalSpec {
            shell,
            ..terminal_spec(PROJECT_B, GROUP_B, TERMINAL_B)
        })
        .unwrap()
        .0;
    snapshot
        .registry_mut()
        .activate_project(project_id(PROJECT_A))
        .unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window) = open_real_root_with_persistence(cx, persistence);

    root.read_with(cx, |root, cx| {
        let visible = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1;
        assert!(visible.read(cx).has_live_worker());
        let exited = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
            .unwrap()
            .1;
        assert!(exited.read(cx).is_exited());
        assert!(
            root.terminals
                .borrow()
                .target(terminal_id(TERMINAL_C))
                .is_none()
        );
        assert!(root.panel_count(project_id(PROJECT_B), cx) == 0);
    });
    activate_project(&root, &mut window, PROJECT_B);
    root.read_with(cx, |root, cx| {
        let hidden = root
            .terminal_panel(project_id(PROJECT_B), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1;
        assert!(hidden.read(cx).has_live_worker());
    });

    for terminal in [TERMINAL_A, TERMINAL_B] {
        let target = root.read_with(cx, |root, _| {
            root.terminals
                .borrow()
                .target(terminal_id(terminal))
                .unwrap()
        });
        let session = root.read_with(cx, |root, _| {
            root.terminals.borrow_mut().remove(target).unwrap()
        });
        session.shutdown().unwrap();
    }
}

#[gpui::test]
fn terminal_restore_real_pty_falls_back_to_project_root(cx: &mut TestAppContext) {
    if !cfg!(windows) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root_path = CanonicalPath::new(temp.path()).unwrap();
    let missing = CanonicalPath::new(temp.path().join("removed-cwd")).unwrap();
    let system = std::env::var_os("SystemRoot").expect("Windows 目录必须存在");
    let shell = CanonicalPath::new(
        std::path::Path::new(&system)
            .join("System32")
            .join("cmd.exe"),
    )
    .unwrap();
    let record = TerminalRecord::new(
        terminal_id(TERMINAL_A),
        project_id(PROJECT_A),
        0,
        missing,
        shell,
        TerminalMetadata::new(None, None),
        RunIntent::Launch,
    )
    .unwrap();
    let (_root, mut window, _state) = open_root(cx);
    let registry = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let terminal = window.update(|window, cx| {
        LocalTerminalLauncher.restore(
            PanelIdAllocator::new(1).allocate().unwrap(),
            &record,
            &registry,
            &root_path,
            window,
            cx,
        )
    });
    terminal.read_with(cx, |view, _| {
        assert!(view.has_live_worker(), "失效 cwd 必须回退并创建真实 PTY");
        assert_eq!(view.cwd, root_path.as_path().to_string_lossy());
        assert!(view.used_cwd_fallback(), "回退必须产生可通知的恢复事实");
    });
    let target = registry.borrow().target(record.id()).unwrap();
    let session = registry.borrow_mut().remove(target).unwrap();
    session.shutdown().unwrap();
}

fn resume_input(receiver: &Receiver<Msg>) -> Option<Vec<u8>> {
    match receiver.try_recv() {
        Ok(Msg::Input(input)) => Some(input.into_owned()),
        Ok(other) => panic!("恢复只应写入一次命令输入，实际: {other:?}"),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => panic!("恢复 worker 不应提前断开"),
    }
}

#[gpui::test]
fn integration_terminal_restore(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut snapshot = terminal_recovery_snapshot();
    snapshot = snapshot
        .with_terminal_agent(
            terminal_id(TERMINAL_A),
            restored_agent("claude", "session-a"),
        )
        .unwrap();
    snapshot = snapshot
        .with_terminal_agent(
            terminal_id(TERMINAL_B),
            restored_agent("unknown-cli", "session-b"),
        )
        .unwrap();
    snapshot = snapshot
        .with_terminal_agent(
            terminal_id(TERMINAL_C),
            restored_agent("codex", "session-c"),
        )
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);

    let (visible_panel, _, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let visible = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    assert!(
        matches!(
            state.borrow().sessions[&visible_panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "Shell 就绪前不得注入恢复命令"
    );
    window.update(|_, cx| {
        visible.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07");
        });
    });
    assert_eq!(
        resume_input(&state.borrow().sessions[&visible_panel.sequence()]),
        Some(b"claude --resume session-a\r".to_vec())
    );
    window.update(|_, cx| {
        visible.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07");
        });
    });
    assert!(
        matches!(
            state.borrow().sessions[&visible_panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "同一 PTY 代次不得重复注入"
    );

    activate_project(&root, &mut window, PROJECT_B);
    let (hidden_panel, _, _) = panel_entity(&root, cx, PROJECT_B, TERMINAL_B);
    let hidden = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_B), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1
    });
    window.update(|_, cx| {
        hidden.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07");
        });
    });
    assert!(
        matches!(
            state.borrow().sessions[&hidden_panel.sequence()].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "AgentCatalog 之外的来源不得构造恢复命令"
    );

    assert_eq!(
        root.read_with(cx, |root, cx| root
            .terminal_agent_status(terminal_id(TERMINAL_B), cx)),
        crate::ai_agents::AgentStatus::Unknown
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(hidden_panel, &TerminalViewEvent::CommandStarted, window, cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root
            .terminal_agent_status(terminal_id(TERMINAL_B), cx)),
        crate::ai_agents::AgentStatus::Unknown,
        "普通命令不得伪造 Agent 工作状态"
    );
    window.update(|_, cx| {
        hidden.update(cx, |view, cx| {
            view.feed_event_for_test(
                TermEvent::Title("SLTERM|D:/work/b|main|claude".to_owned()),
                cx,
            );
            view.feed_event_for_test(TermEvent::Title("Claude Code".to_owned()), cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, cx| {
            root.tab_title(
                &ViewKey::Terminal {
                    project_id: project_id(PROJECT_B),
                    terminal_id: terminal_id(TERMINAL_B),
                },
                cx,
            )
        }),
        "Claude Code"
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(hidden_panel, &TerminalViewEvent::CommandStarted, window, cx);
        });
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root
            .terminal_agent_status(terminal_id(TERMINAL_B), cx)),
        crate::ai_agents::AgentStatus::Working
    );
    assert_eq!(
        root.read_with(cx, |root, _| {
            root.snapshot()
                .terminal(terminal_id(TERMINAL_B))
                .unwrap()
                .agent()
                .map(|agent| agent.source.clone())
        }),
        Some("claude".to_owned())
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.on_terminal_event(
                hidden_panel,
                &TerminalViewEvent::CommandFinished,
                window,
                cx,
            );
        });
    });
    assert_eq!(
        root.read_with(cx, |root, cx| root
            .terminal_agent_status(terminal_id(TERMINAL_B), cx)),
        crate::ai_agents::AgentStatus::Unknown
    );
    assert!(
        root.read_with(cx, |root, _| {
            root.snapshot()
                .terminal(terminal_id(TERMINAL_B))
                .unwrap()
                .agent()
                .is_none()
        }),
        "命令结束后不得保留可误恢复的前台身份"
    );

    activate_project(&root, &mut window, PROJECT_A);
    let (exited_panel, _, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_C);
    let exited = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
            .unwrap()
            .1
    });
    assert!(exited.read_with(cx, |view, _| view.is_exited()));
    assert!(
        !state
            .borrow()
            .sessions
            .contains_key(&exited_panel.sequence())
    );
    let reconnected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::ReconnectTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(
        reconnected.is_ok(),
        "Exited 恢复终端必须可重连: {reconnected:?}"
    );
    window.update(|_, cx| {
        exited.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07");
        });
    });
    assert_eq!(
        resume_input(&state.borrow().sessions[&exited_panel.sequence()]),
        Some(b"codex resume session-c\r".to_vec())
    );
    root.read_with(cx, |root, cx| {
        let expected = snapshot
            .with_terminal_agent(terminal_id(TERMINAL_B), None)
            .unwrap()
            .with_terminal_run_intent(terminal_id(TERMINAL_C), RunIntent::Launch)
            .unwrap();
        assert_eq!(root.snapshot(), &expected);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
    });
}

fn open_restored_agent(
    cx: &mut TestAppContext,
    resume_ai: Option<bool>,
) -> (
    tempfile::TempDir,
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
    Entity<TerminalView>,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let snapshot = terminal_recovery_snapshot()
        .with_terminal_agent(
            terminal_id(TERMINAL_A),
            restored_agent("claude", "session-a"),
        )
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, window, state) =
        open_root_with_persistence_and_size_configured(cx, persistence, None, None, |launcher| {
            launcher.resume_ai = resume_ai
        });
    let terminal = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    (temp, root, window, state, terminal)
}

#[gpui::test]
fn terminal_restore_honors_resume_ai_and_exit_cancellation(cx: &mut TestAppContext) {
    let (_temp, _root, mut window, state, terminal) = open_restored_agent(cx, Some(false));
    window.update(|_, cx| {
        terminal.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07")
        });
    });
    assert!(
        matches!(
            state.borrow().sessions[&1].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "resume_ai=false 必须禁止注入"
    );

    let (_temp, _root, mut window, state, terminal) = open_restored_agent(cx, None);
    window.update(|_, cx| {
        terminal.update(cx, |view, cx| view.mark_test_exited(cx));
        terminal.update(cx, |view, _| {
            view.feed_test_bytes(b"\x1b]133;A\x07\x1b]133;B\x07")
        });
    });
    assert!(
        matches!(
            state.borrow().sessions[&1].try_recv(),
            Err(TryRecvError::Empty)
        ),
        "退出后迟到的提示符不得补写旧恢复命令"
    );
}

#[gpui::test]
fn terminal_restore_timeout_is_bounded_and_single_shot(cx: &mut TestAppContext) {
    let (_temp, _root, window, state, _terminal) = open_restored_agent(cx, None);
    window
        .executor()
        .advance_clock(Duration::from_secs(10) - Duration::from_millis(1));
    window.run_until_parked();
    assert!(matches!(
        state.borrow().sessions[&1].try_recv(),
        Err(TryRecvError::Empty)
    ));
    window.executor().advance_clock(Duration::from_millis(1));
    window.run_until_parked();
    assert_eq!(
        resume_input(&state.borrow().sessions[&1]),
        Some(b"claude --resume session-a\r".to_vec())
    );
    window.executor().advance_clock(Duration::from_secs(20));
    window.run_until_parked();
    assert!(matches!(
        state.borrow().sessions[&1].try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[gpui::test]
fn local_launcher_rejects_non_unicode_shell_without_lossy_conversion(cx: &mut TestAppContext) {
    if !cfg!(windows) {
        return;
    }
    #[cfg(windows)]
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    #[cfg(windows)]
    let shell = {
        let mut wide = std::ffi::OsString::from("C:/slterm-test-non-unicode-shell/")
            .encode_wide()
            .collect::<Vec<_>>();
        wide.push(0xd800);
        wide.extend(std::ffi::OsString::from(".exe").encode_wide());
        CanonicalPath::new(std::ffi::OsString::from_wide(&wide)).unwrap()
    };
    #[cfg(not(windows))]
    let shell = path("C:/unused");

    let record = TerminalRecord::new(
        terminal_id(TERMINAL_A),
        project_id(PROJECT_A),
        0,
        path("D:/work/a"),
        shell,
        TerminalMetadata::new(None, None),
        RunIntent::Launch,
    )
    .unwrap();
    let (_root, mut window, _state) = open_root(cx);
    let registry = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));

    let result = window.update(|window, cx| {
        LocalTerminalLauncher.launch(
            PanelIdAllocator::new(1).allocate().unwrap(),
            &record,
            &registry,
            window,
            cx,
        )
    });

    assert!(
        matches!(result, Err(ref error) if error.kind() == io::ErrorKind::InvalidInput),
        "非 Unicode Shell 必须在 PTY 启动前拒绝，实际: {result:?}"
    );
    let restored = window.update(|window, cx| {
        LocalTerminalLauncher.restore(
            PanelIdAllocator::new(1).allocate().unwrap(),
            &record,
            &registry,
            &path("D:/work/a"),
            window,
            cx,
        )
    });
    restored.read_with(cx, |terminal, _| {
        assert!(terminal.worker_is_retired(), "非法恢复元数据不得启动 PTY");
        assert!(!terminal.is_exited(), "启动失败不得被伪装成 Exited");
    });
}

/// 启动探测：持久根已不存在时真实标记 RelocationRequired，先重定位再恢复。
#[gpui::test]
fn missing_project_root_is_marked_relocation_required_at_startup(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_a = temp.path().join("gone");
    std::fs::create_dir_all(&project_a).unwrap();
    let mut snapshot = ProjectSnapshot::default();
    snapshot
        .registry_mut()
        .add_project(
            project_id(PROJECT_A),
            CanonicalPath::new(project_a.clone()).unwrap(),
        )
        .unwrap();
    snapshot = snapshot
        .with_new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap()
        .0;

    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    std::fs::remove_dir_all(&project_a).unwrap();

    let persistence = AppRootPersistence::load_for_production(&paths, None, None);
    assert_eq!(persistence.domain_recovery_failure(), None);
    let (root, window, state) = open_root_with_persistence(cx, persistence);
    window.run_until_parked();

    assert_eq!(state.borrow().attempts, 0, "根未重定位前不得启动任何 PTY");
    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .root_status(),
            slterm_workspace::ProjectRootStatus::RelocationRequired
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
}
