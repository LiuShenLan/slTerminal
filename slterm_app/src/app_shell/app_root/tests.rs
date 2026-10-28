//! AppRoot 的虚拟窗口所有权测试；所有终端均使用无真实 PTY 的会话。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, TryRecvError};

use gpui::{
    App, AppContext as _, Context, Entity, EntityId, IntoElement, Render, TestAppContext,
    VisualTestContext, Window, div,
};
use gpui_component::Root;
use slterm_path::CanonicalPath;
use slterm_split::SplitDirection;
use slterm_terminal::event::Event as TermEvent;
use slterm_terminal::event_loop::Msg;
use slterm_workspace::{
    AddProjectOutcome, LogicalSize, NewTerminalSpec, PanelGroupId, PanelId, PanelIdAllocator,
    ProjectId, RunIntent, SplitTerminalSpec, TerminalId, TerminalRecord, ViewKey,
};

use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::session;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};

use super::commands::{AppRootError, LocalTerminalLauncher};
use super::{AppRoot, TerminalPanelLauncher};

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const PROJECT_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";

fn project_id(text: &str) -> ProjectId {
    ProjectId::from_str(text).unwrap()
}

fn terminal_id(text: &str) -> TerminalId {
    TerminalId::from_str(text).unwrap()
}

fn group_id(text: &str) -> PanelGroupId {
    PanelGroupId::from_str(text).unwrap()
}

fn path(text: &str) -> CanonicalPath {
    CanonicalPath::new(text).unwrap()
}

fn terminal_spec(project: &str, group: &str, terminal: &str) -> NewTerminalSpec {
    NewTerminalSpec {
        project_id: project_id(project),
        terminal_id: terminal_id(terminal),
        target_group: group_id(group),
        shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
        custom_title: None,
    }
}

#[derive(Default)]
struct LauncherState {
    attempts: usize,
    fail_next: bool,
    sessions: BTreeMap<u64, Receiver<Msg>>,
}

struct FakeLauncher {
    state: Rc<RefCell<LauncherState>>,
}

impl TerminalPanelLauncher for FakeLauncher {
    fn launch(
        &mut self,
        panel_id: PanelId,
        _record: &slterm_workspace::TerminalRecord,
        window: &mut Window,
        cx: &mut App,
    ) -> io::Result<Entity<TerminalView>> {
        self.state.borrow_mut().attempts += 1;
        if self.state.borrow_mut().fail_next {
            self.state.borrow_mut().fail_next = false;
            return Err(io::Error::new(io::ErrorKind::NotFound, "测试启动失败"));
        }
        let terminal = cx.new(|cx| {
            TerminalView::new(
                PaneId(panel_id.sequence()),
                (80, 24),
                TerminalLaunch::Local {
                    cwd: None,
                    shell: Some(slterm_terminal::tty::Shell::new(
                        "slterm-test-missing-shell-executable".into(),
                        Vec::new(),
                    )),
                    shell_name: None,
                    explicit_command: false,
                },
                window,
                cx,
            )
        });
        let (test_session, receiver) = session::test_session();
        terminal.update(cx, |view, _| view.session = Some(test_session));
        self.state
            .borrow_mut()
            .sessions
            .insert(panel_id.sequence(), receiver);
        Ok(terminal)
    }
}

struct Harness {
    _root: Entity<AppRoot>,
}

impl Render for Harness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
    }
}

fn open_root(
    cx: &mut TestAppContext,
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let state = Rc::new(RefCell::new(LauncherState::default()));
    let launcher = FakeLauncher {
        state: Rc::clone(&state),
    };
    let mut slot = None;
    let (_window_root, window) = cx.add_window_view(|window, cx| {
        let root = cx.new(|_| AppRoot::with_launcher(PanelIdAllocator::new(1), Box::new(launcher)));
        slot = Some(root.clone());
        let harness = cx.new(|_| Harness { _root: root });
        Root::new(harness, window, cx)
    });
    (slot.unwrap(), window.clone(), state)
}

fn add_project(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    id: &str,
    root_path: &str,
) -> AddProjectOutcome {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.add_project(project_id(id), path(root_path), window, cx)
        })
    });
    assert!(result.is_ok(), "添加项目失败: {result:?}");
    result.unwrap()
}

fn activate_project(root: &Entity<AppRoot>, window: &mut VisualTestContext, id: &str) {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(id), window, cx)
        })
    });
    assert!(result.is_ok(), "切换项目失败: {result:?}");
}

fn new_terminal(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    project: &str,
    group: &str,
    terminal: &str,
) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal(terminal_spec(project, group, terminal), window, cx)
        })
    });
    assert!(result.is_ok(), "新建终端失败: {result:?}");
    result.unwrap()
}

fn split_terminal(root: &Entity<AppRoot>, window: &mut VisualTestContext) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.split_terminal(
                SplitTerminalSpec {
                    new_terminal: terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
                    new_group_id: group_id(GROUP_B),
                    direction: SplitDirection::LeftRight,
                    ratio: 0.5,
                    viewport: LogicalSize::new(1200.0, 800.0),
                },
                window,
                cx,
            )
        })
    });
    assert!(result.is_ok(), "拆分终端失败: {result:?}");
    result.unwrap()
}

fn panel_entity(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: &str,
    terminal: &str,
) -> (PanelId, EntityId, gpui::WeakEntity<TerminalView>) {
    root.read_with(cx, |root, cx| {
        let (panel, entity) = root
            .terminal_panel(project_id(project), terminal_id(terminal), cx)
            .unwrap();
        (panel, entity.entity_id(), entity.downgrade())
    })
}

fn output_visible(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    project: &str,
    terminal: &str,
) -> bool {
    root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(project), terminal_id(terminal), cx)
            .unwrap()
            .1
            .read(cx)
            .output_visible()
    })
}

#[gpui::test]
fn adding_project_does_not_create_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let outcome = add_project(&root, &mut window, PROJECT_A, "D:/work/a");

    assert_eq!(outcome, AddProjectOutcome::Added(project_id(PROJECT_A)));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot().registry().projects().len(), 1);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.snapshot().terminals().count(), 0);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn failed_launch_keeps_snapshot_and_instances_unchanged(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    state.borrow_mut().fail_next = true;

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A), window, cx)
        })
    });

    assert!(matches!(result, Err(AppRootError::Launch(_))));
    assert_eq!(state.borrow().attempts, 1);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &before);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
}

#[gpui::test]
fn project_switch_preserves_entities_and_hidden_event_processing(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, terminal_a, weak_a) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);

    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    let (_, terminal_b, _) = panel_entity(&root, cx, PROJECT_B, TERMINAL_B);
    assert_ne!(terminal_a, terminal_b);
    assert!(!output_visible(&root, cx, PROJECT_A, TERMINAL_A));
    assert!(output_visible(&root, cx, PROJECT_B, TERMINAL_B));

    // 隐藏只门控重绘；标题事件仍按原路径更新实体。
    window.update(|_, cx| {
        let terminal = root.read_with(cx, |root, cx| {
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
        });
        terminal.update(cx, |terminal, cx| {
            terminal.feed_event_for_test(TermEvent::Title("hidden".into()), cx);
        });
    });
    root.read_with(cx, |root, cx| {
        let terminal = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1;
        assert_eq!(terminal.read(cx).title, "hidden");
    });

    activate_project(&root, &mut window, PROJECT_A);
    assert_eq!(panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1, terminal_a);
    assert_eq!(panel_entity(&root, cx, PROJECT_B, TERMINAL_B).1, terminal_b);
    assert!(weak_a.upgrade().is_some());
    assert!(output_visible(&root, cx, PROJECT_A, TERMINAL_A));
    assert!(!output_visible(&root, cx, PROJECT_B, TERMINAL_B));
    assert_eq!(state.borrow().attempts, 2);
    for receiver in state.borrow().sessions.values() {
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    }
}

#[gpui::test]
fn split_and_activate_reuse_existing_terminal_entities(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, first_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    let second_panel = split_terminal(&root, &mut window);
    assert_ne!(first_panel, second_panel);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(result.is_ok(), "激活终端失败: {result:?}");

    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(
            root.focused_panel(project_id(PROJECT_A), cx),
            Some(first_panel)
        );
        assert_eq!(
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
                .entity_id(),
            first_entity
        );
        let active = root
            .snapshot()
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .group(group_id(GROUP_A))
            .unwrap()
            .active();
        assert_eq!(
            active,
            &ViewKey::Terminal {
                project_id: project_id(PROJECT_A),
                terminal_id: terminal_id(TERMINAL_A)
            }
        );
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn close_terminal_releases_instance_and_keeps_empty_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, _, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot().terminals().count(), 0);
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .is_some()
        );
        assert!(
            root.snapshot()
                .registry()
                .project(project_id(PROJECT_A))
                .unwrap()
                .layout()
                .view_keys()
                .is_empty()
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0);
    });
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert!(weak.upgrade().is_none());
}

#[gpui::test]
fn exited_terminal_retains_panel_and_marks_run_intent(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, _, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);

    window.update(|_, cx| {
        let terminal = root.read_with(cx, |root, cx| {
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
                .unwrap()
                .1
        });
        terminal.update(cx, |_, cx| cx.emit(TerminalViewEvent::Exited));
    });
    window.run_until_parked();

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot()
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .run_intent(),
            RunIntent::Exited
        );
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1);
    });
    assert!(weak.upgrade().is_some());
    assert!(matches!(
        state.borrow().sessions[&panel.sequence()].try_recv(),
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
        None,
        RunIntent::Launch,
    )
    .unwrap();
    let (_root, mut window, _state) = open_root(cx);

    let result = window.update(|window, cx| {
        LocalTerminalLauncher.launch(
            PanelIdAllocator::new(1).allocate().unwrap(),
            &record,
            window,
            cx,
        )
    });

    assert!(
        matches!(result, Err(ref error) if error.kind() == io::ErrorKind::InvalidInput),
        "非 Unicode Shell 必须在 PTY 启动前拒绝，实际: {result:?}"
    );
}

#[gpui::test]
fn duplicate_project_root_reactivates_existing_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");

    let outcome = add_project(&root, &mut window, PROJECT_C, "D:/work/a");

    assert_eq!(outcome, AddProjectOutcome::Existing(project_id(PROJECT_A)));
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().projects().len(), 2);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
    });
    assert_eq!(panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1, entity);
    assert_eq!(state.borrow().attempts, 1);
}
