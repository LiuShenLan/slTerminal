// mock-cli-profile.test.tsx — AC-4 mock profile 全链路验收 L2 用例（五点全表，Stage 07）
//
// 覆盖（spec 06 §7 + stages.md Stage 07 实现要点 2，夹具契约见 helpers/mockCliProfile.ts）：
//   ① OSC 133 命中：matchByCommand("mockcli --flag") 命中 → 页签标题 = mockcli tabTitle /
//      logo = mockcli iconSrc / setAgentSession 写入 agentSession.cliId（useCommandDetection
//      链路 L2 断言——真实 cliProfileRegistry + 真实 TerminalRegistry，未命中零副作用 + OSC 133 D 清会话）
//   ② hooks 能力被真实调用：eventToStatus 经 useXterm 事件路径（spy 入参 event+notificationType +
//      四态/会话写入）、classifyNotification 经通知调度路径（spy 入参 payload + toast 派发）
//   ③ 历史聚合 UI：mock 条目（AgentHistorySession cliId="mockcli"）出现在历史区 +
//      行 logo 按 session.cliId 取 mockcli iconSrc
//   ④ hub 选择行：两枚按钮（claude + mockcli，均 hasConfigEditor=true）+ 双向分派断言
//      （KZ-7：选中 mockcli → 桩组件渲染 data-e2e="mockcli-config-editor" + JsonMode 零调用；
//      选中 claude → JsonMode 被调用 + 桩标记不存在）+ selectedCli 持久化
//      （updateParameters + 显式 onLayoutChange/toJSON）与挂载恢复
//   ⑤ 恢复注入：pty.write 内容 = mock buildRestoreInput 桩输出（可识别前缀 "mockcli --resume"，
//      普通/fork）+ addPanel title = profile.tabTitle
//
// mock 边界：claude 基线（profiles/claude 常量）+ mockcli 夹具（helpers/mockCliProfile）经真实
// cliProfileRegistry 注册；cliProfiles 注册表、TerminalRegistry、stores 均真实（全链路）；
// 仅 mock @xterm/xterm（Terminal 实例捕获）、IPC 层、shortcuts/sidebar/pageApis 隔离。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import {
  render,
  cleanup,
  fireEvent,
  waitFor,
  act,
  renderHook,
} from "@testing-library/react";
import { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import {
  useCommandDetection,
  type TabState,
} from "../panels/terminal/useCommandDetection";
import { useXterm } from "../panels/terminal/useXterm";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
// NAV-08：AgentStatusRow/AgentHistorySections 已退役（视图迁入导航树）——
// 行渲染路径等价组件 = NavSessionRow（活跃行）/ NavHistoryRow（历史行）
import { NavSessionRow } from "../features/navTree/NavSessionRow";
import { NavHistoryRow } from "../features/navTree/NavHistoryRow";
import { useAgentNotifications } from "../features/notifications/useAgentNotifications";
import { restoreHistorySession } from "../features/agentHistory/restoreSession";
import HooksConfigPanel from "../panels/hooksConfig/HooksConfigPanel";
import { cliProfileRegistry } from "../features/cliProfiles";
import {
  claudeProfile,
  CLAUDE_CLI_ID,
} from "../features/cliProfiles/profiles/claude";
import type { AgentStatus } from "../lib/agentStatus";
import { EXPLORER_SELECTION_BG } from "../theme";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { AgentEventPayload } from "../types/agent";
import type { AgentHistorySession } from "../types/agentHistory";
import {
  mockCliProfile,
  registerMockCliProfile,
  resetCliProfileRegistry,
} from "./helpers/mockCliProfile";

// ═══════════════════════════════════════════════════════════════
// vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪
// ═══════════════════════════════════════════════════════════════

const h = vi.hoisted(() => {
  const agentEventCallbackRef: {
    current: ((p: AgentEventPayload) => void) | null;
  } = { current: null };
  return {
    agentEventCallbackRef,
    // Agent 事件捕获（useXterm 与 useAgentNotifications 各自测试内注册，逐测试覆盖）
    mockOnAgentEvent: vi.fn((cb: (p: AgentEventPayload) => void) => {
      agentEventCallbackRef.current = cb;
      return () => {};
    }),
    mockPtySpawn: vi.fn(),
    mockPtyWrite: vi.fn(),
    mockPtyResize: vi.fn(),
    mockPtyKill: vi.fn(),
    mockPtyGetBuildNumber: vi.fn(),
    mockOpenUrl: vi.fn(),
    mockWriteText: vi.fn(),
    mockSendToastNotification: vi.fn(),
    // 权限桩须返回 Promise——useAgentNotifications 对 ensureNotificationPermission().catch()
    mockEnsureNotificationPermission: vi.fn(async () => {}),
    mockDeleteHistorySession: vi.fn(),
    mockConfirmDialog: vi.fn(async () => true),
    mockReadHooksConfig: vi.fn(),
    mockWriteHooksConfig: vi.fn(),
    mockJsonMode: vi.fn(() => null),
    mockInject: vi.fn(),
    mockUninstall: vi.fn(),
    mockGetInjectionStatus: vi.fn(),
    mockSwitchToPageShared: vi.fn(),
    mockGetPageApi: vi.fn(),
    mockSwitchToPageAndFocus: vi.fn(),
    mockFit: vi.fn(),
    mockProposeDimensions: vi.fn(),
    // hub Dockview props mock（照 hooks-config-panel.test.tsx 先例）
    mockApi: {
      updateParameters: vi.fn(),
      onDidParametersChange: vi.fn(() => ({ dispose: vi.fn() })),
      getParameters: vi.fn(() => ({})),
      toJSON: vi.fn(() => ({ mockPanel: true })),
      title: "Hooks 配置",
      close: vi.fn(),
    },
    mockContainerApi: {
      toJSON: vi.fn(() => ({ mockLayout: true })),
    },
  };
});

// ═══════════════════════════════════════════════════════════════
// 捕获 mock Terminal 实例 + OSC 133 handler（照 use-xterm-lifecycle 先例）
// ═══════════════════════════════════════════════════════════════

let capturedTerminal: {
  element: HTMLDivElement;
  open: ReturnType<typeof vi.fn>;
  loadAddon: ReturnType<typeof vi.fn>;
  dispose: ReturnType<typeof vi.fn>;
  onData: ReturnType<typeof vi.fn>;
  write: ReturnType<typeof vi.fn>;
  writeln: ReturnType<typeof vi.fn>;
  focus: ReturnType<typeof vi.fn>;
  getSelection: ReturnType<typeof vi.fn>;
  paste: ReturnType<typeof vi.fn>;
  attachCustomKeyEventHandler: ReturnType<typeof vi.fn>;
  options: Record<string, unknown>;
  parser: {
    registerOscHandler: ReturnType<typeof vi.fn>;
  };
} | null = null;

let capturedOsc133Handler: ((data: string) => boolean) | null = null;

// ═══════════════════════════════════════════════════════════════
// 模块级 mock（路径相对于被测源文件 import 解析）
// ═══════════════════════════════════════════════════════════════

vi.mock("@xterm/xterm", () => {
  class MockTerminal {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    element: any;
    open = vi.fn();
    loadAddon = vi.fn();
    dispose = vi.fn();
    onData = vi.fn(() => ({ dispose: vi.fn() }));
    write = vi.fn();
    writeln = vi.fn();
    focus = vi.fn();
    getSelection = vi.fn(() => "");
    paste = vi.fn();
    attachCustomKeyEventHandler = vi.fn();
    options: Record<string, unknown> = {};
    parser = {
      registerOscHandler: vi.fn(
        (osc: number, handler: (data: string) => boolean) => {
          if (osc === 133) capturedOsc133Handler = handler;
          return { dispose: vi.fn() };
        },
      ),
    };

    constructor() {
      const el = document.createElement("div");
      this.element = el;
      capturedTerminal = this as unknown as typeof capturedTerminal;
    }
  }
  return { Terminal: MockTerminal };
});

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = h.mockFit;
    proposeDimensions = h.mockProposeDimensions;
    dispose = vi.fn();
  },
}));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    dispose = vi.fn();
    onContextLoss = vi.fn();
  },
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: h.mockOpenUrl,
}));

// useXterm 经 src/ipc  barrel 取 pty；restoreSession 直引 src/ipc/pty——两路共用同一 mock
vi.mock("../ipc", () => ({
  pty: {
    spawn: h.mockPtySpawn,
    write: h.mockPtyWrite,
    resize: h.mockPtyResize,
    kill: h.mockPtyKill,
    getWindowsBuildNumber: h.mockPtyGetBuildNumber,
  },
}));

vi.mock("../ipc/pty", () => ({
  write: h.mockPtyWrite,
}));

// 本地覆盖 setup.ts 全局 mock：捕获 onAgentEvent 回调供测试手动触发
vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: h.mockOnAgentEvent,
  inject: h.mockInject,
  uninstall: h.mockUninstall,
  getInjectionStatus: h.mockGetInjectionStatus,
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: h.mockWriteText,
}));

vi.mock("../ipc/notification", () => ({
  sendToastNotification: h.mockSendToastNotification,
  ensureNotificationPermission: h.mockEnsureNotificationPermission,
}));

vi.mock("../ipc/agentHistory", () => ({
  deleteHistorySession: h.mockDeleteHistorySession,
}));

// OV-02：hub dirty 守卫经 src/lib 的 confirmDialog（ask 已从 ipc/dialog 删除）
vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: h.mockConfirmDialog,
}));

vi.mock("../ipc/hooksConfig", () => ({
  readHooksConfig: h.mockReadHooksConfig,
  writeHooksConfig: h.mockWriteHooksConfig,
}));

vi.mock("../ipc/settings", () => ({
  loadSettings: vi.fn(async () => null),
  saveSettings: vi.fn(async () => {}),
}));

vi.mock("../panels/hooksConfig/JsonMode", () => ({
  default: h.mockJsonMode,
}));

vi.mock("../features/shortcuts", () => ({
  usePanelFocus: vi.fn(),
  getShortcutRegistry: () => ({
    register: vi.fn(),
    unregister: vi.fn(),
    pushContext: vi.fn(),
    popContext: vi.fn(),
    resolve: () => false,
    _reset: vi.fn(),
  }),
}));

// NAV-06：makeEmptyLayout 随 SidebarTree 退役迁入 navTree（restoreSession 消费点改引用）
vi.mock("../features/navTree/NavTree", () => ({
  makeEmptyLayout: () => ({}),
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageShared: h.mockSwitchToPageShared,
  getPageApi: h.mockGetPageApi,
  switchToPageAndFocus: h.mockSwitchToPageAndFocus,
}));

// ═══════════════════════════════════════════════════════════════
// 测试数据工厂
// ═══════════════════════════════════════════════════════════════

/** 构造最小 AgentEventPayload（cliId 可选注入，MC-205 分支一） */
function makeAgentPayload(
  partial: Partial<AgentEventPayload>,
): AgentEventPayload {
  return {
    panelId: "terminal-page1-0",
    event: "PreToolUse",
    timestamp: 1,
    sessionId: "s1",
    usageSourcePath: "/t.json",
    cwd: "",
    toolName: null,
    notificationType: null,
    ...partial,
  };
}

const SESSION_ID = "1a2b3c4d-1111-2222-3333-444455556666";

/** 构造最小 AgentHistorySession（缺省 cliId=mockcli，恢复注入断言用） */
function makeHistorySession(
  partial: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: SESSION_ID,
    cwd: "C:\\Users\\test\\proj",
    title: null,
    titleSource: "customTitle",
    firstPrompt: "mock 首条提示",
    mtimeMs: 123456,
    cwdExists: true,
    cliId: "mockcli",
    ...partial,
  };
}

/** TerminalRegistry 条目所需的 fitAddon 桩（仅形状，TerminalRegistry 不消费） */
const fitStub = {
  fit: vi.fn(),
  proposeDimensions: vi.fn(),
  dispose: vi.fn(),
} as unknown as FitAddon;

/** 创建指定尺寸容器（照 xterm-test-utils 先例） */
function createContainer(w = 800, h = 600): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: w, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: h, configurable: true });
  return el;
}

/** 注册表重置为指定集合（claude profile 为 side-effect 注册，测试内自管基线） */
function registerOnly(
  profiles: Parameters<typeof cliProfileRegistry.register>[0][],
) {
  cliProfileRegistry._reset();
  for (const p of profiles) cliProfileRegistry.register(p);
}

/** 种子 stores（照 hooks-config-panel.test.tsx 先例） */
function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
}

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 输入补空格 → "rgba(r, g, b, a)"，照 hooks-config-gui 先例） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  // 新方案 selection 类 token 为 rgba 形态，jsdom 输出 "rgba(r, g, b, a)"
  const m = hex.match(/^rgba\((\d+),(\d+),(\d+),([0-9.]+)\)$/);
  if (m) return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${m[4]})`;
  return hex;
}

/** 渲染 hub 面板（Dockview content component props 经强转传入 mock） */
function renderPanel(params?: Record<string, unknown>) {
  return render(
    React.createElement(HooksConfigPanel, {
      api: h.mockApi,
      containerApi: h.mockContainerApi,
      params,
    } as unknown as React.ComponentProps<typeof HooksConfigPanel>),
  );
}

// ═══════════════════════════════════════════════════════════════
// AC-4① OSC 133 命中（useCommandDetection 链路，真实注册表 + 真实 TerminalRegistry）
// ═══════════════════════════════════════════════════════════════

describe("AC-4① OSC 133 命中（useCommandDetection 链路）", () => {
  const panelId = "ac4-osc-133";
  let onTabStateChange: ReturnType<typeof vi.fn<(state: TabState) => void>>;

  beforeEach(() => {
    onTabStateChange = vi.fn<(state: TabState) => void>();
    capturedTerminal = null;
    capturedOsc133Handler = null;
    TerminalRegistry._reset();
    registerMockCliProfile();
    // 注册面板条目——setAgentSession 需条目存在（真实 TerminalRegistry 语义）
    const term = new Terminal() as unknown as Terminal;
    TerminalRegistry.register(panelId, {
      term,
      sessionId: "sid-osc-133",
      webglAddon: null,
      fitAddon: fitStub,
    });
    renderHook(() =>
      useCommandDetection(
        capturedTerminal as unknown as Terminal,
        panelId,
        onTabStateChange,
      ),
    );
  });

  afterEach(() => {
    cleanup();
    resetCliProfileRegistry();
    TerminalRegistry._reset();
  });

  it("matchByCommand('mockcli --flag') 命中 → 页签标题 + agentSession.cliId（F9 修订：logo 不经此路径）", () => {
    expect(capturedOsc133Handler).not.toBeNull();
    act(() => {
      capturedOsc133Handler!("C;mockcli --flag");
    });
    // 页签标题取自匹配 profile（tabTitle）；logo 由会话绑定驱动（TerminalPanel 订阅
    // sessionChange 按 agentSession.cliId 查 iconSrc），C 路径不再直传 logo
    expect(onTabStateChange).toHaveBeenCalledWith({
      active: true,
      title: mockCliProfile.tabTitle,
      status: "attention",
    });
    // MC-107: setAgentSession 写入 agentSession.cliId（会话绑定 logo 数据源 + hook 事件三级解析反查键）
    const session = TerminalRegistry.get(panelId)?.agentSession;
    expect(session?.cliId).toBe(mockCliProfile.id);
    expect(session?.matchedCommand).toBe(mockCliProfile.id);
  });

  it("未命中命令 → 零副作用（不触发回调/不写会话）", () => {
    act(() => {
      capturedOsc133Handler!("C;npm install");
    });
    expect(onTabStateChange).not.toHaveBeenCalled();
    expect(TerminalRegistry.get(panelId)?.agentSession).toBeUndefined();
  });

  it("OSC 133 D 命令退出 → 清会话 + onTabStateChange({ active: false })", () => {
    act(() => {
      capturedOsc133Handler!("C;mockcli --flag");
    });
    onTabStateChange.mockClear();
    act(() => {
      capturedOsc133Handler!("D;0");
    });
    expect(onTabStateChange).toHaveBeenCalledWith({ active: false });
    expect(TerminalRegistry.get(panelId)?.agentSession).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════
// AC-4② hooks 能力被真实调用（useXterm 事件路径 + 通知调度路径）
// ═══════════════════════════════════════════════════════════════

describe("AC-4② hooks 能力经真实链路调用", () => {
  let container: HTMLDivElement;
  let onTabStateChange: ReturnType<typeof vi.fn<(state: TabState) => void>>;

  beforeEach(() => {
    container = createContainer(800, 600);
    onTabStateChange = vi.fn<(state: TabState) => void>();
    capturedTerminal = null;
    capturedOsc133Handler = null;
    TerminalRegistry._reset();
    registerMockCliProfile();
    h.mockPtySpawn.mockReset().mockResolvedValue("test-session-id");
    h.mockPtyWrite.mockReset().mockResolvedValue(undefined);
    h.mockPtyResize.mockReset().mockResolvedValue(undefined);
    h.mockPtyKill.mockReset().mockResolvedValue(undefined);
    h.mockPtyGetBuildNumber.mockReset().mockResolvedValue(22621);
    h.mockOpenUrl.mockReset().mockResolvedValue(undefined);
    h.mockWriteText.mockReset().mockResolvedValue(undefined);
    h.mockProposeDimensions.mockReturnValue({ cols: 80, rows: 24 });
  });

  afterEach(() => {
    cleanup();
    resetCliProfileRegistry();
    TerminalRegistry._reset();
    delete window.__slterm_windowFocused;
  });

  it("eventToStatus 经 useXterm 事件路径真实调用（spy 入参 + 四态/会话写入）", async () => {
    const eventToStatusSpy = vi.spyOn(
      mockCliProfile.capabilities.hooks!,
      "eventToStatus",
    );
    const panelId = "ac4-hook";
    renderHook(() =>
      useXterm({
        container,
        cols: 80,
        rows: 24,
        panelId,
        onTabStateChange,
      }),
    );
    // PTY spawn 完成 + TerminalRegistry 注册（主 effect 内 onAgentEvent 同步注册）
    await waitFor(() => expect(TerminalRegistry.get(panelId)).toBeDefined());
    expect(h.mockOnAgentEvent).toHaveBeenCalled();
    expect(h.agentEventCallbackRef.current).not.toBeNull();

    // 清除 spawn 成功时 resetCommandState 的 {active:false} 调用
    onTabStateChange.mockClear();
    act(() => {
      h.agentEventCallbackRef.current!(
        makeAgentPayload({
          panelId,
          event: "UserPromptSubmit",
          cliId: mockCliProfile.id,
        }),
      );
    });

    // MC-403: 四态映射委托 profile.hooks.eventToStatus（入参 = event + notificationType）
    expect(eventToStatusSpy).toHaveBeenCalledWith("UserPromptSubmit", null);
    expect(onTabStateChange).toHaveBeenCalledWith({
      active: true,
      status: "working",
    });
    // PF2-FE-04: 非 SessionEnd 事件 → setAgentSession 携 sessionId/usageSourcePath/status
    const session = TerminalRegistry.get(panelId)?.agentSession;
    expect(session?.sessionId).toBe("s1");
    expect(session?.usageSourcePath).toBe("/t.json");
    expect(session?.status).toBe("working");
    eventToStatusSpy.mockRestore();
  });

  it("classifyNotification 经通知调度路径真实调用（spy 入参 + toast 派发）", async () => {
    const classifySpy = vi.spyOn(
      mockCliProfile.capabilities.hooks!,
      "classifyNotification",
    );
    // 失焦门控：窗口未聚焦才触发通知
    window.__slterm_windowFocused = false;
    renderHook(() => useAgentNotifications());
    expect(h.mockOnAgentEvent).toHaveBeenCalled();

    act(() => {
      h.agentEventCallbackRef.current!(
        makeAgentPayload({
          event: "Stop",
          cliId: mockCliProfile.id,
        }),
      );
    });

    // 类别判定委托 profile.hooks.classifyNotification（MC-420，payload 原样透传）
    expect(classifySpy).toHaveBeenCalledTimes(1);
    expect(classifySpy).toHaveBeenCalledWith(
      expect.objectContaining({ event: "Stop", cliId: mockCliProfile.id }),
    );
    // 桩返回 "done" → 任务完成类别 toast 派发
    await waitFor(() => expect(h.mockSendToastNotification).toHaveBeenCalled());
    const [title, opts] = h.mockSendToastNotification.mock
      .calls[0] as [string, { body: string }];
    expect(title).toBe("slTerminal");
    expect(opts.body).toContain("任务完成");
    classifySpy.mockRestore();
  });

  it("computeUsagePercent 经行渲染路径真实调用（spy 入参 + 桩值 42 区别于 claude 官方口径）", () => {
    const computeSpy = vi.spyOn(
      mockCliProfile.capabilities.hooks!,
      "computeUsagePercent",
    );
    // mockcli 行（usage 传 99——桩恒 42 证明百分比口径由 profile 策略决定，非通用层硬编码）
    const row = {
      panelId: "terminal-page1-0",
      pageId: "page1",
      projectId: "proj-1",
      cliId: mockCliProfile.id,
      title: "mockcli 会话",
      status: "working" as AgentStatus,
      lastEventAt: Date.now(),
      usage: { usedPercentage: 99 },
    };
    // NAV-08：渲染等价路径 = NavSessionRow（导航树活跃会话行，口径委托不变）
    const { container } = render(
      React.createElement(NavSessionRow, { row, active: true, onFocus: vi.fn() }),
    );
    const rowEl = container.querySelector(
      '[data-e2e="nav-row-session"]',
    ) as HTMLElement;
    expect(computeSpy).toHaveBeenCalledWith({ usedPercentage: 99 });
    // 桩值 42%——百分比策略由 profile 能力域决定（新增 CLI 自带百分比口径）
    expect(rowEl.textContent).toContain("42%");
    computeSpy.mockRestore();
  });
});

// ═══════════════════════════════════════════════════════════════
// AC-4③ 历史聚合 UI（mock 条目 + 行 logo 按 session.cliId）
// ═══════════════════════════════════════════════════════════════

describe("AC-4③ 历史聚合 UI", () => {
  const sessions = [
    makeHistorySession({
      sessionId: "mock-s1",
      cwd: "D:\\mock",
      title: "mockcli 会话",
      cwdExists: true,
    }),
  ];

  beforeEach(() => {
    registerMockCliProfile();
    TerminalRegistry._reset();
  });

  afterEach(() => {
    cleanup();
    resetCliProfileRegistry();
    TerminalRegistry._reset();
  });

  it("mock 条目出现在历史区 + 行 logo 按 session.cliId 取 mockcli iconSrc（不依赖 status）", () => {
    // F9 行为修订：status 无命中（undefined → 灰档圆点）→ 行 logo 仍渲染（跟随会话名显示）
    const { getByText, container } = render(
      React.createElement(NavHistoryRow, {
        session: sessions[0],
        status: undefined,
        onDoubleClick: () => {},
        onContextMenu: () => {},
      }),
    );
    // mock 条目（cliId="mockcli"）行标题渲染（导航树历史行——NAV-08 承接）
    expect(getByText("mockcli 会话")).toBeTruthy();
    // 行 logo（MC-311）：cliProfileRegistry.get(session.cliId)?.iconSrc —— mockcli 命中
    const row = container.querySelector('[data-e2e="nav-row-session"]');
    expect(row).not.toBeNull();
    const logo = row!.querySelector("img");
    expect(logo?.getAttribute("src")).toBe(mockCliProfile.iconSrc);
  });
});

// ═══════════════════════════════════════════════════════════════
// AC-4④ hub 选择行（claude + mockcli，MC-502~507 全链）
// ═══════════════════════════════════════════════════════════════

describe("AC-4④ hub 选择行", () => {
  beforeEach(() => {
    h.mockReadHooksConfig.mockReset();
    h.mockWriteHooksConfig.mockReset();
    h.mockConfirmDialog.mockReset().mockResolvedValue(true);
    h.mockJsonMode.mockClear();
    h.mockInject.mockReset();
    h.mockUninstall.mockReset();
    h.mockGetInjectionStatus
      .mockReset()
      .mockResolvedValue({ status: "notInjected", version: null });
    h.mockApi.updateParameters.mockReset();
    h.mockApi.getParameters.mockReset().mockReturnValue({});
    h.mockContainerApi.toJSON.mockReset().mockReturnValue({ mockLayout: true });
    resetStores();
    registerOnly([claudeProfile, mockCliProfile]);
  });

  afterEach(() => {
    cleanup();
    resetCliProfileRegistry();
    resetStores();
  });

  it("两枚按钮（claude + mockcli，均 hasConfigEditor=true）", async () => {
    h.mockReadHooksConfig.mockResolvedValue({});
    const { getByRole } = renderPanel();
    expect(await waitFor(() => getByRole("button", { name: "claude" }))).toBeTruthy();
    expect(getByRole("button", { name: mockCliProfile.displayName })).toBeTruthy();
  });

  it("点击切换 → mock 桩编辑器渲染（双向分派）+ selectedCli 持久化（updateParameters + 显式布局保存）", async () => {
    h.mockReadHooksConfig.mockResolvedValue({});
    const { getByRole, container } = renderPanel({ panelId: "hooksConfig-page-1" });
    // 初始缺省回退首个有能力 CLI = claude → claude 编辑器渲染（JsonMode 被调用）
    await waitFor(() => expect(h.mockJsonMode.mock.calls.length).toBeGreaterThan(0));
    expect(h.mockReadHooksConfig.mock.calls[0][0]).toBe(CLAUDE_CLI_ID);
    // 双向分派（claude 方向）：桩标记不存在
    expect(container.querySelector('[data-e2e="mockcli-config-editor"]')).toBeNull();
    h.mockApi.updateParameters.mockClear();
    h.mockContainerApi.toJSON.mockClear();
    h.mockJsonMode.mockClear();

    // 点击 mockcli → 编辑器槽按 profile.configEditor 分派 = mock 桩组件
    fireEvent.click(getByRole("button", { name: mockCliProfile.displayName }));
    await waitFor(() =>
      expect(
        container.querySelector('[data-e2e="mockcli-config-editor"]'),
      ).toBeTruthy(),
    );
    // 双向分派（mockcli 方向）：桩渲染 → claude 编辑器（JsonMode）零调用
    expect(h.mockJsonMode.mock.calls.length).toBe(0);
    // MC-503: updateParameters 写入 params.selectedCli
    expect(h.mockApi.updateParameters).toHaveBeenCalledWith({
      panelId: "hooksConfig-page-1",
      selectedCli: mockCliProfile.id,
    });
    // 显式 onLayoutChange(saveLayout(containerApi))——updateParameters 不触发
    // onDidLayoutChange，必须显式保存（toJSON 被调用即 saveLayout 序列化执行）
    expect(h.mockContainerApi.toJSON).toHaveBeenCalled();
  });

  it("持久化恢复：params.selectedCli=mockcli 挂载恢复选中 + 高亮 + 桩渲染", async () => {
    h.mockReadHooksConfig.mockResolvedValue({});
    const { getByRole, container } = renderPanel({
      panelId: "hooksConfig-page-1",
      selectedCli: mockCliProfile.id,
    });
    // 挂载即选中 mockcli → 编辑器槽按 mockcli 分派 = 桩组件（非默认回退 claude）
    await waitFor(() =>
      expect(
        container.querySelector('[data-e2e="mockcli-config-editor"]'),
      ).toBeTruthy(),
    );
    // 双向分派：claude 编辑器（JsonMode）零调用
    expect(h.mockJsonMode.mock.calls.length).toBe(0);
    const mockBtn = (await waitFor(() =>
      getByRole("button", { name: mockCliProfile.displayName }),
    )) as HTMLButtonElement;
    const claudeBtn = getByRole("button", { name: "claude" }) as HTMLButtonElement;
    expect(mockBtn.style.background).toBe(hexToRgb(EXPLORER_SELECTION_BG));
    expect(claudeBtn.style.background).toBe("transparent");
  });

  it("选中 claude → claude 编辑器渲染（JsonMode 被调用）+ 桩标记不存在 + 保存透传", async () => {
    h.mockReadHooksConfig.mockResolvedValue({});
    const { container } = renderPanel({
      panelId: "hooksConfig-page-1",
      // 显式选中 claude（非 mockcli）——claude 编辑器 = ClaudeHooksConfigEditor
      selectedCli: CLAUDE_CLI_ID,
    });
    // 双向分派（claude 方向）：JsonMode 被调用 + 桩标记不存在
    await waitFor(() => expect(h.mockJsonMode.mock.calls.length).toBeGreaterThan(0));
    expect(h.mockReadHooksConfig.mock.calls[0][0]).toBe(CLAUDE_CLI_ID);
    expect(container.querySelector('[data-e2e="mockcli-config-editor"]')).toBeNull();
    // 合法编辑 → 保存成功 → 提示条文案 = claude profile 的 restartHint（MC-506）
    act(() => {
      const props = h.mockJsonMode.mock.calls[
        h.mockJsonMode.mock.calls.length - 1
      ] as unknown as [
        {
          onChange: (t: string) => void;
          onValidationChange: (v: boolean, d: unknown[]) => void;
        },
      ];
      props[0].onChange(
        JSON.stringify({ PreToolUse: [{ hooks: [{ type: "command", command: "x" }] }] }),
      );
      props[0].onValidationChange(true, []);
    });
    fireEvent.click(
      container.querySelector('[data-e2e="hooks-save"]') as HTMLButtonElement,
    );
    await waitFor(() =>
      expect(
        container.querySelector('[data-e2e="hooks-restart-hint"]'),
      ).toBeTruthy(),
    );
    const hint = container.querySelector(
      '[data-e2e="hooks-restart-hint"]',
    ) as HTMLElement;
    expect(hint.textContent).toContain(
      claudeProfile.capabilities.hooks!.restartHint,
    );
    // 保存携选中态 cliId（claude）
    expect(h.mockWriteHooksConfig).toHaveBeenCalledTimes(1);
    expect(h.mockWriteHooksConfig.mock.calls[0][0]).toBe(CLAUDE_CLI_ID);
  });
});

// ═══════════════════════════════════════════════════════════════
// AC-4⑤ 恢复注入（mock buildRestoreInput 桩输出）
// ═══════════════════════════════════════════════════════════════

describe("AC-4⑤ 恢复注入", () => {
  let apiStub: { addPanel: ReturnType<typeof vi.fn> };
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    registerMockCliProfile();
    TerminalRegistry._reset();
    resetStores();
    h.mockSwitchToPageShared.mockReset().mockResolvedValue(undefined);
    h.mockGetPageApi.mockReset();
    h.mockPtyWrite.mockReset().mockResolvedValue(undefined);
    h.mockSendToastNotification.mockReset();
    apiStub = {
      addPanel: vi.fn((args: { id: string }) => {
        // 模拟真实 Dockview addPanel → TerminalRegistry 注册（恢复编排 waitFor 轮询命中）
        TerminalRegistry.register(args.id, {
          term: new Terminal() as unknown as Terminal,
          sessionId: "session-test-1",
          webglAddon: null,
          fitAddon: fitStub,
        });
      }),
    };
    h.mockGetPageApi.mockReturnValue(apiStub);
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    resetCliProfileRegistry();
    TerminalRegistry._reset();
    consoleErrorSpy.mockRestore();
  });

  it("注入内容 = mock buildRestoreInput 桩输出 + addPanel title = tabTitle", async () => {
    await restoreHistorySession(makeHistorySession({ cliId: mockCliProfile.id }));

    // addPanel title 取自 profile.tabTitle（MC-315 委托）
    expect(apiStub.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({ title: mockCliProfile.tabTitle }),
    );
    // 注入内容 = mockcli history 能力输出（可识别前缀 "mockcli --resume"）
    const [, , data] = h.mockPtyWrite.mock.calls[0] as [
      string,
      string,
      Uint8Array,
    ];
    expect(new TextDecoder().decode(data)).toBe(
      `mockcli --resume ${SESSION_ID}\r`,
    );
  });

  it("fork 变体：注入内容追加 --fork-session", async () => {
    await restoreHistorySession(
      makeHistorySession({ cliId: mockCliProfile.id }),
      { fork: true },
    );
    const [, , data] = h.mockPtyWrite.mock.calls[0] as [
      string,
      string,
      Uint8Array,
    ];
    expect(new TextDecoder().decode(data)).toBe(
      `mockcli --resume ${SESSION_ID} --fork-session\r`,
    );
  });
});
