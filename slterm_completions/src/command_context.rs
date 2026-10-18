//! 字面量命令上下文与字节区间,与终端渲染无关。

use crate::{Span, Suggestion};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellSyntax {
    Posix,
    PowerShell,
    Cmd,
    /// 未知 shell 只共享裸的、无引用的参数。
    Literal,
}

impl ShellSyntax {
    pub fn for_program(program: &str) -> Self {
        let name = program
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(program)
            .to_ascii_lowercase();
        match name.trim_end_matches(".exe") {
            "bash" | "zsh" | "sh" | "dash" | "fish" => Self::Posix,
            "pwsh" | "powershell" => Self::PowerShell,
            "cmd" => Self::Cmd,
            _ => Self::Literal,
        }
    }
}

#[derive(Debug)]
struct Word {
    value: String,
    span: Span,
    quote: Option<char>,
    closed: bool,
    home: bool,
}

/// 字面参数与最后的可编辑词;绝不执行 shell 代码。
#[derive(Debug)]
pub struct CommandContext {
    pub(crate) arguments: Vec<String>,
    following: Vec<String>,
    full_word: Option<String>,
    target: Word,
    syntax: ShellSyntax,
}

impl CommandContext {
    pub fn parse(line: &str, cursor: usize, syntax: ShellSyntax) -> Option<Self> {
        Self::parse_words(line, cursor, syntax)
    }

    pub fn following_arguments(&self) -> &[String] {
        &self.following
    }

    pub(crate) fn full_word(&self) -> &str {
        self.full_word.as_deref().unwrap_or(&self.target.value)
    }

    fn parse_words(line: &str, cursor: usize, syntax: ShellSyntax) -> Option<Self> {
        if cursor > line.len() || !line.is_char_boundary(cursor) || line.len() > 4096 {
            return None;
        }
        let all = words(line, syntax)?;
        let mut words = words(&line[..cursor], syntax)?;
        let mut target = words.pop()?;
        let mut full_word = None;
        let following = if let Some(full) =
            all.iter().find(|word| word.span.start == target.span.start)
        {
            target.span.end = full.span.end;
            if full.value != target.value {
                full_word = Some(full.value.clone());
            }
            all.iter()
                .filter(|word| word.span.start > full.span.start && word.span.end > word.span.start)
                .map(|word| word.value.clone())
                .collect()
        } else {
            all.iter()
                .filter(|word| word.span.start >= cursor && word.span.end > word.span.start)
                .map(|word| word.value.clone())
                .collect()
        };
        // 已完成参数若含展开,无法证明它指向哪个目录;目标词的 home 由路径来源处理。
        if words.iter().any(|word| !word.closed || word.home)
            || all
                .iter()
                .any(|word| word.span.start > target.span.start && (!word.closed || word.home))
        {
            return None;
        }
        Some(Self {
            arguments: words.into_iter().map(|word| word.value).collect(),
            following,
            full_word,
            target,
            syntax,
        })
    }

    pub fn prefix(&self) -> &str {
        &self.target.value
    }

    pub fn expands_home(&self) -> bool {
        self.target.home && (self.prefix() == "~" || self.prefix().starts_with("~/"))
    }

    /// 匹配与引用分离,带评分/模糊的结果走同一编辑合同。
    pub fn candidate(&self, value: &str) -> Option<Suggestion> {
        if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return None;
        }
        let quote = self.target.quote;
        // PowerShell 将 -Fconfig.conf 拆成参数 -Fconfig 和 .conf;整个词需引用。
        // 普通选项仍保持裸写,否则 cmdlet 会把参数名当成位置参数。
        let parameter_split = self.syntax == ShellSyntax::PowerShell
            && value.starts_with('-')
            && value.contains(['.', ':']);
        let safe = !parameter_split
            && value.chars().all(|c| {
                c.is_alphanumeric()
                    || matches!(c, '/' | '.' | '_' | '-' | ':' | '=')
                    || c == '@'
                        && self.syntax != ShellSyntax::Literal
                        && (self.syntax != ShellSyntax::PowerShell || !value.starts_with('@'))
                    || c == ',' && matches!(self.syntax, ShellSyntax::Posix | ShellSyntax::Cmd)
                    || c == '\\'
                        && matches!(self.syntax, ShellSyntax::PowerShell | ShellSyntax::Cmd)
            });
        let cmd_quoted_safe = self.syntax == ShellSyntax::Cmd
            && value.chars().all(|c| {
                c.is_alphanumeric()
                    || matches!(
                        c,
                        '/' | '\\' | '.' | '_' | '-' | ':' | '=' | ' ' | '@' | ','
                    )
            });
        let quoted = match (quote, self.syntax, safe) {
            (Some('\''), ShellSyntax::Posix, _) => format!("'{}'", value.replace('\'', "'\\''")),
            (Some('\''), ShellSyntax::PowerShell, _) => format!("'{}'", value.replace('\'', "''")),
            (Some('"'), ShellSyntax::Posix, _) => format!(
                "\"{}\"",
                value
                    .replace('\\', "\\\\")
                    .replace('$', "\\$")
                    .replace('`', "\\`")
                    .replace('"', "\\\"")
            ),
            (Some('"'), ShellSyntax::PowerShell, _) => {
                format!(
                    "\"{}\"",
                    value
                        .replace('`', "``")
                        .replace('$', "`$")
                        .replace('"', "`\"")
                )
            }
            (_, ShellSyntax::Cmd, _) if cmd_quoted_safe && (quote.is_some() || !safe) => {
                // CMD 内建命令和外部 argv 对尾部反斜杠的规则不同;路径来源先统一为 /。
                if value.ends_with('\\') {
                    return None;
                }
                format!("\"{value}\"")
            }
            (None, _, true) => value.to_owned(),
            (None, ShellSyntax::Posix, false) => format!("'{}'", value.replace('\'', "'\\''")),
            (None, ShellSyntax::PowerShell, false) => format!("'{}'", value.replace('\'', "''")),
            // CMD 的百分号/延迟展开依赖运行中的选项,不能伪造通用转义。
            _ => return None,
        };
        Some(Suggestion {
            value: quoted,
            display_override: Some(value.to_owned()),
            span: self.target.span,
            append_whitespace: false,
            ..Default::default()
        })
    }
}

fn words(line: &str, syntax: ShellSyntax) -> Option<Vec<Word>> {
    let mut result = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some((start, first)) = chars.next() {
        if matches!(first, ' ' | '\t') {
            continue;
        }
        let mut quote = match first {
            '"' if syntax != ShellSyntax::Literal => Some(first),
            '\'' if matches!(syntax, ShellSyntax::Posix | ShellSyntax::PowerShell) => Some(first),
            _ => None,
        };
        let mut word = Word {
            value: String::new(),
            span: Span::new(start, line.len()),
            quote,
            closed: quote.is_none(),
            home: first == '~' && matches!(syntax, ShellSyntax::Posix | ShellSyntax::PowerShell),
        };
        let mut current = if quote.is_some() {
            chars.next()
        } else {
            Some((start, first))
        };
        while let Some((offset, ch)) = current {
            if quote == Some(ch) {
                if syntax == ShellSyntax::PowerShell
                    && chars.peek().is_some_and(|(_, next)| *next == ch)
                {
                    chars.next();
                    word.value.push(ch);
                } else {
                    quote = None;
                    word.closed = true;
                }
            } else if quote.is_none() && matches!(ch, ' ' | '\t') {
                word.span.end = offset;
                break;
            } else if quote.is_none()
                && (ch == '"' && syntax != ShellSyntax::Literal
                    || ch == '\'' && matches!(syntax, ShellSyntax::Posix | ShellSyntax::PowerShell))
            {
                quote = Some(ch);
                word.closed = false;
            } else {
                let escape = match syntax {
                    ShellSyntax::Posix => ch == '\\' && quote != Some('\''),
                    ShellSyntax::PowerShell => ch == '`' && quote != Some('\''),
                    _ => false,
                };
                if escape {
                    let (_, escaped) = chars.next()?;
                    if escaped.is_control() {
                        return None;
                    }
                    // PowerShell 的 `n/`t 等是控制字符,不是被转义的字母。
                    if syntax == ShellSyntax::PowerShell && escaped.is_ascii_alphanumeric() {
                        return None;
                    }
                    if syntax == ShellSyntax::Posix
                        && quote == Some('"')
                        && !matches!(escaped, '$' | '`' | '"' | '\\')
                    {
                        word.value.push(ch);
                    }
                    word.value.push(escaped);
                } else {
                    // 不执行展开、子命令或复合语句;已引用的字面量按 shell 方言判断。
                    let expansion = match syntax {
                        ShellSyntax::Cmd => matches!(ch, '%' | '!' | '^'),
                        ShellSyntax::Posix | ShellSyntax::PowerShell => {
                            quote != Some('\'') && matches!(ch, '$' | '`')
                        }
                        ShellSyntax::Literal => {
                            !ch.is_alphanumeric()
                                && !matches!(ch, '/' | '.' | '_' | '-' | ':' | '=')
                        }
                    };
                    if ch.is_control()
                        || expansion
                        || quote.is_none()
                            && matches!(
                                ch,
                                ';' | '|'
                                    | '&'
                                    | '<'
                                    | '>'
                                    | '('
                                    | ')'
                                    | '{'
                                    | '}'
                                    | '['
                                    | ']'
                                    | '*'
                                    | '?'
                                    | '#'
                                    | '\''
                                    | '"'
                            )
                    {
                        return None;
                    }
                    word.value.push(ch);
                }
            }
            current = chars.next();
        }
        if !word.closed && chars.peek().is_some() {
            return None;
        }
        result.push(word);
        if result.len() > 64 {
            return None;
        }
    }
    if line.ends_with([' ', '\t']) && result.last().is_none_or(|word| word.closed) {
        result.push(Word {
            value: String::new(),
            span: Span::new(line.len(), line.len()),
            quote: None,
            closed: true,
            home: false,
        });
    }
    Some(result)
}

// 测试即上游语义用例的落点;模块名按本仓纪律领域具名。
#[cfg(test)]
mod command_context_tests {
    use super::*;

    /// 只切词、绝不执行:展开与复合语句命中即拒解析。
    #[test]
    fn expansions_and_shell_operators_are_rejected_without_running_anything() {
        let marker = std::env::temp_dir().join("slterm-command-context-must-not-run");
        let _ = std::fs::remove_file(&marker);
        let command = format!("echo $(touch {})", marker.display());
        assert!(CommandContext::parse(&command, command.len(), ShellSyntax::Posix).is_none());
        assert!(!marker.exists(), "parsing must never execute shell code");

        for line in [
            "echo $HOME",
            "echo `whoami`",
            "echo a;b",
            "echo a|b",
            "echo a&b",
            "echo a<b",
            "echo a>b",
            "echo (a)",
            "echo *",
        ] {
            assert!(
                CommandContext::parse(line, line.len(), ShellSyntax::Posix).is_none(),
                "{line}"
            );
        }
        for line in ["echo $env:PATH", "echo `n", "echo a;b", "echo (a)"] {
            assert!(
                CommandContext::parse(line, line.len(), ShellSyntax::PowerShell).is_none(),
                "{line}"
            );
        }
        for line in ["echo %PATH%", "echo a!b", "echo a^b", "echo a;b"] {
            assert!(
                CommandContext::parse(line, line.len(), ShellSyntax::Cmd).is_none(),
                "{line}"
            );
        }
        for line in ["echo a?b", "echo a,b", "echo a@b"] {
            assert!(
                CommandContext::parse(line, line.len(), ShellSyntax::Literal).is_none(),
                "{line}"
            );
        }
        // Literal 允许的标点(路径/赋值形态)仍是字面量,不属于展开。
        assert!(CommandContext::parse("echo a=b", 8, ShellSyntax::Literal).is_some());
    }

    #[test]
    fn line_and_word_budgets_refuse_unsafe_input() {
        let long = "a".repeat(4097);
        assert!(CommandContext::parse(&long, long.len(), ShellSyntax::Posix).is_none());
        let words = std::iter::once("echo")
            .chain((0..64).map(|_| "x"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(CommandContext::parse(&words, words.len(), ShellSyntax::Posix).is_none());
        let exactly_64 = std::iter::once("echo")
            .chain((0..63).map(|_| "x"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(CommandContext::parse(&exactly_64, exactly_64.len(), ShellSyntax::Posix).is_some());
    }

    #[test]
    fn posix_candidate_quotes_with_backslash_and_single_quote_doubling() {
        let context = CommandContext::parse("echo 'a", 7, ShellSyntax::Posix).unwrap();
        assert_eq!(context.candidate("a b").unwrap().value, "'a b'");
        assert_eq!(context.candidate("a'b").unwrap().value, r"'a'\''b'");

        let context = CommandContext::parse("echo \"a", 7, ShellSyntax::Posix).unwrap();
        assert_eq!(
            context.candidate(r#"a$b`c\d"e"#).unwrap().value,
            r#""a\$b\`c\\d\"e""#
        );
    }

    #[test]
    fn powershell_candidate_doubles_single_quotes_and_quotes_split_parameters() {
        let context = CommandContext::parse("echo 'a", 7, ShellSyntax::PowerShell).unwrap();
        assert_eq!(context.candidate("a'b").unwrap().value, "'a''b'");

        let context = CommandContext::parse("echo \"a", 7, ShellSyntax::PowerShell).unwrap();
        assert_eq!(
            context.candidate("a$b`c\"d").unwrap().value,
            "\"a`$b``c`\"d\""
        );

        // -Fconfig.conf 会被 PowerShell 拆成两个参数:整个词引用。
        let context = CommandContext::parse("echo -Fconfig", 13, ShellSyntax::PowerShell).unwrap();
        assert_eq!(
            context.candidate("-Fconfig.conf").unwrap().value,
            "'-Fconfig.conf'"
        );
    }

    #[test]
    fn cmd_candidate_quotes_spaces_and_refuses_trailing_backslashes() {
        let context = CommandContext::parse("echo \"a", 7, ShellSyntax::Cmd).unwrap();
        assert!(context.candidate(r"C:\dir\").is_none());
        let context = CommandContext::parse("echo ", 5, ShellSyntax::Cmd).unwrap();
        assert!(context.candidate(r"C:\my dir\").is_none());
        assert_eq!(
            context.candidate(r"C:\my dir\file.txt").unwrap().value,
            r#""C:\my dir\file.txt""#
        );
        // CMD 的百分号/延迟展开依赖运行中的选项,不能伪造通用转义。
        assert!(context.candidate("a%PATH%b").is_none());
    }

    #[test]
    fn literal_syntax_accepts_only_unquoted_safe_arguments() {
        let context = CommandContext::parse("echo ", 5, ShellSyntax::Literal).unwrap();
        assert_eq!(
            context.candidate("plain/file.exe").unwrap().value,
            "plain/file.exe"
        );
        assert!(context.candidate("space value").is_none());
        assert!(context.candidate("@value").is_none());
    }

    #[test]
    fn candidate_span_covers_the_editable_target_word() {
        let line = "git -C repo status";
        let cursor = line.len();
        let context = CommandContext::parse(line, cursor, ShellSyntax::Posix).unwrap();
        assert_eq!(context.prefix(), "status");
        assert_eq!(context.following_arguments(), &[] as &[String]);
        let suggestion = context.candidate("status --short").unwrap();
        assert_eq!(suggestion.span, Span::new("git -C repo ".len(), cursor));
        assert_eq!(suggestion.display_value(), "status --short");

        let line = "git -C \"repo name\" ch --quiet";
        let cursor = "git -C \"repo name\" ch".len();
        let context = CommandContext::parse(line, cursor, ShellSyntax::PowerShell).unwrap();
        assert_eq!(context.prefix(), "ch");
        assert_eq!(context.following_arguments(), ["--quiet"]);
        assert_eq!(context.candidate("checkout").unwrap().value, "checkout");
    }

    #[test]
    fn for_program_maps_the_four_shell_dialects() {
        for (program, syntax) in [
            ("pwsh", ShellSyntax::PowerShell),
            (
                r"C:\Program Files\PowerShell\7\pwsh.exe",
                ShellSyntax::PowerShell,
            ),
            ("powershell.exe", ShellSyntax::PowerShell),
            ("cmd.exe", ShellSyntax::Cmd),
            ("bash", ShellSyntax::Posix),
            ("/usr/bin/zsh", ShellSyntax::Posix),
            ("fish.exe", ShellSyntax::Posix),
            ("nu", ShellSyntax::Literal),
        ] {
            assert_eq!(ShellSyntax::for_program(program), syntax, "{program}");
        }
    }
}
