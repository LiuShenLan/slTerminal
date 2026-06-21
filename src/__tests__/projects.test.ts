import { describe, it, expect, beforeEach } from "vitest";
import { useProjects, createProjectId, createPageId } from "../stores/projects";
import type { Project, OperationPage } from "../stores/projects";

/** 构造测试用 OperationPage */
function makePage(
  name: string,
  cwd?: string,
): OperationPage {
  return {
    pageId: createPageId(),
    name,
    layout: {},
    cwd,
    createdAt: Date.now(),
    lastAccessedAt: Date.now(),
  };
}

/** 构造测试用 Project */
function makeProject(overrides?: Partial<Project>): Project {
  return {
    projectId: createProjectId(),
    name: "test-project",
    rootPath: "/tmp/test-project",
    pages: [makePage("main-page", "/tmp/test-project")],
    activePageId: null,
    version: 1,
    ...overrides,
  };
}

describe("projects store", () => {
  beforeEach(() => {
    // 重置 store 到初始状态
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
  });

  it("addProject 应在 store 中添加新项目", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    const { projects } = useProjects.getState();
    expect(Object.keys(projects)).toHaveLength(1);
    expect(projects[project.projectId].name).toBe("test-project");
    expect(projects[project.projectId].pages).toHaveLength(1);
  });

  it("removeProject 应从 store 中移除项目", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.getState().removeProject(project.projectId);

    const { projects } = useProjects.getState();
    expect(Object.keys(projects)).toHaveLength(0);
  });

  it("addPage 应在项目中追加操作页面", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    const newPage = makePage("extra-page");
    useProjects.getState().addPage(project.projectId, newPage);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].pages).toHaveLength(2);
  });

  it("removePage 应移除操作页面并更新 activePageId", () => {
    const page1 = makePage("page-1");
    const page2 = makePage("page-2");
    const project = makeProject({ pages: [page1, page2], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page1.pageId);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].pages).toHaveLength(1);
    expect(projects[project.projectId].activePageId).toBe(page2.pageId);
  });

  it("switchToPage 应更新 activePageId 和 lastAccessedAt", () => {
    const page1 = makePage("page-1");
    const project = makeProject({ pages: [page1], activePageId: null });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page1.pageId);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].activePageId).toBe(page1.pageId);
  });

  // S2 重命名
  it("renamePage 应更新操作页面名称", () => {
    const page = makePage("原名");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, page.pageId, "新名称");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("新名称");
  });

  it("renamePage 空字符串 → 允许空名称", () => {
    const page = makePage("原名");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, page.pageId, "");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("");
  });

  it("renamePage 不存在的 pageId → 状态不变", () => {
    const page = makePage("保持");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    useProjects.getState().renamePage(project.projectId, "nonexistent", "新");

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.name).toBe("保持");
  });
});
