// Git IPC 封装 — 前端调用 git 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";
import type { GitStatusEntry, DiffHunk } from "../types/git";

/** 获取指定仓库的文件 git 状态 */
export async function gitStatus(repoPath: string): Promise<GitStatusEntry[]> {
  return invoke<GitStatusEntry[]>("git_status", { repoPath });
}

/** 获取指定文件的 HEAD ↔ 工作区 diff hunks */
export async function gitDiff(
  repoPath: string,
  filePath: string,
): Promise<DiffHunk[]> {
  return invoke<DiffHunk[]>("git_diff", { repoPath, filePath });
}
