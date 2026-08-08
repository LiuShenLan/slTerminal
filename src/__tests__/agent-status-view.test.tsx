// agent-status-view.test.tsx — AgentStatusView L2 测试
//
// 覆盖：三态渲染（no-root / empty / ready）、多行渲染、
// 行点击 switchToPage + focus、用量条正常/降级、切换项目清空行、
// 完整 usage 行行 2 断言（NAH-05）、活跃区标题覆盖集成（NAH-06——
// 真实 useClaudeHistory，非 mock history）。
//
// P2-TE-02 + P2-TE-04

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockTerminalRegistry,
  mockOnHookEventCallback,
  mockContextUsage,
  mockSwitchToPageAndFocus,
  mockScanHistory,
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
    // NAH-06：scanHistory 是真实 useClaudeHistory 唯一的外部数据源（IPC mock）
    mockScanHistory: vi.fn(),
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
  // null/未识别状态 → ""（与真实实现一致：真实 getStatusIcon 对 null 返回 ""，
  // 否则 status null 行会误渲染 emoji 与 CLI logo）
  getStatusIcon: vi.fn(
    (s: string | null) =>
      s != null ? ({ working: "⚡", attention: "🟡", done: "✅", error: "❌" })[s] ?? "" : "",
  ),
  // eventToStatus 不在 mock 中（测试不触发 hook 事件回调），
  // 设为 stub 防 import 时 undefined
  eventToStatus: vi.fn(() => "attention"),
  // STATUS_EMOJI 必须保留真实值——HistorySessionRow 直接访问
  // STATUS_EMOJI[status]（历史区与活跃区四态同源），缺失会抛 TypeError
  STATUS_EMOJI: { working: "⚡", attention: "🟡", done: "✅", error: "❌" },
}));

// ── 历史区数据源 mock（NAH-06：useClaudeHistory 保持真实——标题覆盖集成测试
//    须走真实 scan → sessions → titleBySessionId 派生链，仅 mock IPC 层 scanHistory；
//    历史区列表交互由 claude-history-view.test.tsx 覆盖）──
vi.mock("../ipc/claudeHistory", () => ({
  scanHistory: mockScanHistory,
  deleteHistorySession: vi.fn(),
}));

// restoreSession 依赖 pageApis.switchToPageShared（本文件 mock 不含），
// 历史区交互不在本文件测试范围——整模块 mock 防 import 解析失败
vi.mock("../features/claudeHistory/restoreSession", () => ({
  restoreHistorySession: vi.fn(() => Promise.resolve()),
}));

import React from "react";
import {
  render,
  cleanup,
  fireEvent,
  act,
  waitFor,
} from "@testing-library/react";
import { AgentStatusView } from "../features/agentStatus/AgentStatusView";
import { AgentStatusRow } from "../features/agentStatus/AgentStatusRow";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { AgentSessionRow } from "../features/agentStatus/useAgentStatus";
import type { HistorySession } from "../types/claudeHistory";
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
  // 真实 useClaudeHistory 的 scan 默认解析为空数组——未显式配置的测试展开历史区也不会崩
  mockScanHistory.mockReset();
  mockScanHistory.mockResolvedValue([]);
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

  it("status 非 null → 行1 渲染 CLI logo（src=claude 条目/16×16/位于图标列内）", () => {
    const row = makeRow({ status: "working" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    const logoImg = line1.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("16");
    expect(logoImg?.getAttribute("height")).toBe("16");
  });

  it("status null → 行1 无 CLI logo（仅随 emoji 显示）", () => {
    const row = makeRow({ status: null });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    expect(line1.textContent).not.toContain("⚡");
    expect(line1.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("图标列 = 40px flex 簇（emoji 与 logo 列内居中成组）", () => {
    const row = makeRow({ status: "working" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    const iconCol = line1.children[0] as HTMLElement;
    expect(iconCol.style.width).toBe("40px");
    expect(iconCol.style.display).toBe("flex");
    expect(iconCol.style.alignItems).toBe("center");
    expect(iconCol.style.justifyContent).toBe("center");
    // 列内：emoji 文本节点（不入 children）+ logo img（children[0]）
    expect(iconCol.firstChild?.textContent).toBe("⚡");
    expect(iconCol.children[0]?.getAttribute("alt")).toBe("CLI 图标");
  });

  it("行2 paddingLeft = 48px（对齐行1图标列 40 + gap 8，用量条与标题起点对齐）", () => {
    const row = makeRow({ title: "对齐守卫" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line2 } = rowChildren(container);
    // 行1 标题起点 = 40px 列 + 8px gap
    expect(line2.style.paddingLeft).toBe("48px");
    // 行2 无 logo（logo 只在行1 图标列内）
    expect(line2.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("now prop：lastEventAt 固定，now 推进 → 时间文本重算（问题 1b 定时刷新）", () => {
    const lastEventAt = 1_000_000_000_000;
    const row = makeRow({ lastEventAt });
    const { container, rerender } = render(
      React.createElement(AgentStatusRow, {
        row,
        onFocus: vi.fn(),
        now: 1_000_000_060_000, // +60s → 1 分钟前
      }),
    );
    expect(rowChildren(container).line2.textContent).toContain("1 分钟前");

    rerender(
      React.createElement(AgentStatusRow, {
        row,
        onFocus: vi.fn(),
        now: 1_000_000_900_000, // +15min → 15 分钟前
      }),
    );
    expect(rowChildren(container).line2.textContent).toContain("15 分钟前");
  });

  it("now 缺省（undefined）→ 回退 Date.now()（向后兼容，可正常渲染）", () => {
    const row = makeRow({ lastEventAt: Date.now() });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(rowChildren(container).line2.textContent).toContain("刚刚");
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

  it("完整 usage 行（含 outputTokens）：百分比排除 outputTokens 不计占用 + 相对时间出现（NAH-05）", () => {
    const now = Date.now();
    const row = makeRow({
      title: "完整 usage 会话",
      lastEventAt: now - 10 * 60_000, // 10 分钟前
      usage: {
        inputTokens: 100_000,
        outputTokens: 999_999, // 信息字段——不计占用、不展示（组件设计，见 claudeHistory/CLAUDE.md）
        cacheReadInputTokens: 30_000,
        cacheCreationInputTokens: 20_000,
      },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn(), now }),
    );

    const { line2 } = rowChildren(container);
    // formatRelativeTime 相对时间出现（与历史区口径统一）
    expect(line2.textContent).toContain("10 分钟前");
    // 总占用 = input + cacheRead + cacheCreation = 150_000 / 200_000 → 75%
    // ——outputTokens 不计入占用（守卫用量口径）
    expect(line2.children[1].textContent).toBe("75%");
    // outputTokens 为信息字段不渲染进行 2（行 2 仅用量条 + 百分比 + 相对时间）
    expect(line2.textContent).not.toContain("999999");
    expect(line2.textContent).not.toContain("out");
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

describe("now ticker 透传（问题 1b：idle 会话时间自动推进）", () => {
  it("AgentStatusView 行时间随 60s ticker 推进（useAgentStatus now → AgentStatusRow now 全链路）", () => {
    vi.useFakeTimers();
    try {
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
      // lastEventAt = 建行时刻（= fake now）→ 初始「刚刚」
      expect(rowChildren(container).line2.textContent).toContain("刚刚");

      // 推进 65s → now ticker 触发重渲染 → 行时间变「1 分钟前」（无需任何 hook 事件）
      act(() => {
        vi.advanceTimersByTime(65_000);
      });
      expect(rowChildren(container).line2.textContent).toContain("1 分钟前");
    } finally {
      vi.useRealTimers();
    }
  });
});

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
    expect(mockScanHistory).not.toHaveBeenCalled();
  });

  it("点击历史区标题展开 → 触发真实 scan()（经 ClaudeHistorySections 首次展开 effect）", async () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { getByText } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    // 初始收起 → 未触发
    expect(mockScanHistory).not.toHaveBeenCalled();

    // 展开当前项目历史会话 → 真实 useClaudeHistory.scan → scanHistory IPC 一次
    fireEvent.click(getByText("当前项目历史会话"));
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalledTimes(1);
    });

    // 收起再展开 → 仅首次触发（scanTriggeredRef）
    fireEvent.click(getByText("当前项目历史会话"));
    fireEvent.click(getByText("全部项目历史会话"));
    expect(mockScanHistory).toHaveBeenCalledTimes(1);
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

// ═══════════════════════════════════════════════════════════════
// 活跃区标题覆盖（NAH-06：真实 useClaudeHistory 集成——非 mock history）
// ═══════════════════════════════════════════════════════════════
// 全链路真实执行：TerminalRegistry 注册表 → useAgentStatus 建行（携 sessionId）→
// 展开历史区触发真实 scan() → scanHistory IPC mock 返回 rename 后 title →
// useClaudeHistory.sessions 落地 → AgentStatusView.titleBySessionId 覆盖行标题。
// （旧实现 mock useClaudeHistory，titleBySessionId 派生链未被端到端验证——NAH-06）

describe("活跃区标题覆盖（问题 6：真实 useClaudeHistory 集成）", () => {
  /** 构造带 sessionId 的注册表条目（活跃行 sessionId = 传入值） */
  function seedActiveRowWithSession(sessionId: string | undefined) {
    seedProject("C:/test", "proj-1", [{ pageId: "page1", name: "页面 1" }]);
    seedActivePage("page1");
    const map = new Map<string, unknown>();
    map.set("terminal-page1-0", {
      claudeSession: {
        sessionId,
        lastEventAt: Date.now(),
        status: "attention",
      },
    });
    mockTerminalRegistry.getAll.mockReturnValue(map);
  }

  /** 构造 scanHistory 返回的历史会话 */
  function makeHistorySession(
    sessionId: string,
    title: string | null,
  ): HistorySession {
    return {
      sessionId,
      cwd: "C:/test",
      title,
      titleSource: title != null ? "customTitle" : "none",
      firstPrompt: null,
      mtimeMs: 1000,
      cwdExists: true,
    };
  }

  /** 渲染视图并展开历史区触发真实 scan */
  async function renderAndTriggerScan() {
    const utils = render(
      React.createElement(AgentStatusView, defaultProps),
    );
    fireEvent.click(utils.getByText("当前项目历史会话"));
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalledTimes(1);
    });
    return utils;
  }

  it("scan 结果同 sessionId 且 title 非 null → 活跃区行标题被覆盖（/rename 后刷新同步）", async () => {
    seedActiveRowWithSession("s-rename-1");
    mockScanHistory.mockResolvedValue([
      makeHistorySession("s-rename-1", "rename 后的新标题"),
    ]);

    const { container } = await renderAndTriggerScan();

    // scan 落地 → titleBySessionId 覆盖活跃区行标题
    const row = container.querySelector(
      '[data-e2e="agent-status-row"]',
    ) as HTMLElement;
    await waitFor(() => {
      expect(row.textContent).toContain("rename 后的新标题");
    });
    // 原行标题（dockview 面板标题回退值）被覆盖
    expect(row.textContent).not.toContain("终端 page1");
  });

  it("scan 无匹配 sessionId → 回退原标题（不覆盖）", async () => {
    seedActiveRowWithSession("s-row-1");
    mockScanHistory.mockResolvedValue([
      makeHistorySession("s-other", "别的会话标题"),
    ]);

    const { container } = await renderAndTriggerScan();

    const row = container.querySelector(
      '[data-e2e="agent-status-row"]',
    ) as HTMLElement;
    // 行 sessionId（s-row-1）不在 scan 结果中 → 保留 resolveTitle 回退原标题
    expect(row.textContent).toContain("终端 page1");
    expect(row.textContent).not.toContain("别的会话标题");
  });
});
