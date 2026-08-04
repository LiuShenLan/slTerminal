// ipc-hooks-contract.test.ts — hooks IPC wrapper 合约测试（IHE-06 工厂化）
//
// 经共享工厂 describeIpcContract（helpers/ipc-contract.ts）声明式驱动
// 四命令（inject/uninstall/getInjectionStatus/contextUsage）× 四维
// （命令名 / 参数结构 / 正常返回 / 异常传播）；onHookEvent 为 listen 事件封装，
// 属"wrapper 行为契约"（IHE-01②）——手写模拟驱动断言，不走 invoke 工厂。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";

// 覆盖 setup.ts 全局 mock——导入原始 ../ipc/hooks 模块以测试真实 IPC 合约
vi.mock("../ipc/hooks", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/hooks")>();
});

// mock @tauri-apps/api/event — onHookEvent 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
import type { ContextUsage } from "../types/hooks";
import * as hooks from "../ipc/hooks";

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// hooks_inject
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("hooks_inject 合约", [
  {
    name: "应调用 hooks_inject 命令（无参数）",
    cmd: "hooks_inject",
    call: () => hooks.inject(),
    respond: { status: "injected", version: 1 },
    expectArgs: {},
  },
  {
    name: "应返回 injected 状态的 HookInjectionStatus",
    cmd: "hooks_inject",
    call: () => hooks.inject(),
    respond: { status: "injected", version: 1 },
    expectResult: { status: "injected", version: 1 },
  },
  {
    name: "未注入时返回 notInjected 状态（version 为 null）",
    cmd: "hooks_inject",
    call: () => hooks.inject(),
    respond: { status: "notInjected", version: null },
    expectResult: { status: "notInjected", version: null },
  },
  {
    name: "版本过旧时返回 outdated 状态",
    cmd: "hooks_inject",
    call: () => hooks.inject(),
    respond: { status: "outdated", version: 1 },
    expectResult: { status: "outdated", version: 1 },
  },
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "hooks_inject",
    call: () => hooks.inject(),
    mockThrow: "settings.json 语法错误",
    expectReject: "settings.json 语法错误",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// hooks_uninstall
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("hooks_uninstall 合约", [
  {
    name: "应调用 hooks_uninstall 命令（无参数）",
    cmd: "hooks_uninstall",
    call: () => hooks.uninstall(),
    respond: undefined,
    expectArgs: {},
  },
  {
    name: "正常返回 void",
    cmd: "hooks_uninstall",
    call: () => hooks.uninstall(),
    respond: undefined,
    expectUndefined: true,
  },
  {
    name: "invoke 失败时异常应传播",
    cmd: "hooks_uninstall",
    call: () => hooks.uninstall(),
    mockThrow: "删除脚本目录失败",
    expectReject: "删除脚本目录失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// hooks_injection_status
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("hooks_injection_status 合约", [
  {
    name: "应调用 hooks_injection_status 命令（无参数）",
    cmd: "hooks_injection_status",
    call: () => hooks.getInjectionStatus(),
    respond: { status: "injected", version: 1 },
    expectArgs: {},
  },
  {
    name: "应返回 HookInjectionStatus",
    cmd: "hooks_injection_status",
    call: () => hooks.getInjectionStatus(),
    respond: { status: "injected", version: 1 },
    expectResult: { status: "injected", version: 1 },
  },
  {
    name: "invoke 失败时异常应传播",
    cmd: "hooks_injection_status",
    call: () => hooks.getInjectionStatus(),
    mockThrow: "读取配置失败",
    expectReject: "读取配置失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// hooks_context_usage（token 用量查询）
// ═══════════════════════════════════════════════════════════════════

const mockUsage: ContextUsage = {
  inputTokens: 1500,
  outputTokens: 800,
  cacheReadInputTokens: 200,
  cacheCreationInputTokens: 100,
};

describeIpcContract("contextUsage 合约（hooks_context_usage）", [
  // 维度 1：命令名
  {
    name: "应调用 hooks_context_usage 命令",
    cmd: "hooks_context_usage",
    call: () => hooks.contextUsage("/path/to/transcript.jsonl"),
    respond: null,
  },
  // 维度 2：参数结构
  {
    name: "应传递 { transcriptPath } 参数",
    cmd: "hooks_context_usage",
    call: () => hooks.contextUsage("/tmp/transcript-abc.jsonl"),
    respond: null,
    expectArgs: { transcriptPath: "/tmp/transcript-abc.jsonl" },
    expectExactKeys: ["transcriptPath"],
  },
  // 维度 3：正常返回透传——ContextUsage 对象
  {
    name: "有 usage 数据时透传 ContextUsage 对象",
    cmd: "hooks_context_usage",
    call: () => hooks.contextUsage("/transcript.jsonl"),
    respond: { ...mockUsage },
    expectResult: mockUsage,
  },
  // 维度 3：正常返回透传——null（无 usage 数据）
  {
    name: "无 usage 数据时透传 null",
    cmd: "hooks_context_usage",
    call: () => hooks.contextUsage("/empty.jsonl"),
    respond: null,
    expectResult: null,
  },
  // 维度 4：异常传播
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "hooks_context_usage",
    call: () => hooks.contextUsage("/nonexistent.jsonl"),
    mockThrow: "transcript 文件不存在",
    expectReject: "transcript 文件不存在",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// onHookEvent（事件订阅封装）——wrapper 行为契约（IHE-01②）
//
// listen 封装的回调解包（event.payload）不在 mockIPC 层验证——Tauri 的
// listen 运行时解包由 L4 E2E 守卫。此处用模拟驱动断言 wrapper 自身的
// 解包逻辑：构造 { payload } 事件对象 → 断言 callback 收到解包后 payload。
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

  it("回调应接收 event.payload 传递给 callback（解包行为契约）", () => {
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

// DBG-4 守卫：ContextUsage 键集合精确匹配（四字段）
describe("ContextUsage 字段完整性（DBG-4 守卫）", () => {
  it("ContextUsage 恰好 4 字段（C12 契约）", () => {
    const usage: ContextUsage = {
      inputTokens: 1000,
      outputTokens: 500,
      cacheReadInputTokens: 200,
      cacheCreationInputTokens: 100,
    };

    expect(Object.keys(usage).sort()).toEqual([
      "cacheCreationInputTokens",
      "cacheReadInputTokens",
      "inputTokens",
      "outputTokens",
    ]);
  });
});
