// agent 历史会话 IPC — 扫描/删除/读标题封装（契约见 checklist「跨边界契约」）
//
// 本文件是 agent_history_scan / agent_history_delete / agent_history_read_title
// 三条命令的唯一 invoke 位置（硬约束 #1）。
import { invoke } from "@tauri-apps/api/core";
import type { AgentHistorySession, AgentHistoryTitle } from "../types/agentHistory";

/**
 * 扫描全部历史会话（无参聚合）
 *
 * 后端遍历全部已注册 provider 串行聚合，单 provider 失败不阻塞其他；
 * 全部为空 → 空数组；单文件解析失败降级条目（均非 Err）。
 */
export async function scanHistory(): Promise<AgentHistorySession[]> {
  return invoke("agent_history_scan");
}

/**
 * 删除指定历史会话（transcript jsonl + 同名目录）
 *
 * 后端按 cliId 路由 provider，delete 前经该 provider validate_session_id 前置校验
 * （SEC-05 等价强制——前端不传路径，仅传 cliId + sessionId）；未知 cliId 返回 Err。
 */
export async function deleteHistorySession(
  cliId: string,
  sessionId: string,
): Promise<void> {
  return invoke("agent_history_delete", { cliId, sessionId });
}

/**
 * 读取单会话标题（运行中会话页签/导航树行显示名——人工验证问题 3）
 *
 * 回退链与历史扫描同源（custom-title > ai-title > summary > firstPrompt）；
 * 会话文件不存在 → `{ title: null }`（非 Err——调用方兜底 CLI 名）；
 * 未知 cliId / 非法 sessionId → Err（调用方 catch 静默保持现标题）。
 */
export async function readHistoryTitle(
  cliId: string,
  sessionId: string,
): Promise<AgentHistoryTitle> {
  return invoke("agent_history_read_title", { cliId, sessionId });
}
