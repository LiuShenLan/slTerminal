//! 应用壳:GPUI 消息循环装配与窗口生命周期。
//!
//! 壳即前端:`AppRoot` 直持项目快照与全部运行面板，无跨进程投影层。

// 编译期拦截(照上游随迁):裸 println!/eprintln! 在无效标准句柄上 panic;
// console 子系统下句柄通常有效,拦截宏仍保留——诊断统一走 try_write_stderr 单点。
// 父模块宏文本域覆盖所有子模块,普通 `cargo check` 即可拦截。
#[allow(unused_macros)]
macro_rules! println {
    ($($args:tt)*) => {
        compile_error!("GPUI code must not use println!; use a fallible diagnostic sink")
    };
}

#[allow(unused_macros)]
macro_rules! eprintln {
    ($($args:tt)*) => {
        compile_error!("GPUI code must not use eprintln!; use try_write_stderr")
    };
}

pub(crate) mod app_root;
pub(crate) mod assets;
// 源码文件 tab:普通源码与 gitshow 只读臂。
#[allow(dead_code)] // 生产消费归 Document 与文本视图阶段。
pub(crate) mod code_tab;
pub(crate) mod config;
// 通用 diff 对照面(双栏只读)。
#[allow(dead_code)] // 生产消费归文件与 Git 阶段。
pub(crate) mod diff_tab;
// 文档/图片 tab + openable_in_app 路由单判定。
#[allow(dead_code)] // 生产消费归 Document 与 Markdown 阶段。
pub(crate) mod doc_tabs;
// 编辑器语义槽装配(颜色单源 = UI token)。
#[allow(dead_code)] // 生产消费归 Document 与文本视图阶段。
pub(crate) mod editor_theme;
// 文件树拖放载荷 + 独立幽灵预览。
#[allow(dead_code)] // 生产消费归文件与 Git 阶段。
pub(crate) mod file_drop;
// 文本文件视图骨架(只读视图与编辑内核)。
#[allow(dead_code)] // 生产消费归 Document 与文本视图阶段。
pub(crate) mod file_editor;
// 外部文件系统事件监听(树根递归 + 打开文件父目录;有界唤醒前台)。
#[allow(dead_code)] // 生产消费归文件与 Git 阶段。
pub(crate) mod fs_watch;
pub(crate) mod theme;
// 窗口视效 + 壁纸:模糊材质/透明度纪律/有界壁纸加载。
#[allow(dead_code)] // 预览与设置消费归设置、主题与恢复阶段。
pub(crate) mod wallpaper;
pub(crate) mod windowing;

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::gpui_terminal::PaneId;
use gpui::{App, QuitMode};
use slterm_path::CanonicalPath;

/// GPUI 主窗标准错误句柄不可依赖:保留父进程重定向诊断,但写失败不能再把
/// 非致命错误升级成应用 panic。
pub(crate) fn try_write_stderr(args: fmt::Arguments<'_>) {
    use std::io::Write as _;

    let _ = writeln!(std::io::stderr(), "{args}");
}

/// 壳跨线程唤醒事件；生产者与消费归各领域模块。
pub(crate) enum SltermShellEvent {
    /// 消费(右键「在此处打开」/on_open_urls 落盘)归窗口/平台域;本模块只登记发送端。
    #[allow(dead_code)] // 打开目录载荷由平台消费，壳事件本身只负责转发。
    OpenDirectories(Vec<String>),
    /// toast 点击回窗(统一走窗口 facade)。
    /// 托盘左键/菜单「显示」同样走这条单窗回窗语义。
    NotificationFocus(Option<PaneId>),
    /// 权限确认 toast 的动作按钮回传(只携回窗目标;不回写权限决定)。
    NotificationChoice { pane_id: PaneId },
    /// 托盘「退出」:与关窗同一条退出链(终局快照 + 退出)。
    TrayQuit,
    /// 工作区状态后台写失败；不携带路径、SQL 或状态正文。
    WorkspaceWriteFailed,
}

/// GPUI 外独占 writer 与失败转发线程，窗口只拿非阻塞 client。
struct WorkspaceWriterRuntime {
    writer: crate::persistence::workspace_writer::WorkspaceWriter,
    client: crate::persistence::workspace_writer::WorkspaceWriterClient,
    failure_forwarder: std::thread::JoinHandle<()>,
}

impl WorkspaceWriterRuntime {
    fn start_at(
        paths: &slterm_path::AppPaths,
        events: mpsc::Sender<SltermShellEvent>,
    ) -> Result<Self, crate::persistence::workspace_writer::WorkspaceWriterError> {
        use crate::persistence::workspace_writer::{
            WorkspaceWriter, WorkspaceWriterError, spawn_workspace_failure_forwarder,
        };

        let mut writer = WorkspaceWriter::start(paths.clone())?;
        let client = writer.client();
        let failures = writer
            .take_failure_receiver()
            .expect("新 writer 的失败接收端必须可移交一次");
        let failure_forwarder = spawn_workspace_failure_forwarder(failures, move || {
            let _ = events.send(SltermShellEvent::WorkspaceWriteFailed);
        })
        .map_err(WorkspaceWriterError::Spawn)?;
        Ok(Self {
            writer,
            client,
            failure_forwarder,
        })
    }

    fn client(&self) -> crate::persistence::workspace_writer::WorkspaceWriterClient {
        self.client.clone()
    }

    fn shutdown(self) -> Result<(), crate::persistence::workspace_writer::WorkspaceWriterError> {
        let Self {
            writer,
            client: _,
            failure_forwarder,
        } = self;
        let shutdown = writer.shutdown();
        let forwarder = failure_forwarder.join();
        match (shutdown, forwarder) {
            (Err(error), _) => Err(error),
            (Ok(report), _) if report.failed > 0 => {
                Err(crate::persistence::workspace_writer::WorkspaceWriterError::FlushFailed)
            }
            (Ok(_), Err(_)) => {
                Err(crate::persistence::workspace_writer::WorkspaceWriterError::Stopped)
            }
            (Ok(_), Ok(())) => Ok(()),
        }
    }
}

/// 启动 writer；失败时窗口降级为无 client，由启动计划合并恢复提示。
fn start_workspace_writer(
    paths: &slterm_path::AppPaths,
    events: mpsc::Sender<SltermShellEvent>,
) -> (
    Option<WorkspaceWriterRuntime>,
    Option<crate::persistence::workspace_writer::WorkspaceWriterClient>,
) {
    match WorkspaceWriterRuntime::start_at(paths, events) {
        Ok(runtime) => {
            let client = runtime.client();
            (Some(runtime), Some(client))
        }
        Err(error) => {
            log::warn!("workspace writer startup failed: {error}");
            (None, None)
        }
    }
}

/// 在进入 GPUI 前冻结启动 Shell 与项目根；键位处理不再探测 PATH 或读取 cwd。
fn frozen_startup_paths(
    initial_cwd: Option<PathBuf>,
) -> std::io::Result<(CanonicalPath, CanonicalPath)> {
    let shell = slterm_terminal::tty::resolve_shell(None)?;
    let shell = CanonicalPath::new(PathBuf::from(shell.program()))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let root = match initial_cwd {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let root = CanonicalPath::new(root)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    Ok((shell, root))
}

/// 本次启动是否静默。
///
/// 设置两条件(`silent_start && tray`)由 [`config::Settings::start_hidden`]
/// 单点判定;显式 CLI 目录启动恒不静默——用户主动发起时把窗口藏起来没有意义。
fn starts_hidden(restore_tabs: bool, settings: &config::Settings) -> bool {
    restore_tabs && settings.start_hidden()
}

/// 托盘线程 → 壳事件队列的翻译单点(托盘线程不碰 GPUI 状态)。
fn tray_command_event(command: crate::tray::GpuiTrayCommand) -> SltermShellEvent {
    match command {
        crate::tray::GpuiTrayCommand::Focus(pane) => SltermShellEvent::NotificationFocus(pane),
        crate::tray::GpuiTrayCommand::Quit => SltermShellEvent::TrayQuit,
    }
}

/// 在当前线程启动 GPUI 运行时并打开主窗口,阻塞直至 UI 退出。
///
/// 装配序(因果链照上游):assets → QuitMode → run 闭包内 init → open_window。
/// 关窗即退出(跨领域不变量 5):`QuitMode::LastWindowClosed` + 无 mux 驻留臂,
/// 最后一窗关闭 = 进程退出。
pub fn run_shell(initial_cwd: Option<PathBuf>) {
    // 后台任务运行时复位:静态初值即注册表默认值,这里显式
    // 再对齐一次,保证同进程重启测试/后续配置载入从确定态出发。
    crate::background_tasks::reset_defaults();
    // 安装守卫只阻止与安装事务并发的常驻启动；旧 session 移交恢复已删除。
    match crate::update_download::handoff::installation_in_progress() {
        Ok(true) => {
            log::info!("installation in progress; resident startup deferred");
            return;
        }
        Err(error) => log::warn!("could not inspect installation ownership: {error}"),
        Ok(false) => {}
    }
    let regular_start = initial_cwd.is_none();
    let (shell, initial_root) = match frozen_startup_paths(initial_cwd) {
        Ok(paths) => paths,
        Err(error) => {
            log::error!("启动路径解析失败: {error}");
            try_write_stderr(format_args!("启动路径解析失败: {error}"));
            return;
        }
    };
    let (shell_tx, shell_rx) = mpsc::channel();
    // writer owner 与转发线程留在 GPUI 外；项目快照 client 由持久化协调接入。
    let (workspace_writer, _) =
        start_workspace_writer(crate::platform::dirs::app_paths(), shell_tx.clone());
    let open_urls_tx = shell_tx.clone();
    // 通知通道开机位:toast 点击回投走同一壳事件队列;
    // AUMID 预注册幂等,必须早于第一条通知。
    crate::notify::init_gpui_activation(shell_tx.clone());
    crate::platform::notifications::prepare();
    // 托盘命令回调:托盘线程只经此把用户动作送回壳事件队列。
    crate::tray::init_gpui({
        let shell_events = shell_tx.clone();
        move |command| {
            let _ = shell_events.send(tray_command_event(command));
        }
    });
    let application = gpui_platform::application()
        .with_assets(assets::SltermAssets)
        .with_quit_mode(QuitMode::LastWindowClosed);
    application.on_open_urls(move |urls| {
        let _ = open_urls_tx.send(SltermShellEvent::OpenDirectories(urls));
    });
    application.run(move |cx| {
        init(cx);
        // 静默启动:仅无显式 CLI 目录时参与;窗口以隐藏
        // 形态创建,托盘「显示」经 focus facade 回窗。
        let start_hidden = starts_hidden(regular_start, cx.global::<config::Settings>());
        windowing::open_initial_window(cx, shell_rx, initial_root, shell, start_hidden);
    });
    // GPUI 已退出，在主线程等待最终冲刷并 join 转发器，不阻塞前台。
    if let Some(runtime) = workspace_writer
        && let Err(error) = runtime.shutdown()
    {
        log::warn!("workspace writer shutdown failed: {error}");
    }
    // 托盘收尾:先删图标再退进程,否则任务栏残影要等
    // 用户悬停才消失;join 保证删图标先于进程退出。
    crate::tray::shutdown();
}

/// 组件库/内嵌字体/主题/终端切片/工作区的一次性初始化。
fn init(cx: &mut App) {
    register_bundled_fonts(cx);
    // 组件库必须只初始化一次,否则全局 action、菜单和主题状态重复注册。
    gpui_component::init(cx);
    // 读一次快照装全局:无热应用;渲染路径不得重复读盘。
    let settings = config::Settings::load();
    // 组件库 locale 与自有文案分层:组件内置文案跟解析后的 UiLanguage。
    gpui_component::set_locale(settings.language.gpui_component_locale());
    cx.set_global(settings);
    theme::apply_dark_theme(cx);
    crate::gpui_terminal::init(cx);
    app_root::keymap::register(cx);
}

fn register_bundled_fonts(cx: &App) {
    // 在组件解析字体前注册内嵌 Maple:系统等宽字体无 NF 图标码点,提示符会出
    // 方框;跨机器字形同源是截图/UI 测试可比对的前提。导入字体目录归设置体系。
    if let Err(error) = cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(assets::REQUIRED_FONT_BYTES)])
    {
        try_write_stderr(format_args!(
            "[slterm] failed to register bundled Maple font: {error}"
        ));
    }
}

#[cfg(test)]
mod shell_event_tests {
    use std::str::FromStr;
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::{Value, json};
    use slterm_path::AppPaths;
    use slterm_settings::{RawSettings, RuntimeSettings};
    use slterm_workspace::{
        InvalidViewState, ProjectId, ProjectSnapshot, SideViewId, ViewKey, ViewKeyKind, ViewState,
        ViewStateRegistry, ViewStateSchema,
    };

    use super::{
        SltermShellEvent, WorkspaceWriterRuntime, start_workspace_writer, starts_hidden,
        tray_command_event,
    };
    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::WorkspaceWriterError;
    use crate::tray::GpuiTrayCommand;

    fn silent_settings() -> crate::app_shell::config::Settings {
        let runtime = RuntimeSettings::from_raw(&RawSettings::from_json_bytes(
            br#"{"session":{"silent_start":true},"system":{"tray":true}}"#,
        ));
        crate::app_shell::config::Settings::from_runtime(&runtime)
    }

    /// 显式 CLI 启动(`--working-directory`/`-e`)恒不静默;无显式参数
    /// 的常规启动才允许设置两条件生效。
    #[test]
    fn explicit_cli_launch_never_starts_hidden() {
        let settings = silent_settings();
        assert!(starts_hidden(true, &settings), "常规启动可静默");
        assert!(!starts_hidden(false, &settings), "显式 CLI 启动不得隐藏");
    }

    fn view_state_registry() -> ViewStateRegistry {
        fn decode(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
            serde_json::from_str(text).map_err(|_| InvalidViewState)
        }

        ViewStateRegistry::new(&[ViewStateSchema::new(ViewKeyKind::SideView, decode)]).unwrap()
    }

    fn project_snapshot(name: &str) -> ProjectSnapshot {
        let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut registry = slterm_workspace::ProjectRegistry::new();
        registry
            .add_project(
                project_id,
                slterm_path::CanonicalPath::new("D:/work/a").unwrap(),
            )
            .unwrap();
        registry.rename_project(project_id, name).unwrap();
        ProjectSnapshot::restore(registry, Vec::new()).unwrap()
    }

    /// 启动失败降级为无 client，不另发会被恢复汇总节流的独立事件。
    #[test]
    fn workspace_writer_start_failure_defers_to_recovery_summary() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        std::fs::write(&local, "not a directory").unwrap();
        let paths = AppPaths::new(temp.path().join("config"), local).unwrap();
        let (events, event_rx) = mpsc::channel();

        let (runtime, client) = start_workspace_writer(&paths, events);

        assert!(runtime.is_none());
        assert!(client.is_none());
        assert!(event_rx.try_recv().is_err());
    }

    /// 组合根共享 client、转发失败并在退出时重试待写副本、回收转发线程。
    #[test]
    fn workspace_writer_runtime_forwards_failure_and_flushes_on_shutdown() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let registry = view_state_registry();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer
            .save_view_state(
                &key,
                &ViewState::new(&key, json!({"position": 7}), &registry).unwrap(),
            )
            .unwrap();
        let (events, event_rx) = mpsc::channel();
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();

        client
            .save_view_state(
                &key,
                &ViewState::new(&key, json!({"position": 8}), &registry).unwrap(),
            )
            .unwrap();

        assert!(matches!(
            event_rx.recv_timeout(Duration::from_secs(3)),
            Ok(SltermShellEvent::WorkspaceWriteFailed)
        ));
        fault
            .execute_batch("DROP TRIGGER reject_view_state_write")
            .unwrap();
        runtime.shutdown().unwrap();
        let stored = observer.view_state(&key, &registry).unwrap().unwrap();
        assert_eq!(stored.state.state()["position"], 8);
    }

    /// 项目聚合到期写失败也必须经同一壳事件链通知，并在退出时重试。
    #[test]
    fn workspace_writer_runtime_forwards_project_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer
            .save_project_snapshot(&project_snapshot("original"))
            .unwrap();
        let (events, event_rx) = mpsc::channel();
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_project_write BEFORE UPDATE ON projects \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();

        client
            .save_project_snapshot(&project_snapshot("changed"))
            .unwrap();

        assert!(matches!(
            event_rx.recv_timeout(Duration::from_secs(4)),
            Ok(SltermShellEvent::WorkspaceWriteFailed)
        ));
        fault
            .execute_batch("DROP TRIGGER reject_project_write")
            .unwrap();
        runtime.shutdown().unwrap();
        assert_eq!(
            observer.load_project_snapshot().unwrap(),
            project_snapshot("changed")
        );
    }

    /// 退出时故障未解除必须报告最终冲刷失败，不能丢弃失败计数。
    #[test]
    fn workspace_writer_runtime_shutdown_reports_unflushed_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let registry = view_state_registry();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer
            .save_view_state(
                &key,
                &ViewState::new(&key, json!({"position": 7}), &registry).unwrap(),
            )
            .unwrap();
        let (events, _event_rx) = mpsc::channel();
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();
        client
            .save_view_state(
                &key,
                &ViewState::new(&key, json!({"position": 8}), &registry).unwrap(),
            )
            .unwrap();

        assert!(matches!(
            runtime.shutdown(),
            Err(WorkspaceWriterError::FlushFailed)
        ));
    }

    /// 托盘线程只发命令枚举;壳侧翻译单点必须把 Focus 汇到已有的回窗
    /// 事件、把 Quit 送到关窗同语义的退出事件。
    #[test]
    fn tray_commands_map_to_focus_and_quit_shell_events() {
        assert!(matches!(
            tray_command_event(GpuiTrayCommand::Focus(None)),
            SltermShellEvent::NotificationFocus(None)
        ));
        let pane = crate::gpui_terminal::PaneId(7);
        assert!(matches!(
            tray_command_event(GpuiTrayCommand::Focus(Some(pane))),
            SltermShellEvent::NotificationFocus(Some(focused)) if focused == pane
        ));
        assert!(matches!(
            tray_command_event(GpuiTrayCommand::Quit),
            SltermShellEvent::TrayQuit
        ));
    }
}
