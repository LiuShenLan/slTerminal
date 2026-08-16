// agent-status-lib.test.ts — lib 层会话四态类型契约测试（MC-401 迁移 + IC-03 退役）
//
// 语义来源（MC-401 迁移）：原 claude-status.test.ts（32 用例）拆分——与事件
// 映射无关的 lib 层用例（STATUS_EMOJI 常量 / getStatusIcon / AgentStatus 类型）
// 留本文件；eventToStatus 事件映射用例随实现迁入
// src/features/cliProfiles/profiles/claude/（落点 cli-profile-claude.test.ts）。
// IC-03（UI 重设计）：STATUS_EMOJI/getStatusIcon 已删除——渲染层改 StatusDot
// （src/lib/StatusDot.tsx），本文件保留 AgentStatus 类型契约 + 退役回归守卫。
// 契约依据: src/lib/agentStatus.ts

import { describe, it, expect } from "vitest";
import * as agentStatusModule from "../lib/agentStatus";
import type { AgentStatus } from "../lib/agentStatus";

// ═══════════════════════════════════════════════════════════════════
// AgentStatus 类型
// ═══════════════════════════════════════════════════════════════════

describe("AgentStatus 类型", () => {
  it("接受合法的 5 个值（四态 + null）", () => {
    const states: AgentStatus[] = ["working", "attention", "done", "error", null];
    expect(states).toHaveLength(5);
  });
});

// ═══════════════════════════════════════════════════════════════════
// STATUS_EMOJI / getStatusIcon 退役守卫（IC-03）
// ═══════════════════════════════════════════════════════════════════

describe("STATUS_EMOJI / getStatusIcon 退役（IC-03）", () => {
  it("STATUS_EMOJI 不再导出（渲染层改 StatusDot，emoji 扫描守卫兜底）", () => {
    expect((agentStatusModule as Record<string, unknown>).STATUS_EMOJI).toBeUndefined();
  });

  it("getStatusIcon 不再导出", () => {
    expect((agentStatusModule as Record<string, unknown>).getStatusIcon).toBeUndefined();
  });
});
