//! 路径来源:语义候选与裸行共用同一次文件遍历。
//!
//! shell 转义只由 `slterm_completions::command_context` 负责(边界 3);
//! 本模块只决定「查哪个目录、是否只要目录、候选如何拼写」。

use slterm_completions::command_context::{CommandContext, ShellSyntax};
use slterm_completions::semantic::{Context, Source};
use slterm_completions::{CompletionOptions, SemanticSuggestion, Span, SuggestionKind};

use crate::directory_history::DirectoryHistory;
use crate::suggest::{CompletionStyle, Input, path_wants_directory};

/// 计算路径候选。取消即返回已收集结果;调用方在取消后必须整体丢弃。
pub(crate) fn complete(
    input: &Input<'_>,
    syntax: ShellSyntax,
    history: &DirectoryHistory,
    style: CompletionStyle,
    semantic: Option<&Context>,
    cancelled: &dyn Fn() -> bool,
) -> Vec<SemanticSuggestion> {
    if cancelled() {
        return Vec::new();
    }
    let parsed;
    let context = if let Some(semantic) = semantic {
        semantic.input()
    } else {
        parsed = CommandContext::parse(input.line, input.line.len(), syntax);
        let Some(context) = parsed.as_ref() else {
            return Vec::new();
        };
        context
    };
    let want_dir = semantic.map_or_else(
        || path_wants_directory(input.line),
        |context| {
            matches!(
                context.source,
                Source::Paths {
                    directories_only: true
                }
            )
        },
    );
    let prefix = semantic.map_or_else(|| context.prefix(), Context::value_prefix);
    let mut cwd = input.cwd.to_owned();
    if let Some(semantic) = semantic {
        for directory in &semantic.directories {
            if directory.is_empty() {
                continue;
            }
            let path = std::path::Path::new(directory);
            if !path.is_absolute() && cwd.is_empty() {
                return Vec::new();
            }
            cwd = std::path::Path::new(&cwd)
                .join(path)
                .to_string_lossy()
                .into_owned();
        }
    }
    let make = |path: &str, is_dir| {
        semantic
            .map_or_else(
                || context.candidate(path),
                |semantic| semantic.candidate(path),
            )
            .map(|suggestion| {
                SemanticSuggestion::with_kind(
                    suggestion,
                    if is_dir {
                        SuggestionKind::Directory
                    } else {
                        SuggestionKind::File
                    },
                )
            })
    };
    let expanded;
    let prefix = if context.expands_home() {
        expanded = slterm_completions::file::expand_home(prefix);
        let Some(expanded) = expanded.as_deref() else {
            return Vec::new();
        };
        expanded
    } else {
        prefix
    };
    if !std::path::Path::new(prefix).has_root() && cwd.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    // 旧访问记录仍可优先补到深层目录,但必须经过同一转义规则。
    if semantic.is_none_or(|context| context.directories.is_empty())
        && want_dir
        && prefix
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '/' | '\\' | ':' | '.' | '_' | '-'))
        && let Some(suffix) = history.hint_with_cancel(&format!("cd {prefix}"), &cwd, cancelled)
        && let Some(candidate) = make(
            &crate::platform::local_paths::completion_spelling(
                &format!("{prefix}{suffix}"),
                syntax,
            ),
            true,
        )
    {
        candidates.push(candidate);
        if style != CompletionStyle::Popup {
            return if cancelled() { Vec::new() } else { candidates };
        }
    }
    let options = CompletionOptions {
        case_sensitive: crate::platform::local_paths::completion_case_sensitive(),
        ..Default::default()
    };
    let matches = slterm_completions::file::complete_literal_with_cancel(
        want_dir,
        Span::new(0, prefix.len()),
        prefix,
        &[cwd.as_str()],
        &options,
        cancelled,
    );
    if cancelled() {
        return Vec::new();
    }
    let matches = if want_dir {
        history.rank_file_suggestions(matches, &cwd)
    } else {
        matches
    };
    for item in matches.into_iter().take_while(|_| !cancelled()) {
        let path = crate::platform::local_paths::completion_spelling(&item.path, syntax);
        if let Some(candidate) = make(&path, item.is_dir)
            && !candidates
                .iter()
                .any(|seen| seen.suggestion.value == candidate.suggestion.value)
        {
            candidates.push(candidate);
        }
        if candidates.len() == 256 {
            break;
        }
    }
    if cancelled() { Vec::new() } else { candidates }
}

#[cfg(test)]
mod paths_tests {
    use super::*;
    use crate::directory_history::DirectoryHistory;
    use crate::suggest::SuggestEnv;

    #[test]
    fn cancelled_path_source_returns_nothing() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("candidate.txt"), b"").unwrap();
        let env = SuggestEnv::Local;
        let input = Input {
            cwd: directory.path().to_str().unwrap(),
            env: &env,
            line: "cat ca",
        };
        let result = complete(
            &input,
            ShellSyntax::PowerShell,
            &DirectoryHistory::empty(),
            CompletionStyle::Inline,
            None,
            &|| true,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn shell_quoting_is_delegated_to_the_command_context() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("repo name")).unwrap();
        let env = SuggestEnv::Local;
        let input = Input {
            cwd: directory.path().to_str().unwrap(),
            env: &env,
            line: "cat 'repo na",
        };
        let result = complete(
            &input,
            ShellSyntax::Posix,
            &DirectoryHistory::empty(),
            CompletionStyle::Popup,
            None,
            &|| false,
        );
        let value = &result[0].suggestion.value;
        assert!(value.starts_with('\''), "{value}");
        // 目录尾分隔符在闭合引号之前:Posix 回写形态为 'repo name/'。
        assert!(value.ends_with("/'"), "{value}");
    }
}
