import { describe, it, expect, beforeEach } from "vitest";
import {
  useProjects,
  createProjectId,
  createPageId,
  cancelPendingSave,
} from "../stores/projects";
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

  // ── 已有的 Project CRUD 测试 ──────────────────────────────

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

  // ── 已有的 renamePage 测试 ────────────────────────────────

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

  // ═══════════════════════════════════════════════════════════
  // 以下为本次追加的测试用例
  // ═══════════════════════════════════════════════════════════

  // ── updatePageLayout ─────────────────────────────────────

  it("updatePageLayout 应更新已存在页面的 layout 字段", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    const newLayout = { panels: { panel1: { size: 100 } } };
    useProjects.getState().updatePageLayout(project.projectId, page.pageId, newLayout);

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.layout).toEqual(newLayout);
  });

  it("updatePageLayout 不存在的 projectId → 状态不变", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page] });
    useProjects.getState().addProject(project);

    const stateBefore = useProjects.getState().projects;
    useProjects.getState().updatePageLayout("nonexistent-proj", page.pageId, { x: 1 });

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("updatePageLayout 不存在的 pageId → 页面不变但 version 仍递增（守卫仅检查 projectId）", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 5 });
    useProjects.getState().addProject(project);

    useProjects.getState().updatePageLayout(project.projectId, "nonexistent-page", { x: 1 });

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages[0].layout).toEqual(page.layout);
    expect(updated.version).toBe(6);
  });

  it("updatePageLayout 应递增 version", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 3 });
    useProjects.getState().addProject(project);

    useProjects.getState().updatePageLayout(project.projectId, page.pageId, { y: 2 });

    expect(useProjects.getState().projects[project.projectId].version).toBe(4);
  });

  // ── toggleExpand ─────────────────────────────────────────

  it("toggleExpand 应展开已折叠的节点", () => {
    useProjects.setState({ expandedNodes: { "node-1": false } });

    useProjects.getState().toggleExpand("node-1");

    expect(useProjects.getState().expandedNodes["node-1"]).toBe(true);
  });

  it("toggleExpand 应折叠已展开的节点", () => {
    useProjects.setState({ expandedNodes: { "node-1": true } });

    useProjects.getState().toggleExpand("node-1");

    expect(useProjects.getState().expandedNodes["node-1"]).toBe(false);
  });

  it("toggleExpand 不存在的 nodeId → 新键被设为 true（!undefined === true）", () => {
    expect(useProjects.getState().expandedNodes["new-node"]).toBeUndefined();

    useProjects.getState().toggleExpand("new-node");

    expect(useProjects.getState().expandedNodes["new-node"]).toBe(true);
  });

  // ── cancelPendingSave ────────────────────────────────────

  it("cancelPendingSave 应作为可调用函数导出", () => {
    expect(typeof cancelPendingSave).toBe("function");
    // 无参数调用不抛错（空定时器场景）
    expect(() => cancelPendingSave()).not.toThrow();
  });

  // ── removePage 边界 ──────────────────────────────────────

  it("removePage 移除最后一个页面 → activePageId 变为 null", () => {
    const page = makePage("only-page");
    const project = makeProject({ pages: [page], activePageId: page.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page.pageId);

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages).toHaveLength(0);
    expect(updated.activePageId).toBeNull();
  });

  it("removePage 移除非当前活跃页面 → activePageId 保持不变", () => {
    const page1 = makePage("page-1");
    const page2 = makePage("page-2");
    const page3 = makePage("page-3");
    const project = makeProject({ pages: [page1, page2, page3], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, page2.pageId);

    const updated = useProjects.getState().projects[project.projectId];
    expect(updated.pages).toHaveLength(2);
    expect(updated.activePageId).toBe(page1.pageId);
  });

  // ── switchToPage 增强 ────────────────────────────────────

  it("switchToPage 应更新 lastAccessedAt 为非零值", () => {
    const page = makePage("test-page");
    page.lastAccessedAt = 0; // 设已知初始值
    const project = makeProject({ pages: [page], activePageId: null });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page.pageId);

    const updated = useProjects.getState().projects[project.projectId].pages[0];
    expect(updated.lastAccessedAt).toBeGreaterThan(0);
  });

  it("switchToPage 应递增 version", () => {
    const page = makePage("test-page");
    const project = makeProject({ pages: [page], version: 5 });
    useProjects.getState().addProject(project);

    useProjects.getState().switchToPage(project.projectId, page.pageId);

    expect(useProjects.getState().projects[project.projectId].version).toBe(6);
  });

  // ── 不存在 projectId 的守卫 ──────────────────────────────

  it("addPage 不存在的 projectId → 状态不变", () => {
    const stateBefore = useProjects.getState().projects;
    useProjects.getState().addPage("nonexistent", makePage("p"));

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("removePage 不存在的 projectId → 状态不变", () => {
    const stateBefore = useProjects.getState().projects;
    useProjects.getState().removePage("nonexistent", "any-page");

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  it("switchToPage 不存在的 projectId → 状态不变", () => {
    const stateBefore = useProjects.getState().projects;
    useProjects.getState().switchToPage("nonexistent", "any-page");

    expect(useProjects.getState().projects).toEqual(stateBefore);
  });

  // ── ID 格式 ──────────────────────────────────────────────

  it("createProjectId 应以 'proj-' 开头", () => {
    const id = createProjectId();
    expect(id.startsWith("proj-")).toBe(true);
  });

  it("createPageId 应以 'page-' 开头", () => {
    const id = createPageId();
    expect(id.startsWith("page-")).toBe(true);
  });

  // ── addPage activePageId 保留 ─────────────────────────────

  it("addPage 已有 activePageId 时不改变活跃页面", () => {
    const page1 = makePage("page-1");
    const project = makeProject({ pages: [page1], activePageId: page1.pageId });
    useProjects.getState().addProject(project);

    const page2 = makePage("page-2");
    useProjects.getState().addPage(project.projectId, page2);

    const updated = useProjects.getState().projects[project.projectId];
    // activePageId 应保持为 page1，不因新增页面改变
    expect(updated.activePageId).toBe(page1.pageId);
  });

  // ── markPersistenceReady + subscribe ──────────────────────

  it("markPersistenceReady 应允许后续 save 操作", async () => {
    const { markPersistenceReady } = await import("../stores/projects");
    // 首次调用不抛错
    expect(() => markPersistenceReady()).not.toThrow();
  });

  // ── removeProject 清理 expandedNodes ──────────────────────

  it("removeProject 应同时清理 expandedNodes", () => {
    const project = makeProject();
    useProjects.getState().addProject(project);
    useProjects.setState({ expandedNodes: { [project.projectId]: true } });

    useProjects.getState().removeProject(project.projectId);

    expect(useProjects.getState().expandedNodes[project.projectId]).toBeUndefined();
  });

  // ── removePage 不存在 pageId ────────────────────────────

  it("removePage 不存在的 pageId → version 递增但页面列表不变", () => {
    const page = makePage("keep");
    const project = makeProject({ pages: [page], activePageId: page.pageId, version: 3 });
    useProjects.getState().addProject(project);

    useProjects.getState().removePage(project.projectId, "nonexistent-page");

    const updated = useProjects.getState().projects[project.projectId];
    // 页面列表不变
    expect(updated.pages).toHaveLength(1);
    expect(updated.pages[0].pageId).toBe(page.pageId);
    // version 仍递增（removePage 无条件递增）
    expect(updated.version).toBe(4);
  });
});
