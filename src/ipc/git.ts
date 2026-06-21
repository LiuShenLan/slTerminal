// Git IPC 封装 — 前端调用 Git 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";
import type { WorktreeInfo } from "../types/git";

/** 检查路径是否为 git 仓库 */
export async function isRepo(path: string): Promise<boolean> {
  return invoke<boolean>("git_is_repo", { path });
}

/** 获取 git 仓库根目录 */
export async function getRoot(path: string): Promise<string> {
  return invoke<string>("git_root", { path });
}

/** 列出仓库的所有 worktree */
export async function listWorktrees(repoPath: string): Promise<WorktreeInfo[]> {
  return invoke<WorktreeInfo[]>("git_worktree_list", { repoPath });
}

/** 添加新 worktree */
export async function addWorktree(
  repoPath: string,
  name: string,
): Promise<WorktreeInfo> {
  return invoke<WorktreeInfo>("git_worktree_add", { repoPath, name });
}

/** 移除 worktree */
export async function removeWorktree(
  repoPath: string,
  name: string,
): Promise<void> {
  await invoke("git_worktree_remove", { repoPath, name });
}
