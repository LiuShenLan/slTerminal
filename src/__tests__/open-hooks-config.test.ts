// open-hooks-config.test.ts — 活动栏「配置」钮公共编排 L2 测试（NAV-05）
//
// openHooksConfigFromActivityBar 迁移自 SidebarTree「打开 Hooks 配置」菜单逻辑
// （决策 4 入口唯一化——菜单项已随 SidebarTree 退役，配置钮为唯一入口）：
//   - 目标项目：当前活跃页面所属项目优先，兜底第一个项目；无项目 → 无操作
//   - 目标页面：已有操作页面 → pages[0]；无 → 新建空布局页面
//   - 顺序契约：先 switchToPageShared 切页（hooksConfig 面板只能在活跃页面打开，
//     C13-7）再 openHooksConfigPanel
//
// Mock 策略：stores/projects、stores/layout 真实 + setState 种子；
// workspace/pageApis mock（switchToPageShared/openHooksConfigPanel）；
// navTree/NavTree mock（makeEmptyLayout——避免加载 NavTree 组件依赖链）。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── Hoisted mocks ───
const { mockSwitchToPageShared, mockOpenHooksConfigPanel, mockMakeEmptyLayout } =
  vi.hoisted(() => ({
    mockSwitchToPageShared: vi.fn(() => Promise.resolve()),
    mockOpenHooksConfigPanel: vi.fn(() => Promise.resolve(true)),
    mockMakeEmptyLayout: vi.fn(() => ({})),
  }));

vi.mock("../workspace/pageApis", () => ({
  switchToPageShared: mockSwitchToPageShared,
  openHooksConfigPanel: mockOpenHooksConfigPanel,
}));

vi.mock("../features/navTree/NavTree", () => ({
  makeEmptyLayout: mockMakeEmptyLayout,
}));

import { openHooksConfigFromActivityBar } from "../features/hooksConfig/openHooksConfig";
import { useProjects, createPageId } from "../stores/projects";
import { useLayout } from "../stores/layout";

/** 种子项目：pages 可为空 */
function seedProject(
  projectId: string,
  rootPath: string,
  pages: Array<{ pageId: string; name: string }> = [],
): void {
  useProjects.setState((prev) => ({
    projects: {
      ...prev.projects,
      [projectId]: {
        projectId,
        name: `项目${projectId}`,
        rootPath,
        pages: pages.map((p) => ({
          pageId: p.pageId,
          name: p.name,
          layout: {},
          cwd: rootPath,
          createdAt: 1,
          lastAccessedAt: 1,
        })),
        activePageId: pages[0]?.pageId ?? null,
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  }));
}

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  mockSwitchToPageShared.mockClear();
  mockOpenHooksConfigPanel.mockClear();
  mockMakeEmptyLayout.mockClear();
  mockMakeEmptyLayout.mockReturnValue({});
});

describe("openHooksConfigFromActivityBar（NAV-05 配置钮入口）", () => {
  it("无任何项目 → 无操作（不切页、不开面板）", async () => {
    await openHooksConfigFromActivityBar();
    expect(mockSwitchToPageShared).not.toHaveBeenCalled();
    expect(mockOpenHooksConfigPanel).not.toHaveBeenCalled();
  });

  it("有项目无活跃页面 → 兜底第一个项目 pages[0] → 先切页后开面板", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);
    seedProject("proj-2", "C:/p2", [{ pageId: "page-2", name: "页面 2" }]);

    await openHooksConfigFromActivityBar();

    // 调用顺序契约：切页先行（面板只能在活跃页面打开，C13-7）
    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-1");
    expect(mockOpenHooksConfigPanel).toHaveBeenCalledWith("page-1");
    const switchOrder = mockSwitchToPageShared.mock.invocationCallOrder[0];
    const openOrder = mockOpenHooksConfigPanel.mock.invocationCallOrder[0];
    expect(switchOrder).toBeLessThan(openOrder);
  });

  it("活跃页面所属项目优先于第一个项目", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);
    seedProject("proj-2", "C:/p2", [{ pageId: "page-2", name: "页面 2" }]);
    useLayout.setState({ activePageId: "page-2" });

    await openHooksConfigFromActivityBar();

    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-2");
    expect(mockOpenHooksConfigPanel).toHaveBeenCalledWith("page-2");
  });

  it("项目无操作页面 → 新建空布局页面后切页打开（照 handleNewPage 模式）", async () => {
    seedProject("proj-1", "C:/p1", []);

    await openHooksConfigFromActivityBar();

    // 新建页面：addPage + makeEmptyLayout 空布局
    expect(mockMakeEmptyLayout).toHaveBeenCalled();
    const proj = useProjects.getState().projects["proj-1"];
    expect(proj.pages).toHaveLength(1);
    expect(proj.pages[0].layout).toEqual({});
    const newPageId = proj.pages[0].pageId;
    expect(mockSwitchToPageShared).toHaveBeenCalledWith(newPageId);
    expect(mockOpenHooksConfigPanel).toHaveBeenCalledWith(newPageId);
  });

  it("活跃页面指向已删除项目 → 兜底第一个项目", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);
    // 幽灵 activePageId（指向不存在的项目）
    useLayout.setState({ activePageId: createPageId() });

    await openHooksConfigFromActivityBar();

    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-1");
    expect(mockOpenHooksConfigPanel).toHaveBeenCalledWith("page-1");
  });
});
