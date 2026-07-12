// TerminalRegistry 单元测试——Map 操作 + 幂等性 + 生命周期
//
// TerminalRegistry 是终端跨页面复用的核心基础设施。
// 测试覆盖 register/get/remove/has/_clear 七个场景。

import { describe, it, expect, beforeEach } from "vitest";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal } from "../panels/terminal/TerminalRegistry";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";

/** 构造满足 RegisteredTerminal 接口的最小 stub */
function makeEntry(overrides?: {
  term?: Terminal;
  sessionId?: string;
  webglAddon?: WebglAddon | null;
  fitAddon?: FitAddon;
}): RegisteredTerminal {
  return {
    term: { dispose: () => {} } as unknown as Terminal,
    sessionId: "test-session",
    webglAddon: null,
    fitAddon: { dispose: () => {}, fit: () => {} } as unknown as FitAddon,
    ...overrides,
  };
}

describe("TerminalRegistry", () => {
  beforeEach(() => {
    TerminalRegistry._clear();
  });

  it("register + get 往返：注册后应能取回相同 entry", () => {
    const entry = makeEntry({ sessionId: "sid-001" });
    TerminalRegistry.register("panel-1", entry);
    const retrieved = TerminalRegistry.get("panel-1");
    expect(retrieved).toBeDefined();
    expect(retrieved!.sessionId).toBe("sid-001");
    expect(retrieved!.term).toBe(entry.term);
  });

  it("get 不存在的 key → undefined", () => {
    expect(TerminalRegistry.get("nonexistent")).toBeUndefined();
  });

  it("remove 存在的 key → 返回 true，随后 get 返回 undefined", () => {
    const entry = makeEntry();
    TerminalRegistry.register("panel-2", entry);
    expect(TerminalRegistry.remove("panel-2")).toBe(true);
    expect(TerminalRegistry.get("panel-2")).toBeUndefined();
  });

  it("remove 不存在的 key → 返回 false", () => {
    expect(TerminalRegistry.remove("nonexistent")).toBe(false);
  });

  it("has 应正确反映存在性", () => {
    expect(TerminalRegistry.has("panel-3")).toBe(false);
    TerminalRegistry.register("panel-3", makeEntry());
    expect(TerminalRegistry.has("panel-3")).toBe(true);
    TerminalRegistry.remove("panel-3");
    expect(TerminalRegistry.has("panel-3")).toBe(false);
  });

  it("幂等 register：同一 panelId 写入两次 → get 返回最新 entry", () => {
    const entry1 = makeEntry({ sessionId: "first" });
    const entry2 = makeEntry({ sessionId: "second" });
    TerminalRegistry.register("panel-dup", entry1);
    TerminalRegistry.register("panel-dup", entry2);
    expect(TerminalRegistry.get("panel-dup")!.sessionId).toBe("second");
  });

  it("_clear 应清空所有已注册条目", () => {
    TerminalRegistry.register("a", makeEntry());
    TerminalRegistry.register("b", makeEntry());
    expect(TerminalRegistry.has("a")).toBe(true);
    expect(TerminalRegistry.has("b")).toBe(true);
    TerminalRegistry._clear();
    expect(TerminalRegistry.has("a")).toBe(false);
    expect(TerminalRegistry.has("b")).toBe(false);
  });
});
