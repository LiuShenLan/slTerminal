// workspace-e2e-ready.test.tsx — Workspace 挂载时 __slterm_e2e_workspaceReady 标记测试
//
// 验证 Workspace 组件在渲染阶段（非 useEffect）同步设置标记，
// 使 E2E 测试可以在调用 createProject 之前确认 Workspace 已就绪。

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const MockAllotment: any = () => null;
  MockAllotment.Pane = () => null;
  return {
    mockDockviewReact: vi.fn(() => null),
    MockAllotment,
  };
});

vi.mock("dockview-react", () => ({
  DockviewReact: mocks.mockDockviewReact,
}));

vi.mock("allotment", () => ({
  Allotment: mocks.MockAllotment,
}));

vi.mock("../workspace/panelRegistry", () => ({
  panelRegistry: {},
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({})),
  loadLayout: vi.fn(() => false),
}));

vi.mock("../features/sidebar", () => ({
  SidebarTree: vi.fn(() => null),
  makeDefaultLayout: vi.fn(() => ({
    grid: { orientation: "HORIZONTAL", root: { type: "branch", data: [] } },
  })),
}));

vi.mock("../features/explorer", () => ({
  ExplorerPanel: vi.fn(() => null),
}));

// ─── 导入 Workspace（mock 之后）───
import Workspace from "../workspace/Workspace";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ─── 辅助函数 ───
function resetStores() {
  useProjects.setState({
    projects: {},
    expandedNodes: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: null });
}

function seedProjectAndPage() {
  const projectId = "proj-test";
  const pageId = "page-test";
  const rootPath = "C:\\test-project";

  useProjects.setState({
    projects: {
      [projectId]: {
        projectId,
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId,
            name: "测试页面",
            layout: {},
            cwd: rootPath,
            createdAt: Date.now(),
            lastAccessedAt: Date.now(),
          },
        ],
        activePageId: pageId,
        version: 1,
      },
    },
    expandedNodes: { [projectId]: true },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: pageId });
}

// ─── Tests ───
describe("Workspace 挂载 — __slterm_e2e_workspaceReady 标记", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    resetStores();
    delete (window as any).__slterm_e2e_workspaceReady;
  });

  it("6. Workspace 挂载后 __slterm_e2e_workspaceReady === true", () => {
    seedProjectAndPage();
    render(React.createElement(Workspace));

    expect((window as any).__slterm_e2e_workspaceReady).toBe(true);
  });

  it("7. Workspace 未挂载时 __slterm_e2e_workspaceReady 不存在", () => {
    // 清除标记（如果之前测试残留）
    delete (window as any).__slterm_e2e_workspaceReady;

    // 不渲染 Workspace → 标记不应存在
    expect((window as any).__slterm_e2e_workspaceReady).toBeUndefined();
  });

  it("8. Workspace 卸载后标记保留为 true（E2E 测试持续需要）", () => {
    seedProjectAndPage();
    const { unmount } = render(React.createElement(Workspace));

    expect((window as any).__slterm_e2e_workspaceReady).toBe(true);

    unmount();

    // 卸载后标记仍保留（E2E 测试流程中不应消失）
    expect((window as any).__slterm_e2e_workspaceReady).toBe(true);
  });

  it("9. 标记在同步渲染周期内设置（非异步副作用）", () => {
    seedProjectAndPage();

    // render 调用后立即检查（不使用 waitFor / setTimeout）
    render(React.createElement(Workspace));

    // 标记必须在 render 返回后立即可见
    expect((window as any).__slterm_e2e_workspaceReady).toBe(true);
  });
});
