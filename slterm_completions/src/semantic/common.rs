//! 常用命令的参数角色;数据发现留给应用层,shell 方言只决定已知别名的语义。
//! SSH/WSL 选项表与分支随远程能力整砍(spec 不采纳点 1/2)。

use super::*;

const FILES: Source = Source::Paths {
    directories_only: false,
};
const DIRS: Source = Source::Paths {
    directories_only: true,
};
const POSIX_LS: &[OptionSpec] = &[flag(&[
    "-a", "-A", "-l", "-h", "-R", "-d", "-F", "-t", "-r", "-S", "-1",
])];
const POSIX_CAT: &[OptionSpec] = &[flag(&["-b", "-e", "-n", "-s", "-t", "-u", "-v"])];
const POSIX_COPY: &[OptionSpec] = &[flag(&["-R", "-r", "-f", "-i", "-p", "-v"])];
const POSIX_MOVE: &[OptionSpec] = &[flag(&["-f", "-i", "-n", "-v"])];
const POSIX_REMOVE: &[OptionSpec] = &[flag(&["-f", "-i", "-r", "-R", "-v"])];
const POSIX_MKDIR: &[OptionSpec] = &[flag(&["-p", "-v"]), value(&["-m"], Source::None)];
const POSIX_GREP: &[OptionSpec] = &[
    flag(&[
        "-i", "-n", "-r", "-R", "-v", "-l", "-c", "-E", "-F", "-w", "-x", "-q",
    ]),
    value(&["-e"], Source::None),
    value(&["-f"], FILES),
];
const PS_LIST: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath"], FILES),
    value(&["-Filter", "-Include", "-Exclude", "-Depth"], Source::None),
    flag(&[
        "-Directory",
        "-File",
        "-Force",
        "-Recurse",
        "-Name",
        "-Hidden",
    ]),
];
const PS_CONTENT: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath"], FILES),
    value(
        &[
            "-TotalCount",
            "-Tail",
            "-ReadCount",
            "-Encoding",
            "-Delimiter",
        ],
        Source::None,
    ),
    flag(&["-Raw", "-Wait", "-Force"]),
];
const PS_COPY: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath", "-Destination"], FILES),
    flag(&["-Recurse", "-Force", "-PassThru", "-WhatIf", "-Confirm"]),
];
const PS_MOVE: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath", "-Destination"], FILES),
    flag(&["-Force", "-PassThru", "-WhatIf", "-Confirm"]),
];
const PS_REMOVE: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath"], FILES),
    flag(&["-Force", "-Recurse", "-WhatIf", "-Confirm"]),
];
const PS_CD: &[OptionSpec] = &[
    value(&["-Path", "-LiteralPath"], DIRS),
    flag(&["-PassThru"]),
];
const CMD_CD: &[OptionSpec] = &[flag(&["/d"])];
const CMD_DIR: &[OptionSpec] = &[flag(&[
    "/a", "/b", "/s", "/p", "/w", "/d", "/n", "/o", "/q", "/r", "/x",
])];
const CMD_COPY: &[OptionSpec] = &[flag(&["/y", "/-y", "/v", "/b", "/a", "/z"])];
const CMD_MOVE: &[OptionSpec] = &[flag(&["/y", "/-y"])];
const CMD_DEL: &[OptionSpec] = &[flag(&["/p", "/f", "/s", "/q", "/a"])];

impl Context {
    pub(super) fn common(&mut self, syntax: ShellSyntax) -> Option<Source> {
        let name = &self.input.arguments[0];
        let normalized = if matches!(syntax, ShellSyntax::PowerShell | ShellSyntax::Cmd) {
            name.to_ascii_lowercase()
        } else {
            name.clone()
        };
        let program = normalized.strip_suffix(".exe").unwrap_or(&normalized);
        let powershell = syntax == ShellSyntax::PowerShell;
        let cmd = syntax == ShellSyntax::Cmd;
        let program_lower = program.to_ascii_lowercase();
        let (options, mut source, limit) = if cmd {
            match program {
                "cd" | "chdir" => (CMD_CD, DIRS, 1),
                "dir" => (CMD_DIR, FILES, usize::MAX),
                "copy" => (CMD_COPY, FILES, usize::MAX),
                "move" => (CMD_MOVE, FILES, usize::MAX),
                "del" | "erase" => (CMD_DEL, FILES, usize::MAX),
                "type" => (&[][..], FILES, usize::MAX),
                "mkdir" | "md" => (&[][..], DIRS, usize::MAX),
                _ => return None,
            }
        } else if powershell {
            match program_lower.as_str() {
                "cd" | "chdir" | "set-location" => (PS_CD, DIRS, 1),
                "dir" | "gci" | "get-childitem" => (PS_LIST, FILES, usize::MAX),
                "gc" | "type" | "get-content" => (PS_CONTENT, FILES, usize::MAX),
                "copy" | "cpi" | "copy-item" => (PS_COPY, FILES, usize::MAX),
                "move" | "mi" | "move-item" => (PS_MOVE, FILES, usize::MAX),
                "del" | "erase" | "ri" | "remove-item" => (PS_REMOVE, FILES, usize::MAX),
                // Windows 收敛:ls/cat/cp/mv/rm 在 Windows PowerShell 即 cmdlet 别名,
                // 直接映射 cmdlet 选项表;上游的 Unix PowerShell 兜底(POSIX 表)
                // 随 unix 平台分支整砍。
                "ls" => (PS_LIST, FILES, usize::MAX),
                "cat" => (PS_CONTENT, FILES, usize::MAX),
                "cp" => (PS_COPY, FILES, usize::MAX),
                "mv" => (PS_MOVE, FILES, usize::MAX),
                "rm" => (PS_REMOVE, FILES, usize::MAX),
                _ => return None,
            }
        } else {
            match program {
                "cd" => (&[][..], DIRS, 1),
                "ls" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_LS, FILES, usize::MAX)
                }
                "cat" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_CAT, FILES, usize::MAX)
                }
                "cp" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_COPY, FILES, usize::MAX)
                }
                "mv" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_MOVE, FILES, usize::MAX)
                }
                "rm" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_REMOVE, FILES, usize::MAX)
                }
                "mkdir" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_MKDIR, DIRS, usize::MAX)
                }
                "grep" if matches!(syntax, ShellSyntax::Posix | ShellSyntax::Literal) => {
                    (POSIX_GREP, Source::None, usize::MAX)
                }
                _ => return None,
            }
        };
        self.options = options;
        let mut index = 1;
        let mut positional = 0;
        let mut parse_options = true;
        let mut grep_pattern = false;
        while let Some(arg) = self.input.arguments.get(index) {
            if parse_options && arg == "--" {
                parse_options = false;
            } else if parse_options && arg.starts_with(if cmd { '/' } else { '-' }) {
                if let Some((option, name, attached)) = resolve(options, arg, cmd || powershell) {
                    if let Some(value_source) = option.value {
                        // 选项值只承担消费下一个参数的位置语义;附值形态不占位。
                        if attached.is_none() {
                            index += 1;
                            if self.input.arguments.get(index).is_none() {
                                return Some(value_source);
                            }
                        }
                        if program == "grep" && matches!(name, "-e" | "-f") {
                            grep_pattern = true;
                        }
                    } else if attached.is_some() {
                        return Some(Source::None);
                    }
                } else if !cmd
                    && !arg.starts_with("--")
                    && arg[1..].chars().all(|ch| {
                        options.iter().any(|option| {
                            option.value.is_none()
                                && option
                                    .names
                                    .iter()
                                    .any(|name| name.len() == 2 && name.ends_with(ch))
                        })
                    })
                {
                    // 常见 -al/-vvv 只在每个成员均为无值选项时成立。
                } else {
                    return Some(Source::None);
                }
            } else {
                positional += 1;
                if positional >= limit {
                    return Some(Source::None);
                }
                if program == "grep" {
                    grep_pattern = true;
                }
            }
            index += 1;
        }
        if parse_options && self.input.prefix().starts_with(if cmd { '/' } else { '-' }) {
            if let Some((option, _, Some(offset))) =
                resolve(options, self.input.prefix(), cmd || powershell)
            {
                self.attached = Some(offset);
                return option.value;
            }
            return Some(Source::Options);
        }
        if program == "grep" && grep_pattern {
            source = FILES;
        }
        Some(source)
    }
}

fn resolve<'a>(
    options: &'a [OptionSpec],
    arg: &str,
    case_insensitive: bool,
) -> Option<(&'a OptionSpec, &'static str, Option<usize>)> {
    for option in options {
        for name in option.names {
            if arg == *name || case_insensitive && arg.eq_ignore_ascii_case(name) {
                return Some((option, name, None));
            }
        }
    }
    // OpenSSH/getopt 的短选项值不剥离 '=';-F=file 指的是名为 =file 的文件。
    if !case_insensitive && arg.starts_with('-') && !arg.starts_with("--") {
        for (offset, ch) in arg.char_indices().skip(1) {
            let option = options.iter().find(|option| {
                option
                    .names
                    .iter()
                    .any(|name| name.len() == 2 && name.ends_with(ch))
            })?;
            let name = option
                .names
                .iter()
                .find(|name| name.len() == 2 && name.ends_with(ch))?;
            if option.value.is_some() {
                let end = offset + ch.len_utf8();
                return Some((option, name, (end < arg.len()).then_some(end)));
            }
        }
    }
    None
}
