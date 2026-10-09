use std::borrow::Cow;

use crate::completer::Completer;
use crate::matcher::CandidateMatcher;
use crate::options::CompletionOptions;
use crate::span::Span;
use crate::suggestion::{SemanticSuggestion, Suggestion, SuggestionKind};

/// 对照静态字符串表做匹配的补全来源。
pub struct StaticCompletion {
    options: Cow<'static, [String]>,
}

impl StaticCompletion {
    /// 由自持字符串列表创建补全来源。
    pub fn new(options: Cow<'static, [String]>) -> Self {
        Self { options }
    }

    /// 由 `&'static [&'static str]` 切片创建补全来源。
    pub fn from_static(options: &'static [&'static str]) -> Self {
        let v: Vec<String> = options.iter().map(|s| s.to_string()).collect();
        Self {
            options: Cow::Owned(v),
        }
    }
}

impl Completer for StaticCompletion {
    fn fetch(
        &mut self,
        _cwd: &str,
        prefix: impl AsRef<str>,
        span: Span,
        offset: usize,
        options: &CompletionOptions,
    ) -> Vec<SemanticSuggestion> {
        let mut matcher = CandidateMatcher::new(prefix, options, true);
        let current_span = Span::new(
            span.start.saturating_sub(offset),
            span.end.saturating_sub(offset),
        );

        for option in self.options.iter() {
            matcher.add_suggestion(SemanticSuggestion {
                suggestion: Suggestion {
                    value: option.clone(),
                    span: current_span,
                    description: None,
                    ..Suggestion::default()
                },
                kind: Some(SuggestionKind::Value("string".to_string())),
            });
        }

        matcher.suggestion_results()
    }
}
