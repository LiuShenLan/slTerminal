// claude 历史会话 IPC — 扫描/删除封装（契约见 checklist「跨边界契约」）
//
// 本文件是 claude_history_scan / claude_history_delete 两条命令的唯一 invoke 位置（硬约束 #1）。
import { invoke } from "@tauri-apps/api/core";
import type { HistorySession } from "../types/claudeHistory";

/**
 * 扫描全部历史会话
 *
 * 后端遍历扫描根（env SLTERM_CLAUDE_PROJECTS_DIR 优先，缺省 ~/.claude/projects），
 * 单文件解析失败降级条目、扫描根不存在返回空数组（均非 Err）。
 */
export async function scanHistory(): Promise<HistorySession[]> {
  return invoke("claude_history_scan");
}

/**
 * 删除指定历史会话（transcript jsonl + 同名目录）
 *
 * 后端按 sessionId 定位文件（前端不传路径，SEC-01）；会话不存在返回 Err。
 */
export async function deleteHistorySession(sessionId: string): Promise<void> {
  return invoke("claude_history_delete", { sessionId });
}
