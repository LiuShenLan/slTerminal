// hooks-config-model.test.ts — configModel 双向转换（P3-TE-06）
//
// 覆盖：空配置、单事件单 matcher 单 handler、多事件多 handler、
// 字段缺失/多余容错、不支持 matcher 事件省略 matcher 键、isSltermManaged 判定。

import { describe, it, expect } from "vitest";
import {
  jsonToGui,
  guiToJson,
  isSltermManaged,
  type HooksConfigGui,
} from "../panels/hooksConfig/configModel";

describe("jsonToGui：空配置与非法输入降级", () => {
  it("null / undefined / 非对象 / 数组 → 空模型，不抛错", () => {
    expect(jsonToGui(null)).toEqual({ events: [] });
    expect(jsonToGui(undefined)).toEqual({ events: [] });
    expect(jsonToGui("str")).toEqual({ events: [] });
    expect(jsonToGui(42)).toEqual({ events: [] });
    expect(jsonToGui([])).toEqual({ events: [] });
  });

  it("空对象 → 空事件列表", () => {
    expect(jsonToGui({})).toEqual({ events: [] });
  });

  it("事件值为非数组 → 容错跳过该事件", () => {
    const gui = jsonToGui({ PreToolUse: { matcher: "Bash", hooks: [] } });
    expect(gui.events).toEqual([]);
  });
});

describe("jsonToGui：字段缺失/多余容错", () => {
  it("matcher 缺失 → 默认空串；hooks 缺失 → 空 handler 列表", () => {
    const gui = jsonToGui({ PreToolUse: [{ hooks: [] }] });
    expect(gui.events[0].matcherGroups[0]).toEqual({
      matcher: "",
      handlers: [],
    });
  });

  it("handler type 缺失/非法 → 跳过该 handler", () => {
    const gui = jsonToGui({
      PreToolUse: [
        {
          hooks: [
            { command: "no-type" },
            { type: "weird", command: "x" },
            { type: "command", command: "ok" },
          ],
        },
      ],
    });
    expect(gui.events[0].matcherGroups[0].handlers).toEqual([
      { type: "command", command: "ok" },
    ]);
  });

  it("matcher 组非对象 / handler 非对象 → 容错跳过", () => {
    const gui = jsonToGui({
      PreToolUse: ["not-object", { matcher: "Bash", hooks: [null, 42, { type: "command" }] }],
    });
    const groups = gui.events[0].matcherGroups;
    expect(groups).toHaveLength(1);
    expect(groups[0].matcher).toBe("Bash");
    expect(groups[0].handlers).toHaveLength(1);
  });

  it("未知字段保留到 extraFields，round-trip 不丢数据", () => {
    const json = {
      PreToolUse: [
        { matcher: "Bash", hooks: [{ type: "command", command: "c", customField: 1 }] },
      ],
    };
    const gui = jsonToGui(json);
    expect(gui.events[0].matcherGroups[0].handlers[0].extraFields).toEqual({
      customField: 1,
    });
    // round-trip：guiToJson 写回 extraFields
    expect(guiToJson(gui)).toEqual(json);
  });

  it("未知事件名保留（归未知事件组），round-trip 不丢事件", () => {
    const json = { SomeFutureEvent: [{ hooks: [{ type: "command", command: "c" }] }] };
    const gui = jsonToGui(json);
    expect(gui.events[0].group).toBe("未知事件");
    expect(guiToJson(gui)).toEqual(json);
  });
});

describe("双向转换 round-trip", () => {
  it("单事件单 matcher 单 handler", () => {
    const json = {
      PreToolUse: [
        { matcher: "Bash|Read", hooks: [{ type: "command", command: "echo hi", timeout: 5 }] },
      ],
    };
    const gui = jsonToGui(json);
    expect(gui.events[0]).toMatchObject({
      event: "PreToolUse",
      group: "工具调用",
    });
    expect(gui.events[0].matcherGroups[0]).toMatchObject({
      matcher: "Bash|Read",
    });
    expect(guiToJson(gui)).toEqual(json);
  });

  it("多事件多 handler（含 B/C 档事件与多 matcher 组）", () => {
    const json: Record<string, unknown[]> = {
      SessionStart: [
        {
          matcher: "startup",
          hooks: [
            { type: "command", command: "a", async: true },
            { type: "mcp_tool", server: "s", tool: "t", input: { k: 1 } },
          ],
        },
      ],
      Notification: [
        {
          matcher: "permission_prompt",
          hooks: [{ type: "http", url: "http://x", headers: { "X-A": "1" } }],
        },
      ],
      UserPromptSubmit: [
        { hooks: [{ type: "prompt", prompt: "p", model: "m" }] },
      ],
    };
    const gui = jsonToGui(json);
    expect(gui.events.map((e) => e.event)).toEqual([
      "SessionStart",
      "Notification",
      "UserPromptSubmit",
    ]);
    expect(guiToJson(gui)).toEqual(json);
  });

  it("undefined 字段在序列化时省略", () => {
    const json = { PreToolUse: [{ hooks: [{ type: "command", command: "c" }] }] };
    const gui = jsonToGui(json);
    // 手工补一个 undefined 字段后写回，不应出现该键
    const handler = gui.events[0].matcherGroups[0].handlers[0];
    (handler as unknown as Record<string, unknown>).shell = undefined;
    expect(guiToJson(gui)).toEqual(json);
  });
});

describe("不支持 matcher 的事件省略 matcher 键", () => {
  it("guiToJson 省略 matcher 键但保留数组包裹", () => {
    const gui: HooksConfigGui = {
      events: [
        {
          event: "UserPromptSubmit",
          group: "用户交互",
          matcherGroups: [
            { matcher: "ignored", handlers: [{ type: "command", command: "c" }] },
          ],
        },
      ],
    };
    expect(guiToJson(gui)).toEqual({
      UserPromptSubmit: [{ hooks: [{ type: "command", command: "c" }] }],
    });
  });

  it("10 个无 matcher 事件全部省略 matcher 键", () => {
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
      const gui: HooksConfigGui = {
        events: [
          { event, group: "x", matcherGroups: [{ matcher: "m", handlers: [] }] },
        ],
      };
      expect(guiToJson(gui)[event]).toEqual([{ hooks: [] }]);
    }
  });

  it("支持 matcher 的事件空 matcher 同样省略该键（等价全匹配）", () => {
    const gui: HooksConfigGui = {
      events: [
        {
          event: "PreToolUse",
          group: "工具调用",
          matcherGroups: [{ matcher: "", handlers: [{ type: "command", command: "c" }] }],
        },
      ],
    };
    expect(guiToJson(gui)).toEqual({
      PreToolUse: [{ hooks: [{ type: "command", command: "c" }] }],
    });
  });
});

describe("isSltermManaged", () => {
  it("command 含 slterm-hook-reporter 子串 → true", () => {
    expect(
      isSltermManaged({ type: "command", command: "node \"C:\\Users\\x\\.slterminal\\hooks\\slterm-hook-reporter.js\"" }),
    ).toBe(true);
    expect(
      isSltermManaged({ type: "command", command: "slterm-hook-reporter" }),
    ).toBe(true);
  });

  it("其他 command → false", () => {
    expect(isSltermManaged({ type: "command", command: "echo hi" })).toBe(false);
    expect(isSltermManaged({ type: "command", command: "slterm-hook" })).toBe(false);
  });

  it("非 command 型 handler / 缺 command / 非对象 → false", () => {
    expect(isSltermManaged({ type: "http", url: "http://x" })).toBe(false);
    expect(isSltermManaged({ type: "command" })).toBe(false);
    expect(isSltermManaged(null)).toBe(false);
    expect(isSltermManaged("str")).toBe(false);
  });
});

