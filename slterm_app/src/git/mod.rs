//! git 执行体 facade(07 篇改造节 2;D07-1 已裁「子进程统一」)。
//!
//! 旧栈 `src-tauri/src/git/` 是 git2(libgit2 vendored)封装;新世界不移植
//! git2,操作面统一走 git CLI 子进程(上游子进程形态)。本步(M8.1)只立只读面:
//! 仓库根解析 + HEAD 文件字节——`gitshow` 只读臂的数据源。status/diff/hunks/
//! rollback/unstage 四方法执行归 M8.5 联调;本模块保留统一执行器形状:
//! 单构造口 [`command`],每命令 `--no-optional-locks`(07 篇边界条 7,
//! 禁触发 index 锁与后台 gc),argv 不经 shell,Windows 挂 CREATE_NO_WINDOW。
//!
//! 旧 git2 语义资产(五份因果档案)转测试保留,按步骤迁移:本步迁移
//! **HEAD 不存在错误契约**——`UnbornBranch`(尚无提交)与「文件不在 HEAD
//! tree」统一报 [`GitError::NotInHead`],消息含「HEAD 中不存在」;UI 占位文案
//! 归 06 篇 i18n,不解析具体错误内容。LRU 仓库缓存(BE-09)/不扫 ignored
//! (CP-008)/rename 双检/rollback 三方字节一致归 M8.5 对照用例。

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// git 执行体错误。消息保持用户可读(经 i18n `{error}` 占位展示)。
#[derive(Debug)]
pub(crate) enum GitError {
    /// git 可执行文件缺失或无法启动。
    Unavailable(String),
    /// 路径不在任何 Git worktree 内。
    NotRepository(String),
    /// HEAD 不存在(尚无提交)或文件不在 HEAD tree 中;消息含「HEAD 中不存在」。
    NotInHead(String),
    /// 其他命令失败(M8.5 四方法执行复用此臂)。
    Command(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message)
            | Self::NotRepository(message)
            | Self::NotInHead(message)
            | Self::Command(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for GitError {}

/// 子进程统一构造口:全局 flag 必须位于子命令之前。
///
/// `pub(crate)`:07 篇 side_panel 的 git 状态快照与文件树过滤索引共用本
/// 构造口(D07-1 子进程统一,无第二执行体)。
pub(crate) fn command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("--no-optional-locks").arg("-C").arg(cwd);
    crate::platform::process::hidden_command(&mut command);
    command
}

/// 路径所在 worktree 的根(`git rev-parse --show-toplevel`)。
///
/// 04 篇 `verify_repository(pane_cwd, repo_root)` 的 `repo_root` 数据源:
/// M8.5 的 `git.*` UI 执行器以本函数重查仓库根,与 `expected_cwd` 核对。
pub(crate) fn repository_root(path: &Path) -> Result<PathBuf, GitError> {
    let cwd = if path.is_dir() {
        path
    } else {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    };
    let output = command(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || root.is_empty() {
        return Err(GitError::NotRepository(format!(
            "不在 Git 仓库内: {}",
            cwd.display()
        )));
    }
    Ok(PathBuf::from(root))
}

/// 读取文件在工作树当前 HEAD 中的字节(上限 `limit`,溢出置 truncated)。
///
/// 只读数据面:不做写回,不碰 index。`limit + 1` 探到溢出后截断并终止
/// 子进程,避免把超大 blob 全量读进内存。
pub(crate) fn head_file(path: &Path, limit: usize) -> Result<(Vec<u8>, bool), GitError> {
    let root = repository_root(path)?;
    let relative = relative_to_root(&root, path)?;
    // HEAD 存在性是独立判定:unborn 仓与「文件不在 HEAD tree」在 UI 上
    // 同属占位文案,但前者连 show 的输入都没有。
    verify_head(&root)?;
    let spec = format!("HEAD:{relative}");
    let mut child = command(&root)
        .args(["show", &spec])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitError::Command("无法打开 git show 管道".to_owned()))?;
    let mut bytes = Vec::new();
    stdout
        .by_ref()
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| GitError::Command(format!("读取 HEAD 字节失败: {error}")))?;
    let truncated = bytes.len() > limit;
    if truncated {
        bytes.truncate(limit);
    }
    // 截断时子进程仍可能往管道写:先杀再收尸;stdout 句柄在此前已 drop。
    drop(stdout);
    if truncated {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .map_err(|error| GitError::Command(format!("等待 git show 失败: {error}")))?;
    if !truncated && !output.status.success() {
        return Err(GitError::NotInHead(format!(
            "文件在 HEAD 中不存在: {relative}"
        )));
    }
    Ok((bytes, truncated))
}

/// HEAD 可解析性(尚无提交 = `UnbornBranch` 的 CLI 等价物)。
fn verify_head(root: &Path) -> Result<(), GitError> {
    let output = command(root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::NotInHead(
            "HEAD 中不存在：尚无提交记录".to_owned(),
        ))
    }
}

/// 一条统一 diff hunk(git `@@ -a,b +c,d @@` 解析)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

/// 单文件状态(porcelain v2 XY 归一到 gutter/diff 六值)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileStatus {
    Untracked,
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflicted,
}

/// `git status --porcelain=v2 --branch -z` 解析结果(展示面与写操作 revision
/// 共用;D07-1 子进程统一后本解析是唯一实现)。
#[derive(Clone, Debug, Default)]
pub(crate) struct StatusSnapshot {
    /// `sha256(porcelain stdout)` 的 64 位 hex——写操作防陈旧 revision。
    pub digest: String,
    pub branch: String,
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

#[derive(Clone, Debug)]
pub(crate) struct StatusEntry {
    /// Git 原始路径身份(`/` 分隔,相对仓库根)。
    pub path: String,
    /// rename 的源路径(porcelain v2 `2` 记录后随 NUL 记录)。
    pub original: Option<String>,
    pub index: char,
    pub worktree: char,
    pub conflict: bool,
}

impl StatusEntry {
    pub(crate) fn staged(&self) -> bool {
        !matches!(self.index, '.' | '?')
    }

    pub(crate) fn unstaged(&self) -> bool {
        self.worktree != '.'
    }
}

/// 采集仓库状态快照;每条命令都带 `--no-optional-locks`。
pub(crate) fn status(root: &Path) -> Result<StatusSnapshot, GitError> {
    let output = command(root)
        .arg("--literal-pathspecs")
        .args(["status", "--porcelain=v2", "--branch", "-z"])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "status", &output.stderr));
    }
    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut snapshot =
        parse_status(&raw).map_err(|code| GitError::Command(format!("status 解析失败: {code}")))?;
    snapshot.digest = sha256_hex(&output.stdout);
    Ok(snapshot)
}

/// porcelain v2 `--branch -z` 解析(纯函数;截断/坏记录整体拒绝)。
pub(crate) fn parse_status(output: &str) -> Result<StatusSnapshot, &'static str> {
    if !output.ends_with('\0') {
        return Err("git_invalid_status");
    }
    let mut result = StatusSnapshot::default();
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
            StatusEntry {
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
            StatusEntry {
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

/// 单文件状态:由 porcelain 记录归一(无记录 = `Ok(None)`)。
pub(crate) fn file_status(root: &Path, relative: &str) -> Result<Option<FileStatus>, GitError> {
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--",
            relative,
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "status", &output.stderr));
    }
    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(file_status_from_porcelain(&raw, relative))
}

/// 纯映射:porcelain 记录 → 六值状态(与 [`parse_status`] 同源语义)。
pub(crate) fn file_status_from_porcelain(output: &str, relative: &str) -> Option<FileStatus> {
    if output.trim_matches('\0').is_empty() {
        return None;
    }
    let mut records = output.split_terminator('\0');
    while let Some(record) = records.next() {
        if record.starts_with("# ") || record.starts_with("! ") || record.is_empty() {
            continue;
        }
        let kind = record.as_bytes()[0];
        let (path, xy, conflict) = if kind == b'?' {
            (record.strip_prefix("? ")?, "??", false)
        } else {
            let count = match kind {
                b'1' => 9,
                b'2' => 10,
                b'u' => 11,
                _ => return None,
            };
            let fields: Vec<_> = record.splitn(count, ' ').collect();
            if fields.len() != count || fields[1].len() != 2 {
                return None;
            }
            if kind == b'2' {
                // rename 记录后随源路径 NUL 记录,消费掉保持记录对齐。
                let _original = records.next()?;
            }
            (fields[count - 1], fields[1], kind == b'u')
        };
        if path != relative {
            continue;
        }
        return Some(classify_status(kind, xy, conflict));
    }
    None
}

/// XY/记录类型 → 六值状态。重命名与冲突优先于普通修改。
fn classify_status(kind: u8, xy: &str, conflict: bool) -> FileStatus {
    if conflict || kind == b'u' || matches!(xy, "AA" | "DD") {
        return FileStatus::Conflicted;
    }
    if kind == b'2' || xy.contains('R') {
        return FileStatus::Renamed;
    }
    if xy == "??" || kind == b'?' {
        return FileStatus::Untracked;
    }
    if xy.contains('A') {
        return FileStatus::Added;
    }
    if xy.contains('D') {
        return FileStatus::Deleted;
    }
    FileStatus::Modified
}

/// 文件相对 HEAD 的 hunk 集(`--unified=0`;staged + unstaged 合并)。
///
/// HEAD 不存在(尚无提交)时返回空集:新文件的行级标记由文件级状态承接
/// (gutter 值集不变),不伪造 hunk。
pub(crate) fn file_hunks(root: &Path, relative: &str) -> Result<Vec<FileHunk>, GitError> {
    if verify_head(root).is_err() {
        return Ok(Vec::new());
    }
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "HEAD",
            "--",
            relative,
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "diff", &output.stderr));
    }
    Ok(parse_hunks(&String::from_utf8_lossy(&output.stdout)))
}

/// 文件相对 HEAD 的统一 diff 文本(协议面 `git.diff` 的展示载荷)。
pub(crate) fn diff_text(root: &Path, relative: &str) -> Result<String, GitError> {
    if verify_head(root).is_err() {
        return Ok(String::new());
    }
    let output = command(root)
        .arg("--literal-pathspecs")
        .args([
            "diff",
            "--unified=3",
            "--no-color",
            "--no-ext-diff",
            "HEAD",
            "--",
            relative,
        ])
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if !output.status.success() {
        return Err(command_failure(root, "diff", &output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 解析 `@@ -old_start,old_lines +new_start,new_lines @@` 行(缺省行数 = 1)。
pub(crate) fn parse_hunks(diff: &str) -> Vec<FileHunk> {
    let mut hunks = Vec::new();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some(end) = rest.find(" @@") else {
            continue;
        };
        let mut parts = rest[..end].split(' ');
        let (Some(old), Some(new)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Some(old) = old.strip_prefix('-').and_then(parse_hunk_range) else {
            continue;
        };
        let Some(new) = new.strip_prefix('+').and_then(parse_hunk_range) else {
            continue;
        };
        hunks.push(FileHunk {
            old_start: old.0,
            old_lines: old.1,
            new_start: new.0,
            new_lines: new.1,
        });
    }
    hunks
}

fn parse_hunk_range(value: &str) -> Option<(u32, u32)> {
    let (start, lines) = match value.split_once(',') {
        Some((start, lines)) => (start, lines),
        None => (value, "1"),
    };
    Some((start.parse().ok()?, lines.parse().ok()?))
}

/// 回滚单文件到 HEAD 字节(索引与工作区同时复位 = 三方字节一致)。
///
/// 未跟踪文件 = 删除工作区新文件;目录回滚拒绝(运行时路径是文件粒度)。
pub(crate) fn rollback(root: &Path, relative: &str) -> Result<(), GitError> {
    let target = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if target.is_dir() {
        return Err(GitError::Command("目录不支持回滚".to_owned()));
    }
    match file_status(root, relative)? {
        Some(FileStatus::Untracked) => std::fs::remove_file(&target).map_err(|error| {
            GitError::Command(format!("删除未跟踪文件失败 {}: {error}", target.display()))
        }),
        None => Ok(()),
        Some(_) => {
            verify_head(root)?;
            let output = command(root)
                .arg("--literal-pathspecs")
                .args(["checkout", "HEAD", "--", relative])
                .output()
                .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(command_failure(root, "checkout", &output.stderr))
            }
        }
    }
}

/// 取消暂存(索引 ← HEAD;工作区字节不动);unborn HEAD 回退 `git rm --cached`。
pub(crate) fn unstage(root: &Path, relative: &str) -> Result<(), GitError> {
    let args: &[&str] = if verify_head(root).is_ok() {
        &["restore", "--staged", "--", relative]
    } else {
        &["rm", "--cached", "--ignore-unmatch", "--", relative]
    };
    let output = command(root)
        .arg("--literal-pathspecs")
        .args(args)
        .output()
        .map_err(|error| GitError::Unavailable(format!("无法运行 git: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failure(root, args[0], &output.stderr))
    }
}

/// 路径 → 仓库根相对路径(gutter/diff 的 file-scoped 命令参数)。
pub(crate) fn relative_path(root: &Path, path: &Path) -> Result<String, GitError> {
    relative_to_root(root, path)
}

/// 命令失败的统一错误:stderr 首行进消息,便于 i18n `{error}` 展示。
fn command_failure(root: &Path, operation: &str, stderr: &[u8]) -> GitError {
    let text = String::from_utf8_lossy(stderr);
    let reason = text.lines().next().unwrap_or("unknown error");
    if text.contains("not a git repository") {
        GitError::NotRepository(format!("不在 Git 仓库内: {}", root.display()))
    } else {
        GitError::Command(format!("git {operation} 失败: {reason}"))
    }
}

/// `sha256` 64 位 hex(写操作 revision 摘要)。
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// 绝对路径 → 仓库根相对路径(`/` 分隔,git tree 路径形态)。
fn relative_to_root(root: &Path, path: &Path) -> Result<String, GitError> {
    let relative = match path.strip_prefix(root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => {
            // 8.3 短名/长名或大小写差异:规范化后再比一次(旧 git2 资产同因)。
            let root = std::fs::canonicalize(root).map_err(|error| {
                GitError::Command(format!("无法规范化仓库根 {}: {error}", root.display()))
            })?;
            let path = std::fs::canonicalize(path)
                .map_err(|error| GitError::Command(format!("无法规范化路径: {error}")))?;
            path.strip_prefix(&root)
                .map_err(|_| GitError::NotInHead(format!("文件不在仓库根内: {}", path.display())))?
                .to_path_buf()
        }
    };
    if relative.as_os_str().is_empty() {
        return Err(GitError::NotInHead(
            "文件在 HEAD 中不存在: 路径为空".to_owned(),
        ));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod git_read_tests {
    use super::*;
    use std::path::Path;

    /// 命令形态守卫:每条 git 都带 `--no-optional-locks`(边界条 7 不可谈判)。
    #[test]
    fn every_command_disables_optional_locks() {
        let command = command(Path::new("D:/repo"));
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            ["--no-optional-locks", "-C", "D:/repo"],
            "全局 flag 必须先于子命令"
        );
    }

    /// 仓库根解析:子目录路径上溯到 worktree 根。
    #[test]
    fn repository_root_resolves_the_worktree_top() {
        let repository = test_repository();
        let nested = repository.path().join("src");
        std::fs::create_dir_all(&nested).unwrap();
        let root = repository_root(&nested).expect("子目录必须解析出仓库根");
        assert_eq!(
            canonical(&root),
            canonical(repository.path()),
            "必须上溯到 worktree 根"
        );
    }

    /// HEAD 字节 ≠ 工作区字节:gitshow 数据面读的是已提交内容。
    #[test]
    fn head_file_returns_the_committed_blob_not_the_worktree_copy() {
        let repository = test_repository();
        let path = repository.path().join("tracked.txt");
        std::fs::write(&path, "worktree edit\n").unwrap();
        let (bytes, truncated) = head_file(&path, 1024 * 1024).expect("已提交文件必须可读");
        assert_eq!(bytes, b"tracked\n");
        assert!(!truncated);
    }

    /// 上限截断:只读前 limit 字节并置 truncated。
    #[test]
    fn head_file_bounds_reads_and_reports_truncation() {
        let repository = test_repository();
        let path = repository.path().join("tracked.txt");
        let (bytes, truncated) = head_file(&path, 4).expect("HEAD 读取");
        assert_eq!(bytes, b"trac");
        assert!(truncated);
    }

    /// UnbornBranch:尚无提交 → 错误消息含「HEAD 中不存在」。
    #[test]
    fn unborn_repository_reports_head_missing() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            git(directory.path(), ["init", "--initial-branch=main"])
                .status
                .success()
        );
        std::fs::write(directory.path().join("new.txt"), "new\n").unwrap();
        let error = head_file(&directory.path().join("new.txt"), 1024).expect_err("尚无提交");
        assert!(matches!(error, GitError::NotInHead(_)));
        assert!(
            error.to_string().contains("HEAD 中不存在"),
            "UI 占位契约: {}",
            error
        );
    }

    /// 文件在 HEAD tree 中不存在 → 同一错误契约(消息含相对路径)。
    #[test]
    fn files_absent_from_head_report_the_same_contract() {
        let repository = test_repository();
        let path = repository.path().join("untracked.txt");
        std::fs::write(&path, "untracked\n").unwrap();
        let error = head_file(&path, 1024).expect_err("未跟踪文件不在 HEAD");
        assert!(matches!(error, GitError::NotInHead(_)));
        let message = error.to_string();
        assert!(message.contains("HEAD 中不存在"), "{message}");
        assert!(message.contains("untracked.txt"), "{message}");
    }

    /// 非仓库路径:NotRepository(不 panic、不误报 HEAD)。
    #[test]
    fn paths_outside_a_worktree_are_not_repositories() {
        let directory = tempfile::tempdir().unwrap();
        let ghost = directory.path().join("ghost").join("file.txt");
        let error = repository_root(&ghost).expect_err("非仓库目录");
        assert!(matches!(error, GitError::NotRepository(_)));
    }

    /// porcelain v2:rename 身份、部分暂存与中文/换行文件名都按 NUL 记录原样保留。
    #[test]
    fn status_parser_preserves_rename_identity_and_partial_staging() {
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
    fn status_parser_rejects_truncated_or_malformed_output() {
        assert!(parse_status("# branch.oid abc\0# branch.head main").is_err());
        assert!(parse_status("? file\0").is_err(), "缺分支头");
        assert!(parse_status("# branch.oid abc\0# branch.head main\0? \0").is_err());
    }

    /// 六值映射:未跟踪/新增/修改/删除/重命名/冲突各归其位。
    #[test]
    fn file_status_maps_porcelain_records_to_the_gutter_values() {
        let cases = [
            ("? new.txt\0", FileStatus::Untracked),
            ("1 A. N... 0 0 0 0 0 added.txt\0", FileStatus::Added),
            ("1 .M N... 0 0 0 0 0 tracked.txt\0", FileStatus::Modified),
            ("1 .D N... 0 0 0 0 0 gone.txt\0", FileStatus::Deleted),
            (
                "2 R. N... 0 0 0 0 0 R100 new.txt\0old.txt\0",
                FileStatus::Renamed,
            ),
            (
                "u UU N... 100644 100644 100644 100644 0000 0000 0000 conflict.txt\0",
                FileStatus::Conflicted,
            ),
        ];
        for (record, expected) in cases {
            assert_eq!(
                file_status_from_porcelain(record, record_path(record)),
                Some(expected),
                "{record:?}"
            );
        }
        assert_eq!(
            file_status_from_porcelain("? other.txt\0", "tracked.txt"),
            None,
            "路径不匹配不得串台"
        );
    }

    /// 真仓库:status 摘要 64 hex;修改/暂存/回滚/取消暂存全链字节正确。
    #[test]
    fn repository_status_and_file_operations_round_trip() {
        let repository = test_repository();
        let root = repository.path();
        let path = root.join("tracked.txt");
        let relative = relative_path(root, &path).expect("仓库相对路径");

        std::fs::write(&path, "worktree edit\nmore\n").unwrap();
        let snapshot = status(root).expect("status 快照");
        assert_eq!(snapshot.digest.len(), 64, "写操作 revision 必须是 64 hex");
        assert!(snapshot.digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            file_status(root, &relative).unwrap(),
            Some(FileStatus::Modified)
        );
        let hunks = file_hunks(root, &relative).unwrap();
        assert_eq!(hunks.len(), 1, "单处修改应解析出一条 hunk: {hunks:?}");
        assert_eq!((hunks[0].old_start, hunks[0].old_lines), (1, 1));
        assert_eq!((hunks[0].new_start, hunks[0].new_lines), (1, 2));

        // 回滚:工作区与索引都回到 HEAD 字节。
        rollback(root, &relative).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"tracked\n");
        assert_eq!(file_status(root, &relative).unwrap(), None);

        // 暂存后 unstage:索引清空、工作区保留修改。
        std::fs::write(&path, "staged edit\n").unwrap();
        assert!(
            git(root, ["add", "tracked.txt"]).status.success(),
            "git add"
        );
        assert_eq!(
            file_status(root, &relative).unwrap(),
            Some(FileStatus::Modified)
        );
        unstage(root, &relative).unwrap();
        let after = status(root).unwrap();
        assert!(
            after.entries.iter().all(|entry| !entry.staged()),
            "unstage 后索引不得再有该文件: {:?}",
            after.entries
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"staged edit\n");

        // 未跟踪文件回滚 = 删除工作区新文件。
        let fresh = root.join("fresh.txt");
        std::fs::write(&fresh, b"new\n").unwrap();
        let fresh_relative = relative_path(root, &fresh).unwrap();
        assert_eq!(
            file_status(root, &fresh_relative).unwrap(),
            Some(FileStatus::Untracked)
        );
        rollback(root, &fresh_relative).unwrap();
        assert!(!fresh.exists(), "未跟踪文件回滚必须删除");
    }

    /// hunk 解析:缺省行数与多 hunk 行号保持精确。
    #[test]
    fn hunk_parser_handles_omitted_counts_and_multiple_hunks() {
        let diff = "@@ -1 +1 @@\n-a\n+b\n@@ -10,3 +11,0 @@\n-x\n";
        let hunks = parse_hunks(diff);
        assert_eq!(hunks.len(), 2);
        assert_eq!(
            hunks[0],
            FileHunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1
            }
        );
        assert_eq!(
            hunks[1],
            FileHunk {
                old_start: 10,
                old_lines: 3,
                new_start: 11,
                new_lines: 0
            }
        );
    }

    fn record_path(record: &str) -> &str {
        // rename 记录后随源路径 NUL 记录;取第一条记录本身。
        let record = record.split('\0').next().unwrap_or(record);
        match record.as_bytes().first() {
            Some(b'?') => record.trim_start_matches("? "),
            Some(b'2') => record.splitn(10, ' ').last().unwrap(),
            Some(b'u') => record.splitn(11, ' ').last().unwrap(),
            _ => record.splitn(9, ' ').last().unwrap(),
        }
    }

    fn canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn test_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("create repository directory");
        assert!(
            git(directory.path(), ["init", "--initial-branch=main"])
                .status
                .success()
        );
        // 字节级回滚断言依赖仓库自身配置:全局 autocrlf 不得改写检出字节。
        assert!(
            git(directory.path(), ["config", "core.autocrlf", "false"])
                .status
                .success()
        );
        std::fs::write(directory.path().join("tracked.txt"), "tracked\n").expect("write tracked");
        assert!(
            git(directory.path(), ["add", "tracked.txt"])
                .status
                .success()
        );
        let commit = git(
            directory.path(),
            [
                "-c",
                "user.name=slTerminal Test",
                "-c",
                "user.email=slterm@example.invalid",
                "commit",
                "-m",
                "initial",
            ],
        );
        assert!(
            commit.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
        directory
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> std::process::Output {
        // 测试二进制同样隐藏控制台;autocrlf 关闭,HEAD 字节与写入逐字节一致。
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(cwd)
            .args(["-c", "core.autocrlf=false"])
            .args(args);
        crate::platform::process::hidden_command(&mut command)
            .output()
            .expect("run git")
    }
}
