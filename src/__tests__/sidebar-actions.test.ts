// sidebar-actions.test.ts — 侧栏交互自动化测试
//
// 使用真实 Zustand stores（setState 预填充数据），mock IPC dialog。
// 覆盖：H1 树结构/展开折叠、H3/H4/S1/S3 右键菜单交互

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockSwitchToPage = vi.fn();
  const mockOnDeletePage = vi.fn();
  const mockOpenDialog = vi.fn();

  return {
    mockSwitchToPage,
    mockOnDeletePage,
    mockOpenDialog,
    resetAll() {
      mockSwitchToPage.mockClear();
      mockOnDeletePage.mockClear();
      mockOpenDialog.mockClear();
    },
  };
});

vi.mock("../ipc/dialog", () => ({
  open: mocks.mockOpenDialog,
}));

// 导入真实 stores + SidebarTree（在 IPC mock 之后）
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { SidebarTree } from "../features/sidebar";

// ─── 辅助函数 ───
function populateStore() {
  // 预填充项目数据到真实 store
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath: "C:\\test",
        pages: [
          { pageId: "page-1", name: "操作页面 1", layout: {}, cwd: "C:\\test", createdAt: 1, lastAccessedAt: 1 },
          { pageId: "page-2", name: "操作页面 2", layout: {}, cwd: "C:\\test", createdAt: 2, lastAccessedAt: 2 },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    expandedNodes: { "proj-1": true },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: "page-1", isLayoutSwitching: false });
}

function renderSidebar() {
  return render(
    React.createElement(SidebarTree, {
      switchToPage: mocks.mockSwitchToPage,
      onDeletePage: mocks.mockOnDeletePage,
    }),
  );
}

// ─── Tests ───
describe("侧栏交互", () => {
  beforeEach(() => {
    mocks.resetAll();
    // 重置 store 到空初始状态
    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: null, isLayoutSwitching: false });
  });

  describe("H1 树结构与交互", () => {
    it("1. 渲染项目节点和操作页面节点（展开态二级树）", () => {
      populateStore();
      const { getByText } = renderSidebar();
      expect(getByText("测试项目")).toBeTruthy();
      expect(getByText("操作页面 1")).toBeTruthy();
      expect(getByText("操作页面 2")).toBeTruthy();
    });

    it("2. 点击操作页面 → switchToPage 被调用", () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      // StrictMode 双重渲染导致多个匹配元素，取第一个
      fireEvent.click(getAllByText("操作页面 2")[0]);
      expect(mocks.mockSwitchToPage).toHaveBeenCalledWith("proj-1", "page-2");
    });

    it("3. 空白态显示'+添加项目'按钮", () => {
      // 不填充 store，保持空状态
      const { getAllByTitle } = renderSidebar();
      expect(getAllByTitle("添加项目")[0]).toBeTruthy();
    });
  });

  describe("H3/H4/S3 右键菜单", () => {
    it("4. 右键项目节点 → 菜单含'新建操作页面'和'删除项目'", () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("测试项目")[0]);

      // 右键菜单出现在固定定位的 div 中，用 getAllByText
      expect(getAllByText("新建操作页面").length).toBeGreaterThan(0);
      expect(getAllByText("删除项目").length).toBeGreaterThan(0);
    });

    it("5. 右键操作页面节点 → 菜单含'重命名'和'删除操作页面'", () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      expect(getAllByText("重命名操作页面").length).toBeGreaterThan(0);
      expect(getAllByText("删除操作页面").length).toBeGreaterThan(0);
    });

    it("6. 点击'删除操作页面'→ onDeletePage 被调用", () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);
      fireEvent.click(getAllByText("删除操作页面")[0]);

      expect(mocks.mockOnDeletePage).toHaveBeenCalledWith("proj-1", "page-1");
    });
  });
});
