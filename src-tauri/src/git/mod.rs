//! git 集成模块 — git_status + git_diff 命令
//!
//! 用 git2 0.20 + vendored-libgit2（静态链接，无需系统 git）。
//! 阻塞 I/O 用 spawn_blocking 包裹。

use crate::error::AppError;
use serde::Serialize;

/// 文件 git 状态条目
#[derive(Debug, Clone, Serialize)]
pub struct GitStatusEntry {
    /// 相对仓库根的文件路径
    pub path: String,
    /// git 状态：modified | added | deleted | renamed | untracked | conflict | ignored
    pub status: String,
}

/// diff hunk 信息（old = HEAD, new = 工作区）
#[derive(Debug, Clone, Serialize)]
pub struct DiffHunk {
    /// HEAD 侧起始行号（1-based）
    pub old_start: u32,
    /// HEAD 侧行数
    pub old_lines: u32,
    /// 工作区侧起始行号（1-based）
    pub new_start: u32,
    /// 工作区侧行数
    pub new_lines: u32,
}

/// 获取指定仓库的文件 git 状态
///
/// 非 git 仓库返回 AppError::Git。
#[tauri::command]
pub async fn git_status(repo_path: String) -> Result<Vec<GitStatusEntry>, AppError> {
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| AppError::Git(format!("打开仓库失败: {e}")))?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_ignored(true)
            .include_unreadable(true)
            .include_unreadable_as_untracked(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| AppError::Git(format!("获取状态失败: {e}")))?;

        let mut entries: Vec<GitStatusEntry> = Vec::new();
        for entry in statuses.iter() {
            let path = entry
                .path()
                .unwrap_or("")
                .to_string()
                .replace('\\', "/");

            let status_flag = entry.status();
            let status_str = if status_flag.is_conflicted() {
                "conflict"
            } else if status_flag.contains(git2::Status::WT_DELETED)
                || status_flag.contains(git2::Status::INDEX_DELETED)
            {
                "deleted"
            } else if status_flag.contains(git2::Status::INDEX_RENAMED)
                || status_flag.contains(git2::Status::WT_RENAMED)
            {
                "renamed"
            } else if status_flag.contains(git2::Status::INDEX_NEW)
                || status_flag.contains(git2::Status::WT_NEW)
            {
                "added"
            } else if status_flag.contains(git2::Status::WT_MODIFIED)
                || status_flag.contains(git2::Status::INDEX_MODIFIED)
            {
                "modified"
            } else if status_flag.is_ignored() {
                "ignored"
            } else if status_flag.contains(git2::Status::WT_NEW) {
                "untracked"
            } else {
                continue; // 跳过 Current（无变更）
            };

            entries.push(GitStatusEntry {
                path,
                status: status_str.to_string(),
            });
        }

        Ok(entries)
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

/// 获取指定文件的 HEAD ↔ 工作区 diff hunks
///
/// 用于编辑器行内 diff 边栏。
/// 仓库尚无提交（UnbornBranch）时返回 Err。
#[tauri::command]
pub async fn git_diff(repo_path: String, file_path: String) -> Result<Vec<DiffHunk>, AppError> {
    tokio::task::spawn_blocking(move || {
        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| AppError::Git(format!("打开仓库失败: {e}")))?;

        // 获取 HEAD tree
        let tree = match repo.head() {
            Ok(head) => Some(
                head.peel_to_tree()
                    .map_err(|e| AppError::Git(format!("获取 HEAD tree 失败: {e}")))?,
            ),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(AppError::Git(format!("获取 HEAD 失败: {e}"))),
        };

        // 基准：HEAD tree vs 工作区+index
        let mut opts = git2::DiffOptions::new();
        // 将 file_path 转为相对于仓库根的路径
        let repo_path_std = std::path::Path::new(&repo_path);
        let file_path_std = std::path::Path::new(&file_path);
        let rel_path_str = file_path_std
            .strip_prefix(repo_path_std)
            .unwrap_or(file_path_std)
            .to_string_lossy()
            .replace('\\', "/");
        opts.pathspec(&rel_path_str);

        let diff = repo
            .diff_tree_to_workdir_with_index(tree.as_ref(), Some(&mut opts))
            .map_err(|e| AppError::Git(format!("生成 diff 失败: {e}")))?;

        let mut hunks: Vec<DiffHunk> = Vec::new();

        // 使用 DiffHunk 原生方法提取行信息
        diff.foreach(
            &mut |_delta, _num| true, // file callback — 继续
            None,                      // binary callback — 跳过
            Some(&mut |_delta, hunk| {
                hunks.push(DiffHunk {
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                });
                true
            }),
            None, // line callback — 不需要逐行处理
        )
        .map_err(|e| AppError::Git(format!("diff foreach 失败: {e}")))?;

        Ok(hunks)
    })
    .await
    .map_err(|e| AppError::Io(format!("spawn_blocking 失败: {e}")))?
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    /// 在临时目录中 init 一个 git 仓库，返回 tempdir（自动清理）和路径
    fn init_temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();

        Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .unwrap();

        // 配置 git user（needed for commit）
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&path)
            .output()
            .unwrap();

        (dir, path)
    }

    /// Helper: 在 git 仓库中 commit 一个文件
    fn commit_file(repo_path: &std::path::Path, filename: &str, content: &str) {
        let file_path = repo_path.join(filename);
        fs::write(&file_path, content).unwrap();
        Command::new("git")
            .args(["add", filename])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("commit {filename}")])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    /// Helper: git add 一个文件
    fn git_add(repo_path: &std::path::Path, filename: &str) {
        Command::new("git")
            .args(["add", filename])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    #[test]
    fn git_open_nonexistent_returns_err() {
        let tmp = tempfile::tempdir().unwrap();
        let result = git2::Repository::open(tmp.path());
        assert!(result.is_err(), "非 git 目录应返回错误");
    }

    #[test]
    fn git_status_empty_repo_no_files() {
        let (_dir, path) = init_temp_repo();
        let repo = git2::Repository::open(&path).unwrap();
        let statuses = repo.statuses(None).unwrap();
        assert!(
            statuses.is_empty(),
            "空仓库无文件应返回空状态"
        );
    }

    // ---- B2: git_status 综合测试 ----

    #[test]
    fn git_status_modified_file() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "original");

        // 修改文件
        fs::write(path.join("test.txt"), "modified content").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_modified = statuses.iter().any(|e| {
            e.status().contains(git2::Status::WT_MODIFIED)
        });
        assert!(has_modified, "修改后的文件应显示 WT_MODIFIED");
    }

    #[test]
    fn git_status_untracked_file() {
        let (_dir, path) = init_temp_repo();
        fs::write(path.join("new_file.txt"), "untracked").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_untracked = statuses
            .iter()
            .any(|e| e.status().contains(git2::Status::WT_NEW));
        assert!(has_untracked, "未 add 的新文件应显示 WT_NEW");
    }

    #[test]
    fn git_status_added_file() {
        let (_dir, path) = init_temp_repo();
        fs::write(path.join("staged.txt"), "staged").unwrap();
        git_add(&path, "staged.txt");

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_added = statuses
            .iter()
            .any(|e| e.status().contains(git2::Status::INDEX_NEW));
        assert!(has_added, "git add 后的文件应显示 INDEX_NEW");
    }

    #[test]
    fn git_status_deleted_file() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "to_delete.txt", "will be deleted");

        // 删除文件
        fs::remove_file(path.join("to_delete.txt")).unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_deleted = statuses
            .iter()
            .any(|e| e.status().contains(git2::Status::WT_DELETED));
        assert!(has_deleted, "删除已提交文件应显示 WT_DELETED");
    }

    #[test]
    fn git_status_non_repo_returns_err() {
        let tmp = tempfile::tempdir().unwrap();
        let result = git2::Repository::open(tmp.path());
        assert!(result.is_err(), "非 git 目录应返回错误");
    }

    // ---- B3: git_diff 综合测试 ----

    #[test]
    fn git_diff_returns_hunks() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "diff_test.txt", "line1\nline2\nline3\n");

        // 修改第 2 行
        fs::write(path.join("diff_test.txt"), "line1\nline2 MODIFIED\nline3\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        let mut opts = git2::DiffOptions::new();
        opts.pathspec("diff_test.txt");
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut hunk_count = 0u32;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, hunk| {
                // hunk 包含修改行的起止信息
                let old_lines = hunk.old_lines();
                let new_lines = hunk.new_lines();
                // 修改 1 行：old_lines ≥ 1, new_lines ≥ 1
                assert!(old_lines >= 1 && new_lines >= 1,
                    "修改 1 行的 hunk 应有 old_lines≥1 和 new_lines≥1, 实际: {old_lines}/{new_lines}");
                hunk_count += 1;
                true
            }),
            None,
        )
        .unwrap();

        assert!(hunk_count > 0, "应至少返回 1 个 hunk");
    }

    #[test]
    fn git_diff_new_file_no_head() {
        let (_dir, path) = init_temp_repo();
        // 无 commit → HEAD 为 UnbornBranch
        let repo = git2::Repository::open(&path).unwrap();
        let result = repo.head();
        match result {
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // 预期：空仓库 HEAD 不存在
            }
            _ => {
                panic!("空仓库应返回 UnbornBranch 错误");
            }
        }
    }

    #[test]
    fn git_diff_unchanged_file_empty_hunks() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "unchanged.txt", "same content\n");

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        let mut opts = git2::DiffOptions::new();
        opts.pathspec("unchanged.txt");
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut hunk_count = 0u32;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, _hunk| {
                hunk_count += 1;
                true
            }),
            None,
        )
        .unwrap();

        assert_eq!(hunk_count, 0, "未修改文件应返回 0 hunk");
    }

    #[test]
    fn git_diff_added_lines_hunk() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "add_lines.txt", "line1\n");

        // 追加 3 行
        fs::write(path.join("add_lines.txt"), "line1\nline2\nline3\nline4\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        let mut opts = git2::DiffOptions::new();
        opts.pathspec("add_lines.txt");
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut found_new_gt_old = false;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, hunk| {
                // 追加 3 行：old_lines=1, new_lines=4 (或类似)
                if hunk.new_lines() > hunk.old_lines() {
                    found_new_gt_old = true;
                }
                true
            }),
            None,
        )
        .unwrap();

        assert!(found_new_gt_old, "追加行后 new_lines 应 > old_lines");
    }

    #[test]
    fn git_diff_deleted_lines_hunk() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "del_lines.txt", "a\nb\nc\nd\ne\n");

        // 删除 2 行
        fs::write(path.join("del_lines.txt"), "a\nb\nc\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        let mut opts = git2::DiffOptions::new();
        opts.pathspec("del_lines.txt");
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut found_old_gt_new = false;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, hunk| {
                if hunk.old_lines() > hunk.new_lines() {
                    found_old_gt_new = true;
                }
                true
            }),
            None,
        )
        .unwrap();

        assert!(found_old_gt_new, "删除行后 old_lines 应 > new_lines");
    }
}
