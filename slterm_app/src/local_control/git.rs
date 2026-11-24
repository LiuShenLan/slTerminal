//! Runtime Git 状态与 diff：唯一读取入口在后台，前台只解析目标与授权。

use sha2::{Digest as _, Sha256};
use slterm_document::{
    DiffInput, DiffPairKind, DiffSnapshot, DiffWhitespaceMode, TextStateId, compute_diff,
};
use slterm_path::CanonicalPath;
use slterm_runtime_protocol::{DomainErrorCode, GitStatusEntry, GitStatusResponse, JsonRpcError};
use slterm_workspace::ProjectId;

use crate::app_shell::control_error;
use crate::document_io::read_prefix;
use crate::git::{self, GitError, StatusSnapshot};
use crate::text_document::{MAX_BYTES, TextSnapshot};

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

/// Git diff 文本快照：`revision` 由正文散列派生，供分页游标失效判定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitDiffSnapshot {
    pub(crate) revision: u64,
    pub(crate) text: String,
}

/// 在后台计算单个仓库相对路径的 HEAD 与工作区差异文本。
///
/// 两侧都按同一编码解码路径读取；HEAD 不存在视为全文新增，工作区缺失
/// 视为全文删除。二进制、非法编码、超限或差分预算失败都明确报错，
/// 不把降级占位伪装成空差异。
pub(crate) fn git_diff_snapshot(
    target: &crate::app_shell::RuntimeGitDiffTarget,
) -> Result<GitDiffSnapshot, JsonRpcError> {
    let grants =
        crate::app_shell::app_root::status_grants(&target.root).map_err(git_error_to_rpc)?;
    let binding = target
        .git
        .repository_binding(&target.root, &grants, 0)
        .map_err(git_error_to_rpc)?;
    let worktree_root = binding.worktree_root().canonical_path();
    let path = worktree_root.as_path().join(&target.path);
    let head_text = match git::head_file(&path, MAX_BYTES) {
        Ok((bytes, false)) => decode_text(bytes)?,
        Ok((_, true)) => return Err(diff_too_large()),
        Err(GitError::NotInHead(_)) => String::new(),
        Err(error) => return Err(git_error_to_rpc(error)),
    };
    let worktree_text = match std::fs::File::open(&path) {
        Ok(mut file) => {
            let length = file.metadata().map_err(|_| diff_unavailable())?.len();
            let (bytes, truncated) =
                read_prefix(&mut file, length, MAX_BYTES).map_err(|_| diff_unavailable())?;
            if truncated {
                return Err(diff_too_large());
            }
            decode_text(bytes)?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(diff_unavailable()),
    };
    let input = DiffInput::git(
        head_text.into_bytes().into(),
        worktree_text.into_bytes().into(),
        DiffWhitespaceMode::default(),
        TextStateId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7"),
        binding.head().clone(),
        0,
    );
    let snapshot = compute_diff(&input).map_err(|_| diff_unavailable())?;
    let text = render_diff(&snapshot)?;
    Ok(GitDiffSnapshot {
        revision: text_revision(&text),
        text,
    })
}

fn decode_text(bytes: Vec<u8>) -> Result<String, JsonRpcError> {
    let snapshot = TextSnapshot::decode_prefix(bytes, true, false);
    if snapshot.invalid_encoding {
        return Err(diff_unavailable());
    }
    Ok(snapshot.text)
}

/// 差异文本只含变更行：删除行前缀 `-`、新增行前缀 `+`，等值行不输出。
fn render_diff(snapshot: &DiffSnapshot) -> Result<String, JsonRpcError> {
    let pairs = snapshot.pairs().ok_or_else(diff_unavailable)?;
    let mut text = String::new();
    for pair in pairs {
        match pair.kind {
            DiffPairKind::Equal => {}
            DiffPairKind::Delete => {
                if let Some(line) = &pair.head {
                    push_diff_line(&mut text, '-', &line.text);
                }
            }
            DiffPairKind::Insert => {
                if let Some(line) = &pair.worktree {
                    push_diff_line(&mut text, '+', &line.text);
                }
            }
            DiffPairKind::Replace => {
                if let Some(line) = &pair.head {
                    push_diff_line(&mut text, '-', &line.text);
                }
                if let Some(line) = &pair.worktree {
                    push_diff_line(&mut text, '+', &line.text);
                }
            }
        }
    }
    Ok(text)
}

fn push_diff_line(text: &mut String, prefix: char, line: &str) {
    text.push(prefix);
    text.push_str(line);
    text.push('\n');
}

fn text_revision(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 至少 8 字节"))
}

fn diff_too_large() -> JsonRpcError {
    control_error(DomainErrorCode::NotReady, -32001, "git diff too large")
}

fn diff_unavailable() -> JsonRpcError {
    control_error(DomainErrorCode::NotReady, -32001, "git diff not ready")
}
