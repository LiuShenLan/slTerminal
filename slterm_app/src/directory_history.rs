//! 本机目录智能:frecency 评分(排名 × 四档时间衰减)。
//!
//! 与命令文本启发式不同,本服务只记录本机终端**真实启动或上报过的目录**
//! (shell cwd 集成 / 显式记录),不引入任何 shell 特定命令或别名。评分同时
//! 服务 ghost 余量、路径补全排序与未来的目录选择器。
//!
//! 持久化只经 `LearningService` 的独立 SQLite 写队列；本文件不读旧 JSON。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_TOTAL_RANK: f64 = 10_000.0;
const MAX_ENTRIES: usize = 2_048;
const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

#[derive(Clone, Default)]
pub(crate) struct DirectoryHistory {
    database: DirectoryDatabase,
}

#[derive(Debug, Clone)]
struct Observation {
    path: String,
    normalized: String,
    at: u64,
}

#[derive(Debug, Clone, Default)]
struct DirectoryDatabase {
    entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone)]
struct DirectoryEntry {
    path: String,
    rank: f64,
    last_accessed: u64,
}

impl DirectoryEntry {
    fn score(&self, now: u64) -> f64 {
        let age = now.saturating_sub(self.last_accessed);
        let recency = if age < HOUR {
            4.0
        } else if age < DAY {
            2.0
        } else if age < WEEK {
            0.5
        } else {
            0.25
        };
        self.rank * recency
    }
}

impl DirectoryHistory {
    /// 空的内存实例，供补全测试使用。
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// 从学习库恢复快照；不读取旧 JSON 文件。
    pub(crate) fn from_learning(
        records: Vec<crate::persistence::learning::DirectoryRecord>,
    ) -> Self {
        let entries = records
            .into_iter()
            .filter(|entry| {
                entry.last_used_at_ms >= 0
                    && entry.rank.is_finite()
                    && entry.rank > 0.0
                    && normalize(&entry.path).is_some()
            })
            .map(|entry| DirectoryEntry {
                path: entry.path,
                rank: entry.rank,
                last_accessed: (entry.last_used_at_ms / 1_000) as u64,
            })
            .collect();
        Self {
            database: DirectoryDatabase { entries },
        }
    }

    /// 在不改变内存的前提下词法校验受管目录并生成 SQLite 规范键。
    pub(crate) fn normalize_observation(path: &str) -> Option<String> {
        observation_at(path, 0).map(|observation| observation.normalized)
    }

    /// 只更新当前内存快照，返回用于 SQLite 去重的规范路径。
    pub(crate) fn record_memory_at(&mut self, path: &str, at: u64) -> Option<String> {
        let observation = observation_at(path, at)?;
        let normalized = observation.normalized.clone();
        self.database.apply(&observation);
        self.database.age_and_limit(at);
        Some(normalized)
    }

    /// 记录一个经本机终端确认的目录;控制字符/非法编码返回 false。
    #[cfg(test)]
    pub(crate) fn record(&mut self, path: &str) -> bool {
        self.record_memory_at(path, current_time()).is_some()
    }

    /// cd 类命令的 frecency 余量(Inline ghost 数据源)。
    pub(crate) fn hint(&self, line: &str, cwd: &str) -> Option<String> {
        self.hint_with_cancel(line, cwd, &|| false)
    }

    pub(crate) fn hint_with_cancel(
        &self,
        line: &str,
        cwd: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<String> {
        if cancelled() {
            return None;
        }
        let request = parse_directory_request(line, cwd)?;
        let request_key = normalize(&request.absolute)?;
        let now = current_time();
        let mut candidates: Vec<&DirectoryEntry> = self
            .database
            .entries
            .iter()
            .filter(|entry| {
                normalize(&entry.path).is_some_and(|key| {
                    key.len() > request_key.len() && key.starts_with(&request_key)
                })
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .score(now)
                .total_cmp(&left.score(now))
                .then_with(|| left.path.cmp(&right.path))
        });

        // 只复制最多 12 条路径；后续慢盘元数据查询不再触碰学习服务状态。
        let candidates: Vec<_> = candidates
            .into_iter()
            .take(12)
            .map(|entry| entry.path.clone())
            .collect();
        candidates
            .into_iter()
            .take_while(|_| !cancelled())
            .find_map(|path| {
                if !Path::new(&path).is_dir() {
                    return None;
                }
                let suffix = suffix_after_prefix(&path, &request.absolute)?;
                Some(apply_separator_style(suffix, request.separator))
            })
    }

    /// 精确目录或「包含已访问后代的目录」的 frecency 分。逐段补全时,高频
    /// 项目可抬起其父目录候选。
    pub(crate) fn score(&self, path: &Path) -> Option<f64> {
        let key = normalize_path(path)?;
        let now = current_time();
        self.database.score_path_key(&key, now)
    }

    /// 通用 UI 候选:有序关键词匹配(不引入产品特定 shell 命令)。
    pub(crate) fn search(&self, query: &str, limit: usize) -> Vec<PathBuf> {
        let keywords: Vec<String> = query.split_whitespace().filter_map(normalize).collect();
        let now = current_time();
        let mut entries: Vec<&DirectoryEntry> = self
            .database
            .entries
            .iter()
            .filter(|entry| ordered_keyword_match(&entry.path, &keywords))
            .collect();
        entries.sort_by(|left, right| {
            right
                .score(now)
                .total_cmp(&left.score(now))
                .then_with(|| left.path.cmp(&right.path))
        });
        entries
            .into_iter()
            .filter(|entry| Path::new(&entry.path).is_dir())
            .take(limit)
            .map(|entry| PathBuf::from(&entry.path))
            .collect()
    }

    /// 只重排既有的文件系统补全,不改变匹配集合;未访问候选保持引擎稳定序。
    pub(crate) fn rank_file_suggestions(
        &self,
        suggestions: Vec<slterm_completions::file::FileSuggestion>,
        cwd: &str,
    ) -> Vec<slterm_completions::file::FileSuggestion> {
        let now = current_time();
        let mut scored: Vec<_> = suggestions
            .into_iter()
            .enumerate()
            .map(|(order, suggestion)| {
                let display = suggestion
                    .display_override
                    .as_deref()
                    .unwrap_or(&suggestion.path);
                let score = resolve_candidate(display, cwd)
                    .and_then(|path| self.database.score_path(&path, now));
                (score, order, suggestion)
            })
            .collect();
        scored.sort_by(|left, right| match (left.0, right.0) {
            (Some(left_score), Some(right_score)) => right_score
                .total_cmp(&left_score)
                .then_with(|| left.1.cmp(&right.1)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.1.cmp(&right.1),
        });
        scored
            .into_iter()
            .map(|(_, _, suggestion)| suggestion)
            .collect()
    }
}

impl DirectoryDatabase {
    fn apply(&mut self, observation: &Observation) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            normalize(&entry.path).as_deref() == Some(observation.normalized.as_str())
        }) {
            entry.path.clone_from(&observation.path);
            entry.rank = (entry.rank + 1.0).max(0.0);
            entry.last_accessed = entry.last_accessed.max(observation.at);
        } else {
            self.entries.push(DirectoryEntry {
                path: observation.path.clone(),
                rank: 1.0,
                last_accessed: observation.at,
            });
        }
    }

    fn age_and_limit(&mut self, now: u64) {
        let total = self.entries.iter().map(|entry| entry.rank).sum::<f64>();
        if total > MAX_TOTAL_RANK {
            let factor = 0.9 * MAX_TOTAL_RANK / total;
            for entry in &mut self.entries {
                entry.rank *= factor;
            }
            self.entries.retain(|entry| entry.rank >= 1.0);
        }
        if self.entries.len() > MAX_ENTRIES {
            self.entries.sort_by(|left, right| {
                right
                    .score(now)
                    .total_cmp(&left.score(now))
                    .then_with(|| left.path.cmp(&right.path))
            });
            self.entries.truncate(MAX_ENTRIES);
        }
    }

    fn score_path(&self, path: &Path, now: u64) -> Option<f64> {
        self.score_path_key(&normalize_path(path)?, now)
    }

    fn score_path_key(&self, key: &str, now: u64) -> Option<f64> {
        self.entries
            .iter()
            .filter_map(|entry| {
                let entry_key = normalize(&entry.path)?;
                (entry_key == *key || is_descendant_key(&entry_key, key)).then(|| entry.score(now))
            })
            .max_by(f64::total_cmp)
    }
}

fn observation(path: &str, at: u64) -> Option<Observation> {
    observation_at(path, at)
}

fn observation_at(path: &str, at: u64) -> Option<Observation> {
    if path.is_empty() || path.chars().any(char::is_control) {
        return None;
    }
    let path = PathBuf::from(path);
    // 采集入口只接受受管 cwd 的词事实，不在 GPUI 前台做 metadata I/O。
    // 后台候选查询仍会复核 is_dir，失效目录不会成为可接受候选。
    // 数据库明确使用 UTF-8;拒绝无法无损编码的本地路径,不能用
    // to_string_lossy 生成一个之后再也无法访问的伪路径。
    let path = trim_trailing_separators(path.to_str()?.to_owned());
    let normalized = normalize(&path)?;
    Some(Observation {
        path,
        normalized,
        at,
    })
}

fn resolve_candidate(candidate: &str, cwd: &str) -> Option<PathBuf> {
    // display_override 已经是未转义的真实候选;再次 trim 引号或空格会
    // 破坏合法文件名。这里只去掉补全器为目录追加的末尾分隔符。
    let candidate = trim_trailing_separators(candidate.to_owned());
    if candidate.is_empty() {
        return None;
    }
    if let Some(rest) = candidate.strip_prefix('~') {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        return Some(home.join(rest.trim_start_matches(['/', '\\'])));
    }
    let path = PathBuf::from(&candidate);
    if path.is_absolute() || candidate.as_bytes().get(1) == Some(&b':') {
        Some(path)
    } else if cwd.is_empty() {
        None
    } else {
        Some(PathBuf::from(cwd).join(path))
    }
}

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn normalize_path(path: &Path) -> Option<String> {
    normalize(trim_trailing_separators(path.display().to_string()))
}

fn normalize(value: impl AsRef<str>) -> Option<String> {
    let value = value.as_ref();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    // Windows 定位约束下规范化单一形态:分隔符统一 + 大小写折叠。
    let value = value.replace('/', "\\").to_ascii_lowercase();
    Some(trim_trailing_separators(value))
}

fn trim_trailing_separators(mut path: String) -> String {
    while path.len() > root_len(&path) && path.ends_with(['/', '\\']) {
        path.pop();
    }
    path
}

fn root_len(path: &str) -> usize {
    if path.as_bytes().get(1) == Some(&b':') {
        3.min(path.len())
    } else if path.starts_with(['/', '\\']) {
        1
    } else {
        0
    }
}

fn is_descendant_key(path: &str, parent: &str) -> bool {
    path.strip_prefix(parent)
        .and_then(|rest| rest.chars().next())
        .is_some_and(std::path::is_separator)
}

fn suffix_after_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let byte = path
        .char_indices()
        .nth(prefix.chars().count())
        .map_or(path.len(), |(index, _)| index);
    path.get(byte..).filter(|suffix| !suffix.is_empty())
}

fn ordered_keyword_match(path: &str, keywords: &[String]) -> bool {
    let Some((last, previous)) = keywords.split_last() else {
        return true;
    };
    let Some(path) = normalize(path) else {
        return false;
    };
    let Some(last_index) = path.rfind(last) else {
        return false;
    };
    if path[last_index + last.len()..].contains(std::path::is_separator) {
        return false;
    }
    let mut head = &path[..last_index];
    for keyword in previous.iter().rev() {
        let Some(index) = head.rfind(keyword) else {
            return false;
        };
        head = &head[..index];
    }
    true
}

#[derive(Debug)]
struct DirectoryRequest {
    absolute: String,
    separator: Option<char>,
}

fn parse_directory_request(line: &str, cwd: &str) -> Option<DirectoryRequest> {
    let line = line.trim_start();
    let (command, rest) = split_first_token(line)?;
    if !matches!(
        command.to_ascii_lowercase().as_str(),
        "cd" | "chdir" | "pushd" | "sl" | "set-location"
    ) {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with('-') {
        return None;
    }
    let (target, _) = split_first_token(rest)?;
    let separator = target.chars().rev().find(|ch| matches!(ch, '/' | '\\'));
    let absolute = resolve_request_path(target, cwd)?;
    Some(DirectoryRequest {
        absolute,
        separator,
    })
}

fn resolve_request_path(target: &str, cwd: &str) -> Option<String> {
    if target.is_empty()
        || target == "-"
        || target.starts_with(['$', '%'])
        || target.contains(['*', '?'])
    {
        return None;
    }
    if let Some(rest) = target.strip_prefix('~') {
        if !rest.is_empty() && !rest.starts_with(['/', '\\']) {
            return None;
        }
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        return Some(trim_trailing_separators(
            home.join(rest.trim_start_matches(['/', '\\']))
                .display()
                .to_string(),
        ));
    }
    if Path::new(target).is_absolute() || target.as_bytes().get(1) == Some(&b':') {
        return Some(trim_trailing_separators(target.to_owned()));
    }
    if cwd.is_empty() {
        return None;
    }
    Some(trim_trailing_separators(
        PathBuf::from(cwd).join(target).display().to_string(),
    ))
}

fn split_first_token(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    if matches!(first, '"' | '\'' | '`') {
        let start = first.len_utf8();
        for (index, ch) in chars {
            if ch == first {
                return Some((&input[start..index], &input[index + ch.len_utf8()..]));
            }
        }
        return Some((&input[start..], ""));
    }
    for (index, ch) in input.char_indices() {
        if ch.is_whitespace() {
            return Some((&input[..index], &input[index..]));
        }
    }
    Some((input, ""))
}

fn apply_separator_style(value: &str, separator: Option<char>) -> String {
    let Some(separator) = separator else {
        return value.to_owned();
    };
    value
        .chars()
        .map(|ch| {
            if matches!(ch, '/' | '\\') {
                separator
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod directory_history_tests {
    use super::*;

    fn history(entries: &[(&str, f64, u64)]) -> DirectoryHistory {
        DirectoryHistory {
            database: DirectoryDatabase {
                entries: entries
                    .iter()
                    .map(|(path, rank, last_accessed)| DirectoryEntry {
                        path: (*path).to_owned(),
                        rank: *rank,
                        last_accessed: *last_accessed,
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn directory_hint_uses_snapshot_without_shared_mutable_state() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("candidate");
        std::fs::create_dir(&target).unwrap();
        let history = history(&[(target.to_str().unwrap(), 1.0, current_time())]);
        let result = history.hint_with_cancel("cd ca", temp.path().to_str().unwrap(), &|| false);
        assert!(result.is_some());
        assert!(
            history
                .hint_with_cancel("cd ca", temp.path().to_str().unwrap(), &|| true)
                .is_none()
        );
    }

    #[test]
    fn frequency_and_recency_both_affect_score() {
        let now = 10 * WEEK;
        let frequent = DirectoryEntry {
            path: "frequent".into(),
            rank: 20.0,
            last_accessed: 0,
        };
        let recent = DirectoryEntry {
            path: "recent".into(),
            rank: 4.0,
            last_accessed: now - HOUR / 2,
        };
        assert_eq!(frequent.score(now), 5.0);
        assert_eq!(recent.score(now), 16.0);
    }

    #[test]
    fn aging_bounds_total_rank_and_removes_noise() {
        let mut database = DirectoryDatabase {
            entries: vec![
                DirectoryEntry {
                    path: "a".into(),
                    rank: 10_000.0,
                    last_accessed: 0,
                },
                DirectoryEntry {
                    path: "b".into(),
                    rank: 1.0,
                    last_accessed: 0,
                },
            ],
        };
        database.age_and_limit(0);
        assert!(database.entries.iter().map(|entry| entry.rank).sum::<f64>() < MAX_TOTAL_RANK);
        assert_eq!(database.entries.len(), 1, "low-signal entry ages out");
    }

    #[test]
    fn ordered_keywords_constrain_the_last_path_component() {
        assert!(ordered_keyword_match(
            "D:/temp_build/slterm_app",
            &["temp".into(), "app".into()]
        ));
        assert!(!ordered_keyword_match("D:/app/slterm/src", &["app".into()]));
    }

    #[test]
    fn parent_score_inherits_a_visited_descendant() {
        let now = current_time();
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("workspace").join("project");
        std::fs::create_dir_all(&project).unwrap();
        let history = history(&[(project.to_str().unwrap(), 5.0, now)]);
        assert!(history.score(&temp.path().join("workspace")).is_some());
        assert!(history.score(&temp.path().join("work")).is_none());
    }

    #[test]
    fn hint_uses_frecency_instead_of_insertion_order() {
        let now = current_time();
        let temp = tempfile::tempdir().unwrap();
        let older = temp.path().join("team-old");
        let common = temp.path().join("team-common");
        std::fs::create_dir(&older).unwrap();
        std::fs::create_dir(&common).unwrap();
        let history = history(&[
            (older.to_str().unwrap(), 1.0, now),
            (common.to_str().unwrap(), 10.0, now - DAY),
        ]);
        let prefix = temp.path().join("team-").display().to_string();
        let line = format!("cd {prefix}");
        assert_eq!(history.hint(&line, "").as_deref(), Some("common"));
    }

    #[test]
    fn record_accepts_utf8_spaces_and_rejects_unsafe_paths_without_fs_probes() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("项目 空间");
        let file = temp.path().join("plain-file");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(&file, b"file").unwrap();
        let mut history = history(&[]);

        assert!(history.record(directory.to_str().unwrap()));
        assert!(history.score(&directory).is_some());
        assert_eq!(history.search("项目", 8), vec![directory]);
        assert!(
            history.record(file.to_str().unwrap()),
            "采集入口不得为验证目录而同步读文件系统"
        );
        assert!(!history.record("bad\npath"));
    }

    #[test]
    fn relative_directory_hint_resolves_against_reported_cwd() {
        let now = current_time();
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("项目 空间");
        std::fs::create_dir(&project).unwrap();
        let history = history(&[(project.to_str().unwrap(), 5.0, now)]);

        assert_eq!(
            history
                .hint("cd 项", temp.path().to_str().unwrap())
                .as_deref(),
            Some("目 空间")
        );
    }

    fn suggestion(path: &str) -> slterm_completions::file::FileSuggestion {
        slterm_completions::file::FileSuggestion {
            span: slterm_completions::Span::new(0, path.len()),
            path: path.to_owned(),
            is_dir: true,
            display_override: None,
            match_indices: Vec::new(),
        }
    }

    #[test]
    fn completion_ranking_promotes_visited_paths_and_keeps_unvisited_order() {
        let now = current_time();
        let temp = tempfile::tempdir().unwrap();
        for name in ["alpha", "visited", "zeta"] {
            std::fs::create_dir(temp.path().join(name)).unwrap();
        }
        let visited = temp.path().join("visited");
        let history = history(&[(visited.to_str().unwrap(), 5.0, now)]);
        let separator = std::path::MAIN_SEPARATOR;
        let ranked = history.rank_file_suggestions(
            vec![
                suggestion(&format!("alpha{separator}")),
                suggestion(&format!("visited{separator}")),
                suggestion(&format!("zeta{separator}")),
            ],
            temp.path().to_str().unwrap(),
        );
        let paths: Vec<_> = ranked.into_iter().map(|item| item.path).collect();

        assert_eq!(
            paths,
            vec![
                format!("visited{separator}"),
                format!("alpha{separator}"),
                format!("zeta{separator}"),
            ]
        );
    }

    #[test]
    fn windows_separator_variants_merge_into_one_directory() {
        if !cfg!(windows) {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().to_str().unwrap();
        let alternate = canonical.replace('\\', "/");
        let mut database = DirectoryDatabase::default();

        database.apply(&observation(canonical, 10).unwrap());
        database.apply(&observation(&alternate, 20).unwrap());
        assert_eq!(database.entries.len(), 1);
        assert_eq!(database.entries[0].rank, 2.0);
        assert_eq!(database.entries[0].last_accessed, 20);
    }
}
