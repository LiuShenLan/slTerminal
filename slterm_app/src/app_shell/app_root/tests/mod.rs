//! AppRoot 的虚拟窗口所有权测试；所有终端均使用无真实 PTY 的会话。

mod sidebar;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gpui::{
    App, AppContext as _, Entity, EntityId, Modifiers, MouseButton, StyleRefinement, Styled as _,
    TestAppContext, VisualTestContext, Window,
};
use gpui_component::Root;
use slterm_path::{AppPaths, CanonicalPath};
use slterm_split::{SplitDirection, SplitTree};
use slterm_terminal::event::Event as TermEvent;
use slterm_terminal::event_loop::Msg;
use slterm_workspace::{
    AddProjectOutcome, LogicalSize, NewTerminalSpec, PanelGroupId, PanelId, PanelIdAllocator,
    ProjectId, ProjectSnapshot, RunIntent, SideViewId, SplitTerminalSpec, TerminalId,
    TerminalRecord, ViewKey,
};

use crate::app_shell::SltermShellEvent;
use crate::gpui_terminal::PaneId;
use crate::gpui_terminal::session;
use crate::gpui_terminal::view::{TerminalLaunch, TerminalView, TerminalViewEvent};
use crate::persistence::workspace::WorkspaceDatabase;
use crate::persistence::workspace_writer::WorkspaceWriter;

use super::command_registry::{
    CommandId, CommandOutcome, CommandParameters, CommandRegistry, CommandSource, CommandTarget,
};
use super::commands::{AppRootError, LocalTerminalLauncher};
use super::focus_manager::FocusError;
use super::side_view::AppRootPersistence;
use super::{AppRoot, TerminalPanelLauncher};

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const PROJECT_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
const GROUP_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2c";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";
const TERMINAL_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2c";

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

    fn restore(
        &mut self,
        panel_id: PanelId,
        record: &slterm_workspace::TerminalRecord,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TerminalView> {
        let launch = TerminalLaunch::Local {
            cwd: None,
            shell: Some(slterm_terminal::tty::Shell::new(
                "slterm-test-missing-shell-executable".into(),
                Vec::new(),
            )),
            shell_name: None,
            explicit_command: false,
        };
        let pane_id = PaneId(panel_id.sequence());
        if record.run_intent() == RunIntent::Exited {
            return cx
                .new(|cx| TerminalView::restore_exited(pane_id, (80, 24), launch, window, cx));
        }

        self.state.borrow_mut().attempts += 1;
        let fail = std::mem::replace(&mut self.state.borrow_mut().fail_next, false);
        let terminal =
            cx.new(|cx| TerminalView::restore_launch(pane_id, (80, 24), launch, window, cx));
        if !fail {
            let (test_session, receiver) = session::test_session();
            terminal.update(cx, |view, _| view.install_test_session(test_session));
            self.state
                .borrow_mut()
                .sessions
                .insert(panel_id.sequence(), receiver);
        }
        terminal
    }
}

fn open_root(
    cx: &mut TestAppContext,
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
) {
    open_root_with_persistence(cx, AppRootPersistence::empty(None))
}

fn open_root_with_persistence(
    cx: &mut TestAppContext,
    persistence: AppRootPersistence,
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
) {
    open_root_with_persistence_and_size(cx, persistence, None)
}

fn open_root_with_persistence_and_size(
    cx: &mut TestAppContext,
    persistence: AppRootPersistence,
    window_size: Option<gpui::Size<gpui::Pixels>>,
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
) {
    open_root_with_persistence_and_size_configured(cx, persistence, window_size, |_| {})
}

fn open_root_with_persistence_and_size_configured(
    cx: &mut TestAppContext,
    persistence: AppRootPersistence,
    window_size: Option<gpui::Size<gpui::Pixels>>,
    configure: impl FnOnce(&mut LauncherState),
) -> (
    Entity<AppRoot>,
    VisualTestContext,
    Rc<RefCell<LauncherState>>,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        super::keymap::register(cx);
    });
    let state = Rc::new(RefCell::new(LauncherState::default()));
    configure(&mut state.borrow_mut());
    let launcher = FakeLauncher {
        state: Rc::clone(&state),
    };
    let mut slot = None;
    let window_size =
        window_size.unwrap_or_else(|| cx.update(|cx| gpui::Bounds::maximized(None, cx).size));
    let window = cx.open_window(window_size, |window, cx| {
        let root = cx.new(|cx| {
            AppRoot::with_launcher(
                PanelIdAllocator::new(1),
                path("C:/Program Files/PowerShell/7/pwsh.exe"),
                Box::new(launcher),
                persistence,
                cx,
            )
        });
        root.update(cx, |root, cx| {
            root.observe_focus(window, cx);
            root.restore_active_project(window, cx);
        });
        slot = Some(root.clone());
        Root::new(root, window, cx)
    });
    let window = VisualTestContext::from_window(window.into(), cx);
    (slot.unwrap(), window, state)
}

fn draw_frame(window: &mut VisualTestContext) {
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

fn open_side_menu_for_test(window: &mut VisualTestContext) {
    draw_frame(window);
    let bounds = window
        .debug_bounds("app-root-side-button-explorer")
        .unwrap();
    window.simulate_mouse_down(bounds.center(), MouseButton::Right, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Right, Modifiers::default());
    draw_frame(window);
    assert!(window.debug_bounds("app-root-side-button-menu").is_some());
}

fn group_selector(id: &str) -> &'static str {
    Box::leak(format!("app-root-group-{id}").into_boxed_str())
}

fn terminal_selector(id: &str) -> &'static str {
    Box::leak(format!("app-root-terminal-{id}").into_boxed_str())
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

fn open_generated_project(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    root_path: &str,
) -> ProjectId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_project(path(root_path), window, cx)
        })
    });
    assert!(result.is_ok(), "打开项目失败: {result:?}");
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
            root.new_terminal_with_identity(terminal_spec(project, group, terminal), window, cx)
        })
    });
    assert!(result.is_ok(), "新建终端失败: {result:?}");
    result.unwrap()
}

fn new_generated_terminal(root: &Entity<AppRoot>, window: &mut VisualTestContext) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal(path("C:/Program Files/PowerShell/7/pwsh.exe"), window, cx)
        })
    });
    assert!(result.is_ok(), "生产新建终端失败: {result:?}");
    result.unwrap()
}

fn split_generated_terminal(root: &Entity<AppRoot>, window: &mut VisualTestContext) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::SplitDown,
                CommandSource::Application,
                CommandTarget::Focused,
                Some(CommandParameters::Split {
                    ratio: 0.5,
                    viewport: LogicalSize::new(1200.0, 800.0),
                    shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
                }),
                window,
                cx,
            )
        })
    });
    assert!(result.is_ok(), "生产拆分终端失败: {result:?}");
    let CommandOutcome::TerminalPanel(panel_id) = result.unwrap() else {
        unreachable!("拆分命令必须返回面板身份");
    };
    panel_id
}

fn split_terminal(root: &Entity<AppRoot>, window: &mut VisualTestContext) -> PanelId {
    split_terminal_in_direction(root, window, SplitDirection::LeftRight)
}

fn split_terminal_in_direction(
    root: &Entity<AppRoot>,
    window: &mut VisualTestContext,
    direction: SplitDirection,
) -> PanelId {
    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.split_terminal_with_identity(
                SplitTerminalSpec {
                    new_terminal: terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B),
                    new_group_id: group_id(GROUP_B),
                    direction,
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

fn terminal_recovery_snapshot() -> ProjectSnapshot {
    let mut snapshot = ProjectSnapshot::default();
    snapshot
        .registry_mut()
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    let (next, _) = snapshot
        .with_new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A))
        .unwrap();
    snapshot = next;
    let (next, _) = snapshot
        .with_new_terminal(terminal_spec(PROJECT_A, GROUP_A, TERMINAL_C))
        .unwrap();
    snapshot = next;
    snapshot = snapshot
        .with_terminal_run_intent(terminal_id(TERMINAL_C), RunIntent::Exited)
        .unwrap();
    snapshot
        .registry_mut()
        .add_project(project_id(PROJECT_B), path("D:/work/b"))
        .unwrap();
    let (next, _) = snapshot
        .with_new_terminal(terminal_spec(PROJECT_B, GROUP_B, TERMINAL_B))
        .unwrap();
    snapshot = next;
    snapshot
        .registry_mut()
        .activate_project(project_id(PROJECT_A))
        .unwrap();
    snapshot
}

fn save_terminal_recovery_snapshot(paths: &AppPaths, snapshot: &ProjectSnapshot) {
    let mut store = WorkspaceDatabase::open(paths).unwrap();
    store.save_project_snapshot(snapshot).unwrap();
}

fn assert_project_snapshot_saved(
    root: &Entity<AppRoot>,
    cx: &TestAppContext,
    writer: &WorkspaceWriter,
    paths: &AppPaths,
) {
    writer.flush().unwrap();
    let mut store = WorkspaceDatabase::open(paths).unwrap();
    let persisted = store.load_project_snapshot().unwrap();
    root.read_with(cx, |root, _| assert_eq!(root.snapshot(), &persisted));
}

#[gpui::test]
fn project_recovery_failure_blocks_snapshot_overwrite(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut original = ProjectSnapshot::default();
    original
        .registry_mut()
        .add_project(project_id(PROJECT_A), path("D:/work/a"))
        .unwrap();
    save_terminal_recovery_snapshot(&paths, &original);
    let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    let valid_layout = raw.query_row(
        "SELECT layout_json FROM project_layouts WHERE project_id = ?1",
        [PROJECT_A],
        |row| row.get::<_, String>(0),
    );
    let valid_layout = valid_layout.unwrap();
    raw.execute(
        "UPDATE project_layouts SET layout_json = 'not-json' WHERE project_id = ?1",
        [PROJECT_A],
    )
    .unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    assert!(persistence.project_snapshot_recovery_failed());
    raw.execute(
        "UPDATE project_layouts SET layout_json = ?2 WHERE project_id = ?1",
        rusqlite::params![PROJECT_A, valid_layout],
    )
    .unwrap();
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    open_generated_project(&root, &mut window, "D:/work/b");
    writer.flush().unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(
        store.load_project_snapshot().unwrap(),
        original,
        "恢复失败后的编辑不得覆盖原项目"
    );
    let weak = root.downgrade();
    window.update(|window, _| window.remove_window());
    drop(root);
    cx.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();
    writer.shutdown().unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), original);
}

#[gpui::test]
fn terminal_restore_visibility_and_exit_state(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let snapshot = terminal_recovery_snapshot();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);

    assert_eq!(state.borrow().attempts, 1, "只启动可见 Launch 终端");
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &snapshot);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        let active_panel = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
            .unwrap()
            .0;
        assert_eq!(
            root.focused_panel(project_id(PROJECT_A), cx),
            Some(active_panel),
            "默认焦点必须指向布局 active 面板"
        );
        let visible = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1;
        assert!(visible.read(cx).session.is_some());
        let exited = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_C), cx)
            .unwrap()
            .1;
        assert!(exited.read(cx).session.is_none());
        assert!(exited.read(cx).is_exited());
    });
    assert!(window.update(|window, cx| window.focused(cx)).is_none());
    writer.flush().unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), snapshot);
    draw_frame(&mut window);
    assert!(
        window.debug_bounds(terminal_selector(TERMINAL_C)).is_some(),
        "Exited 恢复面板必须实际可绘制"
    );
    let activation = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(
        activation.is_ok(),
        "恢复项目必须允许面板聚焦: {activation:?}"
    );

    activate_project(&root, &mut window, PROJECT_B);
    assert_eq!(state.borrow().attempts, 2, "隐藏项目首次可见才懒启动");
    let (_, lazy_entity, _) = panel_entity(&root, cx, PROJECT_B, TERMINAL_B);
    root.read_with(cx, |root, cx| {
        let lazy = root
            .terminal_panel(project_id(PROJECT_B), terminal_id(TERMINAL_B), cx)
            .unwrap()
            .1;
        assert!(lazy.read(cx).session.is_some());
    });
    activate_project(&root, &mut window, PROJECT_A);
    activate_project(&root, &mut window, PROJECT_B);
    assert_eq!(state.borrow().attempts, 2, "再次激活不得重复启动");
    assert_eq!(
        panel_entity(&root, cx, PROJECT_B, TERMINAL_B).1,
        lazy_entity
    );
}

#[gpui::test]
fn modal_project_activation_does_not_launch_hidden_terminal(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    open_side_menu_for_test(&mut window);
    let attempts = state.borrow().attempts;

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_project(project_id(PROJECT_B), window, cx)
        })
    });

    assert!(matches!(
        result,
        Err(AppRootError::Focus(FocusError::ModalActive))
    ));
    assert_eq!(state.borrow().attempts, attempts, "激活失败不得启动 PTY");
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
    });
}

#[gpui::test]
fn modal_duplicate_project_open_does_not_launch_hidden_terminal(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    save_terminal_recovery_snapshot(&paths, &terminal_recovery_snapshot());
    let persistence = AppRootPersistence::load(&paths, None);
    let (root, mut window, state) = open_root_with_persistence(cx, persistence);
    open_side_menu_for_test(&mut window);
    let attempts = state.borrow().attempts;

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.add_project(project_id(PROJECT_B), path("D:/work/b"), window, cx)
        })
    });

    assert!(matches!(
        result,
        Err(AppRootError::Focus(FocusError::ModalActive))
    ));
    assert_eq!(state.borrow().attempts, attempts, "打开失败不得启动 PTY");
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_B), cx), 0);
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
    });
}

#[gpui::test]
fn terminal_restore_failure_retains_panel_without_duplicate(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let snapshot = terminal_recovery_snapshot();
    save_terminal_recovery_snapshot(&paths, &snapshot);
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, state) =
        open_root_with_persistence_and_size_configured(cx, persistence, None, |launcher| {
            launcher.fail_next = true
        });

    assert_eq!(state.borrow().attempts, 1);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.snapshot(), &snapshot);
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 2);
        let failed = root
            .terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1;
        assert!(failed.read(cx).session.is_none());
        assert!(failed.read(cx).has_error(), "恢复失败必须保留错误状态");
    });
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
                .unwrap();
        });
    });
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("slterm-terminal-error").is_some(),
        "恢复错误区域必须实际呈现"
    );
    window.update(|window, cx| {
        root.update(cx, |root, cx| root.restore_active_project(window, cx));
    });
    assert_eq!(state.borrow().attempts, 1, "失败面板不得被重复恢复");
    let expected = root.read_with(cx, |root, _| root.snapshot().clone());
    writer.flush().unwrap();
    let mut store = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), expected);
}

#[test]
fn terminal_command_definitions_preserve_reserved_interrupt() {
    let definitions = CommandRegistry::terminal_definitions();
    let bindings = definitions
        .iter()
        .map(|definition| (definition.id, definition.keystroke, definition.context))
        .collect::<Vec<_>>();
    assert_eq!(
        bindings,
        [
            (CommandId::NewTerminal, "ctrl-shift-t", None),
            (CommandId::SplitRight, "ctrl-shift-d", None),
            (CommandId::SplitDown, "ctrl-shift-s", None),
            (CommandId::CloseActiveTerminal, "ctrl-shift-w", None),
        ]
    );
    let mut unique = BTreeSet::new();
    assert!(
        definitions
            .iter()
            .all(|definition| { definition.keystroke != "ctrl+c" && unique.insert(definition.id) })
    );
}

#[gpui::test]
fn modal_terminal_command_is_rejected_without_side_effects(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    open_side_menu_for_test(&mut window);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::NewTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                Some(CommandParameters::NewTerminal {
                    shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
                }),
                window,
                cx,
            )
        })
    });

    assert!(matches!(
        result,
        Err(AppRootError::Focus(FocusError::ModalActive))
    ));
    assert_eq!(state.borrow().attempts, 0);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 0)
    });
}

#[gpui::test]
fn sidebar_command_request_does_not_require_active_project(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::ActivateSideView,
                CommandSource::Pointer,
                CommandTarget::SideView(SideViewId::Explorer),
                None,
                window,
                cx,
            )
        })
    });

    assert!(matches!(result, Ok(CommandOutcome::Completed)));
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.side_views
                .state()
                .open_view(slterm_sidebar::SideSlot::Upper),
            Some(SideViewId::Explorer)
        );
    });
}

#[gpui::test]
fn terminal_command_requests_share_execution_path(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");

    let missing_new = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::NewTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        missing_new,
        Err(AppRootError::UnexpectedParameters)
    ));
    let missing_split = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::SplitRight,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(matches!(
        missing_split,
        Err(AppRootError::UnexpectedParameters)
    ));
    assert_eq!(state.borrow().attempts, 0);

    let created = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::NewTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                Some(CommandParameters::NewTerminal {
                    shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
                }),
                window,
                cx,
            )
        })
    });
    assert!(matches!(&created, Ok(CommandOutcome::TerminalPanel(_))));
    let created = created.unwrap();
    assert!(matches!(created, CommandOutcome::TerminalPanel(_)));
    let split = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::SplitRight,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                Some(CommandParameters::Split {
                    ratio: 0.25,
                    viewport: LogicalSize::new(1200.0, 800.0),
                    shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
                }),
                window,
                cx,
            )
            .unwrap()
        })
    });
    let CommandOutcome::TerminalPanel(split_panel) = split else {
        panic!("拆分命令必须返回面板身份");
    };
    let ratio = root.read_with(cx, |root, _| {
        match root
            .snapshot
            .registry()
            .project(project_id(PROJECT_A))
            .unwrap()
            .layout()
            .tree()
            .unwrap()
        {
            SplitTree::Split { ratio, .. } => *ratio,
            SplitTree::Leaf(_) => panic!("拆分后必须为分割树"),
        }
    });
    assert!((ratio - 0.25).abs() <= f32::EPSILON);
    let expected_terminal = root.read_with(cx, |root, cx| {
        match &root.projects[&project_id(PROJECT_A)].read(cx).panels[&split_panel].view_key {
            ViewKey::Terminal { terminal_id, .. } => *terminal_id,
            _ => panic!("拆分面板必须为终端"),
        }
    });

    let closed = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Keyboard,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
            .unwrap()
        })
    });
    let CommandOutcome::TerminalClosed(record) = closed else {
        panic!("关闭命令必须返回逻辑终端记录");
    };
    assert_eq!(record.id(), expected_terminal);
    root.read_with(cx, |root, cx| {
        assert_eq!(root.panel_count(project_id(PROJECT_A), cx), 1)
    });
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
fn project_snapshot_mutations_persist_through_workspace_writer(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let persistence = AppRootPersistence::load(&paths, Some(writer.client()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    split_terminal(&root, &mut window);
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
                .unwrap();
        });
    });
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    window.update(|_, cx| {
        let terminal = root.read_with(cx, |root, cx| {
            root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_B), cx)
                .unwrap()
                .1
        });
        terminal.update(cx, |_, cx| cx.emit(TerminalViewEvent::Exited));
    });
    window.run_until_parked();
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_B), window, cx)
                .unwrap();
        });
    });
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
    activate_project(&root, &mut window, PROJECT_A);
    assert_project_snapshot_saved(&root, cx, &writer, &paths);
}

#[gpui::test]
fn rejected_project_snapshot_is_submitted_when_app_root_drops(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let client = writer.client();
    let persistence = AppRootPersistence::load(&paths, Some(client.clone()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    client
        .save_project_snapshot(&ProjectSnapshot::default())
        .unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    for _ in 0..=64 {
        if client
            .save_project_snapshot(&ProjectSnapshot::default())
            .is_err()
        {
            break;
        }
    }

    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let expected = root.read_with(cx, |root, _| {
        assert!(root.pending_final_project_snapshot.is_some());
        root.snapshot().clone()
    });
    let weak = root.downgrade();
    window.update(|window, _| window.remove_window());
    drop(root);
    cx.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();

    fault.execute_batch("ROLLBACK").unwrap();
    writer.shutdown().unwrap();
    assert_eq!(observer.load_project_snapshot().unwrap(), expected);
}

#[gpui::test]
fn accepted_project_snapshot_supersedes_rejected_final_state(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
    let mut observer = WorkspaceDatabase::open(&paths).unwrap();
    let writer = WorkspaceWriter::start(paths.clone()).unwrap();
    let client = writer.client();
    let persistence = AppRootPersistence::load(&paths, Some(client.clone()));
    let (root, mut window, _) = open_root_with_persistence(cx, persistence);
    let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
    fault.execute_batch("BEGIN EXCLUSIVE").unwrap();
    client
        .save_project_snapshot(&ProjectSnapshot::default())
        .unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    for _ in 0..=64 {
        if client
            .save_project_snapshot(&ProjectSnapshot::default())
            .is_err()
        {
            break;
        }
    }
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    root.read_with(cx, |root, _| {
        assert!(root.pending_final_project_snapshot.is_some())
    });
    fault.execute_batch("ROLLBACK").unwrap();
    writer.flush().unwrap();

    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    let expected = root.read_with(cx, |root, _| {
        assert!(root.pending_final_project_snapshot.is_none());
        root.snapshot().clone()
    });
    writer.flush().unwrap();
    assert_eq!(observer.load_project_snapshot().unwrap(), expected);
    let weak = root.downgrade();
    window.update(|window, _| window.remove_window());
    drop(root);
    cx.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();
    writer.shutdown().unwrap();

    assert_eq!(
        observer.load_project_snapshot().unwrap(),
        expected,
        "后续成功快照必须作废旧拒收终态"
    );
}

#[gpui::test]
fn failed_launch_keeps_snapshot_and_instances_unchanged(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    state.borrow_mut().fail_next = true;

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(
                terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A),
                window,
                cx,
            )
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
fn project_switch_restores_actual_panel_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_C, TERMINAL_C);
    activate_project(&root, &mut window, PROJECT_A);
    draw_frame(&mut window);

    let first = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    window.update(|window, cx| {
        let focus = first.read(cx).focus_handle.clone();
        focus.focus(window, cx);
    });
    window.run_until_parked();

    activate_project(&root, &mut window, PROJECT_B);
    activate_project(&root, &mut window, PROJECT_A);

    window.update(|window, cx| {
        assert!(first.read(cx).focus_handle.is_focused(window));
    });
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
fn closing_last_panel_releases_actual_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let terminal = root.read_with(cx, |root, cx| {
        root.terminal_panel(project_id(PROJECT_A), terminal_id(TERMINAL_A), cx)
            .unwrap()
            .1
    });
    let terminal_focus = terminal.read_with(cx, |terminal, _| terminal.focus_handle.clone());

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    window.update(|window, cx| {
        let focused = window.focused(cx);
        assert!(focused.is_some(), "空项目必须保留可用焦点落点");
        assert_ne!(focused, Some(terminal_focus), "不得留住已销毁面板焦点");
    });
}

#[gpui::test]
fn closing_remembered_panel_preserves_actual_project_focus(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    let project_focus = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .focus_handle
            .clone()
    });
    window.update(|window, cx| project_focus.focus(window, cx));
    window.run_until_parked();

    let result = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.close_terminal(terminal_id(TERMINAL_B), window, cx)
        })
    });
    assert!(result.is_ok(), "关闭终端失败: {result:?}");
    window.run_until_parked();

    window.update(|window, _| {
        assert!(
            project_focus.is_focused(window),
            "关闭非真实焦点面板不得抢走项目容器焦点"
        );
    });
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
    let restored = window.update(|window, cx| {
        LocalTerminalLauncher.restore(
            PanelIdAllocator::new(1).allocate().unwrap(),
            &record,
            window,
            cx,
        )
    });
    restored.read_with(cx, |terminal, _| {
        assert!(terminal.session.is_none(), "非法恢复元数据不得启动 PTY");
        assert!(!terminal.is_exited(), "启动失败不得被伪装成 Exited");
    });
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

#[gpui::test]
fn open_project_generates_uuid_v7_identity(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);

    let opened = open_generated_project(&root, &mut window, "D:/work/generated");

    assert_eq!(opened.as_uuid().get_version_num(), 7);
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().active(), Some(opened));
        assert_eq!(root.snapshot().terminals().count(), 0);
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn new_terminal_generates_uuid_v7_and_reuses_focused_group(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");

    new_generated_terminal(&root, &mut window);
    new_generated_terminal(&root, &mut window);

    root.read_with(cx, |root, _| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        let groups = layout.group_ids();
        assert_eq!(groups.len(), 1, "连续新建应进入当前聚焦组");
        assert_eq!(groups[0].as_uuid().get_version_num(), 7);
        assert_eq!(layout.group(groups[0]).unwrap().views().len(), 2);
        let terminals: Vec<_> = root.snapshot().terminals().collect();
        assert_eq!(terminals.len(), 2);
        assert!(terminals.iter().all(|record| {
            record.project_id() == project && record.id().as_uuid().get_version_num() == 7
        }));
        assert_ne!(terminals[0].id(), terminals[1].id());
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn split_terminal_generates_uuid_v7_and_uses_focused_group(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);

    let split = split_generated_terminal(&root, &mut window);

    root.read_with(cx, |root, cx| {
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        let groups = layout.group_ids();
        assert_eq!(groups.len(), 2);
        assert_ne!(groups[0], groups[1]);
        assert!(groups.iter().all(|id| id.as_uuid().get_version_num() == 7));
        let terminals: Vec<_> = root.snapshot().terminals().collect();
        assert_eq!(terminals.len(), 2);
        assert!(terminals.iter().all(|record| {
            record.project_id() == project && record.id().as_uuid().get_version_num() == 7
        }));
        assert_eq!(root.focused_panel(project, cx), Some(split));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn close_focused_terminal_closes_split_panel_and_updates_focus(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    let first = new_generated_terminal(&root, &mut window);
    let split = split_generated_terminal(&root, &mut window);

    let closed = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(closed.is_ok(), "关闭聚焦终端失败: {closed:?}");
    let CommandOutcome::TerminalClosed(closed) = closed.unwrap() else {
        unreachable!("关闭命令必须返回逻辑记录");
    };

    root.read_with(cx, |root, cx| {
        assert!(root.snapshot().terminal(closed.id()).is_none());
        assert_eq!(root.snapshot().terminals().count(), 1);
        assert_eq!(root.focused_panel(project, cx), Some(first));
        assert_eq!(root.panel_count(project, cx), 1);
        let layout = root
            .snapshot()
            .registry()
            .project(project)
            .unwrap()
            .layout();
        assert_eq!(layout.group_ids().len(), 1);
    });
    assert!(matches!(
        state.borrow().sessions[&split.sequence()].try_recv(),
        Ok(Msg::Shutdown)
    ));
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn production_commands_reject_missing_active_project_or_focused_group(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);

    let without_project = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal(path("C:/Program Files/PowerShell/7/pwsh.exe"), window, cx)
        })
    });
    assert!(matches!(
        without_project,
        Err(AppRootError::ProjectNotActive)
    ));

    open_generated_project(&root, &mut window, "D:/work/generated");
    let without_group = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::SplitRight,
                CommandSource::Application,
                CommandTarget::Focused,
                Some(CommandParameters::Split {
                    ratio: 0.5,
                    viewport: LogicalSize::new(1200.0, 800.0),
                    shell: path("C:/Program Files/PowerShell/7/pwsh.exe"),
                }),
                window,
                cx,
            )
        })
    });
    assert!(matches!(without_group, Err(AppRootError::MissingPanel)));
    let without_focus = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.execute_command(
                CommandId::CloseActiveTerminal,
                CommandSource::Application,
                CommandTarget::Focused,
                None,
                window,
                cx,
            )
        })
    });
    assert!(matches!(without_focus, Err(AppRootError::MissingPanel)));
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn open_project_duplicate_root_returns_existing_identity(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    let first = open_generated_project(&root, &mut window, "D:/work/generated");

    let second = open_generated_project(&root, &mut window, "D:/work/generated");

    assert_eq!(second, first);
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().projects().len(), 1);
        assert_eq!(root.snapshot().registry().active(), Some(first));
    });
}

#[gpui::test]
fn initial_project_startup_opens_one_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);

    let started = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.open_initial_project(path("D:/work/initial"), window, cx)
        })
    });

    assert!(started.is_ok(), "初始项目启动失败: {started:?}");
    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().projects().len(), 1);
        assert_eq!(root.snapshot().terminals().count(), 1);
        assert_eq!(
            root.snapshot.registry().active().and_then(|id| {
                root.snapshot
                    .registry()
                    .project(id)
                    .map(|project| project.root())
            }),
            Some(&path("D:/work/initial"))
        );
    });
    assert_eq!(state.borrow().attempts, 1);
}

#[gpui::test]
fn notification_focus_activates_the_owning_project_and_panel(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::NotificationFocus(Some(PaneId(first.sequence()))),
                window,
                cx,
            )
        })
    });

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.snapshot().registry().active(),
            Some(project_id(PROJECT_A))
        );
        assert_eq!(root.focused_panel(project_id(PROJECT_A), cx), Some(first));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn open_directories_shell_event_opens_projects_without_terminals(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);

    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.handle_shell_event(
                SltermShellEvent::OpenDirectories(vec!["D:/work/external".to_owned()]),
                window,
                cx,
            )
        })
    });

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().projects().len(), 1);
        assert_eq!(root.snapshot().terminals().count(), 0);
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn shell_event_pump_dispatches_queued_events_to_app_root(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let (sender, receiver) = std::sync::mpsc::channel();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx)
        })
    });

    let _ = sender.send(SltermShellEvent::OpenDirectories(vec![
        "D:/work/pumped".to_owned(),
    ]));
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().registry().projects().len(), 1);
    });
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn shell_event_pump_releases_receiver_on_app_root_drop(cx: &mut TestAppContext) {
    let (root, mut window, _) = open_root(cx);
    let (sender, receiver) = std::sync::mpsc::channel();
    window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.start_shell_event_pump(receiver, window.window_handle(), cx)
        })
    });
    let weak = root.downgrade();

    window.update(|window, _| window.remove_window());
    drop(root);
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();
    cx.update(|_| {});
    weak.assert_released();
    window.executor().advance_clock(Duration::from_millis(121));
    window.run_until_parked();

    assert!(
        sender.send(SltermShellEvent::TrayQuit).is_err(),
        "AppRoot 销毁后不得保留事件接收端"
    );
}

#[gpui::test]
fn new_terminal_shortcut_uses_app_root_command(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("ctrl-shift-t");
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 2)
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn split_right_shortcut_uses_app_root_command(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("ctrl-shift-d");
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 2);
        let project = root
            .snapshot
            .registry()
            .project(root.snapshot.registry().active().unwrap())
            .unwrap();
        assert!(matches!(
            project.layout().tree(),
            Some(slterm_split::SplitTree::Split {
                direction: SplitDirection::LeftRight,
                ..
            })
        ));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn split_shortcut_accounts_for_visible_sidebar_width(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root_with_persistence_and_size(
        cx,
        AppRootPersistence::empty(None),
        Some(gpui::size(gpui::px(1000.0), gpui::px(800.0))),
    );
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    draw_frame(&mut window);
    let content = window.debug_bounds("app-root-content").unwrap();
    let viewport = window.update(|window, _| window.viewport_size());
    assert!(f32::from(content.size.width) < f32::from(viewport.width));

    for _ in 0..3 {
        window.simulate_keystrokes("ctrl-shift-d");
        window.run_until_parked();
    }

    root.read_with(cx, |root, cx| {
        assert_eq!(
            root.panel_count(project, cx),
            3,
            "viewport={viewport:?}, content={content:?}"
        );
        assert_eq!(root.snapshot().terminals().count(), 3);
    });
    assert_eq!(state.borrow().attempts, 3, "项目区不足的拆分不得启动终端");
}

#[gpui::test]
fn split_down_shortcut_uses_app_root_command(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("ctrl-shift-s");
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 2);
        let project = root
            .snapshot
            .registry()
            .project(root.snapshot.registry().active().unwrap())
            .unwrap();
        assert!(matches!(
            project.layout().tree(),
            Some(slterm_split::SplitTree::Split {
                direction: SplitDirection::TopBottom,
                ..
            })
        ));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn close_terminal_shortcut_uses_app_root_command(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, "D:/work/generated");
    new_generated_terminal(&root, &mut window);
    split_generated_terminal(&root, &mut window);
    draw_frame(&mut window);

    window.simulate_keystrokes("ctrl-shift-w");
    window.run_until_parked();

    root.read_with(cx, |root, _| {
        assert_eq!(root.snapshot().terminals().count(), 1);
        assert_eq!(root.snapshot().registry().active(), Some(project));
    });
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn clicking_split_group_activates_existing_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    let first_panel = new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, first_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    draw_frame(&mut window);
    let bounds = window
        .debug_bounds(group_selector(GROUP_A))
        .expect("待激活面板组必须绘制");

    window.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::default());
    window.run_until_parked();

    assert_eq!(
        root.read_with(cx, |root, cx| root.focused_panel(project_id(PROJECT_A), cx)),
        Some(first_panel)
    );
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1,
        first_entity,
        "鼠标激活不得重建终端实体"
    );
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn empty_project_renders_empty_state_without_terminal(cx: &mut TestAppContext) {
    let (_root, mut window, state) = open_root(cx);
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-empty").is_some());

    add_project(&_root, &mut window, PROJECT_A, "D:/work/a");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-empty").is_some());
    assert!(window.debug_bounds("slterm-terminal").is_none());
    assert_eq!(state.borrow().attempts, 0);
}

#[gpui::test]
fn active_group_renders_only_its_active_terminal(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, first_entity, _) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_B);
    draw_frame(&mut window);

    assert!(window.debug_bounds(terminal_selector(TERMINAL_A)).is_none());
    assert!(window.debug_bounds(terminal_selector(TERMINAL_B)).is_some());

    let activated = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.activate_terminal(terminal_id(TERMINAL_A), window, cx)
        })
    });
    assert!(activated.is_ok(), "激活组内终端失败: {activated:?}");
    draw_frame(&mut window);

    assert!(window.debug_bounds(terminal_selector(TERMINAL_A)).is_some());
    assert!(window.debug_bounds(terminal_selector(TERMINAL_B)).is_none());
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1,
        first_entity,
        "切换组内激活项不得重建终端实体"
    );
    assert_eq!(state.borrow().attempts, 2);
}

#[gpui::test]
fn split_groups_render_with_project_layout_geometry(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal(&root, &mut window);
    draw_frame(&mut window);

    let content = window
        .debug_bounds("app-root-content")
        .expect("项目内容必须绘制");
    let first = window
        .debug_bounds(group_selector(GROUP_A))
        .expect("第一面板组必须绘制");
    let second = window
        .debug_bounds(group_selector(GROUP_B))
        .expect("第二面板组必须绘制");
    assert!(first.size.width > gpui::px(0.0));
    assert!(second.size.width > gpui::px(0.0));
    assert_eq!(first.origin, content.origin);
    assert_eq!(first.origin.y, second.origin.y);
    assert!(first.origin.x < second.origin.x);
    let expected_width = (f32::from(content.size.width) - slterm_split::DIVIDER_GAP) / 2.0;
    assert!((f32::from(first.size.width) - expected_width).abs() <= 1.0);
    assert!((f32::from(second.size.width) - expected_width).abs() <= 1.0);
    assert_eq!(first.size.height, content.size.height);
    assert_eq!(second.size.height, content.size.height);
}

#[gpui::test]
fn vertical_split_groups_render_with_project_layout_geometry(cx: &mut TestAppContext) {
    let (root, mut window, _state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    split_terminal_in_direction(&root, &mut window, SplitDirection::TopBottom);
    draw_frame(&mut window);

    let content = window
        .debug_bounds("app-root-content")
        .expect("项目内容必须绘制");
    let first = window
        .debug_bounds(group_selector(GROUP_A))
        .expect("第一面板组必须绘制");
    let second = window
        .debug_bounds(group_selector(GROUP_B))
        .expect("第二面板组必须绘制");
    assert!(first.size.height > gpui::px(0.0));
    assert!(second.size.height > gpui::px(0.0));
    assert_eq!(first.origin, content.origin);
    assert_eq!(first.origin.x, second.origin.x);
    assert!(first.origin.y < second.origin.y);
    let expected_height = (f32::from(content.size.height) - slterm_split::DIVIDER_GAP) / 2.0;
    assert!((f32::from(first.size.height) - expected_height).abs() <= 1.0);
    assert!((f32::from(second.size.height) - expected_height).abs() <= 1.0);
    assert_eq!(first.size.width, content.size.width);
    assert_eq!(second.size.width, content.size.width);
}

#[gpui::test]
fn project_switch_changes_only_the_rendered_project(cx: &mut TestAppContext) {
    let (root, mut window, state) = open_root(cx);
    add_project(&root, &mut window, PROJECT_A, "D:/work/a");
    new_terminal(&root, &mut window, PROJECT_A, GROUP_A, TERMINAL_A);
    let (_, first_entity, weak) = panel_entity(&root, cx, PROJECT_A, TERMINAL_A);
    add_project(&root, &mut window, PROJECT_B, "D:/work/b");
    new_terminal(&root, &mut window, PROJECT_B, GROUP_B, TERMINAL_B);
    draw_frame(&mut window);

    assert!(window.debug_bounds(group_selector(GROUP_A)).is_none());
    assert!(window.debug_bounds(group_selector(GROUP_B)).is_some());
    assert!(weak.upgrade().is_some());

    activate_project(&root, &mut window, PROJECT_A);
    draw_frame(&mut window);

    assert!(window.debug_bounds(group_selector(GROUP_A)).is_some());
    assert!(window.debug_bounds(group_selector(GROUP_B)).is_none());
    assert_eq!(
        panel_entity(&root, cx, PROJECT_A, TERMINAL_A).1,
        first_entity
    );
    assert_eq!(state.borrow().attempts, 2);
}
