// TerminalRegistry 单元测试——Map 操作 + 幂等性 + 生命周期 + agentSession 契约
//
// TerminalRegistry 是终端跨页面复用的核心基础设施。
// 测试覆盖 register/get/remove/has/_reset/setAgentSession/subscribe。
// agentSession 为可选字段——stub 工厂不含该字段编译不炸（契约 1 设计目标）。

import { describe, it, expect, beforeEach } from "vitest";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal } from "../panels/terminal/TerminalRegistry";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";

/** 构造满足 RegisteredTerminal 接口的最小 stub（不含 agentSession——可选字段编译不炸验证） */
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

  // ── agentSession 可选字段编译不炸验证 ──

  it("stub 工厂不含 agentSession 字段也能 register → get 往返（可选字段编译不炸）", () => {
    const entry = makeEntry({ sessionId: "sid-optional" });
    TerminalRegistry.register("panel-opt", entry);
    const retrieved = TerminalRegistry.get("panel-opt")!;
    expect(retrieved.sessionId).toBe("sid-optional");
    // agentSession 为 undefined（未设置默认值）
    expect(retrieved.agentSession).toBeUndefined();
  });
});

// ═══════════════════════════════════════════════════════════════
// setAgentSession 契约测试
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.setAgentSession", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("部分键更新——matchedCommand 保留 usageSourcePath，反之亦然", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);

    // 首次设置 matchedCommand
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });
    let got = TerminalRegistry.get("p1")!;
    expect(got.agentSession?.matchedCommand).toBe("claude");
    expect(got.agentSession?.usageSourcePath).toBeUndefined();
    expect(got.agentSession?.lastEventAt).toBeGreaterThan(0);

    // 后续只更新 usageSourcePath——matchedCommand 应保留（merge）
    TerminalRegistry.setAgentSession("p1", { usageSourcePath: "/t.json" });
    got = TerminalRegistry.get("p1")!;
    expect(got.agentSession?.matchedCommand).toBe("claude");
    expect(got.agentSession?.usageSourcePath).toBe("/t.json");
  });

  it("null 清空 agentSession", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });
    expect(TerminalRegistry.get("p1")!.agentSession).not.toBeNull();

    TerminalRegistry.setAgentSession("p1", null);
    expect(TerminalRegistry.get("p1")!.agentSession).toBeNull();
  });

  it("panelId 不存在 → no-op，不抛异常", () => {
    expect(() => {
      TerminalRegistry.setAgentSession("nonexistent", { matchedCommand: "claude" });
    }).not.toThrow();
    expect(TerminalRegistry.get("nonexistent")).toBeUndefined();
  });

  it("缺 lastEventAt 自动填充 Date.now()", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    const before = Date.now();
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });
    const after = Date.now();
    const ts = TerminalRegistry.get("p1")!.agentSession!.lastEventAt;
    expect(ts).toBeGreaterThanOrEqual(before);
    expect(ts).toBeLessThanOrEqual(after);
  });

  it("undefined 键不覆盖旧值", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude", usageSourcePath: "/orig.json" });

    // 传 usageSourcePath=undefined——不应覆盖旧值
    TerminalRegistry.setAgentSession("p1", { matchedCommand: undefined, usageSourcePath: undefined });

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.matchedCommand).toBe("claude");    // 保留旧值
    expect(got.usageSourcePath).toBe("/orig.json"); // 保留旧值
  });

  it("sessionId/status 存储与透传（hook 事件写入）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", {
      sessionId: "abc-123",
      status: "working",
    });

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.sessionId).toBe("abc-123");
    expect(got.status).toBe("working");
  });

  it("sessionId/status undefined 不覆盖旧值（merge 语义）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", {
      sessionId: "abc-123",
      status: "working",
    });

    // matchedCommand-only 更新（sessionId/status undefined）→ 旧值保留
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.sessionId).toBe("abc-123");
    expect(got.status).toBe("working");
    expect(got.matchedCommand).toBe("claude");
  });

  it("status 显式 null 清空（Notification 普通类型 → 无有效状态）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { status: "working" });

    TerminalRegistry.setAgentSession("p1", { status: null });

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.status).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════
// register 幂等覆盖保留旧 agentSession
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.register 幂等覆盖保留旧 agentSession", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("register 幂等覆盖且不传 agentSession → 保留旧值（StrictMode/重试场景不丢 session）", () => {
    const entry1 = makeEntry();
    TerminalRegistry.register("p1", entry1);
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });

    // 再次 register 不含 agentSession → 旧 session 应保留
    const entry2 = makeEntry({ sessionId: "new-sid" });
    TerminalRegistry.register("p1", entry2);

    const got = TerminalRegistry.get("p1")!;
    expect(got.sessionId).toBe("new-sid");          // 幂等覆盖
    expect(got.agentSession?.matchedCommand).toBe("claude"); // 旧 session 保留
  });

  it("register 显式传 agentSession → 取新值（不保留旧值）", () => {
    const entry1 = makeEntry();
    TerminalRegistry.register("p1", entry1);
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });

    // 再次 register 显式含 agentSession: null → 覆盖
    const entry2 = { ...makeEntry({ sessionId: "new-sid" }), agentSession: null } as RegisteredTerminal;
    TerminalRegistry.register("p1", entry2);

    const got = TerminalRegistry.get("p1")!;
    expect(got.sessionId).toBe("new-sid");
    expect(got.agentSession).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════
// getAll / _size / _dump（调试/测试接口，TRM-08）
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.getAll/_size/_dump", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("getAll 返回已注册条目的只读副本（修改副本不影响内部注册表）", () => {
    TerminalRegistry.register("a", makeEntry({ sessionId: "sid-a" }));
    TerminalRegistry.register("b", makeEntry({ sessionId: "sid-b" }));
    const all = TerminalRegistry.getAll();
    expect(all.size).toBe(2);
    expect(all.get("a")!.sessionId).toBe("sid-a");
    expect(all.get("b")!.sessionId).toBe("sid-b");
    // 只读视图：清空副本后内部注册表不受影响
    (all as Map<string, RegisteredTerminal>).clear();
    expect(TerminalRegistry.get("a")).toBeDefined();
    expect(TerminalRegistry.get("b")).toBeDefined();
    expect(TerminalRegistry._size()).toBe(2);
  });

  it("getAll 空注册表返回空副本", () => {
    expect(TerminalRegistry.getAll().size).toBe(0);
  });

  it("_size 反映当前注册条目数（register/remove 联动）", () => {
    expect(TerminalRegistry._size()).toBe(0);
    TerminalRegistry.register("a", makeEntry());
    TerminalRegistry.register("b", makeEntry());
    expect(TerminalRegistry._size()).toBe(2);
    TerminalRegistry.remove("a");
    expect(TerminalRegistry._size()).toBe(1);
  });

  it("_dump 返回全部注册 panelId 数组", () => {
    TerminalRegistry.register("a", makeEntry());
    TerminalRegistry.register("b", makeEntry());
    const keys = TerminalRegistry._dump();
    expect(keys).toEqual(expect.arrayContaining(["a", "b"]));
    expect(keys).toHaveLength(2);
  });
});

// ═══════════════════════════════════════════════════════════════
// setAgentSession merge 语义补充（NAH-02）
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.setAgentSession merge 语义（NAH-02）", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("全量 set 后增量 { status: 'working' } → 其余字段保留 + lastEventAt 更新", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    // 全量设置（含显式 lastEventAt）
    TerminalRegistry.setAgentSession("p1", {
      sessionId: "abc-123",
      usageSourcePath: "/t.json",
      matchedCommand: "claude",
      status: "done",
      lastEventAt: 1111,
    });
    // 增量仅更新 status——其余字段 merge 保留
    const before = Date.now();
    TerminalRegistry.setAgentSession("p1", { status: "working" });
    const after = Date.now();

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.sessionId).toBe("abc-123");
    expect(got.usageSourcePath).toBe("/t.json");
    expect(got.matchedCommand).toBe("claude");
    expect(got.status).toBe("working");
    // 缺 lastEventAt → 自动填新 Date.now()，替换旧显式值
    expect(got.lastEventAt).not.toBe(1111);
    expect(got.lastEventAt).toBeGreaterThanOrEqual(before);
    expect(got.lastEventAt).toBeLessThanOrEqual(after);
  });

  it("null 清空后增量 patch 不复活旧值（prev=null → undefined 键不回填）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", {
      sessionId: "abc-123",
      usageSourcePath: "/t.json",
    });
    TerminalRegistry.setAgentSession("p1", null);
    expect(TerminalRegistry.get("p1")!.agentSession).toBeNull();

    // null 清空后再增量 patch——prev 为 null，旧 sessionId/usageSourcePath 不回填
    TerminalRegistry.setAgentSession("p1", { status: "working" });
    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.status).toBe("working");
    expect(got.sessionId).toBeUndefined();
    expect(got.usageSourcePath).toBeUndefined();
    expect(got.matchedCommand).toBeUndefined();
    expect(got.cliId).toBeUndefined();
  });
});

// ═════════════════════════════════════════════════════════════
// setAgentSession cliId 字段（MC-402/107：OSC 133 C 命中写入 cliId）
// ═════════════════════════════════════════════════════════════

describe("TerminalRegistry.setAgentSession cliId 字段（MC-402）", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("显式传 cliId → 存储（OSC 133 C 命中形态）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", {
      cliId: "claude",
      matchedCommand: "claude",
    });
    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.cliId).toBe("claude");
    expect(got.matchedCommand).toBe("claude");
  });

  it("cliId undefined 不覆盖旧值（merge——hook 事件路径不传 cliId，保留 OSC 133 C 写入值）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { cliId: "codex", matchedCommand: "codex" });

    // hook 事件路径 patch 不含 cliId → 旧 cliId 保留
    TerminalRegistry.setAgentSession("p1", {
      sessionId: "abc-123",
      status: "working",
    });

    const got = TerminalRegistry.get("p1")!.agentSession!;
    expect(got.cliId).toBe("codex");
    expect(got.sessionId).toBe("abc-123");
  });

  it("首次 setAgentSession 未带 cliId → cliId 为 undefined（缺省回退由消费方 CLAUDE_CLI_ID 兜底）", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { sessionId: "abc-123", status: "working" });
    expect(TerminalRegistry.get("p1")!.agentSession!.cliId).toBeUndefined();
  });

  it("null 清空后再增量 patch → cliId 不回填", () => {
    const entry = makeEntry();
    TerminalRegistry.register("p1", entry);
    TerminalRegistry.setAgentSession("p1", { cliId: "claude" });
    TerminalRegistry.setAgentSession("p1", null);
    TerminalRegistry.setAgentSession("p1", { status: "working" });
    expect(TerminalRegistry.get("p1")!.agentSession!.cliId).toBeUndefined();
  });
});
