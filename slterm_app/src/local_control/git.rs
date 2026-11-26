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
use crate::git::{self, GitError, GitFacade, StatusSnapshot};
use crate::text_document::{MAX_BYTES, TextSnapshot};

/// 将 porcelain v2 快照映射为线缆 DTO；非 Unicode 仓库路径明确失败。
pub(crate) fn git_status_response(
    project_id: ProjectId,
    root: &CanonicalPath,
    snapshot: &StatusSnapshot,
) -> Result<GitStatusResponse, JsonRpcError> {
    // root 是项目根；仓库可位于项目内子目录，状态条目的相对路径以真实
    // worktree 根为准。
    let repository_root = snapshot
        .worktree_root
        .as_ref()
        .map(|root| root.canonical_path())
        .unwrap_or(root);
    let root_text = repository_root.as_path().to_str().ok_or_else(|| {
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
            .map(|entry| {
                Ok(GitStatusEntry {
                    path: entry.path.clone(),
                    original: entry.original.clone(),
                    index: slterm_runtime_protocol::GitStatusCode::from_char(entry.index)
                        .ok_or_else(git_status_encoding_error)?,
                    worktree: slterm_runtime_protocol::GitStatusCode::from_char(entry.worktree)
                        .ok_or_else(git_status_encoding_error)?,
                    conflict: entry.conflict,
                    ignored: entry.ignored,
                    staged: entry.staged(),
                    unstaged: entry.unstaged(),
                })
            })
            .collect::<Result<_, _>>()?,
    })
}

fn git_status_encoding_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::NotReady,
        -32001,
        "git status encoding failed",
    )
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
    // 纵深防御：协议层已拒绝盘符绝对路径等形态，但仓库内符号链接／目录联接
    // 仍可把读取带出仓库根（甚至指向另一个仓库）。规范位置必须仍在绑定
    // 仓库根之内；不存在的目标按空内容处理，但其父链同样不得逃逸。
    match dunce::canonicalize(&path) {
        Ok(canonical) if canonical.starts_with(worktree_root.as_path()) => {}
        Ok(_) => return Err(diff_unavailable()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let Some(parent) = path.parent() else {
                return Err(diff_unavailable());
            };
            match dunce::canonicalize(parent) {
                Ok(parent) if parent.starts_with(worktree_root.as_path()) => {}
                _ => return Err(diff_unavailable()),
            }
        }
        Err(_) => return Err(diff_unavailable()),
    }
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

/// Git 破坏写的意图副本：目标解析在前台，执行只认已解析目标。
pub(crate) enum GitWriteIntent {
    Rollback {
        path: String,
        expected: slterm_runtime_protocol::GitRollbackExpected,
    },
    Delete {
        path: String,
        expected: slterm_runtime_protocol::GitDeleteExpected,
        permanent: bool,
    },
}

impl GitWriteIntent {
    /// 只从已登记的 Git 写意图派生；其他方法返回 None。
    pub(crate) fn from_mutation(mutation: &super::mutations::RuntimeMutation) -> Option<Self> {
        match mutation {
            super::mutations::RuntimeMutation::GitRollback { path, expected, .. } => {
                Some(Self::Rollback {
                    path: path.clone(),
                    expected: *expected,
                })
            }
            super::mutations::RuntimeMutation::GitDelete {
                path,
                expected,
                permanent,
                ..
            } => Some(Self::Delete {
                path: path.clone(),
                expected: *expected,
                permanent: *permanent,
            }),
            _ => None,
        }
    }
}

/// 在后台执行 Git 破坏写；写前重检失败不产生任何写盘。
///
/// 状态重检使用独立 facade，避免命中共享缓存；成功后使共享状态缓存失效。
pub(crate) fn execute_git_write(
    intent: GitWriteIntent,
    target: &crate::app_shell::GitWriteTarget,
) -> Result<(), JsonRpcError> {
    use crate::app_shell::app_root::git_changes::{
        DeleteChangeKind, GitChangeError, UnstagedChangeKind,
    };

    let git = GitFacade::default();
    let result = match intent {
        GitWriteIntent::Rollback { path, expected } => {
            let kind = match expected {
                slterm_runtime_protocol::GitRollbackExpected::Modified => {
                    UnstagedChangeKind::Modified
                }
                slterm_runtime_protocol::GitRollbackExpected::Deleted => {
                    UnstagedChangeKind::Deleted
                }
            };
            crate::app_shell::app_root::git_changes::rollback_unstaged(
                &git,
                &target.root,
                &target.root,
                target.project_id,
                &target.file_operations,
                &path,
                kind,
                "runtime",
            )
        }
        GitWriteIntent::Delete {
            path,
            expected,
            permanent,
        } => {
            let kind = match expected {
                slterm_runtime_protocol::GitDeleteExpected::Untracked => {
                    DeleteChangeKind::Untracked
                }
                slterm_runtime_protocol::GitDeleteExpected::StagedAdded => {
                    DeleteChangeKind::StagedAdded
                }
            };
            crate::app_shell::app_root::git_changes::delete_change(
                &git,
                &target.root,
                &target.root,
                target.project_id,
                &target.file_operations,
                &path,
                kind,
                permanent,
                "runtime",
            )
        }
    };
    match result {
        Ok(()) => {
            // 共享 facade 的短 TTL 缓存可能保留写前状态。
            target.git.invalidate();
            Ok(())
        }
        Err(GitChangeError::Precondition) => Err(git_precondition_error()),
        Err(GitChangeError::Unauthorized) => Err(git_error_to_rpc(GitError::AccessDenied(
            "写目标不在项目根内".to_owned(),
        ))),
        Err(GitChangeError::RecycleUnavailable | GitChangeError::Failed) => Err(control_error(
            DomainErrorCode::NotReady,
            -32001,
            "git write not ready",
        )),
    }
}

/// 破坏前置条件不再成立：客户端必须重新读取状态后再决定。
fn git_precondition_error() -> JsonRpcError {
    control_error(
        DomainErrorCode::RevisionConflict,
        -32005,
        "git state changed",
    )
}

/// Git 打开意图副本：目标解析在前台，绑定捕获与打开链在本线程后台完成。
pub(crate) enum GitOpenIntent {
    Diff { path: String },
    Deleted { path: String },
}

impl GitOpenIntent {
    /// 只从已登记的打开意图派生；其他方法返回 None。
    pub(crate) fn from_mutation(mutation: &super::mutations::RuntimeMutation) -> Option<Self> {
        match mutation {
            super::mutations::RuntimeMutation::GitOpenDiff { path, .. } => {
                Some(Self::Diff { path: path.clone() })
            }
            super::mutations::RuntimeMutation::GitOpenDeleted { path, .. } => {
                Some(Self::Deleted { path: path.clone() })
            }
            _ => None,
        }
    }
}

/// 复核状态映射后把打开请求交给前台；打开只复用 Commit 的同一打开链。
/// 打开执行结果：回执落定才是成功，投递后超时或断连只能证明结果不明。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitOpenOutcome {
    Completed,
    Unknown,
}

pub(crate) fn execute_git_open(
    intent: GitOpenIntent,
    target: &crate::app_shell::GitOpenTarget,
    events: &std::sync::mpsc::SyncSender<crate::app_shell::SltermShellEvent>,
    reply_timeout: std::time::Duration,
) -> Result<GitOpenOutcome, JsonRpcError> {
    use crate::app_shell::RuntimeGitOpenKind;
    use crate::app_shell::app_root::{CommitOpenKind, open_kind_for_status};

    let (path, expected) = match intent {
        GitOpenIntent::Diff { path } => (path, RuntimeGitOpenKind::Diff),
        GitOpenIntent::Deleted { path } => (path, RuntimeGitOpenKind::Deleted),
    };
    let grants =
        crate::app_shell::app_root::status_grants(&target.root).map_err(git_error_to_rpc)?;
    let snapshot = target
        .git
        .status(&target.root, &grants, 0)
        .map_err(git_error_to_rpc)?;
    // 打开映射固定：方法声明的形态必须与当前状态映射一致。
    let entry = snapshot
        .entries
        .iter()
        .find(|entry| entry.path.trim_matches('/') == path)
        .ok_or_else(git_target_changed)?;
    let open_kind =
        open_kind_for_status(entry.index, entry.worktree, entry.conflict, entry.ignored);
    let mapping_ok = match expected {
        RuntimeGitOpenKind::Diff => open_kind == Some(CommitOpenKind::Diff),
        RuntimeGitOpenKind::Deleted => open_kind == Some(CommitOpenKind::GitShow),
    };
    if !mapping_ok {
        return Err(git_target_changed());
    }
    let binding = target
        .git
        .repository_binding(&target.root, &grants, 0)
        .map_err(git_error_to_rpc)?;
    let worktree_root = binding.worktree_root().canonical_path();
    let path = CanonicalPath::new(
        worktree_root
            .as_path()
            .join(path.replace('/', std::path::MAIN_SEPARATOR_STR)),
    )
    .map_err(|_| git_target_changed())?;
    let receipt = crate::app_shell::try_send_runtime_git_open(
        events,
        target.project_id,
        binding,
        path,
        expected,
    )?;
    match receipt.recv_timeout(reply_timeout) {
        Ok(result) => result.map(|()| GitOpenOutcome::Completed),
        // 事件已经进入壳通道；无法取消或证明未执行，只能标记结果不明。
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(GitOpenOutcome::Unknown),
    }
}

/// 目标条目消失或与请求的打开方式不一致：调用方必须重新读取状态。
fn git_target_changed() -> JsonRpcError {
    control_error(
        DomainErrorCode::InvalidRequest,
        -32000,
        "git target changed",
    )
}
