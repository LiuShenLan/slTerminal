//! 未暂存回滚与受限删除的唯一执行体。
//!
//! 两者都只在后台调用：先重读状态复核调用方声明的前置形态，再执行受限写；
//! 校验失败不触碰工作区。写授权只来自项目根，仓库兄弟位置仅可读。
//! 破坏写进程内串行，并只经 audit 通道记录 operation/path/source/result。

use slterm_path::{CanonicalPath, PathGrant};
use slterm_workspace::ProjectId;

use crate::app_shell::file_operation_service::{FileOperationError, FileOperationService};
use crate::audit::{self, AuditEvent, AuditResult};
use crate::git::{GitError, GitFacade, StatusEntry, StatusSnapshot};

use super::explorer::status_grants;

/// 回滚形态：只覆盖精确的未暂存修改或删除。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnstagedChangeKind {
    Modified,
    Deleted,
}

/// 删除形态：只覆盖未跟踪文件与已暂存新增。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeleteChangeKind {
    Untracked,
    StagedAdded,
}

/// 写前重检或执行失败；`Precondition` 保证未写盘。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitChangeError {
    Precondition,
    Unauthorized,
    /// 回收站不可用：唯一允许用户显式升级为永久删除的失败。
    RecycleUnavailable,
    Failed,
}

impl From<GitError> for GitChangeError {
    fn from(error: GitError) -> Self {
        match error {
            GitError::AccessDenied(_) => Self::Unauthorized,
            _ => Self::Failed,
        }
    }
}

impl From<FileOperationError> for GitChangeError {
    fn from(error: FileOperationError) -> Self {
        match error {
            FileOperationError::Unauthorized => Self::Unauthorized,
            FileOperationError::RecycleBin(_) => Self::RecycleUnavailable,
            _ => Self::Failed,
        }
    }
}

/// 放弃精确的未暂存更改并恢复索引版本；不重置索引、不切换 HEAD。
///
/// `git_root` 只用于状态读取与仓库解析；`write_root` 是唯一写授权来源。
#[allow(clippy::too_many_arguments)]
pub(crate) fn rollback_unstaged(
    git: &GitFacade,
    git_root: &CanonicalPath,
    write_root: &CanonicalPath,
    project_id: ProjectId,
    file_operations: &FileOperationService,
    repo_relative: &str,
    kind: UnstagedChangeKind,
    source: &'static str,
) -> Result<(), GitChangeError> {
    let _guard = write_lock();
    let mut audited_path = None;
    let result = (|| {
        let grants = status_grants(git_root)?;
        let snapshot = git.status(git_root, &grants, 0)?;
        let repo_root = worktree_root(&snapshot, git_root);
        let target = repo_path(&repo_root, repo_relative);
        audited_path = Some(target.clone());
        // 写能力只来自项目根；仓库根兄弟位置仅可读，不得借此扩大写授权。
        if !target.starts_with(write_root.as_path()) {
            return Err(GitChangeError::Unauthorized);
        }
        let expected_worktree = match kind {
            UnstagedChangeKind::Modified => 'M',
            UnstagedChangeKind::Deleted => 'D',
        };
        // 写前重检：未暂存形态必须仍在，且索引形态未变为新增/删除/改名/冲突。
        let current = find_entry(&snapshot, repo_relative)?;
        let changed = current.conflict
            || current.worktree != expected_worktree
            || matches!(current.index, 'A' | 'D' | 'R' | 'U');
        if changed {
            return Err(GitChangeError::Precondition);
        }
        // 未暂存 rename：旧路径 deleted 与同目录内内容相同的 untracked 镜像
        // 成对出现。跨目录同内容文件不是改名事实，不参与清理。
        let target_parent = repo_path(&repo_root, repo_relative)
            .parent()
            .map(std::path::Path::to_path_buf);
        let mirror = if kind == UnstagedChangeKind::Deleted {
            let indexed = crate::git::index_bytes(repo_root.as_path(), repo_relative)?;
            snapshot
                .entries
                .iter()
                .filter(|entry| entry.index == '?')
                .find_map(|entry| {
                    let candidate = repo_path(&repo_root, entry.path.trim_matches('/'));
                    if target_parent.as_deref() != candidate.parent() {
                        return None;
                    }
                    if !candidate.starts_with(write_root.as_path()) {
                        return None;
                    }
                    (std::fs::read(&candidate).ok()? == indexed).then_some(candidate)
                })
        } else {
            None
        };
        crate::git::restore_from_index(repo_root.as_path(), &[repo_relative])?;
        // 新路径清除同样经统一写授权与身份复核；失败属于部分成功，如实上报。
        if let Some(mirror) = mirror {
            let lexical = CanonicalPath::new(mirror).map_err(|_| GitChangeError::Unauthorized)?;
            let grants = [PathGrant::project_root(write_root.clone())];
            let generation = crate::app_shell::authorization::current_generation();
            let plan = file_operations.plan_delete(
                slterm_sidebar::FileTreeRoot::Project(project_id),
                &lexical,
                &grants,
                generation,
            )?;
            file_operations.execute(
                &plan,
                &grants,
                crate::app_shell::authorization::current_generation(),
            )?;
        }
        Ok(())
    })();
    record_git_change("git_rollback", source, audited_path.as_deref(), &result);
    result
}

/// 删除未跟踪文件或已暂存新增；回收站不可用时明确失败，不隐式升级为永久删除。
#[allow(clippy::too_many_arguments)]
pub(crate) fn delete_change(
    git: &GitFacade,
    git_root: &CanonicalPath,
    write_root: &CanonicalPath,
    project_id: ProjectId,
    file_operations: &FileOperationService,
    repo_relative: &str,
    kind: DeleteChangeKind,
    permanent: bool,
    source: &'static str,
) -> Result<(), GitChangeError> {
    let _guard = write_lock();
    let mut audited_path = None;
    let result = (|| {
        let grants = status_grants(git_root)?;
        let snapshot = git.status(git_root, &grants, 0)?;
        let repo_root = worktree_root(&snapshot, git_root);
        let target_path = repo_path(&repo_root, repo_relative);
        audited_path = Some(target_path.clone());
        // 写能力只来自项目根；仓库兄弟位置仅可读。
        if !target_path.starts_with(write_root.as_path()) {
            return Err(GitChangeError::Unauthorized);
        }
        // 写前重检：目标必须仍是同形态的 untracked 或 staged added。
        let current = find_entry(&snapshot, repo_relative)?;
        let shape_ok = match kind {
            DeleteChangeKind::Untracked => current.index == '?' && current.worktree == '?',
            DeleteChangeKind::StagedAdded => current.index == 'A' && current.worktree == '.',
        };
        if !shape_ok || current.conflict {
            return Err(GitChangeError::Precondition);
        }
        // staged added 先取消暂存，失败即中止，避免索引与工作区不一致。
        if kind == DeleteChangeKind::StagedAdded {
            crate::git::unstage_path(repo_root.as_path(), repo_relative)?;
        }
        let target = CanonicalPath::new(target_path).map_err(|_| GitChangeError::Unauthorized)?;
        let write_grants = [PathGrant::project_root(write_root.clone())];
        // 永久删除同样经统一计划/执行体：身份、父目录、链接与授权复核不因
        // 用户再次确认而被绕过。
        let generation = crate::app_shell::authorization::current_generation();
        let plan = if permanent {
            file_operations.plan_delete_permanent(
                slterm_sidebar::FileTreeRoot::Project(project_id),
                &target,
                &write_grants,
                generation,
            )?
        } else {
            file_operations.plan_delete(
                slterm_sidebar::FileTreeRoot::Project(project_id),
                &target,
                &write_grants,
                generation,
            )?
        };
        file_operations.execute(
            &plan,
            &write_grants,
            crate::app_shell::authorization::current_generation(),
        )?;
        Ok(())
    })();
    record_git_change("git_delete", source, audited_path.as_deref(), &result);
    result
}

/// 破坏操作进程内串行：面板与 Runtime 不得交错重检与写盘。
fn write_lock() -> std::sync::MutexGuard<'static, ()> {
    static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    WRITE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 审计只记录操作、来源、路径与封闭结果，不携带命令输出或正文。
fn record_git_change(
    operation: &'static str,
    source: &'static str,
    path: Option<&std::path::Path>,
    result: &Result<(), GitChangeError>,
) {
    let audit_result = if result.is_ok() {
        AuditResult::Ok
    } else {
        AuditResult::Error
    };
    let mut event = AuditEvent::new(operation, source, audit_result);
    if let Some(path) = path {
        event = event.with_path(path);
    }
    audit::record(event);
}

/// 仓库工作树根：状态事实优先，纯解析输入回退到读取入口。
fn worktree_root(snapshot: &StatusSnapshot, fallback: &CanonicalPath) -> CanonicalPath {
    snapshot
        .worktree_root
        .as_ref()
        .map(|root| root.canonical_path().clone())
        .unwrap_or_else(|| fallback.clone())
}

fn find_entry<'a>(
    snapshot: &'a StatusSnapshot,
    repo_relative: &str,
) -> Result<&'a StatusEntry, GitChangeError> {
    snapshot
        .entries
        .iter()
        .find(|entry| entry.path.trim_matches('/') == repo_relative)
        .ok_or(GitChangeError::Precondition)
}

fn repo_path(repo_root: &CanonicalPath, repo_relative: &str) -> std::path::PathBuf {
    repo_root
        .as_path()
        .join(repo_relative.replace('/', std::path::MAIN_SEPARATOR_STR))
}
