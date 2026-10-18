//! git 状态快照(07 篇照抄点 21;SVN/tortoise 臂已砍)。
//!
//! 执行体与 `crate::git` 共用 `slterm_app::git::command` 子进程构造口
//! (D07-1 已裁「git 子进程统一」):每条命令都带 `--no-optional-locks`
//! (边界条 7),`-c safe.directory=<root>` 只作用于本次调用——宿主上浏览
//! 别的用户的仓库(如 UNC)不会因 dubious ownership 静默空白。
//!
//! 状态解析用 `--porcelain=v2 --branch -z`:文件名不是展示文本,不能按行
//! 或空格拆分;rename 的原始路径单独占一条 NUL 记录。

use std::path::Path;
use std::process::Command;

use super::{GitCommit, GitInfo};

/// `git status` 参数(NUL 分隔 + 分支头,解析见 [`parse_status`])。
pub(crate) const STATUS_ARGS: &[&str] = &["status", "--porcelain=v2", "--branch", "-z"];

#[derive(Clone, Debug, Default)]
pub(crate) struct GitStatus {
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<GitEntry>,
}

#[derive(Clone, Debug)]
pub(crate) struct GitEntry {
    pub path: String,
    pub original: Option<String>,
    pub index: char,
    pub worktree: char,
    pub conflict: bool,
}

impl GitEntry {
    pub(crate) fn staged(&self) -> bool {
        !matches!(self.index, '.' | '?')
    }

    pub(crate) fn unstaged(&self) -> bool {
        self.worktree != '.'
    }
}

/// 解析 `git status --porcelain=v2 --branch -z` 输出。
pub(crate) fn parse_status(output: &str) -> Result<GitStatus, &'static str> {
    if !output.ends_with('\0') {
        return Err("git_invalid_status");
    }
    let mut result = GitStatus::default();
    let mut records = output.split_terminator('\0');
    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            if let Some(value) = header.strip_prefix("branch.head ") {
                result.branch = value.into();
            } else if let Some(value) = header.strip_prefix("branch.oid ") {
                result.head = value.into();
            } else if let Some(value) = header.strip_prefix("branch.upstream ") {
                result.upstream = Some(value.into());
            } else if let Some(value) = header.strip_prefix("branch.ab ") {
                let (ahead, behind) = value.split_once(' ').ok_or("git_invalid_status")?;
                result.ahead = ahead
                    .strip_prefix('+')
                    .ok_or("git_invalid_status")?
                    .parse()
                    .map_err(|_| "git_invalid_status")?;
                result.behind = behind
                    .strip_prefix('-')
                    .ok_or("git_invalid_status")?
                    .parse()
                    .map_err(|_| "git_invalid_status")?;
            }
            continue;
        }
        if record.starts_with("! ") {
            continue;
        }
        let kind = record
            .as_bytes()
            .first()
            .copied()
            .ok_or("git_invalid_status")?;
        let entry = if kind == b'?' {
            GitEntry {
                path: record
                    .strip_prefix("? ")
                    .ok_or("git_invalid_status")?
                    .into(),
                original: None,
                index: '?',
                worktree: '?',
                conflict: false,
            }
        } else {
            let count = match kind {
                b'1' => 9,
                b'2' => 10,
                b'u' => 11,
                _ => return Err("git_invalid_status"),
            };
            let fields: Vec<_> = record.splitn(count, ' ').collect();
            if fields.len() != count || fields[1].len() != 2 || !fields[1].is_ascii() {
                return Err("git_invalid_status");
            }
            let xy = fields[1].as_bytes();
            GitEntry {
                path: fields[count - 1].into(),
                original: if kind == b'2' {
                    Some(records.next().ok_or("git_invalid_status")?.into())
                } else {
                    None
                },
                index: xy[0] as char,
                worktree: xy[1] as char,
                conflict: kind == b'u',
            }
        };
        if entry.path.is_empty() || entry.original.as_ref().is_some_and(|path| path.is_empty()) {
            return Err("git_invalid_status");
        }
        result.entries.push(entry);
    }
    if result.branch.is_empty() || result.head.is_empty() {
        return Err("git_invalid_status");
    }
    Ok(result)
}

/// 采集 `root` 的 git 快照;`None` = git 缺失或不在 work tree 内。
///
/// 后台快照线程调用(同步子进程);渲染线程零调用。
pub(crate) fn read_git(root: &Path) -> Option<GitInfo> {
    let safe_directory = format!("safe.directory={}", root.display());
    collect_git_info(|args| {
        let mut command = crate::git::command(root);
        command.args(["-c", &safe_directory]).args(args);
        run_git(command, args, root)
    })
}

/// 跑一条已配好的 git 命令,取 stdout;失败记日志并回 `None`。
fn run_git(mut command: Command, args: &[&str], root: &Path) -> Option<String> {
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            log::warn!(
                "git {:?} failed in {}: {error}",
                args.first(),
                root.display()
            );
            return None;
        }
    };
    if !output.status.success() {
        // 留痕而不是静默空白:stderr 首行才说得清真实拒绝原因。
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().next().unwrap_or("unknown error");
        log::debug!(
            "git {:?} failed in {}: {reason}",
            args.first(),
            root.display()
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// porcelain 解析 + 行统计 + 提交历史。
fn collect_git_info(run: impl Fn(&[&str]) -> Option<String>) -> Option<GitInfo> {
    let status = parse_status(&run(STATUS_ARGS)?).ok()?;
    let mut info = GitInfo {
        branch: status.branch,
        ahead: status.ahead,
        ..GitInfo::default()
    };
    for entry in status.entries {
        if entry.conflict {
            info.conflicts.push(('U', entry.path.clone()));
        }
        // 部分暂存的文件会同时出现在两组;路径始终用 Git 的原始身份。
        if entry.staged() {
            info.staged.push((entry.index, entry.path.clone()));
        }
        if entry.unstaged() {
            info.unstaged.push((entry.worktree, entry.path));
        }
    }

    // `3 files changed, 140 insertions(+), 69 deletions(-)` → (140, 69).
    if let Some(stat) = run(&["diff", "--shortstat", "HEAD"]) {
        for part in stat.split(',') {
            let number: u64 = part
                .trim()
                .split(' ')
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            if part.contains("insertion") {
                info.plus = number;
            } else if part.contains("deletion") {
                info.minus = number;
            }
        }
    }
    info.history = run(&[
        "log",
        "--all",
        "--date-order",
        "--decorate=full",
        "--max-count=128",
        "--format=%x1e%H%x1f%h%x1f%D%x1f%s%x1f%an%x1f%ct%x1f%P",
    ])
    .map(|output| parse_git_history(&output))
    .unwrap_or_default();
    Some(info)
}

/// 解析 `%x1e` 记录起点、`%x1f` 分字段的历史格式。
pub(crate) fn parse_git_history(output: &str) -> Vec<GitCommit> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            let record_at = line.find('\x1e')?;
            let mut fields = line[record_at + 1..].split('\x1f');
            let full_hash = fields.next()?.to_owned();
            let short_hash = fields.next()?.to_owned();
            let decorations = fields.next().unwrap_or_default().trim().to_owned();
            let subject = fields.next().unwrap_or_default().to_owned();
            let author = fields.next().unwrap_or_default().to_owned();
            let timestamp = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let parent_hashes = fields
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            Some(GitCommit {
                full_hash,
                short_hash,
                decorations,
                subject,
                author,
                timestamp,
                parent_hashes,
            })
        })
        .collect()
}

#[cfg(test)]
mod vcs_tests {
    use super::*;

    /// rename 身份、部分暂存与中文/换行文件名都按 NUL 记录原样保留。
    #[test]
    fn preserves_rename_identity_and_partial_staging() {
        let status = parse_status(
            "# branch.oid abc\0# branch.head main\0# branch.ab +2 -1\0\
             2 RM N... 100644 100644 100644 abc def R100 新 name\n.txt\0old name.txt\0? -new.txt\0",
        )
        .expect("合法 porcelain v2");
        assert_eq!(status.branch, "main");
        assert_eq!(status.ahead, 2);
        assert_eq!(status.behind, 1);
        assert_eq!(status.entries.len(), 2);
        let renamed = &status.entries[0];
        assert_eq!(renamed.path, "新 name\n.txt");
        assert_eq!(renamed.original.as_deref(), Some("old name.txt"));
        assert!(renamed.staged() && renamed.unstaged());
        assert_eq!(status.entries[1].path, "-new.txt");
    }

    /// 截断/坏记录必须整体拒绝,不能产出半份状态。
    #[test]
    fn rejects_truncated_or_malformed_status() {
        assert!(parse_status("# branch.oid abc\0# branch.head main").is_err());
        assert!(parse_status("? file\0").is_err(), "缺分支头");
        assert!(parse_status("# branch.oid abc\0# branch.head main\0? \0").is_err());
    }

    /// 提交历史:对象 ID/短哈希/装饰/主题/作者/时间/父提交逐字段保留。
    #[test]
    fn history_parser_keeps_commit_identity_parents_and_fields() {
        let output = concat!(
            "\u{1e}aaaaaaaa\u{1f}a1b2c3d\u{1f}HEAD -> refs/heads/main, tag: refs/tags/v1\u{1f}ship it\u{1f}Alice\u{1f}1700000000\u{1f}1111111 2222222\n",
            "\u{1e}dddddddd\u{1f}d4e5f6a\u{1f}refs/heads/topic\u{1f}side work\u{1f}Bob\u{1f}1699999000\u{1f}3333333\n",
        );
        let commits = parse_git_history(output);
        assert_eq!(commits.len(), 2);
        let first = &commits[0];
        assert_eq!(first.full_hash, "aaaaaaaa");
        assert_eq!(first.short_hash, "a1b2c3d");
        assert_eq!(
            first.decorations,
            "HEAD -> refs/heads/main, tag: refs/tags/v1"
        );
        assert_eq!(first.subject, "ship it");
        assert_eq!(first.author, "Alice");
        assert_eq!(first.timestamp, 1_700_000_000);
        assert_eq!(first.parent_hashes, ["1111111", "2222222"]);
        assert_eq!(commits[1].subject, "side work");
    }

    /// 真仓库快照:分支、未跟踪/已暂存分组;非仓库回 None。
    #[test]
    fn read_git_snapshots_branch_and_changes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        run_git(root, &["init", "--initial-branch=main"]);
        std::fs::write(root.join("tracked.txt"), b"one\n").unwrap();
        run_git(root, &["add", "tracked.txt"]);
        run_git(
            root,
            &[
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "initial",
            ],
        );
        std::fs::write(root.join("tracked.txt"), b"two\n").unwrap();
        std::fs::write(root.join("new.txt"), b"new\n").unwrap();

        let info = read_git(root).expect("仓库根必须产出快照");
        assert_eq!(info.branch, "main");
        assert!(
            info.unstaged
                .iter()
                .any(|(status, path)| *status == 'M' && path == "tracked.txt"),
            "工作区修改进入 unstaged: {:?}",
            info.unstaged
        );
        assert!(
            info.unstaged
                .iter()
                .any(|(status, path)| *status == '?' && path == "new.txt"),
            "未跟踪文件进入 unstaged: {:?}",
            info.unstaged
        );
        assert_eq!(
            info.history.first().map(|commit| commit.subject.as_str()),
            Some("initial")
        );

        let outside = tempfile::tempdir().unwrap();
        assert!(read_git(outside.path()).is_none(), "非仓库根必须回 None");
    }

    fn run_git(root: &Path, args: &[&str]) {
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(root).args(args);
        crate::platform::process::hidden_command(&mut command);
        let output = command.output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} 失败: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
