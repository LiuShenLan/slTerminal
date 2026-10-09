use crate::span::Span;

/// 补全候选,补全域的核心值类型。
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// 接受补全时写入的文本。
    pub value: String,
    /// 展示层覆盖文本(展示时代替 `value` 显示)。
    pub display_override: Option<String>,
    /// 可选描述/文档。
    pub description: Option<String>,
    /// 任意附加数据。
    pub extra: Option<String>,
    /// 接受后是否追加空白。
    pub append_whitespace: bool,
    /// 展示文本中命中查询的索引位置(弹窗高亮位)。
    pub match_indices: Option<Vec<usize>>,
    /// 本候选在输入中替换的区间。
    pub span: Span,
}

impl Default for Suggestion {
    fn default() -> Self {
        Self {
            value: String::new(),
            display_override: None,
            description: None,
            extra: None,
            append_whitespace: true,
            match_indices: None,
            span: Span::new(0, 0),
        }
    }
}

impl Suggestion {
    /// 展示值——覆盖文本优先,否则取原始值。
    pub fn display_value(&self) -> &str {
        self.display_override.as_deref().unwrap_or(&self.value)
    }
}

/// 候选的种类标注。
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionKind {
    Command(String),
    Value(String),
    CellPath,
    Directory,
    File,
    Flag,
    Module,
    Operator,
    Variable,
}

/// 候选值与其可选种类标注的配对。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SemanticSuggestion {
    pub suggestion: Suggestion,
    pub kind: Option<SuggestionKind>,
}

impl SemanticSuggestion {
    pub fn new(suggestion: Suggestion) -> Self {
        Self {
            suggestion,
            kind: None,
        }
    }

    pub fn with_kind(suggestion: Suggestion, kind: SuggestionKind) -> Self {
        Self {
            suggestion,
            kind: Some(kind),
        }
    }
}

impl From<Suggestion> for SemanticSuggestion {
    fn from(suggestion: Suggestion) -> Self {
        Self {
            suggestion,
            ..Default::default()
        }
    }
}
