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
});
