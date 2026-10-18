//! 运行期 Agent 的事务型 Git worktree 创建(文件域边界,agent.fork 消费)。
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
use serde_json::{Value, json};

use crate::runtime_api::ApiError;

/// worktree 事务的失败分类。对外错误码只允许落在 Runtime API 已登记集合
/// (`invalid_state` / `action_failed` 与既有 git 错误码);历史内部码经
/// [`Self::legacy_code`] 写入 `details.internal_code`,诊断分类不丢。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    // 已登记语义:对外码与内部码一致。
    InvalidBase,
    InvalidBranch,
    GitUnavailable,
    DirtySource,
    BranchConflict,
    WorktreePathConflict,
    // 历史内部码:收敛到 invalid_state / action_failed。
    NotAGitWorktree,
    GitQueryFailed,
    UnsupportedRepository,
    BranchCreateFailed,
    WorktreeCreateFailed,
    WorktreeRollbackFailed,
    BranchRollbackFailed,
    ParentCreateFailed,
    PathInspectionFailed,
}

impl FailureKind {
    /// 内部码 → 对外码。纯映射,表驱动测试锁死并与文档登记集合比对。
    fn public_code(self) -> &'static str {
        match self {
            Self::InvalidBase => "invalid_base",
            Self::InvalidBranch => "invalid_branch",
            Self::GitUnavailable => "git_unavailable",
            Self::DirtySource => "dirty_source",
            Self::BranchConflict => "branch_conflict",
            Self::WorktreePathConflict => "worktree_path_conflict",
            Self::NotAGitWorktree | Self::UnsupportedRepository => "invalid_state",
            Self::GitQueryFailed
            | Self::BranchCreateFailed
            | Self::WorktreeCreateFailed
            | Self::WorktreeRollbackFailed
            | Self::BranchRollbackFailed
            | Self::ParentCreateFailed
            | Self::PathInspectionFailed => "action_failed",
        }
    }

    /// 历史内部码;已登记语义返回 None,details 不重复记录。
    fn legacy_code(self) -> Option<&'static str> {
        Some(match self {
            Self::NotAGitWorktree => "not_a_git_worktree",
            Self::GitQueryFailed => "git_query_failed",
            Self::UnsupportedRepository => "unsupported_repository",
            Self::BranchCreateFailed => "branch_create_failed",
            Self::WorktreeCreateFailed => "worktree_create_failed",
            Self::WorktreeRollbackFailed => "worktree_rollback_failed",
            Self::BranchRollbackFailed => "branch_rollback_failed",
            Self::ParentCreateFailed => "worktree_parent_create_failed",
            Self::PathInspectionFailed => "worktree_path_inspection_failed",
            Self::InvalidBase
            | Self::InvalidBranch
            | Self::GitUnavailable
            | Self::DirtySource
            | Self::BranchConflict
            | Self::WorktreePathConflict => return None,
        })
    }
}

/// 本模块构造 `ApiError` 的统一出口:对外码收敛,历史内部码进 details。
fn worktree_error(kind: FailureKind, message: impl Into<String>, context: Value) -> ApiError {
    let mut details = match context {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("context".to_owned(), other);
            map
        }
    };
    if let Some(legacy) = kind.legacy_code() {
        details.insert("internal_code".to_owned(), json!(legacy));
    }
    ApiError::new(kind.public_code(), message).details(Value::Object(details))
}

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
            FailureKind::NotAGitWorktree,
            "source_cwd is not inside a Git worktree",
        )?;
        let source_root = PathBuf::from(source_root);
        let common_dir = PathBuf::from(git_text(
            &source_root,
            ["rev-parse", "--path-format=absolute", "--git-common-dir"],
            FailureKind::GitQueryFailed,
            "failed to resolve the repository common directory",
        )?);
        let repo_root = common_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            worktree_error(
                FailureKind::UnsupportedRepository,
                "the Git common directory does not have a parent worktree",
                json!({ "git_common_dir": common_dir }),
            )
        })?;

        if !request.allow_dirty_source {
            let status = git_text(
                &source_root,
                ["status", "--porcelain=v1", "--untracked-files=normal"],
                FailureKind::GitQueryFailed,
                "failed to inspect source worktree changes",
            )?;
            if !status.is_empty() {
                return Err(worktree_error(
                    FailureKind::DirtySource,
                    "source worktree has uncommitted changes; commit them or explicitly allow a dirty source",
                    json!({ "source_root": source_root, "status": status }),
                ));
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
            FailureKind::InvalidBase,
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
                    worktree_error(
                        FailureKind::UnsupportedRepository,
                        "the main repository worktree does not have a directory name",
                        json!({}),
                    )
                })?;
                let parent = repo_root.parent().ok_or_else(|| {
                    worktree_error(
                        FailureKind::UnsupportedRepository,
                        "the main repository worktree does not have a parent directory",
                        json!({}),
                    )
                })?;
                let mut container = repo_name.to_os_string();
                container.push("-worktrees");
                parent.join(container).join(&slug)
            }
        };
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(worktree_error(
                    FailureKind::WorktreePathConflict,
                    "the target worktree path already exists",
                    json!({ "path": path }),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(worktree_error(
                    FailureKind::PathInspectionFailed,
                    "failed to inspect the target worktree path",
                    json!({ "path": path, "reason": error.to_string() }),
                ));
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                worktree_error(
                    FailureKind::ParentCreateFailed,
                    "failed to create the worktree parent directory",
                    json!({ "path": parent, "reason": error.to_string() }),
                )
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
                    FailureKind::WorktreeRollbackFailed,
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
                FailureKind::BranchCreateFailed,
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
                FailureKind::WorktreeCreateFailed,
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
                FailureKind::BranchRollbackFailed,
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
            worktree_error(
                FailureKind::GitUnavailable,
                "Git executable is unavailable",
                json!({ "reason": error.to_string() }),
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure(
            FailureKind::GitUnavailable,
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
            worktree_error(
                FailureKind::GitUnavailable,
                "Git executable is unavailable",
                json!({ "reason": error.to_string() }),
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure(
            FailureKind::InvalidBranch,
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
        Some(0) => Err(worktree_error(
            FailureKind::BranchConflict,
            "the target worktree branch already exists",
            json!({ "branch": branch }),
        )),
        _ => Err(git_failure(
            FailureKind::GitQueryFailed,
            "failed to determine whether the target branch exists",
            &output,
            json!({ "branch": branch }),
        )),
    }
}

fn git_text<const N: usize>(
    cwd: &Path,
    args: [&str; N],
    kind: FailureKind,
    message: &str,
) -> Result<String, ApiError> {
    git_text_os(cwd, args.map(OsString::from), kind, message)
}

fn git_text_os<const N: usize>(
    cwd: &Path,
    args: [OsString; N],
    kind: FailureKind,
    message: &str,
) -> Result<String, ApiError> {
    let output = run_git_os(cwd, args)?;
    if !output.status.success() {
        return Err(git_failure(kind, message, &output, json!({ "cwd": cwd })));
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
            worktree_error(
                FailureKind::GitUnavailable,
                "failed to execute Git",
                json!({ "cwd": cwd, "reason": error.to_string() }),
            )
        })
}

fn git_failure(
    kind: FailureKind,
    message: &str,
    output: &Output,
    context: serde_json::Value,
) -> ApiError {
    worktree_error(
        kind,
        message,
        json!({
            "status": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout).trim(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim(),
            "context": context
        }),
    )
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

    /// `assets/runtime-control-api.md` 错误码段登记的全部 snake_case 码。
    /// 反引号里的方法名(`pane.prompt` 等)含 `.`,不在错误码集合内。
    fn documented_error_codes() -> Vec<String> {
        let document = include_str!("../../assets/runtime-control-api.md");
        let section = document
            .split_once("## 错误码")
            .and_then(|(_, rest)| rest.split_once("\n## "))
            .map(|(section, _)| section)
            .expect("runtime-control-api.md must contain an error code section");
        section
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::trim)
            .filter(|token| {
                !token.is_empty()
                    && token
                        .chars()
                        .all(|ch| ch.is_ascii_lowercase() || ch == '_' || ch.is_ascii_digit())
            })
            .map(str::to_owned)
            .collect()
    }

    /// 原内部码 → 对外码的纯映射:任一映射漂移或落回未登记码,即红。
    #[test]
    fn legacy_worktree_codes_converge_to_documented_api_codes() {
        let documented = documented_error_codes();
        let cases = [
            (
                FailureKind::NotAGitWorktree,
                "invalid_state",
                Some("not_a_git_worktree"),
            ),
            (
                FailureKind::GitQueryFailed,
                "action_failed",
                Some("git_query_failed"),
            ),
            (
                FailureKind::UnsupportedRepository,
                "invalid_state",
                Some("unsupported_repository"),
            ),
            (
                FailureKind::BranchCreateFailed,
                "action_failed",
                Some("branch_create_failed"),
            ),
            (
                FailureKind::WorktreeCreateFailed,
                "action_failed",
                Some("worktree_create_failed"),
            ),
            (
                FailureKind::WorktreeRollbackFailed,
                "action_failed",
                Some("worktree_rollback_failed"),
            ),
            (
                FailureKind::BranchRollbackFailed,
                "action_failed",
                Some("branch_rollback_failed"),
            ),
            (
                FailureKind::ParentCreateFailed,
                "action_failed",
                Some("worktree_parent_create_failed"),
            ),
            (
                FailureKind::PathInspectionFailed,
                "action_failed",
                Some("worktree_path_inspection_failed"),
            ),
            (FailureKind::InvalidBase, "invalid_base", None),
            (FailureKind::InvalidBranch, "invalid_branch", None),
            (FailureKind::GitUnavailable, "git_unavailable", None),
            (FailureKind::DirtySource, "dirty_source", None),
            (FailureKind::BranchConflict, "branch_conflict", None),
            (
                FailureKind::WorktreePathConflict,
                "worktree_path_conflict",
                None,
            ),
        ];
        for (kind, public, legacy) in cases {
            assert_eq!(kind.public_code(), public, "{kind:?}");
            assert_eq!(kind.legacy_code(), legacy, "{kind:?}");
            assert!(
                documented.iter().any(|code| code == public),
                "错误码 {public} 未在 runtime-control-api.md 错误码段登记"
            );
        }
    }

    /// 真实可触发分支的错误码必须全部落在文档登记集合内;收敛码的
    /// message 与 details 保留原诊断。
    #[test]
    fn worktree_failure_codes_stay_within_documented_set() {
        let documented = documented_error_codes();
        let mut errors = Vec::new();

        // 非 git 目录 → invalid_state。
        let plain = tempfile::tempdir().expect("create plain directory");
        errors.push(
            WorktreeTransaction::prepare(WorktreeRequest {
                source_cwd: plain.path().to_path_buf(),
                agent_name: "reviewer".into(),
                branch: Some("slterm/not-a-repo".into()),
                base: None,
                path: Some(plain.path().join("worktrees").join("not-a-repo")),
                allow_dirty_source: false,
            })
            .expect_err("plain directory must be rejected"),
        );

        // 脏源 → dirty_source。
        let dirty = test_repository();
        std::fs::write(dirty.path().join("untracked.txt"), "dirty").expect("write untracked file");
        errors.push(
            WorktreeTransaction::prepare(WorktreeRequest {
                source_cwd: dirty.path().to_path_buf(),
                agent_name: "reviewer".into(),
                branch: Some("slterm/dirty-reviewer".into()),
                base: None,
                path: Some(dirty.path().join("worktrees").join("dirty")),
                allow_dirty_source: false,
            })
            .expect_err("dirty source must be rejected"),
        );

        // 分支冲突与目标路径冲突。
        let repository = test_repository();
        assert!(
            git_output(repository.path(), ["branch", "slterm/taken"])
                .status
                .success()
        );
        errors.push(
            WorktreeTransaction::prepare(WorktreeRequest {
                source_cwd: repository.path().to_path_buf(),
                agent_name: "taken".into(),
                branch: Some("slterm/taken".into()),
                base: None,
                path: Some(repository.path().join("worktrees").join("taken")),
                allow_dirty_source: false,
            })
            .expect_err("existing branch must be rejected"),
        );
        let occupied = repository.path().join("occupied");
        std::fs::create_dir_all(&occupied).expect("create occupied dir");
        errors.push(
            WorktreeTransaction::prepare(WorktreeRequest {
                source_cwd: repository.path().to_path_buf(),
                agent_name: "occupied".into(),
                branch: Some("slterm/occupied".into()),
                base: None,
                path: Some(occupied),
                allow_dirty_source: false,
            })
            .expect_err("occupied path must be rejected"),
        );

        // 目标路径探测/父目录创建失败 → action_failed(具体内部码依
        // 平台错误分类而定,只锁定对外码与诊断留存)。目标放在独立目录,
        // 避免污染源仓库的 dirty 判定。
        let target_root = tempfile::tempdir().expect("create target directory");
        let blocker = target_root.path().join("blocker");
        std::fs::write(&blocker, "file").expect("write blocker file");
        errors.push(
            WorktreeTransaction::prepare(WorktreeRequest {
                source_cwd: repository.path().to_path_buf(),
                agent_name: "blocked".into(),
                branch: Some("slterm/blocked".into()),
                base: None,
                path: Some(blocker.join("worktree")),
                allow_dirty_source: false,
            })
            .expect_err("path through a file must be rejected"),
        );

        // git 不可用 → git_unavailable。
        {
            let _path = EmptyPathGuard::new();
            errors.push(
                WorktreeTransaction::prepare(WorktreeRequest {
                    source_cwd: repository.path().to_path_buf(),
                    agent_name: "no-git".into(),
                    branch: Some("slterm/no-git".into()),
                    base: None,
                    path: Some(repository.path().join("worktrees").join("no-git")),
                    allow_dirty_source: false,
                })
                .expect_err("missing Git must be rejected"),
            );
        }

        let codes: Vec<&str> = errors.iter().map(|error| error.code.as_str()).collect();
        assert_eq!(
            codes,
            [
                "invalid_state",
                "dirty_source",
                "branch_conflict",
                "worktree_path_conflict",
                "action_failed",
                "git_unavailable",
            ]
        );
        for error in &errors {
            assert!(
                documented.iter().any(|code| code == &error.code),
                "未登记错误码 {} (message={})",
                error.code,
                error.message
            );
        }
        // 收敛码:原内部码与原因留在 details,message 原样。
        assert_eq!(
            errors[0]
                .details
                .as_ref()
                .and_then(|details| details["internal_code"].as_str()),
            Some("not_a_git_worktree")
        );
        assert_eq!(errors[0].message, "source_cwd is not inside a Git worktree");
        let action_failed = errors[4].details.as_ref().expect("action_failed details");
        assert!(
            action_failed["internal_code"].is_string(),
            "action_failed 必须保留内部诊断码: {action_failed}"
        );
        assert!(
            action_failed["reason"].is_string(),
            "action_failed 必须保留失败原因: {action_failed}"
        );
    }

    /// 进程级 PATH 置空以注入「git 不可用」;全量测试串行纪律保证用例间
    /// 互斥,Drop 恢复原值,panic 也不泄漏。
    struct EmptyPathGuard(Option<std::ffi::OsString>);

    impl EmptyPathGuard {
        fn new() -> Self {
            let previous = std::env::var_os("PATH");
            // SAFETY: 测试按 --test-threads=1 串行执行,进程级环境变量无并发读写。
            unsafe { std::env::set_var("PATH", "") };
            Self(previous)
        }
    }

    impl Drop for EmptyPathGuard {
        fn drop(&mut self) {
            // SAFETY: 同上,恢复进入用例前的进程级状态。
            unsafe {
                match self.0.take() {
                    Some(previous) => std::env::set_var("PATH", previous),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
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
