use crate::options::CompletionOptions;
use crate::span::Span;
use crate::suggestion::SemanticSuggestion;

/// 可插拔补全来源的合同。
pub trait Completer {
    /// 对给定 `prefix` 取候选、过滤并排序。
    ///
    /// * `cwd` —— 当前工作目录(文件类来源消费)。
    /// * `prefix` —— 用户已输入的部分文本。
    /// * `span` —— `prefix` 在原始输入内覆盖的区间。
    /// * `offset` —— 区间相对行首的偏移(用于修正候选的 span)。
    /// * `options` —— 匹配/排序配置。
    fn fetch(
        &mut self,
        cwd: &str,
        prefix: impl AsRef<str>,
        span: Span,
        offset: usize,
        options: &CompletionOptions,
    ) -> Vec<SemanticSuggestion>;
}
