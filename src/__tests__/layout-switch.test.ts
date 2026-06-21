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
    worktrees: [],
    pages: [],
    activePageId: null,
    version: 1,
    ...overrides,
  };
}

describe("layout store", () => {
  beforeEach(() => {
    useLayout.setState({ activePageId: null, isLayoutSwitching: false });
  });

  it("初始状态 activePageId 应为 null", () => {
    expect(useLayout.getState().activePageId).toBeNull();
  });

  it("setActivePage 应更新 activePageId", () => {
    useLayout.getState().setActivePage("page-123");
    expect(useLayout.getState().activePageId).toBe("page-123");
  });

  it("setLayoutSwitching 应切换布局锁状态", () => {
    expect(useLayout.getState().isLayoutSwitching).toBe(false);
    useLayout.getState().setLayoutSwitching(true);
    expect(useLayout.getState().isLayoutSwitching).toBe(true);
    useLayout.getState().setLayoutSwitching(false);
    expect(useLayout.getState().isLayoutSwitching).toBe(false);
  });
});

describe("操作页面切换集成", () => {
  beforeEach(() => {
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    useLayout.setState({ activePageId: null, isLayoutSwitching: false });
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

  it("有 binding 的页面应可在 layout store 中跟踪活跃页面", () => {
    useLayout.getState().setActivePage("page-with-binding");
    expect(useLayout.getState().activePageId).toBe("page-with-binding");

    const proj = makeProject("proj-2", {
      pages: [
        {
          pageId: "page-with-binding",
          name: "Bound Page",
          layout: {},
          binding: { worktreePath: "/tmp/wt", branchName: "feat" },
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        },
      ],
    });
    useProjects.getState().addProject(proj);

    // 验证 binding 存在
    const page = useProjects.getState().projects["proj-2"].pages[0];
    expect(page.binding?.worktreePath).toBe("/tmp/wt");
    expect(page.binding?.branchName).toBe("feat");
  });
});
