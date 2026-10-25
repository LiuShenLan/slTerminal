//! 命令行入口：无子命令启动 GUI；本地维护子命令执行后退出。
//!
//! `--working-directory` 与 `-e` 只影响首个终端。`notify-test` 和
//! `setup-ai --remove` 不连接运行中实例，也不初始化 GUI 子系统。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[cfg(test)]
use crate::error::AppError;
#[cfg(test)]
use std::ffi::OsString;

/// 顶层入口：`slterm` 无子命令 = GUI，有子命令 = 本地维护操作。
#[derive(Parser, Debug)]
#[command(
    name = "slterm",
    bin_name = "slterm",
    version = env!("CARGO_PKG_VERSION"),
    about = "slTerminal: Windows-native GPUI terminal",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// GUI 启动：首个终端的工作目录。
    #[arg(long, value_name = "DIRECTORY")]
    pub working_directory: Option<PathBuf>,

    /// GUI 启动：在首个终端执行的命令；其余参数原样透传（必须末位）。
    #[arg(
        short = 'e',
        long = "command",
        value_name = "COMMAND",
        allow_hyphen_values = true,
        num_args = 1..
    )]
    pub gui_command: Option<Vec<String>>,

    #[command(subcommand)]
    pub subcommands: Option<Subcommands>,
}

/// 不依赖运行中实例的本地维护命令。
#[derive(Subcommand, Debug)]
pub enum Subcommands {
    /// 诊断原生通知链：AUMID 注册、配置时长与同步测试 toast。
    NotifyTest,
    /// 安装器在删除安装目录前移除受支持 AI CLI 中的 slTerminal hooks。
    SetupAi(SetupAiOptions),
}

#[derive(Args, Debug)]
pub struct SetupAiOptions {
    /// 移除 hook 集成；这是唯一支持的操作。
    #[arg(long)]
    pub remove: bool,
}

/// 本地维护命令总入口；退出码语义与安装器、通知诊断调用方一致。
pub(crate) fn run_local(command: Subcommands) -> i32 {
    match command {
        Subcommands::NotifyTest => crate::platform::notifications::notify_test(),
        Subcommands::SetupAi(options) => run_setup_ai(options),
    }
}

/// 卸载器入口：未安装的 agent 不算失败；任一实际清理失败即返回非零。
fn run_setup_ai(options: SetupAiOptions) -> i32 {
    if !options.remove {
        eprintln!("usage: slterm setup-ai --remove");
        return 2;
    }
    match crate::ai_hook::remove_all_hooks() {
        Ok(failures) if failures.is_empty() => {
            println!("setup-ai: OK (hook integrations removed)");
            0
        }
        Ok(failures) => {
            for (agent, error) in failures {
                eprintln!("setup-ai: {}: {error}", agent.settings_key());
            }
            1
        }
        Err(error) => {
            eprintln!("setup-ai: {error}");
            1
        }
    }
}

/// `-e` 启动命令（启动链一次性消费；serde 持久化面归会话域）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// 无参形态：启动链只产 `WithArgs`；本变体为 `LaunchSession` 预埋。
    #[allow(dead_code)]
    Just(String),
    WithArgs {
        program: String,
        args: Vec<String>,
    },
}

impl Program {
    /// 消费点 = WorkspaceStartup::LaunchTerminal 的 spawn 参数组装。
    pub fn program(&self) -> &str {
        match self {
            Self::Just(program) => program,
            Self::WithArgs { program, .. } => program,
        }
    }

    /// 消费点同上（LaunchTerminal 组装）。
    pub fn args(&self) -> &[String] {
        match self {
            Self::Just(_) => &[],
            Self::WithArgs { args, .. } => args,
        }
    }
}

/// GUI 启动参数：本地维护子命令之外的唯一形态。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartupOptions {
    pub working_directory: Option<PathBuf>,
    pub command: Option<Program>,
}

impl Cli {
    /// GUI 启动视图；本地维护子命令由 `run()` 先分流，本函数不消费。
    pub fn startup_options(self) -> StartupOptions {
        let command = self.gui_command.and_then(|mut rest| {
            if rest.is_empty() {
                return None;
            }
            let program = rest.remove(0);
            Some(Program::WithArgs {
                program,
                args: rest,
            })
        });
        StartupOptions {
            working_directory: self.working_directory.map(repair_context_menu_dir),
            command,
        }
    }
}

/// 可注入的解析入口（测试用）；生产走 `Cli::try_parse()` + `clap::Error::exit`。
#[cfg(test)]
pub fn parse_args<I, S>(args: I) -> Result<Cli, AppError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = std::iter::once(OsString::from("slterm"))
        .chain(args.into_iter().map(Into::into))
        .collect::<Vec<_>>();
    Cli::try_parse_from(args).map_err(|error| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            error.to_string(),
        ))
    })
}

/// 修复资源管理器右键菜单对盘符根目录的引号化：Windows 路径不可能含双引号，
/// 尾部孤立引号必然是被 `CommandLineToArgvW` 吞掉的分隔符，其余路径原样返回。
fn repair_context_menu_dir(dir: PathBuf) -> PathBuf {
    if let Some(stripped) = dir.to_string_lossy().strip_suffix('"') {
        return PathBuf::from(format!("{stripped}\\"));
    }
    dir
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn parse<const N: usize>(args: [&str; N]) -> Result<Cli, AppError> {
        parse_args(args)
    }

    fn startup<const N: usize>(args: [&str; N]) -> StartupOptions {
        parse(args).expect("valid startup args").startup_options()
    }

    #[test]
    fn empty_args_yield_the_default_options() {
        let cli = parse([]).unwrap();
        assert!(cli.subcommands.is_none());
        assert_eq!(cli.startup_options(), StartupOptions::default());
    }

    #[test]
    fn working_directory_accepts_space_and_equals_forms() {
        for args in [
            vec!["--working-directory", "D:/work"],
            vec!["--working-directory=D:/work"],
        ] {
            let options = parse_args(args).unwrap().startup_options();
            assert_eq!(options.working_directory, Some(PathBuf::from("D:/work")));
            assert_eq!(options.command, None);
        }
    }

    #[test]
    fn command_captures_the_tail_verbatim() {
        let options = startup(["-e", "git", "--literal=two words", "--flag"]);
        assert_eq!(
            options.command,
            Some(Program::WithArgs {
                program: "git".into(),
                args: vec!["--literal=two words".into(), "--flag".into()],
            })
        );
        // 命令之后的参数不再解释为本程序旗标。
        let options = startup(["-e", "pwsh", "--working-directory", "x"]);
        assert_eq!(
            options.command,
            Some(Program::WithArgs {
                program: "pwsh".into(),
                args: vec!["--working-directory".into(), "x".into()],
            })
        );
        assert_eq!(options.working_directory, None);
    }

    #[test]
    fn unknown_or_incomplete_flags_are_errors() {
        assert!(parse(["--sploot"]).is_err());
        assert!(parse(["--working-directory"]).is_err());
        assert!(parse(["-e"]).is_err());
    }

    #[test]
    fn legacy_runtime_verbs_are_rejected() {
        for args in [
            vec!["ctl", "describe"],
            vec!["env"],
            vec!["window", "close", "1"],
            vec!["tab", "close", "0", "--window", "1"],
            vec!["pane", "list"],
            vec!["pane", "exec", "17", "--", "cargo", "test"],
            vec!["agent", "list"],
            vec!["agent", "delegate", "codex", "task"],
        ] {
            assert!(
                parse_args(args.clone()).is_err(),
                "旧 Runtime 命令必须拒绝: {args:?}"
            );
        }
    }

    #[test]
    fn local_maintenance_verbs_route_to_the_local_persona() {
        for args in [vec!["notify-test"], vec!["setup-ai", "--remove"]] {
            let cli = parse_args(args.clone())
                .unwrap_or_else(|error| panic!("{args:?} should parse: {error}"));
            assert!(
                cli.subcommands.is_some(),
                "{args:?} must route to the local maintenance persona"
            );
        }
    }

    /// 安装器卸载链依赖的 verb 形态：必须是本地维护命令且带 `--remove`。
    #[test]
    fn setup_ai_remove_parses_to_the_installer_hook() {
        match parse(["setup-ai", "--remove"]).unwrap().subcommands {
            Some(Subcommands::SetupAi(options)) => assert!(options.remove),
            other => panic!("unexpected command: {other:?}"),
        }
        match parse(["setup-ai"]).unwrap().subcommands {
            Some(Subcommands::SetupAi(options)) => assert!(!options.remove),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn unknown_verbs_and_bad_arguments_are_rejected() {
        for args in [
            vec!["bogus"],
            vec!["notify-test", "extra"],
            vec!["setup-ai", "--bogus"],
            vec!["ctl", "describe", "extra"],
        ] {
            assert!(
                parse_args(args.clone()).is_err(),
                "{args:?} must be rejected"
            );
        }
    }

    #[test]
    fn setup_ai_without_remove_returns_a_usage_error() {
        assert_eq!(run_setup_ai(SetupAiOptions { remove: false }), 2);
    }

    #[test]
    fn explorer_drive_root_quote_is_repaired() {
        assert_eq!(
            repair_context_menu_dir(PathBuf::from("D:\"")),
            PathBuf::from("D:\\")
        );
        assert_eq!(
            repair_context_menu_dir(PathBuf::from("D:/work")),
            PathBuf::from("D:/work")
        );
    }

    #[test]
    fn program_accessors_cover_both_variants() {
        let just = Program::Just("pwsh".into());
        assert_eq!(just.program(), "pwsh");
        assert!(just.args().is_empty());
        let full = Program::WithArgs {
            program: "git".into(),
            args: vec!["status".into()],
        };
        assert_eq!(full.program(), "git");
        assert_eq!(full.args(), &["status"]);
    }
}
