//! slTerminal application crate:GPUI 壳、编排、平台适配、i18n、build 生成。
//!
//! 启动链本体在本文件 `run()`(13 篇锚定序);`src/main.rs` 只承载
//! `#![windows_subsystem = "console"]`(D04-1)与委托——D11-1 测试形态以
//! lib.rs 为 crate 根,bin 是薄壳。
//!
//! 依赖边受 architecture/dependencies.toml 门禁约束(application 层,单向依赖
//! 全部 core crate;core 反向不可达)。

#![warn(rust_2018_idioms, future_incompatible)]
#![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![cfg_attr(clippy, deny(warnings))]

pub(crate) mod app_shell;
// M4.2 协议与仲裁层:消费点归 M4.5(壳侧管道接线)/05 篇(pane 宿主),
// 接线落地后摘除本 allow。
#[allow(dead_code)]
pub(crate) mod ai_agents;
#[allow(dead_code)]
pub(crate) mod ai_hook;
#[allow(dead_code)] // 消费点归 M4.4(AnswerInbox/会话投影)。
pub(crate) mod assistant_answer;
pub mod brand;
pub(crate) mod cli;
pub mod error;
pub(crate) mod gpui_terminal;
pub(crate) mod logging;
pub(crate) mod panic;
pub(crate) mod platform;
pub(crate) mod process_tree;
pub(crate) mod session;
pub(crate) mod terminal_registry;

use std::error::Error;

/// 启动链(13 篇「启动链」节,序固定,逐环因果见各环节注释):
/// boot_trace 锚点 → CLI 解析 → panic 钩 → console 准备 → 日志 → run_shell。
///
/// 上游对照剪枝:helper 拦截(ai_session_identity)归 03;环境双名导入层消亡;
/// mux probe / 单实例移交归 04/M7(M3 二次启动 = 双进程过渡态,13 篇开放问题 3)。
pub fn run() -> Result<(), Box<dyn Error>> {
    // 第一句:启动剖析 t=0 锚点(core 侧 PTY 段共用同一开关,见 slterm_terminal)。
    boot_trace("main enter");

    // CLI 只保 --working-directory/-e 两参;clap 资源/动词全量面归 04 篇重建。
    let options = match cli::parse_env_args() {
        Ok(options) => options,
        Err(error) => {
            platform::startup::report_error(&error, true);
            return Err(error.into());
        }
    };
    boot_trace("cli parsed");

    // panic 钩先于一切子系统:启动期 panic 必须有弹面 + 持久日志,不能静默消失。
    panic::attach_handler();

    // console 子系统语义反转后的进程级归一化(D04-1,13 篇改造节 1)。
    if let Err(error) = platform::startup::prepare_gui_process() {
        platform::startup::report_error(&error, true);
        return Err(error.into());
    }
    boot_trace("console prepared");

    // logger 先于一切子系统初始化,捕获后续所有启动阶段的输出。
    let _log_file = logging::initialize().expect("Unable to initialize logger");

    log::info!("Welcome to {}", brand::NAME);
    log::info!("Version {}", env!("CARGO_PKG_VERSION"));

    // 与上游同语义:显式目录须真实存在并归一成绝对路径,无效目录静默回退默认
    // cwd(让 shell 落 home),不为一个坏目录拦停启动。
    let initial_cwd = options
        .working_directory
        .filter(|path| path.is_dir())
        .and_then(|path| std::path::absolute(path).ok());

    app_shell::run_shell(initial_cwd, options.command);
    Ok(())
}

/// 启动剖析:`SLTERM_BOOT_TRACE=1 slterm` 从进程入口起逐段打点到 stderr,
/// 计时自首次调用起。core 侧(ConPTY 起桥段)由 slterm_terminal 的 pty_trace
/// 共用同一开关接力。
pub(crate) fn boot_trace(label: &str) {
    use std::sync::OnceLock;
    use std::time::Instant;
    static T0: OnceLock<Instant> = OnceLock::new();
    static ON: OnceLock<bool> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    if *ON.get_or_init(|| std::env::var_os("SLTERM_BOOT_TRACE").is_some()) {
        eprintln!(
            "[boot +{:>7.1}ms] {label}",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}
