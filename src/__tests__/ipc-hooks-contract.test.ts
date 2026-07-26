// ipc-hooks-contract.test.ts — hooks IPC wrapper 合约测试
//
// 使用 mockIPC 拦截真实的 invoke 调用，验证每个 hooks IPC wrapper：
// 1. 命令名正确（snake_case）
// 2. 参数结构正确
// 3. 返回类型正确
// 4. 异常传播

import { describe, it, expect, afterEach, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

// mock @tauri-apps/api/event — onHookEvent 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
import * as hooks from "../ipc/hooks";

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// hooks_inject
// ═══════════════════════════════════════════════════════════════════

describe("hooks_inject 合约", () => {
  it("应调用 hooks_inject 命令（无参数）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_inject") return { status: "injected", version: 1 };
    });

    await hooks.inject();

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("hooks_inject");
    // 无参数命令：args 应为空对象
    expect(args).toEqual({});
  });

  it("应返回 injected 状态的 HookInjectionStatus", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_inject") return { status: "injected", version: 1 };
    });

    const result = await hooks.inject();

    expect(result).toEqual({ status: "injected", version: 1 });
    expect(result.status).toBe("injected");
    expect(result.version).toBe(1);
  });

  it("未注入时返回 notInjected 状态（version 为 null）", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_inject") return { status: "notInjected", version: null };
    });

    const result = await hooks.inject();

    expect(result).toEqual({ status: "notInjected", version: null });
  });

  it("版本过旧时返回 outdated 状态", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_inject") return { status: "outdated", version: 1 };
    });

    const result = await hooks.inject();

    expect(result.status).toBe("outdated");
    expect(result.version).toBe(1);
  });

  it("invoke 失败时异常应传播给调用方", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_inject") throw new Error("settings.json 语法错误");
    });

    await expect(hooks.inject()).rejects.toThrow("settings.json 语法错误");
  });
});

// ═══════════════════════════════════════════════════════════════════
// hooks_uninstall
// ═══════════════════════════════════════════════════════════════════

describe("hooks_uninstall 合约", () => {
  it("应调用 hooks_uninstall 命令（无参数）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await hooks.uninstall();

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("hooks_uninstall");
    expect(args).toEqual({});
  });

  it("正常返回 void", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_uninstall") return;
    });

    const result = await hooks.uninstall();
    expect(result).toBeUndefined();
  });

  it("invoke 失败时异常应传播", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_uninstall") throw new Error("删除脚本目录失败");
    });

    await expect(hooks.uninstall()).rejects.toThrow("删除脚本目录失败");
  });
});

// ═══════════════════════════════════════════════════════════════════
// hooks_injection_status
// ═══════════════════════════════════════════════════════════════════

describe("hooks_injection_status 合约", () => {
  it("应调用 hooks_injection_status 命令（无参数）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_injection_status") return { status: "injected", version: 1 };
    });

    await hooks.getInjectionStatus();

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("hooks_injection_status");
    expect(args).toEqual({});
  });

  it("应返回 HookInjectionStatus", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_injection_status") return { status: "injected", version: 1 };
    });

    const result = await hooks.getInjectionStatus();

    expect(result).toEqual({ status: "injected", version: 1 });
  });

  it("invoke 失败时异常应传播", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_injection_status") throw new Error("读取配置失败");
    });

    await expect(hooks.getInjectionStatus()).rejects.toThrow("读取配置失败");
  });
});

// ═══════════════════════════════════════════════════════════════════
// onHookEvent（事件订阅封装）
// ═══════════════════════════════════════════════════════════════════

describe("onHookEvent 合约", () => {
  it("应调用 listen 监听 hook-event 事件", () => {
    vi.mocked(listen).mockReturnValue(Promise.resolve(vi.fn()));

    const callback = vi.fn();
    hooks.onHookEvent(callback);

    expect(listen).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledWith("hook-event", expect.any(Function));
  });

  it("返回 unsubscribe 函数", () => {
    vi.mocked(listen).mockReturnValue(Promise.resolve(vi.fn()));

    const unsubscribe = hooks.onHookEvent(vi.fn());

    expect(typeof unsubscribe).toBe("function");
  });

  it("回调应接收 event.payload 传递给 callback", () => {
    const mockPayload: hooks.HookEventPayload = {
      panelId: "terminal-p1-0",
      event: "UserPromptSubmit",
      timestamp: 1700000000000,
      sessionId: "abc-123",
      transcriptPath: "/tmp/transcript.jsonl",
      cwd: "/home/user/project",
      toolName: null,
      notificationType: null,
    };

    let capturedHandler: ((event: { payload: hooks.HookEventPayload }) => void) | null =
      null;
    vi.mocked(listen).mockImplementation(
      (_event: string, handler: unknown) => {
        capturedHandler = handler as typeof capturedHandler;
        return Promise.resolve(vi.fn());
      },
    );

    const callback = vi.fn();
    hooks.onHookEvent(callback);

    // 模拟 Tauri 事件推送
    expect(capturedHandler).not.toBeNull();
    capturedHandler!({ payload: mockPayload });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith(mockPayload);
  });

  it("返回的 unsubscribe 应调用 listen 返回的清理函数", async () => {
    const mockUnlisten = vi.fn();
    vi.mocked(listen).mockReturnValue(Promise.resolve(mockUnlisten));

    const unsubscribe = hooks.onHookEvent(vi.fn());

    // 等待 listen Promise resolve
    await new Promise((resolve) => setTimeout(resolve, 0));

    unsubscribe();
    // .then() 微任务需 flush 后才执行 mockUnlisten
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });

  it("HookEventPayload 字段完整性验证（C1 契约 8 字段）", () => {
    const payload: hooks.HookEventPayload = {
      panelId: "terminal-p1-0",
      event: "PreToolUse",
      timestamp: 1700000000000,
      sessionId: "sess-001",
      transcriptPath: "/path/to/transcript.jsonl",
      cwd: "/project",
      toolName: "read",
      notificationType: null,
    };

    // 验证恰好 8 个字段（C1 契约）
    expect(Object.keys(payload).sort()).toEqual([
      "cwd",
      "event",
      "notificationType",
      "panelId",
      "sessionId",
      "timestamp",
      "toolName",
      "transcriptPath",
    ]);

    // toolName 可为非 null 值（工具事件）
    expect(payload.toolName).toBe("read");
    // notificationType 可为 null
    expect(payload.notificationType).toBeNull();
  });
});
