// 终端注册表订阅通知测试
// 验证 TerminalRegistry.subscribe 的 register/remove 通知 + 退订

import { describe, it, expect, vi, beforeEach } from "vitest";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import type { RegisteredTerminal } from "../panels/terminal/TerminalRegistry";

/** 测试用 stub——仅用作 subscribe 通知 payload，不实际调用 */
function stubTerminal(): RegisteredTerminal {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return { term: {} as any, sessionId: "s1", webglAddon: null, fitAddon: {} as any };
}

beforeEach(() => {
  TerminalRegistry._reset();
});

describe("TerminalRegistry.subscribe", () => {
  it("register 时通知全部 listener（同步，Map 变更之后）", () => {
    const listener = vi.fn();
    TerminalRegistry.subscribe(listener);

    TerminalRegistry.register("terminal-p1-0", stubTerminal());

    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({ type: "register", panelId: "terminal-p1-0" });
    // Map 变更之后——listener 回调时 get 应命中
    expect(TerminalRegistry.get("terminal-p1-0")).toBeDefined();
  });

  it("remove 时通知全部 listener（同步，Map 变更之后）", () => {
    const listener = vi.fn();
    TerminalRegistry.register("terminal-p1-0", stubTerminal());
    TerminalRegistry.subscribe(listener);

    const result = TerminalRegistry.remove("terminal-p1-0");

    expect(result).toBe(true);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({ type: "remove", panelId: "terminal-p1-0" });
    // Map 变更之后——listener 回调时 Map 已删
    expect(TerminalRegistry.get("terminal-p1-0")).toBeUndefined();
  });

  it("退订后不再收到通知", () => {
    const listener = vi.fn();
    const unsubscribe = TerminalRegistry.subscribe(listener);
    unsubscribe();

    TerminalRegistry.register("terminal-p1-0", stubTerminal());
    TerminalRegistry.remove("terminal-p1-0");

    expect(listener).not.toHaveBeenCalled();
  });

  it("remove 不存在的 panelId → 不通知", () => {
    const listener = vi.fn();
    TerminalRegistry.subscribe(listener);

    TerminalRegistry.remove("nonexistent");

    expect(listener).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════
// sessionChange 事件测试（契约 1）
// ═══════════════════════════════════════════════════════════════

describe("TerminalRegistry.subscribe sessionChange", () => {
  beforeEach(() => {
    TerminalRegistry._reset();
  });

  it("setAgentSession（非 null）→ 通知 listener 收到 { type:'sessionChange', panelId } 裸结构", () => {
    TerminalRegistry.register("p1", stubTerminal());
    const listener = vi.fn();
    TerminalRegistry.subscribe(listener);

    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });

    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({
      type: "sessionChange",
      panelId: "p1",
    });
    // 契约 1：payload 不带 session 数据——listener 经 get() 读现值
    const callArg = listener.mock.calls[0][0];
    expect(callArg).not.toHaveProperty("agentSession");
    expect(callArg).not.toHaveProperty("session");
  });

  it("setAgentSession(null) → 通知 listener（清空也触发）", () => {
    TerminalRegistry.register("p1", stubTerminal());
    TerminalRegistry.setAgentSession("p1", { matchedCommand: "claude" });

    const listener = vi.fn();
    TerminalRegistry.subscribe(listener);

    TerminalRegistry.setAgentSession("p1", null);

    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({
      type: "sessionChange",
      panelId: "p1",
    });
  });

  it("setAgentSession 对不存在的 panelId → no-op 不通知", () => {
    const listener = vi.fn();
    TerminalRegistry.subscribe(listener);

    TerminalRegistry.setAgentSession("nonexistent", { matchedCommand: "claude" });

    expect(listener).not.toHaveBeenCalled();
  });
});
