// strategies.ts — claude hooks + history 策略实现（MC-214 前端半 + MC-422 + Stage 05）
//
// claude 合法领地：hook 事件名/notificationType 子类型/命令名 "claude" 字面量
// 只允许出现在 profiles/claude/ 目录（AC-5 守卫——通用层经 profile 能力委托
// 消费，不写 claude 字面量）。本文件实现迁自 lib 层状态映射模块（Stage 02
// MC-401 更名 agentStatus 后落点 src/lib/agentStatus.ts）的
// eventToStatus（32 用例语义不丢，落点 cli-profile-claude.test.ts）与
// src/features/notifications/ 的 classifyEvent 五映射（MC-422，行为零改动）；
// history 策略（buildResumeCommand/buildRestoreInput）Stage 05 迁自
// features/claudeHistory/ 的 historyContextMenu.ts 与 restoreSession.ts，
// 输出与迁出源逐字一致（E2E history.e2e 恢复编排用例零改动通过）；唯一差异
// 点 = cwd 单引号转义（AQ-1 修复，见 buildResumeCommand 注释）。

import type { AgentStatus } from "../../../../lib/agentStatus";
import type { AgentEventPayload } from "../../../../types/agent";
import type { AgentHistorySession } from "../../../../types/agentHistory";

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

// ═══════════════════════════════════════════════════════════════════
// history 能力（MC-315/316 迁入，输出与迁出源逐字一致；差异点 = cwd 单引号转义，AQ-1）
// ═══════════════════════════════════════════════════════════════════

/**
 * 恢复命令构造（右键菜单「复制恢复命令」，MC-316 迁自 historyContextMenu.ts
 * buildResumeCommand，AQ-1 修复）：
 * 有 cwd → `cd '<cwd>' && claude --resume <id>`（带单引号路径）；
 * 无 cwd → 仅 `claude --resume <id>`。
 * cwd 单引号按 PowerShell 规则转义为 `''`（AQ-1 修复）——单引号字符串内
 * `''` = 字面单引号，路径含单引号（如 C:\Bob's Project）时不再断命令；
 * cwd 无单引号时输出与迁出源逐字一致（E2E history.e2e fixture cwd 无
 * 单引号，预期不变）。
 */
export function buildResumeCommand(session: AgentHistorySession): string {
  const resume = `claude --resume ${session.sessionId}`;
  return session.cwd
    ? `cd '${session.cwd.replace(/'/g, "''")}' && ${resume}`
    : resume;
}

/**
 * 恢复注入内容（恢复编排第 4 步，MC-315 迁自 restoreSession.ts:137-139 字面量，
 * 输出必须逐字一致）：`claude --resume <id>` + fork 时追加 " --fork-session" +
 * `\r` 结尾——E2E history.e2e 恢复编排用例零改动通过，断言漂移即实现有误。
 */
export function buildRestoreInput(
  session: AgentHistorySession,
  opts: { fork: boolean },
): string {
  return `claude --resume ${session.sessionId}${
    opts.fork ? " --fork-session" : ""
  }\r`;
}
