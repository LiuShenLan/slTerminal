//! git 集成模块 — git_status + git_diff 命令
//!
//! 用 git2 0.20 + vendored-libgit2（静态链接，无需系统 git）。
//! 阻塞 I/O 用 spawn_blocking 包裹。

use crate::error::AppError;
use crate::state::{AppState, validate_path_within_root};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

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

/// 将 git2::Status flags 映射为前端状态字符串
///
/// 返回 None 表示无变更（Current），调用方跳过该条目。
fn status_to_str(status: git2::Status) -> Option<&'static str> {
    if status.is_conflicted() {
        Some("conflict")
    } else if status.contains(git2::Status::WT_DELETED)
        || status.contains(git2::Status::INDEX_DELETED)
    {
        Some("deleted")
    } else if status.contains(git2::Status::INDEX_RENAMED)
        || status.contains(git2::Status::WT_RENAMED)
    {
        Some("renamed")
    } else if status.contains(git2::Status::INDEX_NEW) {
        // 仅 INDEX_NEW（staged 新文件），不含 WT_NEW（untracked）
        Some("added")
    } else if status.contains(git2::Status::WT_MODIFIED)
        || status.contains(git2::Status::INDEX_MODIFIED)
    {
        Some("modified")
    } else if status.is_ignored() {
        Some("ignored")
    } else if status.contains(git2::Status::WT_NEW) {
        // 纯 WT_NEW（无 INDEX_NEW）→ untracked
        Some("untracked")
    } else {
        None // Current（无变更）→ 跳过
    }
}

/// 从缓存获取或创建 Repository（以 workdir 为 key）
///
/// git2::Repository 是 Send 但未实现 Clone trait；
/// 缓存命中时通过 `Repository::open` 重新打开以绕过生命周期耦合。
/// project_root 用于 discover 路径沙箱校验（防上溯到父仓库泄露），
/// 未设置时在测试模式下豁免。
fn get_or_open_repo(
    cache: &std::sync::Mutex<std::collections::HashMap<PathBuf, git2::Repository>>,
    search_path: &str,
    project_root: &Option<PathBuf>,
) -> Result<(git2::Repository, PathBuf), AppError> {
    let search = PathBuf::from(search_path);

    // 缓存命中检测：仅 search 在 workdir 子树内时命中（不含反向匹配，防子仓库误命中）
    {
        let cache_guard = cache
            .lock()
            .map_err(|e| AppError::Git(format!("获取 git_repo_cache 锁失败: {e}")))?;
        for workdir in cache_guard.keys() {
            if search.starts_with(workdir) {
                let wd = workdir.clone();
                drop(cache_guard);
                // 验证缓存的 workdir 仍在 project_root 内
                validate_path_within_root(project_root, &wd)?;
                let repo = git2::Repository::open(&wd)
                    .map_err(|e| AppError::Git(format!("打开仓库失败: {e}")))?;
                return Ok((repo, wd));
            }
        }
    }

    // 缓存未命中：discover + 缓存
    let repo = git2::Repository::discover(search_path)
        .map_err(|e| AppError::Git(format!("打开仓库失败: {e}")))?;
    let workdir_raw = repo
        .workdir()
        .ok_or_else(|| AppError::Git("仓库无工作目录（可能为 bare repo）".to_string()))?;
    let workdir = dunce::simplified(workdir_raw).to_path_buf();

    // 验证 discover 到的 workdir 在 project_root 内（防上溯到父仓库泄露）
    validate_path_within_root(project_root, &workdir)?;

    // 存入缓存（保留 repo 句柄标记此 workdir 可达）
    let mut cache_guard = cache
        .lock()
        .map_err(|e| AppError::Git(format!("获取 git_repo_cache 锁失败: {e}")))?;
    cache_guard.insert(workdir.clone(), repo);
    drop(cache_guard);

    // 从磁盘重新打开独立实例返回
    let repo = git2::Repository::open(&workdir)
        .map_err(|e| AppError::Git(format!("重新打开仓库失败: {e}")))?;
    Ok((repo, workdir))
}

/// 获取指定仓库的文件 git 状态
///
/// 非 git 仓库返回 AppError::Git。
#[tauri::command]
pub async fn git_status(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<GitStatusEntry>, AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, workdir) = {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        // 路径沙箱校验
        validate_path_within_root(&root, Path::new(&repo_path))?;
        // 从缓存获取/创建 Repository
        get_or_open_repo(&state.git_repo_cache, &repo_path, &root)?
    };

    match tokio::task::spawn_blocking(move || {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_unreadable(true)
            .include_unreadable_as_untracked(true);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| AppError::Git(format!("获取状态失败: {e}")))?;

        // workdir 从缓存 helper 传入（已 dunce::simplified），
        // 确保与 fs_read_dir 返回的 DirEntry.path 格式一致。
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
            let status_str = match status_to_str(status_flag) {
                Some(s) => s,
                None => continue, // 跳过 Current（无变更）
            };

            entries.push(GitStatusEntry {
                path,
                status: status_str.to_string(),
            });
        }

        Ok(entries)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 获取指定文件的 HEAD ↔ 工作区 diff hunks
///
/// 用于编辑器行内 diff 边栏。
/// 仓库尚无提交（UnbornBranch）时返回 Err。
#[tauri::command]
pub async fn git_diff(
    repo_path: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<DiffHunk>, AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, _workdir) = {
        let root = state
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        // 路径沙箱校验
        if !repo_path.is_empty() {
            validate_path_within_root(&root, Path::new(&repo_path))?;
        }
        validate_path_within_root(&root, Path::new(&file_path))?;

        // 从缓存获取/创建 Repository（以 repo_path 或 file_path 搜索）
        let search_path = if !repo_path.is_empty() { &repo_path } else { &file_path };
        get_or_open_repo(&state.git_repo_cache, search_path, &root)?
    };

    match tokio::task::spawn_blocking(move || {
        compute_diff_hunks(&repo, Path::new(&file_path))
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 计算文件 HEAD ↔ 工作区的精确 diff hunks（行级合并）
///
/// 将增删行按上下文分组合并：'-'→'+' 配对为 modified hunk，
/// 纯 '+' → added hunk，纯 '-' → deleted hunk。
/// file_path 为绝对路径，函数内自动 strip workdir 前缀。
/// 仓库尚无提交（UnbornBranch）时返回空 Vec。
pub(crate) fn compute_diff_hunks(
    repo: &git2::Repository,
    file_path: &Path,
) -> Result<Vec<DiffHunk>, AppError> {
    // 获取 HEAD tree
    let tree = match repo.head() {
        Ok(head) => Some(
            head.peel_to_tree()
                .map_err(|e| AppError::Git(format!("获取 HEAD tree 失败: {e}")))?,
        ),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(AppError::Git(format!("获取 HEAD 失败: {e}"))),
    };

    let mut opts = git2::DiffOptions::new();
    let workdir = dunce::simplified(
        repo.workdir()
            .ok_or_else(|| AppError::Git("仓库无工作目录（可能为 bare repo）".to_string()))?,
    );
    let rel = file_path
        .strip_prefix(workdir)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");
    opts.pathspec(&rel);

    let diff = repo
        .diff_tree_to_workdir_with_index(tree.as_ref(), Some(&mut opts))
        .map_err(|e| AppError::Git(format!("生成 diff 失败: {e}")))?;

    // 行级回调收集 hunks（合并连续的 '-' 和 '+' 为 modified hunk）
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut del_start: u32 = 0;
    let mut del_count: u32 = 0;
    let mut add_start: u32 = 0;
    let mut add_count: u32 = 0;

    let mut flush_pending = |ds: u32, dc: u32, as_: u32, ac: u32| {
        if dc == 0 && ac == 0 {
            return;
        }
        if dc > 0 && ac > 0 {
            // 修改：删除+新增 按相同行数配对为 ModifiedMarker
            let shared = dc.min(ac);
            hunks.push(DiffHunk {
                old_start: ds,
                old_lines: shared,
                new_start: as_,
                new_lines: shared,
            });
            // 多余的删除行
            if dc > shared {
                hunks.push(DiffHunk {
                    old_start: ds + shared,
                    old_lines: dc - shared,
                    new_start: ds + shared,
                    new_lines: 0,
                });
            }
            // 多余的新增行
            if ac > shared {
                hunks.push(DiffHunk {
                    old_start: 0,
                    old_lines: 0,
                    new_start: as_ + shared,
                    new_lines: ac - shared,
                });
            }
        } else if dc > 0 {
            // 纯删除
            hunks.push(DiffHunk {
                old_start: ds,
                old_lines: dc,
                new_start: ds,
                new_lines: 0,
            });
        } else {
            // 纯新增
            hunks.push(DiffHunk {
                old_start: 0,
                old_lines: 0,
                new_start: as_,
                new_lines: ac,
            });
        }
    };

    let mut prev_was_del = false; // 上一行是否为 '-'，用于检测 '-→+' 修改模式

    diff.foreach(
        &mut |_delta, _num| true, // file callback
        None,                      // binary callback
        None,                      // hunk callback
        Some(&mut |_delta, _hunk, line| {
            let c = line.origin();
            if c == '+' {
                let n = line.new_lineno().unwrap_or(0);
                if add_count == 0 {
                    add_start = n;
                }
                add_count += 1;
                prev_was_del = false;
            } else if c == '-' {
                let o = line.old_lineno().unwrap_or(0);
                if prev_was_del {
                    // 遇到新的 '-' 组：之前有多余的新增行，无配对删除 → 先 flush
                    if add_count > 0 {
                        flush_pending(0, 0, add_start, add_count);
                        add_start = 0;
                        add_count = 0;
                    }
                }
                if del_count == 0 {
                    del_start = o;
                }
                del_count += 1;
                prev_was_del = true;
            } else {
                // context 行 → flush 当前累积的变更组
                flush_pending(del_start, del_count, add_start, add_count);
                del_start = 0;
                del_count = 0;
                add_start = 0;
                add_count = 0;
                prev_was_del = false;
            }
            true
        }),
    )
    .map_err(|e| AppError::Git(format!("diff foreach 失败: {e}")))?;

    // flush 末尾残留组
    flush_pending(del_start, del_count, add_start, add_count);

    Ok(hunks)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use super::{DiffHunk, GitStatusEntry, compute_diff_hunks};

    /// 在临时目录中 init 一个 git 仓库，返回 tempdir（自动清理）和路径
    fn init_temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        // CI runner 的 %TEMP% 含 8.3 短名（如 RUNNER~1），而 git2 workdir 返回长名，
        // 两者 strip_prefix/路径断言会不匹配（dunce::simplified 只剥 verbatim 前缀、
        // 不解析短名→长名）。canonicalize 统一为长名，从源头消除短/长名差异。
        let path = dunce::canonicalize(dir.path()).unwrap();

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

    // ---- B1: status_to_str 纯函数映射测试 ----

    #[test]
    fn test_status_to_str_all_flags() {
        let cases = vec![
            (git2::Status::WT_NEW, Some("untracked")),
            (git2::Status::INDEX_NEW, Some("added")),
            (git2::Status::INDEX_NEW | git2::Status::WT_NEW, Some("added")),
            (git2::Status::INDEX_NEW | git2::Status::WT_MODIFIED, Some("added")),
            (git2::Status::WT_MODIFIED, Some("modified")),
            (git2::Status::INDEX_MODIFIED, Some("modified")),
            (git2::Status::WT_DELETED, Some("deleted")),
            (git2::Status::INDEX_DELETED, Some("deleted")),
            (git2::Status::INDEX_RENAMED, Some("renamed")),
            (git2::Status::WT_RENAMED, Some("renamed")),
            (git2::Status::IGNORED, Some("ignored")),
            (git2::Status::CURRENT, None),
        ];
        for (flags, expected) in cases {
            assert_eq!(super::status_to_str(flags), expected);
        }
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

    // ---- P0: include_ignored(false) 行为验证 ----

    #[test]
    fn git_status_excludes_ignored_files() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "main.rs", "fn main() {}");
        // 添加 .gitignore 忽略 *.log
        fs::write(path.join(".gitignore"), "*.log\n").unwrap();
        fs::write(path.join("test.log"), "ignored content").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        // 不设置 include_ignored → 被忽略文件不出现
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        // test.log 不应出现在 status 中
        let has_ignored = statuses.iter().any(|e| {
            let rel = e.path().unwrap_or("");
            rel.contains("test.log")
        });
        assert!(!has_ignored, ".gitignore 忽略的文件不应出现在状态输出中");

        // main.rs 无变更 → CURRENT → 被 skip；仅 .gitignore 本身作为 untracked 出现
        let status_paths: Vec<String> = statuses.iter()
            .filter_map(|e| e.path().map(|s| s.to_string()))
            .collect();
        assert!(!status_paths.iter().any(|p| p.contains("test.log")), "test.log 不应在 status 中");
        assert!(status_paths.iter().any(|p| p == ".gitignore"), ".gitignore 本身应作为 untracked 出现");
    }

    #[test]
    fn git_status_includes_untracked_not_ignored() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "main.rs", "fn main() {}");
        // untracked.txt 不匹配 .gitignore
        fs::write(path.join(".gitignore"), "*.log\n").unwrap();
        fs::write(path.join("untracked.txt"), "new file").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_untracked = statuses.iter().any(|e| {
            let rel = e.path().unwrap_or("");
            rel == "untracked.txt" && e.status().contains(git2::Status::WT_NEW)
        });
        assert!(has_untracked, "未被忽略的未跟踪文件仍应出现");
    }

    #[test]
    fn git_status_includes_modified_tracked() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "main.rs", "original");
        fs::write(path.join(".gitignore"), "*.log\n").unwrap();
        // 修改已跟踪文件
        fs::write(path.join("main.rs"), "modified").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let has_modified = statuses.iter().any(|e| {
            let rel = e.path().unwrap_or("");
            rel == "main.rs" && e.status().contains(git2::Status::WT_MODIFIED)
        });
        assert!(has_modified, "修改的已跟踪文件应正常显示状态");
    }

    #[test]
    fn git_status_tracked_then_ignored_still_shows_status() {
        let (_dir, path) = init_temp_repo();
        // 先提交文件
        commit_file(&path, "config.toml", "version = 1");
        // 然后加入 .gitignore
        fs::write(path.join(".gitignore"), "*.toml\n").unwrap();
        // 修改已跟踪文件
        fs::write(path.join("config.toml"), "version = 2").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).unwrap();

        let config_status = statuses.iter().find(|e| {
            e.path().unwrap_or("") == "config.toml"
        });
        assert!(config_status.is_some(), "已跟踪后被忽略的文件仍应出现在状态中");
        assert!(
            config_status.unwrap().status().contains(git2::Status::WT_MODIFIED),
            "已跟踪文件的修改状态应正常显示"
        );
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
        let workdir = dunce::simplified(repo.workdir().unwrap());
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

        let workdir = dunce::simplified(repo.workdir().unwrap());
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
        let workdir = dunce::simplified(repo.workdir().unwrap());
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

    /// 修改一行 → compute_diff_hunks 将删除+新增合并为 modified hunk
    #[test]
    fn git_diff_precise_single_line_modification() {
        let (_dir, path) = init_temp_repo();
        // 5 行文件，修改第 3 行
        commit_file(&path, "f.txt", "line1\nline2\nline3\nline4\nline5\n");
        fs::write(path.join("f.txt"), "line1\nline2\nline3 MODIFIED\nline4\nline5\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 修改 = 删除+新增合并为 1 个 modified hunk（生产算法：'-'→'+' → prev_was_del 配对）
        assert_eq!(hunks.len(), 1, "修改一行应合并为 1 个 modified hunk");
        assert_eq!(hunks[0].old_start, 3);
        assert_eq!(hunks[0].old_lines, 1);
        assert_eq!(hunks[0].new_start, 3);
        assert_eq!(hunks[0].new_lines, 1);
    }

    /// 连续新增多行 → 合并为 1 个 hunk
    #[test]
    fn git_diff_precise_consecutive_additions_merged() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\n");
        fs::write(path.join("f.txt"), "line1\nline2\nline3\nline4\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        assert_eq!(hunks.len(), 1, "连续新增应合并为 1 个 hunk");
        assert_eq!(hunks[0].old_lines, 0, "纯新增 old_lines=0");
        assert_eq!(hunks[0].new_lines, 3, "新增 3 行");
        assert_eq!(hunks[0].new_start, 2);
    }

    /// 连续删除多行 → 合并为 1 个 hunk
    #[test]
    fn git_diff_precise_consecutive_deletions_merged() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "a\nb\nc\nd\n");
        fs::write(path.join("f.txt"), "a\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        assert_eq!(hunks.len(), 1, "连续删除应合并为 1 个 hunk");
        assert_eq!(hunks[0].old_lines, 3, "删除 3 行");
        assert_eq!(hunks[0].new_lines, 0, "纯删除 new_lines=0");
    }

    /// 多处修改由 context 分隔 → 各自独立 modified hunk
    #[test]
    fn git_diff_precise_multiple_groups_separated_by_context() {
        let (_dir, path) = init_temp_repo();
        // 7 行，修改第 2 行和第 6 行（中间 3 行 context 分隔）
        commit_file(&path, "f.txt", "a\nb\nc\nd\ne\nf\ng\n");
        fs::write(path.join("f.txt"), "a\nB\nc\nd\ne\nF\ng\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 两处独立修改，各合并为 modified hunk
        let modified_count = hunks.iter().filter(|h| h.old_lines > 0 && h.new_lines > 0).count();
        assert_eq!(modified_count, 2, "应有 2 个独立的 modified hunk");
        let total_changed: u32 = hunks.iter().map(|h| h.new_lines + h.old_lines).sum();
        assert!(total_changed <= 4, "总变更行数不应超过 4（修改 2 行=4），实际: {total_changed}");
    }

    /// 无修改文件 → 0 hunk
    #[test]
    fn git_diff_precise_no_change_returns_empty() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "unchanged\n");

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();
        assert_eq!(hunks.len(), 0, "无修改应返回 0 hunk");
    }

    // ---- Repository::discover 子目录上溯 ----

    /// 从子目录调用 git discover 也能找到仓库
    #[test]
    fn git_discover_from_subdirectory() {
        let (_dir, path) = init_temp_repo();
        // 先创建子目录和文件
        let sub_dir = path.join("sub").join("deep");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("file.txt"), "content\n").unwrap();
        git_add(&path, "sub/deep/file.txt");
        Command::new("git")
            .args(["commit", "-m", "add deep file"])
            .current_dir(&path)
            .output()
            .unwrap();

        // 从子目录 discover
        let repo = git2::Repository::discover(&sub_dir)
            .expect("从子目录 discover 应能找到仓库");
        assert!(repo.workdir().is_some(), "应能找到 workdir");

        // 验证路径拼接正确
        let workdir = dunce::simplified(repo.workdir().unwrap());
        let abs = workdir.join("sub/deep/file.txt").to_string_lossy().replace('\\', "/");
        let expected = path.join("sub/deep/file.txt").to_string_lossy().replace('\\', "/");
        assert_eq!(abs, expected, "workdir 拼接路径应与实际路径一致");
    }

    // ---- serde camelCase 序列化 ----

    #[test]
    fn diff_hunk_serializes_camelcase() {
        let hunk = DiffHunk { old_start: 10, old_lines: 2, new_start: 12, new_lines: 3 };
        let json = serde_json::to_string(&hunk).unwrap();
        assert!(json.contains("\"oldStart\""), "应包含 camelCase 字段 oldStart: {json}");
        assert!(json.contains("\"oldLines\""), "应包含 camelCase 字段 oldLines: {json}");
        assert!(json.contains("\"newStart\""), "应包含 camelCase 字段 newStart: {json}");
        assert!(json.contains("\"newLines\""), "应包含 camelCase 字段 newLines: {json}");
    }

    #[test]
    fn git_status_entry_serializes_camelcase() {
        let entry = GitStatusEntry { path: "/abs/path".into(), status: "modified".into() };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"path\""), "应包含 path: {json}");
        assert!(json.contains("\"status\""), "应包含 status: {json}");
        assert!(!json.contains("\"Path\""), "不应包含 PascalCase");
    }

    // ---- 行级 diff 精确性测试（line callback 逻辑） ----
    // 共用 compute_diff_hunks 生产函数（见上文 pub(crate) fn），测试直调无需副本。

    #[test]
    fn line_callback_single_modified_line() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\nline2\nline3\n");
        // 修改 line2
        fs::write(path.join("f.txt"), "line1\nline2 MODIFIED\nline3\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        assert_eq!(hunks.len(), 1, "单行修改应只有 1 个 hunk");
        assert_eq!(hunks[0].old_lines, 1, "old_lines=1");
        assert_eq!(hunks[0].new_lines, 1, "new_lines=1 → ModifiedMarker");
    }

    #[test]
    fn line_callback_context_lines_not_included() {
        let (_dir, path) = init_temp_repo();
        // 创建一个有 10 行的文件，只修改中间 1 行
        let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
        commit_file(&path, "f.txt", &content);
        // 修改第 5 行
        let new_content: String = (1..=10).map(|i| {
            if i == 5 { format!("line{i} MODIFIED\n") } else { format!("line{i}\n") }
        }).collect();
        fs::write(path.join("f.txt"), &new_content).unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 只修改了 1 行，应只有 1 个 hunk，old_lines=1（不包含 context）
        assert_eq!(hunks.len(), 1, "单行修改只应有 1 个 hunk，不含 context");
        assert_eq!(hunks[0].old_lines, 1, "不应包含 context 行");
        assert_eq!(hunks[0].new_lines, 1, "不应包含 context 行");
    }

    #[test]
    fn line_callback_pure_addition() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\nline2\n");
        // 在 line1 后插入 3 行
        fs::write(path.join("f.txt"), "line1\nnewA\nnewB\nnewC\nline2\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        let added = hunks.iter().find(|h| h.old_lines == 0);
        assert!(added.is_some(), "应有纯新增 hunk（old_lines=0）");
        assert_eq!(added.unwrap().new_lines, 3, "new_lines=3（3 行新增）");
    }

    #[test]
    fn line_callback_pure_deletion() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\nline2\nline3\nline4\n");
        // 删除 line2, line3
        fs::write(path.join("f.txt"), "line1\nline4\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        let deleted = hunks.iter().find(|h| h.new_lines == 0);
        assert!(deleted.is_some(), "应有纯删除 hunk（new_lines=0）");
        assert_eq!(deleted.unwrap().old_lines, 2, "old_lines=2（2 行删除）");
    }

    #[test]
    fn line_callback_modified_plus_extra_additions() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\nline2\nline3\n");
        fs::write(path.join("f.txt"), "line1\nline2 NEW\nline3\nline4 NEW\nline5 NEW\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 应包含 modified（蓝色）hunk
        let modified = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines > 0);
        assert!(modified.is_some(), "应有 modified hunk");
        assert_eq!(modified.unwrap().old_lines, modified.unwrap().new_lines,
            "modified 的 old/new 行数应相等");

        // 应包含新增（绿色）hunk（old_lines=0）
        let added = hunks.iter().find(|h| h.old_lines == 0 && h.new_lines > 0);
        assert!(added.is_some(), "应有额外新增 hunk");
        assert!(added.unwrap().new_lines > 0, "new_lines > 0");

        // 所有 hunk 的变更行数之和应覆盖全部改动
        let total_del: u32 = hunks.iter().map(|h| h.old_lines).sum();
        let total_add: u32 = hunks.iter().map(|h| h.new_lines).sum();
        assert!(total_del > 0, "应有删除行");
        assert!(total_add > total_del, "新增行应多于删除行");
    }

    #[test]
    fn line_callback_modified_plus_extra_deletions() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "line1\nold2\nold3\nold4\nold5\n");
        fs::write(path.join("f.txt"), "line1\nnew2\nnew3\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 应包含 modified（蓝色）hunk
        let modified = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines > 0);
        assert!(modified.is_some(), "应有 modified hunk");
        assert_eq!(modified.unwrap().old_lines, modified.unwrap().new_lines,
            "modified 的 old/new 行数应相等");

        // 应包含删除（灰三角）hunk（new_lines=0）
        let deleted = hunks.iter().find(|h| h.old_lines > 0 && h.new_lines == 0);
        assert!(deleted.is_some(), "应有多余删除 hunk");
        assert!(deleted.unwrap().old_lines > 0, "old_lines > 0");

        // 删除行应多于修改行
        let total_del: u32 = hunks.iter().map(|h| h.old_lines).sum();
        let total_add: u32 = hunks.iter().map(|h| h.new_lines).sum();
        assert!(total_del > total_add, "删除行应多于新增行");
    }

    #[test]
    fn line_callback_multiple_change_groups() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "A1\nA2\nA3\nB1\nB2\nB3\n");
        // 修改 A2 和 B2（中间有 context 行 A3/B1）
        fs::write(path.join("f.txt"), "A1\nA2 MOD\nA3\nB1\nB2 MOD\nB3\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 两处独立的修改
        let modified = hunks.iter().filter(|h| h.old_lines > 0 && h.new_lines > 0).count();
        assert_eq!(modified, 2, "应有 2 个独立的 modified hunk");
    }

    #[test]
    fn line_callback_no_changes_returns_empty() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "unchanged\n");
        // 不修改

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        assert_eq!(hunks.len(), 0, "无修改应返回 0 hunk");
    }

    #[test]
    fn line_callback_delete_all_lines() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "f.txt", "a\nb\nc\n");
        // 全部删除
        fs::write(path.join("f.txt"), "").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 全删应只有删除类 hunk（new_lines=0），无 modified 或 added
        let deleted_hunks: Vec<_> = hunks.iter().filter(|h| h.new_lines == 0).collect();
        assert!(!deleted_hunks.is_empty(), "全删应有删除 hunk");
        let total_deleted: u32 = deleted_hunks.iter().map(|h| h.old_lines).sum();
        assert_eq!(total_deleted, 3, "总计 3 行删除");
        // 不应有 modified 或 added
        assert!(hunks.iter().all(|h| h.new_lines == 0), "全删不应有新增行");
    }

    #[test]
    fn line_callback_add_all_new_lines_after_commit() {
        let (_dir, path) = init_temp_repo();
        // 先 commit 一个空文件，再追加 3 行 → 纯新增
        commit_file(&path, "f.txt", "original\n");
        fs::write(path.join("f.txt"), "original\nnewA\nnewB\nnewC\n").unwrap();

        let repo = git2::Repository::open(&path).unwrap();
        let hunks = compute_diff_hunks(&repo, &path.join("f.txt")).unwrap();

        // 应有纯新增 hunk
        let added = hunks.iter().find(|h| h.old_lines == 0 && h.new_lines > 0);
        assert!(added.is_some(), "追加行应有 added hunk");
        assert_eq!(added.unwrap().new_lines, 3, "3 行纯新增");
    }

    // ---- Repository::discover 验证 ----

    #[test]
    fn repository_discover_from_subdirectory() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello\n");
        // 在子目录中 discover
        let sub = path.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        let result = git2::Repository::discover(&sub);
        assert!(result.is_ok(), "discover 应从子目录找到仓库");
    }

    #[test]
    fn repository_discover_from_deep_subdirectory() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello\n");
        let deep = path.join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();

        let result = git2::Repository::discover(&deep);
        assert!(result.is_ok(), "discover 应从深层子目录找到仓库");
    }

    // ---- M14: get_or_open_repo 缓存与边界测试 ----

    #[test]
    fn get_or_open_repo_cache_miss() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello");

        let cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let result = super::get_or_open_repo(&cache, &path.to_string_lossy(), &None);
        assert!(result.is_ok(), "首次访问应成功（cache miss → discover → 缓存）");
        let (_repo, workdir) = result.unwrap();
        assert_eq!(workdir, dunce::simplified(path.as_path()));
    }

    // ---- CI 门禁回归守卫（8.3 短名根因 + L1 串行化） ----

    /// T1: init_temp_repo 返回规范化（长名/非 verbatim）路径，与 git2 workdir strip_prefix 成功。
    /// 根因守卫——runner 上若回退为 dir.path() 短名，(a) 立即失败。本地无短名则 (a) 平凡通过。
    #[test]
    fn init_temp_repo_path_canonicalized_and_strips() {
        let (_dir, path) = init_temp_repo();
        // (a) 幂等：已是 canonical 形式
        assert_eq!(path, dunce::canonicalize(&path).unwrap());
        // (b) 无 verbatim 前缀
        assert!(!path.to_string_lossy().contains(r"\\?\"));
        // (c) 与 git2 workdir strip_prefix 成功（8.3 短名根因守卫）
        commit_file(&path, "x.txt", "a\n");
        let repo = git2::Repository::open(&path).unwrap();
        let workdir = dunce::simplified(repo.workdir().unwrap());
        let rel = path
            .join("x.txt")
            .strip_prefix(workdir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert_eq!(rel, "x.txt");
    }

    /// T2: get_or_open_repo 返回的 workdir 与规范化 path 一致（强化 cache_miss 语义）。
    #[test]
    fn get_or_open_repo_workdir_equals_canonical_path() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "t.txt", "x");
        let cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let (_repo, workdir) =
            super::get_or_open_repo(&cache, &path.to_string_lossy(), &None).unwrap();
        assert_eq!(
            workdir,
            dunce::simplified(dunce::canonicalize(&path).unwrap().as_path()),
        );
    }

    /// T3: CI L1 step 必须 --test-threads=1（ConPTY 并发 spawn 死锁防护）。配置不变量守卫。
    #[test]
    fn ci_l1_uses_single_test_thread() {
        let ci = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../.github/workflows/ci.yml"
        ))
        .unwrap();
        assert!(
            ci.contains("--test-threads=1"),
            "CI L1 step 必须 --test-threads=1（ConPTY 并发 spawn 死锁防护）"
        );
    }

    #[test]
    fn get_or_open_repo_cache_hit() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "test.txt", "hello");

        let cache = std::sync::Mutex::new(std::collections::HashMap::new());
        // 首次访问 → 缓存
        let result1 = super::get_or_open_repo(&cache, &path.to_string_lossy(), &None);
        assert!(result1.is_ok(), "首次访问应成功");

        // 从子目录访问 → 缓存命中（子目录在 workdir 子树内）
        let sub = path.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let result2 = super::get_or_open_repo(&cache, &sub.to_string_lossy(), &None);
        assert!(result2.is_ok(), "子目录访问应缓存命中");
    }

    // BE-06: 缓存的子仓库不应被父目录误命中
    #[test]
    fn get_or_open_repo_cache_no_false_hit_for_subrepo() {
        let (_dir, path) = init_temp_repo();
        commit_file(&path, "root.txt", "root content");

        // 在子目录创建嵌套 git 仓库
        let sub = path.join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&sub)
            .output()
            .unwrap();
        // 在子仓库中 commit 一个文件
        let sub_file = sub.join("nested.txt");
        std::fs::write(&sub_file, "nested content").unwrap();
        Command::new("git")
            .args(["add", "nested.txt"])
            .current_dir(&sub)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "nested"])
            .current_dir(&sub)
            .output()
            .unwrap();

        let cache = std::sync::Mutex::new(std::collections::HashMap::new());

        // 先访问子目录 → 缓存子仓库 workdir
        let result_sub = super::get_or_open_repo(&cache, &sub.to_string_lossy(), &None);
        assert!(result_sub.is_ok(), "子仓库访问应成功");
        let (_sub_repo, sub_workdir) = result_sub.unwrap();
        assert_eq!(sub_workdir, dunce::simplified(sub.as_path()));

        // 再访问父目录 → 不应命中子仓库缓存（父目录不在子仓库子树内）
        let result_parent = super::get_or_open_repo(&cache, &path.to_string_lossy(), &None);
        assert!(result_parent.is_ok(), "父目录访问应成功");
        let (_parent_repo, parent_workdir) = result_parent.unwrap();
        assert_eq!(
            parent_workdir,
            dunce::simplified(path.as_path()),
            "父目录不应命中子仓库缓存，应 discover 到父仓库"
        );

        // 缓存中应有父子两个仓库各自的工作目录
        let cache_guard = cache.lock().unwrap();
        assert_eq!(cache_guard.len(), 2, "缓存中应有父子两个仓库");
    }

    #[test]
    fn get_or_open_repo_discover_failure() {
        // 非 git 目录 → discover 失败
        let tmp = tempfile::tempdir().unwrap();
        let non_repo = tmp.path().join("not_a_repo");
        std::fs::create_dir_all(&non_repo).unwrap();

        let cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let result = super::get_or_open_repo(&cache, &non_repo.to_string_lossy(), &None);
        assert!(result.is_err(), "非 git 目录 discover 应失败");
    }

    #[test]
    fn get_or_open_repo_bare_repo_returns_err() {
        // bare repo 无工作目录 → workdir() 返回 None → Err
        let tmp = tempfile::tempdir().unwrap();
        let bare_path = tmp.path().join("bare.git");
        git2::Repository::init_bare(&bare_path).unwrap();

        let cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let result = super::get_or_open_repo(&cache, &bare_path.to_string_lossy(), &None);
        assert!(result.is_err(), "bare repo 无 workdir 应返回错误");
    }
}
