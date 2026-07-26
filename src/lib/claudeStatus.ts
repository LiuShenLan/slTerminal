// claudeStatus.ts — 四态类型定义、emoji 常量、事件→状态纯函数
// 状态机完整表见 docs/hooks-dev/feature-plan/phase1-status-core.md F3 节

/** Claude 会话状态 */
export type ClaudeStatus = "working" | "attention" | "done" | "error" | null;

/** 各状态对应 emoji，不含 null（null 表示无图标） */
export const STATUS_EMOJI: Record<Exclude<ClaudeStatus, null>, string> = {
  working: "⚡",
  attention: "🟡",
  done: "✅",
  error: "❌",
};

/**
 * 根据 ClaudeStatus 返回对应 emoji 图标。
 * null 状态返回空字符串（无图标），未识别状态也返回空字符串。
 */
export function getStatusIcon(status: ClaudeStatus): string {
  if (status === null) return "";
  return STATUS_EMOJI[status] ?? "";
}

/** Notification 事件中需要用户处理的子类型 */
const ATTENTION_NOTIFICATION_TYPES = new Set([
  "permission_prompt",
  "idle_prompt",
  "agent_needs_input",
]);

/**
 * 把 hook 事件名映射为 ClaudeStatus。
 *
 * - Notification 事件通过 notificationType 区分：三类 attention 子类型返回 "attention"，其余返回 null
 * - 未识别的事件返回 null
 */
export function eventToStatus(
  event: string,
  notificationType?: string | null,
): ClaudeStatus {
  switch (event) {
    case "SessionStart":
      return "attention";
    case "UserPromptSubmit":
    case "PreToolUse":
    case "PostToolUse":
      return "working";
    case "Notification":
      // 仅 permission_prompt / idle_prompt / agent_needs_input 需要用户处理
      if (
        notificationType != null &&
        ATTENTION_NOTIFICATION_TYPES.has(notificationType)
      ) {
        return "attention";
      }
      // 其他 notification 类型不改变状态
      return null;
    case "PermissionRequest":
      return "attention";
    case "Stop":
      return "done";
    case "PostToolUseFailure":
    case "StopFailure":
      return "error";
    case "SessionEnd":
      return null;
    default:
      // 未识别事件不改变状态
      return null;
  }
}
