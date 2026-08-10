// agentStatus.ts — 编码 CLI 会话四态类型与 emoji 常量（CLI 中立，MC-401 迁移）
//
// 事件→状态映射单点已随 MC-401 迁入 CLI profile hooks 能力：消费方按
// profile.hooks.eventToStatus 分发（claude 实现在
// src/features/cliProfiles/profiles/claude/strategies.ts）。lib 层不再含
// 任何 claude 事件名字面量（AC-5 守卫兼容）。
//
// 已知行为假设（无法自动化验证）：
// - Ctrl+C 用户主动中断不发射任何 hook 事件（完成/错误事件为预期语义），
//   working(⚡) 无中断出边为预期行为，依赖下一事件覆盖或空闲提示(~60s) 衰减转 🟡

/** 编码 CLI 会话状态（四态 + null 表示无状态无图标） */
export type AgentStatus = "working" | "attention" | "done" | "error" | null;

/**
 * 各状态对应 emoji，不含 null（null 表示无图标）。
 * 与通知类别 emoji（🔐❌✅，见 src/features/notifications/ 的 CATEGORY_EMOJI）
 * 值有重叠但语义不同——两常量集不合并（MC-404）。
 */
export const STATUS_EMOJI: Record<Exclude<AgentStatus, null>, string> = {
  working: "⚡",
  attention: "🟡",
  done: "✅",
  error: "❌",
};

/**
 * 根据 AgentStatus 返回对应 emoji 图标。
 * null 状态返回空字符串（无图标），未识别状态也返回空字符串。
 */
export function getStatusIcon(status: AgentStatus): string {
  if (status === null) return "";
  return STATUS_EMOJI[status] ?? "";
}
