//! GitFacade 的真实仓库绑定捕获与复核；全部进程读取有界且禁网。

use std::process::Command;
use std::str::FromStr as _;
use std::time::Duration;

use slterm_document::{CommitId, RepositoryBinding};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};

use super::{GitError, GitFacade};
use crate::platform::{
    document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver},
    process_output::{ProcessOutput, ProcessOutputError, run_cancellable},
};

const REPOSITORY_TIMEOUT: Duration = Duration::from_secs(3);
const REPOSITORY_OUTPUT_LIMIT: usize = 64 * 1024;

impl GitFacade {
    pub(crate) fn repository_binding(
        &self,
        project_path: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<RepositoryBinding, GitError> {
        authorize(project_path, grants, authorization_generation)?;
        let mut command = repository_command(project_path);
        command.args([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
            "HEAD",
        ]);
        let output = run_repository(command)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(super::classify_git_failure(&stderr).unwrap_or_else(|| {
                GitError::NotRepository("路径不在可读取的 Git 仓库内".to_owned())
            }));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| GitError::Command("Git 仓库信息不是有效 UTF-8".to_owned()))?;
        let mut lines = stdout.lines();
        let worktree = lines.next().ok_or_else(invalid_repository_output)?;
        let git_dir = lines.next().ok_or_else(invalid_repository_output)?;
        let common_dir = lines.next().ok_or_else(invalid_repository_output)?;
        let head = lines.next().ok_or_else(invalid_repository_output)?;
        if lines.next().is_some()
            || [worktree, git_dir, common_dir, head]
                .iter()
                .any(|line| line.is_empty())
        {
            return Err(invalid_repository_output());
        }
        let worktree = resolve_authorized(worktree, grants, authorization_generation)?;
        let git_dir = resolve_authorized(git_dir, grants, authorization_generation)?;
        let common_dir = resolve_authorized(common_dir, grants, authorization_generation)?;
        let head = CommitId::from_str(head)
            .map_err(|_| GitError::Command("Git HEAD 不是完整对象 ID".to_owned()))?;
        Ok(RepositoryBinding::new(worktree, git_dir, common_dir, head))
    }

    pub(crate) fn revalidate_binding(
        &self,
        original: &RepositoryBinding,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<(), GitError> {
        let current = self
            .repository_binding(
                original.worktree_root().canonical_path(),
                grants,
                authorization_generation,
            )
            .map_err(|_| GitError::BindingUnavailable)?;
        let mut command = repository_command(current.worktree_root().canonical_path());
        command.args(["cat-file", "-e", &format!("{}^{{commit}}", original.head())]);
        let original_head_exists = run_repository(command)
            .map_err(|_| GitError::BindingUnavailable)?
            .status
            .success();
        original
            .revalidate(&current, original_head_exists)
            .map_err(|_| GitError::BindingUnavailable)
    }
}

fn repository_command(path: &CanonicalPath) -> Command {
    let mut command = super::command(path.as_path());
    command
        .arg("--literal-pathspecs")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "");
    command
}

fn run_repository(command: Command) -> Result<ProcessOutput, GitError> {
    run_cancellable(
        command,
        REPOSITORY_TIMEOUT,
        REPOSITORY_OUTPUT_LIMIT,
        &|| false,
    )
    .map_err(|error| match error {
        ProcessOutputError::Spawn(error) => GitError::Unavailable(format!("无法运行 git: {error}")),
        ProcessOutputError::Io(_) => GitError::Command("Git 仓库读取失败".to_owned()),
        ProcessOutputError::Cancelled => GitError::Cancelled,
        ProcessOutputError::Timeout => GitError::Timeout,
        ProcessOutputError::OutputLimit { limit } => GitError::OutputLimit { limit },
        ProcessOutputError::Exit(_) => unreachable!("双通道进程接口保留非零退出码"),
    })
}

fn resolve_authorized(
    path: &str,
    grants: &[PathGrant],
    authorization_generation: u64,
) -> Result<ResolvedPath, GitError> {
    let path = CanonicalPath::new(path)
        .map_err(|_| GitError::Command("Git 返回了非规范仓库路径".to_owned()))?;
    authorize(&path, grants, authorization_generation)
}

fn authorize(
    path: &CanonicalPath,
    grants: &[PathGrant],
    authorization_generation: u64,
) -> Result<ResolvedPath, GitError> {
    let resolved = SystemDocumentPathResolver
        .resolve(path)
        .map_err(|_| GitError::Command("无法解析仓库真实路径".to_owned()))?;
    let request = PathRequest {
        operation: PathOperation::Read,
        lexical_path: path.as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: authorization_generation,
    };
    PathPolicy::authorize(&request, &resolved, grants, authorization_generation)
        .map(|authorization| authorization.resolved_path().clone())
        .map_err(|_| GitError::AccessDenied("仓库路径未授予读取能力".to_owned()))
}

fn invalid_repository_output() -> GitError {
    GitError::Command("Git 仓库信息不完整".to_owned())
}
