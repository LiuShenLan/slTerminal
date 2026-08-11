// cli-profile-claude.test.ts — claude profile 身份域 + hooks/history 策略 L2 测试
//
// 语义来源（MC-104 迁移）：原 tab-rules.test.ts（6 用例：side-effect 注册/手动
// 注册/_reset 恢复语义）。
// 覆盖：claude 身份域字段断言（MC-104）+ CLAUDE_CLI_ID 常量一致性 + side-effect
// 注册 + hooks 能力字段（MC-214 前端半：eventToStatus/classifyNotification/
// contextLimit/restartHint/hasConfigEditor）+ history 能力字段（MC-315/316：
// supportsFork/buildResumeCommand/buildRestoreInput）。
// hooks 策略用例（MC-401/MC-422 迁入，Stage 02）：eventToStatus 26 用例语义
// 迁自原 claude-status.test.ts（事件映射部分），落点改此；classifyNotification
// 五映射表驱动（NAH-03 语义）迁自 notifications.test.ts 纯函数层。
// history 策略用例（Stage 05）：buildResumeCommand/buildRestoreInput 输出与
// 迁出源（historyContextMenu.ts / restoreSession.ts）逐字一致——断言漂移即
// 实现有误（E2E history.e2e 恢复编排用例零改动通过）；差异点 = cwd 单引号按
// PowerShell 规则转义为 ''（AQ-1 修复，见 buildResumeCommand 回归用例）。
// logo 资源守卫（MC-108）已移至 cli-profile-registry.test.ts（遍历注册表全部
// profile 断言 iconSrc 磁盘存在 + PNG 魔数——img 404 无报错通道，资源缺失靠此
// 守卫；含 mockcli.png 先行资源，决策 5，Stage 07 mock 夹具引用）。

import { describe, it, expect, afterEach } from "vitest";
// 导入触发 side-effect 注册（照 tab-rules.test.ts 先例，模块加载即注册 claude）
import {
  CLAUDE_CLI_ID,
  claudeProfile,
} from "../features/cliProfiles/profiles/claude";
import {
  eventToStatus,
  classifyNotification,
  buildResumeCommand,
  buildRestoreInput,
} from "../features/cliProfiles/profiles/claude/strategies";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { STATUS_EMOJI, type AgentStatus } from "../lib/agentStatus";
import type { AgentEventPayload } from "../types/agent";
import type { AgentHistorySession } from "../types/agentHistory";

/** 每用例后恢复 claude 注册（全局单例隔离，照 cli-icons.test.ts 模式） */
afterEach(() => {
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
});

describe("claude profile 身份域（MC-104）", () => {
  it("side-effect 注册生效：import profiles/claude 后 matchByCommand('claude') 命中", () => {
    // tabRules 先例——模块顶层 import 已执行注册，此处直接 match 验证副作用
    const profile = cliProfileRegistry.matchByCommand("claude");
    expect(profile).not.toBeNull();
    expect(profile!.id).toBe(CLAUDE_CLI_ID);
  });

  it('CLAUDE_CLI_ID 常量 = "claude"（缺省回退常量定义）', () => {
    expect(CLAUDE_CLI_ID).toBe("claude");
  });

  it("CLAUDE_CLI_ID 与注册 profile.id 一致（防常量与注册漂移）", () => {
    const profile = cliProfileRegistry.get(CLAUDE_CLI_ID);
    expect(profile).not.toBeUndefined();
    expect(profile!.id).toBe(CLAUDE_CLI_ID);
  });

  it("身份域字段完整：id/displayName/commands/iconSrc/tabTitle/capabilities.hooks+history（含策略函数引用）", () => {
    const profile = cliProfileRegistry.get(CLAUDE_CLI_ID)!;
    expect(profile).toEqual({
      id: "claude",
      displayName: "claude",
      commands: ["claude"],
      iconSrc: "/cli-icons/claude.png",
      tabTitle: "claude",
      capabilities: {
        hooks: {
          eventToStatus,
          classifyNotification,
          contextLimit: 200_000,
          restartHint: "hooks 改动需重启 claude 会话生效",
          hasConfigEditor: true,
        },
        history: {
          supportsFork: true,
          buildResumeCommand,
          buildRestoreInput,
        },
      },
    });
  });

  it("capabilities.hooks 五字段齐备（eventToStatus/classifyNotification/contextLimit/restartHint/hasConfigEditor）", () => {
    const hooks = cliProfileRegistry.get(CLAUDE_CLI_ID)!.capabilities.hooks;
    expect(hooks).toBeDefined();
    expect(typeof hooks!.eventToStatus).toBe("function");
    expect(typeof hooks!.classifyNotification).toBe("function");
    expect(hooks!.contextLimit).toBe(200_000);
    expect(hooks!.restartHint).toBe("hooks 改动需重启 claude 会话生效");
    expect(hooks!.hasConfigEditor).toBe(true);
  });

  it("capabilities.history 三字段齐备（supportsFork/buildResumeCommand/buildRestoreInput）", () => {
    const history = cliProfileRegistry.get(CLAUDE_CLI_ID)!.capabilities.history;
    expect(history).toBeDefined();
    expect(history!.supportsFork).toBe(true);
    expect(typeof history!.buildResumeCommand).toBe("function");
    expect(typeof history!.buildRestoreInput).toBe("function");
  });

  it("matchByCommand 带参变体命中 claude（claude --resume xxx）", () => {
    const profile = cliProfileRegistry.matchByCommand("claude --resume abc123");
    expect(profile).not.toBeNull();
    expect(profile!.tabTitle).toBe(CLAUDE_CLI_ID);
  });

  it("_reset 清空后手动重新注册恢复", () => {
    cliProfileRegistry._reset();
    expect(cliProfileRegistry.matchByCommand("claude")).toBeNull();
    cliProfileRegistry.register(claudeProfile);
    expect(cliProfileRegistry.matchByCommand("claude")).not.toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════
// eventToStatus — 10 事件全分支覆盖（MC-401 迁入，原 claude-status 26 用例语义）
// ═══════════════════════════════════════════════════════════════════
// 状态机完整表见 profiles/claude/strategies.ts eventToStatus（F3 事件→状态映射单点）

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

  // ── STATUS_EMOJI × eventToStatus 联合守卫（原 claude-status 语义）────

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

  it("eventToStatus 返回值始终可赋值给 AgentStatus 类型", () => {
    // 编译期验证：所有 eventToStatus 返回值类型与 AgentStatus 兼容
    const s1: AgentStatus = eventToStatus("UserPromptSubmit");
    expect(s1).toBe("working");

    const s2: AgentStatus = eventToStatus("Notification", "permission_prompt");
    expect(s2).toBe("attention");

    const s3: AgentStatus = eventToStatus("SessionEnd");
    expect(s3).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════
// classifyNotification — 五映射表驱动（MC-422 迁入，行为零改动）
// ═══════════════════════════════════════════════════════════════════

/** 构造最小 AgentEventPayload（缺省字段事件用 PreToolUse 占位） */
function makePayload(
  partial: Partial<AgentEventPayload>,
): AgentEventPayload {
  return {
    panelId: "terminal-page1-0",
    event: "PreToolUse",
    timestamp: 1,
    sessionId: "s1",
    transcriptPath: "",
    cwd: "",
    toolName: null,
    notificationType: null,
    ...partial,
  };
}

describe("classifyNotification 五映射表驱动（NAH-03 语义迁入）", () => {
  const classifyTable: Array<{
    name: string;
    payload: Partial<AgentEventPayload>;
    expected: "permission" | "error" | "done" | null;
  }> = [
    { name: "PermissionRequest → permission", payload: { event: "PermissionRequest" }, expected: "permission" },
    { name: "Notification + permission_prompt → permission", payload: { event: "Notification", notificationType: "permission_prompt" }, expected: "permission" },
    { name: "Notification + idle_prompt → null（其他子类型不通知）", payload: { event: "Notification", notificationType: "idle_prompt" }, expected: null },
    { name: "Notification + 无 notificationType → null", payload: { event: "Notification", notificationType: null }, expected: null },
    { name: "Stop → done", payload: { event: "Stop" }, expected: "done" },
    { name: "StopFailure → error", payload: { event: "StopFailure" }, expected: "error" },
    { name: "PostToolUseFailure → error", payload: { event: "PostToolUseFailure" }, expected: "error" },
    { name: "未识别事件（PreToolUse）→ null", payload: { event: "PreToolUse" }, expected: null },
    { name: "未识别事件（SessionStart）→ null", payload: { event: "SessionStart" }, expected: null },
  ];

  it.each(classifyTable)("$name", ({ payload, expected }) => {
    expect(classifyNotification(makePayload(payload))).toBe(expected);
  });
});

// ═══════════════════════════════════════════════════════════════════
// history 策略 — buildResumeCommand / buildRestoreInput（MC-315/316 迁入）
// ═══════════════════════════════════════════════════════════════════
// 输出断言与迁出源（historyContextMenu.ts buildResumeCommand /
// restoreSession.ts:137-139 字面量）逐字一致，断言漂移即实现有误；差异点 =
// cwd 单引号按 PowerShell 规则转义为 ''（AQ-1 修复，buildResumeCommand 专用，
// buildRestoreInput 不含 cwd 不受影响）。

/** 构造最小 AgentHistorySession（缺省字段占位，cwd 由用例指定） */
function makeSession(partial: Partial<AgentHistorySession>): AgentHistorySession {
  return {
    sessionId: "abc",
    cwd: null,
    title: null,
    titleSource: "firstPrompt",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId: "claude",
    ...partial,
  };
}

describe("buildResumeCommand（MC-316 迁入 + AQ-1 cwd 单引号转义）", () => {
  it("有 cwd → `cd '<cwd>' && claude --resume <id>`（单引号路径）", () => {
    expect(
      buildResumeCommand(makeSession({ sessionId: "abc", cwd: "D:\\proj" })),
    ).toBe("cd 'D:\\proj' && claude --resume abc");
  });

  it("无 cwd（null）→ 仅 `claude --resume <id>`", () => {
    expect(buildResumeCommand(makeSession({ sessionId: "abc" }))).toBe(
      "claude --resume abc",
    );
  });

  it("cwd 无单引号：与迁出源 historyContextMenu.buildResumeCommand 模板逐字一致（含 sessionId 原样透传）", () => {
    const session = makeSession({ sessionId: "uuid-123", cwd: "C:/work/x" });
    // 与迁出源模板逐字比对：`cd '${cwd}' && claude --resume ${sessionId}`
    // AQ-1 修复后唯一差异点：cwd 单引号转义为 ''（此处 cwd 无单引号，与迁出源一致）
    expect(buildResumeCommand(session)).toBe(
      `cd '${session.cwd!.replace(/'/g, "''")}' && claude --resume ${session.sessionId}`,
    );
  });

  it("cwd 含单引号 → 按 PowerShell 规则转义为 ''（AQ-1 回归：C:\\Bob's Project）", () => {
    expect(
      buildResumeCommand(
        makeSession({ sessionId: "abc", cwd: "C:\\Bob's Project" }),
      ),
    ).toBe("cd 'C:\\Bob''s Project' && claude --resume abc");
  });
});

describe("buildRestoreInput（MC-315 迁入，输出与 restoreSession.ts 字面量逐字一致）", () => {
  it("普通恢复：`claude --resume <id>` + \\r 结尾（无 \\n）", () => {
    expect(
      buildRestoreInput(makeSession({ sessionId: "abc", cwd: "D:\\proj" }), {
        fork: false,
      }),
    ).toBe("claude --resume abc\r");
  });

  it("fork 恢复：追加 ` --fork-session`", () => {
    expect(
      buildRestoreInput(makeSession({ sessionId: "abc", cwd: "D:\\proj" }), {
        fork: true,
      }),
    ).toBe("claude --resume abc --fork-session\r");
  });

  it("\\r 结尾且不含 \\r\\n（照现状 pty.write 注入形态）", () => {
    const out = buildRestoreInput(makeSession({ sessionId: "abc" }), {
      fork: false,
    });
    expect(out.endsWith("\r")).toBe(true);
    expect(out.endsWith("\r\n")).toBe(false);
  });

  it("无 cwd 会话同样可注入（注入内容不依赖 cwd，仅 resume 命令）", () => {
    expect(
      buildRestoreInput(makeSession({ sessionId: "abc" }), { fork: false }),
    ).toBe("claude --resume abc\r");
  });
});
