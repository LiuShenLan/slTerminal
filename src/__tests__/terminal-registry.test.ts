// TerminalRegistry 单元测试——Map 操作 + 幂等性 + 生命周期 + claudeSession 契约
//
// TerminalRegistry 是终端跨页面复用的核心基础设施。
// 测试覆盖 register/get/remove/has/_reset/setClaudeSession/subscribe。
// claudeSession 为可选字段——stub 工厂不含该字段编译不炸（契约 1 设计目标）。

import { describe, it, expect, beforeEach } from "vitest";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal } from "../panels/terminal/TerminalRegistry";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";

/** 构造满足 RegisteredTerminal 接口的最小 stub（不含 claudeSession——可选字段编译不炸验证） */
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
    TerminalRegistry._reset();
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

  it("_reset 应清空所有已注册条目", () => {
    TerminalRegistry.register("a", makeEntry());
    TerminalRegistry.register("b", makeEntry());
    expect(TerminalRegistry.has("a")).toBe(true);
    expect(TerminalRegistry.has("b")).toBe(true);
    TerminalRegistry._reset();
    expect(TerminalRegistry.has("a")).toBe(false);
    expect(TerminalRegistry.has("b")).toBe(false);
  });

  // ── claudeSession 可选字段编译不炸验证 ──

  it("stub 工厂不含 claudeSession 字段也能 register → get 往返（可选字段编译不炸）", () => {
    const entry = makeEntry({ sessionId: "sid-optional" });
    TerminalRegistry.register("panel-opt", entry);
    const retrieved = TerminalRegistry.get("panel-opt")!;
    expect(retrieved.sessionId).toBe("sid-optional");
    // claudeSession 为 undefined（未设置默认值）
    expect(retrieved.claudeSession).toBeUndefined();
  });
});

// ═══════════════════════════════════════════════════════════════
// setClaudeSession 契约测试
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.setClaudeSession", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("部分键更新——matchedCommand 保留 transcriptPath，反之亦然", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);

    // 首次设置 matchedCommand
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude" });
    let got = TerminalRegistry.get("p1")!;
    expect(got.claudeSession?.matchedCommand).toBe("claude");
    expect(got.claudeSession?.transcriptPath).toBeUndefined();
    expect(got.claudeSession?.lastEventAt).toBeGreaterThan(0);

    // 后续只更新 transcriptPath——matchedCommand 应保留（merge）
    TerminalRegistry.setClaudeSession("p1", { transcriptPath: "/t.json" });
    got = TerminalRegistry.get("p1")!;
    expect(got.claudeSession?.matchedCommand).toBe("claude");
    expect(got.claudeSession?.transcriptPath).toBe("/t.json");
  });

  it("null 清空 claudeSession", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude" });
    expect(TerminalRegistry.get("p1")!.claudeSession).not.toBeNull();

    TerminalRegistry.setClaudeSession("p1", null);
    expect(TerminalRegistry.get("p1")!.claudeSession).toBeNull();
  });

  it("panelId 不存在 → no-op，不抛异常", () => {
    expect(() => {
      TerminalRegistry.setClaudeSession("nonexistent", { matchedCommand: "claude" });
    }).not.toThrow();
    expect(TerminalRegistry.get("nonexistent")).toBeUndefined();
  });

  it("缺 lastEventAt 自动填充 Date.now()", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    const before = Date.now();
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude" });
    const after = Date.now();
    const ts = TerminalRegistry.get("p1")!.claudeSession!.lastEventAt;
    expect(ts).toBeGreaterThanOrEqual(before);
    expect(ts).toBeLessThanOrEqual(after);
  });

  it("undefined 键不覆盖旧值", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude", transcriptPath: "/orig.json" });

    // 传 transcriptPath=undefined——不应覆盖旧值
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: undefined, transcriptPath: undefined });

    const got = TerminalRegistry.get("p1")!.claudeSession!;
    expect(got.matchedCommand).toBe("claude");    // 保留旧值
    expect(got.transcriptPath).toBe("/orig.json"); // 保留旧值
  });
});

// ═══════════════════════════════════════════════════════════════
// register 幂等覆盖保留旧 claudeSession
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.register 幂等覆盖保留旧 claudeSession", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("register 幂等覆盖且不传 claudeSession → 保留旧值（StrictMode/重试场景不丢 session）", () => {
    const entry1 = makeEntry();
    TerminalRegistry.register("p1", entry1);
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude" });

    // 再次 register 不含 claudeSession → 旧 session 应保留
    const entry2 = makeEntry({ sessionId: "new-sid" });
    TerminalRegistry.register("p1", entry2);

    const got = TerminalRegistry.get("p1")!;
    expect(got.sessionId).toBe("new-sid");          // 幂等覆盖
    expect(got.claudeSession?.matchedCommand).toBe("claude"); // 旧 session 保留
  });

  it("register 显式传 claudeSession → 取新值（不保留旧值）", () => {
    const entry1 = makeEntry();
    TerminalRegistry.register("p1", entry1);
    TerminalRegistry.setClaudeSession("p1", { matchedCommand: "claude" });

    // 再次 register 显式含 claudeSession: null → 覆盖
    const entry2 = { ...makeEntry({ sessionId: "new-sid" }), claudeSession: null } as RegisteredTerminal;
    TerminalRegistry.register("p1", entry2);

    const got = TerminalRegistry.get("p1")!;
    expect(got.sessionId).toBe("new-sid");
    expect(got.claudeSession).toBeNull();
  });
});
