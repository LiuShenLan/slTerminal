// strategies.ts — claude hooks 策略实现（MC-214 前端半 + MC-422 迁入）
//
// claude 合法领地：hook 事件名/notificationType 子类型字面量只允许出现在
// profiles/claude/ 目录（AC-5 守卫——通用层经 profile.hooks 能力委托消费，
// 不写 claude 字面量）。本文件实现迁自 lib 层状态映射模块（Stage 02
// MC-401 更名 agentStatus 后落点 src/lib/agentStatus.ts）的
// eventToStatus（32 用例语义不丢，落点 cli-profile-claude.test.ts）与
// src/features/notifications/ 的 classifyEvent 五映射（MC-422，行为零改动）。

import type { AgentStatus } from "../../../../lib/agentStatus";
import type { AgentEventPayload } from "../../../../types/agent";

/** Notification 事件中需要用户处理的子类型 */
export const ATTENTION_NOTIFICATION_TYPES = new Set([
  "permission_prompt",
  "idle_prompt",
  "agent_needs_input",
]);

/**
 * 把 hook 事件名映射为 AgentStatus（F3 状态机，10 事件映射）。
 *
 * - Notification 事件通过 notificationType 区分：三类 attention 子类型返回 "attention"，其余返回 null
 * - 未识别的事件返回 null
 */
export function eventToStatus(
  event: string,
  notificationType?: string | null,
): AgentStatus {
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

/**
 * hook 事件负载 → 通知类别（五映射，行为与迁入前 classifyEvent 一致）
 *
 * 规则（优先级自上而下）：
 *   - 权限请求：event === "PermissionRequest" 或 (event === "Notification" 且 notificationType === "permission_prompt")
 *   - 错误：event === "StopFailure" 或 "PostToolUseFailure"
 *   - 任务完成：event === "Stop"
 *   - 其他：不触发通知
 */
export function classifyNotification(
  payload: AgentEventPayload,
): "permission" | "error" | "done" | null {
  // 权限请求
  if (payload.event === "PermissionRequest") return "permission";
  if (
    payload.event === "Notification" &&
    payload.notificationType === "permission_prompt"
  )
    return "permission";

  // 错误
  if (payload.event === "StopFailure" || payload.event === "PostToolUseFailure")
    return "error";

  // 任务完成
  if (payload.event === "Stop") return "done";

  // 其他事件（PreToolUse / PostToolUse / SessionStart / SessionEnd 等）不触发 toast
  return null;
}
