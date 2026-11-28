use std::borrow::Cow;

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::options::{CompletionOptions, MatchAlgorithm};
use crate::suggestion::SemanticSuggestion;

// ---------------------------------------------------------------------------
// IgnoreCaseExt —— 经 unicase 的大小写折叠
// ---------------------------------------------------------------------------

pub trait IgnoreCaseExt {
    /// 本字符串的大小写折叠形态(用于比较,不用于展示)。
    fn to_folded_case(&self) -> String;
    /// 大小写无关的相等判定。
    fn eq_ignore_case(&self, other: &str) -> bool;
}

impl IgnoreCaseExt for str {
    fn to_folded_case(&self) -> String {
        unicase::UniCase::new(self).to_folded_case()
    }

    fn eq_ignore_case(&self, other: &str) -> bool {
        unicase::UniCase::new(self) == unicase::UniCase::new(other)
    }
}

// ---------------------------------------------------------------------------
// CandidateMatcher —— 核心 fuzzy / prefix / substring 匹配引擎
// ---------------------------------------------------------------------------

const QUOTES: [char; 3] = ['"', '\'', '`'];

/// nucleo 的索引单位取决于内部表示，这里统一换算为 grapheme 序号。
///
/// `Utf32Str::new` 对 ASCII 直接用字节、对非 ASCII 建 grapheme 首字符表；
/// 若首字符仍全为 ASCII（如 `e` + 组合符），它会退回原始字节表示。
/// 对外合同是「被匹配文本的 grapheme 索引」，因此三种形态都要归一。
fn nucleo_index_to_grapheme(haystack: &str, index: usize) -> usize {
    if haystack.is_ascii() {
        return index;
    }
    let grapheme_firsts_ascii = haystack
        .graphemes(true)
        .map(|grapheme| {
            grapheme
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii())
        })
        .all(|ascii| ascii);
    if !grapheme_firsts_ascii {
        return index;
    }
    haystack
        .grapheme_indices(true)
        .take_while(|(offset, _)| *offset < index)
        .count()
}

/// fuzzy 命中索引归一：排序去重后按 grapheme 计数。
fn nucleo_indices_to_graphemes(haystack: &str, indices: &[u32]) -> Vec<usize> {
    let mut graphemes: Vec<usize> = indices
        .iter()
        .map(|index| {
            let index = usize::try_from(*index)
                .expect("index should fit in usize on 32+ bit systems");
            nucleo_index_to_grapheme(haystack, index)
        })
        .collect();
    graphemes.sort_unstable();
    graphemes.dedup();
    graphemes
}

/// 把大小写折叠文本中的字节位置映射回原串的 grapheme 序号。
///
/// 折叠可能改变字节长度（如 `İ` 展开为两个码点），不能把折叠串的 grapheme
/// 数直接当作展示串的索引。
fn folded_byte_to_grapheme(haystack: &str, folded_byte: usize) -> usize {
    let mut folded = 0;
    for (index, grapheme) in haystack.graphemes(true).enumerate() {
        if folded >= folded_byte {
            return index;
        }
        folded += grapheme.to_folded_case().len();
    }
    haystack.graphemes(true).count()
}

/// 不评分(prefix / substring)匹配的内部数据。
struct UnscoredMatch<T> {
    item: T,
    haystack: String,
    match_indices: Vec<usize>,
}

/// fuzzy(评分)匹配的内部数据。
struct FuzzyMatch<T> {
    item: T,
    haystack: String,
    score: u16,
    match_indices: Vec<usize>,
}

enum State<T> {
    Unscored(Vec<UnscoredMatch<T>>),
    Fuzzy {
        matcher: Matcher,
        atom: Atom,
        matches: Vec<FuzzyMatch<T>>,
    },
}

/// 按配置的 [`MatchAlgorithm`] 对候选补全做过滤与排序。
///
/// 支持三种算法:
/// * [`Prefix`](MatchAlgorithm::Prefix) —— `starts_with`
/// * [`Substring`](MatchAlgorithm::Substring) —— `contains`
/// * [`Fuzzy`](MatchAlgorithm::Fuzzy) —— 经 `nucleo_matcher` 的跳字匹配
pub struct CandidateMatcher<'a, T> {
    options: &'a CompletionOptions,
    should_sort: bool,
    needle: String,
    strip_quotes: bool,
    state: State<T>,
}

impl<T> CandidateMatcher<'_, T> {
    /// 创建匹配器。
    ///
    /// * `needle` —— 待搜索文本;首尾引号会被剥离(终端回写形态)。
    /// * `options` —— 匹配配置(算法、大小写敏感等)。
    /// * `should_sort` —— 返回前是否排序。
    pub fn new(
        needle: impl AsRef<str>,
        options: &CompletionOptions,
        should_sort: bool,
    ) -> CandidateMatcher<'_, T> {
        let needle = needle.as_ref().trim_matches(QUOTES);
        let mut matcher = Self::literal(needle, options, should_sort);
        matcher.strip_quotes = true;
        matcher
    }

    /// 匹配解码后的值;引号字符属于文件名/参数本体。
    pub fn literal<'a>(
        needle: &str,
        options: &'a CompletionOptions,
        should_sort: bool,
    ) -> CandidateMatcher<'a, T> {
        match options.match_algorithm {
            MatchAlgorithm::Prefix | MatchAlgorithm::Substring => {
                let lowercase_needle = if options.case_sensitive {
                    needle.to_owned()
                } else {
                    needle.to_folded_case()
                };
                CandidateMatcher {
                    options,
                    should_sort,
                    needle: lowercase_needle,
                    strip_quotes: false,
                    state: State::Unscored(Vec::new()),
                }
            }
            MatchAlgorithm::Fuzzy => {
                let atom = Atom::new(
                    needle,
                    if options.case_sensitive {
                        CaseMatching::Respect
                    } else {
                        CaseMatching::Ignore
                    },
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                    false,
                );
                CandidateMatcher {
                    options,
                    should_sort,
                    needle: needle.to_owned(),
                    strip_quotes: false,
                    state: State::Fuzzy {
                        matcher: Matcher::new({
                            let mut cfg = Config::DEFAULT;
                            cfg.prefer_prefix = true;
                            cfg
                        }),
                        atom,
                        matches: Vec::new(),
                    },
                }
            }
        }
    }

    /// 内部:用 needle 检验 `haystack`,可选地存入 item。
    fn matches_aux(&mut self, orig_haystack: &str, item: Option<T>) -> Option<Vec<usize>> {
        let haystack = if self.strip_quotes {
            orig_haystack.trim_start_matches(QUOTES)
        } else {
            orig_haystack
        };
        let offset = orig_haystack.len() - haystack.len();
        let haystack = if self.strip_quotes {
            haystack.trim_end_matches(QUOTES)
        } else {
            haystack
        };
        match &mut self.state {
            State::Unscored(matches) => {
                let haystack_folded = if self.options.case_sensitive {
                    Cow::Borrowed(haystack)
                } else {
                    Cow::Owned(haystack.to_folded_case())
                };
                let match_start = match self.options.match_algorithm {
                    MatchAlgorithm::Prefix => {
                        if haystack_folded.starts_with(self.needle.as_str()) {
                            Some(0)
                        } else {
                            None
                        }
                    }
                    MatchAlgorithm::Substring => haystack_folded.find(self.needle.as_str()),
                    _ => unreachable!("Only prefix and substring algorithms don't use score"),
                };
                match_start.map(|byte_start| {
                    // 折叠只影响大小写与字节长度;对外索引始终按展示文本的 grapheme
                    // 计数，不混入字节或码点单位。
                    let byte_end = byte_start + self.needle.len();
                    let grapheme_start = folded_byte_to_grapheme(haystack, byte_start);
                    let grapheme_end = folded_byte_to_grapheme(haystack, byte_end);
                    let quote_offset = orig_haystack[..offset].graphemes(true).count();
                    let match_indices: Vec<usize> =
                        (quote_offset + grapheme_start..quote_offset + grapheme_end).collect();
                    if let Some(item) = item {
                        matches.push(UnscoredMatch {
                            item,
                            haystack: haystack.to_string(),
                            match_indices: match_indices.clone(),
                        });
                    }
                    match_indices
                })
            }
            State::Fuzzy {
                matcher,
                atom,
                matches,
            } => {
                let mut haystack_buf = Vec::new();
                let haystack_utf32 = Utf32Str::new(haystack, &mut haystack_buf);
                let mut indices = Vec::new();
                let score = atom.indices(haystack_utf32, matcher, &mut indices)?;
                let quote_offset = orig_haystack[..offset].graphemes(true).count();
                let indices: Vec<usize> = nucleo_indices_to_graphemes(haystack, &indices)
                    .into_iter()
                    .map(|index| quote_offset + index)
                    .collect();
                if let Some(item) = item {
                    matches.push(FuzzyMatch {
                        item,
                        haystack: haystack.to_string(),
                        score,
                        match_indices: indices.clone(),
                    });
                }
                Some(indices)
            }
        }
    }

    /// 若 `haystack` 命中 needle 则加入 `item`。
    ///
    /// 返回是否已加入。
    pub fn add(&mut self, haystack: impl AsRef<str>, item: T) -> bool {
        self.matches_aux(haystack.as_ref(), Some(item)).is_some()
    }

    /// 只检验 `haystack` 是否命中 needle,不插入。
    ///
    /// 命中时返回 `Some(match_indices)`。
    pub fn check_match(&mut self, haystack: &str) -> Option<Vec<usize>> {
        self.matches_aux(haystack, None)
    }

    fn sort(&mut self) {
        match &mut self.state {
            State::Unscored(matches) => {
                matches.sort_by(|a, b| {
                    let cmp_sensitive = a.haystack.cmp(&b.haystack);
                    if self.options.case_sensitive {
                        cmp_sensitive
                    } else {
                        a.haystack
                            .to_folded_case()
                            .cmp(&b.haystack.to_folded_case())
                            .then(cmp_sensitive)
                    }
                });
            }
            State::Fuzzy { matches, .. } => match self.options.sort {
                crate::options::CompletionSort::Alphabetical => {
                    matches.sort_by(|a, b| a.haystack.cmp(&b.haystack));
                }
                crate::options::CompletionSort::Smart => {
                    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.haystack.cmp(&b.haystack)));
                }
            },
        }
    }

    /// 排序(若配置)并返回全部命中项及其命中索引。
    pub fn results(mut self) -> Vec<(T, Vec<usize>)> {
        if self.should_sort {
            self.sort();
        }
        match self.state {
            State::Unscored(matches) => matches
                .into_iter()
                .map(|m| (m.item, m.match_indices))
                .collect(),
            State::Fuzzy { matches, .. } => matches
                .into_iter()
                .map(|m| (m.item, m.match_indices))
                .collect(),
        }
    }
}

// —— SemanticSuggestion 便捷辅助 -------------------------------------------

impl CandidateMatcher<'_, SemanticSuggestion> {
    /// 加入一个 `SemanticSuggestion`(按其展示值匹配)。
    pub fn add_suggestion(&mut self, sugg: SemanticSuggestion) -> bool {
        let value = sugg.suggestion.display_value().to_string();
        self.add(value, sugg)
    }

    /// 返回排序后的全部命中候选,并写入命中索引。
    pub fn suggestion_results(self) -> Vec<SemanticSuggestion> {
        self.results()
            .into_iter()
            .map(|(mut sugg, indices)| {
                sugg.suggestion.match_indices = Some(indices);
                sugg
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// 测试(移植自 nushell `completion_options.rs`,名字全部剥离)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod matcher_tests {
    use super::*;

    #[test]
    fn prefix_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Prefix,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("examp", &opts, true);
        assert!(m.add("example text", "example text"));
        assert!(!m.add("text", "text"));
        let results: Vec<_> = m.results().iter().map(|r| r.0).collect();
        assert_eq!(vec!["example text"], results);
    }

    #[test]
    fn prefix_no_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Prefix,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("text", &opts, true);
        assert!(!m.add("example text", "example text"));
        let results: Vec<_> = m.results().iter().map(|r| r.0).collect();
        let empty: Vec<&str> = Vec::new();
        assert_eq!(empty, results);
    }

    #[test]
    fn substring_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Substring,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("text", &opts, true);
        assert!(m.add("example text", "example text"));
        let results: Vec<_> = m.results().iter().map(|r| r.0).collect();
        assert_eq!(vec!["example text"], results);
    }

    #[test]
    fn substring_no_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Substring,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("mplxt", &opts, true);
        assert!(!m.add("example text", "example text"));
    }

    #[test]
    fn fuzzy_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Fuzzy,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("ext", &opts, true);
        assert!(m.add("example text", "example text"));
        let results: Vec<_> = m.results().iter().map(|r| r.0).collect();
        assert_eq!(vec!["example text"], results);
    }

    #[test]
    fn fuzzy_no_match() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Fuzzy,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("mpp", &opts, true);
        assert!(!m.add("example text", "example text"));
    }

    #[test]
    fn fuzzy_sort_by_score() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Fuzzy,
            sort: crate::options::CompletionSort::Smart,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("fob", &opts, true);
        for item in ["foo/bar", "fob", "foo bar"] {
            m.add(item, item);
        }
        let results = m.results();
        assert_eq!(3, results.len());
        // 评分最高者在前;同分按字母序决胜
        assert_eq!("fob", results[0].0);
        assert_eq!("foo bar", results[1].0);
        assert_eq!("foo/bar", results[2].0);
    }

    /// 命中索引统一为 grapheme 单位:宽字符、组合字符与 emoji 簇都只占一个索引。
    #[test]
    fn prefix_and_substring_match_indices_are_grapheme_aligned() {
        let prefix = CompletionOptions {
            match_algorithm: MatchAlgorithm::Prefix,
            ..Default::default()
        };
        let mut matcher = CandidateMatcher::<&str>::new("中文", &prefix, true);
        assert_eq!(matcher.check_match("中文abc"), Some(vec![0, 1]));
        assert_eq!(matcher.check_match("'中文abc'"), Some(vec![1, 2]));

        let substring = CompletionOptions {
            match_algorithm: MatchAlgorithm::Substring,
            ..Default::default()
        };
        let mut matcher = CandidateMatcher::<&str>::new("中文", &substring, true);
        assert_eq!(matcher.check_match("ab中文cd"), Some(vec![2, 3]));

        // 组合字符 e + U+0301 是一个 grapheme;后续 x 的索引必须是 1 而不是 2。
        let mut matcher = CandidateMatcher::<&str>::new("x", &substring, true);
        assert_eq!(matcher.check_match("e\u{301}x"), Some(vec![1]));
    }

    /// fuzzy 的 nucleo 索引是 UTF-32 码点;对外必须换算为 grapheme。
    #[test]
    fn fuzzy_match_indices_are_grapheme_aligned() {
        let options = CompletionOptions {
            match_algorithm: MatchAlgorithm::Fuzzy,
            ..Default::default()
        };
        let mut matcher = CandidateMatcher::<&str>::new("x", &options, true);
        assert_eq!(matcher.check_match("e\u{301}x"), Some(vec![1]));
        // 家庭 emoji 由多个码点组成一个 grapheme。
        assert_eq!(
            matcher.check_match("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}x"),
            Some(vec![1])
        );
        // 前导引号被剥离后仍按 grapheme 偏移计入。
        assert_eq!(
            matcher.check_match(
                "'\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}x'"
            ),
            Some(vec![2])
        );

        let mut matcher = CandidateMatcher::<&str>::new("fb", &options, true);
        assert_eq!(matcher.check_match("foo/bar"), Some(vec![0, 4]));
    }

    /// 大小写折叠路径不改变索引单位;折叠展开不得把索引推到错误位置。
    #[test]
    fn case_folded_match_indices_keep_grapheme_units() {
        for (needle, case_sensitive) in [("cargo", false), ("Cargo", true)] {
            let prefix = CompletionOptions {
                match_algorithm: MatchAlgorithm::Prefix,
                case_sensitive,
                ..Default::default()
            };
            let mut matcher = CandidateMatcher::<&str>::new(needle, &prefix, true);
            assert_eq!(
                matcher.check_match("CargoBuild"),
                Some(vec![0, 1, 2, 3, 4])
            );

            let substring = CompletionOptions {
                match_algorithm: MatchAlgorithm::Substring,
                case_sensitive,
                ..Default::default()
            };
            let substring_needle = if case_sensitive { "CARGO" } else { "cargo" };
            let mut matcher = CandidateMatcher::<&str>::new(substring_needle, &substring, true);
            assert_eq!(
                matcher.check_match("abCARGOcd"),
                Some(vec![2, 3, 4, 5, 6])
            );
        }
    }

    #[test]
    fn fuzzy_strip_quotes() {
        let opts = CompletionOptions {
            match_algorithm: MatchAlgorithm::Fuzzy,
            ..Default::default()
        };
        let mut m = CandidateMatcher::new("'love spaces' ", &opts, true);
        for item in [
            "'i love spaces'",
            "'i love spaces' so much",
            "'lovespaces' ",
        ] {
            m.add(item, item);
        }
        let results = m.results();
        // 只有 "'i love spaces' so much" 命中,因为查询在 "spaces" 后含空格
        assert_eq!(1, results.len());
        assert_eq!("'i love spaces' so much", results[0].0);
    }
}
