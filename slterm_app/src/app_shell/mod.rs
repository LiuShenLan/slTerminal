//! 应用壳:GPUI 消息循环装配与窗口生命周期(模块名映射:上游 `gpui_shell` →
//! `app_shell`,层级压平一格——无旧壳对照,旧名失去参照对象)。
//!
//! 壳即前端:壳内状态权威 = `SltermWorkspace` 直持 pane 实体与展示态,
//! 无跨进程投影层。

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
// 源码文件 tab:普通源码与 gitshow 只读臂。
pub(crate) mod code_tab;
pub(crate) mod config;
// 通用 diff 对照面(双栏只读)。
pub(crate) mod diff_tab;
// 文档/图片 tab + openable_in_app 路由单判定。
pub(crate) mod doc_tabs;
// 编辑器语义槽装配(颜色单源 = UI token)。
pub(crate) mod editor_theme;
// 文件树拖放载荷 + 独立幽灵预览。
pub(crate) mod file_drop;
// 文本文件视图骨架(只读视图与编辑内核)。
pub(crate) mod file_editor;
// 外部文件系统事件监听(树根递归 + 打开文件父目录;有界唤醒前台)。
pub(crate) mod fs_watch;
// 布局转换对(「布局单点」);消费点 = 快照/恢复闭环。
pub(crate) mod session_restore;
// GUI 设置页:左导航 + 右表单 + 路由式搜索 + 统一写通道。
pub(crate) mod settings_pane;
pub(crate) mod theme;
#[cfg(all(test, feature = "gpui-test-support"))]
mod ui_tests;
// 窗口视效 + 壁纸:模糊材质/透明度纪律/有界壁纸加载。
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

/// 壳跨线程唤醒事件；生产者与消费归各领域模块。
pub(crate) enum SltermShellEvent {
    /// 消费(右键「在此处打开」/on_open_urls 落盘)归窗口/平台域;本模块只登记发送端。
    #[allow(dead_code)] // 打开目录载荷由平台消费，壳事件本身只负责转发。
    OpenDirectories(Vec<String>),
    /// toast 点击回窗(统一走窗口 facade)。
    /// 托盘左键/菜单「显示」同样走这条单窗回窗语义。
    NotificationFocus(Option<PaneId>),
    /// 权限确认 toast 的动作按钮回传(安全半程消费;本片只交付通道面)。
    NotificationChoice {
        pane_id: PaneId,
        request_id: u64,
        choice: usize,
    },
    /// 托盘「退出」:与关窗同一条退出链(终局快照 + 退出)。
    TrayQuit,
    /// 后台更新检查结果:提示 + 按 `auto_download_updates`
    /// 预取安装包。安装授权仍需用户经 handoff 两阶段流程提交。
    UpdateAvailable(crate::update_check::UpdateCheckResult),
    /// AI hook 自愈失败边(类别 + agent,不含合成命令)。消费 = 进程级
    /// 通知,不绑定 pane,也不直出技术化命令文本。
    AiHookAlert(crate::ai_hook::HookFailureKind),
}

/// 本次启动是否静默。
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
    // 后台任务运行时复位:静态初值即注册表默认值,这里显式
    // 再对齐一次,保证同进程重启测试/后续配置载入从确定态出发。
    crate::background_tasks::reset_defaults();
    // 安装守卫:安装事务进行中时不启动常驻资源;预约安装
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
    // 会话恢复决策在开窗前完成:窗口尺寸/侧栏宽度要先进
    // 窗口定形与 spawn 网格;CLI 显式 cwd/-e 启动只回放几何、不恢复 tabs。
    let restore_tabs = initial_command.is_none() && initial_cwd.is_none();
    let mut startup_plan = workspace::session_recovery::prepare_startup(restore_tabs);
    // 更新交接恢复优先于普通会话恢复——成功/回滚安装后的首个启动用
    // 交接快照还原更新前的窗口;单窗口定位只取首个窗口。
    let update_restored_windows = if restore_tabs {
        match crate::update_download::handoff::restore_ticket() {
            Some(mut sessions) => match sessions.drain(..).next() {
                Some(session) => {
                    startup_plan.sidebar = session.sidebar;
                    startup_plan.window = session.window;
                    startup_plan.crashed = false;
                    startup_plan.session = Some(session);
                    log::info!("update handoff: restored workspace snapshot");
                    1
                }
                None => 0,
            },
            None => 0,
        }
    } else {
        0
    };
    let (shell_tx, shell_rx) = mpsc::channel();
    let open_urls_tx = shell_tx.clone();
    // 进程级 hook 管道(单实例,非 per-pane):必须在首个 PTY spawn 之前发布
    // `SLTERM_NOTIFY_PIPE`——ConPTY 合并当前进程环境,全部 pane 的子进程树
    // 天然携带作用域闸门。句柄活到事件循环退出,关窗后确定性回收 accept 线程。
    let (ai_events, pipe_server) = crate::ai_hook::spawn_gpui_server();
    // 配置守卫(跨进程 hook 配置变更自愈)挂在设置域启动链上,
    // 随壳生命周期存活,Drop 时 join 线程。失败边经壳事件队列投进程级
    // 通知——只传类别/agent,不传合成命令。
    let _config_guard = crate::ai_hook::spawn_config_guard_with({
        let shell_events = shell_tx.clone();
        move |alert| {
            let _ = shell_events.send(SltermShellEvent::AiHookAlert(alert));
        }
    });
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
    // 启动更新检查:GPUI background executor 载体,
    // 立即水合 + 缓存提示,12s 一次性延迟后联网;结果经壳事件队列回 UI。
    let update_events = shell_tx.clone();
    let application = gpui_platform::application()
        .with_assets(assets::SltermAssets)
        .with_quit_mode(QuitMode::LastWindowClosed);
    application.on_open_urls(move |urls| {
        let _ = open_urls_tx.send(SltermShellEvent::OpenDirectories(urls));
    });
    application.run(move |cx| {
        init(cx);
        // 更新检查:注册表 Once 任务,12s 启动延迟契约
        // 由 `background_tasks::startup_delay` 单点给出;立即段先水合缓存态。
        crate::update_check::start_gpui(cx, move |result| {
            let _ = update_events.send(SltermShellEvent::UpdateAvailable(result));
        });
        // 套餐余量轮询:注册表订阅 + 后台循环,首轮立即
        // 执行,此后按注册表节拍(默认 10s);订阅守卫随任务存活,Drop 退订。
        // 阻塞 HTTP 全程在 background executor,UI 线程零阻塞。
        crate::plan_balance::start_background_poll(cx.background_executor().clone());
        // 静默启动:仅无显式 CLI 启动时参与;窗口以隐藏
        // 形态创建,托盘「显示」经 focus facade 回窗。
        let start_hidden = starts_hidden(restore_tabs, cx.global::<config::Settings>());
        windowing::open_initial_window(
            cx,
            shell_rx,
            ai_events,
            initial_cwd,
            initial_command,
            startup_plan,
            start_hidden,
        );
        // 窗口建立后按实际恢复的窗口数确认交接 ticket(不一致则
        // 留待下次尝试,3 次上限后放弃)。
        if update_restored_windows > 0 {
            crate::update_download::handoff::acknowledge_restore(update_restored_windows);
        }
    });
    // 托盘收尾:先删图标再退进程,否则任务栏残影要等
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
    // 读一次快照装全局:无热应用;渲染路径不得重复读盘。
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
