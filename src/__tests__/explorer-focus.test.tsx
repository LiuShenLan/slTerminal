// explorer-focus.test.tsx — ExplorerPanel 焦点管理测试
//
// 测试 usePanelFocus("explorer") 注册、tabIndex、单击聚焦、active pointer 同步
// （EXP-04：补充 focusin/focusout 上下文栈链路断言）

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
import React from "react";
import { ExplorerPanel } from "../features/explorer/ExplorerPanel";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import * as fsMock from "../ipc/fs";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";

// ─── Hoisted mocks ───
const activeExplorerMocks = vi.hoisted(() => ({
  setActiveExplorer: vi.fn(),
  clearActiveExplorer: vi.fn(),
  getActiveExplorer: vi.fn(),
}));

// Mock 依赖
vi.mock("../ipc/fs", () => ({
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
  readDir: vi.fn().mockResolvedValue([]),
}));

vi.mock("../ipc/notify", () => ({
  startWatch: vi.fn().mockResolvedValue(undefined),
  stopWatch: vi.fn().mockResolvedValue(undefined),
  // 必须返回 unlisten 函数（useFileTree 卸载时调用）
  onFsEvent: () => () => {},
}));

vi.mock("../ipc/git", () => ({
  gitStatus: vi.fn().mockResolvedValue([]),
}));

vi.mock("../ipc/dialog", () => ({
  ask: vi.fn().mockResolvedValue(false),
}));

// Mock activeExplorer 模块（spy set/clear，验证焦点链路）
vi.mock("../features/explorer/activeExplorer", () => ({
  setActiveExplorer: activeExplorerMocks.setActiveExplorer,
  clearActiveExplorer: activeExplorerMocks.clearActiveExplorer,
  getActiveExplorer: activeExplorerMocks.getActiveExplorer,
}));

// 提供 dockview API
beforeEach(() => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: vi.fn(),
    getPanel: vi.fn(),
  };
  activeExplorerMocks.setActiveExplorer.mockClear();
  activeExplorerMocks.clearActiveExplorer.mockClear();
  useProjects.setState({ projects: {}, expandedNodes: {} });
  useLayout.setState({ activePageId: null });
  // 清空 ShortcutRegistry 上下文栈，防跨测试污染
  getShortcutRegistry()._reset();
});

afterEach(() => {
  cleanup();
  getShortcutRegistry()._reset();
});

describe("ExplorerPanel 焦点管理", () => {
  it("容器挂载后有 data-e2e='explorer-tree-container'", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    // 因为需要项目数据才有文件树容器，无项目时显示提示文字
    const hint = container.querySelector('[data-e2e="explorer-tree-container"]');
    // 无项目时 rootPath=null，不渲染 FileTree 但容器仍存在
    expect(hint).not.toBeNull();
  });

  it("容器有 tabIndex={-1}", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    const treeContainer = container.querySelector('[data-e2e="explorer-tree-container"]');
    expect(treeContainer).not.toBeNull();
    expect((treeContainer as HTMLElement).getAttribute("tabindex")).toBe("-1");
  });

  it("容器不设 outline 抑制（全局 :focus-visible 环接管，UI-808）", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    const treeContainer = container.querySelector('[data-e2e="explorer-tree-container"]') as HTMLElement;
    // style 上无显式 outline 抑制——outline 由全局 :focus-visible 规则接管
    // （鼠标点击不匹配 :focus-visible 视觉无变化；键盘编程聚焦时可见）
    expect(treeContainer.style.outline).toBe("");
  });
});

describe("ExplorerPanel 焦点上下文链路（EXP-04）", () => {
  it("容器 focusin → pushContext('explorer') + setActiveExplorer(explorerActions)", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    const treeContainer = container.querySelector('[data-e2e="explorer-tree-container"]') as HTMLElement;

    fireEvent.focusIn(treeContainer);

    // ShortcutRegistry 上下文栈压入 "explorer"
    expect(getShortcutRegistry()._contextStack()).toContain("explorer");
    // activeExplorer 指针设置
    expect(activeExplorerMocks.setActiveExplorer).toHaveBeenCalledTimes(1);
    // 参数为 explorerActions（含五个方法）
    const actions = activeExplorerMocks.setActiveExplorer.mock.calls[0][0];
    expect(typeof actions.getSelectedPath).toBe("function");
    expect(typeof actions.deleteSelected).toBe("function");
    expect(typeof actions.openSelected).toBe("function");
    expect(typeof actions.renameSelected).toBe("function");
    expect(typeof actions.isRenaming).toBe("function");
  });

  it("容器 focusout（离开子树）→ popContext('explorer') + clearActiveExplorer", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    const treeContainer = container.querySelector('[data-e2e="explorer-tree-container"]') as HTMLElement;

    fireEvent.focusIn(treeContainer);
    expect(activeExplorerMocks.setActiveExplorer).toHaveBeenCalledTimes(1);
    expect(getShortcutRegistry()._contextStack()).toContain("explorer");

    fireEvent.focusOut(treeContainer);

    // 上下文栈弹出 + activeExplorer 指针清除
    expect(getShortcutRegistry()._contextStack()).not.toContain("explorer");
    expect(activeExplorerMocks.clearActiveExplorer).toHaveBeenCalledTimes(1);
  });

  it("单击文件行 → 容器 focus() 触发 focusin → setActiveExplorer（单击即建焦点上下文）", async () => {
    // 需要项目数据渲染文件行：先 seed 项目再等待树加载
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "测试项目",
          rootPath: "C:/test-project",
          pages: [
            { pageId: "page-1", name: "页面 1", layout: {}, cwd: "C:/test-project", createdAt: 1, lastAccessedAt: 1 },
          ],
          activePageId: "page-1",
          version: 1,
        },
      },
      expandedNodes: { "proj-1": true },
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: "page-1" });

    // readDir mock 返回文件行（覆盖上方 vi.mock 默认空数组）
    (fsMock.readDir as ReturnType<typeof vi.fn>).mockResolvedValue([
      { name: "a.ts", path: "C:/test-project/a.ts", isDir: false, size: 10, modified: 1 },
    ]);

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("a.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 单击行 → handleSelect → containerRef.current.focus() → focusin
    fireEvent.click(getAllByText("a.ts")[0]);

    await waitFor(() => {
      expect(activeExplorerMocks.setActiveExplorer).toHaveBeenCalled();
    }, { timeout: 3000 });
  });
});
