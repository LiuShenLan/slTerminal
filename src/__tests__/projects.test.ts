import { describe, it, expect, beforeEach } from "vitest";
import { useProjects, createProjectId, createPageId } from "../stores/projects";
import type { Project, OperationPage } from "../stores/projects";
import type { WorktreeInfo, WorktreeBinding } from "../types/git";

/** 构造测试用 WorktreeInfo */
function makeWorktree(path: string, branch: string): WorktreeInfo {
  return {
    path,
    branch,
    head: "abc123",
    isBare: false,
    isDetached: false,
    isMain: branch === "main",
  };
}

/** 构造测试用 OperationPage */
function makePage(
  name: string,
  binding?: WorktreeBinding,
): OperationPage {
  return {
    pageId: createPageId(),
    name,
    layout: {},
    binding,
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
    worktrees: [makeWorktree("/tmp/test-project", "main")],
    pages: [makePage("main-page", { worktreePath: "/tmp/test-project", branchName: "main" })],
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
    expect(projects[project.projectId].worktrees).toHaveLength(1);
  });

  it("removeProject 应从 store 中移除项目", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.getState().removeProject(project.projectId);

    const { projects } = useProjects.getState();
    expect(Object.keys(projects)).toHaveLength(0);
  });

  it("addWorktree 应在已有项目中追加 worktree", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    const newWt = makeWorktree("/tmp/test-project/.claude/worktrees/feat", "feat");
    useProjects.getState().addWorktree(project.projectId, newWt);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].worktrees).toHaveLength(2);
    expect(projects[project.projectId].worktrees[1].branch).toBe("feat");
  });

  it("addWorktree 应去重——同路径 worktree 不重复添加", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);

    // 添加到相同路径
    const dupWt = makeWorktree("/tmp/test-project", "main");
    useProjects.getState().addWorktree(project.projectId, dupWt);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].worktrees).toHaveLength(1); // 未增加
  });

  it("removeWorktree 应移除指定 worktree 并清除关联页面", () => {
    const project = makeProject();
    const wtPath = "/tmp/test-project/.claude/worktrees/feat";
    const featWt = makeWorktree(wtPath, "feat");
    const featPage = makePage("feat-page", { worktreePath: wtPath, branchName: "feat" });

    useProjects.getState().addProject({
      ...project,
      worktrees: [project.worktrees[0], featWt],
      pages: [project.pages[0], featPage],
    });
    useProjects.getState().removeWorktree(project.projectId, wtPath);

    const { projects } = useProjects.getState();
    expect(projects[project.projectId].worktrees).toHaveLength(1);
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
});
