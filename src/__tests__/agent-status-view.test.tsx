// agent-status-view.test.tsx — AgentStatusView L2 测试
//
// 覆盖：三态渲染（no-root / empty / ready）、多行渲染、
// 行点击 switchToPage + focus、用量条正常/降级、切换项目清空行。
//
// P2-TE-02 + P2-TE-04

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockTerminalRegistry,
  mockOnHookEventCallback,
  mockContextUsage,
  mockSwitchToPageAndFocus,
} = vi.hoisted(() => {
  let onHookEventCb: ((payload: unknown) => void) | null = null;
  return {
    mockTerminalRegistry: {
      getAll: vi.fn(() => new Map<string, unknown>()),
      subscribe: vi.fn(() => () => {}),
      _reset: vi.fn(),
      _size: vi.fn(() => 0),
    },
    mockOnHookEventCallback: {
      set cb(fn: ((payload: unknown) => void) | null) {
        onHookEventCb = fn;
      },
      get cb() {
        return onHookEventCb;
      },
      // 手动触发 hook 事件
      trigger(payload: unknown) {
        onHookEventCb?.(payload);
      },
    },
    mockContextUsage: vi.fn(),
    mockSwitchToPageAndFocus: vi.fn(),
  };
});

// ── 模块级 mock ──

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: mockTerminalRegistry,
}));

vi.mock("../ipc/hooks", () => ({
  onHookEvent: vi.fn((cb: (payload: unknown) => void) => {
    mockOnHookEventCallback.cb = cb;
    return () => {
      mockOnHookEventCallback.cb = null;
    };
  }),
  contextUsage: mockContextUsage,
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageAndFocus: mockSwitchToPageAndFocus,
  getPageApi: vi.fn(() => undefined),
}));

vi.mock("../lib/claudeStatus", () => ({
  getStatusIcon: vi.fn(
    (s: string) =>
      ({ working: "⚡", attention: "🟡", done: "✅", error: "❌" })[s] ?? "🟡",
  ),
  // eventToStatus 不在 mock 中（测试不触发 hook 事件回调），
  // 设为 stub 防 import 时 undefined
  eventToStatus: vi.fn(() => "attention"),
}));

// ── 历史区 mock（FE-08：本文件聚焦 AgentStatusView 三区结构；
//    历史区内部行为——搜索/菜单/双击分派——由 claude-history-view.test.tsx 覆盖）──
const historyMock = vi.hoisted(() => ({
  shape: {
    state: "ready",
    sessions: [] as unknown[],
    activeStatuses: new Map<string, string>(),
    rootPath: "C:/test",
  },
  scan: vi.fn(() => Promise.resolve()),
  removeLocal: vi.fn(),
}));

vi.mock("../features/claudeHistory/useClaudeHistory", () => ({
  useClaudeHistory: () => ({
    state: historyMock.shape.state,
    sessions: historyMock.shape.sessions,
    activeStatuses: historyMock.shape.activeStatuses,
    rootPath: historyMock.shape.rootPath,
    scan: historyMock.scan,
    removeLocal: historyMock.removeLocal,
  }),
}));

// restoreSession 依赖 pageApis.switchToPageShared（本文件 mock 不含），
// 历史区交互不在本文件测试范围——整模块 mock 防 import 解析失败
vi.mock("../features/claudeHistory/restoreSession", () => ({
  restoreHistorySession: vi.fn(() => Promise.resolve()),
}));

import React from "react";
import { render, cleanup, fireEvent } from "@testing-library/react";
import { AgentStatusView } from "../features/agentStatus/AgentStatusView";
import { AgentStatusRow } from "../features/agentStatus/AgentStatusRow";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { AgentSessionRow } from "../features/agentStatus/useAgentStatus";
import { AGENT_STATUS_USAGE_COLORS } from "../theme";
// ── 辅助函数 ──

/** 种子 projects store：创建一个含指定页面列表的项目 */
function seedProject(
  rootPath: string,
  projectId: string,
  pages: Array<{ pageId: string; name: string }>,
) {
  useProjects.setState({
    projects: {
      [projectId]: {
        projectId,
        name: "测试项目",
        rootPath,
        pages: pages.map((p) => ({
          pageId: p.pageId,
          name: p.name,
          layout: {},
          cwd: undefined,
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        })),
        activePageId: pages[0]?.pageId ?? null,
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { [projectId]: true },
  });
}

/** 种子 layout store：设置活跃页面 */
function seedActivePage(pageId: string | null) {
  useLayout.setState({ activePageId: pageId });
}

/** 构建 TerminalRegistry 模拟 Map——entry 含 claudeSession 非 null（行建模改后纯 shell 无行） */
function makeTerminalMap(panelIds: string[]): Map<string, unknown> {
  const map = new Map<string, unknown>();
  for (const pid of panelIds) {
    map.set(pid, { claudeSession: { lastEventAt: Date.now() } });
  }
  return map;
}

/** 重置 stores + mocks + window */
function resetAll() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  mockTerminalRegistry.getAll.mockReset();
  mockTerminalRegistry.getAll.mockReturnValue(new Map());
  mockContextUsage.mockReset();
  mockSwitchToPageAndFocus.mockReset();
  mockOnHookEventCallback.cb = null;
  historyMock.scan.mockClear();
}

/** 构造 AgentSessionRow */
function makeRow(overrides: Partial<AgentSessionRow> = {}): AgentSessionRow {
  return {
    panelId: "terminal-page1-0",
    pageId: "page1",
    projectId: "proj-1",
    title: "终端 page1",
    status: "attention",
    lastEventAt: Date.now(),
    usage: undefined,
    ...overrides,
  };
}

// ── 默认 props ──
const defaultProps = {
  switchToPage: vi.fn(),
  onDeletePage: vi.fn(),
};

beforeEach(() => {
  resetAll();
});

afterEach(() => {
  cleanup();
});

// ═══════════════════════════════════════════════════════════════
// AgentStatusView 三态渲染
// ═══════════════════════════════════════════════════════════════

describe("AgentStatusView 三态渲染", () => {
  it("无 rootPath 时显示 no-root 占位文案", () => {
    // 不种子任何 project —— activePageId 为 null → projectRoot 为 null → no-root
    seedActivePage(null);

    const { getByText } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    expect(
      getByText("选择一个项目以查看 Agent 状态"),
    ).toBeTruthy();
  });

  it("当前项目无终端时显示 empty 占位文案", () => {
    // 种子一个项目 + 活跃页面，但 TerminalRegistry 为空
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "操作页面 1" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(new Map());

    const { getByText } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    expect(
      getByText("当前项目无运行中的 claude 会话"),
    ).toBeTruthy();
  });

  it("TerminalRegistry 含两个 panelId 时渲染两行", () => {
    // 一个项目含两个页面，TerminalRegistry 中各有终端
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(
      makeTerminalMap(["terminal-page1-0", "terminal-page2-0"]),
    );

    const { container } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    const rows = container.querySelectorAll('[data-e2e="agent-status-row"]');
    expect(rows.length).toBe(2);
  });
});

// ═══════════════════════════════════════════════════════════════
// 行点击
// ═══════════════════════════════════════════════════════════════

describe("行点击 → switchToPageAndFocus", () => {
  it("点击行时调用 switchToPageAndFocus(pageId, panelId)", async () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(
      makeTerminalMap(["terminal-page1-0"]),
    );

    const { container } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    const row = container.querySelector(
      '[data-e2e="agent-status-row"]',
    ) as HTMLElement;
    expect(row).toBeTruthy();

    fireEvent.click(row);

    // 等待 async handleFocus 完成 → 断言 switchToPageAndFocus 被调用
    await vi.waitFor(() => {
      expect(mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
      );
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 用量条
// ═══════════════════════════════════════════════════════════════

/** 双行布局：row.children[0]=行1（图标+标题），row.children[1]=行2（用量条+百分比+时间） */
function rowChildren(container: HTMLElement) {
  const row = container.querySelector(
    '[data-e2e="agent-status-row"]',
  ) as HTMLElement;
  return { row, line1: row.children[0] as HTMLElement, line2: row.children[1] as HTMLElement };
}

describe("AgentStatusRow 双行布局（问题 1 修复）", () => {
  it("结构断言：标题与用量条不在同一 flex 行（行1 = 图标+标题，行2 = 用量+时间）", () => {
    const row = makeRow({
      title: "修复 context 用量计算",
      usage: {
        inputTokens: 100_000,
        outputTokens: 0,
        cacheReadInputTokens: 0,
        cacheCreationInputTokens: 0,
      },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { row: root, line1, line2 } = rowChildren(container);
    // 根容器 column 布局
    expect(root.style.flexDirection).toBe("column");
    // 行1 = 图标 + 标题
    expect(line1.textContent).toContain("修复 context 用量计算");
    expect(line1.textContent).not.toContain("%");
    // 行2 = 用量条 + 百分比 + 时间（不含标题）
    expect(line2.textContent).toContain("%");
    expect(line2.textContent).not.toContain("修复 context 用量计算");
  });

  it("行1 标题 12px 粗体；行2 11px（问题 4 三级字号层级）", () => {
    const row = makeRow({ title: "标题" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1, line2 } = rowChildren(container);
    const titleEl = Array.from(line1.querySelectorAll("span")).find(
      (s) => s.textContent === "标题",
    ) as HTMLElement;
    expect(titleEl.style.fontSize).toBe("12px");
    expect(titleEl.style.fontWeight).toBe("bold");
    // 行2 字号 11px
    expect(line2.querySelector("span")!.style.fontSize).toBe("11px");
  });

  it("时间 = formatRelativeTime(lastEventAt, now)（与历史区口径统一，mock Date.now 固定）", () => {
    const now = Date.now();
    const lastEventAt = now - 5 * 60_000; // 5 分钟前
    const row = makeRow({ lastEventAt });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line2 } = rowChildren(container);
    // 相对时间格式（5 分钟前）；不含 HH:MM:SS 冒号形态
    expect(line2.textContent).toContain("5 分钟前");
  });

  it("状态图标仍在行1（E2E 兼容：emoji 文本断言）", () => {
    const row = makeRow({ status: "working" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    expect(line1.textContent).toContain("⚡");
  });
});

describe("用量条", () => {
  it("contextUsage 返回正常值 → 用量条填充宽度按 200000 上限计算", () => {
    const row = makeRow({
      usage: {
        inputTokens: 100_000,
        outputTokens: 50_000,
        cacheReadInputTokens: 30_000,
        cacheCreationInputTokens: 20_000,
      },
    });
    // 总 tokens = input + cacheRead + cacheCreation = 150_000，上限 = 200_000 → percent = 75%

    const { container } = render(
      React.createElement(AgentStatusRow, {
        row,
        onFocus: vi.fn(),
      }),
    );

    // 行2 内：children[0]=用量条容器，children[1]=百分比文本，children[2]=时间
    const { line2 } = rowChildren(container);
    const barContainer = line2.children[0] as HTMLElement;
    const innerBar = barContainer.firstElementChild as HTMLElement;

    expect(innerBar.style.width).toBe("75%");
    expect(line2.children[1].textContent).toBe("75%");
  });

  it("contextUsage 返回 null → 用量条显示不可用态 '--'", () => {
    const row = makeRow({ usage: null });

    const { container } = render(
      React.createElement(AgentStatusRow, {
        row,
        onFocus: vi.fn(),
      }),
    );

    const { line2 } = rowChildren(container);
    expect(line2.children[1].textContent).toBe("--");
  });

  it("usage 为 undefined 时同样显示 '--'", () => {
    const row = makeRow({ usage: undefined });

    const { container } = render(
      React.createElement(AgentStatusRow, {
        row,
        onFocus: vi.fn(),
      }),
    );

    const { line2 } = rowChildren(container);
    expect(line2.children[1].textContent).toBe("--");
  });

  /** 获取用量条内层填充 div 的 backgroundColor（rgb 格式，jsdom 自动规范化 hex→rgb） */
  function getUsageBarColor(container: HTMLElement): string {
    const { line2 } = rowChildren(container);
    const barContainer = line2.children[0] as HTMLElement;
    const innerBar = barContainer.firstElementChild as HTMLElement;
    return innerBar.style.backgroundColor.replace(/\s/g, "");  // "rgb(98, 151, 85)"
  }

  /** hex → rgb 字符串，对齐 jsdom rgb() 规范化 */
  function hexToRgbStr(hex: string): string {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r},${g},${b})`;
  }

  it("用量 < 50% → 颜色为 low token (#629755)", () => {
    // 40% = 80_000 / 200_000（input + cacheRead + cacheCreation）
    const row = makeRow({
      usage: {
        inputTokens: 50_000,
        outputTokens: 30_000,
        cacheReadInputTokens: 20_000,
        cacheCreationInputTokens: 10_000,
      },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(container)).toBe(hexToRgbStr(AGENT_STATUS_USAGE_COLORS.low));
  });

  it("用量 50%~80% → 颜色为 medium token (#BBB529)", () => {
    // 60% = 120_000 / 200_000（input + cacheRead + cacheCreation）
    const row = makeRow({
      usage: {
        inputTokens: 70_000,
        outputTokens: 50_000,
        cacheReadInputTokens: 30_000,
        cacheCreationInputTokens: 20_000,
      },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(container)).toBe(hexToRgbStr(AGENT_STATUS_USAGE_COLORS.medium));
  });

  it("用量 > 80% → 颜色为 high token (#F44747)", () => {
    // 90% = 180_000 / 200_000（input + cacheRead + cacheCreation）
    const row = makeRow({
      usage: {
        inputTokens: 100_000,
        outputTokens: 80_000,
        cacheReadInputTokens: 40_000,
        cacheCreationInputTokens: 40_000,
      },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(container)).toBe(hexToRgbStr(AGENT_STATUS_USAGE_COLORS.high));
  });
});

// ═══════════════════════════════════════════════════════════════
// 切换项目清空行
// ═══════════════════════════════════════════════════════════════

describe("切换 activePageId 清空行", () => {
  it("切换 activePageId 到另一项目 → 行列表清空", () => {
    // 先种子项目 A 并渲染
    seedProject("C:/projA", "proj-A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockTerminalRegistry.getAll.mockReturnValue(
      makeTerminalMap(["terminal-pageA-0"]),
    );

    const { container, rerender } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    // 项目 A 应有一行
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(1);

    // 切换到项目 B
    seedProject("C:/projB", "proj-B", [
      { pageId: "pageB", name: "页面 B" },
    ]);
    seedActivePage("pageB");
    mockTerminalRegistry.getAll.mockReturnValue(new Map());

    rerender(React.createElement(AgentStatusView, defaultProps));

    // 项目 B 无终端 → empty 态，行数为 0
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(0);
  });
});

// ═══════════════════════════════════════════════════════════════
// 三下拉框结构（FE-08）
// ═══════════════════════════════════════════════════════════════

describe("三下拉框结构（FE-08）", () => {
  it("三个区块头存在：活跃会话 + 当前项目历史会话 + 全部项目历史会话", () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { getByText, container } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    expect(getByText("活跃会话")).toBeTruthy();
    expect(getByText("当前项目历史会话")).toBeTruthy();
    expect(getByText("全部项目历史会话")).toBeTruthy();

    // 两个历史区块容器（FE-12）
    expect(
      container.querySelector('[data-e2e="agent-history-section-current"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-e2e="agent-history-section-all"]'),
    ).toBeTruthy();
  });

  it("默认态：活跃展开、两历史区收起——活跃行可见、无历史行、scan 未触发", () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(
      makeTerminalMap(["terminal-page1-0"]),
    );

    const { container } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    // 活跃区展开：行可见
    expect(
      container.querySelectorAll('[data-e2e="agent-status-row"]').length,
    ).toBe(1);
    // 历史区收起：无历史行、scan 未触发
    expect(
      container.querySelectorAll('[data-e2e="agent-history-row"]').length,
    ).toBe(0);
    expect(historyMock.scan).not.toHaveBeenCalled();
  });

  it("点击历史区标题展开 → 触发 scan()（经 ClaudeHistorySections 首次展开 effect）", () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { getByText } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    // 初始收起 → 未触发
    expect(historyMock.scan).not.toHaveBeenCalled();

    // 展开当前项目历史会话 → scan 一次
    fireEvent.click(getByText("当前项目历史会话"));
    expect(historyMock.scan).toHaveBeenCalledTimes(1);

    // 收起再展开 → 仅首次触发（scanTriggeredRef）
    fireEvent.click(getByText("当前项目历史会话"));
    fireEvent.click(getByText("全部项目历史会话"));
    expect(historyMock.scan).toHaveBeenCalledTimes(1);
  });

  it("E2E 兼容红线四件：agent-status-view / agent-status-row / AGENT STATUS / 两条空态文案", () => {
    // 红线 1+2+3：根容器 + 标题栏文本（no-root 态）
    seedActivePage(null);
    const first = render(React.createElement(AgentStatusView, defaultProps));
    expect(
      first.container.querySelector('[data-e2e="agent-status-view"]'),
    ).toBeTruthy();
    expect(first.getByText("AGENT STATUS")).toBeTruthy();
    // 红线 4a：空态文案「选择一个项目」
    expect(
      first.getByText("选择一个项目以查看 Agent 状态"),
    ).toBeTruthy();
    first.unmount();

    // 红线 4b：空态文案「无运行中的 claude 会话」+ 活跃行选择器（ready 态）
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(new Map());
    const second = render(React.createElement(AgentStatusView, defaultProps));
    expect(
      second.getByText("当前项目无运行中的 claude 会话"),
    ).toBeTruthy();
    second.unmount();

    // 红线：活跃行 data-e2e="agent-status-row"（ready 态）
    mockTerminalRegistry.getAll.mockReturnValue(
      makeTerminalMap(["terminal-page1-0"]),
    );
    const third = render(React.createElement(AgentStatusView, defaultProps));
    expect(
      third.container.querySelector('[data-e2e="agent-status-row"]'),
    ).toBeTruthy();
  });
});
