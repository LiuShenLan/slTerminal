//! 应用壳:GPUI 消息循环装配与窗口生命周期(模块名映射:上游 `gpui_shell` →
//! `app_shell`,层级压平一格——无旧壳对照,旧名失去参照对象)。
//!
//! 壳即前端:壳内状态权威雏形 = `SltermWorkspace` 直持 pane 实体与展示态,
//! 无投影层(13 篇改造节 2;RuntimeHub facade 缝归 M7.2)。

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

pub(crate) mod assets;
// 源码文件 tab(07 篇 M8.1):普通源码与 gitshow 只读臂。
pub(crate) mod code_tab;
pub(crate) mod config;
// 通用 diff 对照面(07 篇 M8.5;双栏只读)。
pub(crate) mod diff_tab;
// 文档/图片 tab + openable_in_app 路由单判定(07 篇 M8.1)。
pub(crate) mod doc_tabs;
// 编辑器语义槽装配(07 篇 M8.5;颜色单源 = 06 篇 UI token)。
pub(crate) mod editor_theme;
// 文件树拖放载荷 + 独立幽灵预览(07 篇 M8.4)。
pub(crate) mod file_drop;
// 文本文件视图骨架(07 篇 M8.1 只读;M8.2 接编辑内核)。
pub(crate) mod file_editor;
// 布局转换对(「布局单点」);消费点 = M5.5 快照/恢复闭环。
pub(crate) mod session_restore;
// GUI 设置页(06 篇):左导航 + 右表单 + 路由式搜索 + 统一写通道。
pub(crate) mod settings_pane;
pub(crate) mod theme;
#[cfg(all(test, feature = "gpui-test-support"))]
mod ui_tests;
// 窗口视效 + 壁纸(09 篇 M10.7):模糊材质/透明度纪律/有界壁纸加载。
pub(crate) mod wallpaper;
pub(crate) mod windowing;
pub(crate) mod workspace;

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;

use gpui::{App, QuitMode};
use slterm_split::PaneId;

use crate::cli::Program;

/// GPUI 主窗标准错误句柄不可依赖:保留父进程重定向诊断,但写失败不能再把
/// 非致命错误升级成应用 panic。
pub(crate) fn try_write_stderr(args: fmt::Arguments<'_>) {
    use std::io::Write as _;

    let _ = writeln!(std::io::stderr(), "{args}");
}

/// 壳跨线程唤醒事件(上游同名枚举裁剪:Tray*/Notification*/SshPrompt/
/// UpdateAvailable 臂不迁)。
///
/// M3 变体收窄为三个,生产者/消费归各篇:
/// - `RuntimeControl` 载荷是 04 篇 M7.3 锚定的 `Arc<RuntimeDispatch>`;
///   生产者 = M7.5 `RuntimeServer::spawn_callback`,UI owner 执行后 respond;
/// - `OpenDirectories` 的消费(on_open_urls/右键「在此处打开」)归 09 篇,
///   M3 登记不接线。
#[allow(dead_code)] // M7.5 接线前仅 `MuxAttach` 无生产者;逐变体注释为证。
pub(crate) enum SltermShellEvent {
    /// 预埋缝(13 篇缝合点 9):生产者归 04 篇 M7.5 RuntimeServer::spawn_callback。
    MuxAttach,
    /// 写命令派发:UI owner 执行后经 `RuntimeDispatch::respond` 回执。
    RuntimeControl(std::sync::Arc<crate::runtime_api::RuntimeDispatch>),
    /// 消费(右键「在此处打开」/on_open_urls 落盘)归 09 篇;M3 只登记发送端。
    OpenDirectories(Vec<String>),
    /// toast 点击回窗(09 篇 M10.1;M10.4 窗口 facade 接管四路回窗)。
    /// 托盘左键/菜单「显示」同样走这条单窗回窗语义(M10.3)。
    NotificationFocus(Option<PaneId>),
    /// 权限确认 toast 的动作按钮回传(M10 安全半程消费;本片只交付通道面)。
    NotificationChoice {
        pane_id: PaneId,
        request_id: u64,
        choice: usize,
    },
    /// 托盘「退出」(09 篇 M10.3):与关窗同一条退出链(终局快照 + 退出)。
    TrayQuit,
    /// 后台更新检查结果(09 篇 M10.5):提示 + 按 `auto_download_updates`
    /// 预取安装包。安装授权仍需用户经 handoff 两阶段流程提交。
    UpdateAvailable(crate::update_check::UpdateCheckResult),
}

/// 本次启动是否静默(09 篇 M10.6)。
///
/// 设置两条件(`silent_start && tray`)由 [`config::Settings::start_hidden`]
/// 单点判定;显式 CLI 启动(`--working-directory`/`-e`)恒不静默——用户
/// 主动发起时把窗口藏起来没有意义(上游同因果:仅 RestoreOrDefault
/// 启动形态参与 start_hidden)。
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
pub fn run_shell(initial_cwd: Option<PathBuf>, initial_command: Option<Program>) {
    // 安装守卫(09 篇 M10.5):安装事务进行中时不启动常驻资源;预约安装
    // 在此执行,成功后本进程立即退出,由 helper 覆盖安装后重启。
    match crate::update_download::handoff::installation_in_progress() {
        Ok(true) => {
            log::info!("installation in progress; resident startup deferred");
            return;
        }
        Err(error) => log::warn!("could not inspect installation ownership: {error}"),
        Ok(false) => {}
    }
    if crate::update_download::handoff::apply_scheduled() {
        return;
    }
    // 会话恢复决策在开窗前完成(05 篇 M5.5):窗口尺寸/侧栏宽度要先进
    // 窗口定形与 spawn 网格;CLI 显式 cwd/-e 启动只回放几何、不恢复 tabs。
    let restore_tabs = initial_command.is_none() && initial_cwd.is_none();
    let startup_plan = workspace::session_recovery::prepare_startup(restore_tabs);
    let (shell_tx, shell_rx) = mpsc::channel();
    let open_urls_tx = shell_tx.clone();
    // 04 篇 M7.5:单实例控制面先于窗口创建。hub 是壳与服务端共享的同一份
    // 状态权威;回调把 ATTACH/Control 桥进壳事件队列。
    let runtime_hub = crate::runtime_api::RuntimeHub::new();
    let runtime_server = {
        let shell_events = shell_tx.clone();
        crate::runtime_api::RuntimeServer::spawn_callback(
            move |callback| {
                let event = match callback {
                    crate::runtime_api::RuntimeCallback::Attach => SltermShellEvent::MuxAttach,
                    crate::runtime_api::RuntimeCallback::Control(dispatch) => {
                        SltermShellEvent::RuntimeControl(dispatch)
                    }
                };
                let _ = shell_events.send(event);
            },
            runtime_hub.clone(),
        )
    };
    if runtime_server.is_none() && initial_command.is_none() {
        // 与对端抢锁竞态:对方可能已拿锁但端点尚未落盘;40 × 25ms 重试
        // 后再决定自启还是移交(D04-2 契约数值)。
        let handover_cwd = initial_cwd.clone().or_else(|| std::env::current_dir().ok());
        if crate::runtime_api::handover_to_existing_instance(handover_cwd.as_deref(), None) {
            log::info!("runtime handover succeeded; this instance exits without a window");
            return;
        }
    }
    let _runtime_server = runtime_server; // 随壳生命周期持有;Drop 删自己的端口记录
    // 进程级 hook 管道(单实例,非 per-pane):必须在首个 PTY spawn 之前发布
    // `SLTERM_NOTIFY_PIPE`——ConPTY 合并当前进程环境,全部 pane 的子进程树
    // 天然携带作用域闸门。句柄活到事件循环退出,关窗后确定性回收 accept 线程。
    let (ai_events, pipe_server) = crate::ai_hook::spawn_gpui_server();
    // 通知通道开机位(09 篇 M10.1):toast 点击回投走同一壳事件队列;
    // AUMID 预注册幂等,必须早于第一条通知。
    crate::notify::init_gpui_activation(shell_tx.clone());
    crate::platform::notifications::prepare();
    // 托盘命令回调(09 篇 M10.3):托盘线程只经此把用户动作送回壳事件队列。
    crate::tray::init_gpui({
        let shell_events = shell_tx.clone();
        move |command| {
            let _ = shell_events.send(tray_command_event(command));
        }
    });
    // 启动更新检查(09 篇 M10.5):独立线程,延迟 12s 等首窗安顿;失败只
    // 落 debug 日志。结果经壳事件队列回到 UI 线程。
    crate::update_check::spawn_once({
        let shell_events = shell_tx.clone();
        move |result| {
            let _ = shell_events.send(SltermShellEvent::UpdateAvailable(result));
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
        // 静默启动(09 篇 M10.6):仅无显式 CLI 启动时参与;窗口以隐藏
        // 形态创建,托盘「显示」经 M10.4 focus facade 回窗。
        let start_hidden = starts_hidden(restore_tabs, cx.global::<config::Settings>());
        windowing::open_initial_window(
            cx,
            shell_rx,
            ai_events,
            initial_cwd,
            initial_command,
            startup_plan,
            runtime_hub,
            start_hidden,
        );
    });
    // 托盘收尾(09 篇 M10.3):先删图标再退进程,否则任务栏残影要等
    // 用户悬停才消失;join 保证删图标先于进程退出。
    crate::tray::shutdown();
    // run 返回 = 最后窗口已关闭;Drop 完成自连唤醒 + join。
    drop(pipe_server);
}

/// 组件库/内嵌字体/主题/终端切片/工作区的一次性初始化。
fn init(cx: &mut App) {
    register_bundled_fonts(cx);
    // 组件库必须只初始化一次,否则全局 action、菜单和主题状态重复注册。
    gpui_component::init(cx);
    // 读一次快照装全局:M3 无热应用;渲染路径不得重复读盘。
    let settings = config::Settings::load();
    // 组件库 locale 与自有文案分层:组件内置文案跟解析后的 UiLanguage。
    gpui_component::set_locale(settings.language.gpui_component_locale());
    cx.set_global(settings);
    theme::apply_dark_theme(cx);
    crate::gpui_terminal::init(cx);
    workspace::init(cx);
}

fn register_bundled_fonts(cx: &App) {
    // 在组件解析字体前注册内嵌 Maple:系统等宽字体无 NF 图标码点,提示符会出
    // 方框;跨机器字形同源是截图/UI 测试可比对的前提。导入字体目录归 06 篇。
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
    use slterm_settings::{RawSettings, RuntimeSettings};

    use super::{SltermShellEvent, starts_hidden, tray_command_event};
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

    /// 托盘线程只发命令枚举;壳侧翻译单点必须把 Focus 汇到已有的回窗
    /// 事件、把 Quit 送到关窗同语义的退出事件。
    #[test]
    fn tray_commands_map_to_focus_and_quit_shell_events() {
        assert!(matches!(
            tray_command_event(GpuiTrayCommand::Focus(None)),
            SltermShellEvent::NotificationFocus(None)
        ));
        let pane = slterm_split::PaneId(7);
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
