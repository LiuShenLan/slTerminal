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
