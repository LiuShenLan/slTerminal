// nav-tree-history.test.tsx — 导航树历史折叠节点 L2 测试（NAV-10 L2 部分，NAV-03）
//
// 规格契约（写死于 docs/ui-redesign-impl/workflows/stage-06-sidebar.js 脚本头
// + checklist.md NAV-03 条目 + stages.md Stage 06 实现要点）：
//   - 历史会话折叠节点挂项目下（nav-history-node）：IconHistory 时钟 +「历史」+ 计数 pill
//     （SIDEBAR_BG 底 #1a1a1e + PLACEHOLDER_FG 字 fg-4）
//   - 归属：cwd 前缀匹配项目 rootPath
//   - 展开 = 历史行（StatusDot + logo + 标题 + 右侧相对时间，单行 30px）
//   - prompt 预览 → 原生 title tooltip（行容器 title 属性）
//   - 双击恢复 + 右键菜单（复制恢复命令/分支恢复/删除）沿用 historyContextMenu 策略
//   - 空历史显示空态（不写死文案，断言无历史行渲染）
//
// Mock 策略（照 src/__tests__/ 既有种子模式——agent-status-view.test.tsx 先例）：
//   useAgentStatus 模块 mock（NavTree 会调用）；agentHistory scan 走真实 useAgentHistory +
//   ipc/agentHistory.scanHistory mock（数据链真实：scan → sessions → 归属过滤）；
//   stores/projects、stores/layout 真实 store + setState 种子。
//   NavTree 本体由 navtree-new agent 按契约产出，本文件只锁定契约点。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ─── Hoisted mocks ───
const { mockUseAgentStatus, mockScanHistory, mockRestoreHistorySession } =
  vi.hoisted(() => ({
    mockUseAgentStatus: vi.fn(),
    mockScanHistory: vi.fn(),
    mockRestoreHistorySession: vi.fn(() => Promise.resolve()),
  }));

vi.mock("../features/agentStatus/useAgentStatus", () => ({
  useAgentStatus: () => mockUseAgentStatus(),
}));

// 历史数据源：真实 useAgentHistory，仅 mock IPC 层 scanHistory
vi.mock("../ipc/agentHistory", () => ({
  scanHistory: mockScanHistory,
  deleteHistorySession: vi.fn(),
}));

// useAgentHistory 订阅 TerminalRegistry——空实现即可（历史数据不依赖注册表）
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    subscribe: vi.fn(() => () => {}),
    getAll: vi.fn(() => new Map()),
    get: vi.fn(() => undefined),
    _reset: vi.fn(),
    _size: vi.fn(() => 0),
  },
}));

// 双击恢复编排 mock（防 pageApis.switchToPageShared 依赖——本文件只断言调度入参）
vi.mock("../features/agentHistory/restoreSession", () => ({
  restoreHistorySession: mockRestoreHistorySession,
}));

// StatusDot mock 为可识别 span（只断言接线）
vi.mock("../lib/StatusDot", async () => {
  const { createElement } = await import("react");
  return {
    StatusDot: ({ status }: { status: string | null }) =>
      status == null
        ? null
        : createElement("span", { "data-testid": "status-dot" }, status),
  };
});

import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";
import { NavTree } from "../features/navTree/NavTree";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { AgentHistorySession } from "../types/agentHistory";
import { CLAUDE_CLI_ID } from "../features/cliProfiles/profiles/claude";
import { SIDEBAR_BG, PLACEHOLDER_FG } from "../theme/colors";

// ── 测试辅助 ──

/** #hex → jsdom rgb() 规范化形态 */
function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

/** 查询指定 data-e2e 的行元素列表 */
function getRows(container: HTMLElement, e2e: string): HTMLElement[] {
  return Array.from(
    container.querySelectorAll(`[data-e2e="${e2e}"]`),
  ) as HTMLElement[];
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

function seedActivePage(pageId: string | null): void {
  useLayout.setState({ activePageId: pageId });
}

/** 构造历史会话（claude 会话，supportsFork=true → 菜单含分支恢复） */
function makeHistorySession(
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: "s-1",
    cwd: "C:/projA",
    title: "重构导航树",
    titleSource: "aiTitle",
    firstPrompt: "重构侧栏导航树",
    mtimeMs: Date.now() - 5 * 60_000, // 5 分钟前
    cwdExists: true,
    cliId: CLAUDE_CLI_ID,
    ...overrides,
  };
}

/** 展开指定项目的历史折叠节点并等待 scan 落地 */
async function expandHistoryNode(
  container: HTMLElement,
  projectSelectorText: string,
): Promise<HTMLElement> {
  const nodes = getRows(container, "nav-history-node");
  const node = nodes.find((n) =>
    n.textContent?.includes(projectSelectorText),
  ) as HTMLElement;
  expect(node).toBeTruthy();
  fireEvent.click(node);
  await waitFor(() => {
    expect(mockScanHistory).toHaveBeenCalled();
  });
  return node;
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
  mockScanHistory.mockReset();
  mockScanHistory.mockResolvedValue([]);
  mockRestoreHistorySession.mockReset();
  mockRestoreHistorySession.mockResolvedValue(undefined);
}

beforeEach(() => {
  resetAll();
});

afterEach(() => {
  cleanup();
});

// ═══════════════════════════════════════════════════════════════
// 历史折叠节点：渲染与计数 pill
// ═══════════════════════════════════════════════════════════════

describe("历史折叠节点渲染", () => {
  it("项目下渲染 nav-history-node：时钟图标 +「历史」+ 计数 pill（SIDEBAR_BG 底 + PLACEHOLDER_FG 字）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ cwd: "C:/projA" }),
    ]);

    const { container } = render(<NavTree />);
    // 挂载即 scan 为 async——等待计数落地后断言（NAV-10 契约：历史折叠节点常驻）
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史1/);
      return el;
    });

    // 「历史」文案（NAV-03 契约）+ 时钟图标（lucide Clock svg）
    expect(node.textContent).toContain("历史");
    expect(node.querySelector("svg")).toBeTruthy();

    // 计数 pill：SIDEBAR_BG 底 + PLACEHOLDER_FG 字，文本 = 匹配会话数
    const pill = Array.from(node.querySelectorAll("*")).find(
      (el) =>
        (el as HTMLElement).style.backgroundColor === hexToRgb(SIDEBAR_BG) &&
        (el as HTMLElement).style.color === hexToRgb(PLACEHOLDER_FG),
    ) as HTMLElement | undefined;
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("1");
  });

  it("计数 = 同一项目匹配会话数（2 个 cwd 前缀匹配会话 → 计数 2）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ sessionId: "s-1", cwd: "C:/projA" }),
      makeHistorySession({ sessionId: "s-2", cwd: "C:/projA/sub" }),
    ]);

    const { container } = render(<NavTree />);
    // 挂载即 scan 为 async——等待计数落地（同上一用例时序）
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史2/);
      return el;
    });
    expect(node.textContent).toMatch(/历史/);
    // 计数 pill 文本 = 2
    const pill = Array.from(node.querySelectorAll("*")).find(
      (el) =>
        (el as HTMLElement).style.backgroundColor === hexToRgb(SIDEBAR_BG),
    ) as HTMLElement | undefined;
    expect(pill?.textContent).toBe("2");
  });
});

// ═══════════════════════════════════════════════════════════════
// 历史归属：cwd 前缀匹配项目 rootPath
// ═══════════════════════════════════════════════════════════════

describe("历史会话项目归属（cwd 前缀匹配）", () => {
  it("cwd 前缀匹配项目 rootPath → 会话挂该项目下；不匹配 → 该项目无历史", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedProject("C:/projB", "proj-B", "项目B", [
      { pageId: "pageB", name: "页面 B" },
    ]);
    seedActivePage("pageA");
    // s-1 cwd 前缀匹配 projA；s-2 cwd 与 projA/projB 均不匹配
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ sessionId: "s-1", cwd: "C:/projA/sub", title: "A 的历史会话" }),
      makeHistorySession({ sessionId: "s-2", cwd: "C:/elsewhere", title: "无归属会话" }),
    ]);

    const { container } = render(<NavTree />);
    const [nodeA, nodeB] = getRows(container, "nav-history-node");
    // 节点按项目顺序渲染（proj-A 在前）
    expect(nodeA.textContent).toContain("历史");
    expect(nodeB.textContent).toContain("历史");

    // 展开 projA 历史 → 渲染 A 的历史会话行
    fireEvent.click(nodeA);
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(nodeA.textContent).toContain("A 的历史会话");
    });
    expect(nodeA.textContent).not.toContain("无归属会话");

    // 展开 projB 历史 → 无历史行（s-2 不归属）
    // scan 调用次数 = 挂载即扫（NAV-10 契约）+ 两次展开刷新 ≥ 2——容错断言
    fireEvent.click(nodeB);
    await waitFor(() => {
      expect(mockScanHistory.mock.calls.length).toBeGreaterThanOrEqual(2);
    });
    expect(nodeB.textContent).not.toContain("无归属会话");
  });
});

// ═══════════════════════════════════════════════════════════════
// 历史行构成（StatusDot + logo + 标题 + 相对时间 + title tooltip）
// ═══════════════════════════════════════════════════════════════

describe("历史行构成", () => {
  it("展开历史节点 → 历史行 = StatusDot + CLI logo 14px + 标题 + 右侧相对时间", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ title: "修复历史恢复" }),
    ]);

    const { container } = render(<NavTree />);
    const node = await expandHistoryNode(container, "历史");

    // 历史行标题渲染
    const titleEl = Array.from(node.querySelectorAll("*")).find(
      (el) => el.textContent === "修复历史恢复",
    );
    expect(titleEl).toBeTruthy();

    // StatusDot（mock span）+ CLI logo 14px（按 cliId 查 profile.iconSrc）
    expect(node.querySelector('[data-testid="status-dot"]')).toBeTruthy();
    const logoImg = node.querySelector('img[alt="CLI 图标"]');
    expect(logoImg).toBeTruthy();
    expect(logoImg?.getAttribute("src")).toBe("/cli-icons/claude.png");
    expect(logoImg?.getAttribute("width")).toBe("14");
    expect(logoImg?.getAttribute("height")).toBe("14");

    // 右侧相对时间（真实 formatRelativeTime：5 分钟前）
    expect(node.textContent).toContain("5 分钟前");
  });

  it("prompt 预览 → 行容器原生 title tooltip（预览文本不再渲染为可见文本）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ firstPrompt: "重构历史行预览" }),
    ]);

    const { container } = render(<NavTree />);
    const node = await expandHistoryNode(container, "历史");

    // title 属性承载预览（原生 tooltip）
    expect(
      node.querySelector('[title*="重构历史行预览"]'),
    ).toBeTruthy();
    // 预览文本不作为可见行内容（单行化：第二行预览删除）
    expect(
      Array.from(node.querySelectorAll("*")).some(
        (el) =>
          el.children.length === 0 && el.textContent === "重构历史行预览",
      ),
    ).toBe(false);
  });

  it("空历史 → 展开后无历史行渲染（空态，不抛异常）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([]);

    const { container } = render(<NavTree />);
    const node = await expandHistoryNode(container, "历史");

    // scan 空 → 无历史行（无任何会话标题/logo）
    expect(node.querySelector('img[alt="CLI 图标"]')).toBeNull();
    expect(node.querySelector('[data-testid="status-dot"]')).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════
// 双击恢复 + 右键菜单（沿用 historyContextMenu 策略）
// ═══════════════════════════════════════════════════════════════

describe("历史行交互", () => {
  it("双击历史行 → restoreHistorySession(session)（四步恢复编排入口）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ sessionId: "s-dblclick" }),
    ]);

    const { container } = render(<NavTree />);
    const node = await expandHistoryNode(container, "历史");

    // 双击历史行（标题元素事件冒泡到行容器）
    const titleEl = Array.from(node.querySelectorAll("*")).find(
      (el) => el.textContent === "重构导航树",
    ) as HTMLElement;
    expect(titleEl).toBeTruthy();
    fireEvent.doubleClick(titleEl);

    await waitFor(() => {
      expect(mockRestoreHistorySession).toHaveBeenCalledWith(
        expect.objectContaining({ sessionId: "s-dblclick" }),
        expect.anything(),
      );
    });
  });

  it("历史行右键菜单：复制恢复命令/分支恢复/删除（historyContextMenu 策略），无「打开 Hooks 配置」", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([makeHistorySession()]);

    const { container, queryByText } = render(<NavTree />);
    const node = await expandHistoryNode(container, "历史");

    const titleEl = Array.from(node.querySelectorAll("*")).find(
      (el) => el.textContent === "重构导航树",
    ) as HTMLElement;
    fireEvent.contextMenu(titleEl);

    // claude profile supportsFork=true → 三项齐全（NAV-03 沿用 historyContextMenu 策略）
    expect(queryByText("复制恢复命令")).toBeTruthy();
    expect(queryByText("分支恢复")).toBeTruthy();
    expect(queryByText("删除")).toBeTruthy();
    // 入口唯一化：历史行菜单同样无「打开 Hooks 配置」
    expect(queryByText("打开 Hooks 配置")).toBeNull();
  });
});
