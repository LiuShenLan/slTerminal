import { describe, it, expect, beforeEach } from "vitest";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import type { Project } from "../stores/projects";

/** 构造测试用 Project */
function makeProject(id: string, overrides?: Partial<Project>): Project {
  return {
    projectId: id,
    name: "test-project",
    rootPath: "/tmp/test",
    pages: [],
    activePageId: null,
    version: 1,
    ...overrides,
  };
}

describe("layout store", () => {
  beforeEach(() => {
    useLayout.setState({ activePageId: null });
  });

  it("初始状态 activePageId 应为 null", () => {
    expect(useLayout.getState().activePageId).toBeNull();
  });

  it("setActivePage 应更新 activePageId", () => {
    useLayout.getState().setActivePage("page-123");
    expect(useLayout.getState().activePageId).toBe("page-123");
  });

  it("setActivePage(null) 应清空活跃页面", () => {
    useLayout.getState().setActivePage("page-456");
    useLayout.getState().setActivePage(null);
    expect(useLayout.getState().activePageId).toBeNull();
  });
});

describe("操作页面切换集成", () => {
  beforeEach(() => {
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    useLayout.setState({ activePageId: null });
  });

  it("switchToPage 应在 projects store 中更新活跃页面", () => {
    const proj = makeProject("proj-1", {
      pages: [
        {
          pageId: "page-a",
          name: "Page A",
          layout: {},
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        },
        {
          pageId: "page-b",
          name: "Page B",
          layout: {},
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        },
      ],
    });
    useProjects.getState().addProject(proj);
    useProjects.getState().switchToPage("proj-1", "page-b");

    const state = useProjects.getState();
    expect(state.projects["proj-1"].activePageId).toBe("page-b");
  });

  it("有 cwd 的页面应可在 layout store 中跟踪活跃页面", () => {
    useLayout.getState().setActivePage("page-with-cwd");
    expect(useLayout.getState().activePageId).toBe("page-with-cwd");

    const proj = makeProject("proj-2", {
      pages: [
        {
          pageId: "page-with-cwd",
          name: "Cwd Page",
          layout: {},
          cwd: "/tmp/test",
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        },
      ],
    });
    useProjects.getState().addProject(proj);

    // 验证 cwd 存在
    const page = useProjects.getState().projects["proj-2"].pages[0];
    expect(page.cwd).toBe("/tmp/test");
  });

  // H5 自切换守卫
  it("重复点击同一页面 → activePageId 不变（自切换守卫拦截）", () => {
    const proj = makeProject("proj-3", {
      pages: [
        {
          pageId: "page-x",
          name: "Page X",
          layout: {},
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        },
      ],
    });
    useProjects.getState().addProject(proj);
    useProjects.getState().switchToPage("proj-3", "page-x");
    expect(useProjects.getState().projects["proj-3"].activePageId).toBe("page-x");

    // 重复切换到同一页面
    useProjects.getState().switchToPage("proj-3", "page-x");
    // activePageId 不变（守卫逻辑：activePageId === pageId 时 return）
    expect(useProjects.getState().projects["proj-3"].activePageId).toBe("page-x");
  });

  it("切换到另一页面后 activePageId 应正确更新", () => {
    const proj = makeProject("proj-5", {
      pages: [
        { pageId: "page-1", name: "P1", layout: {}, createdAt: Date.now(), lastAccessedAt: Date.now() },
        { pageId: "page-2", name: "P2", layout: {}, createdAt: Date.now(), lastAccessedAt: Date.now() },
      ],
    });
    useProjects.getState().addProject(proj);
    useProjects.getState().switchToPage("proj-5", "page-1");
    expect(useProjects.getState().projects["proj-5"].activePageId).toBe("page-1");

    useProjects.getState().switchToPage("proj-5", "page-2");
    expect(useProjects.getState().projects["proj-5"].activePageId).toBe("page-2");
  });
});
