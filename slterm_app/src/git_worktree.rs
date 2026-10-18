//! 运行期 Agent 的事务型 Git worktree 创建(07 篇边界,04 篇消费)。
//!
//! `agent.fork` 先在 transport 线程解析源目录并建出 branch + worktree,再把
//! 新路径交给 UI owner 启动 Tab/PTY。事务语义只有一条:**只回滚本事务证明
//! 自己创建过的资源**——branch 建成功、worktree add 失败时只删 branch,
//! 绝不猜测性删除目标目录。
//!
//! 本模块不经过 shell,所有 Git 命令都是 argv 子进程;Windows 上统一挂
//! `CREATE_NO_WINDOW`(slterm 是 console 子系统,但一次 fork 会连跑多条
//! Git,不压掉就会在部分宿主上闪控制台窗)。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::runtime_api::ApiError;

/// worktree 归属记录:写进 managed agent 与快照,外部客户端据此知道
/// Agent 的 checkout 从哪来、归谁所有。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeProvenance {
    /// 主仓库根(worktree add/remove 的执行目录)。
    pub repo_root: PathBuf,
    /// 源 checkout 根(可能是主仓库或另一个 worktree)。
    pub source_root: PathBuf,
    /// 新 checkout 的绝对路径。
    pub path: PathBuf,
    pub branch: String,
    pub base_commit: String,
    /// 恒为 true:provenance 只在成功创建后产出。
    pub created: bool,
}

#[derive(Debug)]
pub struct WorktreeRequest {
    pub source_cwd: PathBuf,
    pub agent_name: String,
    pub branch: Option<String>,
    pub base: Option<String>,
    pub path: Option<PathBuf>,
    pub allow_dirty_source: bool,
}

/// 事务句柄:Drop 不回滚(失败路径必须显式 rollback/commit),避免
/// 「以为已经提交、结果被隐式回滚」的歧义。
#[derive(Debug)]
pub struct WorktreeTransaction {
    provenance: WorktreeProvenance,
    branch_owned: bool,
    worktree_owned: bool,
}

impl WorktreeTransaction {
    /// 校验序:源目录 → repo root → dirty → base → branch → path → 创建。
    /// 任何一步失败都发生在下一项副作用之前。
    pub fn prepare(request: WorktreeRequest) -> Result<Self, ApiError> {
        if !request.source_cwd.is_absolute() {
            return Err(ApiError::invalid_params(
                "source_cwd must be an absolute path when source_pane_id is not used",
            ));
        }

        ensure_git_available()?;
        let source_root = git_text(
            &request.source_cwd,
            ["rev-parse", "--show-toplevel"],
            "not_a_git_worktree",
            "source_cwd is not inside a Git worktree",
        )?;
        let source_root = PathBuf::from(source_root);
        let common_dir = PathBuf::from(git_text(
            &source_root,
            ["rev-parse", "--path-format=absolute", "--git-common-dir"],
            "git_query_failed",
            "failed to resolve the repository common directory",
        )?);
        let repo_root = common_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            ApiError::new(
                "unsupported_repository",
                "the Git common directory does not have a parent worktree",
            )
            .details(json!({ "git_common_dir": common_dir }))
        })?;

        if !request.allow_dirty_source {
            let status = git_text(
                &source_root,
                ["status", "--porcelain=v1", "--untracked-files=normal"],
                "git_query_failed",
                "failed to inspect source worktree changes",
            )?;
            if !status.is_empty() {
                return Err(ApiError::new(
                    "dirty_source",
                    "source worktree has uncommitted changes; commit them or explicitly allow a dirty source",
                )
                .details(json!({ "source_root": source_root, "status": status })));
            }
        }

        let base = request.base.as_deref().unwrap_or("HEAD");
        if base.trim().is_empty() || base.chars().any(char::is_control) {
            return Err(ApiError::invalid_params(
                "base must be a non-empty Git revision",
            ));
        }
        let verify = format!("{base}^{{commit}}");
        let base_commit = git_text_os(
            &source_root,
            [
                OsString::from("rev-parse"),
                OsString::from("--verify"),
                OsString::from("--end-of-options"),
                OsString::from(verify),
            ],
            "invalid_base",
            "base does not resolve to a Git commit",
        )?;

        let slug = worktree_slug(&request.agent_name);
        let branch = request.branch.unwrap_or_else(|| format!("slterm/{slug}"));
        validate_branch(&branch)?;
        ensure_branch_absent(&source_root, &branch)?;

        let path = match request.path {
            Some(path) => {
                if !path.is_absolute() {
                    return Err(ApiError::invalid_params("path must be absolute"));
                }
                path
            }
            None => {
                let repo_name = repo_root.file_name().ok_or_else(|| {
                    ApiError::new(
                        "unsupported_repository",
                        "the main repository worktree does not have a directory name",
                    )
                })?;
                let parent = repo_root.parent().ok_or_else(|| {
                    ApiError::new(
                        "unsupported_repository",
                        "the main repository worktree does not have a parent directory",
                    )
                })?;
                let mut container = repo_name.to_os_string();
                container.push("-worktrees");
                parent.join(container).join(&slug)
            }
        };
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(ApiError::new(
                    "worktree_path_conflict",
                    "the target worktree path already exists",
                )
                .details(json!({ "path": path })));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ApiError::new(
                    "worktree_path_inspection_failed",
                    "failed to inspect the target worktree path",
                )
                .details(json!({ "path": path, "reason": error.to_string() })));
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ApiError::new(
                    "worktree_parent_create_failed",
                    "failed to create the worktree parent directory",
                )
                .details(json!({ "path": parent, "reason": error.to_string() }))
            })?;
        }

        let provenance = WorktreeProvenance {
            repo_root,
            source_root,
            path,
            branch,
            base_commit,
            created: true,
        };
        let mut transaction = Self {
            provenance,
            branch_owned: false,
            worktree_owned: false,
        };

        transaction.create_branch()?;
        if let Err(error) = transaction.create_worktree() {
            // branch 创建是独立成功步骤,能证明归本事务所有;worktree add
            // 未明确成功时绝不直接删除目标目录。
            let rollback = transaction.rollback_branch_only();
            return Err(attach_rollback(error, rollback));
        }
        Ok(transaction)
    }

    pub fn provenance(&self) -> &WorktreeProvenance {
        &self.provenance
    }

    /// 提交:放弃所有权,资源留给新 Agent 使用。
    pub fn commit(mut self) -> WorktreeProvenance {
        self.branch_owned = false;
        self.worktree_owned = false;
        self.provenance
    }

    /// 显式回滚:先摘 worktree 再删 branch(顺序反了 Git 会拒绝删 branch)。
    pub fn rollback(mut self) -> Result<(), ApiError> {
        if self.worktree_owned {
            let output = run_git_os(
                &self.provenance.repo_root,
                [
                    OsString::from("worktree"),
                    OsString::from("remove"),
                    OsString::from("--force"),
                    self.provenance.path.as_os_str().to_os_string(),
                ],
            )?;
            if !output.status.success() {
                return Err(git_failure(
                    "worktree_rollback_failed",
                    "failed to remove the worktree created by this request",
                    &output,
                    json!({ "path": self.provenance.path }),
                ));
            }
            self.worktree_owned = false;
        }
        self.rollback_branch_only()
    }

    fn create_branch(&mut self) -> Result<(), ApiError> {
        let output = run_git_os(
            &self.provenance.source_root,
            [
                OsString::from("branch"),
                OsString::from("--no-track"),
                self.provenance.branch.clone().into(),
                self.provenance.base_commit.clone().into(),
            ],
        )?;
        if !output.status.success() {
            return Err(git_failure(
                "branch_create_failed",
                "failed to create the worktree branch",
                &output,
                json!({
                    "branch": self.provenance.branch,
                    "base_commit": self.provenance.base_commit
                }),
            ));
        }
        self.branch_owned = true;
        Ok(())
    }

    fn create_worktree(&mut self) -> Result<(), ApiError> {
        let output = run_git_os(
            &self.provenance.repo_root,
            [
                OsString::from("worktree"),
                OsString::from("add"),
                self.provenance.path.as_os_str().to_os_string(),
                self.provenance.branch.clone().into(),
            ],
        )?;
        if !output.status.success() {
            return Err(git_failure(
                "worktree_create_failed",
                "failed to create the Git worktree",
                &output,
                json!({
                    "path": self.provenance.path,
                    "branch": self.provenance.branch
                }),
            ));
        }
        self.worktree_owned = true;
        Ok(())
    }

    fn rollback_branch_only(&mut self) -> Result<(), ApiError> {
        if !self.branch_owned {
            return Ok(());
        }
        let output = run_git_os(
            &self.provenance.repo_root,
            [
                OsString::from("branch"),
                OsString::from("-D"),
                OsString::from("--"),
                self.provenance.branch.clone().into(),
            ],
        )?;
        if !output.status.success() {
            return Err(git_failure(
                "branch_rollback_failed",
                "failed to remove the branch created by this request",
                &output,
                json!({ "branch": self.provenance.branch }),
            ));
        }
        self.branch_owned = false;
        Ok(())
    }
}

/// 本模块所有 `git` 子进程的统一构造口。
fn hidden_git_command() -> Command {
    let mut command = Command::new("git");
    crate::platform::process::hidden_command(&mut command);
    command
}

fn ensure_git_available() -> Result<(), ApiError> {
    let output = hidden_git_command()
        .arg("--version")
        .output()
        .map_err(|error| {
            ApiError::new("git_unavailable", "Git executable is unavailable")
                .details(json!({ "reason": error.to_string() }))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure(
            "git_unavailable",
            "Git executable could not report its version",
            &output,
            json!({}),
        ))
    }
}

fn validate_branch(branch: &str) -> Result<(), ApiError> {
    if branch.trim() != branch || branch.is_empty() || branch.chars().any(char::is_control) {
        return Err(ApiError::invalid_params(
            "branch is not a valid Git branch name",
        ));
    }
    let output = hidden_git_command()
        .args(["check-ref-format", "--branch", branch])
        .output()
        .map_err(|error| {
            ApiError::new("git_unavailable", "Git executable is unavailable")
                .details(json!({ "reason": error.to_string() }))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure(
            "invalid_branch",
            "branch is not a valid Git branch name",
            &output,
            json!({ "branch": branch }),
        ))
    }
}

fn ensure_branch_absent(cwd: &Path, branch: &str) -> Result<(), ApiError> {
    let reference = format!("refs/heads/{branch}");
    let output = run_git_os(
        cwd,
        [
            OsString::from("show-ref"),
            OsString::from("--verify"),
            OsString::from("--quiet"),
            reference.into(),
        ],
    )?;
    match output.status.code() {
        Some(1) => Ok(()),
        Some(0) => Err(ApiError::new(
            "branch_conflict",
            "the target worktree branch already exists",
        )
        .details(json!({ "branch": branch }))),
        _ => Err(git_failure(
            "git_query_failed",
            "failed to determine whether the target branch exists",
            &output,
            json!({ "branch": branch }),
        )),
    }
}

fn git_text<const N: usize>(
    cwd: &Path,
    args: [&str; N],
    code: &str,
    message: &str,
) -> Result<String, ApiError> {
    git_text_os(cwd, args.map(OsString::from), code, message)
}

fn git_text_os<const N: usize>(
    cwd: &Path,
    args: [OsString; N],
    code: &str,
    message: &str,
) -> Result<String, ApiError> {
    let output = run_git_os(cwd, args)?;
    if !output.status.success() {
        return Err(git_failure(code, message, &output, json!({ "cwd": cwd })));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_git_os<I, S>(cwd: &Path, args: I) -> Result<Output, ApiError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    hidden_git_command()
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|error| {
            ApiError::new("git_unavailable", "failed to execute Git")
                .details(json!({ "cwd": cwd, "reason": error.to_string() }))
        })
}

fn git_failure(code: &str, message: &str, output: &Output, context: serde_json::Value) -> ApiError {
    ApiError::new(code, message).details(json!({
        "status": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout).trim(),
        "stderr": String::from_utf8_lossy(&output.stderr).trim(),
        "context": context
    }))
}

fn attach_rollback(mut error: ApiError, rollback: Result<(), ApiError>) -> ApiError {
    if let Err(rollback_error) = rollback {
        error.details = Some(json!({
            "operation": error.details,
            "rollback": rollback_error
        }));
    }
    error
}

/// Agent 名 → 路径/分支安全的 slug(48 字节内,非 ASCII 保留字符)。
fn worktree_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars() {
        let accepted = character.is_alphanumeric() || matches!(character, '-' | '_' | '.');
        if accepted {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            if slug.len() + character.len_utf8() > 48 {
                break;
            }
            slug.extend(character.to_lowercase());
        } else {
            separator = true;
        }
    }
    let trimmed = slug.trim_matches(['-', '_', '.']).to_owned();
    if trimmed.is_empty() {
        format!("agent-{:08x}", stable_hash(name))
    } else {
        trimmed
    }
}

fn stable_hash(value: &str) -> u32 {
    value
        .as_bytes()
        .iter()
        .fold(2_166_136_261_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
        })
}

#[cfg(test)]
mod git_worktree_tests {
    use super::*;

    #[test]
    fn slug_is_path_and_branch_safe() {
        assert_eq!(worktree_slug("Fix Login / Windows"), "fix-login-windows");
        assert_eq!(worktree_slug("  审查 API  "), "审查-api");
        assert!(worktree_slug("///").starts_with("agent-"));
    }

    #[test]
    fn transaction_creates_and_rolls_back_owned_resources() {
        let repository = test_repository();
        let target = repository.path().join("worktrees").join("reviewer");
        let transaction = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: Some("slterm/test-reviewer".into()),
            base: None,
            path: Some(target.clone()),
            allow_dirty_source: false,
        })
        .expect("worktree should be created");
        assert!(target.join("tracked.txt").is_file());
        assert_eq!(transaction.provenance().branch, "slterm/test-reviewer");
        transaction
            .rollback()
            .expect("owned worktree should roll back");
        assert!(!target.exists());
        assert!(
            !git_output(
                repository.path(),
                [
                    "show-ref",
                    "--verify",
                    "--quiet",
                    "refs/heads/slterm/test-reviewer"
                ]
            )
            .status
            .success()
        );
    }

    #[test]
    fn default_branch_uses_the_slterm_prefix() {
        let repository = test_repository();
        let transaction = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: None,
            base: None,
            path: Some(repository.path().join("worktrees").join("reviewer")),
            allow_dirty_source: false,
        })
        .expect("default worktree should be created");
        assert_eq!(transaction.provenance().branch, "slterm/reviewer");
        transaction.rollback().expect("rollback");
    }

    #[test]
    fn dirty_source_requires_explicit_opt_in() {
        let repository = test_repository();
        std::fs::write(repository.path().join("untracked.txt"), "dirty")
            .expect("write untracked file");
        let error = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: Some("slterm/dirty-reviewer".into()),
            base: None,
            path: Some(repository.path().join("worktrees").join("dirty")),
            allow_dirty_source: false,
        })
        .expect_err("dirty source must be rejected");
        assert_eq!(error.code, "dirty_source");
    }

    #[test]
    fn conflicting_path_and_branch_are_rejected_before_creation() {
        let repository = test_repository();
        let existing = repository.path().join("occupied");
        std::fs::create_dir_all(&existing).expect("create occupied dir");
        let error = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: Some("slterm/conflict".into()),
            base: None,
            path: Some(existing),
            allow_dirty_source: false,
        })
        .expect_err("existing path must be rejected");
        assert_eq!(error.code, "worktree_path_conflict");

        // 先把同名 branch 建出来,再 fork 同一 branch:branch_conflict 发生在
        // worktree add 之前,目标路径不会被创建。
        assert!(
            git_output(repository.path(), ["branch", "slterm/taken"])
                .status
                .success()
        );
        let target = repository.path().join("worktrees").join("taken");
        let error = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "taken".into(),
            branch: Some("slterm/taken".into()),
            base: None,
            path: Some(target.clone()),
            allow_dirty_source: false,
        })
        .expect_err("existing branch must be rejected");
        assert_eq!(error.code, "branch_conflict");
        assert!(!target.exists());
    }

    #[test]
    fn invalid_base_and_branch_are_reported() {
        let repository = test_repository();
        let error = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: Some("slterm/base-check".into()),
            base: Some("does-not-exist".into()),
            path: Some(repository.path().join("worktrees").join("base-check")),
            allow_dirty_source: false,
        })
        .expect_err("invalid base");
        assert_eq!(error.code, "invalid_base");

        let error = WorktreeTransaction::prepare(WorktreeRequest {
            source_cwd: repository.path().to_path_buf(),
            agent_name: "reviewer".into(),
            branch: Some("refs/heads/not a branch".into()),
            base: None,
            path: Some(repository.path().join("worktrees").join("bad-branch")),
            allow_dirty_source: false,
        })
        .expect_err("invalid branch");
        assert!(
            matches!(error.code.as_str(), "invalid_branch" | "invalid_params"),
            "unexpected code {}",
            error.code
        );
    }

    fn test_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("create repository directory");
        assert!(
            git_output(directory.path(), ["init", "--initial-branch=main"])
                .status
                .success()
        );
        std::fs::write(directory.path().join("tracked.txt"), "tracked")
            .expect("write tracked file");
        assert!(
            git_output(directory.path(), ["add", "tracked.txt"])
                .status
                .success()
        );
        assert!(
            git_output(
                directory.path(),
                [
                    "-c",
                    "user.name=slTerminal Test",
                    "-c",
                    "user.email=slterm@example.invalid",
                    "commit",
                    "-m",
                    "initial"
                ]
            )
            .status
            .success()
        );
        directory
    }

    fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Output {
        hidden_git_command()
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("run git")
    }
}
