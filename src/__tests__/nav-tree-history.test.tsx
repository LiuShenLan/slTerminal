// nav-tree-history.test.tsx — 导航树历史折叠节点 L2 测试（NAV-10 L2 部分，NAV-03）
//
// 规格契约（写死于 NAV-03 决策，登记于 src/features/navTree/CLAUDE.md）：
//   - 历史会话折叠节点挂项目下（nav-history-node）：IconHistory 时钟 +「历史session」+ 计数 pill
//     （SIDEBAR_BG 底 #1a1a1e + PLACEHOLDER_FG 字 fg-4）
//   - 归属：cwd 前缀匹配项目 rootPath
//   - 展开 = 历史行（StatusDot + logo + 标题 + 右侧相对时间，单行 30px）
//   - prompt 预览 → 原生 title tooltip（行容器 title 属性）
//   - 双击恢复 + 右键菜单（复制恢复命令/分支恢复/删除）沿用 historyContextMenu 策略
//   - 无历史会话项目（total=0）不渲染节点（NAV-10 修订 2026-09-03：节点随项目
//     展开态显示——挂载默认收起，用例先展开项目行；原「暂无历史会话」空态文案
//     随 total=0 隐藏成死代码已删除）
//
// Mock 策略（照 src/__tests__/ 既有种子模式——agent-status-view.test.tsx 先例）：
//   useAgentStatus 模块 mock（NavTree 会调用）；agentHistory scan 走真实 useAgentHistory +
//   ipc/agentHistory.scanAgentHistory mock（数据链真实：scan → sessions → 归属过滤）；
//   stores/projects、stores/layout 真实 store + setState 种子。
//   NavTree 本体由 navtree-new agent 按契约产出，本文件只锁定契约点。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// ─── Hoisted mocks ───
const {
  mockUseAgentStatus,
  mockScanHistory,
  mockListBackgroundTasks,
  mockRestoreHistorySession,
} = vi.hoisted(() => ({
  mockUseAgentStatus: vi.fn(),
  mockScanHistory: vi.fn(),
  mockListBackgroundTasks: vi.fn(),
  mockRestoreHistorySession: vi.fn(() => Promise.resolve()),
}));

vi.mock("../features/agentStatus/useAgentStatus", () => ({
  useAgentStatus: () => mockUseAgentStatus(),
}));

// 历史数据源：真实 useAgentHistory，仅 mock IPC 层 scanAgentHistory
vi.mock("../ipc/agentHistory", () => ({
  scanAgentHistory: mockScanHistory,
  deleteHistorySession: vi.fn(),
}));

// F12：调度器 activate 读配置——文件级 mock 覆盖 setup.ts 全局 mock
// （listBackgroundTasks 恒返回 sessionRefresh enabled=true intervalSec=300——
// 大间隔防 tick 干扰断言；set/onUpdated 防御 stub）
vi.mock("../ipc/backgroundTasks", () => ({
  listBackgroundTasks: mockListBackgroundTasks,
  setBackgroundTaskConfig: vi.fn().mockResolvedValue([]),
  onBackgroundTasksUpdated: vi.fn(() => () => {}),
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
import { CLAUDE_CLI_ID, claudeProfile } from "../features/cliProfiles/profiles/claude";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { backgroundTaskScheduler } from "../features/backgroundTasks/scheduler";
import { runSessionRefresh } from "../features/backgroundTasks/sessionRefreshTask";
import "../features/backgroundTasks/tasks"; // 注册触发点（side-effect import，硬约束 #13）
import { SESSION_REFRESH_TASK_ID } from "../types/backgroundTasks";
import { SIDEBAR_BG, PLACEHOLDER_FG } from "../theme/colors";

// ── 测试辅助 ──

/** sessionRefresh 任务配置（大间隔防 tick 干扰断言——300s 远超用例时长） */
const SESSION_REFRESH_CONFIG = {
  taskId: "sessionRefresh",
  title: "会话历史刷新",
  enabled: true,
  intervalSec: 300,
  intervalMin: 2,
  intervalMax: 300,
};

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

/** 展开全部项目行至页面行可见（NAV-10 修订 2026-09-03：历史节点随项目展开态
 *  渲染——项目收起时无 nav-history-node，先展开项目行；页面行出现 = 展开收敛） */
async function expandProjects(container: HTMLElement): Promise<void> {
  const rows = getRows(container, "nav-row-project");
  expect(rows.length).toBeGreaterThan(0);
  for (const row of rows) fireEvent.click(row);
  await waitFor(() => {
    expect(getRows(container, "nav-row-page").length).toBeGreaterThan(0);
  });
}

/** 展开历史折叠节点并等待 scan 落地（先展开项目行——节点随项目展开渲染） */
async function expandHistoryNode(container: HTMLElement): Promise<HTMLElement> {
  await expandProjects(container);
  const node = await waitFor(() => {
    const el = getRows(container, "nav-history-node")[0];
    expect(el).toBeTruthy();
    return el as HTMLElement;
  });
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
  mockListBackgroundTasks.mockReset();
  mockListBackgroundTasks.mockResolvedValue([SESSION_REFRESH_CONFIG]);
  mockRestoreHistorySession.mockReset();
  mockRestoreHistorySession.mockResolvedValue(undefined);
}

// F12：调度器/注册表每用例重置 + 任务重注册（_reset 清空后恢复——runSessionRefresh
// 导出供测试重注册，注册触发点仍收敛 tasks.ts）+ claude profile（history 能力参与扫描）
beforeEach(() => {
  backgroundTaskScheduler._reset();
  backgroundTaskScheduler.register({ id: SESSION_REFRESH_TASK_ID, run: runSessionRefresh });
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
  resetAll();
});

afterEach(() => {
  cleanup();
});

// ═══════════════════════════════════════════════════════════════
// 历史折叠节点：渲染与计数 pill
// ═══════════════════════════════════════════════════════════════

describe("历史折叠节点渲染", () => {
  it("项目下渲染 nav-history-node：时钟图标 +「历史session」+ 计数 pill（SIDEBAR_BG 底 + PLACEHOLDER_FG 字）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ cwd: "C:/projA" }),
    ]);

    const { container } = render(<NavTree />);
    // NAV-10 修订：节点随项目展开态渲染——先展开项目行（挂载默认收起）
    await expandProjects(container);
    // 挂载即 scan 为 async——等待计数落地后断言（节点出现 = total>0 落地）
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史session1/);
      return el;
    });

    // 「历史session」文案（NAV-03 契约 + 人工验证修订）+ 时钟图标（lucide Clock svg）
    // + 计数 pill（SIDEBAR_BG 底 + PLACEHOLDER_FG 字，文本 = 匹配会话数）
    // ——重渲染可能未稳定，统一在 waitFor 内等待落地
    await waitFor(() => {
      expect(node.textContent).toContain("历史session");
      expect(node.querySelector("svg")).toBeTruthy();

      const pill = Array.from(node.querySelectorAll("*")).find(
        (el) =>
          (el as HTMLElement).style.backgroundColor === hexToRgb(SIDEBAR_BG) &&
          (el as HTMLElement).style.color === hexToRgb(PLACEHOLDER_FG),
      ) as HTMLElement | undefined;
      expect(pill).toBeTruthy();
      expect(pill?.textContent).toBe("1");
    });
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
    // NAV-10 修订：先展开项目行（节点随项目展开态渲染）
    await expandProjects(container);
    // 挂载即 scan 为 async——等待计数落地（同上一用例时序）
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史session2/);
      return el;
    });
    // 重渲染可能未稳定——textContent/计数 pill 断言统一在 waitFor 内等待落地
    await waitFor(() => {
      expect(node.textContent).toMatch(/历史session/);
      // 计数 pill 文本 = 2
      const pill = Array.from(node.querySelectorAll("*")).find(
        (el) =>
          (el as HTMLElement).style.backgroundColor === hexToRgb(SIDEBAR_BG),
      ) as HTMLElement | undefined;
      expect(pill?.textContent).toBe("2");
    });
  });

  it("防回归：项目收起（默认态）不渲染 nav-history-node——展开出现、再收起消失（NAV-10 修订废除常驻）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([makeHistorySession()]);

    const { container } = render(<NavTree />);
    // 收起态：即使 scan 落地（total=1）也不渲染节点——历史节点不再常驻项目下
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    expect(getRows(container, "nav-history-node")).toHaveLength(0);

    // 展开项目 → 节点出现
    const projRow = getRows(container, "nav-row-project")[0];
    fireEvent.click(projRow);
    await waitFor(() => {
      expect(getRows(container, "nav-history-node")).toHaveLength(1);
    });
    // 收起 → 整体消失
    fireEvent.click(projRow);
    await waitFor(() => {
      expect(getRows(container, "nav-history-node")).toHaveLength(0);
    });
    // 再展开 → 再现（toggle 反复）
    fireEvent.click(projRow);
    await waitFor(() => {
      expect(getRows(container, "nav-history-node")).toHaveLength(1);
    });
  });

  it("展开后历史节点位于页面行之后（同 childrenStyle 容器末位——恒置最下方）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA1", name: "页面 A1" },
      { pageId: "pageA2", name: "页面 A2" },
    ]);
    seedActivePage("pageA1");
    mockScanHistory.mockResolvedValue([makeHistorySession()]);

    const { container } = render(<NavTree />);
    await expandProjects(container);
    // 等 scan 落地节点出现
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el).toBeTruthy();
      return el as HTMLElement;
    });
    // 展开容器 = 项目行父容器的第 2 子级（childrenStyle）；页面行先行渲染于容器内
    const sub = getRows(container, "nav-row-project")[0].parentElement
      ?.children[1] as HTMLElement;
    expect(getRows(container, "nav-row-page").length).toBe(2);
    expect(sub.style.marginLeft).toBe("15px"); // childrenStyle 缩进（同级）
    // 恒置最下方：历史节点是子容器最后一个元素（两页面行 div 之后）
    expect(sub.children[sub.children.length - 1]).toBe(node);
  });
});

// ═══════════════════════════════════════════════════════════════
// 历史归属：cwd 前缀匹配项目 rootPath
// ═══════════════════════════════════════════════════════════════

describe("历史会话项目归属（cwd 前缀匹配）", () => {
  it("cwd 前缀匹配项目 rootPath → 会话挂该项目下；无归属项目会话不展示（total=0 项目无节点）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedProject("C:/projB", "proj-B", "项目B", [
      { pageId: "pageB", name: "页面 B" },
    ]);
    seedActivePage("pageA");
    // s-1 cwd 前缀匹配 projA；s-2 cwd 与 projA/projB 均不匹配（孤儿目录不展示）
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ sessionId: "s-1", cwd: "C:/projA/sub", title: "A 的历史会话" }),
      makeHistorySession({ sessionId: "s-2", cwd: "C:/elsewhere", title: "无归属会话" }),
    ]);

    const { container } = render(<NavTree />);
    // NAV-10 修订：先展开两项目行（节点随项目展开态渲染）
    await expandProjects(container);
    // 仅 projA 有归属会话 → 唯一节点属 A；projB 无历史 → 无节点（total=0 隐藏）
    const nodeA = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史session1/);
      return el;
    });
    expect(getRows(container, "nav-history-node")).toHaveLength(1);

    // 展开 projA 历史 → 渲染 A 的历史会话行（s-2 无归属不渲染）
    fireEvent.click(nodeA);
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(nodeA.textContent).toContain("A 的历史会话");
    });
    expect(nodeA.textContent).not.toContain("无归属会话");

    // FE-19：扫描次数恒为 1（订阅首轮即扫一次，NAV-10 契约——useAgentHistory
    // 首个订阅者立即执行一轮，接管「挂载即扫」语义），展开/折叠不触发 scan
    expect(mockScanHistory).toHaveBeenCalledTimes(1);
  });
});

// ═══════════════════════════════════════════════════════════════
// FE-16：历史归属 Map 索引（嵌套 rootPath 最深前缀命中）
// ═══════════════════════════════════════════════════════════════

describe("历史会话项目归属（FE-16 索引：嵌套 rootPath 最深前缀命中）", () => {
  it("cwd 命中嵌套 rootPath → 归属最深前缀项目（根项目计数 0）", async () => {
    seedProject("C:/root", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedProject("C:/root/sub", "proj-B", "项目B", [
      { pageId: "pageB", name: "页面 B" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ sessionId: "s-1", cwd: "C:/root/sub/x", title: "嵌套项目会话" }),
      makeHistorySession({ sessionId: "s-2", cwd: "C:/root/other", title: "根项目会话" }),
    ]);

    const { container } = render(<NavTree />);
    // NAV-10 修订：先展开两项目行（节点随项目展开态渲染）
    await expandProjects(container);
    // 挂载即 scan——等待计数落地；s-1 cwd C:/root/sub/x 最深前缀命中 proj-B、
    // s-2 仅命中 proj-A（根）→ 两项目各 1（旧首命中实现下 proj-A 会得 2、proj-B 0）
    const [nodeA, nodeB] = await waitFor(() => {
      const els = getRows(container, "nav-history-node");
      expect(els.length).toBe(2);
      expect(els[0].textContent).toMatch(/历史session1/);
      expect(els[1].textContent).toMatch(/历史session1/);
      return els as [HTMLElement, HTMLElement];
    });

    // 展开 projB → 渲染嵌套项目会话（projA 不误含——s-1 未归属根项目）
    fireEvent.click(nodeB);
    await waitFor(() => {
      expect(nodeB.textContent).toContain("嵌套项目会话");
    });
    expect(nodeB.textContent).not.toContain("根项目会话");
    expect(nodeA.textContent).not.toContain("嵌套项目会话");
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
    const node = await expandHistoryNode(container);

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
    const node = await expandHistoryNode(container);

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

  it("无历史会话项目 → 展开后无 nav-history-node（total=0 隐藏；空态文案随死代码删除）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([]);

    const { container } = render(<NavTree />);
    await expandProjects(container);
    // scan 空落地（total=0）后节点恒不渲染——不再显示「历史session 0」占位
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    expect(getRows(container, "nav-history-node")).toHaveLength(0);
    // 原「暂无历史会话」空态已随 total=0 隐藏成为死代码（NAV-10 修订 2026-09-03）
    expect(container.textContent).not.toContain("暂无历史会话");
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
    const node = await expandHistoryNode(container);

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
    const node = await expandHistoryNode(container);

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

// ═══════════════════════════════════════════════════════════════
// FE-19：扫描时机——订阅首轮一次，展开不重复 scan，刷新钮 triggerNow 显式重扫
// ═══════════════════════════════════════════════════════════════

describe("历史扫描时机（FE-19：订阅首轮 + 展开不重复 scan + 刷新钮 triggerNow）", () => {
  it("订阅首轮即扫一次（挂载即扫语义）；展开/折叠历史节点不触发第二次 scan", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([makeHistorySession()]);

    const { container } = render(<NavTree />);
    // 订阅首轮（useAgentHistory 首个订阅者立即执行一轮）——等 scan 落地
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalledTimes(1);
    });
    // NAV-10 修订：展开项目行——节点随项目展开态渲染（不展开则计数不可见）
    await expandProjects(container);
    // 等计数 pill 落地（数据驱动渲染完成）
    const node = await waitFor(() => {
      const el = getRows(container, "nav-history-node")[0];
      expect(el?.textContent).toMatch(/历史session1/);
      return el as HTMLElement;
    });

    // 展开 → 仍 1 次（FE-19：不重复 scan）
    fireEvent.click(node);
    await waitFor(() => {
      expect(getRows(container, "nav-row-session").length).toBe(1);
    });
    expect(mockScanHistory).toHaveBeenCalledTimes(1);

    // 折叠再展开 → 仍 1 次
    fireEvent.click(node);
    await waitFor(() => {
      expect(getRows(container, "nav-row-session").length).toBe(0);
    });
    fireEvent.click(node);
    await waitFor(() => {
      expect(getRows(container, "nav-row-session").length).toBe(1);
    });
    expect(mockScanHistory).toHaveBeenCalledTimes(1);
  });

  it("「导航」头刷新钮 → triggerNow 手动重扫（scan 第二次调用且 force=true）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([]);

    const { container } = render(<NavTree />);
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalledTimes(1);
    });
    // 订阅首轮：按 CLAUDE_CLI_ID 扫描，force 恒 true（执行体内恒定，规格 §8）
    expect(mockScanHistory).toHaveBeenCalledWith("claude", true);

    // NAV-10 修订：先展开项目行——初始 total=0 无节点，刷新数据落地后节点才出现
    await expandProjects(container);
    expect(getRows(container, "nav-history-node")).toHaveLength(0);

    // 新会话落盘 → 点刷新钮 → triggerNow 手动重扫（与定时 tick 同一执行体，
    // force=true 绕过 BE-19 缓存——契约断链接线回归）
    mockScanHistory.mockResolvedValue([makeHistorySession()]);
    const refreshBtn = container.querySelector(
      'button[aria-label="刷新"]',
    ) as HTMLElement;
    expect(refreshBtn).toBeTruthy();
    fireEvent.click(refreshBtn);

    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalledTimes(2);
    });
    expect(mockScanHistory).toHaveBeenLastCalledWith("claude", true);
    // 数据落地：计数 pill 更新（节点出现 = total 从 0 转 1）
    await waitFor(() => {
      expect(
        getRows(container, "nav-history-node")[0].textContent,
      ).toMatch(/历史session1/);
    });
  });
});

// ═══════════════════════════════════════════════════════════════
// 搜索与历史节点（NAV-10 修订：节点随项目展开渲染后，searching 命中链仍自动展开）
// ═══════════════════════════════════════════════════════════════

describe("历史节点与搜索", () => {
  it("query 命中历史行标题 → 项目与历史节点自动展开、命中行可见（无需手动展开）", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ title: "重构导航树" }),
    ]);

    const { container } = render(<NavTree />);
    // 等 scan 落地（渲染条件 total>0 依赖数据就绪）
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    const input = container.querySelector(
      'input[placeholder="搜索项目 / 页面 / 会话…"]',
    ) as HTMLInputElement;
    expect(input).toBeTruthy();
    fireEvent.change(input, { target: { value: "重构" } });

    // 命中链自动展开：searching 覆盖手动展开态——节点与命中行直接可见
    await waitFor(() => {
      const node = getRows(container, "nav-history-node")[0];
      expect(node?.textContent).toContain("重构导航树");
    });
  });

  it("query 命中页面但历史无命中 → 项目展开但历史节点不显示", async () => {
    seedProject("C:/projA", "proj-A", "项目A", [
      { pageId: "pageA", name: "页面 A" },
    ]);
    seedActivePage("pageA");
    mockScanHistory.mockResolvedValue([
      makeHistorySession({ title: "重构导航树" }),
    ]);

    const { container } = render(<NavTree />);
    await waitFor(() => {
      expect(mockScanHistory).toHaveBeenCalled();
    });
    const input = container.querySelector(
      'input[placeholder="搜索项目 / 页面 / 会话…"]',
    ) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "页面" } });

    // 页面行命中 → 项目自动展开（searching 覆盖手动展开态）
    await waitFor(() => {
      expect(getRows(container, "nav-row-page").length).toBe(1);
    });
    // 历史标题未命中 → 历史节点不显示
    expect(getRows(container, "nav-history-node")).toHaveLength(0);
  });
});
