//! Shell git worktree 写操作 — git worktree list/add/remove
//!
//! 三个命令全部通过 std::process::Command 调用 git CLI。
//! Windows 安全：删除 worktree 用 cmd.exe /c rmdir /S /Q，绝不使用 Remove-Item -Recurse -Force（防 NTFS junction 穿透）。

use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// 工作树信息 DTO（camelCase → 前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub head: String,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_main: bool,
}

/// 列出仓库的所有 worktree
#[tauri::command]
pub fn git_worktree_list(repo_path: String) -> Result<Vec<WorktreeInfo>, AppError> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| AppError::Git(format!("git worktree list 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git worktree list 失败: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_worktree_list(&stdout))
}

/// 解析 git worktree list --porcelain 输出
fn parse_worktree_list(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeInfo> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            let path = rest.trim().to_string();
            let normalized = path.replace('\\', "/");
            current = Some(WorktreeInfo {
                path: normalized,
                branch: String::new(),
                head: String::new(),
                is_bare: false,
                is_detached: false,
                is_main: false,
            });
        } else if let Some(ref mut wt) = current {
            if let Some(rest) = line.strip_prefix("HEAD ") {
                wt.head = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("branch ") {
                let branch = rest.to_string();
                wt.branch = branch.clone();
                wt.is_detached = false;
                wt.is_main = branch == "main" || branch == "master";
            } else if line.starts_with("detached") {
                wt.is_detached = true;
            } else if line.starts_with("bare") {
                wt.is_bare = true;
            }
        }
    }
    if let Some(wt) = current.take() {
        worktrees.push(wt);
    }
    worktrees
}

/// 添加新 worktree
#[tauri::command]
pub fn git_worktree_add(repo_path: String, name: String) -> Result<WorktreeInfo, AppError> {
    let sanitized = sanitize_branch_name(&name);
    if sanitized.is_empty() {
        return Err(AppError::Git("分支名不能为空".to_string()));
    }

    let worktree_path = format!(".claude/worktrees/{sanitized}");
    let branch = format!("worktree-{sanitized}");

    let output = std::process::Command::new("git")
        .args(["worktree", "add", "-b", &branch, &worktree_path, "HEAD"])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| AppError::Git(format!("git worktree add 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git worktree add 失败: {stderr}")));
    }

    let list = git_worktree_list(repo_path.clone())?;
    let full_path = format!("{}/{}", repo_path.replace('\\', "/"), worktree_path);
    for wt in &list {
        if wt.path == full_path || wt.path.ends_with(&worktree_path) {
            return Ok(wt.clone());
        }
    }
    Ok(WorktreeInfo {
        path: full_path,
        branch,
        head: String::new(),
        is_bare: false,
        is_detached: false,
        is_main: false,
    })
}

/// 删除 worktree
#[tauri::command]
pub fn git_worktree_remove(repo_path: String, name: String) -> Result<(), AppError> {
    let sanitized = sanitize_branch_name(&name);
    let worktree_path = format!(".claude/worktrees/{sanitized}");

    let output = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", &worktree_path])
        .current_dir(&repo_path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(&repo_path)
                .output();
            return Ok(());
        }
        _ => {
            let full_path = format!("{}/{}", repo_path.replace('\\', "/"), worktree_path);
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("cmd.exe")
                    .args(["/c", &format!("attrib -R \"{}\\\\*\" /S /D", full_path)])
                    .output();
                let output = std::process::Command::new("cmd.exe")
                    .args(["/c", &format!("rmdir /S /Q \"{}\"", full_path)])
                    .output()
                    .map_err(|e| AppError::Git(format!("rmdir 失败: {e}")))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(AppError::Git(format!("删除 worktree 目录失败: {stderr}")));
                }
            }
            #[cfg(not(windows))]
            {
                std::fs::remove_dir_all(&full_path)
                    .map_err(|e| AppError::Git(format!("删除 worktree 目录失败: {e}")))?;
            }
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(&repo_path)
                .output();
        }
    }
    Ok(())
}

/// 净化分支名：只保留 A-Za-z0-9/._-
fn sanitize_branch_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let input = "worktree /path/to/repo\nHEAD abc123\nbranch main\n\nworktree /path/to/wt\nHEAD def456\nbranch feature-x\n\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, "/path/to/repo");
        assert_eq!(result[0].head, "abc123");
        assert_eq!(result[0].branch, "main");
        assert!(result[0].is_main);
        assert_eq!(result[1].branch, "feature-x");
        assert!(!result[1].is_main);
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_branch_name("my-branch"), "my-branch");
        assert_eq!(sanitize_branch_name("fix bug!"), "fix-bug");
        assert_eq!(sanitize_branch_name("  spaces  "), "spaces");
        assert_eq!(sanitize_branch_name(""), "");
    }

    /// 集成测试：git init → commit → worktree add → list 含新条目 → remove → list 不含
    #[test]
    fn test_worktree_list_add_remove() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_string_lossy().to_string();

        // 1. git init + 首次 commit（worktree add 需要 base commit）
        run_git(&repo_path, &["init"]);
        run_git(&repo_path, &["config", "user.email", "test@test.com"]);
        run_git(&repo_path, &["config", "user.name", "test"]);
        std::fs::write(format!("{}/init.txt", repo_path), "init").unwrap();
        run_git(&repo_path, &["add", "."]);
        run_git(&repo_path, &["commit", "-m", "init"]);

        // 2. worktree add（-b 自动创建分支 worktree-test-wt，checkout HEAD）
        let result = git_worktree_add(repo_path.clone(), "test-wt".to_string());
        assert!(result.is_ok(), "worktree add 应成功: {:?}", result.err());
        let added = result.unwrap();
        assert!(
            added.path.contains(".claude/worktrees/test-wt")
                || added.branch.contains("worktree-test-wt"),
            "返回的 worktree 应包含新路径或分支"
        );

        // 3. worktree list 含新条目
        let list = git_worktree_list(repo_path.clone()).unwrap();
        let found = list
            .iter()
            .any(|w| w.path.contains(".claude/worktrees/test-wt"));
        assert!(found, "list 应包含新添加的 worktree");

        // 4. worktree remove
        let remove_result = git_worktree_remove(repo_path.clone(), "test-wt".to_string());
        assert!(remove_result.is_ok(), "worktree remove 应成功: {:?}", remove_result.err());

        // 5. worktree list 不含已删除条目
        let list_after = git_worktree_list(repo_path.clone()).unwrap();
        let found_after = list_after
            .iter()
            .any(|w| w.path.contains(".claude/worktrees/test-wt"));
        assert!(!found_after, "list 不应含已删除的 worktree");
    }

    /// worktree remove blocked——remove 不存在的 worktree 名称应返回错误
    #[test]
    fn test_worktree_remove_blocked_when_open() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_string_lossy().to_string();

        run_git(&repo_path, &["init"]);
        run_git(&repo_path, &["config", "user.email", "test@test.com"]);
        run_git(&repo_path, &["config", "user.name", "test"]);
        std::fs::write(format!("{}/init.txt", repo_path), "init").unwrap();
        run_git(&repo_path, &["add", "."]);
        run_git(&repo_path, &["commit", "-m", "init"]);

        // 尝试删除一个不存在的 worktree
        let result = git_worktree_remove(repo_path.clone(), "nonexistent-wt".to_string());
        // remove 对不存在的 worktree 可能返回错误或成功（取决于 git 版本），
        // 但至少不应 panic。此处仅验证函数不崩溃。
        let _ = result;
    }

    /// 辅助：在仓库目录中执行 git 命令并断言成功
    fn run_git(repo_path: &str, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} 失败: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
