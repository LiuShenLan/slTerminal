//! Runtime Git 状态：唯一读取入口在后台，前台只解析目标与授权。

use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{DomainErrorCode, GitStatusEntry, GitStatusResponse, JsonRpcError};
use slterm_workspace::ProjectId;

use crate::app_shell::control_error;
use crate::git::{GitError, StatusSnapshot};

/// 将 porcelain v2 快照映射为线缆 DTO；非 Unicode 仓库路径明确失败。
pub(crate) fn git_status_response(
    project_id: ProjectId,
    root: &CanonicalPath,
    snapshot: &StatusSnapshot,
) -> Result<GitStatusResponse, JsonRpcError> {
    let root_text = root.as_path().to_str().ok_or_else(|| {
        control_error(
            DomainErrorCode::NotReady,
            -32001,
            "git status encoding failed",
        )
    })?;
    Ok(GitStatusResponse {
        project_id: project_id.to_string(),
        root: root_text.to_owned(),
        branch: snapshot.branch.clone(),
        head: snapshot.head.clone(),
        upstream: snapshot.upstream.clone(),
        ahead: snapshot.ahead,
        behind: snapshot.behind,
        entries: snapshot
            .entries
            .iter()
            .map(|entry| GitStatusEntry {
                path: entry.path.clone(),
                original: entry.original.clone(),
                index: entry.index.to_string(),
                worktree: entry.worktree.to_string(),
                conflict: entry.conflict,
                ignored: entry.ignored,
                staged: entry.staged(),
                unstaged: entry.unstaged(),
            })
            .collect(),
    })
}

/// Git 错误映射为封闭领域码；不携带命令输出、路径或仓库正文。
pub(crate) fn git_error_to_rpc(error: GitError) -> JsonRpcError {
    match error {
        GitError::AccessDenied(_) => {
            control_error(DomainErrorCode::Unauthorized, -32002, "git access denied")
        }
        _ => control_error(DomainErrorCode::NotReady, -32001, "git status not ready"),
    }
}
