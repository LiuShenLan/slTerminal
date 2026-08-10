//! git_status 命令域测试（GIT-12 拆分）
//!
//! 原 `git/mod.rs` 的 `#[cfg(test)] mod tests` 按命令拆分为独立集成测试文件。
//! 本文件覆盖：status_to_str 映射、git_status 状态行为、include_ignored 行为、
//! 绝对路径格式、递归未跟踪目录、renamed oldPath，以及 git2 底层原语
//! （dunce / discover / get_or_open_repo 缓存——跨命令共享，归入主命令域）。
//!
//! GIT-01：命令层测试段（`git_status_impl` 直调——最小 AppState + block_on await
//! 真实命令实现，覆盖 State 注入、路径沙箱、spawn_blocking、错误消息契约）；
//! 其余直接调用 git2 API 的测试为**底层原语**行为验证（GIT-01 保留，非命令层）。
//!
//! 依赖系统 git CLI（`commit_file`/`git_add` 调用 git add/commit，见 common 工厂）；
//! 最低版本 2.28——`init_temp_repo` 的 `git -c init.defaultBranch=main init`
//! （`init.defaultBranch` 配置 2.28 引入，早期版本会静默忽略该键）。版本声明由
//! Stage 17 DOC-04 收编进 `src-tauri/src/git/CLAUDE.md`（GIT-08③）。

mod common;

use std::fs;
use std::process::Command;

use common::{block_on, commit_file, git_add, init_temp_repo, make_app_state};
use slterminal_lib::git::{get_or_open_repo, git_status_impl, status_to_str, GitStatusEntry};
use slterminal_lib::AppError;

// ---- B1: status_to_str 纯函数映射测试 ----

#[test]
fn test_status_to_str_all_flags() {
    let cases = vec![
        (git2::Status::WT_NEW, Some("untracked")),
        (git2::Status::INDEX_NEW, Some("added")),
        (
            git2::Status::INDEX_NEW | git2::Status::WT_NEW,
            Some("added"),
        ),
        (
            git2::Status::INDEX_NEW | git2::Status::WT_MODIFIED,
            Some("added"),
        ),
        (git2::Status::WT_MODIFIED, Some("modified")),
        (git2::Status::INDEX_MODIFIED, Some("modified")),
        (git2::Status::WT_DELETED, Some("deleted")),
        (git2::Status::INDEX_DELETED, Some("deleted")),
        (git2::Status::INDEX_RENAMED, Some("renamed")),
        (git2::Status::WT_RENAMED, Some("renamed")),
        (git2::Status::IGNORED, Some("ignored")),
        // GIT-04：conflict 分支（status.is_conflicted() → "conflict"）
        (git2::Status::CONFLICTED, Some("conflict")),
        (git2::Status::CURRENT, None),
    ];
    for (flags, expected) in cases {
        assert_eq!(status_to_str(flags), expected);
    }
}

/// 底层原语：git2::Repository::open 对非 git 目录返回 Err
/// （命令层错误契约见下方 GIT-01 命令层测试段）
#[test]
fn git_open_nonexistent_returns_err() {
    let tmp = tempfile::tempdir().unwrap();
    let result = git2::Repository::open(tmp.path());
    assert!(result.is_err(), "非 git 目录应返回错误");
}

/// 底层原语：空仓库 statuses 为空
#[test]
fn git_status_empty_repo_no_files() {
    let (_dir, path) = init_temp_repo();
    let repo = git2::Repository::open(&path).unwrap();
    let statuses = repo.statuses(None).unwrap();
    assert!(statuses.is_empty(), "空仓库无文件应返回空状态");
}

// ---- B2: git_status 状态行为（git2 底层原语：直接 repo.statuses，不经过命令） ----

/// 收集 (相对路径, 状态串) 列表（状态串经生产 status_to_str 映射）——精确断言辅助，
/// 路径集合 + 状态串 + 条目数三者同时锁定（GIT-07 精确化，D7 payload 键集合同款思路）
fn collect_statuses(statuses: &git2::Statuses) -> Vec<(String, String)> {
    statuses
        .iter()
        .map(|e| {
            (
                e.path().unwrap_or("").to_string(),
                status_to_str(e.status()).unwrap_or("").to_string(),
            )
        })
        .collect()
}

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

    assert_eq!(
        collect_statuses(&statuses),
        vec![("test.txt".to_string(), "modified".to_string())],
        "修改文件应精确返回 1 条 modified 条目"
    );
}

#[test]
fn git_status_untracked_file() {
    let (_dir, path) = init_temp_repo();
    fs::write(path.join("new_file.txt"), "untracked").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    assert_eq!(
        collect_statuses(&statuses),
        vec![("new_file.txt".to_string(), "untracked".to_string())],
        "未 add 的新文件应精确返回 1 条 untracked 条目"
    );
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

    assert_eq!(
        collect_statuses(&statuses),
        vec![("staged.txt".to_string(), "added".to_string())],
        "git add 后的文件应精确返回 1 条 added 条目"
    );
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

    assert_eq!(
        collect_statuses(&statuses),
        vec![("to_delete.txt".to_string(), "deleted".to_string())],
        "删除已提交文件应精确返回 1 条 deleted 条目"
    );
}

#[test]
fn git_status_non_repo_returns_err() {
    let tmp = tempfile::tempdir().unwrap();
    let result = git2::Repository::open(tmp.path());
    assert!(result.is_err(), "非 git 目录应返回错误");
}

// ---- P0: include_ignored(false) 行为验证（git2 底层原语） ----
// GIT-08②：ignore 规则一律用 git2 内存规则 `repo.add_ignore_rule`（git_ignore_add_rule）
// 注入，不再写磁盘 .gitignore——消除磁盘写入时序（CI 偶发：.gitignore 写入与 status
// 扫描的时序竞态）。

#[test]
fn git_status_excludes_ignored_files() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "main.rs", "fn main() {}");
    let repo = git2::Repository::open(&path).unwrap();
    // 内存 ignore 规则 *.log（替代磁盘 .gitignore 写入）
    repo.add_ignore_rule("*.log").unwrap();
    fs::write(path.join("test.log"), "ignored content").unwrap();

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    // 不设置 include_ignored → 被忽略文件不出现
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    // main.rs 无变更（CURRENT 跳过）+ test.log 被忽略 → 精确空列表（GIT-07：数量锁定）
    assert_eq!(
        statuses.iter().count(),
        0,
        "仅含被忽略文件时应返回空状态列表（0 条目）"
    );
}

#[test]
fn git_status_includes_untracked_not_ignored() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "main.rs", "fn main() {}");
    let repo = git2::Repository::open(&path).unwrap();
    // 内存 ignore 规则 *.log；untracked.txt 不匹配
    repo.add_ignore_rule("*.log").unwrap();
    fs::write(path.join("untracked.txt"), "new file").unwrap();

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    assert_eq!(
        collect_statuses(&statuses),
        vec![("untracked.txt".to_string(), "untracked".to_string())],
        "未被忽略的未跟踪文件应精确返回 1 条 untracked 条目"
    );
}

#[test]
fn git_status_includes_modified_tracked() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "main.rs", "original");
    let repo = git2::Repository::open(&path).unwrap();
    // 内存 ignore 规则 *.log
    repo.add_ignore_rule("*.log").unwrap();
    // 修改已跟踪文件
    fs::write(path.join("main.rs"), "modified").unwrap();

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    assert_eq!(
        collect_statuses(&statuses),
        vec![("main.rs".to_string(), "modified".to_string())],
        "修改的已跟踪文件应精确返回 1 条 modified 条目"
    );
}

#[test]
fn git_status_tracked_then_ignored_still_shows_status() {
    let (_dir, path) = init_temp_repo();
    // 先提交文件
    commit_file(&path, "config.toml", "version = 1");
    let repo = git2::Repository::open(&path).unwrap();
    // 内存 ignore 规则 *.toml——已跟踪文件不受 ignore 规则影响
    repo.add_ignore_rule("*.toml").unwrap();
    // 修改已跟踪文件
    fs::write(path.join("config.toml"), "version = 2").unwrap();

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    assert_eq!(
        collect_statuses(&statuses),
        vec![("config.toml".to_string(), "modified".to_string())],
        "已跟踪后被忽略的文件仍应精确返回 1 条 modified 条目"
    );
}

// ---- P1: 绝对路径拼接验证（git2 底层原语：模拟生产拼接逻辑，不经命令） ----

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
        assert!(abs.ends_with("test.txt"), "绝对路径应以文件名结尾: {abs}");
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
            assert_eq!(
                abs,
                format!("{repo_path_str}/test.txt"),
                "modified 文件的绝对路径应为 repo/test.txt"
            );
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
        if entry.status().contains(git2::Status::WT_NEW)
            && !entry.status().contains(git2::Status::INDEX_NEW)
        {
            let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
            let abs = status_entry_to_absolute(&path, &rel);
            assert_eq!(
                abs,
                format!("{repo_path_str}/new_file.txt"),
                "untracked 文件的绝对路径应为 repo/new_file.txt"
            );
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
            assert_eq!(
                abs,
                format!("{repo_path_str}/staged.txt"),
                "added 文件的绝对路径应为 repo/staged.txt"
            );
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
            assert_eq!(
                abs,
                format!("{repo_path_str}/to_delete.txt"),
                "deleted 文件的绝对路径应为 repo/to_delete.txt"
            );
            found = true;
        }
    }
    assert!(found, "应找到 deleted 文件");
}

// ---- CV-BE-01: recurse_untracked_dirs 递归列出未跟踪目录内文件（git2 底层原语） ----

#[test]
fn git_status_recurse_untracked_dirs() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "tracked.txt", "tracked");

    // 创建含多文件的未跟踪目录
    let sub = path.join("newdir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.txt"), "a").unwrap();
    fs::write(sub.join("b.txt"), "b").unwrap();
    // 嵌套子目录
    fs::create_dir(sub.join("nested")).unwrap();
    fs::write(sub.join("nested").join("c.txt"), "c").unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    // 收集所有 untracked 路径
    let untracked_paths: Vec<String> = statuses
        .iter()
        .filter(|e| {
            let s = e.status();
            s.contains(git2::Status::WT_NEW) && !s.contains(git2::Status::INDEX_NEW)
        })
        .map(|e| e.path().unwrap_or("").to_string().replace('\\', "/"))
        .collect();

    // 每个文件单独出现，不是目录单条目 "newdir/"
    assert!(
        untracked_paths.contains(&"newdir/a.txt".to_string()),
        "应包含 newdir/a.txt，实际: {untracked_paths:?}"
    );
    assert!(
        untracked_paths.contains(&"newdir/b.txt".to_string()),
        "应包含 newdir/b.txt，实际: {untracked_paths:?}"
    );
    assert!(
        untracked_paths.contains(&"newdir/nested/c.txt".to_string()),
        "应包含 newdir/nested/c.txt，实际: {untracked_paths:?}"
    );
    // 不应出现目录条目
    assert!(
        !untracked_paths.contains(&"newdir/".to_string()),
        "递归模式下不应出现目录单条目 newdir/，实际: {untracked_paths:?}"
    );
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
    let fs_read_dir_path = path
        .join("compare.txt")
        .to_string_lossy()
        .replace('\\', "/");

    for entry in statuses.iter() {
        let rel = entry.path().unwrap_or("").to_string().replace('\\', "/");
        let abs = status_entry_to_absolute(&path, &rel);
        assert_eq!(
            abs, fs_read_dir_path,
            "git_status 的绝对路径应与 fs_read_dir 的 DirEntry.path 格式完全一致"
        );
    }
}

// ---- CV-BE-02: GitStatusEntry oldPath 重命名检测（git2 底层原语：delta.old_file 提取） ----
// 命令层 oldPath 字段契约（oldPath === null / renamed === 旧路径）见下方 GIT-03 重写测试。

#[test]
fn git_status_renamed_has_old_path() {
    let (_dir, path) = init_temp_repo();
    // commit 文件后用 git mv 重命名（blob 相同 → git2 rename 检测识别）
    commit_file(&path, "old_name.txt", "identical content");
    Command::new("git")
        .args(["mv", "old_name.txt", "new_name.txt"])
        .current_dir(&path)
        .output()
        .unwrap();

    let repo = git2::Repository::open(&path).unwrap();
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    let workdir = dunce::simplified(repo.workdir().unwrap());
    let expected_old = workdir
        .join("old_name.txt")
        .to_string_lossy()
        .replace('\\', "/");

    // 查找 renamed 条目
    let renamed: Vec<_> = statuses
        .iter()
        .filter(|e| {
            let s = e.status();
            s.contains(git2::Status::INDEX_RENAMED) || s.contains(git2::Status::WT_RENAMED)
        })
        .collect();

    assert!(!renamed.is_empty(), "git mv 后应检测到 renamed 条目");
    for entry in &renamed {
        let status_flag = entry.status();
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
        assert_eq!(
            old_path.as_deref(),
            Some(&expected_old[..]),
            "renamed 条目的 oldPath 应为旧绝对路径"
        );
    }
}

/// GIT-03 重写：命令层 oldPath 字段契约（调真实 git_status_impl）
///
/// 旧测试在循环内 `continue` 跳过 renamed 条目后再断言 oldPath 为 none——
/// 条件恒真，永不可失败。重写为真实命令双场景：
/// ① 非 renamed（modified）条目 oldPath === null；
/// ② renamed 条目（git mv 真实构造）oldPath === 旧绝对路径
///    （一手证据：生产已开 renames 检测，git/mod.rs StatusOptions）。
#[test]
fn git_status_command_old_path_field_contract() {
    let (_dir, path) = init_temp_repo();
    // ① 场景：modified 文件（非 renamed）
    commit_file(&path, "test.txt", "original");
    fs::write(path.join("test.txt"), "modified content").unwrap();
    // ② 场景：git mv 重命名（blob 相同 → rename 检测识别）
    commit_file(&path, "old_name.txt", "identical content");
    Command::new("git")
        .args(["mv", "old_name.txt", "new_name.txt"])
        .current_dir(&path)
        .output()
        .unwrap();

    let app = make_app_state(Some(path.clone()));
    let entries = block_on(git_status_impl(&app, &path.to_string_lossy())).unwrap();

    let workdir_str = path.to_string_lossy().replace('\\', "/");

    // ① 非 renamed 条目 oldPath 为 null
    let modified = entries
        .iter()
        .find(|e| e.status == "modified")
        .expect("应有 modified 条目（test.txt 已修改）");
    assert_eq!(modified.old_path, None, "非 renamed 条目 oldPath 应为 null");

    // ② renamed 条目 oldPath 为旧绝对路径（正斜杠格式，与 path 字段同规格）
    let renamed = entries
        .iter()
        .find(|e| e.status == "renamed")
        .expect("应有 renamed 条目（git mv 后）");
    let expected_old = format!("{workdir_str}/old_name.txt");
    assert_eq!(
        renamed.old_path.as_deref(),
        Some(expected_old.as_str()),
        "renamed 条目 oldPath 应为旧绝对路径，实际: {:?}",
        renamed.old_path
    );
    // renamed 条目的 path 字段 = 旧绝对路径：git2-rs StatusEntry::path_bytes()
    // （0.20.4 status.rs）对 head_to_index 非空的条目返回 delta.old_file.path——
    // rename 场景下即旧路径（vendored-libgit2 已锁版本，行为固定）。故命令层
    // path 与 old_path 同源（均取自 old_file）；前端 renamed 分派经 filePath +
    // oldPath 双参定位工作区/HEAD 两侧。
    assert_eq!(renamed.path, format!("{workdir_str}/old_name.txt"));
}

// ---- serde CamelCase 序列化（GitStatusEntry DTO） ----

#[test]
fn git_status_entry_serializes_camelcase() {
    let entry = GitStatusEntry {
        path: "/abs/path".into(),
        status: "modified".into(),
        old_path: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"path\""), "应包含 path: {json}");
    assert!(json.contains("\"status\""), "应包含 status: {json}");
    assert!(
        json.contains("\"oldPath\""),
        "应包含 camelCase 字段 oldPath: {json}"
    );
    assert!(!json.contains("\"Path\""), "不应包含 PascalCase");
    // 非 renamed 条目 oldPath 应为 null
    assert!(
        json.contains("\"oldPath\":null"),
        "非 renamed 条目 oldPath 应为 null: {json}"
    );
}

#[test]
fn git_status_entry_renamed_has_old_path_camelcase() {
    let entry = GitStatusEntry {
        path: "/repo/new.txt".into(),
        status: "renamed".into(),
        old_path: Some("/repo/old.txt".into()),
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(
        json.contains("\"oldPath\":\"/repo/old.txt\""),
        "renamed 条目 oldPath 应为旧路径: {json}"
    );
}

// ---- dunce::simplified() 路径前缀剥离（git2/路径底层原语） ----

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

// ---- Repository::discover 子目录上溯（git2 底层原语） ----

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
    let repo = git2::Repository::discover(&sub_dir).expect("从子目录 discover 应能找到仓库");
    assert!(repo.workdir().is_some(), "应能找到 workdir");

    // 验证路径拼接正确
    let workdir = dunce::simplified(repo.workdir().unwrap());
    let abs = workdir
        .join("sub/deep/file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    let expected = path
        .join("sub/deep/file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(abs, expected, "workdir 拼接路径应与实际路径一致");
}

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
// 集成测试环境（tests/）下 lib 不编译 cfg(test)——validate_path_within_root
// 无"未设置 project_root 豁免"，故 root 传 Some(path) 而非 None。

#[test]
fn get_or_open_repo_cache_miss() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "test.txt", "hello");

    let cache = std::sync::Mutex::new(std::collections::HashMap::new());
    let result = get_or_open_repo(&cache, &path.to_string_lossy(), &Some(path.clone()));
    assert!(
        result.is_ok(),
        "首次访问应成功（cache miss → discover → 缓存）"
    );
    let (_repo, workdir) = result.unwrap();
    assert_eq!(workdir, dunce::simplified(path.as_path()));
}

// ---- CI 门禁回归守卫（8.3 短名根因） ----

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
        get_or_open_repo(&cache, &path.to_string_lossy(), &Some(path.clone())).unwrap();
    assert_eq!(
        workdir,
        dunce::simplified(dunce::canonicalize(&path).unwrap().as_path()),
    );
}

#[test]
fn get_or_open_repo_cache_hit() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "test.txt", "hello");

    let cache = std::sync::Mutex::new(std::collections::HashMap::new());
    // 首次访问 → 缓存
    let result1 = get_or_open_repo(&cache, &path.to_string_lossy(), &Some(path.clone()));
    assert!(result1.is_ok(), "首次访问应成功");

    // 从子目录访问 → 缓存命中（子目录在 workdir 子树内）
    let sub = path.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let result2 = get_or_open_repo(&cache, &sub.to_string_lossy(), &Some(path.clone()));
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
    // 子仓库同样设置仓库局部 user 配置（GIT-06：不依赖 runner 全局配置）
    for (key, value) in [("user.email", "test@test.com"), ("user.name", "Test")] {
        Command::new("git")
            .args(["config", key, value])
            .current_dir(&sub)
            .output()
            .unwrap();
    }
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
    let result_sub = get_or_open_repo(&cache, &sub.to_string_lossy(), &Some(path.clone()));
    assert!(result_sub.is_ok(), "子仓库访问应成功");
    let (_sub_repo, sub_workdir) = result_sub.unwrap();
    assert_eq!(sub_workdir, dunce::simplified(sub.as_path()));

    // 再访问父目录 → 不应命中子仓库缓存（父目录不在子仓库子树内）
    let result_parent = get_or_open_repo(&cache, &path.to_string_lossy(), &Some(path.clone()));
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
    let result = get_or_open_repo(&cache, &non_repo.to_string_lossy(), &Some(non_repo.clone()));
    assert!(result.is_err(), "非 git 目录 discover 应失败");
}

#[test]
fn get_or_open_repo_bare_repo_returns_err() {
    // bare repo 无工作目录 → workdir() 返回 None → Err
    let tmp = tempfile::tempdir().unwrap();
    let bare_path = tmp.path().join("bare.git");
    git2::Repository::init_bare(&bare_path).unwrap();

    let cache = std::sync::Mutex::new(std::collections::HashMap::new());
    let result = get_or_open_repo(
        &cache,
        &bare_path.to_string_lossy(),
        &Some(bare_path.clone()),
    );
    assert!(result.is_err(), "bare repo 无 workdir 应返回错误");
}

// ---- GIT-01 命令层测试：git_status_impl（最小 AppState + block_on await 真实命令） ----

/// happy 路径：modified 文件 → 精确断言（路径 + 状态 + 数量 + oldPath）
#[test]
fn git_status_command_modified_happy_path() {
    let (_dir, path) = init_temp_repo();
    commit_file(&path, "test.txt", "original");
    fs::write(path.join("test.txt"), "modified content").unwrap();

    let app = make_app_state(Some(path.clone()));
    let entries = block_on(git_status_impl(&app, &path.to_string_lossy())).unwrap();

    assert_eq!(entries.len(), 1, "仅 test.txt 一个变更条目");
    assert_eq!(
        entries[0].path,
        path.join("test.txt").to_string_lossy().replace('\\', "/"),
        "path 应为绝对路径（正斜杠，与 fs_read_dir 同规格）"
    );
    assert_eq!(entries[0].status, "modified");
    assert_eq!(entries[0].old_path, None);
}

/// 沙箱拒绝（SEC-01 / GIT-10）：repo_path 在 project_root 外 → 拒绝，不改磁盘
#[test]
fn git_status_command_sandbox_rejects_outside_path() {
    let (_dir, path) = init_temp_repo();
    let outside = tempfile::tempdir().unwrap();
    let outside_repo = outside.path().join("outside_repo");
    std::fs::create_dir_all(&outside_repo).unwrap();

    let app = make_app_state(Some(path.clone()));
    let err = block_on(git_status_impl(&app, &outside_repo.to_string_lossy())).unwrap_err();
    assert!(
        matches!(err, AppError::IoKind { .. }),
        "根外路径应被沙箱拒绝为 IoKind，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("项目范围"),
        "错误消息应含'项目范围'，实际: {err}"
    );
}

/// 错误契约：非 git 目录 → AppError::Git，消息含"打开仓库失败"
#[test]
fn git_status_command_non_repo_error_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let not_repo = tmp.path().join("not_a_repo");
    std::fs::create_dir_all(&not_repo).unwrap();

    let app = make_app_state(Some(tmp.path().to_path_buf()));
    let err = block_on(git_status_impl(&app, &not_repo.to_string_lossy())).unwrap_err();
    assert!(
        matches!(err, AppError::Git(_)),
        "非 git 目录应返回 Git 错误，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("打开仓库失败"),
        "错误消息应含'打开仓库失败'，实际: {err}"
    );
}
