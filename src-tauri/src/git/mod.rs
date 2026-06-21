//! Git 模块 — git2 读操作（status/log/diff）+ shell worktree 写操作
//!
//! 读写分离（决策 D5）：
//! - git2：is_repo / root（读操作，快速稳定）
//! - shell git CLI：worktree list/add/remove（写操作，git2 Windows worktree bug #4977/#5280 仍 OPEN）

use crate::error::AppError;
use git2::Repository;

pub mod worktree;

/// 检查路径是否为 git 仓库（git2，读操作）
#[tauri::command]
pub fn git_is_repo(path: String) -> Result<bool, AppError> {
    Ok(Repository::open(&path).is_ok())
}

/// 获取 git 仓库根目录（git2，读操作）
#[tauri::command]
pub fn git_root(path: String) -> Result<String, AppError> {
    let repo = Repository::open(&path).map_err(|e| AppError::Git(e.to_string()))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("仓库无工作目录（bare repo）".to_string()))?;
    Ok(workdir.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_init_temp() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(&path)
            .output()
            .unwrap();
        std::fs::write(format!("{}/test.txt", &path), "hello").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&path)
            .output()
            .unwrap();
        (dir, path)
    }

    #[test]
    fn test_git_is_repo_positive_and_negative() {
        let (_dir, path) = git_init_temp();
        assert!(git_is_repo(path.clone()).unwrap());

        let non_repo = std::env::temp_dir().to_string_lossy().to_string();
        assert!(!git_is_repo(non_repo).unwrap());
    }

    #[test]
    fn test_git_root_returns_correct_path() {
        let (_dir, path) = git_init_temp();
        let root = git_root(path.clone()).unwrap();
        assert!(root.len() > 0);
    }
}
