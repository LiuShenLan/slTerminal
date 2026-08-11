// resolvePayloadCliId — agent-event payload 来源 CLI 标识解析单点（MC-205 三级解析）
//
// 契约 4：三处消费方（useXterm / useAgentStatus / useAgentNotifications）曾各自
// 内联 `payload.cliId ?? TerminalRegistry.get()?.agentSession?.cliId ?? CLAUDE_CLI_ID`
// 链——ZQ-2 抽此单点，统一三处语义，且空串/仅空白与 null/undefined 同等回退
// （原 ?? 链不处理空串：payload.cliId="" 会短路成空串，下游 profile 解析全部失效）。

import type { AgentEventPayload } from "../../types/agent";
import { TerminalRegistry } from "./TerminalRegistry";
// AC-5: 缺省回退常量经 profiles/claude 导出（禁止在通用层写 "claude" 字面量）
import { CLAUDE_CLI_ID } from "../../features/cliProfiles/profiles/claude";

/**
 * 解析 agent-event payload 的来源 CLI 标识（MC-205 三级解析语义不变）：
 * 1. payload.cliId 显式分支——trim 后非空才取用（ZQ-2：空串/仅空白/null/undefined 同等回退）；
 * 2. 反查分支——TerminalRegistry.get(panelId)?.agentSession?.cliId（OSC 133 C 命中时写入）；
 * 3. 缺省分支——CLAUDE_CLI_ID（兼容无 cliId 字段的旧信号，serde default）。
 */
export function resolvePayloadCliId(payload: AgentEventPayload): string {
  return (
    payload.cliId?.trim() ||
    TerminalRegistry.get(payload.panelId)?.agentSession?.cliId ||
    CLAUDE_CLI_ID
  );
}
