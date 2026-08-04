// notifications.test.ts — F4 通知门控与事件映射测试 (P2-TE-04)
//
// 覆盖：
//   1. 窗口失焦 + PermissionRequest → toast 发送 + 任务栏闪烁
//   2. 窗口失焦 + Stop → toast 发送 + 任务栏闪烁
//   3. 窗口失焦 + StopFailure → toast 发送（错误类别）+ 任务栏闪烁
//   4. 窗口聚焦时三类事件 → toast 不发送、任务栏不闪烁
//   5. toast 正文含项目名 + 事件类别（去路由化后不含面板标题）
//   P2-TE-04: sendClickableNotification → sendToastNotification（两参数无 onClick）
//             删 onClick 路由 describe 整块；任务栏闪烁三类均触发（原仅 permission）

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ═══════════════════════════════════════════════════════════
// vi.hoisted() — 所有 mock 状态在模块级 mock 前创建
// ═══════════════════════════════════════════════════════════

const {
  mockSendToastNotification,
  mockRequestUserAttention,
  mockOnHookEventCallback,
  mockEnsureNotificationPermission,
} = vi.hoisted(() => ({
  mockSendToastNotification: vi.fn(),
  mockRequestUserAttention: vi.fn().mockResolvedValue(undefined),
  mockOnHookEventCallback: {
    cb: null as ((payload: import("../ipc/hooks").HookEventPayload) => void) | null,
  },
  mockEnsureNotificationPermission: vi.fn().mockResolvedValue(true),
}));

// ═══════════════════════════════════════════════════════════
// 模块级 mock — 覆盖 setup.ts 中的全局 mock
// ═══════════════════════════════════════════════════════════

vi.mock("../ipc/notification", () => ({
  sendToastNotification: mockSendToastNotification,
  ensureNotificationPermission: mockEnsureNotificationPermission,
  requestPermission: vi.fn(),
  isPermissionGranted: vi.fn(() => Promise.resolve(true)),
}));

vi.mock("../ipc/hooks", () => ({
  onHookEvent: vi.fn((cb: (payload: import("../ipc/hooks").HookEventPayload) => void) => {
    mockOnHookEventCallback.cb = cb;
    return () => {};
  }),
  contextUsage: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    requestUserAttention: mockRequestUserAttention,
    onFocusChanged: vi.fn(() => () => {}),
  })),
  UserAttentionType: { Critical: 1 },
}));

// ═══════════════════════════════════════════════════════════
// 导入 — mock 之后才能 import 被测模块与 stores
// ═══════════════════════════════════════════════════════════

import {
  useClaudeNotifications,
  classifyEvent,
  type NotifyCategory,
} from "../features/notifications/useClaudeNotifications";
import { useProjects } from "../stores/projects";

// ── 辅助函数 ───────────────────────────────────────────────

/** 构造 HookEventPayload（最小字段 + 可覆盖） */
function makePayload(
  overrides: Partial<import("../ipc/hooks").HookEventPayload> = {},
): import("../ipc/hooks").HookEventPayload {
  return {
    panelId: "terminal-p1-0",
    event: "Stop",
    timestamp: Date.now(),
    sessionId: "s1",
    transcriptPath: "/tmp/t.jsonl",
    cwd: "/home/user/proj",
    toolName: null,
    notificationType: null,
    ...overrides,
  };
}

/** 在 useProjects 中种子项目与页面数据 */
function seedProjects(): void {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath: "C:\\Users\\test\\proj",
        pages: [
          {
            pageId: "p1",
            name: "页面 1",
            layout: {},
            cwd: "C:\\Users\\test\\proj",
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "p1",
        version: 1,
      },
    },
    expandedNodes: { "proj-1": true, p1: true },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
}

/** 设置窗口焦点状态 */
function setWindowFocused(focused: boolean): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__slterm_windowFocused = focused;
}

// ═══════════════════════════════════════════════════════════
// classifyEvent 表驱动（NAH-03，D2 导出纯函数）
// ═══════════════════════════════════════════════════════════

describe("classifyEvent 分类表驱动（NAH-03）", () => {
  beforeEach(() => {
    // 重置所有 mock
    vi.clearAllMocks();
    mockOnHookEventCallback.cb = null;

    // 重置 stores 到初始状态
    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });

    // 清除 window 全局
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  // ── 纯函数层：事件 × notificationType → 类别映射 ──────────

  const classifyTable: Array<{
    name: string;
    payload: Partial<import("../ipc/hooks").HookEventPayload>;
    expected: NotifyCategory | null;
  }> = [
    { name: "PermissionRequest → permission", payload: { event: "PermissionRequest" }, expected: "permission" },
    { name: "Notification + permission_prompt → permission", payload: { event: "Notification", notificationType: "permission_prompt" }, expected: "permission" },
    { name: "Notification + idle_prompt → null（其他子类型不通知）", payload: { event: "Notification", notificationType: "idle_prompt" }, expected: null },
    { name: "Notification + 无 notificationType → null", payload: { event: "Notification", notificationType: null }, expected: null },
    { name: "Stop → done", payload: { event: "Stop" }, expected: "done" },
    { name: "StopFailure → error", payload: { event: "StopFailure" }, expected: "error" },
    { name: "PostToolUseFailure → error", payload: { event: "PostToolUseFailure" }, expected: "error" },
    { name: "未识别事件（PreToolUse）→ null", payload: { event: "PreToolUse" }, expected: null },
    { name: "未识别事件（SessionStart）→ null", payload: { event: "SessionStart" }, expected: null },
  ];

  it.each(classifyTable)("$name", ({ payload, expected }) => {
    expect(classifyEvent(makePayload(payload))).toBe(expected);
  });

  // ── hook 链路层：toast 触发与否 + 标题/正文 ──────────────

  const hookTable: Array<{
    name: string;
    payload: Partial<import("../ipc/hooks").HookEventPayload>;
    label: string | null; // null = 不触发 toast
  }> = [
    { name: "失焦 + PermissionRequest → toast「权限请求」", payload: { event: "PermissionRequest" }, label: "权限请求" },
    { name: "失焦 + Notification(permission_prompt) → toast「权限请求」", payload: { event: "Notification", notificationType: "permission_prompt" }, label: "权限请求" },
    { name: "失焦 + Notification(idle_prompt) → 不 toast", payload: { event: "Notification", notificationType: "idle_prompt" }, label: null },
    { name: "失焦 + Stop → toast「任务完成」", payload: { event: "Stop" }, label: "任务完成" },
    { name: "失焦 + StopFailure → toast「错误」", payload: { event: "StopFailure" }, label: "错误" },
    { name: "失焦 + PostToolUseFailure → toast「错误」", payload: { event: "PostToolUseFailure" }, label: "错误" },
    { name: "失焦 + 未识别事件（PreToolUse）→ 不 toast", payload: { event: "PreToolUse" }, label: null },
  ];

  it.each(hookTable)("$name", ({ payload, label }) => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload(payload));
    });

    if (label === null) {
      expect(mockSendToastNotification).not.toHaveBeenCalled();
      expect(mockRequestUserAttention).not.toHaveBeenCalled();
    } else {
      expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
      const [title, options] = mockSendToastNotification.mock
        .calls[0] as [string, { body: string }];
      expect(title).toBe("slTerminal");
      expect(options.body).toContain(label);
      expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
    }
  });
});

// ═══════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════

describe("F4 通知门控", () => {
  beforeEach(() => {
    // 重置所有 mock
    vi.clearAllMocks();
    mockOnHookEventCallback.cb = null;

    // 重置 stores 到初始状态
    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    // layout 重置（notifications 测试不依赖 activePageId）

    // 清除 window 全局
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  afterEach(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  // ── 基础渲染 ─────────────────────────────────────────────

  it("挂载时注册 onHookEvent 监听", () => {
    const { unmount } = renderHook(() => useClaudeNotifications());
    expect(mockOnHookEventCallback.cb).not.toBeNull();
    unmount();
  });

  it("卸载时清理 onHookEvent 监听", () => {
    const { unmount } = renderHook(() => useClaudeNotifications());
    expect(mockOnHookEventCallback.cb).not.toBeNull();
    unmount();
  });

  // ── 失焦 + 权限请求事件 ──────────────────────────────────

  it("窗口失焦 + PermissionRequest 事件 → 发送 toast 且闪烁任务栏", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "PermissionRequest" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    // 验证 toast 发送（两参数，无 onClick）
    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [title, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(title).toBe("slTerminal");
    expect(options.body).toContain("权限请求");

    // 验证任务栏闪烁（三类事件均触发）
    expect(mockRequestUserAttention).toHaveBeenCalledTimes(1);
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1); // UserAttentionType.Critical
  });

  it("窗口失焦 + Notification(permission_prompt) 事件 → 发送 toast + 闪烁", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({
      event: "Notification",
      notificationType: "permission_prompt",
    });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(options.body).toContain("权限请求");
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  // ── 失焦 + Stop 事件（任务完成）──────────────────────────

  it("窗口失焦 + Stop 事件 → 发送 toast + 任务栏闪烁", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "Stop" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(options.body).toContain("任务完成");

    // P2-TE-04: Stop 也触发闪烁（三类全覆盖）
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  // ── 失焦 + StopFailure 事件（错误）───────────────────────

  it("窗口失焦 + StopFailure 事件 → 发送 toast（错误类别）+ 闪烁", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "StopFailure" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(options.body).toContain("错误");

    // P2-TE-04: StopFailure 也触发闪烁
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  it("窗口失焦 + PostToolUseFailure 事件 → 发送 toast（错误类别）+ 闪烁", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "PostToolUseFailure" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(options.body).toContain("错误");

    // P2-TE-04: PostToolUseFailure 也触发闪烁
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  // ── 聚焦时门控 ───────────────────────────────────────────

  it("窗口聚焦 + PermissionRequest 事件 → 不发送 toast、不闪烁", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("窗口聚焦 + Stop 事件 → 不发送 toast、不闪烁", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("窗口聚焦 + StopFailure 事件 → 不发送 toast、不闪烁", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "StopFailure" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  // ── 窗口聚焦状态缺失时的行为 ────────────────────────────

  it("__slterm_windowFocused 未定义时按聚焦处理（不发送通知）", () => {
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
  });

  // ── 非通知事件不触发 ────────────────────────────────────

  it("失焦 + PreToolUse 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PreToolUse" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("失焦 + SessionStart 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "SessionStart" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
  });

  it("失焦 + SessionEnd 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "SessionEnd" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
  });

  it("失焦 + PostToolUse 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PostToolUse" }));
    });

    expect(mockSendToastNotification).not.toHaveBeenCalled();
  });

  // ── 60s 内同事件去重 ─────────────────────────────────────

  it("60s 内同一 session+event+timestamp 只发送一次 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "Stop", sessionId: "s1", timestamp: 1000 });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);

    // 同一事件再次触发 → 去重跳过
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
  });

  it("不同 sessionId 或不同 event 的事件各自发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 1000,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);

    // 不同的 sessionId
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s2", timestamp: 1000,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(2);

    // 不同事件
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "StopFailure", sessionId: "s1", timestamp: 2000,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(3);
  });

  it("去重缓存超 200 截断保留最近 100 条，最旧事件重新触发再弹 toast（NAH-04）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    // 构造 250 个不同事件（timestamp 递增推进）→ 各自 toast
    for (let i = 1; i <= 250; i++) {
      act(() => {
        mockOnHookEventCallback.cb!(makePayload({
          event: "Stop", sessionId: "s1", timestamp: i,
        }));
      });
    }
    expect(mockSendToastNotification).toHaveBeenCalledTimes(250);

    // 第 201 个事件触发时缓存超 200 → slice(-100) 截断，
    // 第 101 个（被截断移除的边界）重新触发 → 再弹 toast
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 101,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(251);

    // 最旧事件（第 1 个）已被截断移除 → 重新触发再弹 toast（核心断言）
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 1,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(252);

    // 截断保留的第 102 个与最新第 250 个仍被去重 → 不再弹 toast
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 102,
      }));
    });
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 250,
      }));
    });
    expect(mockSendToastNotification).toHaveBeenCalledTimes(252);
  });

  // ── toast 正文内容验证（去面板标题后）─────────────────────

  it("toast 正文含项目名和事件类别（不再含面板标题）", () => {
    seedProjects();
    setWindowFocused(false);

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop",
        panelId: "terminal-p1-0",
      }));
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const [, options] = mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(options.body).toContain("测试项目");
    expect(options.body).toContain("任务完成");
    // 去路由化后 body 不再含 panelId/面板标题
    expect(options.body).not.toContain("terminal-p1-0");
  });

  it("sendToastNotification 仅接收两个参数（无 onClick）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    expect(mockSendToastNotification).toHaveBeenCalledTimes(1);
    const args = mockSendToastNotification.mock.calls[0];
    // 仅两参数：title + options，无 onClick
    expect(args).toHaveLength(2);
    expect(typeof args[0]).toBe("string");
    expect(typeof args[1]).toBe("object");
    expect(args[1]).toHaveProperty("body");
  });
});

// ═══════════════════════════════════════════════════════════
// 任务栏闪烁细分（P2-TE-04 反转：三类事件均闪烁）
// ═══════════════════════════════════════════════════════════

describe("任务栏闪烁（UserAttention）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnHookEventCallback.cb = null;

    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    // layout 重置（notifications 测试不依赖 activePageId）

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  it("PermissionRequest 触发时调用 requestUserAttention 并传入 Critical", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    expect(mockRequestUserAttention).toHaveBeenCalledTimes(1);
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1); // UserAttentionType.Critical
  });

  it("Notification(permission_prompt) 触发时调用 requestUserAttention", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Notification",
        notificationType: "permission_prompt",
      }));
    });

    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  it("Stop 事件触发 requestUserAttention（P2-TE-04 反转）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  it("StopFailure 事件触发 requestUserAttention（P2-TE-04 反转）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "StopFailure" }));
    });

    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  it("PostToolUseFailure 事件触发 requestUserAttention（P2-TE-04 反转）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PostToolUseFailure" }));
    });

    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  it("聚焦时不触发 requestUserAttention", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });
});
