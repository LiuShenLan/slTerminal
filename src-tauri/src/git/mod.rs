//! git 集成模块 — git_status + git_diff 命令
//!
//! 用 git2 0.20 + vendored-libgit2（静态链接，无需系统 git）。
//! 阻塞 I/O 用 spawn_blocking 包裹。

use crate::error::AppError;
use serde::Serialize;

/// 文件 git 状态条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    /// 文件绝对路径（repo_path + git2 相对路径，与 fs_read_dir 的 DirEntry.path 格式一致）
    pub path: String,
    /// git 状态：modified | added | deleted | renamed | untracked | conflict | ignored
    pub status: String,
}

/// diff hunk 信息（old = HEAD, new = 工作区）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
        let repo = // 用 discover 而非 open——支持从子目录上溯查找 .git
        git2::Repository::discover(&repo_path)
            .map_err(|e| AppError::Git(format!("打开仓库失败: {e}")))?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_ignored(true)
            .include_unreadable(true)
            .include_unreadable_as_untracked(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| AppError::Git(format!("获取状态失败: {e}")))?;

        // 用 repo.workdir() 获取仓库根（git2 返回的路径相对此目录），
        // 而非信任传入的 repo_path（可能为子目录 cwd）。
        // dunce::simplified() 剥离 Windows \\?\ 扩展路径前缀，
        // 确保与 fs_read_dir 返回的 DirEntry.path 格式一致。
        let workdir_raw = repo
            .workdir()
            .ok_or_else(|| AppError::Git("仓库无工作目录（可能为 bare repo）".to_string()))?;
        let workdir = dunce::simplified(workdir_raw);
        let mut entries: Vec<GitStatusEntry> = Vec::new();
        for entry in statuses.iter() {
            let rel = entry
                .path()
                .unwrap_or("")
                .to_string()
                .replace('\\', "/");
            // 拼接为绝对路径：workdir + "/" + rel
            let path = workdir
                .join(&rel)
                .to_string_lossy()
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
        let repo = // 用 discover 而非 open——支持从子目录上溯查找 .git（前端可能传父目录而非仓库根）
        git2::Repository::discover(&repo_path)
            .or_else(|_| git2::Repository::discover(&file_path))
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
        // 用 repo.workdir() 做 strip_prefix 基准（而非信任传入的 repo_path）。
        // dunce::simplified() 剥离 Windows \\?\ 扩展路径前缀，
        // 确保与 fs_read_dir 路径格式一致，strip_prefix 不会因前缀不同而失败。
        let workdir_raw = repo
            .workdir()
            .ok_or_else(|| AppError::Git("仓库无工作目录（可能为 bare repo）".to_string()))?;
        let workdir = dunce::simplified(workdir_raw);
        let file_path_std = std::path::Path::new(&file_path);
        let rel_path_str = file_path_std
            .strip_prefix(workdir)
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

    // ---- P1: git_status 绝对路径验证 ----

    /// 验证路径拼接逻辑：repo_path + git2 相对路径 = 绝对路径
    fn status_entry_to_absolute(repo_path: &std::path::Path, rel: &str) -> String {
        repo_path.join(rel).to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn git_status_absolute_path_for_root_file() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello");
        fs::write(path.join("test.txt"), "modified").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");

        for entry in statuses.iter() {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            let abs = status_entry_to_absolute(&path, &rel);
            assert!(
                abs.starts_with(&repo_path_str),
                "绝对路径应以仓库根开头: {abs} vs {repo_path_str}"
            );
            assert!(
                abs.ends_with("test.txt"),
                "绝对路径应以文件名结尾: {abs}"
            );
            // 应为 repo_path + "/" + filename 格式（非 Windows 原始反斜杠）
            assert!(!abs.contains('\\'), "路径不应含反斜杠: {abs}");
        }
    }

    #[test]
    fn git_status_absolute_path_for_nested_file() {
        let (_dir, path) = init_temp_repo();
        // 在子目录中创建文件
        let sub_dir = path.join("src");
        std::fs::create_dir(&sub_dir).unwrap();
        let file_path = sub_dir.join("main.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();
        git_add(&path, "src/main.rs");
        Command::new("git")
            .args(["commit", "-m", "add src/main.rs"])
            .current_dir(&path)
            .output()
            .unwrap();
        // 修改以产生 status
        std::fs::write(&file_path, "fn main() { println!(); }").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");

        for entry in statuses.iter() {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            let abs = status_entry_to_absolute(&path, &rel);
            assert!(
                abs.starts_with(&repo_path_str),
                "嵌套文件路径应以仓库根开头: {abs}"
            );
            assert!(
                abs.ends_with("src/main.rs"),
                "嵌套文件路径应含完整子路径: {abs}"
            );
            assert!(!abs.contains('\\'), "路径不应含反斜杠: {abs}");
        }
    }

    #[test]
    fn git_status_modified_file_absolute_path() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "original");
        fs::write(path.join("test.txt"), "modified content").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");
        let mut found = false;
        for entry in statuses.iter() {
            if entry.status().contains(git2::Status::WT_MODIFIED) {
                let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
                let abs = status_entry_to_absolute(&path, &rel);
                assert_eq!(abs, format!("{repo_path_str}/test.txt"),
                    "modified 文件的绝对路径应为 repo/test.txt");
                found = true;
            }
        }
        assert!(found, "应找到 modified 文件");
    }

    #[test]
    fn git_status_untracked_file_absolute_path() {
        let (_dir, path) = init_temp_repo();
        fs::write(path.join("new_file.txt"), "untracked").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");
        let mut found = false;
        for entry in statuses.iter() {
            if entry.status().contains(git2::Status::WT_NEW) && !entry.status().contains(git2::Status::INDEX_NEW) {
                let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
                let abs = status_entry_to_absolute(&path, &rel);
                assert_eq!(abs, format!("{repo_path_str}/new_file.txt"),
                    "untracked 文件的绝对路径应为 repo/new_file.txt");
                found = true;
            }
        }
        assert!(found, "应找到 untracked 文件");
    }

    #[test]
    fn git_status_added_file_absolute_path() {
        let (_dir, path) = init_temp_repo();
        fs::write(path.join("staged.txt"), "staged").unwrap();
        git_add(&path, "staged.txt");

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");
        let mut found = false;
        for entry in statuses.iter() {
            if entry.status().contains(git2::Status::INDEX_NEW) {
                let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
                let abs = status_entry_to_absolute(&path, &rel);
                assert_eq!(abs, format!("{repo_path_str}/staged.txt"),
                    "added 文件的绝对路径应为 repo/staged.txt");
                found = true;
            }
        }
        assert!(found, "应找到 added 文件");
    }

    #[test]
    fn git_status_deleted_file_absolute_path() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "to_delete.txt", "will be deleted");
        fs::remove_file(path.join("to_delete.txt")).unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let repo_path_str = path.to_string_lossy().replace('\\', "/");
        let mut found = false;
        for entry in statuses.iter() {
            if entry.status().contains(git2::Status::WT_DELETED) {
                let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
                let abs = status_entry_to_absolute(&path, &rel);
                assert_eq!(abs, format!("{repo_path_str}/to_delete.txt"),
                    "deleted 文件的绝对路径应为 repo/to_delete.txt");
                found = true;
            }
        }
        assert!(found, "应找到 deleted 文件");
    }

    #[test]
    fn git_status_path_matches_fs_read_dir_format() {
        // A10: 验证 git_status 路径格式与 fs_read_dir 一致
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "compare.txt", "data");
        fs::write(path.join("compare.txt"), "changed").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        // 模拟 fs_read_dir 的路径格式
        let fs_read_dir_path = path.join("compare.txt")
            .to_string_lossy()
            .replace('\\', "/");

        for entry in statuses.iter() {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            let abs = status_entry_to_absolute(&path, &rel);
            assert_eq!(abs, fs_read_dir_path,
                "git_status 的绝对路径应与 fs_read_dir 的 DirEntry.path 格式完全一致");
        }
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

    // ---- P8+P11: git_diff 路径验证 ----

    /// 验证 pathspec 使用 workdir() 而非传入的 repo_path。
    /// 从子目录传入 repo_path 不影响实际的 pathspec 计算。
    #[test]
    fn git_diff_pathspec_uses_workdir() {
        let (_dir, path) = init_temp_repo();
        // 先创建子目录，再 commit 文件
        std::fs::create_dir_all(path.join("src")).unwrap();
        commit_file(&path, "src/main.rs", "fn main() {}\n");

        // 在子目录中修改文件
        std::fs::write(path.join("src").join("main.rs"), "fn main() { println!(); }\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        // 模拟：从子目录调用 git_diff，传入 parent_dir 作为 repo_path
        let workdir = repo.workdir().unwrap();
        let file_path = path.join("src").join("main.rs");
        let rel = file_path.strip_prefix(workdir).unwrap().to_string_lossy().replace('\\', "/");
        // pathspec 应为 repo-相对路径，如 "src/main.rs"
        assert_eq!(rel, "src/main.rs", "pathspec 应为 repo-相对路径而非仅文件名");

        let mut opts = git2::DiffOptions::new();
        opts.pathspec(&rel);
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut hunk_count = 0u32;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, _hunk| { hunk_count += 1; true }),
            None,
        )
        .unwrap();
        assert!(hunk_count > 0, "正确的 pathspec 应匹配到 diff");
    }

    /// 从错误父目录传入 repo_path 时，workdir() 纠正后仍能正确 strip
    #[test]
    fn git_diff_absolute_file_path_works() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "lib.rs", "pub fn add() -> i32 { 1 }\n");
        std::fs::write(path.join("lib.rs"), "pub fn add() -> i32 { 2 }\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let head = repo.head().unwrap();
        let tree = head.peel_to_tree().unwrap();

        let workdir = repo.workdir().unwrap();
        let file_path = path.join("lib.rs");
        let rel = file_path.strip_prefix(workdir).unwrap().to_string_lossy().replace('\\', "/");
        assert_eq!(rel, "lib.rs");

        let mut opts = git2::DiffOptions::new();
        opts.pathspec(&rel);
        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .unwrap();

        let mut hunk_count = 0u32;
        diff.foreach(
            &mut |_delta, _num| true,
            None,
            Some(&mut |_delta, _hunk| { hunk_count += 1; true }),
            None,
        )
        .unwrap();
        assert!(hunk_count > 0, "绝对路径 strip 后应正确匹配");
    }

    /// 反斜杠路径 → pathspec 归一化为正斜杠
    #[test]
    fn git_diff_path_forward_slash_normalized() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.rs", "// comment\n");

        let repo = git2::Repository::open(&path).unwrap();
        let workdir = repo.workdir().unwrap();

        // 模拟 Windows 反斜杠路径
        let raw = format!("{}\\test.rs", workdir.display());
        let file_path_std = std::path::Path::new(&raw);
        let rel = file_path_std
            .strip_prefix(workdir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        // 不应含反斜杠
        assert!(!rel.contains('\\'), "pathspec 不应含反斜杠: {rel}");
        assert_eq!(rel, "test.rs");
    }

    /// 深层嵌套文件：pathspec 应为完整相对路径
    #[test]
    fn git_diff_deep_nested_file() {
        let (_dir, path) = init_temp_repo();
        let deep = path.join("src").join("components").join("ui");
        std::fs::create_dir_all(&deep).unwrap();
        let deep_file = deep.join("Button.tsx");
        std::fs::write(&deep_file, "export const Button = () => null;\n").unwrap();
        git_add(&path, "src/components/ui/Button.tsx");
        Command::new("git")
            .args(["commit", "-m", "add deep file"])
            .current_dir(&path)
            .output()
            .unwrap();
        std::fs::write(&deep_file, "export const Button = () => <div/>;\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let workdir = repo.workdir().unwrap();
        let rel = deep_file.strip_prefix(workdir).unwrap().to_string_lossy().replace('\\', "/");
        assert_eq!(
            rel,
            "src/components/ui/Button.tsx",
            "深层嵌套文件 pathspec 应为完整 repo-相对路径"
        );
    }

    // ---- dunce::simplified() 路径前缀剥离 ----

    #[test]
    fn dunce_simplified_strips_verbatim_prefix() {
        let verbatim = std::path::Path::new(r"\\?\D:\project");
        let simplified = dunce::simplified(verbatim);
        assert_eq!(
            simplified.to_string_lossy(),
            r"D:\project",
            "应剥离 \\\\?\\ 前缀"
        );
    }

    #[test]
    fn dunce_simplified_regular_path_unchanged() {
        let regular = std::path::Path::new(r"D:\data\code\slTerminal");
        let simplified = dunce::simplified(regular);
        assert_eq!(
            simplified.to_string_lossy(),
            r"D:\data\code\slTerminal",
            "普通路径不应改变"
        );
    }

    #[test]
    fn dunce_simplified_unc_path() {
        let unc = std::path::Path::new(r"\\?\UNC\server\share");
        let simplified = dunce::simplified(unc);
        // dunce 保持 \\?\UNC\ 不变（该格式本身是有效 Windows UNC 表示）
        // 本地驱动器路径才需要剥离 \\?\（如 \\?\D:\ → D:\）
        // 因此只验证 simplify 后不崩溃、不新增前缀
        let s = simplified.to_string_lossy();
        assert!(s.contains("server"), "应保留服务器名");
        assert!(s.contains("share"), "应保留共享名");
    }

    /// 模拟 workdir 含 \\?\ 前缀时，git_status entry path 仍然不含前缀
    #[test]
    fn git_status_path_after_dunce_no_verbatim_prefix() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello");
        fs::write(path.join("test.txt"), "modified").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let workdir_raw = repo.workdir().unwrap();
        let workdir = dunce::simplified(workdir_raw);

        // workdir 不应含 \\?\ 前缀
        let wd_str = workdir.to_string_lossy();
        assert!(
            !wd_str.starts_with(r"\\?\"),
            "simplified 后不应含 \\\\?\\ 前缀: {wd_str}"
        );
        assert!(
            !wd_str.contains("//?/"),
            "simplified 后不应含 //?/ : {wd_str}"
        );

        // 路径应与 fs_read_dir 格式一致：普通反斜杠转正斜杠
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();
        for entry in statuses.iter() {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            let abs = workdir.join(&rel).to_string_lossy().replace('\\', "/");
            assert!(!abs.contains("//?/"), "status path 不应含 //?/ : {abs}");
            assert!(!abs.contains(r"\\?\"), "status path 不应含 \\\\?\\ : {abs}");
        }
    }

    /// 模拟 workdir 含 \\?\ 前缀时，git_diff strip_prefix 仍然成功
    #[test]
    fn git_diff_strip_prefix_with_verbatim_workdir() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "diff.txt", "line1\nline2\n");
        fs::write(path.join("diff.txt"), "line1\nline2 MOD\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let workdir_raw = repo.workdir().unwrap();
        let workdir = dunce::simplified(workdir_raw);

        // 模拟 fs_read_dir 风格的绝对路径（无 \\?\ 前缀）
        let file_path = path.join("diff.txt");
        let rel = file_path.strip_prefix(workdir).unwrap().to_string_lossy().replace('\\', "/");
        assert_eq!(rel, "diff.txt", "strip_prefix 应成功得到相对路径");

        // 验证 pathspec 不是绝对路径
        assert!(!rel.contains(':'), "pathspec 不应是绝对路径: {rel}");
    }

    /// 防御性断言：pathspec 不可能是绝对路径（含盘符）
    #[test]
    fn git_diff_pathspec_never_absolute() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "check.txt", "data\n");
        fs::write(path.join("check.txt"), "data2\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let workdir = dunce::simplified(repo.workdir().unwrap());
        let file_path = path.join("check.txt");
        let rel = file_path.strip_prefix(workdir).unwrap_or(&file_path).to_string_lossy().replace('\\', "/");

        // 如果 rel 含盘符，说明 strip_prefix 失败，绝对是 bug
        if rel.contains(':') {
            panic!("pathspec 不应含盘符（绝对路径）: {rel} — strip_prefix 失败！");
        }
        // 正常情况：rel 为相对路径
        assert_eq!(rel, "check.txt");
    }
}
