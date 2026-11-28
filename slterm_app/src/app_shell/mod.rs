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
pub(crate) mod authorization;
#[cfg(test)]
mod crash_recovery;
mod global_close;
mod workspace_writer_shutdown;
// Markdown 安全资源：SVG 静态解码与逐资源本地授权。
#[allow(dead_code)] // 生产消费归 Markdown 块视图接线。
pub(crate) mod markdown;
mod startup_diagnostics;
// 文件树按文本打开的路由判定。
pub(crate) mod code_tab;
pub(crate) mod config;
// 文档打开路由判定(Markdown 归 AppRoot 渲染面板)。
pub(crate) mod doc_tabs;
#[allow(dead_code)] // 生产消费归 AppRoot Document 生命周期与退出链。
pub(crate) mod document_autosave;
pub(crate) mod document_close;
pub(crate) mod document_external;
#[allow(dead_code)] // 生产消费点归 AppRoot Document 面板接线。
pub(crate) mod documents;
// 编辑器语义槽装配(颜色单源 = UI token)。
#[allow(dead_code)] // 生产消费归 Document 与文本视图阶段。
pub(crate) mod editor_theme;
// Explorer 与 Agent 文件树共用的单项写计划与执行前重检。
#[allow(dead_code)] // 生产消费归文件与 Git 阶段。
pub(crate) mod file_operation_service;
#[cfg(test)]
mod file_operation_service_tests;
// Document 真实 I/O、统一保存与 Git 行标端口。
#[allow(dead_code)] // 生产消费归 Document 与文本视图阶段。
pub(crate) mod file_editor;
// Document 专用文本视图与 Unicode/IME 坐标。
#[allow(dead_code)] // 生产消费归 AppRoot Paged Document 打开与逐页读取。
pub(crate) mod paged_document;
// 粘贴 Journal 的规范事实与专属启动恢复器。
#[allow(dead_code)] // 生产消费归文件操作 writer 与启动维护链。
pub(crate) mod paste_journal;
#[cfg(test)]
mod paste_journal_tests;
// 粘贴冲突聚合、逐项串行执行与部分成功报告。
#[allow(dead_code)] // 生产消费归 Explorer 与 Agent 文件树接线。
pub(crate) mod paste_service;
#[cfg(test)]
mod paste_service_tests;
pub(crate) mod text_view;
// 外部文件系统事件监听(树根递归 + 打开文件父目录;有界唤醒前台)。
pub(crate) mod fs_watch;
pub(crate) mod theme;
// 窗口视效 + 壁纸:模糊材质/透明度纪律/有界壁纸加载。
mod shell_events;
#[allow(dead_code)] // 预览与设置消费归设置、主题与恢复阶段。
pub(crate) mod wallpaper;
#[allow(dead_code)] // 生产消费归 Explorer 与 Agent 文件树接线。
pub(crate) mod watcher_service;
#[cfg(test)]
mod watcher_service_tests;
pub(crate) mod windowing;
pub(crate) use shell_events::*;

use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use gpui::{App, QuitMode};
use slterm_path::CanonicalPath;

/// GPUI 主窗标准错误句柄不可依赖:保留父进程重定向诊断,但写失败不能再把
/// 非致命错误升级成应用 panic。
pub(crate) fn try_write_stderr(args: fmt::Arguments<'_>) {
    use std::io::Write as _;

    let _ = writeln!(std::io::stderr(), "{args}");
}

/// 控制面清理结果；未启动服务按成功处理，失败只闸门正常退出标记。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlPlaneCleanup {
    Succeeded,
    Failed,
}

/// GPUI 外独占 writer 与失败转发线程，窗口只拿非阻塞 client。
pub(crate) struct WorkspaceWriterRuntime {
    writer: crate::persistence::workspace_writer::WorkspaceWriter,
    client: crate::persistence::workspace_writer::WorkspaceWriterClient,
    failure_forwarder: std::thread::JoinHandle<()>,
    paths: slterm_path::AppPaths,
    metadata: crate::persistence::metadata::WorkspaceMetadata,
    previous_run: Option<crate::persistence::metadata::WorkspaceMetadata>,
}

impl WorkspaceWriterRuntime {
    fn start_at(
        paths: &slterm_path::AppPaths,
        events: mpsc::SyncSender<SltermShellEvent>,
    ) -> Result<Self, crate::persistence::workspace_writer::WorkspaceWriterError> {
        use crate::persistence::workspace_writer::{
            WorkspaceWriter, spawn_workspace_failure_forwarder,
        };

        Self::start_with(
            paths,
            events,
            WorkspaceWriter::start,
            spawn_workspace_failure_forwarder,
            if cfg!(test) {
                // 隔离测试不得清理真实用户临时目录；接线故障由 start_with 注入覆盖。
                (|| Ok(())) as fn() -> std::io::Result<()>
            } else {
                crate::git::cleanup_expired_system_open
            },
        )
    }

    fn start_with<StartWriter, SpawnForwarder, CleanupTemp>(
        paths: &slterm_path::AppPaths,
        events: mpsc::SyncSender<SltermShellEvent>,
        start_writer: StartWriter,
        spawn_forwarder: SpawnForwarder,
        cleanup_temp: CleanupTemp,
    ) -> Result<Self, crate::persistence::workspace_writer::WorkspaceWriterError>
    where
        StartWriter: FnOnce(
            slterm_path::AppPaths,
        ) -> Result<
            crate::persistence::workspace_writer::WorkspaceWriter,
            crate::persistence::workspace_writer::WorkspaceWriterError,
        >,
        SpawnForwarder: FnOnce(
            mpsc::Receiver<crate::persistence::workspace_writer::WorkspaceWriteFailure>,
            Box<dyn FnMut() + Send>,
        ) -> std::io::Result<std::thread::JoinHandle<()>>,
        CleanupTemp: FnOnce() -> std::io::Result<()>,
    {
        use crate::persistence::startup::{
            JournalRecoverer, JournalRecovererRegistry, MaintenanceOutcome,
            MaintenanceRequestRecoverer, MaintenanceRequestRecovererRegistry, MaintenanceRunner,
            open_workspace_with_recovery,
        };
        use crate::persistence::workspace_writer::WorkspaceWriterError;

        // 维护通过前只持有当前数据库连接；writer、转发器与领域后台任务均未创建。
        let recovered =
            open_workspace_with_recovery(paths).map_err(WorkspaceWriterError::Recovery)?;
        if let Some(notice) = recovered.notice() {
            try_send_shell_event(
                &events,
                SltermShellEvent::WorkspaceRecovered(notice),
                "workspace",
            );
        }
        let mut store = recovered.store;
        let run = crate::persistence::metadata::RunMarker {
            run_id: uuid::Uuid::now_v7().to_string(),
            process_id: std::process::id(),
            process_created_at_ms:
                crate::platform::update_installation::current_process_created_at_ms()
                    .map_err(WorkspaceWriterError::ProcessCreated)?,
        };
        let started = store
            .begin_run(&uuid::Uuid::now_v7().to_string(), run)
            .map_err(WorkspaceWriterError::Metadata)?;
        let paste_recoverer = crate::app_shell::paste_journal::PasteJournalRecoverer;
        let hooks_recoverer = crate::ai_hook::HooksJournalRecoverer::new(paths.local_root());
        let recoverers = JournalRecovererRegistry::from([
            (
                crate::app_shell::paste_journal::PASTE_JOURNAL_KIND,
                &paste_recoverer as &dyn JournalRecoverer,
            ),
            (
                crate::ai_hook::HOOKS_INSTALL_KIND,
                &hooks_recoverer as &dyn JournalRecoverer,
            ),
            (
                crate::ai_hook::HOOKS_UNINSTALL_KIND,
                &hooks_recoverer as &dyn JournalRecoverer,
            ),
        ]);
        let backup_recoverer = crate::encrypted_backup::maintenance::BackupRestoreRecoverer::new(
            paths.clone(),
            crate::app_shell::app_root::workspace_view_state_registry(),
        );
        let undo_recoverer = crate::encrypted_backup::undo::BackupUndoRecoverer::new(
            paths.clone(),
            crate::app_shell::app_root::workspace_view_state_registry(),
        );
        let request_recoverers = MaintenanceRequestRecovererRegistry::from([
            (
                crate::encrypted_backup::maintenance::BACKUP_RESTORE_KIND,
                &backup_recoverer as &dyn MaintenanceRequestRecoverer,
            ),
            (
                crate::encrypted_backup::undo::BACKUP_UNDO_KIND,
                &undo_recoverer as &dyn MaintenanceRequestRecoverer,
            ),
        ]);
        let quarantine_maintainer =
            crate::persistence::startup::FileHistoryQuarantineMaintainer::new(paths);
        let maintenance = MaintenanceRunner::new(
            &mut store,
            &started.current.store_epoch,
            &recoverers,
            &request_recoverers,
        )
        .with_local_root(paths.local_root())
        .with_history_quarantine(&quarantine_maintainer)
        .run()
        .map_err(WorkspaceWriterError::Maintenance)?;
        if let MaintenanceOutcome::NeedsAttention { stage } = maintenance {
            return Err(WorkspaceWriterError::MaintenanceNeedsAttention(stage));
        }
        hooks_recoverer.cleanup_orphans(&store);
        // 维护成功即刷新灾难副本，副本不得停留在旧世代。
        if let Err(error) = store.refresh_disaster_copy(paths) {
            log::warn!("workspace disaster copy refresh failed at startup: {error}");
        }
        drop(store);
        // 单实例锁保证此时没有另一实例的存活中转句柄；过期清理失败不阻断工作区。
        let _ = cleanup_temp();

        let mut writer = start_writer(paths.clone())?;
        let client = writer.client();
        let failures = writer
            .take_failure_receiver()
            .expect("新 writer 的失败接收端必须可移交一次");
        let failure_forwarder = spawn_forwarder(
            failures,
            Box::new(move || {
                try_send_shell_event(&events, SltermShellEvent::WorkspaceWriteFailed, "workspace");
            }),
        )
        .map_err(WorkspaceWriterError::Spawn)?;
        let metadata = started.current;
        let previous_run = started.previous;
        Ok(Self {
            writer,
            client,
            failure_forwarder,
            paths: paths.clone(),
            metadata,
            previous_run,
        })
    }

    /// 领域恢复失败：不得开窗；显式停止 writer 并 join 转发线程，不留孤儿。
    fn abort_startup(self) {
        let Self {
            writer,
            failure_forwarder,
            ..
        } = self;
        if let Err(error) = writer.shutdown() {
            log::warn!("workspace writer shutdown after domain recovery failure failed: {error}");
        }
        let _ = failure_forwarder.join();
    }

    fn client(&self) -> crate::persistence::workspace_writer::WorkspaceWriterClient {
        self.client.clone()
    }

    fn metadata(&self) -> &crate::persistence::metadata::WorkspaceMetadata {
        &self.metadata
    }

    fn previous_run(&self) -> Option<&crate::persistence::metadata::WorkspaceMetadata> {
        self.previous_run.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FinalCommitDecision {
    Retry,
    Abandon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FinalShutdownOutcome {
    Completed,
    Abandoned,
}

/// 启动 writer；任何维护或启动失败都阻止进入领域恢复与窗口装配。
fn start_workspace_writer(
    paths: &slterm_path::AppPaths,
    events: mpsc::SyncSender<SltermShellEvent>,
) -> Result<
    (
        WorkspaceWriterRuntime,
        crate::persistence::workspace_writer::WorkspaceWriterClient,
        Option<crate::persistence::metadata::WorkspaceMetadata>,
    ),
    crate::persistence::workspace_writer::WorkspaceWriterError,
> {
    let runtime = WorkspaceWriterRuntime::start_at(paths, events)?;
    let client = runtime.client();
    let previous_run = runtime.previous_run().cloned();
    Ok((runtime, client, previous_run))
}

/// writer 提供本次真实运行元数据；服务启动失败只降级控制面，不伪造运行身份。
fn start_local_control_service(
    workspace_writer: Option<&WorkspaceWriterRuntime>,
    events: mpsc::SyncSender<SltermShellEvent>,
) -> Option<(
    crate::local_control::LocalControlService,
    crate::local_control::RuntimeEventPublisher,
)> {
    let metadata = workspace_writer?.metadata().clone();
    let writer = workspace_writer?.client();
    match crate::local_control::LocalControlService::start(metadata, events, Some(writer)) {
        Ok(service) => {
            let publisher = service.event_publisher();
            Some((service, publisher))
        }
        Err(error) => {
            log::warn!("local control service startup failed: {}", error.kind());
            None
        }
    }
}

/// GPUI 外显式回收监听与发现记录；未启动服务不产生清理负担。
fn shutdown_local_control_service(
    service: Option<&mut crate::local_control::LocalControlService>,
) -> ControlPlaneCleanup {
    match service {
        Some(service) => match service.shutdown() {
            Ok(()) => ControlPlaneCleanup::Succeeded,
            Err(error) => {
                log::warn!("local control service shutdown failed: {}", error.kind());
                ControlPlaneCleanup::Failed
            }
        },
        None => ControlPlaneCleanup::Succeeded,
    }
}

/// 初始项目根：CLI 指定恒优先；设置启动目录必须是真实目录，失效回退当前目录。
fn startup_root(
    initial_cwd: Option<PathBuf>,
    configured: Option<&Path>,
) -> std::io::Result<PathBuf> {
    if let Some(path) = initial_cwd {
        return Ok(path);
    }
    match configured {
        Some(directory) if directory.is_dir() => Ok(directory.to_path_buf()),
        Some(_) => {
            log::warn!("配置的启动目录不可用，回退当前目录");
            std::env::current_dir()
        }
        None => std::env::current_dir(),
    }
}

/// 在进入 GPUI 前冻结启动 Shell 与项目根；键位处理不再探测 PATH 或读取 cwd。
fn frozen_startup_paths(
    initial_cwd: Option<PathBuf>,
    settings: &config::Settings,
) -> std::io::Result<(CanonicalPath, CanonicalPath)> {
    let user_shell = settings.shell.as_deref();
    let shell = match slterm_terminal::tty::resolve_shell(user_shell) {
        Ok(shell) => shell,
        Err(error) if user_shell.is_some() => {
            log::warn!("配置的 Shell 不可用，回退默认探测链: {error}");
            slterm_terminal::tty::resolve_shell(None)?
        }
        Err(error) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error.to_string(),
            ));
        }
    };
    let shell = CanonicalPath::new(PathBuf::from(shell.program()))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let root = startup_root(initial_cwd, settings.startup_directory.as_deref())?;
    let root = CanonicalPath::new(root)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    Ok((shell, root))
}

/// 本次启动是否静默。
///
/// 设置两条件(`silent_start && tray`)由 [`config::Settings::start_hidden`]
/// 单点判定：只有登录自启且设置允许、托盘可用时才静默。
///
/// 手工启动不携带 `--autostart`，即使设置开启也显示窗口；启动路径顺带把
/// 托盘设置交给端口，使可用性判定基于本次真实运行而非设置值。
fn starts_hidden(
    autostart: bool,
    settings: &config::Settings,
    windows: &dyn crate::platform::windows_integration::WindowsIntegration,
) -> bool {
    windows.set_tray_enabled(settings.tray);
    // 静默启动必须等托盘真实挂载：建窗或图标失败时隐藏窗口没有恢复入口。
    if autostart && settings.start_hidden() && settings.tray {
        windows.wait_tray_available(std::time::Duration::from_secs(2));
    }
    crate::platform::windows_integration::start_hidden_with(
        autostart,
        settings.start_hidden(),
        windows,
    )
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
pub fn run_shell(initial_cwd: Option<PathBuf>, autostart: bool) {
    run_shell_loop(initial_cwd, |cwd| run_shell_once(cwd, autostart));
}

/// 用户显式重试才重新进入启动链；原始 cwd 在每轮保持不变。
fn run_shell_loop(
    initial_cwd: Option<PathBuf>,
    mut run: impl FnMut(Option<PathBuf>) -> startup_diagnostics::ShellRunOutcome,
) {
    loop {
        if run(initial_cwd.clone()) == startup_diagnostics::ShellRunOutcome::Exit {
            break;
        }
    }
}

fn run_shell_once(
    initial_cwd: Option<PathBuf>,
    autostart: bool,
) -> startup_diagnostics::ShellRunOutcome {
    // 后台任务运行时复位:静态初值即注册表默认值,这里显式
    // 再对齐一次,保证同进程重启测试/后续配置载入从确定态出发。
    crate::background_tasks::reset_defaults();
    // 光标闪烁默认值跟随系统减少动效；必须在设置快照构建前写入。
    config::set_reduced_motion(crate::platform::animation::reduced_motion());
    // 安装守卫只阻止与安装事务并发的常驻启动；旧 session 移交恢复已删除。
    match crate::update_download::handoff::installation_in_progress() {
        Ok(true) => {
            log::info!("installation in progress; resident startup deferred");
            return startup_diagnostics::ShellRunOutcome::Exit;
        }
        Err(error) => log::warn!("could not inspect installation ownership: {error}"),
        Ok(false) => {}
    }
    // 预约安装：helper 就绪后本进程必须退出，覆盖安装由 helper 完成。
    // 无预约、便携版或需手动重装时立即返回 false，继续正常启动。
    if crate::update_download::handoff::apply_scheduled() {
        log::info!("scheduled update handed off; exiting for installation");
        return startup_diagnostics::ShellRunOutcome::Exit;
    }
    let regular_start = initial_cwd.is_none();
    // 设置与自定义主题在进入 GPUI 事件循环前只读一次：启动回调内不得做
    // 文件 I/O，首帧只消费已构建的快照。
    let (startup_settings, settings_warning) = config::Settings::load_with_warning();
    // Windows 集成端口:启动静默判定与设置页对账共用同一真实实例。
    let windows: std::sync::Arc<dyn crate::platform::windows_integration::WindowsIntegration> =
        std::sync::Arc::new(crate::platform::windows_integration::SystemWindowsIntegration);
    let (shell, initial_root) = match frozen_startup_paths(initial_cwd, &startup_settings) {
        Ok(paths) => paths,
        Err(error) => {
            log::error!("启动路径解析失败: {error}");
            try_write_stderr(format_args!("启动路径解析失败: {error}"));
            return startup_diagnostics::ShellRunOutcome::Exit;
        }
    };
    let (shell_tx, shell_rx) = shell_event_channel();
    // writer owner 与转发线程留在 GPUI 外；项目快照 client 由持久化协调接入。
    let app_paths = crate::platform::dirs::app_paths();
    let (workspace_writer, workspace_writer_client, previous_run) =
        match start_workspace_writer(app_paths, shell_tx.clone()) {
            Ok(started) => started,
            Err(error) => {
                // 维护失败只开放只读诊断；不得加载领域状态或启动控制面。
                log::warn!("workspace writer startup failed: {error}");
                return startup_diagnostics::run(error, app_paths.clone());
            }
        };
    // 首帧前恢复视图私有状态与上一运行退出事实；读取或解码失败只设置对应写屏障。
    let mut side_view_persistence = app_root::AppRootPersistence::load_for_production(
        app_paths,
        Some(workspace_writer_client),
        previous_run,
    );
    // 布局或快照领域恢复失败不得以空工作区静默开窗：回收 writer 后进入同一只读诊断与重试链。
    if let Some(failure) = side_view_persistence.domain_recovery_failure() {
        log::warn!("workspace domain recovery failed: {failure:?}");
        workspace_writer.abort_startup();
        return startup_diagnostics::run_domain(failure, app_paths.clone());
    }
    // 本机窗口几何在开窗前只交付一次；损坏或无记录回退默认尺寸。
    let window_geometry = side_view_persistence.take_window_geometry();
    // 崩溃快照写线程与工作区 writer 同层：本体留在 GPUI 外，前台只持 client。
    let snapshot_run_id = workspace_writer.metadata().run.run_id.clone();
    let mut snapshot_writer =
        match crate::persistence::document_snapshot::writer::DocumentSnapshotWriter::start(
            app_paths,
        ) {
            Ok(writer) => Some(writer),
            Err(error) => {
                log::warn!("document snapshot writer unavailable: {error}");
                None
            }
        };
    let snapshot_client = snapshot_writer.as_ref().map(|writer| writer.client());
    // 学习库独立于工作区库；损坏或高版本只暂停持久学习，不覆盖原文件。
    let (learning, learning_runtime) = match crate::learning::LearningService::start(app_paths) {
        Ok(started) => {
            let (service, runtime) = started;
            (service, Some(runtime))
        }
        Err(error) => {
            log::warn!("learning database unavailable; using memory only: {error}");
            (crate::learning::LearningService::memory_only(), None)
        }
    };
    // 控制面与 writer 移交给 GPUI 外关闭所有者；窗口只持有有界请求 client。
    let (local_control, runtime_events) =
        match start_local_control_service(Some(&workspace_writer), shell_tx.clone()) {
            Some((service, publisher)) => (Some(service), Some(publisher)),
            None => (None, None),
        };
    let global_close =
        match global_close::GlobalCloseRuntime::start(workspace_writer, local_control) {
            Ok(runtime) => runtime,
            Err(error) => {
                log::warn!("global close runtime startup failed: {error}");
                return startup_diagnostics::ShellRunOutcome::Exit;
            }
        };
    let global_close_client = global_close.client();
    let open_urls_tx = shell_tx.clone();
    // 通知通道开机位:toast 点击回投走同一壳事件队列;
    // AUMID 预注册幂等,必须早于第一条通知。
    crate::notify::init_gpui_activation(shell_tx.clone());
    crate::platform::notifications::prepare();
    // 通知与设置页共用同一 Windows 集成端口：任务栏进度只经端口落平台。
    crate::notify::install_windows_integration(windows.clone());
    // 设置域自愈：按授权安装/修复 CLI hooks，失败类别经同一壳事件队列回前台。
    let hook_alert_tx = shell_tx.clone();
    let _config_guard = crate::ai_hook::spawn_config_guard_with(move |kind| {
        try_send_shell_event(
            &hook_alert_tx,
            SltermShellEvent::AiHookAlert(kind),
            "ai-hook-guard",
        );
    });
    // 命名管道是 hook 事件唯一权威通道：服务线程只做阻塞 I/O，泵线程把有界
    // 批次投进壳事件队列，pane 所有权校验与仲裁都在 GPUI 前台完成。
    let (hook_rx, pipe_server) = crate::ai_hook::spawn_gpui_server();
    let hook_events_tx = shell_tx.clone();
    let hook_pump = std::thread::Builder::new()
        .name("slterm-ai-hook-pump".into())
        .spawn(move || {
            while let Ok(first) = hook_rx.recv() {
                let mut batch = vec![first];
                while batch.len() < 64 {
                    match hook_rx.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(_) => break,
                    }
                }
                match hook_events_tx.try_send(SltermShellEvent::AiHookEvents(batch)) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        log::warn!("ai-hook event queue full; dropping a hook batch")
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
        })
        .map_err(|error| log::warn!("ai_hook: failed to spawn event pump: {error}"))
        .ok();
    let mut pipe_server = Some(pipe_server);
    let mut hook_pump = hook_pump;
    // 托盘命令回调:托盘线程只经此把用户动作送回壳事件队列。
    crate::tray::init_gpui({
        let shell_events = shell_tx.clone();
        move |command| {
            try_send_shell_event(&shell_events, tray_command_event(command), "tray");
        }
    });
    let application = gpui_platform::application()
        .with_assets(assets::SltermAssets)
        .with_quit_mode(QuitMode::LastWindowClosed);
    application.on_open_urls(move |urls| {
        try_send_shell_event(
            &open_urls_tx,
            SltermShellEvent::OpenDirectories(urls),
            "open-urls",
        );
    });
    application.run(move |cx| {
        init(cx, &startup_settings, settings_warning);
        // 更新检查结果只经壳事件回前台；检查与延迟都在后台执行体。
        let update_events = shell_tx.clone();
        crate::update_check::start_gpui(cx, move |result| {
            try_send_shell_event(
                &update_events,
                SltermShellEvent::UpdateAvailable(result),
                "update-check",
            );
        });
        // 静默启动：只有登录自启标记参与；窗口以隐藏形态创建，
        // 托盘「显示」经 focus facade 回窗。
        let start_hidden =
            starts_hidden(autostart, cx.global::<config::Settings>(), windows.as_ref());
        windowing::open_initial_window(
            cx,
            windowing::InitialWindowSpec {
                shell_rx,
                shell_events: shell_tx,
                initial_root,
                shell,
                services: app_root::AppRootServices {
                    persistence: side_view_persistence,
                    global_close: global_close_client,
                    learning: learning.clone(),
                    runtime_events,
                    windows: windows.clone(),
                    snapshot_client,
                    snapshot_run_id,
                },
                window_geometry,
                start_hidden,
                regular_start,
            },
        );
    });
    // 窗口结束后先停管道服务（阻塞 accept/read 线程），接收端随之关闭，
    // 再 join 泵线程；不留孤儿线程。
    drop(pipe_server.take());
    if let Some(handle) = hook_pump.take() {
        let _ = handle.join();
    }
    // 正常路径已在窗口销毁前完成全局段；异常返回由断连兜底回收。
    match global_close.join() {
        Ok(global_close::GlobalCloseSummary::Completed) => {}
        Ok(global_close::GlobalCloseSummary::Abandoned) => {
            log::warn!("user abandoned final workspace state commit after explicit warning")
        }
        Ok(global_close::GlobalCloseSummary::Failed) => {
            log::warn!("global workspace shutdown failed")
        }
        Err(_) => log::warn!("global close worker terminated abnormally"),
    }
    // 学习写线程由 GPUI 外启动作用域独占；窗口结束后仍显式排空并 join。
    if let Some(runtime) = learning_runtime
        && let Err(error) = runtime.shutdown()
    {
        log::warn!("learning writer shutdown failed: {error}");
    }
    // 崩溃快照最终冲刷：窗口销毁后仍保留待写正文，失败保留给下次启动恢复。
    if let Some(mut writer) = snapshot_writer.take()
        && let Err(error) = writer.shutdown()
    {
        log::warn!("document snapshot writer shutdown failed: {error}");
    }
    // 托盘收尾:先删图标再退进程,否则任务栏残影要等
    // 用户悬停才消失;join 保证删图标先于进程退出。
    crate::tray::shutdown();
    startup_diagnostics::ShellRunOutcome::Exit
}

/// 组件库/内嵌字体/主题/终端切片/工作区的一次性初始化。
///
/// `settings` 是在 GPUI 事件循环外读取并解析完成的启动快照；本函数只做纯
/// 内存装配，不读文件或环境。
fn init(
    cx: &mut App,
    settings: &config::Settings,
    settings_warning: Option<crate::settings_state::SettingsLoadWarning>,
) {
    register_bundled_fonts(cx);
    // 组件库必须只初始化一次,否则全局 action、菜单和主题状态重复注册。
    gpui_component::init(cx);
    cx.set_global(crate::settings_state::SettingsLoadState {
        warning: settings_warning,
    });
    // 组件库 locale 与自有文案分层:组件内置文案跟解析后的 UiLanguage。
    gpui_component::set_locale(settings.language.gpui_component_locale());
    // 通知投递不读盘:语言只随壳快照热应用。
    crate::notify::set_language(settings.language);
    cx.set_global(settings.clone());
    theme::apply_dark_theme(cx);
    text_view::init(cx);
    markdown::init(cx);
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
    use slterm_runtime_protocol::{
        DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError,
    };
    use slterm_settings::{RawSettings, RuntimeSettings};
    use slterm_workspace::{
        CloseIntent, CloseScope, InvalidViewState, ProjectId, ProjectSnapshot, SideViewId, ViewKey,
        ViewKeyKind, ViewState, ViewStateRegistry, ViewStateSchema,
    };

    use super::{
        ControlPlaneCleanup, FinalCommitDecision, FinalShutdownOutcome, SltermShellEvent,
        WorkspaceWriterRuntime, shutdown_local_control_service, start_local_control_service,
        start_workspace_writer, starts_hidden, tray_command_event, try_send_handover,
    };
    use crate::persistence::close::{CloseCommitStatus, CloseStoreError, PersistedCloseIntent};
    use crate::persistence::database::DatabaseError;
    use crate::persistence::journal::{
        JournalRegistration, JournalStatus, JournalStepRegistration,
    };
    use crate::persistence::metadata::RunMarker;
    use crate::persistence::startup::{MaintenanceError, MaintenanceStage};
    use crate::persistence::workspace::WorkspaceDatabase;
    use crate::persistence::workspace_writer::{WorkspaceWriter, WorkspaceWriterError};
    use crate::platform::windows_integration::FakeWindowsIntegration;
    use crate::tray::GpuiTrayCommand;

    fn silent_settings() -> crate::app_shell::config::Settings {
        let runtime = RuntimeSettings::from_raw(&RawSettings::from_json_bytes(
            br#"{"session":{"silent_start":true},"system":{"tray":true}}"#,
        ));
        crate::app_shell::config::Settings::from_runtime(&runtime)
    }

    /// 手动启动不带登录自启标记，即使设置两条件成立也不隐藏；
    /// 只有安装器／启动项写入 `--autostart` 才进入静默分支。
    #[test]
    fn manual_launch_never_starts_hidden() {
        let settings = silent_settings();
        let windows = FakeWindowsIntegration::default();
        assert!(starts_hidden(true, &settings, &windows), "登录自启可静默");
        assert!(
            !starts_hidden(false, &settings, &windows),
            "手工启动不得隐藏"
        );
        assert!(windows.tray_enabled(), "启动路径必须应用托盘设置");
    }

    /// 托盘不可用时静默驻留让位:隐藏窗口没有恢复入口。
    #[test]
    fn silent_start_falls_back_to_a_visible_window_without_a_usable_tray() {
        let settings = silent_settings();
        let windows = FakeWindowsIntegration::default();
        windows.set_tray_available(false);
        assert!(!starts_hidden(true, &settings, &windows));
        assert!(windows.tray_enabled(), "托盘设置仍须交给端口");
    }

    #[test]
    fn shell_event_channel_is_bounded() {
        let (events, _event_rx) = super::shell_event_channel();
        for _ in 0..super::SHELL_EVENT_CAPACITY {
            events.try_send(SltermShellEvent::TrayQuit).unwrap();
        }
        assert!(matches!(
            events.try_send(SltermShellEvent::TrayQuit),
            Err(mpsc::TrySendError::Full(_))
        ));
    }

    fn assert_handover_error(error: &JsonRpcError, numeric: i32, domain: DomainErrorCode) {
        assert_eq!(error.code, numeric);
        assert_eq!(error.data.as_ref().map(|data| data.code), Some(domain));
        assert!(!error.message.contains("D:/private"));
        assert!(!error.message.contains("sk-test"));
    }

    #[test]
    fn handover_helper_returns_bounded_completion_receiver() {
        let (events, event_rx) = super::shell_event_channel();

        let response = try_send_handover(&events, HandoverParams::Focus).unwrap();

        let SltermShellEvent::Handover { params, completion } = event_rx.try_recv().unwrap() else {
            panic!("helper 必须投递 handover 事件");
        };
        assert_eq!(params, HandoverParams::Focus);
        completion
            .try_send(Ok(HandoverResponse::accepted()))
            .unwrap();
        assert!(matches!(
            completion.try_send(Ok(HandoverResponse::accepted())),
            Err(mpsc::TrySendError::Full(_))
        ));
        assert!(response.try_recv().unwrap().unwrap().is_accepted());
    }

    #[test]
    fn handover_helper_maps_full_queue_without_echoing_params() {
        let (events, _event_rx) = super::shell_event_channel();
        for _ in 0..super::SHELL_EVENT_CAPACITY {
            events.try_send(SltermShellEvent::TrayQuit).unwrap();
        }

        let error = try_send_handover(
            &events,
            HandoverParams::OpenProject {
                project_root: "D:/private/sk-test/project".to_owned(),
            },
        )
        .unwrap_err();

        assert_handover_error(&error, -32003, DomainErrorCode::QueueFull);
    }

    #[test]
    fn handover_helper_maps_disconnected_queue_to_closing() {
        let (events, event_rx) = super::shell_event_channel();
        drop(event_rx);

        let error = try_send_handover(
            &events,
            HandoverParams::OpenFile {
                path: "D:/private/sk-test/secret.txt".to_owned(),
            },
        )
        .unwrap_err();

        assert_handover_error(&error, -32004, DomainErrorCode::Closing);
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

    pub(super) fn seed_maintenance_run(store: &mut WorkspaceDatabase) {
        store
            .begin_run(
                "epoch-maintenance",
                RunMarker {
                    run_id: "run-maintenance-seed".into(),
                    process_id: 1,
                    process_created_at_ms: 1,
                },
            )
            .unwrap();
    }

    fn assert_forwarder_sender_released(event_rx: &mpsc::Receiver<SltermShellEvent>) {
        // 通道断开只证明 forwarder 发送端释放，不证明 writer 线程已回收。
        assert!(matches!(
            event_rx.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn workspace_isolation_emits_explicit_startup_notice() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        std::fs::create_dir_all(paths.local_root()).unwrap();
        std::fs::write(paths.workspace_db_path(), b"corrupt-main").unwrap();
        std::fs::write(paths.workspace_disaster_copy_path(), b"corrupt-backup").unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();

        assert!(matches!(
            event_rx.recv_timeout(Duration::from_secs(3)),
            Ok(SltermShellEvent::WorkspaceRecovered(
                crate::persistence::startup::WorkspaceRecoveryNotice::Isolated
            ))
        ));
        runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    }

    /// 未完成 Journal 必须在 writer 与转发器取得所有权前阻断，且不改写原材料。
    #[test]
    fn workspace_writer_maintenance_journal_blocks_before_thread_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        seed_maintenance_run(&mut store);
        let registration = JournalRegistration {
            operation_id: None,
            journal_id: "journal-maintenance".into(),
            kind: "unknown.maintenance".into(),
            store_epoch: "epoch-maintenance".into(),
            before_fingerprint: Some([1; 32]),
            steps: vec![JournalStepRegistration {
                step_key: "step-maintenance".into(),
                kind: "replace".into(),
                target: json!({"version": 1}),
                authorization_generation: 1,
                before_fingerprint: Some([2; 32]),
            }],
        };
        let _ = store.register_journal(registration).unwrap();
        let _ = store
            .transition_journal("journal-maintenance", JournalStatus::NeedsAttention, None)
            .unwrap();
        let before = store.journal("journal-maintenance").unwrap();
        drop(store);
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let error = match WorkspaceWriterRuntime::start_at(&paths, events) {
            Err(error) => error,
            Ok(_) => panic!("维护未通过时不得取得 writer runtime"),
        };

        assert!(
            matches!(
                &error,
                WorkspaceWriterError::MaintenanceNeedsAttention(MaintenanceStage::Journal)
            ),
            "实际错误: {error:?}"
        );
        assert_forwarder_sender_released(&event_rx);
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        assert!(!observer.metadata().unwrap().unwrap().clean_shutdown);
        assert_eq!(observer.journal("journal-maintenance").unwrap(), before);
    }

    /// 未完成关闭意图必须经封闭阶段错误阻断，运行标记保持未正常退出。
    #[test]
    fn workspace_writer_maintenance_close_intent_blocks_without_rewrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        seed_maintenance_run(&mut store);
        let close = PersistedCloseIntent {
            close_id: "close-maintenance".into(),
            scope: CloseScope::ApplicationExit,
            store_epoch: "epoch-maintenance".into(),
            intent: CloseIntent::new(Vec::new(), [4; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        };
        store.register_close_intent(&close).unwrap();
        let before = store.load_close_intent("close-maintenance").unwrap();
        drop(store);
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let error = match WorkspaceWriterRuntime::start_at(&paths, events) {
            Err(error) => error,
            Ok(_) => panic!("维护未通过时不得取得 writer runtime"),
        };

        assert!(
            matches!(
                &error,
                WorkspaceWriterError::MaintenanceNeedsAttention(MaintenanceStage::CloseIntents)
            ),
            "实际错误: {error:?}"
        );
        assert_forwarder_sender_released(&event_rx);
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        assert!(!observer.metadata().unwrap().unwrap().clean_shutdown);
        assert_eq!(
            observer.load_close_intent("close-maintenance").unwrap(),
            before
        );
    }

    /// 维护执行错误保持独立分类与脱敏展示，不创建可取得的 writer。
    #[test]
    fn workspace_writer_maintenance_error_is_fail_closed_and_redacted() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        seed_maintenance_run(&mut store);
        let close = PersistedCloseIntent {
            close_id: "close-maintenance-error".into(),
            scope: CloseScope::ApplicationExit,
            store_epoch: "epoch-maintenance".into(),
            intent: CloseIntent::new(Vec::new(), [5; 32]).unwrap(),
            commit_status: CloseCommitStatus::Collected,
        };
        store.register_close_intent(&close).unwrap();
        drop(store);
        let marker = "sensitive_maintenance_sql_marker";
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute(
                "UPDATE close_intents SET commit_status = ?1 WHERE close_id = ?2",
                rusqlite::params![marker, "close-maintenance-error"],
            )
            .unwrap();
        drop(fault);
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let error = match WorkspaceWriterRuntime::start_at(&paths, events) {
            Err(error) => error,
            Ok(_) => panic!("维护未通过时不得取得 writer runtime"),
        };

        assert!(
            matches!(
                &error,
                WorkspaceWriterError::Maintenance(MaintenanceError::Close(
                    CloseStoreError::CorruptRecord
                ))
            ),
            "实际错误: {error:?}"
        );
        assert!(!error.to_string().contains(marker));
        assert_forwarder_sender_released(&event_rx);
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        assert!(!observer.metadata().unwrap().unwrap().clean_shutdown);
    }

    /// 维护 Ready 后的真实 writer 打开失败必须上抛，并保留本次未正常退出标记。
    #[test]
    fn workspace_writer_ready_writer_start_failure_preserves_unclean_run() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let cleanup_called = std::cell::Cell::new(false);
        let error = match WorkspaceWriterRuntime::start_with(
            &paths,
            events,
            |writer_paths| {
                // 故障在维护完成后注入，真实 WorkspaceWriter::start 打开时才发现结构损坏。
                let fault = rusqlite::Connection::open(writer_paths.workspace_db_path()).unwrap();
                fault
                    .execute("DROP TABLE maintenance_requests", [])
                    .unwrap();
                drop(fault);
                WorkspaceWriter::start(writer_paths)
            },
            |_, _| -> std::io::Result<std::thread::JoinHandle<()>> {
                panic!("writer 启动失败不得创建 forwarder")
            },
            || {
                cleanup_called.set(true);
                Err(std::io::Error::other(
                    "injected expired temp cleanup failure",
                ))
            },
        ) {
            Err(error) => error,
            Ok(_) => panic!("writer 启动失败不得取得 runtime"),
        };

        assert!(
            matches!(
                &error,
                WorkspaceWriterError::Open(DatabaseError::IncompatibleSchema)
            ),
            "实际错误: {error:?}"
        );
        assert!(cleanup_called.get());
        let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        let clean_shutdown = raw
            .query_row(
                "SELECT clean_shutdown FROM workspace_meta WHERE singleton = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(clean_shutdown, 0);
    }

    /// forwarder 启动失败必须在错误返回前 shutdown 并 join 已启动的 writer。
    #[test]
    fn workspace_writer_forwarder_spawn_failure_joins_started_writer() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let (join_probe, joined) = mpsc::sync_channel(1);

        let error = match WorkspaceWriterRuntime::start_with(
            &paths,
            events,
            move |writer_paths| {
                let mut writer = WorkspaceWriter::start(writer_paths)?;
                writer.set_join_completion_probe(join_probe);
                Ok(writer)
            },
            |_, _| Err(std::io::Error::other("injected forwarder spawn failure")),
            || Ok(()),
        ) {
            Err(error) => error,
            Ok(_) => panic!("forwarder 启动失败不得取得 runtime"),
        };

        assert!(
            matches!(&error, WorkspaceWriterError::Spawn(_)),
            "实际错误: {error:?}"
        );
        assert_eq!(
            joined.try_recv(),
            Ok(()),
            "错误返回前必须完成 writer shutdown 与 JoinHandle::join"
        );
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        assert!(!observer.metadata().unwrap().unwrap().clean_shutdown);
    }

    /// 启动失败返回错误，不降级为无 client 的恢复路径。
    #[test]
    fn workspace_writer_start_failure_is_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        std::fs::write(&local, "not a directory").unwrap();
        let paths = AppPaths::new(temp.path().join("config"), local).unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        let result = start_workspace_writer(&paths, events);

        assert!(result.is_err(), "启动失败必须向调用方返回错误");
        assert_forwarder_sender_released(&event_rx);
    }

    /// writer 不可用时不存在真实运行元数据，控制面必须保持关闭。
    #[test]
    fn local_control_startup_requires_workspace_writer() {
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);

        assert!(start_local_control_service(None, events).is_none());
    }

    /// 未启动服务不产生控制面清理负担，writer 可沿用正常退出语义。
    #[test]
    fn absent_local_control_cleanup_is_successful() {
        assert_eq!(
            shutdown_local_control_service(None),
            ControlPlaneCleanup::Succeeded
        );
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
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
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
        runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
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
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
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
        runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
        assert_eq!(
            observer.load_project_snapshot().unwrap(),
            project_snapshot("changed")
        );
    }

    /// 最终冲刷成功后才登记正常退出，且项目快照使用同一运行标记。
    #[test]
    fn workspace_writer_runtime_marks_clean_shutdown_after_flush() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        let started = observer.metadata().unwrap();
        assert!(started.is_some(), "启动必须登记运行标记");
        let started = started.unwrap();
        assert!(!started.clean_shutdown);

        client
            .save_project_snapshot(&project_snapshot("changed"))
            .unwrap();
        runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

        let finished = observer.metadata().unwrap().unwrap();
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(finished.clean_shutdown);
        assert_eq!(
            observer.load_project_snapshot().unwrap(),
            project_snapshot("changed")
        );
    }

    /// Runtime 暴露的身份必须与 begin_run 已提交的同一份元数据完全一致。
    #[test]
    fn workspace_writer_runtime_exposes_current_begin_run_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();

        let stored = observer.metadata().unwrap().unwrap();
        assert_eq!(runtime.metadata(), &stored);
        assert!(!runtime.metadata().clean_shutdown);
        runtime.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    }

    /// 控制面清理失败不得跳过项目与侧栏终态冲刷、转发器 join 或运行标记闸门。
    #[test]
    fn workspace_writer_runtime_control_plane_failure_flushes_without_clean_mark() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let started = observer_metadata(&paths);
        let snapshot = project_snapshot("changed");
        let mut sidebar = slterm_sidebar::SidebarState::default();
        sidebar.set_width(333);
        client.save_final_project_snapshot(&snapshot).unwrap();
        client.save_final_sidebar_state(&sidebar).unwrap();

        let result = runtime.shutdown(ControlPlaneCleanup::Failed);

        assert!(
            matches!(result, Err(WorkspaceWriterError::ControlPlaneCleanupFailed)),
            "控制面失败必须使整体关闭结果失败"
        );
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(observer.load_project_snapshot().unwrap(), snapshot);
        assert_eq!(
            observer.sidebar_state().unwrap().unwrap().state,
            sidebar,
            "控制面失败不得跳过侧栏终态冲刷"
        );
        let finished = observer.metadata().unwrap().unwrap();
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown);
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "失败转发线程必须完成 join");
    }

    /// 控制面失败不得掩盖最终冲刷失败；故障在 writer Drop 后仍保持有效。
    #[test]
    fn workspace_writer_runtime_flush_failure_precedes_control_plane_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer
            .save_project_snapshot(&project_snapshot("original"))
            .unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_project_write BEFORE UPDATE ON projects \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();
        runtime
            .client()
            .save_final_project_snapshot(&project_snapshot("changed"))
            .unwrap();

        let result = runtime.shutdown(ControlPlaneCleanup::Failed);

        assert!(
            matches!(result, Err(WorkspaceWriterError::FlushFailed)),
            "最终冲刷失败必须优先于控制面清理失败: {result:?}"
        );
        assert_eq!(
            observer.load_project_snapshot().unwrap(),
            project_snapshot("original"),
            "故障保持期间 Drop 不得成功冲刷并掩盖提前返回"
        );
        let finished = observer.metadata().unwrap().unwrap();
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown);
        let trigger_count = fault
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'trigger' AND name = 'reject_project_write'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(trigger_count, 1, "断言前故障必须仍然保留");
    }

    /// 闩门释放前 shutdown 必须等待真实 forwarder join，之后才能进入清理闸门。
    #[test]
    fn workspace_writer_runtime_shutdown_waits_for_forwarder_before_control_plane_gate() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let mut runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let (forwarder_started_tx, forwarder_started_rx) = mpsc::sync_channel(1);
        let (release_forwarder_tx, release_forwarder_rx) = mpsc::sync_channel(1);
        let (forwarder_finished_tx, forwarder_finished_rx) = mpsc::sync_channel(1);
        let gated_forwarder = std::thread::spawn(move || {
            forwarder_started_tx.send(()).unwrap();
            release_forwarder_rx.recv().unwrap();
            forwarder_finished_tx.send(()).unwrap();
        });
        let original_forwarder = std::mem::replace(&mut runtime.failure_forwarder, gated_forwarder);
        forwarder_started_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("闩门 forwarder 必须启动");
        let (shutdown_done_tx, shutdown_done_rx) = mpsc::sync_channel(1);
        let shutdown = std::thread::spawn(move || {
            let result = runtime.shutdown(ControlPlaneCleanup::Failed);
            shutdown_done_tx.send(result).unwrap();
        });

        let premature = shutdown_done_rx.recv_timeout(Duration::from_millis(100));
        let waited_for_release = matches!(premature, Err(mpsc::RecvTimeoutError::Timeout));
        assert!(forwarder_finished_rx.try_recv().is_err());
        release_forwarder_tx.send(()).unwrap();
        forwarder_finished_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("释放后闩门 forwarder 必须完成");
        let result = match premature {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => shutdown_done_rx
                .recv_timeout(Duration::from_secs(3))
                .expect("释放后 shutdown 必须完成"),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("shutdown 完成通道不得提前断开")
            }
        };
        shutdown.join().unwrap();
        original_forwarder
            .join()
            .expect("writer 关闭失败通道后原 forwarder 必须自然回收");

        assert!(waited_for_release, "闩门释放前 shutdown 不得返回");
        assert!(
            matches!(result, Err(WorkspaceWriterError::ControlPlaneCleanupFailed)),
            "join 后必须继续进入控制面清理闸门: {result:?}"
        );
        let finished = observer_metadata(&paths);
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown);
    }

    /// 领域恢复失败必须关闭 writer 并等待转发线程，不得留下孤儿或伪造正常退出。
    #[test]
    fn abort_startup_stops_writer_and_joins_forwarder() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let mut runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let (forwarder_started_tx, forwarder_started_rx) = mpsc::sync_channel(1);
        let (release_forwarder_tx, release_forwarder_rx) = mpsc::sync_channel(1);
        let (forwarder_finished_tx, forwarder_finished_rx) = mpsc::sync_channel(1);
        let gated_forwarder = std::thread::spawn(move || {
            forwarder_started_tx.send(()).unwrap();
            release_forwarder_rx.recv().unwrap();
            forwarder_finished_tx.send(()).unwrap();
        });
        let original_forwarder = std::mem::replace(&mut runtime.failure_forwarder, gated_forwarder);
        forwarder_started_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("闩门 forwarder 必须启动");
        let (abort_done_tx, abort_done_rx) = mpsc::sync_channel(1);
        let abort = std::thread::spawn(move || {
            runtime.abort_startup();
            abort_done_tx.send(()).unwrap();
        });

        let premature = abort_done_rx.recv_timeout(Duration::from_millis(100));
        assert!(
            matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)),
            "释放前 abort 不得返回"
        );
        release_forwarder_tx.send(()).unwrap();
        forwarder_finished_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("释放后闩门 forwarder 必须完成");
        abort_done_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("abort 必须完成");
        abort.join().unwrap();
        original_forwarder
            .join()
            .expect("writer 关闭失败通道后原 forwarder 必须自然回收");

        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let finished = observer.metadata().unwrap().unwrap();
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown, "领域恢复失败不得写正常退出");
    }

    /// 每次启动必须保留上一运行退出事实，且不得改变既有工作区世代。
    #[test]
    fn workspace_writer_runtime_preserves_previous_run_and_store_epoch() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let observer = WorkspaceDatabase::open(&paths).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let first = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let first_metadata = observer.metadata().unwrap().unwrap();
        let epoch = first_metadata.store_epoch.clone();
        first.shutdown(ControlPlaneCleanup::Succeeded).unwrap();

        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let second = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let previous = second.previous_run();
        assert!(previous.is_some(), "必须返回上一运行事实");
        let previous = previous.unwrap();
        assert_eq!(previous.run.run_id, first_metadata.run.run_id);
        assert!(previous.clean_shutdown);
        assert_eq!(observer.metadata().unwrap().unwrap().store_epoch, epoch);
        second.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
        let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        raw.execute_batch("UPDATE workspace_meta SET clean_shutdown = 0")
            .unwrap();

        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let third = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let previous = third.previous_run();
        assert!(previous.is_some(), "异常上一运行事实同样必须保留");
        assert!(!previous.unwrap().clean_shutdown);
        assert_eq!(observer.metadata().unwrap().unwrap().store_epoch, epoch);
        third.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    }

    /// 启动失败后，下一次启动仍必须能取得上一运行退出事实。
    #[test]
    fn workspace_writer_failed_start_preserves_previous_run() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let first = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let first_metadata = observer_metadata(&paths);
        first.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
        let raw = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        raw.execute_batch("PRAGMA user_version = 3").unwrap();

        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let failed = WorkspaceWriterRuntime::start_at(&paths, events);
        let error = failed.as_ref().err();
        assert!(
            matches!(error, Some(WorkspaceWriterError::Metadata(_)),),
            "实际错误: {error:?}"
        );
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "启动失败必须回收转发线程");
        raw.execute_batch("PRAGMA user_version = 2").unwrap();

        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let next = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let previous = next.previous_run();
        assert!(previous.is_some());
        assert_eq!(previous.unwrap().run.run_id, first_metadata.run.run_id);
        assert!(previous.unwrap().clean_shutdown);
        next.shutdown(ControlPlaneCleanup::Succeeded).unwrap();
    }

    /// 转发器 panic 不得把关闭登记为正常退出。
    #[test]
    fn workspace_writer_runtime_forwarder_panic_keeps_run_unclean() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let mut runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        runtime.failure_forwarder = std::thread::spawn(|| panic!("injected forwarder panic"));

        let result = runtime.shutdown(ControlPlaneCleanup::Succeeded);

        assert!(matches!(result, Err(WorkspaceWriterError::Stopped)));
        let finished = observer_metadata(&paths);
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown);
    }

    /// 正常退出标记失败必须保留 unclean 并返回元数据错误。
    #[test]
    fn workspace_writer_runtime_mark_failure_is_reported() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_clean_shutdown BEFORE UPDATE ON workspace_meta \
                 BEGIN SELECT RAISE(ABORT, 'injected metadata failure'); END;",
            )
            .unwrap();

        let result = runtime.shutdown(ControlPlaneCleanup::Succeeded);

        assert!(
            matches!(
                result,
                Err(WorkspaceWriterError::Open(
                    DatabaseError::IncompatibleSchema
                ))
            ),
            "实际错误: {result:?}"
        );
        let (run_id, clean_shutdown) = fault
            .query_row(
                "SELECT run_id, clean_shutdown FROM workspace_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(run_id, started.run.run_id);
        assert_eq!(clean_shutdown, 0);
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "失败转发线程必须完成 join");
    }

    /// 用户撤除故障后，重试必须完成最终正常退出提交。
    #[test]
    fn final_commit_failure_retries_after_user_confirmation() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_clean_shutdown BEFORE UPDATE ON workspace_meta \
                 BEGIN SELECT RAISE(ABORT, 'injected metadata failure'); END;",
            )
            .unwrap();
        let mut decisions = 0;

        let outcome = runtime
            .shutdown_with_decision(ControlPlaneCleanup::Succeeded, || {
                decisions += 1;
                fault
                    .execute_batch("DROP TRIGGER reject_clean_shutdown")
                    .unwrap();
                FinalCommitDecision::Retry
            })
            .unwrap();

        assert_eq!(outcome, FinalShutdownOutcome::Completed);
        assert_eq!(decisions, 1);
        assert!(observer_metadata(&paths).clean_shutdown);
    }

    /// 用户明确承担恢复后果后，允许不提交最终状态而退出。
    #[test]
    fn close_final_commit_failure_allows_explicit_abandon() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let fault = rusqlite::Connection::open(paths.workspace_db_path()).unwrap();
        fault
            .execute_batch(
                "CREATE TRIGGER reject_clean_shutdown BEFORE UPDATE ON workspace_meta \
                 BEGIN SELECT RAISE(ABORT, 'injected metadata failure'); END;",
            )
            .unwrap();

        let outcome = runtime
            .shutdown_with_decision(ControlPlaneCleanup::Succeeded, || {
                FinalCommitDecision::Abandon
            })
            .unwrap();

        assert_eq!(outcome, FinalShutdownOutcome::Abandoned);
        fault
            .execute_batch("DROP TRIGGER reject_clean_shutdown")
            .unwrap();
        let finished = observer_metadata(&paths);
        assert_eq!(finished.run.run_id, started.run.run_id);
        assert!(!finished.clean_shutdown);
    }

    /// 数据库可打开但不可写时，真正的正常退出更新必须失败且保持 unclean。
    #[test]
    fn workspace_writer_runtime_readonly_mark_is_reported() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let started = observer_metadata(&paths);
        let database = paths.workspace_db_path();
        let mut permissions = std::fs::metadata(&database).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&database, permissions.clone()).unwrap();

        let result = runtime.shutdown(ControlPlaneCleanup::Succeeded);

        #[allow(clippy::permissions_set_readonly_false)]
        // 仅恢复本用例临时数据库的写权限，避免影响后续清理。
        permissions.set_readonly(false);
        std::fs::set_permissions(&database, permissions).unwrap();
        assert!(matches!(result, Err(WorkspaceWriterError::Metadata(_))));
        let raw = rusqlite::Connection::open(&database).unwrap();
        let (run_id, clean_shutdown) = raw
            .query_row(
                "SELECT run_id, clean_shutdown FROM workspace_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(run_id, started.run.run_id);
        assert_eq!(clean_shutdown, 0);
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "失败转发线程必须完成 join");
    }

    fn observer_metadata(paths: &AppPaths) -> crate::persistence::metadata::WorkspaceMetadata {
        WorkspaceDatabase::open(paths)
            .unwrap()
            .metadata()
            .unwrap()
            .unwrap()
    }

    /// 最终冲刷失败不得把本次运行登记为正常退出。
    #[test]
    fn workspace_writer_runtime_failed_flush_keeps_run_unclean() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut observer = WorkspaceDatabase::open(&paths).unwrap();
        observer
            .save_project_snapshot(&project_snapshot("original"))
            .unwrap();
        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let runtime = WorkspaceWriterRuntime::start_at(&paths, events).unwrap();
        let client = runtime.client();
        let started = observer.metadata().unwrap();
        assert!(started.is_some(), "启动必须登记运行标记");
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

        let result = runtime.shutdown(ControlPlaneCleanup::Succeeded);
        assert!(matches!(result, Err(WorkspaceWriterError::FlushFailed)));

        let finished = observer.metadata().unwrap().unwrap();
        assert_eq!(finished.run.run_id, started.unwrap().run.run_id);
        assert!(!finished.clean_shutdown);
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "失败转发线程必须完成 join");
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
        let (events, _event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
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
            runtime.shutdown(ControlPlaneCleanup::Succeeded),
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

#[cfg(test)]
mod startup_shell_tests {
    use slterm_settings::json::Value;

    use super::{frozen_startup_paths, startup_root};

    /// 启动目录设置只在无 CLI 根且目录真实存在时生效。
    #[test]
    fn startup_directory_overrides_only_without_cli_root_and_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let configured = startup_root(None, Some(temp.path())).unwrap();
        assert_eq!(configured, temp.path(), "有效启动目录必须成为初始根");

        let missing = temp.path().join("missing");
        let fallback = startup_root(None, Some(&missing)).unwrap();
        assert_eq!(
            fallback,
            std::env::current_dir().unwrap(),
            "失效启动目录必须回退当前目录"
        );

        let cli = temp.path().join("cli");
        let chosen = startup_root(Some(cli.clone()), Some(temp.path())).unwrap();
        assert_eq!(chosen, cli, "CLI 指定根优先于设置");
    }

    /// 启动 Shell 跟随设置；无效设置回退默认探测链（由 resolve 自身失败路径覆盖）。
    #[test]
    fn startup_shell_follows_settings_and_restores_original_file() {
        let settings_path = crate::platform::dirs::app_paths().settings_path();
        let original = std::fs::read(&settings_path).ok();
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        let powershell = format!(r"{system_root}\System32\WindowsPowerShell\v1.0\powershell.exe");
        assert!(
            std::path::Path::new(&powershell).is_file(),
            "测试依赖真实系统 PowerShell: {powershell}"
        );
        crate::settings_store::save_settings_blocking(&[(
            "shell",
            Value::String(powershell.clone()),
        )])
        .unwrap();

        let (settings, _warning) = super::config::Settings::load_with_warning();
        let (shell, _root) = frozen_startup_paths(None, &settings).unwrap();
        assert_eq!(
            shell.as_path().file_name().and_then(|name| name.to_str()),
            Some("powershell.exe"),
            "启动 Shell 必须采用设置中的真实程序"
        );

        match original {
            Some(bytes) => std::fs::write(&settings_path, bytes).unwrap(),
            None => {
                let _ = std::fs::remove_file(&settings_path);
            }
        }
    }
}

#[cfg(test)]
mod hooks_journal_tests;
