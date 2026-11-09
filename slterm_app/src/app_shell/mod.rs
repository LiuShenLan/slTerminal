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
mod global_close;
mod startup_diagnostics;
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
use slterm_runtime_protocol::{
    DomainErrorCode, HandoverParams, HandoverResponse, JsonRpcError, JsonRpcErrorData,
};

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
    /// 第二实例意图与一次性完成回执；AppRoot 前台消费并同步答复。
    Handover {
        params: HandoverParams,
        completion: mpsc::SyncSender<Result<HandoverResponse, JsonRpcError>>,
    },
}

const SHELL_EVENT_CAPACITY: usize = 64;

fn shell_event_channel() -> (
    mpsc::SyncSender<SltermShellEvent>,
    mpsc::Receiver<SltermShellEvent>,
) {
    mpsc::sync_channel(SHELL_EVENT_CAPACITY)
}

pub(crate) fn try_send_shell_event(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    event: SltermShellEvent,
    queue: &'static str,
) {
    match sender.try_send(event) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(_)) => log::warn!("{queue} event queue full"),
        Err(mpsc::TrySendError::Disconnected(_)) => log::debug!("{queue} event channel closed"),
    }
}

fn handover_error(domain: DomainErrorCode, numeric: i32, message: &'static str) -> JsonRpcError {
    JsonRpcError::new(numeric, message, Some(JsonRpcErrorData { code: domain }))
}

/// 非阻塞投递第二实例意图；调用方只在后台执行体等待完成回执。
pub(crate) fn try_send_handover(
    sender: &mpsc::SyncSender<SltermShellEvent>,
    params: HandoverParams,
) -> Result<mpsc::Receiver<Result<HandoverResponse, JsonRpcError>>, JsonRpcError> {
    let (completion, response) = mpsc::sync_channel(1);
    match sender.try_send(SltermShellEvent::Handover { params, completion }) {
        Ok(()) => Ok(response),
        Err(mpsc::TrySendError::Full(_)) => {
            log::warn!("handover event queue full");
            Err(handover_error(
                DomainErrorCode::QueueFull,
                -32003,
                "handover queue full",
            ))
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {
            log::debug!("handover event channel closed");
            Err(handover_error(DomainErrorCode::Closing, -32004, "closing"))
        }
    }
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

        Self::start_with(paths, events, WorkspaceWriter::start, |failures, notify| {
            spawn_workspace_failure_forwarder(failures, notify)
        })
    }

    fn start_with<StartWriter, SpawnForwarder>(
        paths: &slterm_path::AppPaths,
        events: mpsc::SyncSender<SltermShellEvent>,
        start_writer: StartWriter,
        spawn_forwarder: SpawnForwarder,
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
    {
        use crate::persistence::startup::{
            JournalRecovererRegistry, MaintenanceOutcome, MaintenanceRunner,
        };
        use crate::persistence::workspace_writer::WorkspaceWriterError;

        // 维护通过前只持有当前数据库连接；writer、转发器与领域后台任务均未创建。
        let mut store = crate::persistence::workspace::WorkspaceDatabase::open(paths)
            .map_err(WorkspaceWriterError::Open)?;
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
        let recoverers = JournalRecovererRegistry::default();
        let maintenance =
            MaintenanceRunner::new(&mut store, &started.current.store_epoch, &recoverers)
                .run()
                .map_err(WorkspaceWriterError::Maintenance)?;
        if let MaintenanceOutcome::NeedsAttention { stage } = maintenance {
            return Err(WorkspaceWriterError::MaintenanceNeedsAttention(stage));
        }
        drop(store);

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

    fn client(&self) -> crate::persistence::workspace_writer::WorkspaceWriterClient {
        self.client.clone()
    }

    fn metadata(&self) -> &crate::persistence::metadata::WorkspaceMetadata {
        &self.metadata
    }

    fn previous_run(&self) -> Option<&crate::persistence::metadata::WorkspaceMetadata> {
        self.previous_run.as_ref()
    }

    #[cfg(test)]
    fn shutdown(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
    ) -> Result<(), crate::persistence::workspace_writer::WorkspaceWriterError> {
        match self.shutdown_internal(control_plane_cleanup, |error| {
            FinalCommitRecovery::Fail(error)
        })? {
            FinalShutdownOutcome::Completed => Ok(()),
            FinalShutdownOutcome::Abandoned => {
                unreachable!("默认最终提交失败路径不得转为知情放弃")
            }
        }
    }

    #[cfg(test)]
    /// 最终提交失败由生产调用方决定重试或承担旧状态恢复后果。
    fn shutdown_with_decision(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut decide: impl FnMut() -> FinalCommitDecision,
    ) -> Result<FinalShutdownOutcome, crate::persistence::workspace_writer::WorkspaceWriterError>
    {
        self.shutdown_with_fallible_decision(control_plane_cleanup, move || Ok(decide()))
    }

    /// 确认失败不是用户放弃；显式上抛并保持未正常退出。
    fn shutdown_with_fallible_decision(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut decide: impl FnMut() -> Result<
            FinalCommitDecision,
            crate::persistence::workspace_writer::WorkspaceWriterError,
        >,
    ) -> Result<FinalShutdownOutcome, crate::persistence::workspace_writer::WorkspaceWriterError>
    {
        self.shutdown_internal(control_plane_cleanup, move |_| match decide() {
            Ok(FinalCommitDecision::Retry) => FinalCommitRecovery::Retry,
            Ok(FinalCommitDecision::Abandon) => FinalCommitRecovery::Abandon,
            Err(error) => FinalCommitRecovery::Fail(error),
        })
    }

    fn shutdown_internal(
        self,
        control_plane_cleanup: ControlPlaneCleanup,
        mut on_final_error: impl FnMut(
            crate::persistence::workspace_writer::WorkspaceWriterError,
        ) -> FinalCommitRecovery,
    ) -> Result<FinalShutdownOutcome, crate::persistence::workspace_writer::WorkspaceWriterError>
    {
        let Self {
            writer,
            client: _,
            failure_forwarder,
            paths,
            metadata,
            previous_run: _,
        } = self;
        let shutdown = writer.shutdown();
        let forwarder = failure_forwarder.join();
        let report = shutdown?;
        if report.failed > 0 {
            return Err(crate::persistence::workspace_writer::WorkspaceWriterError::FlushFailed);
        }
        forwarder
            .map_err(|_| crate::persistence::workspace_writer::WorkspaceWriterError::Stopped)?;
        if control_plane_cleanup == ControlPlaneCleanup::Failed {
            return Err(
                crate::persistence::workspace_writer::WorkspaceWriterError::ControlPlaneCleanupFailed,
            );
        }
        loop {
            let result = (|| {
                let mut store = crate::persistence::workspace::WorkspaceDatabase::open(&paths)
                    .map_err(crate::persistence::workspace_writer::WorkspaceWriterError::Open)?;
                store
                    .mark_clean_shutdown(&metadata.run.run_id)
                    .map_err(crate::persistence::workspace_writer::WorkspaceWriterError::Metadata)
            })();
            match result {
                Ok(()) => return Ok(FinalShutdownOutcome::Completed),
                Err(error) => match on_final_error(error) {
                    FinalCommitRecovery::Retry => {}
                    FinalCommitRecovery::Abandon => return Ok(FinalShutdownOutcome::Abandoned),
                    FinalCommitRecovery::Fail(error) => return Err(error),
                },
            }
        }
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

enum FinalCommitRecovery {
    Retry,
    Abandon,
    Fail(crate::persistence::workspace_writer::WorkspaceWriterError),
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
) -> Option<crate::local_control::LocalControlService> {
    let metadata = workspace_writer?.metadata().clone();
    match crate::local_control::LocalControlService::start(metadata, events) {
        Ok(service) => Some(service),
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
    run_shell_loop(initial_cwd, run_shell_once);
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

fn run_shell_once(initial_cwd: Option<PathBuf>) -> startup_diagnostics::ShellRunOutcome {
    // 后台任务运行时复位:静态初值即注册表默认值,这里显式
    // 再对齐一次,保证同进程重启测试/后续配置载入从确定态出发。
    crate::background_tasks::reset_defaults();
    // 安装守卫只阻止与安装事务并发的常驻启动；旧 session 移交恢复已删除。
    match crate::update_download::handoff::installation_in_progress() {
        Ok(true) => {
            log::info!("installation in progress; resident startup deferred");
            return startup_diagnostics::ShellRunOutcome::Exit;
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
                return startup_diagnostics::run(error);
            }
        };
    // 首帧前恢复视图私有状态与上一运行退出事实；读取或解码失败只设置对应写屏障。
    let side_view_persistence = app_root::AppRootPersistence::load_with_previous_run(
        app_paths,
        Some(workspace_writer_client),
        previous_run,
    );
    // 控制面与 writer 移交给 GPUI 外关闭所有者；窗口只持有有界请求 client。
    let local_control = start_local_control_service(Some(&workspace_writer), shell_tx.clone());
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
        init(cx);
        // 静默启动:仅无显式 CLI 目录时参与;窗口以隐藏
        // 形态创建,托盘「显示」经 focus facade 回窗。
        let start_hidden = starts_hidden(regular_start, cx.global::<config::Settings>());
        windowing::open_initial_window(
            cx,
            shell_rx,
            initial_root,
            shell,
            app_root::AppRootServices {
                persistence: side_view_persistence,
                global_close: global_close_client,
            },
            start_hidden,
            regular_start,
        );
    });
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
    // 托盘收尾:先删图标再退进程,否则任务栏残影要等
    // 用户悬停才消失;join 保证删图标先于进程退出。
    crate::tray::shutdown();
    startup_diagnostics::ShellRunOutcome::Exit
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

    fn seed_maintenance_run(store: &mut WorkspaceDatabase) {
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
        raw.execute_batch("PRAGMA user_version = 2").unwrap();

        let (events, event_rx) = mpsc::sync_channel(super::SHELL_EVENT_CAPACITY);
        let failed = WorkspaceWriterRuntime::start_at(&paths, events);
        let error = failed.as_ref().err();
        assert!(
            matches!(error, Some(WorkspaceWriterError::Metadata(_)),),
            "实际错误: {error:?}"
        );
        while event_rx.try_recv().is_ok() {}
        assert!(event_rx.recv().is_err(), "启动失败必须回收转发线程");
        raw.execute_batch("PRAGMA user_version = 1").unwrap();

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
