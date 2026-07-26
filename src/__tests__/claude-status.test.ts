// claude-status.test.ts — 四态映射纯函数测试
//
// 覆盖 eventToStatus 全分支（10 事件 × notificationType 组合）
// 状态机完整表见 docs/hooks-dev/feature-plan/phase1-status-core.md F3 节
// 契约依据: docs/hooks-dev/contract.md C7

import { describe, it, expect } from "vitest";
import {
  eventToStatus,
  STATUS_EMOJI,
  type ClaudeStatus,
} from "../lib/claudeStatus";

// ═══════════════════════════════════════════════════════════════════
// eventToStatus — 10 事件全分支覆盖
// ═══════════════════════════════════════════════════════════════════

describe("eventToStatus", () => {
  // ── 会话生命周期（2 事件）───────────────────────────────────

  describe("SessionStart", () => {
    it("应返回 attention（会话就绪，等待用户输入）", () => {
      expect(eventToStatus("SessionStart")).toBe("attention");
    });

    it("notificationType 不影响结果（SessionStart 不读取该字段）", () => {
      expect(eventToStatus("SessionStart", "idle_prompt")).toBe("attention");
      expect(eventToStatus("SessionStart", null)).toBe("attention");
    });
  });

  describe("SessionEnd", () => {
    it("应返回 null（会话结束，恢复默认无图标）", () => {
      expect(eventToStatus("SessionEnd")).toBeNull();
    });

    it("notificationType 不影响结果", () => {
      expect(eventToStatus("SessionEnd", "permission_prompt")).toBeNull();
      expect(eventToStatus("SessionEnd", null)).toBeNull();
    });
  });

  // ── 用户交互（3 事件）───────────────────────────────────────

  describe("UserPromptSubmit", () => {
    it("应返回 working（用户提交 prompt，claude 处理中）", () => {
      expect(eventToStatus("UserPromptSubmit")).toBe("working");
    });
  });

  describe("Stop", () => {
    it("应返回 done（本轮完成）", () => {
      expect(eventToStatus("Stop")).toBe("done");
    });
  });

  describe("StopFailure", () => {
    it("应返回 error（停止失败）", () => {
      expect(eventToStatus("StopFailure")).toBe("error");
    });
  });

  // ── 工具调用（3 事件）───────────────────────────────────────

  describe("PreToolUse", () => {
    it("应返回 working（工具执行开始）", () => {
      expect(eventToStatus("PreToolUse")).toBe("working");
    });
  });

  describe("PostToolUse", () => {
    it("应返回 working（工具执行完成，继续处理）", () => {
      expect(eventToStatus("PostToolUse")).toBe("working");
    });
  });

  describe("PostToolUseFailure", () => {
    it("应返回 error（工具执行失败）", () => {
      expect(eventToStatus("PostToolUseFailure")).toBe("error");
    });
  });

  // ── 注意信号（2 事件）───────────────────────────────────────

  describe("PermissionRequest", () => {
    it("应返回 attention（等待用户授权）", () => {
      expect(eventToStatus("PermissionRequest")).toBe("attention");
    });
  });

  // ── Notification（10 事件之一，按 notificationType 区分）────

  describe("Notification", () => {
    describe("attention 子类型（需用户处理）", () => {
      it("permission_prompt → attention（权限请求需要用户处理）", () => {
        expect(eventToStatus("Notification", "permission_prompt")).toBe("attention");
      });

      it("idle_prompt → attention（空闲等待用户输入）", () => {
        expect(eventToStatus("Notification", "idle_prompt")).toBe("attention");
      });

      it("agent_needs_input → attention（agent 需要用户输入）", () => {
        expect(eventToStatus("Notification", "agent_needs_input")).toBe("attention");
      });
    });

    describe("非 attention 子类型（不改变当前状态）", () => {
      it("auth_success → null", () => {
        expect(eventToStatus("Notification", "auth_success")).toBeNull();
      });

      it("其他未知子类型 → null", () => {
        expect(eventToStatus("Notification", "info")).toBeNull();
        expect(eventToStatus("Notification", "warning")).toBeNull();
      });
    });

    describe("notificationType 缺失", () => {
      it("notificationType 为 null → null", () => {
        expect(eventToStatus("Notification", null)).toBeNull();
      });

      it("notificationType 为 undefined → null", () => {
        expect(eventToStatus("Notification", undefined)).toBeNull();
      });

      it("无 notificationType 参数（默认 undefined）→ null", () => {
        expect(eventToStatus("Notification")).toBeNull();
      });
    });
  });

  // ── 未识别事件（default 分支）───────────────────────────────

  describe("未识别事件", () => {
    it("应返回 null（不改变当前状态）", () => {
      expect(eventToStatus("UnknownEvent")).toBeNull();
      expect(eventToStatus("SubagentStart")).toBeNull();
      expect(eventToStatus("PreCompact")).toBeNull();
      expect(eventToStatus("ConfigChange")).toBeNull();
      expect(eventToStatus("")).toBeNull();
    });
  });

  // ── 事件驱动覆盖生命周期 ────────────────────────────────────

  it("完整会话周期：启动 → 处理 → 注意 → 处理 → 完成 → 新轮次 → 错误 → 退出", () => {
    // 启动
    expect(eventToStatus("SessionStart")).toBe("attention"); // 🟡 等待
    // 用户提交 prompt
    expect(eventToStatus("UserPromptSubmit")).toBe("working"); // ⚡ 处理中
    // 工具执行
    expect(eventToStatus("PreToolUse")).toBe("working"); // ⚡ 工具执行
    expect(eventToStatus("PostToolUse")).toBe("working"); // ⚡ 继续处理
    // 权限请求（需要用户处理）
    expect(eventToStatus("Notification", "permission_prompt")).toBe("attention"); // 🟡 等待
    // 授权后继续
    expect(eventToStatus("PreToolUse")).toBe("working"); // ⚡ 覆盖 attention
    // 完成
    expect(eventToStatus("Stop")).toBe("done"); // ✅ 完成
    // 新一轮 prompt
    expect(eventToStatus("UserPromptSubmit")).toBe("working"); // ⚡ 再处理
    // 失败
    expect(eventToStatus("StopFailure")).toBe("error"); // ❌ 失败
    // 会话结束
    expect(eventToStatus("SessionEnd")).toBeNull(); // 无图标
  });

  it("Notification attention 后继续被后续事件覆盖", () => {
    expect(eventToStatus("Notification", "permission_prompt")).toBe("attention");
    // 授权后 claude 继续执行 → 下一个 PreToolUse 覆盖
    expect(eventToStatus("PreToolUse")).toBe("working");
  });

  it("Notification 非 attention 类型不覆盖当前状态（返回 null = 不改变）", () => {
    // auth_success notification 在上层应被解释为"不改变当前状态"
    expect(eventToStatus("Notification", "auth_success")).toBeNull();
  });
});

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

  it("eventToStatus 返回 non-null 时 STATUS_EMOJI[status] 必定合法", () => {
    // 所有能产生 non-null 结果的事件组合，STATUS_EMOJI 都必须有对应 emoji
    const nonNullCases: Array<[string, string?, string?]> = [
      ["SessionStart", undefined, "attention"],
      ["UserPromptSubmit", undefined, "working"],
      ["PreToolUse", undefined, "working"],
      ["PostToolUse", undefined, "working"],
      ["Stop", undefined, "done"],
      ["PostToolUseFailure", undefined, "error"],
      ["StopFailure", undefined, "error"],
      ["PermissionRequest", undefined, "attention"],
      ["Notification", "permission_prompt", "attention"],
      ["Notification", "idle_prompt", "attention"],
      ["Notification", "agent_needs_input", "attention"],
    ];

    for (const [event, notifType, expected] of nonNullCases) {
      const status = eventToStatus(event, notifType);
      expect(status).toBe(expected);
      // 非 null 状态必须能索引到 emoji
      expect(STATUS_EMOJI[status!]).toBeDefined();
      expect(typeof STATUS_EMOJI[status!]).toBe("string");
    }
  });

  it("eventToStatus 返回 null 时不索引 STATUS_EMOJI（null 无图标）", () => {
    const nullCases: Array<[string, string?]> = [
      ["SessionEnd"],
      ["SessionEnd", undefined],
      ["Notification", "auth_success"],
      ["Notification", undefined],
      ["Notification", undefined],
      ["UnknownEvent"],
    ];

    for (const [event, notifType] of nullCases) {
      expect(eventToStatus(event, notifType)).toBeNull();
    }
  });
});

// ═══════════════════════════════════════════════════════════════════
// ClaudeStatus 类型
// ═══════════════════════════════════════════════════════════════════

describe("ClaudeStatus 类型", () => {
  it("接受合法的 5 个值（四态 + null）", () => {
    const states: ClaudeStatus[] = ["working", "attention", "done", "error", null];
    expect(states).toHaveLength(5);
  });

  it("eventToStatus 返回值始终可赋值给 ClaudeStatus 类型", () => {
    // 编译期验证：所有 eventToStatus 返回值类型与 ClaudeStatus 兼容
    const s1: ClaudeStatus = eventToStatus("UserPromptSubmit");
    expect(s1).toBe("working");

    const s2: ClaudeStatus = eventToStatus("Notification", "permission_prompt");
    expect(s2).toBe("attention");

    const s3: ClaudeStatus = eventToStatus("SessionEnd");
    expect(s3).toBeNull();
  });
});
