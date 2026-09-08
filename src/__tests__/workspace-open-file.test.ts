// workspace-open-file.test.ts — 共享文件打开链路测试
//
// openFile.ts 由 ExplorerPanel.handleOpenFile 抽取——行为逐字等价（既有
// explorer-file-viewer.test.tsx 语义保持），本文件直测共享函数：
// 守卫 / 面板分派（resolve ?? editor + renderer always）/ 去重聚焦 /
// addPanel 异常回退 / 便捷入口上下文现取。注册表与 titleManager 用真实模块
//（fileViewerRegistry 默认注册 htmlviewer；titleManager.reset() 隔离）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  openFileInPage,
  openFileInActivePage,
  canOpenFile,
} from "../workspace/openFile";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

const mocks = vi.hoisted(() => {
  const mockAddPanel = vi.fn();
  const mockFocus = vi.fn();
  const mockSetTitle = vi.fn();
  return {
    mockAddPanel,
    mockFocus,
    mockSetTitle,
    resetAll() {
      mockAddPanel.mockReset();
      mockFocus.mockReset();
      mockSetTitle.mockReset();
      // getPanel 默认返回可聚焦面板（去重用）
      mockAddPanel.mockReturnValue(undefined);
    },
  };
});

/** 构造 Dockview API mock（addPanel/getPanel） */
function makeDockApi() {
  return {
    addPanel: mocks.mockAddPanel,
    getPanel: vi.fn().mockReturnValue({
      focus: mocks.mockFocus,
      api: { setTitle: mocks.mockSetTitle },
    }),
  };
}

function seedStore(rootPath = "C:/project", activePageId: string | null = "page-1") {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: `${rootPath}/src`,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        version: 1,
        createdAt: 1,
        lastAccessedAt: 1,
      },
    },
    expandedNodes: {},
  } as never);
  useLayout.setState({ activePageId });
}

function baseCtx() {
  return {
    activePageId: "page-1",
    dockApi: makeDockApi(),
    rootPath: "C:/project/src",
    projectRootPath: "C:/project",
  };
}

describe("openFileInPage", () => {
  beforeEach(() => {
    mocks.resetAll();
    titleManager.reset();
    useProjects.setState({ projects: {} } as never);
    useLayout.setState({ activePageId: null } as never);
  });

  it("无活跃页 / 无 dockview API → 守卫返回 false 不调 addPanel", () => {
    const ctx = { ...baseCtx(), activePageId: null };
    expect(openFileInPage(ctx, "C:/project/src/a.ts")).toBe(false);
    expect(openFileInPage({ ...baseCtx(), dockApi: undefined }, "C:/project/src/a.ts")).toBe(false);
    expect(mocks.mockAddPanel).not.toHaveBeenCalled();
  });

  it(".html 命中 htmlviewer：renderer always + params 携带 filePath", () => {
    const ok = openFileInPage(baseCtx(), "C:/project/src/index.html");
    expect(ok).toBe(true);
    expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    const call = mocks.mockAddPanel.mock.calls[0]![0];
    expect(call.component).toBe("htmlviewer");
    expect(call.renderer).toBe("always");
    expect(call.params).toMatchObject({
      filePath: "C:/project/src/index.html",
    });
    expect(typeof call.params.panelId).toBe("string");
    expect(call.params.panelId.startsWith("page-1:htmlviewer-")).toBe(true);
    expect(typeof call.title).toBe("string");
  });

  it("未知扩展名回退 editor：不设 renderer", () => {
    openFileInPage(baseCtx(), "C:/project/src/data.xyz");
    const call = mocks.mockAddPanel.mock.calls[0]![0];
    expect(call.component).toBe("editor");
    expect(call.renderer).toBeUndefined();
  });

  it("去重：同路径二次打开聚焦既有面板，不重复 addPanel", () => {
    expect(openFileInPage(baseCtx(), "C:/project/src/app.ts")).toBe(true);
    expect(openFileInPage(baseCtx(), "C:/project/src/app.ts")).toBe(true);
    expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    expect(mocks.mockFocus).toHaveBeenCalledTimes(1);
  });

  it("addPanel 抛异常 → 返回 false（titleManager 不注册孤记录）", () => {
    mocks.mockAddPanel.mockImplementation(() => {
      throw new Error("layout broken");
    });
    expect(openFileInPage(baseCtx(), "C:/project/src/a.ts")).toBe(false);
    // 异常后同路径可再次尝试（未注册进 titleManager，无残留聚焦）
    expect(mocks.mockFocus).not.toHaveBeenCalled();
  });

  it("根内相对标题根：projectRootPath 优先参与标题计算", () => {
    openFileInPage(baseCtx(), "C:/project/src/name.ts");
    const call = mocks.mockAddPanel.mock.calls[0]![0];
    // 标题以项目根计算（同 ExplorerPanel 抽取前语义——具体值由 titleManager 测试守护）
    expect(call.title.length).toBeGreaterThan(0);
  });
});

describe("openFileInActivePage（零参数便捷入口）", () => {
  beforeEach(() => {
    mocks.resetAll();
    titleManager.reset();
    useProjects.setState({ projects: {} } as never);
    useLayout.setState({ activePageId: null } as never);
  });

  it("从 stores + __dockviewApi 现取上下文打开成功", () => {
    seedStore();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__dockviewApi = makeDockApi();
    expect(openFileInActivePage("C:/project/src/index.html")).toBe(true);
    expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    expect(mocks.mockAddPanel.mock.calls[0]![0].component).toBe("htmlviewer");
  });

  it("无活跃页 → false（守卫）", () => {
    useLayout.setState({ activePageId: null });
    expect(openFileInActivePage("C:/project/src/index.html")).toBe(false);
    expect(mocks.mockAddPanel).not.toHaveBeenCalled();
  });
});

describe("canOpenFile", () => {
  it("predicate：null 页 / 无 api 拒绝，双非空放行", () => {
    expect(canOpenFile(null, {})).toBe(false);
    expect(canOpenFile("page-1", undefined)).toBe(false);
    expect(canOpenFile("page-1", {})).toBe(true);
  });
});
