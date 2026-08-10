// agent-status-lib.test.ts — lib 层会话四态类型与 emoji 常量测试（MC-401 迁移）
//
// 语义来源（MC-401 迁移）：原 claude-status.test.ts（32 用例）拆分——与事件
// 映射无关的 lib 层用例（STATUS_EMOJI 常量 / getStatusIcon / AgentStatus 类型）
// 留本文件（6 用例）；eventToStatus 事件映射用例（26 用例）随实现迁入
// src/features/cliProfiles/profiles/claude/（落点 cli-profile-claude.test.ts，
// 语义不丢）。lib 层已不含 claude 事件名字面量（AC-5 守卫兼容）。
// 契约依据: src/lib/agentStatus.ts（C7）

import { describe, it, expect } from "vitest";
import {
  STATUS_EMOJI,
  getStatusIcon,
  type AgentStatus,
} from "../lib/agentStatus";

// ═══════════════════════════════════════════════════════════════════
// STATUS_EMOJI 常量
// ═══════════════════════════════════════════════════════════════════

describe("STATUS_EMOJI", () => {
  it("应包含恰好 4 个键（四态：working/attention/done/error）", () => {
    const keys = Object.keys(STATUS_EMOJI);
    expect(keys).toHaveLength(4);
    expect(keys.sort()).toEqual(["attention", "done", "error", "working"]);
  });

  it("每个状态映射到正确的 emoji（契约 C7）", () => {
    expect(STATUS_EMOJI.working).toBe("⚡");
    expect(STATUS_EMOJI.attention).toBe("🟡");
    expect(STATUS_EMOJI.done).toBe("✅");
    expect(STATUS_EMOJI.error).toBe("❌");
  });

  it("null 状态不在映射表中（null 表示无图标）", () => {
    expect(STATUS_EMOJI).not.toHaveProperty("null");
    expect("null" in STATUS_EMOJI).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════
// getStatusIcon — 状态 → emoji 映射
// ═══════════════════════════════════════════════════════════════════

describe("getStatusIcon", () => {
  it("null 状态返回空字符串（无图标）", () => {
    expect(getStatusIcon(null)).toBe("");
  });

  it("working 状态返回 ⚡", () => {
    expect(getStatusIcon("working")).toBe("⚡");
  });
});

// ═══════════════════════════════════════════════════════════════════
// AgentStatus 类型
// ═══════════════════════════════════════════════════════════════════

describe("AgentStatus 类型", () => {
  it("接受合法的 5 个值（四态 + null）", () => {
    const states: AgentStatus[] = ["working", "attention", "done", "error", null];
    expect(states).toHaveLength(5);
  });
});
