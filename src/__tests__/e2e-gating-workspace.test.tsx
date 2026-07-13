// e2e-gating-workspace.test.tsx — Workspace 的 E2E_ENABLED 门控双路径（T2）
//
// 补齐"禁用路径"：E2E_ENABLED=false 时 Workspace 不置 __slterm_e2e_workspaceReady。
// 启用路径（=true→置 true）另由 workspace-e2e-ready.test.tsx 覆盖；此处用 getter 切换
// 在同一文件内同时验证两向，直接断言可观察行为（用真实 markWorkspaceReady，非 spy）。

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup } from "@testing-library/react";

// E2E 门控开关：getter 读取可变量，逐用例切换（consumer 每次渲染读 live binding → 触发 getter）
const gate = vi.hoisted(() => ({ enabled: true }));
vi.mock("../lib/e2eEnabled", () => ({
  get E2E_ENABLED() {
    return gate.enabled;
  },
  computeE2eEnabled: (dev: boolean, viteE2e: string | undefined) =>
    dev || viteE2e === "1",
}));

// ─── Workspace 依赖 mock（对齐 workspace-e2e-ready.test.tsx）───
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
vi.mock("../panelRegistry", () => ({
  panelRegistry: {},
}));
vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({})),
  loadLayout: vi.fn(() => false),
}));
vi.mock("../features/sidebar", () => ({
  SidebarTree: vi.fn(() => null),
  makeEmptyLayout: vi.fn(() => ({})),
}));
vi.mock("../features/explorer", () => ({
  ExplorerPanel: vi.fn(() => null),
}));

import Workspace from "../workspace/Workspace";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

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

describe("Workspace — E2E_ENABLED 门控 markWorkspaceReady", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    resetStores();
    delete (window as any).__slterm_e2e_workspaceReady;
    gate.enabled = true;
  });

  it("启用（true）→ 挂载后 __slterm_e2e_workspaceReady === true", () => {
    gate.enabled = true;
    seedProjectAndPage();
    render(React.createElement(Workspace));
    expect((window as any).__slterm_e2e_workspaceReady).toBe(true);
  });

  it("禁用（false）→ 挂载后 __slterm_e2e_workspaceReady 保持 undefined（生产语义）", () => {
    gate.enabled = false;
    seedProjectAndPage();
    render(React.createElement(Workspace));
    expect((window as any).__slterm_e2e_workspaceReady).toBeUndefined();
  });
});
