// nav-tree.test.tsx — 导航树 NavTree L2 测试（NAV-10 L2 部分）
//
// 规格契约（写死于 docs/ui-redesign-impl/workflows/stage-06-sidebar.js 脚本头
// + checklist.md NAV-01/02/03/04/09 条目 + stages.md Stage 06 实现要点）：
//   - data-e2e 选择器：nav-tree / nav-row-project / nav-row-page / nav-row-session / nav-history-node
//   - 三级层级：项目→页面→会话；活跃会话经 panelId→pageId 挂页面下
//   - 行高 28px（会话行 30px）、圆角 5px；hover #222227（SIDEBAR_COLORS.hover）
//   - 选中态：ACTIVE_SELECTION_BG 底（rgba(110,159,242,0.13)）+ fg-1（SIDEBAR_FG）；
//     选中行 hover → SELECTION_HOVER_BG（rgba(110,159,242,0.22)）
//   - 活跃会话行构成（NAV-02）：StatusDot + CLI logo 14px + 标题 +
//     右侧 32×3 迷你用量条 + 百分比 11px fg-4（DIM_FG）
//   - 「当前」pill（NAV-09）：ACTIVE_SELECTION_BG 底 + ACCENT_FG 字 10px（当前活跃项目）；
//     计数 pill #1a1a1e 底（SIDEBAR_BG）+ fg-4（PLACEHOLDER_FG）
//   - 搜索框（NAV-04）：分组标题「导航」+ 占位「搜索项目 / 页面 / 会话…」；
//     子串不区分大小写过滤，父节点因子命中而显示
//   - 右键菜单（NAV-06）：承接 SidebarTree 菜单但删除「打开 Hooks 配置」项（入口唯一化）
//
// Mock 策略（照 src/__tests__/ 既有种子模式——agent-status-view.test.tsx 先例）：
//   - useAgentStatus / ipc/agentHistory（scanAgentHistory）模块级 vi.mock + vi.hoisted 状态；
//   - stores/projects、stores/layout 真实 store + setState 种子；
//   - StatusDot mock 为可识别 span（data-testid="status-dot"，只断言接线不依赖其内部 DOM）；
//   - TerminalRegistry / workspace/pageApis / restoreSession mock。
// NavTree 本体由 navtree-new agent 按上述契约产出（本 Stage 并行 agent），本文件只锁定契约点，
// 不依赖其内部实现细节（className 等）。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ─── Hoisted mocks（防 store/组件 import 时加载真实依赖） ───
const {
  mockUseAgentStatus,
  mockSwitchToPageAndFocus,
  mockScanHistory,
  mockWriteText,
  mockSendToast,
  mockTerminalGetAll,
  mockRestoreHistorySession,
  mockConfirmDialog,
} = vi.hoisted(() => ({
  mockUseAgentStatus: vi.fn(),
  mockSwitchToPageAndFocus: vi.fn(),
  mockScanHistory: vi.fn(),
  mockWriteText: vi.fn(() => Promise.resolve()),
  mockSendToast: vi.fn(),
  mockTerminalGetAll: vi.fn(() => new Map()),
  mockRestoreHistorySession: vi.fn(() => Promise.resolve()),
  mockConfirmDialog: vi.fn(() => Promise.resolve(false)),
}));

// useAgentStatus 数据层 mock（NAV-02 数据源——rows 由各测试显式注入）
vi.mock("../features/agentStatus/useAgentStatus", () => ({
  useAgentStatus: () => mockUseAgentStatus(),
}));

// agentHistory scan mock（真实 useAgentHistory 的唯一外部数据源，照 agent-status-view 先例）
vi.mock("../ipc/agentHistory", () => ({
  scanAgentHistory: mockScanHistory,
  deleteHistorySession: vi.fn(),
}));

// FE-09：findPanelForSession/findPageIdForPanelId 已上提 pageApis——用真实实现
// （TerminalRegistry/useProjects 分别被下方 mock 与 store 种子驱动），
// 仅覆盖切换入口与 API 访问点
vi.mock("../workspace/pageApis", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../workspace/pageApis")>();
  return {
    ...actual,
    switchToPageAndFocus: mockSwitchToPageAndFocus,
    getPageApi: vi.fn(() => undefined),
  };
});

// useAgentHistory 订阅 TerminalRegistry（deriveActiveSessionStatuses 触发源）——
// getAll 经 mockTerminalGetAll 可注入（历史行运行中判定 + SessionActionDialog 反查）
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    subscribe: vi.fn(() => () => {}),
    getAll: () => mockTerminalGetAll(),
    get: vi.fn(() => undefined),
    _reset: vi.fn(),
    _size: vi.fn(() => 0),
  },
}));

// FE-03：删除项目确认弹窗 mock（barrel partial——confirmDialog 覆盖，
// 其余导出保持真实，icons/StatusDot 等经各自独立路径不受影响）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, confirmDialog: mockConfirmDialog };
});

// 历史行右键「复制恢复命令」→ writeText；删除 → confirmDialog + deleteHistorySession
// （OV-02：NavTree 不再引用 ipc/dialog ask，确认弹窗改经 src/lib barrel 的 confirmDialog）；
// SessionActionDialog「切换到该会话」反查失败 → sendToastNotification
vi.mock("../ipc/clipboard", () => ({
  writeText: mockWriteText,
  readText: vi.fn(() => Promise.resolve("")),
}));
vi.mock("../ipc/dialog", () => ({
  open: vi.fn(),
}));
vi.mock("../ipc/notification", () => ({
  sendToastNotification: mockSendToast,
}));

// IC-03：状态渲染改 StatusDot——mock 为可识别 span（只断言接线，不依赖其内部 DOM）
vi.mock("../lib/StatusDot", async () => {
  const { createElement } = await import("react");
  return {
    StatusDot: ({ status }: { status: string | null }) =>
      status == null
        ? null
        : createElement("span", { "data-testid": "status-dot" }, status),
  };
});

// restoreSession 依赖 pageApis.switchToPageShared（本文件 mock 不含）——整模块 mock 防解析失败
vi.mock("../features/agentHistory/restoreSession", () => ({
  restoreHistorySession: mockRestoreHistorySession,
}));

import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";
import { NavTree } from "../features/navTree/NavTree";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { AgentSessionRow } from "../features/agentStatus/useAgentStatus";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";
import {
  ACTIVE_SELECTION_BG,
  SELECTION_HOVER_BG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
  SIDEBAR_BG,
  PLACEHOLDER_FG,
  DIM_FG,
  ACCENT_FG,
  AGENT_STATUS_USAGE_COLORS,
} from "../theme/colors";
import type { AgentHistorySession } from "../types/agentHistory";

// ── 测试辅助 ──

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 去除内空白后比对） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  return hex;
}

/** rgba 输入 jsdom 可能在逗号后补空格——比对前去空白 */
function normColor(c: string): string {
  return c.replace(/\s/g, "");
}

/** 查询指定 data-e2e 的行元素列表 */
function getRows(container: HTMLElement, e2e: string): HTMLElement[] {
  return Array.from(
    container.querySelectorAll(`[data-e2e="${e2e}"]`),
  ) as HTMLElement[];
}

/** 若目标行尚未渲染，点击其父级行展开（最多两级）——不依赖组件默认展开态 */
function expandTo(container: HTMLElement, selector: string): void {
  if (container.querySelector(selector)) return;
  const proj = container.querySelector('[data-e2e="nav-row-project"]');
  if (proj) fireEvent.click(proj as HTMLElement);
  if (container.querySelector(selector)) return;
  const page = container.querySelector('[data-e2e="nav-row-page"]');
  if (page) fireEvent.click(page as HTMLElement);
}

/** 种子 projects store：创建一个含指定页面列表的项目
 *  注意：必须函数式合并（Zustand setState 浅合并会整体替换 projects 键——
 *  第二次调用会覆盖首个项目，双项目种子用例将失效） */
function seedProject(
  rootPath: string,
  projectId: string,
  name: string,
  pages: Array<{ pageId: string; name: string }>,
): void {
  useProjects.setState((prev) => ({
    projects: {
      ...prev.projects,
      [projectId]: {
        projectId,
        name,
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
    expandedNodes: { ...prev.expandedNodes, [projectId]: true },
  }));
}

/** 种子 layout store：设置活跃页面 */
function seedActivePage(pageId: string | null): void {
  useLayout.setState({ activePageId: pageId });
}

/** 构造 AgentSessionRow（cliId 缺省 CLAUDE_CLI_ID——真实注册表 claude profile 生效） */
function makeRow(overrides: Partial<AgentSessionRow> = {}): AgentSessionRow {
  return {
    panelId: "terminal-page1-0",
    pageId: "page1",
    projectId: "proj-1",
    cliId: CLAUDE_CLI_ID,
    title: "终端 page1",
    status: "working",
    lastEventAt: Date.now(),
    usage: undefined,
    ...overrides,
  };
}

/** 重置 stores + mocks */
function resetAll(): void {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  mockUseAgentStatus.mockReset();
  mockUseAgentStatus.mockReturnValue({
    state: { kind: "ready" },
    rows: [],
    currentProjectName: null,
    now: Date.now(),
  });
  mockSwitchToPageAndFocus.mockReset();
  mockScanHistory.mockReset();
  mockScanHistory.mockResolvedValue([]);
  mockWriteText.mockReset();
  mockSendToast.mockReset();
  mockTerminalGetAll.mockReset();
  mockTerminalGetAll.mockReturnValue(new Map());
  mockRestoreHistorySession.mockReset();
  mockRestoreHistorySession.mockResolvedValue(undefined);
  mockConfirmDialog.mockReset();
  mockConfirmDialog.mockResolvedValue(false);
}

beforeEach(() => {
  resetAll();
});

afterEach(() => {
  cleanup();
});

// ═══════════════════════════════════════════════════════════════
// 三级层级渲染（项目→页面→会话）
// ═══════════════════════════════════════════════════════════════

describe("三级层级渲染（项目→页面→会话）", () => {
  it("种子 1 项目 2 页面 + 1 活跃会话 → 三级行各按其数量渲染", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow()],
      currentProjectName: "测试项目",
      now: Date.now(),
    });

    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');

    expect(getRows(container, "nav-row-project")).toHaveLength(1);
    expect(getRows(container, "nav-row-page")).toHaveLength(2);
    expect(getRows(container, "nav-row-session")).toHaveLength(1);
    // 会话行标题 = useAgentStatus 行标题（panelId→pageId 归属页面后渲染）
    expect(getRows(container, "nav-row-session")[0].textContent).toContain(
      "终端 page1",
    );
  });

  it("页面行渲染 IconPage（FileText 文档图标）14px fg-3——与历史session 时钟区分", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-page"]');
    const pageRow = getRows(container, "nav-row-page")[0];
    // 行内 svg 含 chevron（12px）+ IconPage（14px）——按槽位容器宽度定位页面图标
    const iconWrap = Array.from(pageRow.querySelectorAll("svg"))
      .map((s) => s.parentElement as HTMLElement)
      .find((p) => p.style.width === "14px");
    expect(iconWrap).toBeTruthy();
    expect(iconWrap?.querySelector("svg")?.getAttribute("aria-hidden")).toBe(
      "true",
    );
    expect(iconWrap?.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("活跃会话经 panelId→pageId 归属到对应页面下（page2 会话不出现在 page1 区域之外）", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [
        makeRow({ panelId: "terminal-page2-0", pageId: "page2", title: "终端 page2" }),
      ],
      currentProjectName: "测试项目",
      now: Date.now(),
    });

    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');

    // 页面 2 展开前会话行不可见；展开页面 2 行后可见
    const pages = getRows(container, "nav-row-page");
    const page2Row = pages.find((p) => p.textContent?.includes("页面 2"));
    expect(page2Row).toBeTruthy();
    fireEvent.click(page2Row as HTMLElement);

    const sessions = getRows(container, "nav-row-session");
    expect(sessions).toHaveLength(1);
    expect(sessions[0].textContent).toContain("终端 page2");
  });

  it("点击项目行 toggle 页面行显隐（与默认展开态无关的翻转断言）", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");

    const { container } = render(<NavTree />);
    const before = getRows(container, "nav-row-page").length;

    const projRow = getRows(container, "nav-row-project")[0];
    fireEvent.click(projRow);

    const after = getRows(container, "nav-row-page").length;
    expect(after).toBe(before === 0 ? 2 : 0);
  });

  it("点击页面行 toggle 会话行显隐", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow()],
      currentProjectName: "测试项目",
      now: Date.now(),
    });

    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');
    const before = getRows(container, "nav-row-session").length;
    expect(before).toBe(1);

    const pageRow = getRows(container, "nav-row-page")[0];
    fireEvent.click(pageRow);

    expect(getRows(container, "nav-row-session").length).toBe(0);
  });
});

// ═══════════════════════════════════════════════════════════════
// 行高规格与选中态 token
// ═══════════════════════════════════════════════════════════════

describe("行高规格与选中态 token", () => {
  function seedBasicTree(): HTMLElement {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow()],
      currentProjectName: "测试项目",
      now: Date.now(),
    });
    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');
    return container;
  }

  it("行高规格：项目/页面行 28px、会话行 30px，圆角 5px", () => {
    const container = seedBasicTree();

    for (const row of getRows(container, "nav-row-project")) {
      expect(row.style.height).toBe("28px");
      expect(row.style.borderRadius).toBe("5px");
    }
    for (const row of getRows(container, "nav-row-page")) {
      expect(row.style.height).toBe("28px");
      expect(row.style.borderRadius).toBe("5px");
    }
    for (const row of getRows(container, "nav-row-session")) {
      expect(row.style.height).toBe("30px");
    }
  });

  it("点击行 → 选中态 token：ACTIVE_SELECTION_BG 底 + fg-1 文字", () => {
    const container = seedBasicTree();
    const pageRow = getRows(container, "nav-row-page")[0];

    fireEvent.click(pageRow);
    expect(normColor(pageRow.style.backgroundColor)).toBe(
      normColor(ACTIVE_SELECTION_BG),
    );
    expect(pageRow.style.color).toBe(hexToRgb(SIDEBAR_FG));
  });

  it("hover 非选中行 → SIDEBAR_COLORS.hover；选中行 hover → SELECTION_HOVER_BG", () => {
    const container = seedBasicTree();
    // 种子 activePageId="page1" → 首个页面行选中、第二个非选中
    const [, page2] = getRows(container, "nav-row-page");

    // 非选中行 hover → hover token（#222227）
    fireEvent.mouseEnter(page2);
    expect(page2.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.hover));
    fireEvent.mouseLeave(page2);

    // 选中行 hover → SELECTION_HOVER_BG（0.22 强于 0.13）
    fireEvent.click(page2); // page2 变为选中
    expect(normColor(page2.style.backgroundColor)).toBe(
      normColor(ACTIVE_SELECTION_BG),
    );
    fireEvent.mouseEnter(page2);
    expect(normColor(page2.style.backgroundColor)).toBe(
      normColor(SELECTION_HOVER_BG),
    );
  });
});

// ═══════════════════════════════════════════════════════════════
// 活跃会话行构成（NAV-02：StatusDot + logo + 标题 + 迷你用量条 + 百分比）
// ═══════════════════════════════════════════════════════════════

describe("活跃会话行构成", () => {
  function seedSessionRow(
    overrides: Partial<AgentSessionRow> = {},
  ): HTMLElement {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow(overrides)],
      currentProjectName: "测试项目",
      now: Date.now(),
    });
    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');
    return getRows(container, "nav-row-session")[0];
  }

  it("构成：StatusDot（status 透传）+ CLI logo 14px + 标题", () => {
    const row = seedSessionRow({
      title: "修复 context 用量计算",
      status: "working",
    });

    // StatusDot mock 为 data-testid span——断言 status 值透传
    const dot = row.querySelector('[data-testid="status-dot"]');
    expect(dot).toBeTruthy();
    expect(dot?.textContent).toBe("working");

    // CLI logo：按行 cliId 查 profile.iconSrc，14px（NAV-02 契约）
    const logoImg = row.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("14");
    expect(logoImg?.getAttribute("height")).toBe("14");

    // 标题
    expect(row.textContent).toContain("修复 context 用量计算");
  });

  it("右侧迷你用量条 32×3 + 百分比 11px fg-4（DIM_FG）", () => {
    const row = seedSessionRow({ usage: { usedPercentage: 42 } });

    // 迷你用量条容器 32×3
    const barContainer = Array.from(row.querySelectorAll("div")).find(
      (el) =>
        (el as HTMLElement).style.width === "32px" &&
        (el as HTMLElement).style.height === "3px",
    ) as HTMLElement | undefined;
    expect(barContainer).toBeTruthy();

    // 内层填充宽度 = 百分比
    const innerBar = barContainer?.firstElementChild as HTMLElement | null;
    expect(innerBar?.style.width).toBe("42%");

    // 百分比文本 11px + DIM_FG
    const pctEl = Array.from(row.querySelectorAll("span")).find((s) =>
      s.textContent?.includes("42%"),
    ) as HTMLElement | undefined;
    expect(pctEl).toBeTruthy();
    expect(pctEl?.style.fontSize).toBe("11px");
    expect(pctEl?.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("usage 缺省 → 百分比显示不可用态 '--'", () => {
    const row = seedSessionRow({ usage: undefined });
    expect(row.textContent).toContain("--");
  });

  // 四档分级（迁移自 agent-status-view.test.tsx 行为等价：≥90 critical / ≥70 high /
  // ≥50 medium / else low——NavSessionRow.usageBarColor 照 AgentStatusRow 逻辑不变）
  it.each([
    [95, AGENT_STATUS_USAGE_COLORS.critical],
    [75, AGENT_STATUS_USAGE_COLORS.high],
    [55, AGENT_STATUS_USAGE_COLORS.medium],
    [45, AGENT_STATUS_USAGE_COLORS.low],
  ] as const)("用量条四档分级：%i%% → %s", (usedPercentage, expectedColor) => {
    const row = seedSessionRow({ usage: { usedPercentage } });
    const barContainer = Array.from(row.querySelectorAll("div")).find(
      (el) =>
        (el as HTMLElement).style.width === "32px" &&
        (el as HTMLElement).style.height === "3px",
    ) as HTMLElement | undefined;
    expect(barContainer).toBeTruthy();
    const innerBar = barContainer?.firstElementChild as HTMLElement | null;
    expect(innerBar?.style.width).toBe(`${usedPercentage}%`);
    expect(normColor(innerBar?.style.backgroundColor ?? "")).toBe(
      normColor(hexToRgb(expectedColor)),
    );
  });

  it("点击会话行 → switchToPageAndFocus(pageId, panelId, signal)（FE-26 携 AbortSignal）", async () => {
    const row = seedSessionRow();
    fireEvent.click(row);

    await waitFor(() => {
      expect(mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
        expect.any(AbortSignal),
      );
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 搜索过滤（NAV-04：子串不区分大小写，父节点因子命中而显示）
// ═══════════════════════════════════════════════════════════════

describe("搜索过滤", () => {
  function seedTwoProjects(): HTMLElement {
    seedProject("C:/projA", "proj-A", "项目Alpha", [
      { pageId: "pageA1", name: "页面Alpha" },
    ]);
    seedProject("C:/projB", "proj-B", "项目Beta", [
      { pageId: "pageB1", name: "页面Beta" },
    ]);
    seedActivePage("pageA1");
    const { container } = render(<NavTree />);
    return container;
  }

  function getSearchInput(container: HTMLElement): HTMLInputElement {
    const input = container.querySelector("input");
    if (!input) throw new Error("搜索框未渲染");
    return input;
  }

  it("页面名子串命中（不区分大小写）→ 该页面行显示、未命中页面行隐藏", () => {
    const container = seedTwoProjects();
    fireEvent.change(getSearchInput(container), {
      target: { value: "alpha" },
    });

    const pages = getRows(container, "nav-row-page");
    expect(pages).toHaveLength(1);
    expect(pages[0].textContent).toContain("页面Alpha");
  });

  it("未命中 → 无任何项目行（搜索无结果显示空态，不抛异常）", () => {
    const container = seedTwoProjects();
    fireEvent.change(getSearchInput(container), {
      target: { value: "zzz" },
    });

    expect(getRows(container, "nav-row-project")).toHaveLength(0);
    expect(getRows(container, "nav-row-page")).toHaveLength(0);
  });

  it("父节点因子：页面命中 → 父项目行一并显示（即使项目名不匹配）", () => {
    const container = seedTwoProjects();
    // 命中页面Alpha——父项目「项目Alpha」行必须显示
    fireEvent.change(getSearchInput(container), {
      target: { value: "页面Alpha" },
    });

    const projects = getRows(container, "nav-row-project");
    expect(projects).toHaveLength(1);
    expect(projects[0].textContent).toContain("项目Alpha");
    expect(getRows(container, "nav-row-page")[0].textContent).toContain(
      "页面Alpha",
    );
  });

  it("父节点因子：项目名命中 → 仅项目行显示、未命中页面行隐藏（match 链单向）", () => {
    const container = seedTwoProjects();
    // 查询词仅命中项目名「项目Beta」——页面名「页面Beta」不含该子串，
    // 排除页面命中因子；NAV-04 match 链为单向（子命中→父显示），
    // 父命中不向下传播——未命中页面整行隐藏（NavTree renderPage 返回 null）
    fireEvent.change(getSearchInput(container), {
      target: { value: "项目Beta" },
    });

    const projects = getRows(container, "nav-row-project");
    expect(projects).toHaveLength(1);
    expect(projects[0].textContent).toContain("项目Beta");
    // 对照上用例「页面命中→父项目行显示」：单向链只保证子→父方向，
    // 项目名命中不会令未命中的页面行渲染
    expect(getRows(container, "nav-row-page")).toHaveLength(0);
  });

  it("会话名命中 → 会话行显示", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow({ title: "重构导航树" })],
      currentProjectName: "测试项目",
      now: Date.now(),
    });
    const { container } = render(<NavTree />);

    fireEvent.change(container.querySelector("input") as HTMLInputElement, {
      target: { value: "导航树" },
    });

    const sessions = getRows(container, "nav-row-session");
    expect(sessions).toHaveLength(1);
    expect(sessions[0].textContent).toContain("重构导航树");
  });
});

// ═══════════════════════════════════════════════════════════════
// 「当前」pill 与计数 pill（NAV-09）
// ═══════════════════════════════════════════════════════════════

describe("「当前」pill 与计数 pill", () => {
  it("当前活跃项目行显示「当前」pill（ACTIVE_SELECTION_BG 底 + ACCENT_FG 字 10px），非当前项目无", () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
      { pageId: "pageA2", name: "页面 A2" },
    ]);
    seedProject("C:/projB", "proj-B", "项目B", [
      { pageId: "pageB", name: "页面 B" },
    ]);
    seedActivePage("pageA");

    const { container } = render(<NavTree />);
    const [projA, projB] = getRows(container, "nav-row-project");

    // 当前项目行内「当前」pill：token + 字号
    const pillA = Array.from(projA.querySelectorAll("*")).find(
      (el) => el.textContent === "当前",
    ) as HTMLElement | undefined;
    expect(pillA).toBeTruthy();
    expect(normColor(pillA?.style.backgroundColor ?? "")).toBe(
      normColor(ACTIVE_SELECTION_BG),
    );
    expect(pillA?.style.color).toBe(hexToRgb(ACCENT_FG));
    expect(pillA?.style.fontSize).toBe("10px");

    // 非当前项目行无「当前」pill
    expect(projB.textContent).not.toContain("当前");
  });

  it("项目行右侧计数 pill：SIDEBAR_BG 底 + PLACEHOLDER_FG 字（页面数）", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
      { pageId: "page2", name: "页面 2" },
    ]);
    seedActivePage("page1");

    const { container } = render(<NavTree />);
    const projRow = getRows(container, "nav-row-project")[0];

    // 计数 pill 底 #1a1a1e（SIDEBAR_BG）
    const pill = Array.from(projRow.querySelectorAll("*")).find(
      (el) =>
        (el as HTMLElement).style.backgroundColor === hexToRgb(SIDEBAR_BG) &&
        (el as HTMLElement).style.color === hexToRgb(PLACEHOLDER_FG),
    ) as HTMLElement | undefined;
    expect(pill).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 右键菜单（NAV-06：承接 SidebarTree 菜单但删除「打开 Hooks 配置」项）
// ═══════════════════════════════════════════════════════════════

describe("右键菜单", () => {
  it("项目行右键菜单：含「新建操作页面」「删除项目」，无「打开 Hooks 配置」", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { container, queryByText } = render(<NavTree />);
    fireEvent.contextMenu(getRows(container, "nav-row-project")[0]);

    // 其余菜单项承接 SidebarTree（NAV-06：仅删「打开 Hooks 配置」）
    expect(queryByText("新建操作页面")).toBeTruthy();
    expect(queryByText("删除项目")).toBeTruthy();
    // 入口唯一化：右键菜单无「打开 Hooks 配置」（NAV-06 删除）
    expect(queryByText("打开 Hooks 配置")).toBeNull();
  });

  it("页面行右键菜单：含「重命名操作页面」「删除操作页面」，无「打开 Hooks 配置」", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { container, queryByText } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-page"]');
    fireEvent.contextMenu(getRows(container, "nav-row-page")[0]);

    expect(queryByText("重命名操作页面")).toBeTruthy();
    expect(queryByText("删除操作页面")).toBeTruthy();
    expect(queryByText("打开 Hooks 配置")).toBeNull();
  });

  it("删除项目（FE-03）：confirmDialog 确认（danger: true），确认后 removeProject、取消保留", async () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    const { container } = render(<NavTree />);

    // 右键项目行 → 点「删除项目」→ 应用内 confirmDialog 携带项目名与危险标记
    fireEvent.contextMenu(getRows(container, "nav-row-project")[0]);
    fireEvent.click(
      Array.from(document.body.querySelectorAll("div")).find(
        (el) => el.textContent === "删除项目",
      )!,
    );
    await waitFor(() => {
      expect(mockConfirmDialog).toHaveBeenCalledWith({
        title: "确认删除",
        message: `确定删除项目 "测试项目"？`,
        danger: true,
      });
    });

    // 取消（默认 resolve false）→ 项目保留
    expect(getRows(container, "nav-row-project")).toHaveLength(1);

    // 确认（下一次 resolve true）→ removeProject 生效（项目行消失 → 空态）
    mockConfirmDialog.mockResolvedValueOnce(true);
    fireEvent.contextMenu(getRows(container, "nav-row-project")[0]);
    fireEvent.click(
      Array.from(document.body.querySelectorAll("div")).find(
        (el) => el.textContent === "删除项目",
      )!,
    );
    await waitFor(() => {
      expect(getRows(container, "nav-row-project")).toHaveLength(0);
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// data-e2e 选择器与视图骨架
// ═══════════════════════════════════════════════════════════════

describe("data-e2e 选择器与视图骨架", () => {
  it("容器 nav-tree + 分组标题「导航」+ 搜索框占位「搜索项目 / 页面 / 会话…」", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");

    const { container, getByText, getByPlaceholderText } = render(<NavTree />);

    expect(container.querySelector('[data-e2e="nav-tree"]')).toBeTruthy();
    // 分组标题「导航」（NAV-04：11px 全大写 0.08em fg-3 是视觉规范，文案写死）
    expect(getByText("导航")).toBeTruthy();
    // 搜索框占位（NAV-04 契约文案逐字）
    expect(getByPlaceholderText("搜索项目 / 页面 / 会话…")).toBeTruthy();
  });

  it("五枚 data-e2e 选择器齐备（契约写死）", () => {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockUseAgentStatus.mockReturnValue({
      state: { kind: "ready" },
      rows: [makeRow()],
      currentProjectName: "测试项目",
      now: Date.now(),
    });

    const { container } = render(<NavTree />);
    expandTo(container, '[data-e2e="nav-row-session"]');

    expect(getRows(container, "nav-row-project").length).toBeGreaterThan(0);
    expect(getRows(container, "nav-row-page").length).toBeGreaterThan(0);
    expect(getRows(container, "nav-row-session").length).toBeGreaterThan(0);
  });
});

// ═══════════════════════════════════════════════════════════════
// 历史节点（NAV-03，行为用例迁移自 agent-history-view.test.tsx——
// AgentHistorySections 已随 NAV-08 删除，历史区承接方 = NavHistoryNode/NavHistoryRow）
// ═══════════════════════════════════════════════════════════════

describe("历史节点（NAV-03）", () => {
  /** 构造历史会话（cwd 归属性：C:/test 项目下） */
  function makeHistorySession(
    id: string,
    overrides: Partial<AgentHistorySession> = {},
  ): AgentHistorySession {
    return {
      sessionId: id,
      cwd: "C:/test",
      title: `历史会话 ${id}`,
      titleSource: "ai",
      firstPrompt: "修复 context 用量计算",
      mtimeMs: Date.now(),
      cwdExists: true,
      cliId: CLAUDE_CLI_ID,
      ...overrides,
    };
  }

  /** 种子项目 + 历史扫描结果 + 渲染 NavTree，返回容器 */
  function renderWithHistory(
    sessions: AgentHistorySession[],
    overrides: { terminalEntry?: unknown } = {},
  ): HTMLElement {
    seedProject("C:/test", "proj-1", "测试项目", [
      { pageId: "page1", name: "页面 1" },
    ]);
    seedActivePage("page1");
    mockScanHistory.mockResolvedValue(sessions);
    if (overrides.terminalEntry !== undefined) {
      mockTerminalGetAll.mockReturnValue(
        new Map([["terminal-page1-0", overrides.terminalEntry]]),
      );
    }
    const { container } = render(<NavTree />);
    return container;
  }

  it("历史折叠节点常驻项目下：计数 pill + 展开渲染历史行（StatusDot + logo + 标题 + 相对时间）", async () => {
    const container = renderWithHistory([makeHistorySession("s1")]);

    // 历史节点常驻（不随项目展开态隐藏——NAV-10 契约）；
    // 计数 pill 显示会话数（挂载即 scan 为 async——等待落地）
    const node = await waitFor(() => {
      const el = container.querySelector('[data-e2e="nav-history-node"]');
      expect(el?.textContent).toContain("1");
      return el as HTMLElement;
    });

    // 展开 → 历史行渲染（标题 + logo + title tooltip = prompt 预览）
    fireEvent.click(node);
    await waitFor(() => {
      expect(getRows(container, "nav-row-session").length).toBe(1);
    });
    const row = getRows(container, "nav-row-session")[0];
    expect(row.textContent).toContain("历史会话 s1");
    expect(row.querySelector('img[alt="CLI 图标"]')).toBeTruthy();
    expect(row.getAttribute("title")).toBe("修复 context 用量计算");
  });

  it("历史session 节点与操作页面同级缩进（childrenStyle 容器）且恒位于页面容器之后", async () => {
    const container = renderWithHistory([makeHistorySession("s1")]);
    const projRow = getRows(container, "nav-row-project")[0];
    const projectContainer = projRow.parentElement as HTMLElement;

    // 项目收起（默认态）：容器 children = [项目行, 历史容器]——历史常驻
    let children = Array.from(projectContainer.children);
    expect(children).toHaveLength(2);
    const collapsedHistoryWrap = children[1] as HTMLElement;
    expect(
      collapsedHistoryWrap.querySelector('[data-e2e="nav-history-node"]'),
    ).toBeTruthy();

    // 展开项目 → [项目行, 页面容器, 历史容器]——历史恒位于末位（最下方）
    fireEvent.click(projRow);
    await waitFor(() => expect(getRows(container, "nav-row-page").length).toBe(1));
    children = Array.from(projectContainer.children);
    expect(children.length).toBeGreaterThanOrEqual(3);
    const historyWrap = children[children.length - 1] as HTMLElement;
    expect(historyWrap.querySelector('[data-e2e="nav-history-node"]')).toBeTruthy();
    // 同级缩进：与页面容器同规格 childrenStyle（marginLeft 15 + 1px 发丝引导线）
    const pagesWrap = children[1] as HTMLElement;
    expect(historyWrap.style.marginLeft).toBe("15px");
    expect(historyWrap.style.marginLeft).toBe(pagesWrap.style.marginLeft);
    expect(historyWrap.style.borderLeft).toBe(pagesWrap.style.borderLeft);
    expect(historyWrap.style.borderLeft).toContain("1px solid");

    // 再次收起 → 历史容器仍存在（NAV-10 常驻契约不破）
    fireEvent.click(projRow);
    await waitFor(() => expect(getRows(container, "nav-row-page").length).toBe(0));
    expect(
      (projRow.parentElement as HTMLElement).querySelector(
        '[data-e2e="nav-history-node"]',
      ),
    ).toBeTruthy();
  });

  it("双击普通历史行 → restoreHistorySession(session, { fork: false })", async () => {
    const session = makeHistorySession("s1");
    const container = renderWithHistory([session]);
    fireEvent.click(container.querySelector('[data-e2e="nav-history-node"]') as HTMLElement);
    await waitFor(() => expect(getRows(container, "nav-row-session").length).toBe(1));
    fireEvent.doubleClick(getRows(container, "nav-row-session")[0]);
    await waitFor(() => {
      expect(mockRestoreHistorySession).toHaveBeenCalledWith(session, {
        fork: false,
      });
    });
  });

  it("双击运行中历史行 → SessionActionDialog →「切换到该会话操作页面」经 switchToPageAndFocus 跳转", async () => {
    const session = makeHistorySession("s1");
    // 运行中：TerminalRegistry 含该复合键会话（deriveActiveSessionStatuses 派生四态）
    const container = renderWithHistory([session], {
      terminalEntry: {
        agentSession: {
          sessionId: "s1",
          cliId: CLAUDE_CLI_ID,
          status: "working",
          lastEventAt: Date.now(),
        },
      },
    });
    fireEvent.click(container.querySelector('[data-e2e="nav-history-node"]') as HTMLElement);
    await waitFor(() => expect(getRows(container, "nav-row-session").length).toBe(1));

    fireEvent.doubleClick(getRows(container, "nav-row-session")[0]);

    // SessionActionDialog 弹出（运行中恢复冲突提示——照原 HistorySessionList（已删）分派语义）
    const dialog = await waitFor(() =>
      document.querySelector('[data-e2e="agent-history-action-dialog"]'),
    );
    expect(dialog).toBeTruthy();
    // 「切换到该会话操作页面」：复合键反查 TerminalRegistry → panelId 前缀匹配 page1
    // → switchToPageAndFocus(pageId, panelId)（B14 防御分层）
    fireEvent.click(
      Array.from(dialog!.querySelectorAll("button")).find((b) =>
        b.textContent?.includes("切换到该会话操作页面"),
      )!,
    );
    await waitFor(() => {
      expect(mockSwitchToPageAndFocus).toHaveBeenCalledWith(
        "page1",
        "terminal-page1-0",
        expect.any(AbortSignal), // FE-26: 调用方携 AbortSignal（再次点击/卸载时 abort）
      );
    });
  });

  it("历史行右键菜单：复制恢复命令 / 分支恢复 / 删除（danger）", async () => {
    const session = makeHistorySession("s1");
    const container = renderWithHistory([session]);
    fireEvent.click(container.querySelector('[data-e2e="nav-history-node"]') as HTMLElement);
    await waitFor(() => expect(getRows(container, "nav-row-session").length).toBe(1));

    fireEvent.contextMenu(getRows(container, "nav-row-session")[0]);
    expect(document.body.textContent).toContain("复制恢复命令");
    expect(document.body.textContent).toContain("分支恢复");
    expect(document.body.textContent).toContain("删除");

    // 复制恢复命令 → writeText（buildResumeCommand 委托 claude profile 策略）
    fireEvent.click(
      Array.from(document.body.querySelectorAll("div")).find((el) =>
        el.textContent === "复制恢复命令",
      )!,
    );
    await waitFor(() => expect(mockWriteText).toHaveBeenCalled());

    // 分支恢复 → restoreHistorySession(session, { fork: true })
    fireEvent.contextMenu(getRows(container, "nav-row-session")[0]);
    fireEvent.click(
      Array.from(document.body.querySelectorAll("div")).find((el) =>
        el.textContent === "分支恢复",
      )!,
    );
    await waitFor(() => {
      expect(mockRestoreHistorySession).toHaveBeenCalledWith(session, {
        fork: true,
      });
    });
  });

  it("无历史会话 → 展开显示空态「暂无历史会话」", async () => {
    const container = renderWithHistory([]);
    fireEvent.click(container.querySelector('[data-e2e="nav-history-node"]') as HTMLElement);
    await waitFor(() => {
      expect(container.textContent).toContain("暂无历史会话");
    });
  });
});
