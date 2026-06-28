// sidebar-actions.test.ts — 侧栏交互自动化测试
//
// 使用真实 Zustand stores（setState 预填充数据），mock IPC dialog。
// 覆盖：H1 树结构/展开折叠、H3/H4/S1/S3 右键菜单交互

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

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
  useLayout.setState({ activePageId: "page-1" });
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
    cleanup(); // 清理前一个测试的 DOM 残留（测试间隔离）
    mocks.resetAll();
    // 重置 store 到空初始状态
    useProjects.setState({
      projects: {},
      expandedNodes: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: null });
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

    it("4. 点击展开/折叠按钮 → 子节点可见性切换", () => {
      populateStore();
      const { getAllByText, queryByText } = renderSidebar();

      // 初始展开，子页面可见
      expect(getAllByText("操作页面 1")[0]).toBeTruthy();
      expect(getAllByText("操作页面 2")[0]).toBeTruthy();

      // 点击项目节点切换折叠
      fireEvent.click(getAllByText("测试项目")[0]);

      // 折叠后子页面不可见
      expect(queryByText("操作页面 1")).toBeNull();
      expect(queryByText("操作页面 2")).toBeNull();
      expect(useProjects.getState().expandedNodes["proj-1"]).toBe(false);
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

  describe("S3 内联重命名", () => {
    it("7. 右键菜单包含'重命名操作页面'选项", () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      // 验证菜单项存在
      const items = getAllByText("重命名操作页面");
      expect(items.length).toBeGreaterThan(0);
    });

    it("8. 空白态按'+'不触发系统错误", () => {
      const { getAllByTitle } = renderSidebar();
      // 空 store 下点击添加项目按钮不抛错
      const buttons = getAllByTitle("添加项目");
      expect(() => fireEvent.click(buttons[0])).not.toThrow();
    });

    it("9. 右键操作页面 → 点击'重命名操作页面' → 出现 input 且值为原名称", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      // 找到右键菜单项并用原生 click 触发（绕开 fireEvent 在 jsdom 中的 React 事件处理差异）
      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      // setRenamingPageId 触发 PageRow 重渲染 → 等 input 出现
      await waitFor(() => {
        const input = document.querySelector("input");
        expect(input).toBeTruthy();
        expect((input as HTMLInputElement).value).toBe("操作页面 1");
      });
    });

    it("10. 内联重命名 → 输入新名称 → Enter 确认 → 页面名更新", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      await waitFor(() => {
        expect(document.querySelector("input")).toBeTruthy();
      });

      const input = document.querySelector("input")!;
      fireEvent.change(input, { target: { value: "新名称" } });
      fireEvent.keyDown(input, { key: "Enter" });

      const page = useProjects
        .getState()
        .projects["proj-1"]
        ?.pages.find((p) => p.pageId === "page-1");
      expect(page?.name).toBe("新名称");
    });

    it("11. 内联重命名 → Escape 取消 → 原名不变", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      await waitFor(() => {
        expect(document.querySelector("input")).toBeTruthy();
      });

      const input = document.querySelector("input")!;
      fireEvent.change(input, { target: { value: "新名称" } });
      fireEvent.keyDown(input, { key: "Escape" });

      const page = useProjects
        .getState()
        .projects["proj-1"]
        ?.pages.find((p) => p.pageId === "page-1");
      expect(page?.name).toBe("操作页面 1");
    });

    it("12. 内联重命名 → 输入空白 → Enter → 走取消分支（不调用 renamePage）", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      await waitFor(() => {
        expect(document.querySelector("input")).toBeTruthy();
      });

      const input = document.querySelector("input")!;
      fireEvent.change(input, { target: { value: "   " } });
      fireEvent.keyDown(input, { key: "Enter" });

      const page = useProjects
        .getState()
        .projects["proj-1"]
        ?.pages.find((p) => p.pageId === "page-1");
      expect(page?.name).toBe("操作页面 1");
    });

    it("13. 内联重命名 → 输入与原名称相同 → Enter → 不调用 renamePage", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);

      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      await waitFor(() => {
        expect(document.querySelector("input")).toBeTruthy();
      });

      const input = document.querySelector("input")!;
      fireEvent.change(input, { target: { value: "操作页面 1" } });
      fireEvent.keyDown(input, { key: "Enter" });

      const page = useProjects
        .getState()
        .projects["proj-1"]
        ?.pages.find((p) => p.pageId === "page-1");
      expect(page?.name).toBe("操作页面 1");
    });
  });

  describe("项目删除确认", () => {
    it("14. 右键'删除项目'+ confirm 拒绝 → 不删除", () => {
      const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
      populateStore();

      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("测试项目")[0]);
      fireEvent.click(getAllByText("删除项目")[0]);

      // store 中项目仍存在
      expect(useProjects.getState().projects["proj-1"]).toBeDefined();
      confirmSpy.mockRestore();
    });

    it("15. 右键'删除项目'+ confirm 接受 → 项目被移除", () => {
      const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
      populateStore();

      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("测试项目")[0]);
      fireEvent.click(getAllByText("删除项目")[0]);

      expect(useProjects.getState().projects["proj-1"]).toBeUndefined();
      confirmSpy.mockRestore();
    });

    it("16. 项目删除 confirm 消息含项目名和'确定删除项目'", () => {
      const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
      populateStore();

      const { getAllByText } = renderSidebar();
      fireEvent.contextMenu(getAllByText("测试项目")[0]);
      fireEvent.click(getAllByText("删除项目")[0]);

      expect(confirmSpy).toHaveBeenCalledTimes(1);
      const message = confirmSpy.mock.calls[0][0];
      expect(message).toContain("测试项目");
      expect(message).toContain("确定删除项目");

      confirmSpy.mockRestore();
    });
  });

  describe("布局样式 — 宽度自适应", () => {
    it("17. 根 div width = '100%'（非固定像素）", () => {
      populateStore();
      const { container } = renderSidebar();
      // SidebarTree 根 div = container.firstChild 下第一个 div
      const rootDiv = container.querySelector('div[style*="width"]');
      expect(rootDiv).toBeTruthy();
      // width 应为 "100%" 而非固定像素
      expect((rootDiv as HTMLElement).style.width).toBe("100%");
    });

    it("18. 根 div minWidth = '0px'（非 250px）", () => {
      populateStore();
      const { container } = renderSidebar();
      const rootDiv = container.querySelector('div[style*="width"]');
      expect(rootDiv).toBeTruthy();
      // minWidth 应为 0（允许缩窄到 Allotment minSize=160）
      expect((rootDiv as HTMLElement).style.minWidth).not.toBe("250px");
      // jsdom 把 minWidth:0 序列化为 "0px"
      const mw = (rootDiv as HTMLElement).style.minWidth;
      expect(mw === "0px" || mw === "0").toBe(true);
    });

    it("19. 根 div overflow = 'hidden' — 窄宽度下内容不溢出", () => {
      populateStore();
      const { container } = renderSidebar();
      const rootDiv = container.querySelector('div[style*="width"]');
      expect(rootDiv).toBeTruthy();
      expect((rootDiv as HTMLElement).style.overflow).toBe("hidden");
    });

    it("20. 根 div height = '100%' — 高度填满 Pane", () => {
      populateStore();
      const { container } = renderSidebar();
      const rootDiv = container.querySelector('div[style*="width"]');
      expect(rootDiv).toBeTruthy();
      expect((rootDiv as HTMLElement).style.height).toBe("100%");
    });

    it("21. 项目名称 span 含文本截断 CSS", () => {
      populateStore();
      const { getByText } = renderSidebar();
      // 项目名称 "测试项目" 在 span 中，其容器为 ProjectRow
      // 找到名称 span：overflow:hidden + textOverflow:ellipsis + whiteSpace:nowrap
      const nameSpan = getByText("测试项目");
      expect(nameSpan).toBeTruthy();
      const style = (nameSpan as HTMLElement).style;
      expect(style.overflow).toBe("hidden");
      expect(style.textOverflow).toBe("ellipsis");
      expect(style.whiteSpace).toBe("nowrap");
    });

    it("22. 操作页面名称 span 含文本截断 CSS", () => {
      populateStore();
      const { getByText } = renderSidebar();
      const nameSpan = getByText("操作页面 1");
      expect(nameSpan).toBeTruthy();
      const style = (nameSpan as HTMLElement).style;
      expect(style.overflow).toBe("hidden");
      expect(style.textOverflow).toBe("ellipsis");
      expect(style.whiteSpace).toBe("nowrap");
    });

    it("23. 重命名 input minWidth = 0 — 窄宽度下可收缩", async () => {
      populateStore();
      const { getAllByText } = renderSidebar();
      // 触发重命名 → 等 input 出现
      fireEvent.contextMenu(getAllByText("操作页面 1")[0]);
      const renameItems = getAllByText("重命名操作页面");
      expect(renameItems.length).toBeGreaterThan(0);
      (renameItems[0] as HTMLElement).click();

      await waitFor(() => {
        expect(document.querySelector("input")).toBeTruthy();
      });

      const input = document.querySelector("input")!;
      expect(input.style.minWidth).not.toBe("250px");
      const mw = input.style.minWidth;
      expect(mw === "0px" || mw === "0").toBe(true);
    });

    it("24. SIDEBAR_WIDTH 常量已移除 — 根 div width 非 250px", () => {
      populateStore();
      const { container } = renderSidebar();
      const rootDiv = container.querySelector('div[style*="width"]');
      expect(rootDiv).toBeTruthy();
      // SIDEBAR_WIDTH=250 已被删除，width 不应再是 "250px"
      expect((rootDiv as HTMLElement).style.width).not.toBe("250px");
      // minWidth 也不应再是 "250px"
      expect((rootDiv as HTMLElement).style.minWidth).not.toBe("250px");
    });
  });

  describe("添加项目完整流程", () => {
    it("16. 点击 + 添加项目 → dialog.open 选择文件夹 → store 创建项目", async () => {
      mocks.mockOpenDialog.mockResolvedValueOnce("C:\\new-project");

      const { getAllByTitle } = renderSidebar();
      fireEvent.click(getAllByTitle("添加项目")[0]);

      // 等待异步操作完成
      await waitFor(() => {
        const projects = Object.values(useProjects.getState().projects);
        expect(projects.length).toBeGreaterThan(0);
      });

      const state = useProjects.getState();
      const project = Object.values(state.projects)[0];
      expect(project.rootPath).toBe("C:\\new-project");
      expect(project.pages.length).toBe(1);
      expect(project.pages[0].cwd).toBe("C:\\new-project");
      expect(mocks.mockOpenDialog).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: "选择项目文件夹",
      });
    });
  });
});
