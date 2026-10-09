use crate::completer::Completer;
use crate::file::complete_item;
use crate::options::CompletionOptions;
use crate::span::Span;
use crate::suggestion::{SemanticSuggestion, Suggestion, SuggestionKind};

/// 只产出目录条目的补全来源。
pub struct DirectoryCompletion;

impl Completer for DirectoryCompletion {
    fn fetch(
        &mut self,
        cwd: &str,
        prefix: impl AsRef<str>,
        span: Span,
        offset: usize,
        options: &CompletionOptions,
    ) -> Vec<SemanticSuggestion> {
        let prefix = prefix.as_ref();

        let items = complete_item(true, span, prefix, &[cwd], options);

        let current_span = Span::new(
            span.start.saturating_sub(offset),
            span.end.saturating_sub(offset),
        );

        let mut hidden = Vec::new();
        let mut non_hidden = Vec::new();

        for x in items {
            let sugg = SemanticSuggestion {
                suggestion: Suggestion {
                    value: x.path,
                    span: current_span,
                    display_override: x.display_override,
                    match_indices: Some(x.match_indices),
                    ..Suggestion::default()
                },
                kind: Some(SuggestionKind::Directory),
            };

            let item_path = std::path::Path::new(&sugg.suggestion.value);
            if let Some(val) = item_path.file_name().and_then(|v| v.to_str()) {
                if val.starts_with('.') {
                    hidden.push(sugg);
                } else {
                    non_hidden.push(sugg);
                }
            }
        }

        // 隐藏文件后置排序:非隐藏条目在前。
        non_hidden.append(&mut hidden);
        non_hidden
    }
}
