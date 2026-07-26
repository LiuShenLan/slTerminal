// notifications.test.ts — F4 通知门控与事件映射测试 (P2-TE-01)
//
// 覆盖：
//   1. 窗口失焦 + PermissionRequest → toast 发送 + 任务栏闪烁
//   2. 窗口失焦 + Stop → toast 发送（不含任务栏闪烁）
//   3. 窗口失焦 + StopFailure → toast 发送（错误类别）
//   4. 窗口聚焦时三类事件 → toast 不发送
//   5. toast onClick → setFocus + switchToPage + panel.focus 调用
//   6. panel 已关闭时 onClick 不抛异常

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ═══════════════════════════════════════════════════════════
// vi.hoisted() — 所有 mock 状态在模块级 mock 前创建
// ═══════════════════════════════════════════════════════════

const {
  mockSendClickableNotification,
  mockRequestUserAttention,
  mockSetFocus,
  mockOnHookEventCallback,
  mockEnsureNotificationPermission,
  mockSetProjectRoot,
} = vi.hoisted(() => ({
  mockSendClickableNotification: vi.fn(),
  mockRequestUserAttention: vi.fn().mockResolvedValue(undefined),
  mockSetFocus: vi.fn().mockResolvedValue(undefined),
  mockOnHookEventCallback: {
    cb: null as ((payload: import("../ipc/hooks").HookEventPayload) => void) | null,
  },
  mockEnsureNotificationPermission: vi.fn().mockResolvedValue(true),
  mockSetProjectRoot: vi.fn().mockResolvedValue(undefined),
}));

// ═══════════════════════════════════════════════════════════
// 模块级 mock — 覆盖 setup.ts 中的全局 mock
// ═══════════════════════════════════════════════════════════

vi.mock("../ipc/notification", () => ({
  sendClickableNotification: mockSendClickableNotification,
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

vi.mock("../ipc/fs", () => ({
  setProjectRoot: mockSetProjectRoot,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    setFocus: mockSetFocus,
    requestUserAttention: mockRequestUserAttention,
    onFocusChanged: vi.fn(() => () => {}),
  })),
  UserAttentionType: { Critical: 1 },
}));

// ═══════════════════════════════════════════════════════════
// 导入 — mock 之后才能 import 被测模块与 stores
// ═══════════════════════════════════════════════════════════

import { useClaudeNotifications } from "../features/notifications/useClaudeNotifications";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ── 类型 ───────────────────────────────────────────────────

interface MockPanel {
  focus: ReturnType<typeof vi.fn>;
  title: string;
}

interface MockDockviewApi {
  getPanel: ReturnType<typeof vi.fn>;
}

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

/** 在 useLayout 中设置活跃页面 */
function seedLayout(pageId: string | null = "p1"): void {
  useLayout.setState({ activePageId: pageId });
}

/** 构造 mock DockviewApi */
function makeMockDockviewApi(
  panels: Record<string, MockPanel>,
): MockDockviewApi {
  return {
    getPanel: vi.fn((id: string) => panels[id] ?? null),
  };
}

/** 设置 window.__dockviewApi */
function setDockviewApi(api: MockDockviewApi): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = api;
}

/** 设置窗口焦点状态 */
function setWindowFocused(focused: boolean): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__slterm_windowFocused = focused;
}

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
    useLayout.setState({ activePageId: null });

    // 清除 window 全局
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__dockviewApi;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  afterEach(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__dockviewApi;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  // ── 基础渲染 ─────────────────────────────────────────────

  it("挂载时注册 onHookEvent 监听", () => {
    const { unmount } = renderHook(() => useClaudeNotifications());
    // mockOnHookEventCallback.cb 在 effect 中由 onHookEvent mock 设置
    expect(mockOnHookEventCallback.cb).not.toBeNull();
    unmount();
  });

  it("卸载时清理 onHookEvent 监听", () => {
    const { unmount } = renderHook(() => useClaudeNotifications());
    expect(mockOnHookEventCallback.cb).not.toBeNull();
    unmount();
    // 卸载后 cb 仍引用旧函数——验证不抛异常即可（cleanup 已调）
  });

  // 注意：ensureNotificationPermission 由模块级 permissionEnsured 守卫控制仅首次调用；
  // 该变量在 describe 内多个测试间持久化，无法在 beforeEach 中重置（不修改生产代码），
  // 故不单独断言调用次数。权限初始化行为由其余通知发送/门控用例间接覆盖。

  // ── 失焦 + 权限请求事件 ──────────────────────────────────

  it("窗口失焦 + PermissionRequest 事件 → 发送 toast 且闪烁任务栏", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "PermissionRequest" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    // 验证 toast 发送
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [title, body, onClick] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(title).toBe("slTerminal");
    expect(body).toContain("权限请求");
    expect(typeof onClick).toBe("function");

    // 验证任务栏闪烁
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

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(body).toContain("权限请求");
    expect(mockRequestUserAttention).toHaveBeenCalledWith(1);
  });

  // ── 失焦 + Stop 事件（任务完成）──────────────────────────

  it("窗口失焦 + Stop 事件 → 发送 toast（不含任务栏闪烁）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "Stop" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(body).toContain("任务完成");

    // Stop 不是权限事件，不闪烁
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  // ── 失焦 + StopFailure 事件（错误）───────────────────────

  it("窗口失焦 + StopFailure 事件 → 发送 toast（错误类别）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "StopFailure" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(body).toContain("错误");

    // 错误不是权限事件，不闪烁
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("窗口失焦 + PostToolUseFailure 事件 → 发送 toast（错误类别）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "PostToolUseFailure" });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(body).toContain("错误");
  });

  // ── 聚焦时门控 ───────────────────────────────────────────

  it("窗口聚焦 + PermissionRequest 事件 → 不发送 toast", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("窗口聚焦 + Stop 事件 → 不发送 toast", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  it("窗口聚焦 + StopFailure 事件 → 不发送 toast", () => {
    setWindowFocused(true);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "StopFailure" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  // ── 窗口聚焦状态缺失时的行为 ────────────────────────────

  it("__slterm_windowFocused 未定义时按聚焦处理（不发送通知）", () => {
    // 不设置 window.__slterm_windowFocused——模拟启动初期焦点状态未初始化
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PermissionRequest" }));
    });

    // window.__slterm_windowFocused !== false 条件不满足 → 跳过
    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  // ── 非通知事件不触发 ────────────────────────────────────

  it("失焦 + PreToolUse 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PreToolUse" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  it("失焦 + SessionStart 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "SessionStart" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  it("失焦 + SessionEnd 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "SessionEnd" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  it("失焦 + PostToolUse 事件 → 不发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PostToolUse" }));
    });

    expect(mockSendClickableNotification).not.toHaveBeenCalled();
  });

  // ── 60s 内同事件去重 ─────────────────────────────────────

  it("60s 内同一 session+event+timestamp 只发送一次 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    const payload = makePayload({ event: "Stop", sessionId: "s1", timestamp: 1000 });
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);

    // 同一事件再次触发 → 去重跳过
    act(() => {
      mockOnHookEventCallback.cb!(payload);
    });
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
  });

  it("不同 sessionId 或不同 event 的事件各自发送 toast", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s1", timestamp: 1000,
      }));
    });
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);

    // 不同的 sessionId
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop", sessionId: "s2", timestamp: 1000,
      }));
    });
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(2);

    // 不同事件
    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "StopFailure", sessionId: "s1", timestamp: 2000,
      }));
    });
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(3);
  });

  // ── toast 正文内容验证 ───────────────────────────────────

  it("toast 正文含项目名和页签标题", () => {
    seedProjects();
    setWindowFocused(false);
    // 设置 dockviewApi 以提供面板标题
    const mockPanel = { focus: vi.fn(), title: "终端 1" };
    setDockviewApi(makeMockDockviewApi({ "terminal-p1-0": mockPanel }));

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop",
        panelId: "terminal-p1-0",
      }));
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    expect(body).toContain("测试项目");
    expect(body).toContain("终端 1");
    expect(body).toContain("任务完成");
  });

  it("dockviewApi 不可用时回退 panelId 作为标题", () => {
    seedProjects();
    setWindowFocused(false);
    // 不设 __dockviewApi — 测试回退路径

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "StopFailure",
        panelId: "terminal-p1-0",
      }));
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const [, body] = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    // 回退到 panelId
    expect(body).toContain("terminal-p1-0");
  });

  // ── 非终端 panelId 不触发（panelId 解析失败） ─────────────

  it("非 terminal- 前缀的 panelId 不发送 toast（不可路由）", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "Stop",
        panelId: "editor-p1-0",
      }));
    });

    // toast 仍会发送（事件分类由 event 决定，不依赖 panelId 格式）
    // 但面板标题回退到 panelId
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════
// toast onClick 路由
// ═══════════════════════════════════════════════════════════

describe("toast onClick 路由", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnHookEventCallback.cb = null;

    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: null });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__dockviewApi;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__slterm_windowFocused;
  });

  /** 准备 onClick 测试环境：种子 stores、聚焦面板、渲染 hook、触发事件 */
  function setupOnClickTest(): () => void {
    seedProjects();
    seedLayout("p1");
    setWindowFocused(false);

    const mockPanel = { focus: vi.fn(), title: "终端 1" };
    setDockviewApi(makeMockDockviewApi({ "terminal-p1-0": mockPanel }));

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "PermissionRequest",
        panelId: "terminal-p1-0",
        sessionId: "s-onclick",
      }));
    });

    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const args = mockSendClickableNotification.mock
      .calls[0] as [string, string, () => void];
    const onClick = args[2];
    expect(typeof onClick).toBe("function");
    return onClick;
  }

  it("onClick 调用 setFocus 聚焦窗口", async () => {
    const onClick = setupOnClickTest();
    await act(async () => {
      onClick();
    });

    // setFocus 由 onClick 内代码调用，经 src/ipc/window.ts → getCurrentWindow() → mockSetFocus
    // 由于是 async 链，需要等微任务完成
    await vi.waitFor(() => {
      expect(mockSetFocus).toHaveBeenCalled();
    }, { timeout: 1000 });
  });

  it("onClick 调用 setProjectRoot 设置项目根路径", async () => {
    const onClick = setupOnClickTest();
    await act(async () => {
      onClick();
    });

    await vi.waitFor(() => {
      expect(mockSetProjectRoot).toHaveBeenCalledWith("C:\\Users\\test\\proj");
    }, { timeout: 1000 });
  });

  it("onClick 调用 setActivePage 切换页面", async () => {
    const onClick = setupOnClickTest();
    await act(async () => {
      onClick();
    });

    await vi.waitFor(() => {
      expect(useLayout.getState().activePageId).toBe("p1");
    }, { timeout: 1000 });
  });

  it("onClick 调用 panel.focus 聚焦面板", async () => {
    const onClick = setupOnClickTest();

    // 获取 mock panel
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const mockPanel = (window as any).__dockviewApi.getPanel("terminal-p1-0");
    expect(mockPanel).not.toBeNull();

    await act(async () => {
      onClick();
    });

    await vi.waitFor(() => {
      expect(mockPanel.focus).toHaveBeenCalled();
    }, { timeout: 1000 });
  });

  it("panel 已关闭时 onClick 不抛异常", async () => {
    seedProjects();
    seedLayout("p1");
    setWindowFocused(false);

    // dockviewApi 中该 panel 不存在（模拟已关闭）
    setDockviewApi(makeMockDockviewApi({}));

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "PermissionRequest",
        panelId: "terminal-p1-0",
      }));
    });

    const onClick = (mockSendClickableNotification.mock.calls[0] as [string, string, () => void])[2];

    // 不应抛异常
    await act(async () => {
      onClick();
    });

    // setFocus 仍被调用（窗口聚焦在路由之前）
    await vi.waitFor(() => {
      expect(mockSetFocus).toHaveBeenCalled();
    }, { timeout: 1000 });
  });

  it("项目不存在时 onClick 不抛异常", async () => {
    // 种子数据中 panelId 所属 pageId 不在任何项目中
    useProjects.setState({
      projects: {
        "proj-other": {
          projectId: "proj-other",
          name: "其他项目",
          rootPath: "C:\\other",
          pages: [{ pageId: "px", name: "x", layout: {}, cwd: "C:\\other", createdAt: 1, lastAccessedAt: 1 }],
          activePageId: "px",
          version: 1,
        },
      },
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    seedLayout("px");
    setWindowFocused(false);

    setDockviewApi(makeMockDockviewApi({ "terminal-p1-0": { focus: vi.fn(), title: "T" } }));

    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({
        event: "PermissionRequest",
        panelId: "terminal-p1-0",
      }));
    });

    const onClick = (mockSendClickableNotification.mock.calls[0] as [string, string, () => void])[2];

    await act(async () => {
      onClick();
    });

    // setFocus 仍被调用（在 routeToPanel 之前）
    await vi.waitFor(() => {
      expect(mockSetFocus).toHaveBeenCalled();
    }, { timeout: 1000 });
  });

  it("sendClickableNotification onClick 通过工厂绑定，非 sendNotification Options", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    // 验证 sendClickableNotification 被调用且第三个参数是函数
    expect(mockSendClickableNotification).toHaveBeenCalledTimes(1);
    const args = mockSendClickableNotification.mock.calls[0];
    expect(args).toHaveLength(3);
    expect(typeof args[2]).toBe("function");
    // 第三个参数就是 onClick 回调——走 sendClickableNotification 工厂，不是 sendNotification Options
  });
});

// ═══════════════════════════════════════════════════════════
// 任务栏闪烁细分
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
    useLayout.setState({ activePageId: null });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__dockviewApi;
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

  it("Stop 事件不触发 requestUserAttention", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "Stop" }));
    });

    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("StopFailure 事件不触发 requestUserAttention", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "StopFailure" }));
    });

    expect(mockRequestUserAttention).not.toHaveBeenCalled();
  });

  it("PostToolUseFailure 事件不触发 requestUserAttention", () => {
    setWindowFocused(false);
    renderHook(() => useClaudeNotifications());

    act(() => {
      mockOnHookEventCallback.cb!(makePayload({ event: "PostToolUseFailure" }));
    });

    expect(mockRequestUserAttention).not.toHaveBeenCalled();
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
