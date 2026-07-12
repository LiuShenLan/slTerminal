// helpers/workspace-setup.ts — store 重置 + 种子数据共享工厂
//
// 消除 ~7 个 explorer/workspace 测试文件中重复的 populateStore/resetStore/seedProject/setupTwoPages 函数。
//
// 用法示例：
//
//   import { seedExplorerProject, resetStores } from "./helpers/workspace-setup";
//
//   beforeEach(() => {
//     resetStores();
//     seedExplorerProject("C:/my-project");
//   });

import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import type { Project, OperationPage } from "../../stores/projects";

// ─── 基本重置 ───

/** 重置 projects + layout store 到初始状态 */
export function resetProjectStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
}

/** 仅重置 projects store 到空状态（保留 expandedNodes） */
export function resetProjectsOnly() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
}

// ─── 页面工厂 ───

/** 创建最简单页面种子（explorer 测试通用模式） */
export function seedExplorerProject(rootPath = "C:\\project", pageCwd?: string) {
  const page: OperationPage = {
    pageId: "page-1",
    name: "操作页面 1",
    layout: {},
    cwd: pageCwd ?? `${rootPath}\\src`,
    createdAt: 1,
    lastAccessedAt: 1,
  };

  const project: Project = {
    projectId: "proj-1",
    name: "测试项目",
    rootPath,
    pages: [page],
    activePageId: "page-1",
    version: 1,
  };

  useProjects.setState({
    projects: { "proj-1": project },
  });
  useLayout.setState({ activePageId: "page-1" });
}

/** 创建多页面种子（workspace 多实例测试用） */
export function seedMultiPageProject(projId = "proj-multi", pageIds = ["page-alpha", "page-beta"]) {
  const pages: OperationPage[] = pageIds.map((pid) => ({
    pageId: pid,
    name: pid,
    layout: {},
    cwd: "/tmp/multi",
    createdAt: Date.now(),
    lastAccessedAt: Date.now(),
  }));

  useProjects.getState().addProject({
    projectId: projId,
    name: "multi-test",
    rootPath: "/tmp/multi",
    pages,
    activePageId: pages[0].pageId,
    version: 1,
  });

  const expanded = useProjects.getState().expandedNodes;
  useProjects.setState({ expandedNodes: { ...expanded, [projId]: true } });

  useLayout.setState({ activePageId: pages[0].pageId });

  return { projId, pages };
}
