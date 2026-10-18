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
// 持久化小原语(原子写/文件锁):ai_hook 安装器与各域状态写共用。
pub(crate) mod atomic_file;
// M4.5 单 pane 宿主:事件泵派发与 AgentActivity/AnswerInbox 归属。
pub(crate) mod agent_host;
// 屏幕规则与身份面已由 agent_host 的看门狗消费;`parse_command` 等
// OSC 133 匹配器消费归 05 篇 M5.3,设置/会话面归 05/06 篇。
#[allow(dead_code)]
pub(crate) mod ai_agents;
// 事件通道已由 M4.5 接线;安装/检视面(settings.rs 等)的消费点归 06 篇
// 设置域,桥接资产与自愈入口在 M4.3 落盘但尚无壳侧调用方。
#[allow(dead_code)]
pub(crate) mod ai_hook;
// 会话注册表写入已由 M5.5 SessionStart 边接线;扫描列表/手动恢复面板的
// 消费点归后续(命令面板)。
#[allow(dead_code)]
pub(crate) mod ai_sessions;
// 沙箱放行集消费点归 07 篇文件域;设置页展示归 05/06。
#[allow(dead_code)]
pub(crate) mod agent_dirs;
// per-pane 环境注入消费点归 02 篇 PTY spawn 链。
#[allow(dead_code)]
pub(crate) mod agent_env;
#[allow(dead_code)] // 挂 pane 宿主归 05 篇,回答展示归 07 篇。
pub(crate) mod assistant_answer;
pub mod brand;
pub(crate) mod cli;
// 事务型 Git worktree(07 篇边界,04 篇 agent.fork 消费):只回滚本事务
// 证明创建过的资源,失败路径不留半成品。
pub mod error;
pub(crate) mod git_worktree;
pub(crate) mod gpui_terminal;
// 界面语言(06 篇 M6.4):build.rs 静态生成的 typed Message 查询 API;
// 运行时零解析、零文件读、零锁。
pub(crate) mod i18n;
pub(crate) mod logging;
pub(crate) mod panic;
pub(crate) mod platform;
pub(crate) mod process_tree;
// Runtime 控制面(04 篇 M7):版本化信封 + loopback JSON Lines 传输;
// RuntimeHub/方法族/CLI/单实例移交归 M7.2–M7.5 继续接线。
pub(crate) mod runtime_api;
// 非 TTY pane.exec 执行器(04 篇):argv 无 shell 展开、双流并行排水、
// 超时整树回收;UI owner 以 `spawn` 把结果 respond 回 transport。
pub(crate) mod runtime_exec;
pub(crate) mod session;
// settings.json 写通道适配层(06 篇 M6.1):白名单校验/保存锁/原子写/.bak/
// 损坏三态;`slterm-settings` 保持值权威与零生产依赖。
pub(crate) mod settings_store;
pub(crate) mod terminal_registry;
// 主题库应用侧(06 篇 M6.3):版本化 JSON 信封/自定义主题持久化/主题包
// 有界检查。设置层管值、应用层管格式的分层边界件。
pub(crate) mod theme_library;

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
