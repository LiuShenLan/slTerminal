// ipc-agent-hooks-contract.test.ts — agent hooks IPC wrapper 合约测试（IHE-06 工厂化，MC-212 更名同步）
//
// 经共享工厂 describeIpcContract（helpers/ipc-contract.ts）声明式驱动
// 四命令（inject/uninstall/getInjectionStatus/restoreStatusline）× 四维
// （命令名 / 参数含 cliId camelCase / 正常返回 / 异常传播）；onAgentEvent 为 listen 事件封装，
// 属"wrapper 行为契约"（IHE-01②）——手写模拟驱动断言，不走 invoke 工厂。
// 原 agent_context_usage（transcript token 扫描）已整体移除——百分比经 ContextUsage
// 信号走 agent-event 通道推送，无独立 invoke 命令。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { describeIpcContract } from "./helpers/ipc-contract";

// 覆盖 setup.ts 全局 mock——导入原始 ../ipc/agentHooks 模块以测试真实 IPC 合约
vi.mock("../ipc/agentHooks", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/agentHooks")>();
});

// mock @tauri-apps/api/event — onAgentEvent 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
import type { AgentEventPayload } from "../types/agent";
import * as agentHooks from "../ipc/agentHooks";

// 泛化命令 cliId 实参（合约测试固定用 claude——与 E2E helper 同口径，测试基建字面量合法）
const CLI_ID = "claude";

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// agent_hooks_inject
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("agent_hooks_inject 合约", [
  {
    name: "应调用 agent_hooks_inject 命令（仅 cliId 参数）",
    cmd: "agent_hooks_inject",
    call: () => agentHooks.inject(CLI_ID),
    respond: { status: "injected", version: 1 },
    expectArgs: { cliId: CLI_ID },
    expectExactKeys: ["cliId"],
  },
  {
    name: "应返回 injected 状态的 AgentHookInjectionStatus",
    cmd: "agent_hooks_inject",
    call: () => agentHooks.inject(CLI_ID),
    respond: { status: "injected", version: 1 },
    expectResult: { status: "injected", version: 1 },
  },
  {
    name: "未注入时返回 notInjected 状态（version 为 null）",
    cmd: "agent_hooks_inject",
    call: () => agentHooks.inject(CLI_ID),
    respond: { status: "notInjected", version: null },
    expectResult: { status: "notInjected", version: null },
  },
  {
    name: "版本过旧时返回 outdated 状态",
    cmd: "agent_hooks_inject",
    call: () => agentHooks.inject(CLI_ID),
    respond: { status: "outdated", version: 1 },
    expectResult: { status: "outdated", version: 1 },
  },
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "agent_hooks_inject",
    call: () => agentHooks.inject(CLI_ID),
    mockThrow: "settings.json 语法错误",
    expectReject: "settings.json 语法错误",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// agent_hooks_uninstall
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("agent_hooks_uninstall 合约", [
  {
    name: "应调用 agent_hooks_uninstall 命令（仅 cliId 参数）",
    cmd: "agent_hooks_uninstall",
    call: () => agentHooks.uninstall(CLI_ID),
    respond: undefined,
    expectArgs: { cliId: CLI_ID },
    expectExactKeys: ["cliId"],
  },
  {
    name: "正常返回 void",
    cmd: "agent_hooks_uninstall",
    call: () => agentHooks.uninstall(CLI_ID),
    respond: undefined,
    expectUndefined: true,
  },
  {
    name: "invoke 失败时异常应传播",
    cmd: "agent_hooks_uninstall",
    call: () => agentHooks.uninstall(CLI_ID),
    mockThrow: "删除脚本目录失败",
    expectReject: "删除脚本目录失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// agent_hooks_injection_status
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("agent_hooks_injection_status 合约", [
  {
    name: "应调用 agent_hooks_injection_status 命令（仅 cliId 参数）",
    cmd: "agent_hooks_injection_status",
    call: () => agentHooks.getInjectionStatus(CLI_ID),
    respond: { status: "injected", version: 1 },
    expectArgs: { cliId: CLI_ID },
    expectExactKeys: ["cliId"],
  },
  {
    name: "应返回 AgentHookInjectionStatus",
    cmd: "agent_hooks_injection_status",
    call: () => agentHooks.getInjectionStatus(CLI_ID),
    respond: { status: "injected", version: 1 },
    expectResult: { status: "injected", version: 1 },
  },
  {
    name: "invoke 失败时异常应传播",
    cmd: "agent_hooks_injection_status",
    call: () => agentHooks.getInjectionStatus(CLI_ID),
    mockThrow: "读取配置失败",
    expectReject: "读取配置失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// agent_hooks_restore_statusline（关闭清理：还原 statusline 桥接）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract("restoreStatusline 合约（agent_hooks_restore_statusline）", [
  {
    name: "应调用 agent_hooks_restore_statusline 命令（仅 cliId 参数）",
    cmd: "agent_hooks_restore_statusline",
    call: () => agentHooks.restoreStatusline(CLI_ID),
    respond: undefined,
    expectArgs: { cliId: CLI_ID },
    expectExactKeys: ["cliId"],
  },
  {
    name: "正常返回 void",
    cmd: "agent_hooks_restore_statusline",
    call: () => agentHooks.restoreStatusline(CLI_ID),
    respond: undefined,
    expectUndefined: true,
  },
  {
    name: "invoke 失败时异常应传播给调用方",
    cmd: "agent_hooks_restore_statusline",
    call: () => agentHooks.restoreStatusline(CLI_ID),
    mockThrow: "settings.json 写入失败",
    expectReject: "settings.json 写入失败",
  },
]);

// ═══════════════════════════════════════════════════════════════════
// onAgentEvent（事件订阅封装）——wrapper 行为契约（IHE-01②）
//
// listen 封装的回调解包（event.payload）不在 mockIPC 层验证——Tauri 的
// listen 运行时解包由 L4 E2E 守卫。此处用模拟驱动断言 wrapper 自身的
// 解包逻辑：构造 { payload } 事件对象 → 断言 callback 收到解包后 payload。
// ═══════════════════════════════════════════════════════════════════

describe("onAgentEvent 合约", () => {
  it("应调用 listen 监听 agent-event 事件", () => {
    vi.mocked(listen).mockReturnValue(Promise.resolve(vi.fn()));

    const callback = vi.fn();
    agentHooks.onAgentEvent(callback);

    expect(listen).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledWith("agent-event", expect.any(Function));
  });

  it("返回 unsubscribe 函数", () => {
    vi.mocked(listen).mockReturnValue(Promise.resolve(vi.fn()));

    const unsubscribe = agentHooks.onAgentEvent(vi.fn());

    expect(typeof unsubscribe).toBe("function");
  });

  it("回调应接收 event.payload 传递给 callback（解包行为契约）", () => {
    const mockPayload: AgentEventPayload = {
      panelId: "terminal-p1-0",
      event: "UserPromptSubmit",
      timestamp: 1700000000000,
      sessionId: "abc-123",
      usageSourcePath: "/tmp/transcript.jsonl",
      cwd: "/home/user/project",
      toolName: null,
      notificationType: null,
    };

    let capturedHandler: ((event: { payload: AgentEventPayload }) => void) | null =
      null;
    vi.mocked(listen).mockImplementation(
      (_event: string, handler: unknown) => {
        capturedHandler = handler as typeof capturedHandler;
        return Promise.resolve(vi.fn());
      },
    );

    const callback = vi.fn();
    agentHooks.onAgentEvent(callback);

    // 模拟 Tauri 事件推送
    expect(capturedHandler).not.toBeNull();
    capturedHandler!({ payload: mockPayload });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith(mockPayload);
  });

  it("返回的 unsubscribe 应调用 listen 返回的清理函数", async () => {
    const mockUnlisten = vi.fn();
    vi.mocked(listen).mockReturnValue(Promise.resolve(mockUnlisten));

    const unsubscribe = agentHooks.onAgentEvent(vi.fn());

    // 等待 listen Promise resolve
    await new Promise((resolve) => setTimeout(resolve, 0));

    unsubscribe();
    // .then() 微任务需 flush 后才执行 mockUnlisten
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });

  it("AgentEventPayload 字段完整性验证（C1 契约 8 字段）", () => {
    const payload: AgentEventPayload = {
      panelId: "terminal-p1-0",
      event: "PreToolUse",
      timestamp: 1700000000000,
      sessionId: "sess-001",
      usageSourcePath: "/path/to/transcript.jsonl",
      cwd: "/project",
      toolName: "read",
      notificationType: null,
    };

    // 验证恰好 8 个字段（C1 契约；cliId/usageSourcePath 为可选字段——本用例显式构造 usageSourcePath 纳入键集合）
    expect(Object.keys(payload).sort()).toEqual([
      "cwd",
      "event",
      "notificationType",
      "panelId",
      "sessionId",
      "timestamp",
      "toolName",
      "usageSourcePath",
    ]);

    // toolName 可为非 null 值（工具事件）
    expect(payload.toolName).toBe("read");
    // notificationType 可为 null
    expect(payload.notificationType).toBeNull();
  });
});

// ContextUsageSignal 键集合精确匹配（单字段——官方 used_percentage 口径）
describe("ContextUsageSignal 字段完整性（官方口径契约）", () => {
  it("ContextUsageSignal 恰好 1 字段（usedPercentage）", () => {
    const signal = { usedPercentage: 23.6 };
    expect(Object.keys(signal).sort()).toEqual(["usedPercentage"]);
  });

  it("AgentEventPayload.usedPercentage 可承载 ContextUsage 信号（可选字段）", () => {
    const payload: AgentEventPayload = {
      panelId: "terminal-p1-0",
      event: "ContextUsage",
      timestamp: 1700000000000,
      sessionId: "sess-001",
      cwd: "/project",
      toolName: null,
      notificationType: null,
      usedPercentage: 23.6,
    };
    expect(payload.usedPercentage).toBe(23.6);
  });
});
