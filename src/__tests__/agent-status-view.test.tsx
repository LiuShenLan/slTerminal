// agent-status-view.test.tsx — AgentStatusView L2 测试
//
// 覆盖：三态渲染（no-root / empty / ready）、多行渲染、
// 行点击 switchToPage + focus、用量条正常/降级（官方 used_percentage 口径）、
// 切换项目清空行、完整 usage 行行 2 断言（NAH-05）、活跃区标题覆盖集成
// （NAH-06——真实 useAgentHistory，非 mock history）。
//
// P2-TE-02 + P2-TE-04

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockTerminalRegistry,
  mockOnAgentEventCallback,
  mockSwitchToPageAndFocus,
  mockScanHistory,
} = vi.hoisted(() => {
  let onAgentEventCb: ((payload: unknown) => void) | null = null;
  return {
    mockTerminalRegistry: {
      getAll: vi.fn(() => new Map<string, unknown>()),
      subscribe: vi.fn(() => () => {}),
      _reset: vi.fn(),
      _size: vi.fn(() => 0),
    },
    mockOnAgentEventCallback: {
      set cb(fn: ((payload: unknown) => void) | null) {
        onAgentEventCb = fn;
      },
      get cb() {
        return onAgentEventCb;
      },
      // 手动触发 agent 事件
      trigger(payload: unknown) {
        onAgentEventCb?.(payload);
      },
    },
    mockSwitchToPageAndFocus: vi.fn(),
    // NAH-06：scanHistory 是真实 useAgentHistory 唯一的外部数据源（IPC mock）
    mockScanHistory: vi.fn(),
  };
});

// ── 模块级 mock ──

vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: mockTerminalRegistry,
}));

vi.mock("../ipc/agentHooks", () => ({
  onAgentEvent: vi.fn((cb: (payload: unknown) => void) => {
    mockOnAgentEventCallback.cb = cb;
    return () => {
      mockOnAgentEventCallback.cb = null;
    };
  }),
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageAndFocus: mockSwitchToPageAndFocus,
  getPageApi: vi.fn(() => undefined),
}));

// IC-03：lib/agentStatus 已删除 STATUS_EMOJI/getStatusIcon（仅剩类型导出），
// 状态渲染改 StatusDot——本文件 mock StatusDot 为可识别 span（data-testid=
// "status-dot"，文本 = status 值），只断言接线不依赖其内部 DOM
vi.mock("../lib/StatusDot", async () => {
  const { createElement } = await import("react");
  return {
    StatusDot: ({ status }: { status: string | null }) =>
      status == null
        ? null
        : createElement("span", { "data-testid": "status-dot" }, status),
  };
});

// ── 历史区数据源 mock（NAH-06：useAgentHistory 保持真实——标题覆盖集成测试
//    须走真实 scan → sessions → titleBySessionId 派生链，仅 mock IPC 层 scanHistory；
//    历史区列表交互由 claude-history-view.test.tsx 覆盖）──
vi.mock("../ipc/agentHistory", () => ({
  scanHistory: mockScanHistory,
  deleteHistorySession: vi.fn(),
}));

// restoreSession 依赖 pageApis.switchToPageShared（本文件 mock 不含），
// 历史区交互不在本文件测试范围——整模块 mock 防 import 解析失败
// （Stage 05 目录更名同步，D-05）
vi.mock("../features/agentHistory/restoreSession", () => ({
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
import type { AgentHistorySession } from "../types/agentHistory";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";
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

/** 构建 TerminalRegistry 模拟 Map——entry 含 agentSession 非 null（行建模改后纯 shell 无行） */
function makeTerminalMap(panelIds: string[]): Map<string, unknown> {
  const map = new Map<string, unknown>();
  for (const pid of panelIds) {
    map.set(pid, { agentSession: { lastEventAt: Date.now() } });
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
  mockSwitchToPageAndFocus.mockReset();
  mockOnAgentEventCallback.cb = null;
  // 真实 useAgentHistory 的 scan 默认解析为空数组——未显式配置的测试展开历史区也不会崩
  mockScanHistory.mockReset();
  mockScanHistory.mockResolvedValue([]);
}

/** 构造 AgentSessionRow（cliId 缺省 CLAUDE_CLI_ID——真实注册表 claude profile 的
    computeUsagePercent/iconSrc 生效） */
function makeRow(overrides: Partial<AgentSessionRow> = {}): AgentSessionRow {
  return {
    panelId: "terminal-page1-0",
    pageId: "page1",
    projectId: "proj-1",
    cliId: CLAUDE_CLI_ID,
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
      getByText("当前项目无运行中的编码 CLI 会话"),
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
      usage: { usedPercentage: 50 },
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

  it("状态圆点在行1（StatusDot 透传 status 值）", () => {
    const row = makeRow({ status: "working" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    const dot = line1.querySelector('[data-testid="status-dot"]');
    expect(dot).toBeTruthy();
    expect(dot?.textContent).toBe("working");
  });

  it("status 非 null → 行1 渲染 CLI logo（按 row.cliId 查 profile.iconSrc/16×16/位于图标列内）", () => {
    // 行 cliId = CLAUDE_CLI_ID（真实注册表）→ iconSrc = /cli-icons/claude.png
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

  it("未注册 cliId → 行1 无 logo 不报错（MC-411 降级语义）", () => {
    const row = makeRow({ status: "working", cliId: "unknown-cli" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    expect(line1.querySelector('[data-testid="status-dot"]')?.textContent).toBe("working"); // 圆点仍显示
    expect(line1.querySelector('img[alt="CLI 图标"]')).toBeNull(); // 无 logo
  });

  it("status null → 行1 仍渲染 CLI logo（F9 行为修订：跟随会话名显示，不依赖状态圆点）", () => {
    // ZQ-3 决策 2：status=null 行（无图标）同样有会话名与会话——logo 显示
    const row = makeRow({ status: null });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    expect(line1.querySelector('[data-testid="status-dot"]')).toBeNull(); // 无圆点
    const logoImg = line1.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    // 标题起点不变（图标列空占位保留）
    const titleSpan = Array.from(line1.querySelectorAll("span")).find(
      (s) => s.textContent === "终端 page1",
    );
    expect(titleSpan).toBeTruthy();
  });

  it("status null + 未注册 cliId → 无 logo 不报错（降级语义与 status 无关）", () => {
    const row = makeRow({ status: null, cliId: "unknown-cli" });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );

    const { line1 } = rowChildren(container);
    expect(line1.querySelector('[data-testid="status-dot"]')).toBeNull();
    expect(line1.querySelector('img[alt="CLI 图标"]')).toBeNull();
  });

  it("图标列 = 40px flex 簇（圆点与 logo 列内居中成组）", () => {
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
    // 列内：状态圆点 span（children[0]）+ logo img（children[1]）
    expect(iconCol.children[0]?.getAttribute("data-testid")).toBe("status-dot");
    expect(iconCol.children[0]?.textContent).toBe("working");
    expect(iconCol.children[1]?.getAttribute("alt")).toBe("CLI 图标");
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

describe("用量条（官方 used_percentage 口径）", () => {
  it("官方 used_percentage 23.6 → 取整 24%（round）+ 用量条填充 24%", () => {
    const row = makeRow({ usage: { usedPercentage: 23.6 } });

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

    expect(innerBar.style.width).toBe("24%");
    expect(line2.children[1].textContent).toBe("24%");
  });

  it("round 边界 99.6 → 100%（无 99.6 浮点残留）", () => {
    const row = makeRow({ usage: { usedPercentage: 99.6 } });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    const { line2 } = rowChildren(container);
    expect(line2.children[1].textContent).toBe("100%");
  });

  it("clamp 上界 100.4 → 100%；下界 -5 → 0%（对齐 statusline 参考脚本钳位）", () => {
    const upper = makeRow({ usage: { usedPercentage: 100.4 } });
    const u = render(
      React.createElement(AgentStatusRow, { row: upper, onFocus: vi.fn() }),
    );
    expect(rowChildren(u.container).line2.children[1].textContent).toBe("100%");
    u.unmount();

    const lower = makeRow({ usage: { usedPercentage: -5 } });
    const l = render(
      React.createElement(AgentStatusRow, { row: lower, onFocus: vi.fn() }),
    );
    expect(rowChildren(l.container).line2.children[1].textContent).toBe("0%");
  });

  it("完整 usage 行：相对时间出现（NAH-05）", () => {
    const now = Date.now();
    const row = makeRow({
      title: "完整 usage 会话",
      lastEventAt: now - 10 * 60_000, // 10 分钟前
      usage: { usedPercentage: 75 },
    });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn(), now }),
    );

    const { line2 } = rowChildren(container);
    // formatRelativeTime 相对时间出现（与历史区口径统一）
    expect(line2.textContent).toContain("10 分钟前");
    // 官方 used_percentage 直接渲染（profile 策略取整钳位后 75）
    expect(line2.children[1].textContent).toBe("75%");
  });

  it("usage 为 null → 用量条显示不可用态 '--'", () => {
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

  it("usage 有值但 cliId 无 hooks 能力（computeUsagePercent 缺失）→ 用量条显示不可用态 '--'（MC-412 语义保留）", () => {
    const row = makeRow({
      cliId: "unknown-cli", // 未注册 cliId → profile 缺失 → computeUsagePercent undefined
      usage: { usedPercentage: 50 },
    });

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
    return innerBar.style.backgroundColor.replace(/\s/g, "");  // "rgb(115, 189, 121)"
  }

  /** hex → rgb 字符串，对齐 jsdom rgb() 规范化 */
  function hexToRgbStr(hex: string): string {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r},${g},${b})`;
  }

  it("用量 49% → low token（<50 绿）", () => {
    const row = makeRow({ usage: { usedPercentage: 49 } });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(container)).toBe(hexToRgbStr(AGENT_STATUS_USAGE_COLORS.low));
  });

  it("用量 50% → medium token（≥50 黄，边界含）", () => {
    const row = makeRow({ usage: { usedPercentage: 50 } });
    const { container } = render(
      React.createElement(AgentStatusRow, { row, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(container)).toBe(hexToRgbStr(AGENT_STATUS_USAGE_COLORS.medium));
  });

  it("用量 69% → medium；70% → high token（≥70 橙，边界含）", () => {
    const medium = makeRow({ usage: { usedPercentage: 69 } });
    const m = render(
      React.createElement(AgentStatusRow, { row: medium, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(m.container)).toBe(
      hexToRgbStr(AGENT_STATUS_USAGE_COLORS.medium),
    );
    m.unmount();

    const high = makeRow({ usage: { usedPercentage: 70 } });
    const h = render(
      React.createElement(AgentStatusRow, { row: high, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(h.container)).toBe(
      hexToRgbStr(AGENT_STATUS_USAGE_COLORS.high),
    );
  });

  it("用量 89% → high；90% → critical token（≥90 红，边界含）", () => {
    const high = makeRow({ usage: { usedPercentage: 89 } });
    const h = render(
      React.createElement(AgentStatusRow, { row: high, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(h.container)).toBe(
      hexToRgbStr(AGENT_STATUS_USAGE_COLORS.high),
    );
    h.unmount();

    const critical = makeRow({ usage: { usedPercentage: 90 } });
    const c = render(
      React.createElement(AgentStatusRow, { row: critical, onFocus: vi.fn() }),
    );
    expect(getUsageBarColor(c.container)).toBe(
      hexToRgbStr(AGENT_STATUS_USAGE_COLORS.critical),
    );
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

  it("点击历史区标题展开 → 触发真实 scan()（经 AgentHistorySections 首次展开 effect）", async () => {
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { getByText } = render(
      React.createElement(AgentStatusView, defaultProps),
    );

    // 初始收起 → 未触发
    expect(mockScanHistory).not.toHaveBeenCalled();

    // 展开当前项目历史会话 → 真实 useAgentHistory.scan → scanHistory IPC 一次
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

    // 红线 4b：空态文案「无运行中的编码 CLI 会话」+ 活跃行选择器（ready 态）
    seedProject("C:/test", "proj-1", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockTerminalRegistry.getAll.mockReturnValue(new Map());
    const second = render(React.createElement(AgentStatusView, defaultProps));
    expect(
      second.getByText("当前项目无运行中的编码 CLI 会话"),
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
// 活跃区标题覆盖（NAH-06：真实 useAgentHistory 集成——非 mock history）
// ═══════════════════════════════════════════════════════════════
// 全链路真实执行：TerminalRegistry 注册表 → useAgentStatus 建行（携 sessionId）→
// 展开历史区触发真实 scan() → scanHistory IPC mock 返回 rename 后 title →
// useAgentHistory.sessions 落地 → AgentStatusView.titleBySessionId 覆盖行标题。
// （旧实现 mock useAgentHistory，titleBySessionId 派生链未被端到端验证——NAH-06）

describe("活跃区标题覆盖（问题 6：真实 useAgentHistory 集成）", () => {
  /** 构造带 sessionId 的注册表条目（活跃行 sessionId = 传入值） */
  function seedActiveRowWithSession(sessionId: string | undefined) {
    seedProject("C:/test", "proj-1", [{ pageId: "page1", name: "页面 1" }]);
    seedActivePage("page1");
    const map = new Map<string, unknown>();
    map.set("terminal-page1-0", {
      agentSession: {
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
  ): AgentHistorySession {
    return {
      sessionId,
      cwd: "C:/test",
      title,
      titleSource: title != null ? "customTitle" : "none",
      firstPrompt: null,
      mtimeMs: 1000,
      cwdExists: true,
      // Stage 04 新增必填字段（provider 打标；测试数据恒为 claude 会话）
      cliId: CLAUDE_CLI_ID,
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
