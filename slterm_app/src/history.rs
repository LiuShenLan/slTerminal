//! fish 式本地命令历史:jsonl 追加 + `BTreeMap` 前缀范围索引。
//!
//! 单进程本机形态下历史只有本地单池:远程作用域随定位
//! 约束整砍,`HistoryScope` 仅保留 `Local`。池容器仍保留 `HashMap` 形态,
//! 为未来若引入新的执行环境留出分池位;当前 `normalized()` 塌缩为单值。
//!
//! 数据流:提交命令(Enter)后 `record` 追加一行 `{ts,cwd,cmd}`;ghost 走
//! `hint` 的 `BTreeMap` 前缀 range(开销随命中数而非历史总量增长),popup 走
//! `search_with_cancel` 的 `CommandQuery` 词级评分。扫描期间不持有外部锁。

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 单池上限;超出后淘汰最旧条目。
const HISTORY_MAX: usize = 5_000;

/// 本地历史文件名(改名映射)。
pub(crate) const HISTORY_FILE: &str = "history.jsonl";

/// 历史作用域。远程变体已随源收敛整砍,仅剩本地单态。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum HistoryScope {
    #[default]
    Local,
}

impl HistoryScope {
    fn normalized(self) -> Self {
        Self::Local
    }

    fn file_name(&self) -> &'static str {
        HISTORY_FILE
    }
}

#[derive(Debug, Default)]
struct HistoryIndex {
    entries: Vec<String>,
    index: BTreeMap<String, usize>,
}

impl HistoryIndex {
    /// 取最新一条以 `prefix` 开头的命令余量(Inline ghost)。
    fn hint_with_cancel(&self, prefix: &str, cancelled: &dyn Fn() -> bool) -> Option<&str> {
        if prefix.is_empty() {
            return None;
        }
        let mut best: Option<(usize, &str)> = None;
        for (cmd, &pos) in self.index.range(prefix.to_owned()..) {
            if cancelled() {
                return None;
            }
            if !cmd.starts_with(prefix) {
                break;
            }
            if cmd.len() == prefix.len() {
                continue;
            }
            if best.is_none_or(|(best_pos, _)| pos > best_pos) {
                best = Some((pos, &cmd[prefix.len()..]));
            }
        }
        best.map(|(_, remainder)| remainder)
    }

    fn insert(&mut self, cmd: String) {
        if let Some(&old) = self.index.get(&cmd) {
            self.entries.remove(old);
            self.entries.push(cmd);
            self.reindex();
            return;
        }
        self.entries.push(cmd.clone());
        self.index.insert(cmd, self.entries.len() - 1);
        if self.entries.len() > HISTORY_MAX {
            let drop = self.entries.len() - HISTORY_MAX;
            self.entries.drain(0..drop);
            self.reindex();
        }
    }

    fn reindex(&mut self) {
        self.index.clear();
        for (position, command) in self.entries.iter().enumerate() {
            self.index.insert(command.clone(), position);
        }
    }
}

#[derive(Debug, Default)]
pub struct SltermHistory {
    /// 单池语义下只有 `Local` 一个键;保留分池容器以备未来执行环境扩展。
    pools: HashMap<HistoryScope, HistoryIndex>,
}

impl SltermHistory {
    /// 只读本地 `history.jsonl`;文件缺失/损坏行静默跳过,历史不是启动硬依赖。
    pub(crate) fn load() -> Self {
        Self::load_from(&history_path())
    }

    fn load_from(path: &Path) -> Self {
        let mut history = Self::default();
        let Ok(data) = std::fs::read_to_string(path) else {
            return history;
        };
        for line in data.lines() {
            let Some((scope, cmd)) = parse_record(line) else {
                continue;
            };
            history.pools.entry(scope).or_default().insert(cmd);
        }
        history
    }

    /// 提交一条命令:连续重复去重,jsonl 追加,内存索引前移。
    pub(crate) fn record(&mut self, scope: &HistoryScope, cmd: &str, cwd: &str) {
        let scope = scope.clone().normalized();
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }
        let pool = self.pools.entry(scope.clone()).or_default();
        if pool.entries.last().map(String::as_str) == Some(cmd) {
            return;
        }
        append(&scope, cmd, cwd);
        pool.insert(cmd.to_owned());
    }

    /// 只写内存池的测试入口:避免测试污染真实 `history.jsonl`。
    #[cfg(test)]
    pub(crate) fn insert_for_test(&mut self, scope: &HistoryScope, cmd: &str) {
        self.pools
            .entry(scope.clone().normalized())
            .or_default()
            .insert(cmd.to_owned());
    }

    /// Inline 前缀余量:最新一条以 `prefix` 开头的命令的后缀。
    pub(crate) fn hint(&self, scope: &HistoryScope, prefix: &str) -> Option<&str> {
        self.hint_with_cancel(scope, prefix, &|| false)
    }

    pub(crate) fn hint_with_cancel(
        &self,
        scope: &HistoryScope,
        prefix: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<&str> {
        self.pools
            .get(&scope.clone().normalized())?
            .hint_with_cancel(prefix, cancelled)
    }

    /// popup 搜索:保留存储拼写,同分按新旧排序;取消即弃部分结果。
    pub(crate) fn search_with_cancel(
        &self,
        scope: &HistoryScope,
        text: &str,
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Vec<&str> {
        if text.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(pool) = self.pools.get(&scope.clone().normalized()) else {
            return Vec::new();
        };
        let mut query = slterm_completions::command_search::CommandQuery::new(text);
        let mut matches: Vec<_> = pool
            .entries
            .iter()
            .enumerate()
            .take_while(|_| !cancelled())
            .filter_map(|(index, command)| {
                (command != text)
                    .then(|| query.score(command))
                    .flatten()
                    .map(|score| (score, index, command.as_str()))
            })
            .collect();
        if cancelled() {
            return Vec::new();
        }
        matches.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        matches
            .into_iter()
            .take(limit)
            .map(|(_, _, command)| command)
            .collect()
    }
}

fn history_path() -> PathBuf {
    crate::platform::dirs::data_dir().join(HISTORY_FILE)
}

fn append(scope: &HistoryScope, cmd: &str, cwd: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let record = serde_json::json!({ "ts": ts, "cwd": cwd, "cmd": cmd });
    let path = history_path_for(scope);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{record}");
    }
}

fn history_path_for(scope: &HistoryScope) -> PathBuf {
    crate::platform::dirs::data_dir().join(scope.file_name())
}

/// 单池记录校验:无 `scope` 或 `scope=local` 才属于本地池;远程残留记录拒绝。
///
/// 用路径而非内联条件,`load_from` 才能在没有数据目录的测试里复用同一校验。
fn parse_record(line: &str) -> Option<(HistoryScope, String)> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let cmd = value.get("cmd")?.as_str()?.trim();
    if cmd.is_empty() {
        return None;
    }
    match value.get("scope").and_then(serde_json::Value::as_str) {
        None | Some("local") => Some((HistoryScope::Local, cmd.to_owned())),
        // 远程记录不得在单池里复活;未知作用域同样拒绝。
        Some(_) => None,
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    fn hist(cmds: &[&str]) -> SltermHistory {
        let mut history = SltermHistory::default();
        let pool = history.pools.entry(HistoryScope::Local).or_default();
        for command in cmds {
            pool.insert((*command).to_owned());
        }
        history
    }

    #[test]
    fn hint_returns_remainder_of_newest_match() {
        let history = hist(&["cargo build", "cargo test", "git status"]);
        assert_eq!(history.hint(&HistoryScope::Local, "cargo "), Some("test"));
        assert_eq!(history.hint(&HistoryScope::Local, "git "), Some("status"));
    }

    #[test]
    fn exact_missing_and_empty_prefixes_have_no_hint() {
        let history = hist(&["cargo build"]);
        assert_eq!(history.hint(&HistoryScope::Local, "cargo build"), None);
        assert_eq!(history.hint(&HistoryScope::Local, "npm "), None);
        assert_eq!(history.hint(&HistoryScope::Local, ""), None);
    }

    #[test]
    fn rerunning_a_command_moves_it_to_the_newest_position() {
        let history = hist(&["ls", "ll", "ls"]);
        assert_eq!(history.hint(&HistoryScope::Local, "l"), Some("s"));
    }

    #[test]
    fn prefix_ranges_do_not_bleed_into_adjacent_commands() {
        let history = hist(&["git push", "gitk"]);
        assert_eq!(history.hint(&HistoryScope::Local, "git "), Some("push"));
    }

    #[test]
    fn popup_search_ignores_case_and_orders_by_score() {
        let history = hist(&["Docker compose up", "docker compose logs", "git status"]);
        assert_eq!(
            history.search_with_cancel(&HistoryScope::Local, "COMPOSE DOCKER", 8, &|| false),
            vec!["docker compose logs", "Docker compose up"]
        );
        assert_eq!(
            history.search_with_cancel(&HistoryScope::Local, "dck cmp", 1, &|| false),
            vec!["docker compose logs"]
        );
        assert!(
            history
                .search_with_cancel(&HistoryScope::Local, "remote-only", 8, &|| false)
                .is_empty()
        );
    }

    #[test]
    fn cancellation_stops_history_matching_before_traversing_the_whole_pool() {
        let commands: Vec<_> = (0..100).map(|i| format!("git switch branch-{i}")).collect();
        let history = hist(&commands.iter().map(String::as_str).collect::<Vec<_>>());
        for popup in [false, true] {
            let checks = std::cell::Cell::new(0);
            let cancelled = || {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            };
            if popup {
                assert!(
                    history
                        .search_with_cancel(&HistoryScope::Local, "git", 8, &cancelled)
                        .is_empty()
                );
            } else {
                assert!(
                    history
                        .hint_with_cancel(&HistoryScope::Local, "git", &cancelled)
                        .is_none()
                );
            }
            assert!(
                checks.get() <= 4,
                "old input must not scan the remaining history"
            );
        }
        assert_eq!(
            history.hint(&HistoryScope::Local, "git switch "),
            Some("branch-99")
        );
    }

    #[test]
    fn duplicate_submission_is_skipped_and_pool_is_bounded() {
        let mut history = SltermHistory::default();
        // `record` 会写数据目录;测试只关心内存池语义,先放一条再走重复分支。
        let pool = history.pools.entry(HistoryScope::Local).or_default();
        pool.insert("echo one".to_owned());
        pool.insert("echo one".to_owned());
        assert_eq!(pool.entries.len(), 1);
        for index in 0..HISTORY_MAX + 10 {
            pool.insert(format!("echo {index}"));
        }
        assert_eq!(pool.entries.len(), HISTORY_MAX);
        assert_eq!(pool.entries.last().map(String::as_str), Some("echo 5009"));
    }

    #[test]
    fn load_reads_local_records_and_rejects_remote_scope_residue() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(HISTORY_FILE);
        std::fs::write(
            &path,
            concat!(
                r#"{"ts":1,"cwd":"D:/","cmd":"git status"}"#,
                "\n",
                r#"{"ts":2,"cwd":"D:/","cmd":"cargo test"}"#,
                "\n",
                r#"{"ts":3,"cwd":"/","cmd":"sudo apt update","scope":"wsl"}"#,
                "\n",
                r#"{"ts":4,"cwd":"/","cmd":"systemctl status","scope":"ssh"}"#,
                "\n",
                "not json\n"
            ),
        )
        .unwrap();
        let history = SltermHistory::load_from(&path);
        let pool = history.pools.get(&HistoryScope::Local).unwrap();
        assert_eq!(pool.entries, vec!["git status", "cargo test"]);
        assert_eq!(history.hint(&HistoryScope::Local, "git "), Some("status"));
        assert_eq!(history.hint(&HistoryScope::Local, "sudo "), None);
    }
}
