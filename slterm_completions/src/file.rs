use std::fmt::Write;
use std::path::{Component, MAIN_SEPARATOR as SEP, Path, PathBuf, is_separator};

use unicode_segmentation::UnicodeSegmentation;

use crate::matcher::{CandidateMatcher, IgnoreCaseExt};
use crate::options::{CompletionOptions, MatchAlgorithm};
use crate::span::Span;

// ---------------------------------------------------------------------------
// 目录读取端口
// ---------------------------------------------------------------------------

/// 目录中的一个条目；OS 读取由 application 端口完成，core 只消费值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub is_dir: bool,
}

/// 目录读取端口：core 的遍历与匹配不直接触碰文件系统。
///
/// application 注入真实实现；不可读目录返回空集合，逐条目容错语义不变。
pub trait DirectoryReader {
    fn read_directory(&self, path: &Path) -> Vec<DirectoryEntry>;
}

// ---------------------------------------------------------------------------
// FileSuggestion
// ---------------------------------------------------------------------------

/// [`complete_literal_with_cancel`] 产出的单个文件/目录补全候选。
pub struct FileSuggestion {
    pub span: Span,
    pub path: String,
    pub is_dir: bool,
    pub display_override: Option<String>,
    pub match_indices: Vec<usize>,
}

// ---------------------------------------------------------------------------
// 递归路径补全的辅助类型
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct PathBuiltFromString {
    cwd: PathBuf,
    parts: Vec<MatchedPart>,
    isdir: bool,
}

#[derive(Clone, Default)]
struct MatchedPart {
    text: String,
    match_indices: Vec<usize>,
}

#[derive(Debug)]
enum OriginalCwd {
    None,
    Prefix(String),
}

/// 目录读取与取消的组合输入，避免递归函数参数继续膨胀。
struct CompletionIo<'a> {
    reader: &'a dyn DirectoryReader,
    cancelled: &'a dyn Fn() -> bool,
}

/// 递归走目录并收集命中路径；目录内容只从注入端口取得。
fn complete_rec(
    partial: &[&str],
    built_paths: &[PathBuiltFromString],
    options: &CompletionOptions,
    want_directory: bool,
    isdir: bool,
    enable_exact_match: bool,
    io: &CompletionIo<'_>,
) -> Vec<PathBuiltFromString> {
    if (io.cancelled)() {
        return Vec::new();
    }
    let has_more = !partial.is_empty() && (partial.len() > 1 || isdir);

    if let Some((&base, rest)) = partial.split_first()
        && matches!(base, "." | "..")
        && has_more
    {
        let built_paths: Vec<_> = built_paths
            .iter()
            .map(|built| {
                let mut built = built.clone();
                built.parts.push(MatchedPart {
                    text: base.to_string(),
                    match_indices: Vec::new(),
                });
                built.isdir = true;
                built
            })
            .collect();
        return complete_rec(
            rest,
            &built_paths,
            options,
            want_directory,
            isdir,
            enable_exact_match,
            io,
        );
    }

    let prefix = partial.first().unwrap_or(&"");
    let mut matcher = CandidateMatcher::literal(prefix, options, true);

    let mut exact_match = None;
    let mut multiple_exact_matches = false;

    for built in built_paths {
        if (io.cancelled)() {
            return Vec::new();
        }
        let mut path = built.cwd.clone();
        for part in &built.parts {
            path.push(part.text.as_str());
        }

        for entry in io.reader.read_directory(&path) {
            // 读取端口可能一次返回整目录；每个条目之间检查，避免旧输入继续
            // 处理大目录。
            if (io.cancelled)() {
                return Vec::new();
            }
            let entry_name = entry.name;
            let entry_isdir = entry.is_dir;
            let mut built = built.clone();
            built.isdir = entry_isdir;

            if !want_directory || entry_isdir {
                if enable_exact_match && !multiple_exact_matches && has_more {
                    let matches = if options.case_sensitive {
                        entry_name.eq(prefix)
                    } else {
                        entry_name.eq_ignore_case(prefix)
                    };
                    if matches {
                        if exact_match.is_none() {
                            let mut built_exact = built.clone();
                            let match_indices: Vec<usize> =
                                (0..entry_name.graphemes(true).count()).collect();
                            built_exact.parts.push(MatchedPart {
                                text: entry_name.clone(),
                                match_indices,
                            });
                            exact_match = Some(built_exact);
                        } else {
                            multiple_exact_matches = true;
                        }
                    }
                }

                matcher.add(entry_name.clone(), (built, entry_name));
            }
        }
    }

    // 单一精确命中 → 直接钻入(隐藏兄弟条目)
    if !multiple_exact_matches && let Some(built) = exact_match {
        return complete_rec(
            &partial[1..],
            &[built],
            options,
            want_directory,
            isdir,
            true,
            io,
        );
    }

    let completion_iter =
        matcher
            .results()
            .into_iter()
            .map(|((mut built, last_entry_name), last_match_indices)| {
                built.parts.push(MatchedPart {
                    text: last_entry_name,
                    match_indices: last_match_indices,
                });
                built
            });

    if has_more {
        completion_iter
            .flat_map(|completion| {
                complete_rec(
                    &partial[1..],
                    &[completion],
                    options,
                    want_directory,
                    isdir,
                    false,
                    io,
                )
            })
            .collect()
    } else {
        completion_iter.collect()
    }
}

/// 匹配解码后的字面路径：不做 shell 引用、home 或 n-dot 解释。
/// shell 拼写归调用方负责；遍历经注入端口，取消语义不变。
pub fn complete_literal_with_cancel(
    want_directory: bool,
    span: Span,
    partial: &str,
    cwds: &[impl AsRef<str>],
    options: &CompletionOptions,
    reader: &dyn DirectoryReader,
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileSuggestion> {
    if cancelled() {
        return Vec::new();
    }
    let isdir = partial.ends_with(is_separator);
    let mut partial = PathBuf::from(partial).to_string_lossy().to_string();

    // Windows 形态：/ 与 \ 皆为分隔符，沿用用户最近一次输入的那个。
    let path_separator = partial
        .chars()
        .rfind(|c: &char| is_separator(*c))
        .unwrap_or(SEP);

    // 处理尾部点号的情形
    if partial.ends_with(&format!("{path_separator}.")) {
        write!(partial, "{path_separator}.").expect("write to String is infallible");
    }

    let cwd_pathbufs: Vec<_> = cwds
        .iter()
        .map(|cwd| Path::new(cwd.as_ref()).to_path_buf())
        .collect();

    let mut cwds = cwd_pathbufs;
    let mut prefix_len = 0;
    let mut original_cwd = OriginalCwd::None;

    let mut components = Path::new(&partial).components().peekable();
    match components.peek().cloned() {
        Some(c @ Component::Prefix(..)) => {
            // Windows 前缀(如 `C:`)
            cwds = vec![[c, Component::RootDir].iter().collect()];
            prefix_len = c.as_os_str().len();
            original_cwd = OriginalCwd::Prefix(c.as_os_str().to_string_lossy().into_owned());
        }
        Some(c @ Component::RootDir) => {
            cwds = vec![PathBuf::from(c.as_os_str())];
            prefix_len = 1;
            original_cwd = OriginalCwd::Prefix(String::new());
        }
        _ => {}
    }

    let after_prefix = &partial[prefix_len..];
    let partial: Vec<_> = after_prefix
        .strip_prefix(is_separator)
        .unwrap_or(after_prefix)
        .split(is_separator)
        .filter(|s| !s.is_empty())
        .collect();

    let completed = complete_rec(
        partial.as_slice(),
        &cwds
            .into_iter()
            .map(|cwd| PathBuiltFromString {
                cwd,
                parts: Vec::new(),
                isdir: false,
            })
            .collect::<Vec<_>>(),
        options,
        want_directory,
        isdir,
        options.match_algorithm == MatchAlgorithm::Prefix,
        &CompletionIo { reader, cancelled },
    );
    if cancelled() {
        return Vec::new();
    }
    let suggestions = completed
        .into_iter()
        .take_while(|_| !cancelled())
        .map(|p| {
            let is_dir = p.isdir;

            let mut path = match &original_cwd {
                OriginalCwd::None => String::new(),
                OriginalCwd::Prefix(s) => format!("{s}{path_separator}"),
            };
            let mut match_index_offset = path.graphemes(true).count();
            let mut match_indices = Vec::new();
            for (i, part) in p.parts.iter().enumerate() {
                path.push_str(&part.text);
                for ind in &part.match_indices {
                    match_indices.push(ind + match_index_offset);
                }
                match_index_offset += part.text.graphemes(true).count();
                if i != p.parts.len() - 1 {
                    path.push(path_separator);
                    // 分隔符是一个 char 也是一个 grapheme;索引偏移按 grapheme 计数。
                    match_index_offset += 1;
                }
            }
            if p.isdir {
                path.push(path_separator);
            }

            FileSuggestion {
                span,
                path,
                is_dir,
                display_override: None,
                match_indices,
            }
        })
        .collect();
    if cancelled() { Vec::new() } else { suggestions }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::cell::Cell;
    use std::path::PathBuf;

    /// 测试端口：按路径返回固定条目，并记录读取顺序。
    struct FakeReader {
        entries: Vec<DirectoryEntry>,
        reads: Cell<usize>,
        paths: std::cell::RefCell<Vec<PathBuf>>,
    }

    impl FakeReader {
        fn new(entries: &[(&str, bool)]) -> Self {
            Self {
                entries: entries
                    .iter()
                    .map(|(name, is_dir)| DirectoryEntry {
                        name: (*name).to_owned(),
                        is_dir: *is_dir,
                    })
                    .collect(),
                reads: Cell::new(0),
                paths: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl DirectoryReader for FakeReader {
        fn read_directory(&self, path: &Path) -> Vec<DirectoryEntry> {
            self.reads.set(self.reads.get() + 1);
            self.paths.borrow_mut().push(path.to_path_buf());
            self.entries.clone()
        }
    }

    /// 机械门禁：本 crate 的生产面不得触碰文件系统或进程环境；目录读取与
    /// home 解析只经 application 注入。
    #[test]
    fn production_sources_do_not_access_the_filesystem_or_environment() {
        let production = include_str!("file.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("production section");
        for banned in ["std::fs::", "std::env::", ".read_dir(", ".is_dir("] {
            assert!(!production.contains(banned), "core 生产面不得出现 {banned}");
        }
    }

    #[test]
    fn literal_completion_reads_through_the_injected_directory_port() {
        let reader = FakeReader::new(&[("src", true), ("stuff.txt", false), ("alpha", false)]);
        let options = CompletionOptions::default();
        let items = complete_literal_with_cancel(
            false,
            Span::new(0, 2),
            "sr",
            &["C:/work"],
            &options,
            &reader,
            &|| false,
        );
        let expected_path = format!("src{SEP}");
        let paths: Vec<_> = items.iter().map(|item| item.path.as_str()).collect();
        assert_eq!(paths, [expected_path.as_str()]);
        assert!(items[0].is_dir);
        assert_eq!(reader.paths.borrow().as_slice(), [PathBuf::from("C:/work")]);
    }

    /// 文件候选的命中索引也按展示路径的 grapheme 计数,不按字节数。
    #[test]
    fn completed_paths_report_grapheme_match_indices() {
        // 本 crate 不引入测试临时目录依赖;用进程唯一的系统临时目录。
        let directory = std::env::temp_dir().join(format!(
            "slterm-completions-grapheme-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("中文文件.txt"), b"").unwrap();
        let cwd = directory.to_str().unwrap();
        let options = CompletionOptions {
            match_algorithm: MatchAlgorithm::Prefix,
            ..Default::default()
        };
        let items = complete_literal_with_cancel(
            false,
            Span::new(0, "中文".len()),
            "中文",
            &[cwd],
            &options,
            &|| false,
        );
        let item = items
            .iter()
            .find(|item| item.path.contains("中文文件"))
            .expect("临时目录必须补出中文文件");
        assert_eq!(item.match_indices, vec![0, 1]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cancellation_stops_directory_enumeration_and_discards_partial_results() {
        let options = CompletionOptions::default();
        let expected_reader = FakeReader::new(&[
            ("alpha", false),
            ("beta", false),
            ("gamma", false),
            ("delta", false),
        ]);
        let expected = complete_literal_with_cancel(
            false,
            Span::new(0, 0),
            "",
            &["C:/work"],
            &options,
            &expected_reader,
            &|| false,
        );
        assert_eq!(expected.len(), 4);

        let reader = FakeReader::new(&[
            ("alpha", false),
            ("beta", false),
            ("gamma", false),
            ("delta", false),
        ]);
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= 4
        };
        let actual = complete_literal_with_cancel(
            false,
            Span::new(0, 0),
            "",
            &["C:/work"],
            &options,
            &reader,
            &cancelled,
        );
        assert!(actual.is_empty());
        assert_eq!(reader.reads.get(), 1, "取消后不得继续读取目录");

        let reader = FakeReader::new(&[
            ("alpha", false),
            ("beta", false),
            ("gamma", false),
            ("delta", false),
        ]);
        let actual = complete_literal_with_cancel(
            false,
            Span::new(0, 0),
            "",
            &["C:/work"],
            &options,
            &reader,
            &|| false,
        );
        let values = |items: Vec<FileSuggestion>| {
            items
                .into_iter()
                .map(|item| {
                    (
                        item.path,
                        item.is_dir,
                        item.display_override,
                        item.match_indices,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(values(actual), values(expected));
    }
}
