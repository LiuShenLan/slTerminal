//! git 集成模块 — git_status + git_diff 命令
//!
//! 用 git2 0.20 + vendored-libgit2（静态链接，无需系统 git）。
//! 阻塞 I/O 用 spawn_blocking 包裹。
//!
//! 测试：GIT-12 已按命令拆分为 `src-tauri/tests/` 下独立测试文件
//! （git_status_tests / git_diff_tests / git_file_at_head_tests /
//! git_rollback_tests / git_unstage_tests + 共享 common 工厂），本文件无内嵌测试。

use crate::error::AppError;
use crate::state::{validate_path_within_root, AppState};
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
    /// 重命名前的旧绝对路径（仅 renamed 条目有值，camelCase 序列化为 oldPath）
    pub old_path: Option<String>,
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
/// pub：GIT-12 拆分后供集成测试（tests/git_status_tests.rs）直接调用。
pub fn status_to_str(status: git2::Status) -> Option<&'static str> {
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
/// pub：GIT-12 拆分后供集成测试（tests/ 下 git 测试文件）直接调用。
pub fn get_or_open_repo(
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

/// git_status 命令核心实现（GIT-01：从 #[tauri::command] 壳抽取）
///
/// State 注入以 `&AppState` 传参替代 tauri::State——命令层测试构造最小 AppState，
/// 经 block_on await 本函数，覆盖路径沙箱、spawn_blocking、错误消息契约。
/// pub：GIT-01 命令层测试（tests/git_status_tests.rs）直接调用。
pub async fn git_status_impl(
    app: &AppState,
    repo_path: &str,
) -> Result<Vec<GitStatusEntry>, AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, workdir) = {
        let root = app
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        // 路径沙箱校验
        validate_path_within_root(&root, Path::new(repo_path))?;
        // 从缓存获取/创建 Repository
        get_or_open_repo(&app.git_repo_cache, repo_path, &root)?
    };

    match tokio::task::spawn_blocking(move || {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .include_unreadable(true)
            .include_unreadable_as_untracked(true)
            .recurse_untracked_dirs(true) // FR-4: 递归列出未跟踪目录内文件
            .renames_head_to_index(true) // CV-BE-02: 启用 HEAD→index 重命名检测
            .renames_index_to_workdir(true); // CV-BE-02: 启用 index→workdir 重命名检测

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| AppError::Git(format!("获取状态失败: {e}")))?;

        // workdir 从缓存 helper 传入（已 dunce::simplified），
        // 确保与 fs_read_dir 返回的 DirEntry.path 格式一致。
        let mut entries: Vec<GitStatusEntry> = Vec::new();
        for entry in statuses.iter() {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            // 拼接为绝对路径：workdir + "/" + rel
            let path = workdir.join(&rel).to_string_lossy().replace('\\', "/");

            let status_flag = entry.status();
            let status_str = match status_to_str(status_flag) {
                Some(s) => s,
                None => continue, // 跳过 Current（无变更）
            };

            // 提取重命名条目的旧路径（绝对路径，\\→/ 规范化）
            let old_path = if status_flag.contains(git2::Status::INDEX_RENAMED) {
                entry.head_to_index().and_then(|delta| {
                    delta
                        .old_file()
                        .path()
                        .map(|p| workdir.join(p).to_string_lossy().replace('\\', "/"))
                })
            } else if status_flag.contains(git2::Status::WT_RENAMED) {
                entry.index_to_workdir().and_then(|delta| {
                    delta
                        .old_file()
                        .path()
                        .map(|p| workdir.join(p).to_string_lossy().replace('\\', "/"))
                })
            } else {
                None
            };

            entries.push(GitStatusEntry {
                path,
                status: status_str.to_string(),
                old_path,
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

/// 获取指定仓库的文件 git 状态（Tauri 命令壳）
///
/// 非 git 仓库返回 AppError::Git。实现见 [`git_status_impl`]。
#[tauri::command]
pub async fn git_status(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<GitStatusEntry>, AppError> {
    git_status_impl(&state, &repo_path).await
}

/// git_diff 命令核心实现（GIT-01：从 #[tauri::command] 壳抽取）
///
/// State 注入以 `&AppState` 传参替代 tauri::State——命令层测试构造最小 AppState，
/// 经 block_on await 本函数，覆盖路径沙箱、spawn_blocking、错误消息契约。
/// pub：GIT-01 命令层测试（tests/git_diff_tests.rs）直接调用。
pub async fn git_diff_impl(
    app: &AppState,
    repo_path: &str,
    file_path: &str,
) -> Result<Vec<DiffHunk>, AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, _workdir) = {
        let root = app
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        // 路径沙箱校验
        if !repo_path.is_empty() {
            validate_path_within_root(&root, Path::new(repo_path))?;
        }
        validate_path_within_root(&root, Path::new(file_path))?;

        // 从缓存获取/创建 Repository（以 repo_path 或 file_path 搜索）
        let search_path = if !repo_path.is_empty() {
            repo_path
        } else {
            file_path
        };
        get_or_open_repo(&app.git_repo_cache, search_path, &root)?
    };

    // spawn_blocking 闭包要求 'static：&str 参数转 owned String 后 move 捕获
    let file_path = file_path.to_string();
    match tokio::task::spawn_blocking(move || compute_diff_hunks(&repo, Path::new(&file_path)))
        .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 获取指定文件的 HEAD ↔ 工作区 diff hunks（Tauri 命令壳）
///
/// 用于编辑器行内 diff 边栏。
/// 仓库尚无提交（UnbornBranch）时返回 Err。
/// 实现见 [`git_diff_impl`]。
#[tauri::command]
pub async fn git_diff(
    repo_path: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<DiffHunk>, AppError> {
    git_diff_impl(&state, &repo_path, &file_path).await
}

/// 计算文件 HEAD ↔ 工作区的精确 diff hunks（行级合并）
///
/// 将增删行按上下文分组合并：'-'→'+' 配对为 modified hunk，
/// 纯 '+' → added hunk，纯 '-' → deleted hunk。
/// file_path 为绝对路径，函数内自动 strip workdir 前缀。
/// 仓库尚无提交（UnbornBranch）时返回空 Vec。
/// pub：GIT-12 拆分后供集成测试（tests/git_diff_tests.rs）直接调用。
pub fn compute_diff_hunks(
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
        None,                     // binary callback
        None,                     // hunk callback
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

/// git_file_at_head 命令核心实现（GIT-01：从 #[tauri::command] 壳抽取）
///
/// State 注入以 `&AppState` 传参替代 tauri::State——命令层测试构造最小 AppState，
/// 经 block_on await 本函数，覆盖路径沙箱、spawn_blocking、错误消息契约。
/// pub：GIT-01 命令层测试（tests/git_file_at_head_tests.rs）直接调用。
pub async fn git_file_at_head_impl(
    app: &AppState,
    repo_path: &str,
    file_path: &str,
) -> Result<String, AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, workdir) = {
        let root = app
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        // 路径沙箱校验
        validate_path_within_root(&root, Path::new(file_path))?;
        // 从缓存获取/创建 Repository
        let search_path = if !repo_path.is_empty() {
            repo_path
        } else {
            file_path
        };
        get_or_open_repo(&app.git_repo_cache, search_path, &root)?
    };

    // spawn_blocking 闭包要求 'static：&str 参数转 owned String 后 move 捕获
    let file_path = file_path.to_string();
    match tokio::task::spawn_blocking(move || {
        let tree = match repo.head() {
            Ok(head) => head
                .peel_to_tree()
                .map_err(|e| AppError::Git(format!("获取 HEAD tree 失败: {e}")))?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                return Err(AppError::Git("HEAD 中不存在：尚无提交记录".to_string()));
            }
            Err(e) => return Err(AppError::Git(format!("获取 HEAD 失败: {e}"))),
        };

        // workdir 由 get_or_open_repo 返回（已 dunce::simplified），防 8.3 短名
        let rel = Path::new(&file_path)
            .strip_prefix(&workdir)
            .unwrap_or(Path::new(&file_path))
            .to_string_lossy()
            .replace('\\', "/");

        let entry = tree.get_path(Path::new(&rel)).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                AppError::Git(format!("文件在 HEAD 中不存在: {rel}"))
            } else {
                AppError::Git(format!("获取 tree 条目失败: {e}"))
            }
        })?;

        let blob = entry
            .to_object(&repo)
            .map_err(|e| AppError::Git(format!("获取 object 失败: {e}")))?
            .peel_to_blob()
            .map_err(|e| AppError::Git(format!("peel to blob 失败: {e}")))?;

        Ok(String::from_utf8_lossy(blob.content()).to_string())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 获取指定文件在 HEAD commit 中的内容（Tauri 命令壳）
///
/// 仓库尚无提交（UnbornBranch）或文件不在 HEAD tree → AppError::Git，消息含"HEAD 中不存在"。
/// 实现见 [`git_file_at_head_impl`]。
#[tauri::command]
pub async fn git_file_at_head(
    repo_path: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    git_file_at_head_impl(&state, &repo_path, &file_path).await
}

/// git_rollback 命令核心实现（GIT-01：从 #[tauri::command] 壳抽取）
///
/// State 注入以 `&AppState` 传参替代 tauri::State——命令层测试构造最小 AppState，
/// 经 block_on await 本函数，覆盖路径沙箱、spawn_blocking、错误消息契约。
/// pub：GIT-01 命令层测试（tests/git_rollback_tests.rs）直接调用。
pub async fn git_rollback_impl(
    app: &AppState,
    repo_path: &str,
    file_path: &str,
) -> Result<(), AppError> {
    // 块作用域限界：RwLockReadGuard 非 Send，必须在 .await 前 drop
    let (repo, workdir) = {
        let root = app
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        validate_path_within_root(&root, Path::new(file_path))?;
        let search_path = if !repo_path.is_empty() {
            repo_path
        } else {
            file_path
        };
        get_or_open_repo(&app.git_repo_cache, search_path, &root)?
    };

    // spawn_blocking 闭包要求 'static：&str 参数转 owned String 后 move 捕获
    let file_path = file_path.to_string();
    match tokio::task::spawn_blocking(move || {
        let rel = Path::new(&file_path)
            .strip_prefix(&workdir)
            .unwrap_or(Path::new(&file_path))
            .to_string_lossy()
            .replace('\\', "/");

        // 读取 HEAD blob 内容
        // UnbornBranch 映射对齐 git_file_at_head_impl：仓库尚无提交时 head() 即失败，
        // 错误消息须含"HEAD 中不存在"（HEAD 不存在错误约定，GIT-01 错误契约测试守卫）
        let tree = match repo.head() {
            Ok(head) => head.peel_to_tree().map_err(|e| {
                if e.code() == git2::ErrorCode::UnbornBranch {
                    AppError::Git("HEAD 中不存在：尚无提交记录".to_string())
                } else {
                    AppError::Git(format!("获取 HEAD tree 失败: {e}"))
                }
            })?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                return Err(AppError::Git("HEAD 中不存在：尚无提交记录".to_string()));
            }
            Err(e) => return Err(AppError::Git(format!("获取 HEAD 失败: {e}"))),
        };

        let entry = tree.get_path(Path::new(&rel)).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                AppError::Git(format!("文件在 HEAD 中不存在: {rel}"))
            } else {
                AppError::Git(format!("获取 tree 条目失败: {e}"))
            }
        })?;
        let blob = entry
            .to_object(&repo)
            .map_err(|e| AppError::Git(format!("获取 object 失败: {e}")))?
            .peel_to_blob()
            .map_err(|e| AppError::Git(format!("peel to blob 失败: {e}")))?;

        // 步骤1：写入 HEAD blob 原始字节到工作区
        std::fs::write(&file_path, blob.content())
            .map_err(|e| AppError::Git(format!("写入文件失败: {e}")))?;

        // 步骤2：从磁盘重建 index 条目——同步 stat 信息和 blob 哈希，
        // 确保 statuses() stat 快速路径或哈希比对均判定干净。
        let mut index = repo
            .index()
            .map_err(|e| AppError::Git(format!("获取 index 失败: {e}")))?;
        index
            .add_path(Path::new(&rel))
            .map_err(|e| AppError::Git(format!("添加文件到 index 失败: {e}")))?;
        index
            .write()
            .map_err(|e| AppError::Git(format!("写入 index 失败: {e}")))?;

        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 将指定文件恢复到 HEAD 版本（Tauri 命令壳，git checkout HEAD -- &lt;file&gt;）
///
/// 读取 HEAD blob 内容并写回磁盘。对 modified/deleted 文件均有效。
/// 仓库尚无提交（UnbornBranch）或文件不在 HEAD tree → AppError::Git。
/// 实现见 [`git_rollback_impl`]。
#[tauri::command]
pub async fn git_rollback(
    repo_path: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    git_rollback_impl(&state, &repo_path, &file_path).await
}

/// git_unstage 命令核心实现（GIT-01：从 #[tauri::command] 壳抽取）
///
/// State 注入以 `&AppState` 传参替代 tauri::State——命令层测试构造最小 AppState，
/// 经 block_on await 本函数，覆盖路径沙箱、spawn_blocking、错误消息契约。
/// pub：GIT-01 命令层测试（tests/git_unstage_tests.rs）直接调用。
pub async fn git_unstage_impl(
    app: &AppState,
    repo_path: &str,
    file_path: &str,
) -> Result<(), AppError> {
    let (repo, workdir) = {
        let root = app
            .project_root
            .read()
            .map_err(|e| AppError::Git(format!("获取 project_root 锁失败: {e}")))?;
        validate_path_within_root(&root, Path::new(file_path))?;
        let search_path = if !repo_path.is_empty() {
            repo_path
        } else {
            file_path
        };
        get_or_open_repo(&app.git_repo_cache, search_path, &root)?
    };

    // spawn_blocking 闭包要求 'static：&str 参数转 owned String 后 move 捕获
    let file_path = file_path.to_string();
    match tokio::task::spawn_blocking(move || {
        let rel = Path::new(&file_path)
            .strip_prefix(&workdir)
            .unwrap_or(Path::new(&file_path))
            .to_string_lossy()
            .replace('\\', "/");

        let mut index = repo
            .index()
            .map_err(|e| AppError::Git(format!("获取 index 失败: {e}")))?;

        // remove_path 对 INDEX_NEW 有效（仅删除 index 条目，不查 HEAD）
        index
            .remove_path(Path::new(&rel))
            .map_err(|e| AppError::Git(format!("从 index 移除文件失败: {e}")))?;

        index
            .write()
            .map_err(|e| AppError::Git(format!("写入 index 失败: {e}")))?;

        Ok(())
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}

/// 将指定文件从 git index 中移除（Tauri 命令壳，git reset HEAD -- &lt;file&gt;）
///
/// 对 INDEX_NEW（staged 新文件，HEAD 中不存在）有效——仅从 index 删除条目。
/// 文件不在 index → Err。
/// 实现见 [`git_unstage_impl`]。
#[tauri::command]
pub async fn git_unstage(
    repo_path: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    git_unstage_impl(&state, &repo_path, &file_path).await
}
