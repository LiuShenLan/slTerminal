//! 启动命令行解析。M3 最小面:`--working-directory` / `-e` 两参,手写最小解析;
//! clap 资源/动词全量面(子命令、--help、版本)归 04 篇重建,本模块届时被替换。

use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::AppError;

/// `-e` 启动命令(上游 config::ui_config::Program 的裁剪形态:serde 持久化面
/// 随 05 篇 LaunchSession 回填,此处只承载启动链的一次性消费)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// 无参形态:M3.1 启动链只产 `WithArgs`(-e 尾捕),本变体为 05 篇
    /// LaunchSession serde 面预埋的类型配平。
    #[allow(dead_code)]
    Just(String),
    WithArgs {
        program: String,
        args: Vec<String>,
    },
}

impl Program {
    /// 消费点 = M3.2 WorkspaceStartup::LaunchTerminal 的 spawn 参数组装。
    #[allow(dead_code)]
    pub fn program(&self) -> &str {
        match self {
            Self::Just(program) => program,
            Self::WithArgs { program, .. } => program,
        }
    }

    /// 消费点同上(LaunchTerminal 组装)。
    #[allow(dead_code)]
    pub fn args(&self) -> &[String] {
        match self {
            Self::Just(_) => &[],
            Self::WithArgs { args, .. } => args,
        }
    }
}

/// M3 保留的两个启动参数。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartupOptions {
    pub working_directory: Option<PathBuf>,
    /// `-e`/`--command` 捕获的尾部命令(原样透传,不切分)。
    pub command: Option<Program>,
}

/// 从进程环境参数解析(含跳过 argv[0])。
pub fn parse_env_args() -> Result<StartupOptions, AppError> {
    parse_args(std::env::args_os().skip(1))
}

pub fn parse_args<I, S>(args: I) -> Result<StartupOptions, AppError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut options = StartupOptions::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let arg = arg.into();
        let text = arg.to_string_lossy();
        if let Some(value) = text.strip_prefix("--working-directory=") {
            options.working_directory = Some(repair_context_menu_dir(PathBuf::from(value)));
        } else if text == "--working-directory" {
            let Some(value) = iter.next() else {
                return Err(invalid("--working-directory 需要一个目录参数"));
            };
            options.working_directory = Some(repair_context_menu_dir(PathBuf::from(value.into())));
        } else if text == "-e" || text == "--command" {
            // -e 必须末位:其余参数原样归命令行,不再按本程序旗标解释。
            let rest: Vec<String> = iter
                .map(|item| item.into().to_string_lossy().into_owned())
                .collect();
            let (program, args) = rest
                .split_first()
                .ok_or_else(|| invalid("-e 需要一个命令"))?;
            options.command = Some(Program::WithArgs {
                program: program.clone(),
                args: args.to_vec(),
            });
            return Ok(options);
        } else {
            return Err(invalid(format!("未知参数:{text}")));
        }
    }
    Ok(options)
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.into(),
    ))
}

/// 撤销资源管理器右键菜单引号化对盘符根目录的破坏(因果链照上游 cli.rs):
/// 安装器 verb 传 `--working-directory "%V"`;盘符根 `%V` 是 `D:\`,命令行以
/// `\"` 收尾,CommandLineToArgvW 把尾反斜杠读成转义引号——进程实际收到 `D:"`
/// 并被判为非法目录(上游 issue #36)。Windows 路径不可能含双引号,尾部孤立
/// 引号必然是被吞的分隔符,补回即可;其余路径原样返回。
fn repair_context_menu_dir(dir: PathBuf) -> PathBuf {
    if let Some(stripped) = dir.to_string_lossy().strip_suffix('"') {
        return PathBuf::from(format!("{stripped}\\"));
    }
    dir
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn parse<const N: usize>(args: [&str; N]) -> Result<StartupOptions, AppError> {
        parse_args(args)
    }

    #[test]
    fn empty_args_yield_the_default_options() {
        assert_eq!(parse([]).unwrap(), StartupOptions::default());
    }

    #[test]
    fn working_directory_accepts_space_and_equals_forms() {
        for (args, expected) in [
            (vec!["--working-directory", "D:/work"], "D:/work"),
            (vec!["--working-directory=D:/work"], "D:/work"),
        ] {
            let options = parse_args(args).unwrap();
            assert_eq!(options.working_directory, Some(PathBuf::from(expected)));
            assert_eq!(options.command, None);
        }
    }

    #[test]
    fn command_captures_the_tail_verbatim() {
        let options = parse(["-e", "git", "--literal=two words", "--flag"]).unwrap();
        assert_eq!(
            options.command,
            Some(Program::WithArgs {
                program: "git".into(),
                args: vec!["--literal=two words".into(), "--flag".into()],
            })
        );
        // 命令之后的参数不再解释为本程序旗标。
        let options = parse(["-e", "pwsh", "--working-directory", "x"]).unwrap();
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
    fn explorer_drive_root_quote_is_repaired() {
        // 防回归(上游 issue #36):盘符根目录尾引号补回分隔符,其余形态不动。
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
