// explorer-delete.test.tsx — 文件浏览器右键删除确认自动化测试
//
// 覆盖：
//   E1 组：FileTree 文件/文件夹删除 — ask 调用与 onDelete 回调
//   E2 组：ask 参数验证 — 消息/title/kind
//   E3 组：ExplorerPanel 集成 — deleteEntry → refresh 链路
//   E4 组：边界条件 — 右键菜单包含"删除"项

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockAsk = vi.fn();
  const mockDeleteEntry = vi.fn();
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockCreateDir = vi.fn();
  const mockRename = vi.fn();
  const mockWriteFile = vi.fn();

  return {
    mockAsk,
    mockDeleteEntry,
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockCreateDir,
    mockRename,
    mockWriteFile,
    resetAll() {
      mockAsk.mockClear();
      mockDeleteEntry.mockClear();
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
      mockStartWatch.mockClear();
      mockCreateDir.mockClear();
      mockRename.mockClear();
      mockWriteFile.mockClear();
    },
  };
});

// Mock @tauri-apps/plugin-dialog（FileTree 通过 ../../ipc/dialog 的 ask 间接引用）
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: mocks.mockAsk,
  save: vi.fn(),
  open: vi.fn(),
}));

// Mock ipc/fs
vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: mocks.mockCreateDir,
  deleteEntry: mocks.mockDeleteEntry,
  rename: mocks.mockRename,
  writeFile: mocks.mockWriteFile,
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { FileTree } from "../features/explorer/FileTree";
import { ExplorerPanel } from "../features/explorer";
import type { TreeNode } from "../features/explorer/useFileTree";

// ─── 辅助函数 ───

/** 构造 TreeNode 测试数据 */
function makeFileNode(name: string, path: string): TreeNode {
  return {
    entry: { name, path, isDir: false, size: 100, modified: Date.now() },
    expanded: false,
    children: [],
    loading: false,
  };
}

function makeDirNode(name: string, path: string, children: TreeNode[] = []): TreeNode {
  return {
    entry: { name, path, isDir: true, size: undefined, modified: undefined },
    expanded: false,
    children,
    loading: false,
  };
}

/** 渲染 FileTree（独立组件，绕过 ExplorerPanel） */
function renderFileTree(nodes: TreeNode[], overrides: Partial<{
  onDelete: (path: string) => void;
  onToggleExpand: (path: string) => void;
  onOpenFile: (path: string) => void;
}> = {}) {
  const defaultProps = {
    nodes,
    depth: 0,
    gitStatusMap: new Map<string, string>(),
    onToggleExpand: overrides.onToggleExpand ?? vi.fn(),
    onOpenFile: overrides.onOpenFile ?? vi.fn(),
    onOpenInTerminal: vi.fn(),
    onRename: vi.fn(),
    onDelete: overrides.onDelete ?? vi.fn(),
    onNewFile: vi.fn(),
    onNewFolder: vi.fn(),
  };
  return render(React.createElement(FileTree, defaultProps));
}

/** 渲染 ExplorerPanel（集成测试） */
function renderExplorerPanel() {
  return render(React.createElement(ExplorerPanel));
}

/** 在 store 中注入项目数据，使 ExplorerPanel 有 rootPath */
function seedProject(rootPath: string = "C:/test-project") {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          { pageId: "page-1", name: "页面 1", layout: {}, cwd: rootPath, createdAt: 1, lastAccessedAt: 1 },
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

// ─── 通用 setup ───
beforeEach(() => {
  vi.clearAllMocks();
  mocks.resetAll();
  mocks.mockAsk.mockResolvedValue(false); // 默认取消
  mocks.mockReadDir.mockResolvedValue([]);
  mocks.mockGitStatus.mockResolvedValue([]);
  mocks.mockStartWatch.mockResolvedValue(undefined);
  cleanup();
  useProjects.setState({ projects: {}, expandedNodes: {} });
  useLayout.setState({ activePageId: null });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: vi.fn(),
    removePanel: vi.fn(),
  };
});

// =====================================================================
// E1 组：FileTree 文件/文件夹删除 — ask 调用与 onDelete 回调
// =====================================================================

describe("FileTree 删除确认 — ask 弹窗分支", () => {
  it("1. 文件删除 + 用户确认 → onDelete 调用传入路径", async () => {
    mocks.mockAsk.mockResolvedValue(true);
    const onDelete = vi.fn();
    const fileNode = makeFileNode("test.ts", "C:/project/test.ts");

    const { getAllByText } = renderFileTree([fileNode], { onDelete });

    // 右键点击文件名触发菜单
    fireEvent.contextMenu(getAllByText("test.ts")[0]);

    // 菜单中应出现"删除"，点击它
    const deleteItems = getAllByText("删除");
    fireEvent.click(deleteItems[0]);

    // ask 是 async，等待 Promise 微任务清空
    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith("C:/project/test.ts");
    });
  });

  it("2. 文件删除 + 用户取消 → onDelete 不调用", async () => {
    mocks.mockAsk.mockResolvedValue(false);
    const onDelete = vi.fn();
    const fileNode = makeFileNode("app.rs", "C:/project/app.rs");

    const { getAllByText } = renderFileTree([fileNode], { onDelete });

    fireEvent.contextMenu(getAllByText("app.rs")[0]);
    const deleteItems = getAllByText("删除");
    fireEvent.click(deleteItems[0]);

    // 等待微任务清空后 onDelete 不应被调用
    await new Promise<void>((r) => setTimeout(r, 10));
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("3. 文件夹删除 + 用户确认 → onDelete 调用", async () => {
    mocks.mockAsk.mockResolvedValue(true);
    const onDelete = vi.fn();
    const dirNode = makeDirNode("src", "C:/project/src");

    const { getAllByText } = renderFileTree([dirNode], { onDelete });

    fireEvent.contextMenu(getAllByText("src")[0]);
    const deleteItems = getAllByText("删除");
    fireEvent.click(deleteItems[0]);

    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith("C:/project/src");
    });
  });

  it("4. 文件夹删除 + 用户取消 → onDelete 不调用", async () => {
    mocks.mockAsk.mockResolvedValue(false);
    const onDelete = vi.fn();
    const dirNode = makeDirNode("lib", "C:/project/lib");

    const { getAllByText } = renderFileTree([dirNode], { onDelete });

    fireEvent.contextMenu(getAllByText("lib")[0]);
    const deleteItems = getAllByText("删除");
    fireEvent.click(deleteItems[0]);

    await new Promise<void>((r) => setTimeout(r, 10));
    expect(onDelete).not.toHaveBeenCalled();
  });
});

// =====================================================================
// E2 组：ask 参数验证
// =====================================================================

describe("FileTree 删除确认 — ask 参数", () => {
  it("5. 文件删除 ask 参数：消息含文件名+不可撤销，title/k kind 正确", () => {
    const onDelete = vi.fn();
    const fileNode = makeFileNode("main.tsx", "C:/project/main.tsx");

    const { getAllByText } = renderFileTree([fileNode], { onDelete });

    fireEvent.contextMenu(getAllByText("main.tsx")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    expect(mocks.mockAsk).toHaveBeenCalledTimes(1);
    const [message, opts] = mocks.mockAsk.mock.calls[0];

    expect(message).toContain("main.tsx");
    expect(message).toContain("不可撤销");
    expect(opts).toEqual({ title: "确认删除", kind: "warning" });
  });

  it("6. 文件夹删除 ask 参数：消息含文件夹名+不可撤销，title/k kind 正确", () => {
    const onDelete = vi.fn();
    const dirNode = makeDirNode("components", "C:/project/components");

    const { getAllByText } = renderFileTree([dirNode], { onDelete });

    fireEvent.contextMenu(getAllByText("components")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    expect(mocks.mockAsk).toHaveBeenCalledTimes(1);
    const [message, opts] = mocks.mockAsk.mock.calls[0];

    expect(message).toContain("components");
    expect(message).toContain("不可撤销");
    expect(opts).toEqual({ title: "确认删除", kind: "warning" });
  });
});

// =====================================================================
// E3 组：ExplorerPanel 集成 — deleteEntry → refresh
// =====================================================================

describe("ExplorerPanel 删除集成", () => {
  it("7. 删除成功 → deleteEntry 调用并触发 refresh", async () => {
    mocks.mockAsk.mockResolvedValue(true);
    mocks.mockDeleteEntry.mockResolvedValue(undefined);
    // readDir 返回一个文件节点，供 FileTree 渲染
    mocks.mockReadDir.mockResolvedValue([
      { name: "config.json", path: "C:/test-project/config.json", isDir: false, size: 64, modified: 1 },
    ]);

    seedProject();

    const { getAllByText } = renderExplorerPanel();

    // 等待异步 loadDirectory 完成
    await waitFor(() => {
      expect(getAllByText("config.json").length).toBeGreaterThan(0);
    });

    fireEvent.contextMenu(getAllByText("config.json")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    await waitFor(() => {
      expect(mocks.mockDeleteEntry).toHaveBeenCalledWith("C:/test-project/config.json");
    });

    // refresh 会再次调用 readDir
    // 初始化时两个 useEffect 各调用一次（useFileTree.loadRoot + ExplorerPanel.refresh），删除后 refresh 再调一次 = 3
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(3);
    });
  });

  it("8. 删除失败 → console.error 捕获，不崩溃", async () => {
    mocks.mockAsk.mockResolvedValue(true);
    mocks.mockDeleteEntry.mockRejectedValue(new Error("权限不足"));

    mocks.mockReadDir.mockResolvedValue([
      { name: "readonly.txt", path: "C:/test-project/readonly.txt", isDir: false, size: 32, modified: 1 },
    ]);

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    seedProject();

    const { getAllByText } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("readonly.txt").length).toBeGreaterThan(0);
    });

    fireEvent.contextMenu(getAllByText("readonly.txt")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalledWith("删除失败:", expect.any(Error));
    });

    consoleSpy.mockRestore();
  });
});

// =====================================================================
// E4 组：边界条件 — 右键菜单"删除"项存在
// =====================================================================

describe("FileTree 右键菜单结构", () => {
  it("9. 文件右键菜单包含'删除'项", () => {
    const fileNode = makeFileNode("index.ts", "C:/project/index.ts");

    const { getAllByText } = renderFileTree([fileNode]);
    fireEvent.contextMenu(getAllByText("index.ts")[0]);

    // 菜单中"删除"文本出现
    expect(getAllByText("删除").length).toBeGreaterThanOrEqual(1);
  });

  it("10. 文件夹右键菜单包含'删除'项", () => {
    const dirNode = makeDirNode("public", "C:/project/public");

    const { getAllByText } = renderFileTree([dirNode]);
    fireEvent.contextMenu(getAllByText("public")[0]);

    expect(getAllByText("删除").length).toBeGreaterThanOrEqual(1);
  });
});
