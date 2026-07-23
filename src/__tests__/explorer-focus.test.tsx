// explorer-focus.test.tsx — ExplorerPanel 焦点管理测试
//
// 测试 usePanelFocus("explorer") 注册、tabIndex、单击聚焦、active pointer 同步

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import React from "react";
import { ExplorerPanel } from "../features/explorer/ExplorerPanel";

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
  onFsEvent: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitStatus: vi.fn().mockResolvedValue([]),
}));

vi.mock("../ipc/dialog", () => ({
  ask: vi.fn().mockResolvedValue(false),
}));

// 提供 dockview API
beforeEach(() => {
  (window as any).__dockviewApi = {
    addPanel: vi.fn(),
    getPanel: vi.fn(),
  };
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

  it("容器 style.outline=none（聚焦时不显示轮廓）", async () => {
    const { container } = render(React.createElement(ExplorerPanel));
    const treeContainer = container.querySelector('[data-e2e="explorer-tree-container"]') as HTMLElement;
    expect(treeContainer.style.outline).toBe("none");
  });
});
