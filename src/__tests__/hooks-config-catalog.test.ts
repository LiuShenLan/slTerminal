// hooks-config-catalog.test.ts — eventsCatalog 常量守卫（P3-TE-19）
//
// 覆盖：30 事件齐全唯一、10 分组齐全、handler 支持矩阵与 C13-4 一致
// （三档抽查 + 全量断言）、10 个无 matcher 事件标记、5 种 handler
// 字段矩阵与 C13-3 一致。

import { describe, it, expect } from "vitest";
import {
  HOOK_EVENTS,
  EVENT_GROUPS,
  HANDLER_FIELD_MATRIX,
  HANDLER_COMMON_FIELDS,
  HANDLER_TYPES_BY_LEVEL,
  getEventMeta,
  getSupportedHandlerTypes,
  isMatcherSupported,
  getEventsByGroup,
  getGroups,
  RESTRICTED_MATCHER_CHARSET_EVENTS,
} from "../features/cliProfiles/profiles/claude/configEditor/eventsCatalog";

describe("事件目录完整性", () => {
  it("30 事件齐全且唯一", () => {
    expect(HOOK_EVENTS).toHaveLength(30);
    const names = HOOK_EVENTS.map((m) => m.event);
    expect(new Set(names).size).toBe(30);
  });

  it("10 分组齐全（顺序与目录表一致）", () => {
    expect(EVENT_GROUPS).toEqual([
      "会话生命周期",
      "用户交互",
      "工具调用",
      "通知与消息",
      "子代理与任务",
      "上下文管理",
      "停止与错误",
      "配置与文件变更",
      "工作树",
      "启发式交互",
    ]);
    expect(getGroups()).toEqual(EVENT_GROUPS);
  });

  it("每个分组事件数正确（3/2/6/2/5/2/2/4/2/2）", () => {
    const expected: Record<string, number> = {
      会话生命周期: 3,
      用户交互: 2,
      工具调用: 6,
      通知与消息: 2,
      子代理与任务: 5,
      上下文管理: 2,
      停止与错误: 2,
      配置与文件变更: 4,
      工作树: 2,
      启发式交互: 2,
    };
    for (const group of EVENT_GROUPS) {
      expect(getEventsByGroup(group)).toHaveLength(expected[group]);
    }
  });

  it("查询函数命中/未命中", () => {
    expect(getEventMeta("PreToolUse")?.group).toBe("工具调用");
    expect(getEventMeta("UnknownEvent")).toBeUndefined();
    expect(isMatcherSupported("UnknownEvent")).toBe(false);
  });
});

describe("matcher 支持标记（C13-5 配套）", () => {
  it("10 个无 matcher 事件标记正确", () => {
    const noMatcherEvents = [
      "UserPromptSubmit",
      "PostToolBatch",
      "MessageDisplay",
      "TaskCreated",
      "TaskCompleted",
      "TeammateIdle",
      "Stop",
      "CwdChanged",
      "WorktreeCreate",
      "WorktreeRemove",
    ];
    for (const event of noMatcherEvents) {
      expect(isMatcherSupported(event)).toBe(false);
      expect(getEventMeta(event)?.matcherTarget).toBeNull();
    }
  });

  it("其余 20 事件支持 matcher 且带匹配目标", () => {
    const supported = HOOK_EVENTS.filter((m) => m.supportsMatcher);
    expect(supported).toHaveLength(20);
    for (const meta of supported) {
      expect(meta.matcherTarget).toBeTruthy();
    }
  });

  it("matcher 匹配目标抽查（对照目录表）", () => {
    expect(getEventMeta("PreToolUse")?.matcherTarget).toBe("工具名");
    expect(getEventMeta("SessionStart")?.matcherTarget).toBe(
      "source（startup/resume/clear/compact）",
    );
    expect(getEventMeta("Notification")?.matcherTarget).toBe("notification_type");
    expect(getEventMeta("PreCompact")?.matcherTarget).toBe("manual/auto");
    expect(getEventMeta("FileChanged")?.matcherTarget).toBe("文件名模式（basename）");
    expect(getEventMeta("Elicitation")?.matcherTarget).toBe("MCP 服务器名称");
    expect(getEventMeta("StopFailure")?.matcherTarget).toBe("错误类型");
    expect(getEventMeta("SubagentStop")?.matcherTarget).toBe("子代理类型名");
  });

  it("受限窄字符集事件仅 FileChanged/StopFailure", () => {
    expect([...RESTRICTED_MATCHER_CHARSET_EVENTS].sort()).toEqual([
      "FileChanged",
      "StopFailure",
    ]);
  });
});

describe("handler 支持矩阵（C13-4 全量断言 + 抽查）", () => {
  // C13-4：A = 全 5 种（13 事件）；B = command+http+mcp_tool
  //（14 官方事件 + MessageDisplay 保守 B* 推断 = 15）；C = command+mcp_tool（2）
  const A_EVENTS = [
    "UserPromptSubmit",
    "UserPromptExpansion",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "TeammateIdle",
    "Stop",
  ];
  const B_EVENTS = [
    "SessionEnd",
    "Notification",
    "MessageDisplay",
    "SubagentStart",
    "PreCompact",
    "PostCompact",
    "StopFailure",
    "ConfigChange",
    "CwdChanged",
    "FileChanged",
    "InstructionsLoaded",
    "WorktreeCreate",
    "WorktreeRemove",
    "Elicitation",
    "ElicitationResult",
  ];
  const C_EVENTS = ["SessionStart", "Setup"];

  it("全量断言：每事件 handler 档与 C13-4 一致", () => {
    for (const event of A_EVENTS) {
      expect(getEventMeta(event)?.handlerLevel, event).toBe("A");
    }
    for (const event of B_EVENTS) {
      expect(getEventMeta(event)?.handlerLevel, event).toBe("B");
    }
    for (const event of C_EVENTS) {
      expect(getEventMeta(event)?.handlerLevel, event).toBe("C");
    }
  });

  it("30 事件全部被三档覆盖（无遗漏）", () => {
    const covered = new Set([...A_EVENTS, ...B_EVENTS, ...C_EVENTS]);
    expect(covered.size).toBe(30);
    for (const meta of HOOK_EVENTS) {
      expect(covered.has(meta.event)).toBe(true);
    }
  });

  it("handler 类型列表按档展开正确", () => {
    expect(HANDLER_TYPES_BY_LEVEL.A).toEqual([
      "command",
      "http",
      "mcp_tool",
      "prompt",
      "agent",
    ]);
    expect(HANDLER_TYPES_BY_LEVEL.B).toEqual(["command", "http", "mcp_tool"]);
    expect(HANDLER_TYPES_BY_LEVEL.C).toEqual(["command", "mcp_tool"]);
  });

  it("抽查：getSupportedHandlerTypes 与档一致", () => {
    // A 档（PreToolUse 全 5 种）
    expect(getSupportedHandlerTypes("PreToolUse")).toEqual(HANDLER_TYPES_BY_LEVEL.A);
    // B 档（Notification 无 prompt/agent）
    expect(getSupportedHandlerTypes("Notification")).toEqual(
      HANDLER_TYPES_BY_LEVEL.B,
    );
    // C 档（SessionStart 仅 command/mcp_tool）
    expect(getSupportedHandlerTypes("SessionStart")).toEqual(
      HANDLER_TYPES_BY_LEVEL.C,
    );
  });
});

describe("5 种 handler 字段矩阵（C13-3 官方版）", () => {
  it("command：command*/args/async/asyncRewake/shell", () => {
    const fields = HANDLER_FIELD_MATRIX.command.map((f) => f.key);
    expect(fields).toEqual(["command", "args", "async", "asyncRewake", "shell"]);
    expect(HANDLER_FIELD_MATRIX.command.find((f) => f.key === "command")?.required).toBe(true);
    expect(HANDLER_FIELD_MATRIX.command.filter((f) => f.required)).toHaveLength(1);
  });

  it("http：url*/headers/allowedEnvVars（无 method/body）", () => {
    const fields = HANDLER_FIELD_MATRIX.http.map((f) => f.key);
    expect(fields).toEqual(["url", "headers", "allowedEnvVars"]);
    expect(fields).not.toContain("method");
    expect(fields).not.toContain("body");
    expect(HANDLER_FIELD_MATRIX.http.find((f) => f.key === "url")?.required).toBe(true);
  });

  it("mcp_tool：server*/tool*/input（字段名是 input 非 args）", () => {
    const fields = HANDLER_FIELD_MATRIX.mcp_tool.map((f) => f.key);
    expect(fields).toEqual(["server", "tool", "input"]);
    expect(fields).not.toContain("args");
    const required = HANDLER_FIELD_MATRIX.mcp_tool.filter((f) => f.required).map((f) => f.key);
    expect(required).toEqual(["server", "tool"]);
  });

  it("prompt：prompt*/model/continueOnBlock", () => {
    const fields = HANDLER_FIELD_MATRIX.prompt.map((f) => f.key);
    expect(fields).toEqual(["prompt", "model", "continueOnBlock"]);
    expect(HANDLER_FIELD_MATRIX.prompt.find((f) => f.key === "prompt")?.required).toBe(true);
  });

  it("agent：prompt*/model（无 description/subagent_type）", () => {
    const fields = HANDLER_FIELD_MATRIX.agent.map((f) => f.key);
    expect(fields).toEqual(["prompt", "model"]);
    expect(fields).not.toContain("description");
    expect(fields).not.toContain("subagent_type");
    expect(HANDLER_FIELD_MATRIX.agent.find((f) => f.key === "prompt")?.required).toBe(true);
  });

  it("通用字段：if/timeout/statusMessage；once 不展示", () => {
    const common = HANDLER_COMMON_FIELDS.map((f) => f.key);
    expect(common).toEqual(["if", "timeout", "statusMessage"]);
    const allKeys = [
      ...HANDLER_FIELD_MATRIX.command.map((f) => f.key),
      ...HANDLER_FIELD_MATRIX.http.map((f) => f.key),
      ...HANDLER_FIELD_MATRIX.mcp_tool.map((f) => f.key),
      ...HANDLER_FIELD_MATRIX.prompt.map((f) => f.key),
      ...HANDLER_FIELD_MATRIX.agent.map((f) => f.key),
      ...common,
    ];
    expect(allKeys).not.toContain("once");
    expect(allKeys).not.toContain("asyncTimeout");
  });

  it("全部通用字段非必填", () => {
    for (const f of HANDLER_COMMON_FIELDS) {
      expect(f.required).toBe(false);
    }
  });
});
