// open-settings.test.ts — 活动栏「配置」钮公共编排 L2 测试（F11 / R1）
//
// openSettings 迁移自 openHooksConfigFromActivityBar 编排：
//   - 无项目 → toast「请先创建项目」+ return，不切页（R1 修订：取代原静默 return）
//   - 目标项目：当前活跃页面所属项目优先，兜底第一个项目
//   - 目标页面：已有操作页面 → pages[0]；无 → 新建空布局页面
//   - 顺序契约：先 switchToPageShared 切页（面板只能在活跃页面打开）再 openSettingsPanel
//   - settingsPageId 深链透传 openSettingsPanel
//
// Mock 策略：stores/projects、stores/layout 真实 + setState 种子；
// workspace/pageApis mock（switchToPageShared/openSettingsPanel）；
// navTree/NavTree mock（makeEmptyLayout——避免加载 NavTree 组件依赖链）；
// lib barrel importOriginal mock（toast.show 可断言，其余导出保留真实实现）。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── Hoisted mocks ───
const {
  mockSwitchToPageShared,
  mockOpenSettingsPanel,
  mockMakeEmptyLayout,
  mockToastShow,
} = vi.hoisted(() => ({
  mockSwitchToPageShared: vi.fn(() => Promise.resolve()),
  mockOpenSettingsPanel: vi.fn(() => Promise.resolve(true)),
  mockMakeEmptyLayout: vi.fn(() => ({})),
  mockToastShow: vi.fn(),
}));

vi.mock("../workspace/pageApis", () => ({
  switchToPageShared: mockSwitchToPageShared,
  openSettingsPanel: mockOpenSettingsPanel,
}));

vi.mock("../features/navTree/NavTree", () => ({
  makeEmptyLayout: mockMakeEmptyLayout,
}));

vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return { ...actual, toast: { ...actual.toast, show: mockToastShow } };
});

import { openSettings } from "../features/settingsCenter/openSettings";
import { useProjects } from "../stores/projects";
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
  mockOpenSettingsPanel.mockClear();
  mockMakeEmptyLayout.mockClear();
  mockMakeEmptyLayout.mockReturnValue({});
  mockToastShow.mockClear();
});

describe("openSettings（F11 配置钮入口）", () => {
  it("无任何项目 → toast「请先创建项目」且不切页、不开面板（R1）", async () => {
    await openSettings();

    expect(mockToastShow).toHaveBeenCalledWith("warning", "请先创建项目");
    expect(mockSwitchToPageShared).not.toHaveBeenCalled();
    expect(mockOpenSettingsPanel).not.toHaveBeenCalled();
  });

  it("活跃页面所属项目优先于第一个项目", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);
    seedProject("proj-2", "C:/p2", [{ pageId: "page-2", name: "页面 2" }]);
    useLayout.setState({ activePageId: "page-2" });

    await openSettings();

    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-2");
    expect(mockOpenSettingsPanel).toHaveBeenCalledWith("page-2", undefined);
  });

  it("无活跃页面 → 兜底第一个项目 pages[0]", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);
    seedProject("proj-2", "C:/p2", [{ pageId: "page-2", name: "页面 2" }]);

    await openSettings();

    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-1");
    expect(mockOpenSettingsPanel).toHaveBeenCalledWith("page-1", undefined);
  });

  it("切页先于开面板（invocationCallOrder 契约）", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);

    await openSettings();

    expect(mockSwitchToPageShared).toHaveBeenCalledWith("page-1");
    expect(mockOpenSettingsPanel).toHaveBeenCalledWith("page-1", undefined);
    const switchOrder = mockSwitchToPageShared.mock.invocationCallOrder[0];
    const openOrder = mockOpenSettingsPanel.mock.invocationCallOrder[0];
    expect(switchOrder).toBeLessThan(openOrder);
  });

  it("settingsPageId 深链透传 openSettingsPanel 第二参", async () => {
    seedProject("proj-1", "C:/p1", [{ pageId: "page-1", name: "页面 1" }]);

    await openSettings("planBalance");

    expect(mockOpenSettingsPanel).toHaveBeenCalledWith("page-1", "planBalance");
  });

  it("项目无操作页面 → 新建空布局页面后切页打开（照 handleNewPage 模式）", async () => {
    seedProject("proj-1", "C:/p1", []);

    await openSettings();

    expect(mockMakeEmptyLayout).toHaveBeenCalled();
    const proj = useProjects.getState().projects["proj-1"];
    expect(proj.pages).toHaveLength(1);
    expect(proj.pages[0].layout).toEqual({});
    const newPageId = proj.pages[0].pageId;
    expect(mockSwitchToPageShared).toHaveBeenCalledWith(newPageId);
    expect(mockOpenSettingsPanel).toHaveBeenCalledWith(newPageId, undefined);
  });
});
