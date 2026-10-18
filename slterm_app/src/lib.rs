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
// 后台任务注册表(09 篇 M10.8):元数据单点、订阅生命周期、重入闸门与
// 失败策略;1 Hz 看门狗/托盘快照与更新检查/预取收编于此。
pub(crate) mod background_tasks;
// 屏幕规则与身份面已由 agent_host 的看门狗消费;`parse_command` 等
// OSC 133 匹配器消费归 05 篇 M5.3,设置/会话面归 05/06 篇。
#[allow(dead_code)]
pub(crate) mod ai_agents;
// AI assistant 触发链(08 篇 M9.2):便宜门链纯函数、独立配置与建议条
// 数据形态;建议条渲染与请求发送归 M9.5。
#[allow(dead_code)]
pub(crate) mod ai_assistant;
// 事件通道已由 M4.5 接线;安装/检视面(settings.rs 等)的消费点归 06 篇
// 设置域,桥接资产与自愈入口在 M4.3 落盘但尚无壳侧调用方。
#[allow(dead_code)]
pub(crate) mod ai_hook;
// provider 元数据与凭据引用(08 篇 M9.2;凭据纪律锚点归 10 篇):
// 十三家预设、尾四掩码、注入式凭据读写;设置页消费归 M9.5/06。
#[allow(dead_code)]
pub(crate) mod ai_providers;
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
// 补全应用层(08 篇 M9.1):Session/Request 分层、来源缓存与后台计算。
// ghost/popup 呈现与按键接线归 M9.5;引擎本体在 slterm_completions。
#[allow(dead_code)] // 消费点归 M9.5 终端输入路径。
pub(crate) mod completion;
// frecency 目录智能(08 篇 M9.1):cd 补全与目录 ghost 的数据源。
#[allow(dead_code)] // 消费点归 M9.5 ghost/path 来源。
pub(crate) mod directory_history;
// 有界前缀读取(07 篇):内存上限先于解码生效。
pub(crate) mod document_io;
// 加密备份(10 篇 M10.10):白名单收集/口令密封/先验后写恢复 + 选择性恢复
// 与加密恢复点撤回。UI 入口归 06 设置页安全分区(D10-4),本步只落引擎。
#[allow(dead_code)]
pub(crate) mod encrypted_backup;
// 事务型 Git worktree(07 篇边界,04 篇 agent.fork 消费):只回滚本事务
// 证明创建过的资源,失败路径不留半成品。
pub mod error;
// git 子进程执行体 facade(07 篇 D07-1 已裁):M8.1 只读面 = 仓库根 +
// HEAD 字节(gitshow 数据源);四方法执行归 M8.5。
pub(crate) mod git;
// 有界本地 Git 发现(08 篇 M9.1):3s/1MB 双预算 + TTL + 代次失效。
#[allow(dead_code)]
pub(crate) mod git_completion;
pub(crate) mod git_worktree;
pub(crate) mod gpui_terminal;
// fish 式本地历史(08 篇 M9.1):单池 jsonl + 前缀 range。
#[allow(dead_code)] // 消费点归 M9.5 提交事件与 ghost。
pub(crate) mod history;
// 界面语言(06 篇 M6.4):build.rs 静态生成的 typed Message 查询 API;
// 运行时零解析、零文件读、零锁。
pub(crate) mod i18n;
pub(crate) mod logging;
// 数学引擎(08 篇 M9.4):后端无关 TeX 子集——parser→validate→IR→layout→
// compile→rasterizer/bitmap/cache/font/spacing 十模块。呈现接线(终端
// 覆盖层 paint pass、markdown 数学)归 M9.5。
#[allow(dead_code)] // 消费点归 M9.5。
pub(crate) mod math;
// 自含 markdown 文档模型(08 篇 M9.5,管线 B):扁平块序列 + 数学片段;
// 数学适配(MathRun/measure/fit)与终端覆盖层共用引擎单点。
#[allow(dead_code)] // 阅读器渲染接线归 07 篇后续步骤;本步交付模型与数学适配。
pub(crate) mod markdown;
// 通知中心单一漏斗(09 篇 M10.1):六变体、去重/节流/失败冷却/前台门控;
// 系统 toast 投递在 platform::notifications。
pub(crate) mod notify;
pub(crate) mod panic;
// 套餐余量与后台轮询(10 篇 M10.11):user 层 settings.json 来源 + 双套餐查询
// 注册表 + 快照槽/watch 推送 + GPUI executor 执行体;查询语义 F10 原样。
#[allow(dead_code)] // UI 消费点归 M10.12;执行体已挂 background_tasks 注册表。
pub(crate) mod plan_balance;
pub(crate) mod platform;
pub(crate) mod process_tree;
// provider 连通性测试语义化结果(08 篇):UI 边界再转本地化文案。
#[allow(dead_code)]
pub(crate) mod provider_test;
// Runtime 控制面(04 篇 M7):版本化信封 + loopback JSON Lines 传输;
// RuntimeHub/方法族/CLI/单实例移交归 M7.2–M7.5 继续接线。
pub(crate) mod runtime_api;
// 非 TTY pane.exec 执行器(04 篇):argv 无 shell 展开、双流并行排水、
// 超时整树回收;UI owner 以 `spawn` 把结果 respond 回 transport。
pub(crate) mod runtime_exec;
// 凭据域消费接口(10 篇 M9.2 过渡形态):trait + 出域句柄;真实
// Windows 凭据管理器三原语归 M10.9。
pub(crate) mod secrets;
// 回归语料(08 篇 M9.4):数学条目逐例编译/栅格不回归;化学/生物条目已裁。
#[cfg(test)]
pub(crate) mod scientific_corpus;
pub(crate) mod session;
// 侧栏文件树模型族(07 篇 M8.4):目录枚举/gitignore/文件名搜索/git 快照。
// 模型与渲染完全分层;WSL/SVN 臂已裁。
pub(crate) mod side_panel;
// settings.json 写通道适配层(06 篇 M6.1):白名单校验/保存锁/原子写/.bak/
// 损坏三态;`slterm-settings` 保持值权威与零生产依赖。
pub(crate) mod settings_store;
// 补全/建议计算核心(08 篇 M9.1):ghost 余量与 popup 多候选同源。
#[allow(dead_code)] // 消费点归 M9.5 呈现接线。
pub(crate) mod suggest;
// 任务栏进度(09 篇 M10.2):OSC 9;4 五态宽容映射 + 进程级单显示路由;
// `ITaskbarList3` 投递在 Windows 分支自管 COM,失败一律静默。
pub(crate) mod taskbar;
// 系统托盘(09 篇 M10.3):独立消息泵线程 + 右键 agent 直达菜单 + 1 Hz
// 快照去抖;attention 聚合驱动橙点图标。图标资产归 M12。
pub(crate) mod tray;
// 自动更新检查(09 篇 M10.5):GitHub Releases 解析/资产四重校验/版本
// 比较/按版本提示状态;网络请求与下载共用自解析代理。
pub(crate) mod update_check;
// 更新包下载与校验(09 篇 M10.5):.part 流式 + 长度/MZ/SHA-256/512MiB
// 全过才原子替换;generation 作废链与启动 hydrate 双证据。
pub(crate) mod update_download;
// 更新 HTTP 客户端与代理解析(09 篇 M10.5):注册表 ProxyServer 按协议
// 取值、socks 归 socks5、env 回退、NO_PROXY、https_only 双超时。
pub(crate) mod update_proxy;
// 窗口管理 facade(09 篇 M10.4):四路回窗(toast/托盘/按钮/ATTACH)与
// HWND 查询的编译期单一实现点;Win32 焦点/还原原语只在本模块出现。
pub(crate) mod windowing;
// 终端数学覆盖层(08 篇 M9.4):有界扫描、LineProjection、CoverageMask 与
// 后端无关绘制计划。终端仍是真值——不改 cell/滚动/选区/复制语义。
#[allow(dead_code)] // 消费点归 M9.5(GPUI paint pass)。
pub(crate) mod terminal_math;
pub(crate) mod terminal_registry;
// 文本快照(07 篇):编码/行尾/只读合同单点;M8.2 编辑内核的共享底座。
pub(crate) mod text_document;
// 主题库应用侧(06 篇 M6.3):版本化 JSON 信封/自定义主题持久化/主题包
// 有界检查。设置层管值、应用层管格式的分层边界件。
pub(crate) mod theme_library;

use std::error::Error;

/// 启动链(13 篇「启动链」节,序固定,逐环因果见各环节注释):
/// boot_trace 锚点 → CLI 解析/同 exe 分流 → panic 钩 → console 准备 → 日志 → run_shell。
///
/// 04 篇 M7.4 方案 A:带子命令 = CLI 客户端人格(请求-响应后退出,不初始化
/// GUI 子系统);无子命令 = GUI 启动。
///
/// 上游对照剪枝:helper 拦截(ai_session_identity)归 03;环境双名导入层消亡;
/// mux probe / 单实例移交归 04/M7(M3 二次启动 = 双进程过渡态,13 篇开放问题 3)。
pub fn run() -> Result<(), Box<dyn Error>> {
    use clap::Parser as _;

    // 第一句:启动剖析 t=0 锚点(core 侧 PTY 段共用同一开关,见 slterm_terminal)。
    boot_trace("main enter");

    // clap 统一承载 --help/--version 与解析拒绝的退出码语义。
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };
    // CLI 人格:响应一律 JSON,退出码由 run_cli 决定;control plane 是给
    // pane 里的 AI CLI 用的,不启动 GUI/日志/窗口。
    if let Some(command) = cli.subcommands {
        std::process::exit(crate::runtime_api::run_cli(command));
    }
    let options = cli.startup_options();
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
