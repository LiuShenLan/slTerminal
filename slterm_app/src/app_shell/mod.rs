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
pub(crate) mod config;
pub(crate) mod theme;
#[cfg(all(test, feature = "gpui-test-support"))]
mod ui_tests;
pub(crate) mod windowing;
pub(crate) mod workspace;

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;

use gpui::{App, QuitMode};

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
/// M3 变体收窄为三个,均无生产者——泵与消费归 M3.2+/各篇:
/// - `RuntimeControl` 是 04 篇 M7 `RuntimeCallback` 的预埋缝(payload
///   `Arc<RuntimeDispatch>` 的类型面归 04 篇锚定,届时补挂;M3 先立变体不占类型);
/// - `OpenDirectories` 的消费(on_open_urls/右键「在此处打开」)归 09 篇,
///   M3 登记不接线。
#[allow(dead_code)] // M3 三变体均无生产者/消费者,纯预埋缝(逐变体注释为证)。
pub(crate) enum SltermShellEvent {
    /// 预埋缝(13 篇缝合点 9):生产者归 04 篇 M7 RuntimeServer::spawn_callback。
    MuxAttach,
    /// 预埋缝:payload `Arc<RuntimeDispatch>` 的类型面归 04 篇锚定,届时补挂。
    RuntimeControl,
    /// 消费(右键「在此处打开」/on_open_urls 落盘)归 09 篇;M3 只登记发送端。
    OpenDirectories(Vec<String>),
}

/// 在当前线程启动 GPUI 运行时并打开主窗口,阻塞直至 UI 退出。
///
/// 装配序(因果链照上游):assets → QuitMode → run 闭包内 init → open_window。
/// 关窗即退出(跨领域不变量 5):`QuitMode::LastWindowClosed` + 无 mux 驻留臂,
/// 最后一窗关闭 = 进程退出。
pub fn run_shell(initial_cwd: Option<PathBuf>, initial_command: Option<Program>) {
    let (shell_tx, shell_rx) = mpsc::channel();
    let open_urls_tx = shell_tx.clone();
    // 进程级 hook 管道(单实例,非 per-pane):必须在首个 PTY spawn 之前发布
    // `SLTERM_NOTIFY_PIPE`——ConPTY 合并当前进程环境,全部 pane 的子进程树
    // 天然携带作用域闸门。句柄活到事件循环退出,关窗后确定性回收 accept 线程。
    let (ai_events, pipe_server) = crate::ai_hook::spawn_gpui_server();
    let application = gpui_platform::application()
        .with_assets(assets::SltermAssets)
        .with_quit_mode(QuitMode::LastWindowClosed);
    application.on_open_urls(move |urls| {
        let _ = open_urls_tx.send(SltermShellEvent::OpenDirectories(urls));
    });
    application.run(move |cx| {
        init(cx);
        windowing::open_initial_window(cx, shell_rx, ai_events, initial_cwd, initial_command);
    });
    // run 返回 = 最后窗口已关闭;Drop 完成自连唤醒 + join。
    drop(pipe_server);
}

/// 组件库/内嵌字体/主题/终端切片/工作区的一次性初始化。
fn init(cx: &mut App) {
    register_bundled_fonts(cx);
    // 组件库必须只初始化一次,否则全局 action、菜单和主题状态重复注册。
    gpui_component::init(cx);
    // 读一次快照装全局:M3 无热应用;渲染路径不得重复读盘。
    cx.set_global(config::Settings::load());
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
