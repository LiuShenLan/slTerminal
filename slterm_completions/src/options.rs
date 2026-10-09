use std::fmt::Display;

/// 候选与输入前缀的匹配算法。
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MatchAlgorithm {
    /// 只展示以输入开头的候选。
    ///
    /// 例:`"git switch"` 被 `"git sw"` 命中
    Prefix,

    /// 只展示包含输入子串的候选。
    ///
    /// 例:`"git checkout"` 被 `"checkout"` 命中
    Substring,

    /// 模糊匹配——字符按序出现即可,位置任意。
    ///
    /// 例:`"git checkout"` 被 `"gco"` 命中
    Fuzzy,
}

/// 补全结果的排序策略。
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum CompletionSort {
    /// 按字母序。
    #[default]
    Alphabetical,
    /// 智能排序:先按相关度评分,再按字母序。
    Smart,
}

/// 候选匹配与排序的配置。
#[derive(Clone)]
pub struct CompletionOptions {
    pub case_sensitive: bool,
    pub match_algorithm: MatchAlgorithm,
    pub sort: CompletionSort,
    /// 是否同时对候选描述文本做匹配。
    pub match_description: bool,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            case_sensitive: true,
            match_algorithm: MatchAlgorithm::Prefix,
            sort: CompletionSort::default(),
            match_description: false,
        }
    }
}

#[derive(Debug)]
pub enum InvalidMatchAlgorithm {
    Unknown,
}

impl Display for InvalidMatchAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown match algorithm")
    }
}

impl std::error::Error for InvalidMatchAlgorithm {}

impl TryFrom<String> for MatchAlgorithm {
    type Error = InvalidMatchAlgorithm;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "prefix" => Ok(Self::Prefix),
            "substring" => Ok(Self::Substring),
            "fuzzy" => Ok(Self::Fuzzy),
            _ => Err(InvalidMatchAlgorithm::Unknown),
        }
    }
}
