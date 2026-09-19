// L2 TerminalRegistry promptReady 事件测试——恢复注入闸门信号源
// 覆盖：置位通知 / 幂等 / 早到 A pending 补发（竞态加固防复发）/ remove 清 pending / _reset
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal, RegistryEvent } from "../panels/terminal/TerminalRegistry";

/** 构造 stub 条目（字段照生产 register 形态） */
function stubEntry(): RegisteredTerminal {
  return {
    term: {} as unknown as RegisteredTerminal["term"],
    sessionId: "stub-session",
    webglAddon: null,
    fitAddon: {} as unknown as RegisteredTerminal["fitAddon"],
    shellKind: "pwsh",
    promptReady: false,
  };
}

/** 收集事件流 */
function collectEvents(): { events: RegistryEvent[]; unsubscribe: () => void } {
  const events: RegistryEvent[] = [];
  const unsubscribe = TerminalRegistry.subscribe((e) => events.push(e));
  return { events, unsubscribe };
}

beforeEach(() => {
  TerminalRegistry._reset();
});

afterEach(() => {
  TerminalRegistry._reset();
  vi.restoreAllMocks();
});

describe("TerminalRegistry promptReady 事件", () => {
  it("markPromptReady 置位 → 订阅者收到一次 promptReady 事件", () => {
    TerminalRegistry.register("p1", stubEntry());
    const { events, unsubscribe } = collectEvents();

    TerminalRegistry.markPromptReady("p1");

    expect(events).toEqual([{ type: "promptReady", panelId: "p1" }]);
    expect(TerminalRegistry.get("p1")?.promptReady).toBe(true);
    unsubscribe();
  });

  it("重复 markPromptReady → 幂等，不重复 notify", () => {
    TerminalRegistry.register("p2", stubEntry());
    TerminalRegistry.markPromptReady("p2");
    const { events, unsubscribe } = collectEvents();

    TerminalRegistry.markPromptReady("p2");
    TerminalRegistry.markPromptReady("p2");

    expect(events).toEqual([]);
    unsubscribe();
  });

  it("早到 A（未注册 panelId）→ 入 pending 不 notify；register 时落库并补发事件（竞态防复发）", () => {
    const { events, unsubscribe } = collectEvents();

    // reader 线程先于 spawn resolve 启动：133;A 可早于 register 到达
    TerminalRegistry.markPromptReady("p3");
    expect(events).toEqual([]); // 未注册不建条目不通知

    TerminalRegistry.register("p3", stubEntry());
    const entry = TerminalRegistry.get("p3");
    expect(entry?.promptReady).toBe(true); // pending 落库
    expect(events).toEqual([
      { type: "register", panelId: "p3" },
      { type: "promptReady", panelId: "p3" }, // 补发——闸门订阅者不错过
    ]);
    unsubscribe();
  });

  it("remove 清 pending：pending 后 remove 再 register → 不复活就绪态", () => {
    TerminalRegistry.markPromptReady("p4"); // 入 pending
    TerminalRegistry.register("p4", stubEntry()); // pending 落库（就绪）
    TerminalRegistry.remove("p4");
    // 再次 markPromptReady + register：新一轮 pending 独立语义
    TerminalRegistry.remove("p4"); // 幂等
    const { events, unsubscribe } = collectEvents();
    TerminalRegistry.register("p4", stubEntry());
    expect(TerminalRegistry.get("p4")?.promptReady).toBe(false);
    expect(events).toEqual([{ type: "register", panelId: "p4" }]);
    unsubscribe();
  });

  it("remove 清 pending：pending 未注册即 remove → 迟到 register 不补发", () => {
    TerminalRegistry.markPromptReady("p5"); // 入 pending（终端未注册）
    TerminalRegistry.remove("p5"); // 卸载清 pending
    const { events, unsubscribe } = collectEvents();
    TerminalRegistry.register("p5", stubEntry());
    expect(TerminalRegistry.get("p5")?.promptReady).toBe(false);
    expect(events).toEqual([{ type: "register", panelId: "p5" }]);
    unsubscribe();
  });

  it("_reset 清空 listeners + pending", () => {
    TerminalRegistry.markPromptReady("p6"); // pending
    const { events } = collectEvents();
    TerminalRegistry._reset();

    TerminalRegistry.register("p6", stubEntry());
    TerminalRegistry.markPromptReady("p6");
    // 旧 listener 已清（零事件）；pending 已清（register 不落库就绪）
    expect(events).toEqual([]);
    expect(TerminalRegistry.get("p6")?.promptReady).toBe(true); // 本次正常置位
  });
});
