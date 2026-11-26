//! GitFacade 的真实仓库绑定捕获与复核；全部进程读取有界且禁网。

use std::path::Path;
use std::process::Command;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slterm_document::{CommitId, DiffBinding, RepoPath, RepositoryBinding};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};

use super::{GitError, GitFacade, StatusSnapshot, completion::RepositoryStatusCache};
use crate::platform::{
    document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver},
    process_output::{ProcessOutput, ProcessOutputError, run_cancellable},
};

const REPOSITORY_TIMEOUT: Duration = Duration::from_secs(3);
/// 单次仓库命令合并输出预算；head_file 等按调用方 limit 叠加该余量读取。
pub(super) const REPOSITORY_OUTPUT_LIMIT: usize = 1024 * 1024;

const STATUS_CACHE_TTL: Duration = Duration::from_millis(500);

impl GitFacade {
    pub(crate) fn status(
        &self,
        project_path: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<Arc<StatusSnapshot>, GitError> {
        authorize(project_path, grants, authorization_generation)?;
        let root = capture_worktree_root(project_path, grants, authorization_generation)?;
        let mut cache = self
            .status_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.as_ref()
            && cached.root == *root.canonical_path()
            && cached.captured.elapsed() < STATUS_CACHE_TTL
        {
            return Ok(cached.snapshot.clone());
        }
        let generation = self.generation();
        let mut snapshot = super::status(root.canonical_path().as_path())?;
        snapshot.worktree_root = Some(root.clone());
        let snapshot = Arc::new(snapshot);
        if self.is_current(generation) {
            *cache = Some(RepositoryStatusCache {
                root: root.canonical_path().clone(),
                captured: Instant::now(),
                snapshot: snapshot.clone(),
            });
        }
        Ok(snapshot)
    }

    pub(crate) fn repository_binding(
        &self,
        project_path: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<RepositoryBinding, GitError> {
        authorize(project_path, grants, authorization_generation)?;
        let mut command = repository_command(project_path.as_path());
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

    /// 以项目内真实路径捕获当前 HEAD，并固定 Document 对应的仓库相对旧路径。
    pub(crate) fn diff_binding_for_path(
        &self,
        project_path: &CanonicalPath,
        document_path: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<DiffBinding, GitError> {
        let binding = self.repository_binding(project_path, grants, authorization_generation)?;
        let relative = document_path
            .as_path()
            .strip_prefix(binding.worktree_root().canonical_path().as_path())
            .map_err(|_| GitError::NotRepository("Document 不在仓库工作区内".to_owned()))?;
        let relative = relative
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| GitError::Command("仓库相对路径不是有效 UTF-8".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let path = RepoPath::from_str(&relative)
            .map_err(|_| GitError::Command("Document 路径不是规范仓库相对路径".to_owned()))?;
        Ok(DiffBinding::new(binding, path))
    }

    pub(crate) fn revalidate_binding(
        &self,
        original: &RepositoryBinding,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<(), GitError> {
        let (current, original_head_exists) =
            self.current_binding_and_original_head(original, grants, authorization_generation)?;
        original
            .revalidate(&current, original_head_exists)
            .map_err(|_| GitError::BindingUnavailable)
    }

    /// 只在原仓库身份仍成立时推进 Diff HEAD，旧路径保持不变。
    pub(crate) fn refresh_diff_binding(
        &self,
        original: &DiffBinding,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<DiffBinding, GitError> {
        let (current, original_head_exists) = self.current_binding_and_original_head(
            original.repository_binding(),
            grants,
            authorization_generation,
        )?;
        let mut refreshed = original.clone();
        refreshed
            .refresh_head(&current, original_head_exists)
            .map_err(|_| GitError::BindingUnavailable)?;
        Ok(refreshed)
    }

    fn current_binding_and_original_head(
        &self,
        original: &RepositoryBinding,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<(RepositoryBinding, bool), GitError> {
        let current = self
            .repository_binding(
                original.worktree_root().canonical_path(),
                grants,
                authorization_generation,
            )
            .map_err(|_| GitError::BindingUnavailable)?;
        let mut command = repository_command(current.worktree_root().canonical_path().as_path());
        command.args(["cat-file", "-e", &format!("{}^{{commit}}", original.head())]);
        let original_head_exists = run_repository(command)
            .map_err(|_| GitError::BindingUnavailable)?
            .status
            .success();
        Ok((current, original_head_exists))
    }
}

fn capture_worktree_root(
    project_path: &CanonicalPath,
    grants: &[PathGrant],
    authorization_generation: u64,
) -> Result<ResolvedPath, GitError> {
    let mut command = repository_command(project_path.as_path());
    command.args(["rev-parse", "--path-format=absolute", "--show-toplevel"]);
    let output = run_repository(command)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(super::classify_git_failure(&stderr)
            .unwrap_or_else(|| GitError::NotRepository("路径不在可读取的 Git 仓库内".to_owned())));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| GitError::Command("Git 仓库根不是有效 UTF-8".to_owned()))?;
    let mut lines = stdout.lines();
    let root = lines.next().ok_or_else(invalid_repository_output)?;
    if root.is_empty() || lines.next().is_some() {
        return Err(invalid_repository_output());
    }
    resolve_authorized(root, grants, authorization_generation)
}

pub(super) fn repository_command(path: &Path) -> Command {
    let mut command = super::command(path);
    command
        .arg("--literal-pathspecs")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "");
    command
}

pub(super) fn run_repository(command: Command) -> Result<ProcessOutput, GitError> {
    run_repository_with_limit(command, REPOSITORY_OUTPUT_LIMIT)
}

pub(super) fn run_repository_with_limit(
    command: Command,
    output_limit: usize,
) -> Result<ProcessOutput, GitError> {
    run_cancellable(command, REPOSITORY_TIMEOUT, output_limit, &|| false).map_err(|error| {
        match error {
            ProcessOutputError::Spawn(error) => {
                GitError::Unavailable(format!("无法运行 git: {error}"))
            }
            ProcessOutputError::Io(_) => GitError::Command("Git 仓库读取失败".to_owned()),
            ProcessOutputError::Cancelled => GitError::Cancelled,
            ProcessOutputError::Timeout => GitError::Timeout,
            ProcessOutputError::OutputLimit { limit } => GitError::OutputLimit { limit },
            ProcessOutputError::Exit(_) => unreachable!("双通道进程接口保留非零退出码"),
        }
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
