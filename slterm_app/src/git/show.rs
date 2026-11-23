//! 固定仓库、提交与路径的 GitShow 对象读取；不回退工作区或后续 HEAD。

use std::{str::FromStr as _, sync::Arc};

use slterm_document::{CommitId, RepoPath, RepositoryBinding};
use slterm_path::PathGrant;

use super::{
    GitError, GitFacade,
    repository::{repository_command, run_repository},
};

#[derive(Clone, Debug)]
pub(crate) struct GitShowSnapshot {
    pub(crate) commit: CommitId,
    pub(crate) path: RepoPath,
    pub(crate) blob: CommitId,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) generation: u64,
}

impl GitFacade {
    pub(crate) fn show(
        &self,
        binding: &RepositoryBinding,
        path: &RepoPath,
        grants: &[PathGrant],
        authorization_generation: u64,
        generation: u64,
    ) -> Result<GitShowSnapshot, GitError> {
        self.revalidate_binding(binding, grants, authorization_generation)?;
        let mut ls_tree = repository_command(binding.worktree_root().canonical_path().as_path());
        ls_tree.args([
            "ls-tree",
            "-z",
            binding.head().as_str(),
            "--",
            path.as_str(),
        ]);
        let output = run_repository(ls_tree)?;
        if !output.status.success() {
            return Err(object_failure(&output.stderr));
        }
        let output = String::from_utf8(output.stdout)
            .map_err(|_| GitError::Command("ls-tree 输出不是有效 UTF-8".to_owned()))?;
        let blob = parse_blob(&output, path)?;
        let mut cat_file = repository_command(binding.worktree_root().canonical_path().as_path());
        cat_file.args(["cat-file", "blob", blob.as_str()]);
        let content = run_repository(cat_file)?;
        if !content.status.success() {
            return Err(object_failure(&content.stderr));
        }
        Ok(GitShowSnapshot {
            commit: binding.head().clone(),
            path: path.clone(),
            blob,
            bytes: Arc::from(content.stdout),
            generation,
        })
    }
}

fn parse_blob(output: &str, path: &RepoPath) -> Result<CommitId, GitError> {
    let mut records = output.split_terminator('\0');
    let record = records.next().ok_or(GitError::ObjectUnavailable)?;
    let (metadata, actual_path) = record.split_once('\t').ok_or(GitError::ObjectUnavailable)?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if records.next().is_some()
        || actual_path != path.as_str()
        || fields.len() != 3
        || fields[1] != "blob"
    {
        return Err(GitError::ObjectUnavailable);
    }
    CommitId::from_str(fields[2]).map_err(|_| GitError::ObjectUnavailable)
}

fn object_failure(stderr: &[u8]) -> GitError {
    let stderr = String::from_utf8_lossy(stderr);
    super::classify_git_failure(&stderr).unwrap_or(GitError::ObjectUnavailable)
}
