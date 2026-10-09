use std::fmt::Write;
use std::path::{Component, MAIN_SEPARATOR as SEP, Path, PathBuf, is_separator};

use unicode_segmentation::UnicodeSegmentation;

use crate::matcher::{CandidateMatcher, IgnoreCaseExt};
use crate::options::{CompletionOptions, MatchAlgorithm};
use crate::span::Span;

// ---------------------------------------------------------------------------
// FileSuggestion
// ---------------------------------------------------------------------------

/// [`complete_item`] 产出的单个文件/目录补全候选。
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
    Home,
    Prefix(String),
}

/// 展开 n-dots(`...` → `../..`,`....` → `../../..`,以此类推)。
fn expand_ndots(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '.' {
            let mut dot_count = 0;
            while chars.peek() == Some(&'.') {
                chars.next();
                dot_count += 1;
            }
            if dot_count > 2 {
                // n 个点 → n-1 组 ".."
                for i in 0..dot_count - 1 {
                    if i > 0 {
                        result.push(SEP);
                    }
                    result.push_str("..");
                }
            } else {
                for _ in 0..dot_count {
                    result.push('.');
                }
            }
        } else {
            result.push(chars.next().unwrap());
        }
    }

    PathBuf::from(result)
}

/// 把连续的 `..` 段折叠回 n-dots。
fn collapse_ndots(path: PathBuiltFromString) -> PathBuiltFromString {
    let mut result = PathBuiltFromString {
        parts: Vec::with_capacity(path.parts.len()),
        isdir: path.isdir,
        cwd: path.cwd,
    };
    let mut dot_count = 0;

    for part in path.parts {
        if part.text == ".." {
            dot_count += 1;
        } else {
            if dot_count > 0 {
                result.parts.push(MatchedPart {
                    text: ".".repeat(dot_count + 1),
                    match_indices: Vec::new(),
                });
                dot_count = 0;
            }
            result.parts.push(part);
        }
    }
    if dot_count > 0 {
        result.parts.push(MatchedPart {
            text: ".".repeat(dot_count + 1),
            match_indices: Vec::new(),
        });
    }
    result
}

/// 递归走目录并收集命中路径。
fn complete_rec(
    partial: &[&str],
    built_paths: &[PathBuiltFromString],
    options: &CompletionOptions,
    want_directory: bool,
    isdir: bool,
    enable_exact_match: bool,
    cancelled: &dyn Fn() -> bool,
) -> Vec<PathBuiltFromString> {
    if cancelled() {
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
            cancelled,
        );
    }

    let prefix = partial.first().unwrap_or(&"");
    let mut matcher = CandidateMatcher::literal(prefix, options, true);

    let mut exact_match = None;
    let mut multiple_exact_matches = false;

    for built in built_paths {
        if cancelled() {
            return Vec::new();
        }
        let mut path = built.cwd.clone();
        for part in &built.parts {
            path.push(part.text.as_str());
        }

        let Ok(result) = path.read_dir() else {
            continue;
        };

        for entry in result.filter_map(|e| e.ok()) {
            // 系统调用本身无法中断;每个条目之间检查,避免旧输入继续遍历大目录。
            if cancelled() {
                return Vec::new();
            }
            let entry_name = entry.file_name().to_string_lossy().into_owned();
            let entry_isdir = entry.path().is_dir();
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
            cancelled,
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
                    cancelled,
                )
            })
            .collect()
    } else {
        completion_iter.collect()
    }
}

/// 为安全写入对路径中的特殊字符转义。
///
/// 需要转义时返回 `Some(escaped)`;路径本身安全则返回 `None`。
pub fn escape_path(path: &str) -> Option<String> {
    // 检查类 glob 字符或反引号
    let has_glob = path.contains(['*', '?', '[', ']']);
    if has_glob || path.contains('`') {
        // 简单波浪号展开
        let expanded = if path.starts_with('~') {
            dirs_next_home().map_or_else(
                || path.to_string(),
                |home| {
                    let rest = &path[1..];
                    if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
                        format!("{}{}", home.display(), rest)
                    } else {
                        // ~user 形态——原样保留
                        path.to_string()
                    }
                },
            )
        } else {
            path.to_string()
        };

        if expanded.contains('\'') {
            Some(format!("{expanded:?}")) // debug 转义引号
        } else {
            Some(format!("'{expanded}'"))
        }
    } else {
        let contaminated =
            path.contains(['\'', '"', ' ', '#', '(', ')', '{', '}', '[', ']', '|', ';']);
        let maybe_flag = path.starts_with('-');
        let maybe_variable = path.starts_with('$');
        let maybe_number = path.parse::<f64>().is_ok();
        if contaminated || maybe_flag || maybe_variable || maybe_number {
            Some(format!("`{path}`"))
        } else {
            None
        }
    }
}

/// 取 home 目录(小辅助,能用 `std` 就不引入 `dirs` crate)。
fn dirs_next_home() -> Option<PathBuf> {
    // 先试标准环境变量,再回退到 home_dir(已废弃但可用)
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOMEDRIVE").and_then(|drive| {
                std::env::var_os("HOMEPATH").map(|path| {
                    let mut p = PathBuf::from(drive);
                    p.push(path);
                    p
                })
            })
        })
}

/// 只展开已验证的前导 home 记号;被引用的字面波浪号绝不走这里。
pub fn expand_home(partial: &str) -> Option<String> {
    let rest = partial.strip_prefix('~')?;
    if !rest.is_empty() && !rest.starts_with('/') {
        return None;
    }
    Some(format!("{}{rest}", dirs_next_home()?.to_str()?))
}

/// 去掉部分路径的首尾引号。
pub fn surround_remove(partial: &str) -> String {
    for c in ['`', '"', '\''] {
        if partial.starts_with(c) {
            let ret = partial.strip_prefix(c).unwrap_or(partial);
            return match ret.split(c).collect::<Vec<_>>()[..] {
                [inside] => inside.to_string(),
                [inside, outside] if inside.ends_with(is_separator) => {
                    format!("{inside}{outside}")
                }
                _ => ret.to_string(),
            };
        }
    }
    partial.to_string()
}

/// 递归补全文件/目录。
///
/// * `want_directory` —— 为 `true` 时只返回目录。
/// * `span` —— 部分路径在输入中的区间。
/// * `partial` —— 用户已输入的部分路径。
/// * `cwds` —— 搜索起点目录,可多个。
/// * `options` —— 匹配配置。
pub fn complete_item(
    want_directory: bool,
    span: Span,
    partial: &str,
    cwds: &[impl AsRef<str>],
    options: &CompletionOptions,
) -> Vec<FileSuggestion> {
    complete_item_with_cancel(want_directory, span, partial, cwds, options, &|| false)
}

/// 在目录操作之间做协作式取消的路径补全。
/// 取消即弃掉部分结果;进行中的 OS 读取无法打断。
pub fn complete_item_with_cancel(
    want_directory: bool,
    span: Span,
    partial: &str,
    cwds: &[impl AsRef<str>],
    options: &CompletionOptions,
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileSuggestion> {
    complete_paths(
        want_directory,
        span,
        partial,
        cwds,
        options,
        false,
        cancelled,
    )
}

/// 匹配解码后的字面路径:不做 shell 引用、home 或 n-dot 解释。
/// shell 拼写归调用方负责;遍历与取消仍共用。
pub fn complete_literal_with_cancel(
    want_directory: bool,
    span: Span,
    partial: &str,
    cwds: &[impl AsRef<str>],
    options: &CompletionOptions,
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileSuggestion> {
    complete_paths(
        want_directory,
        span,
        partial,
        cwds,
        options,
        true,
        cancelled,
    )
}

fn complete_paths(
    want_directory: bool,
    span: Span,
    partial: &str,
    cwds: &[impl AsRef<str>],
    options: &CompletionOptions,
    literal: bool,
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileSuggestion> {
    if cancelled() {
        return Vec::new();
    }
    let cleaned_partial = if literal {
        partial.to_owned()
    } else {
        surround_remove(partial)
    };
    let isdir = cleaned_partial.ends_with(is_separator);
    let expanded_partial = if literal {
        PathBuf::from(&cleaned_partial)
    } else {
        expand_ndots(Path::new(&cleaned_partial))
    };
    let should_collapse_dots = expanded_partial != Path::new(&cleaned_partial);
    let mut partial = expanded_partial.to_string_lossy().to_string();

    // Windows 形态:/ 与 \ 皆为分隔符,沿用用户最近一次输入的那个。
    let path_separator = cleaned_partial
        .chars()
        .rfind(|c: &char| is_separator(*c))
        .unwrap_or(SEP);

    // 处理尾部点号的情形
    if cleaned_partial.ends_with(&format!("{path_separator}.")) {
        write!(partial, "{path_separator}.").expect("write to String is infallible");
    }

    let cwd_pathbufs: Vec<_> = cwds
        .iter()
        .map(|cwd| Path::new(cwd.as_ref()).to_path_buf())
        .collect();

    let mut cwds = cwd_pathbufs.clone();
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
        Some(Component::Normal(home)) if !literal && home.to_string_lossy() == "~" => {
            cwds = dirs_next_home()
                .map(|dir| vec![dir])
                .unwrap_or(cwd_pathbufs);
            prefix_len = 1;
            original_cwd = OriginalCwd::Home;
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
        cancelled,
    );
    if cancelled() {
        return Vec::new();
    }
    let suggestions = completed
        .into_iter()
        .take_while(|_| !cancelled())
        .map(|mut p| {
            if should_collapse_dots {
                p = collapse_ndots(p);
            }
            let is_dir = p.isdir;

            let mut path = match &original_cwd {
                OriginalCwd::None => String::new(),
                OriginalCwd::Home => format!("~{path_separator}"),
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
                    match_index_offset += path_separator.len_utf8();
                }
            }
            if p.isdir {
                path.push(path_separator);
            }

            let (value, display_override) = if !literal && let Some(escaped) = escape_path(&path) {
                (escaped, Some(path))
            } else {
                (path, None)
            };
            FileSuggestion {
                span,
                path: value,
                is_dir,
                display_override,
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

    #[test]
    fn cancellation_stops_directory_enumeration_and_discards_partial_results() {
        let options = CompletionOptions::default();
        let cwd = [env!("CARGO_MANIFEST_DIR")];
        let span = Span::new(0, 4);
        let expected = complete_item(false, span, "src/", &cwd, &options);
        assert!(
            expected.len() > 3,
            "source directory supplies multiple real entries"
        );
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= 5
        };
        let actual = complete_item_with_cancel(false, span, "src/", &cwd, &options, &cancelled);
        assert!(actual.is_empty());
        assert!(
            checks.get() <= 7,
            "cancellation must stop enumeration, not just hide its output"
        );
        let actual = complete_item_with_cancel(false, span, "src/", &cwd, &options, &|| false);
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
