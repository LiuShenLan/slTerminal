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

/**
 * 获取指定文件在 HEAD commit 中的内容（只读 blob）
 * 仓库尚无提交或文件不在 HEAD → invoke 抛出 AppError::Git，消息含"HEAD 中不存在"
 */
export async function gitFileAtHead(
  repoPath: string,
  filePath: string,
): Promise<string> {
  return invoke<string>("git_file_at_head", { repoPath, filePath });
}
