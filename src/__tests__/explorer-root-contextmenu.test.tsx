// explorer-root-contextmenu.test.tsx — 文件浏览器空白区域右键菜单自动化测试
//
// 覆盖：
//   R1–R5 组：空白区域右键菜单 UI 行为
//   R6–R12 组：根级创建文件/文件夹交互
//   R13   组：文件夹右键菜单不受影响（回归）
//   R14   组：ExplorerPanel 集成——根级创建失败

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockCreateDir = vi.fn();
  const mockWriteFile = vi.fn();
  const mockAsk = vi.fn();

  return {
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockCreateDir,
    mockWriteFile,
    mockAsk,
    resetAll() {
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
      mockStartWatch.mockClear();
      mockCreateDir.mockClear();
      mockWriteFile.mockClear();
      mockAsk.mockClear();
    },
  };
});

// Mock @tauri-apps/plugin-dialog
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: mocks.mockAsk,
  save: vi.fn(),
  open: vi.fn(),
}));

// Mock ipc/fs
vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: mocks.mockCreateDir,
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: mocks.mockWriteFile,
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  onFsEvent: () => () => {},
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { FileTree } from "../features/explorer/FileTree";
import { ExplorerPanel } from "../features/explorer";
import type { TreeNode } from "../features/explorer/useFileTree";

// ─── 辅助函数 ───

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

interface FileTreeOverrides {
  nodes?: TreeNode[];
  depth?: number;
  rootPath?: string;
  onNewFile?: (path: string) => void;
  onNewFolder?: (path: string) => void;
  onToggleExpand?: (path: string) => void;
  onOpenFile?: (path: string) => void;
  onDelete?: (path: string) => void;
  onRename?: (path: string, newName: string) => void;
  onOpenInTerminal?: (path: string) => void;
}

/** 渲染 FileTree（独立组件） */
function renderFileTree(overrides: FileTreeOverrides = {}) {
  const nodes = overrides.nodes ?? [];
  const defaultProps = {
    nodes,
    depth: overrides.depth ?? 0,
    gitStatusMap: new Map<string, string>(),
    onToggleExpand: overrides.onToggleExpand ?? vi.fn(),
    onOpenFile: overrides.onOpenFile ?? vi.fn(),
    onOpenInTerminal: overrides.onOpenInTerminal ?? vi.fn(),
    onRename: overrides.onRename ?? vi.fn(),
    onDelete: overrides.onDelete ?? vi.fn(),
    onNewFile: overrides.onNewFile ?? vi.fn(),
    onNewFolder: overrides.onNewFolder ?? vi.fn(),
    rootPath: overrides.rootPath,
  };
  return render(React.createElement(FileTree, defaultProps));
}

function renderExplorerPanel() {
  return render(React.createElement(ExplorerPanel));
}

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
  mocks.mockAsk.mockResolvedValue(false);
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
// R1–R5 组：空白区域右键菜单 UI 行为
// =====================================================================

describe("FileTree 空白区域右键菜单 — UI 行为", () => {
  it("R1: 空白区域右键弹出菜单，含「新建文件」「新建文件夹」", () => {
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [makeFileNode("a.ts", "C:/proj/a.ts")],
    });

    // 右键 wrapper div（空白区域）
    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);

    expect(getAllByText("新建文件").length).toBeGreaterThanOrEqual(1);
    expect(getAllByText("新建文件夹").length).toBeGreaterThanOrEqual(1);
  });

  it("R2: 根级菜单仅含「新建文件」「新建文件夹」", () => {
    const { container, queryByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [makeFileNode("a.ts", "C:/proj/a.ts")],
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);

    // 根级菜单不应包含文件/文件夹级选项
    expect(queryByText("删除")).toBeNull();
    expect(queryByText("重命名")).toBeNull();
    expect(queryByText("打开")).toBeNull();
  });

  it("R3: 树节点右键不触发根菜单，仍弹出文件菜单", () => {
    const { getAllByText, queryByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [makeFileNode("index.ts", "C:/proj/index.ts")],
    });

    // 右键文件节点（非空白区域）
    fireEvent.contextMenu(getAllByText("index.ts")[0]);

    // 应弹出文件菜单（含「打开」），非根菜单
    expect(getAllByText("打开").length).toBeGreaterThanOrEqual(1);
    expect(getAllByText("删除").length).toBeGreaterThanOrEqual(1);
    // 根菜单不应出现（虽然文件菜单没有"新建文件"，但确保 contextMenu 是文件菜单）
    expect(queryByText("新建文件")).toBeNull();
  });

  it("R4: 无 rootPath 时空白区域右键无反应", () => {
    const { container } = renderFileTree({
      // 不传 rootPath
      nodes: [makeFileNode("a.ts", "C:/proj/a.ts")],
    });

    const wrapper = container.firstElementChild;
    // wrapper 可能不是 div（无 rootPath 时 depth===0 返回 treeContent fragment）
    if (wrapper) {
      fireEvent.contextMenu(wrapper);
    }

    // 菜单不应可见（ContextMenu 不可见时返回 null，DOM 中无菜单元素）
    expect(document.querySelectorAll('[style*="position: fixed"]').length).toBe(0);
  });

  it("R5: 空目录右键弹出菜单", () => {
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);

    expect(getAllByText("新建文件").length).toBeGreaterThanOrEqual(1);
    expect(getAllByText("新建文件夹").length).toBeGreaterThanOrEqual(1);
    // 空目录提示应存在
    expect(getAllByText("空目录").length).toBeGreaterThanOrEqual(1);
  });
});

// =====================================================================
// R6–R12 组：根级创建文件/文件夹交互
// =====================================================================

describe("FileTree 根级创建文件/文件夹", () => {
  it("R6: 点击「新建文件」→ 树顶部出现输入框", () => {
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [makeFileNode("a.ts", "C:/proj/a.ts")],
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件")[0]);

    // 应出现 placeholder="文件名" 的 inline 输入框
    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    expect(newFileInput.placeholder).toBe("文件名");
    expect(newFileInput).toBeTruthy();
  });

  it("R7: 点击「新建文件夹」→ 树顶部出现输入框", () => {
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [makeFileNode("a.ts", "C:/proj/a.ts")],
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件夹")[0]);

    const inputs = document.querySelectorAll('input');
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;
    expect(newFolderInput.placeholder).toBe("文件夹名");
    expect(newFolderInput).toBeTruthy();
  });

  it("R8: 输入文件名回车 → 调 onNewFile", () => {
    const onNewFile = vi.fn();
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
      onNewFile,
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件")[0]);

    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "newfile.ts" } });
    fireEvent.keyDown(newFileInput, { key: "Enter" });

    expect(onNewFile).toHaveBeenCalledWith("C:/proj/newfile.ts");
    expect(onNewFile).toHaveBeenCalledTimes(1);
  });

  it("R9: 输入文件夹名回车 → 调 onNewFolder", () => {
    const onNewFolder = vi.fn();
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
      onNewFolder,
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件夹")[0]);

    const inputs = document.querySelectorAll('input');
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFolderInput, { target: { value: "src" } });
    fireEvent.keyDown(newFolderInput, { key: "Enter" });

    expect(onNewFolder).toHaveBeenCalledWith("C:/proj/src");
    expect(onNewFolder).toHaveBeenCalledTimes(1);
  });

  it("R10: Escape 取消新建文件", () => {
    const onNewFile = vi.fn();
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
      onNewFile,
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件")[0]);

    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "a.ts" } });
    fireEvent.keyDown(newFileInput, { key: "Escape" });

    expect(onNewFile).not.toHaveBeenCalled();
    // 输入框应消失
    expect(document.querySelectorAll('input').length).toBe(0);
  });

  it("R11: blur 空值不创建", () => {
    const onNewFile = vi.fn();
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
      onNewFile,
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件")[0]);

    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    // 不输入，直接 blur
    fireEvent.blur(newFileInput);

    expect(onNewFile).not.toHaveBeenCalled();
  });

  it("R12: blur 非空值创建", () => {
    const onNewFile = vi.fn();
    const { container, getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [],
      onNewFile,
    });

    const wrapper = container.firstElementChild!;
    fireEvent.contextMenu(wrapper);
    fireEvent.click(getAllByText("新建文件")[0]);

    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "blur-create.ts" } });
    fireEvent.blur(newFileInput);

    expect(onNewFile).toHaveBeenCalledWith("C:/proj/blur-create.ts");
    expect(onNewFile).toHaveBeenCalledTimes(1);
  });
});

// =====================================================================
// R13 组：文件夹右键菜单不受影响（回归）
// =====================================================================

describe("FileTree 根菜单不影响已有右键菜单", () => {
  it("R13: 文件夹右键「新建文件」仍然在该文件夹下创建", () => {
    const onNewFile = vi.fn();
    const dirNode = makeDirNode("src", "C:/proj/src");
    const { getAllByText } = renderFileTree({
      rootPath: "C:/proj",
      nodes: [dirNode],
      onNewFile,
    });

    // 右键文件夹节点 → 弹出文件夹菜单（含「新建文件」）
    fireEvent.contextMenu(getAllByText("src")[0]);
    fireEvent.click(getAllByText("新建文件")[0]);

    // 输入框应出现在文件夹下（newFileName === "C:/proj/src"）
    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "sub.ts" } });
    fireEvent.keyDown(newFileInput, { key: "Enter" });

    expect(onNewFile).toHaveBeenCalledWith("C:/proj/src/sub.ts");
  });
});

// =====================================================================
// R14 组：ExplorerPanel 集成——根级创建失败
// =====================================================================

describe("ExplorerPanel 根级重命名失败", () => {
  it("R14: 空白区域右键新建文件失败 → 错误横幅显示", async () => {
    mocks.mockWriteFile.mockRejectedValue(new Error("磁盘空间不足"));
    mocks.mockReadDir.mockResolvedValue([
      { name: "existing.txt", path: "C:/test-project/existing.txt", isDir: false, size: 32, modified: 1 },
    ]);
    mocks.mockAsk.mockResolvedValue(false);

    seedProject();

    const { container, getByTestId } = renderExplorerPanel();

    // 等待文件树加载
    await waitFor(() => {
      expect(document.body.textContent).toContain("existing.txt");
    });

    // 右键空白区域
    const wrapper = container.querySelector('[style*="min-height: 100%"]');
    if (wrapper) {
      fireEvent.contextMenu(wrapper);
    }

    // 点击「新建文件」
    const newFileItems = document.querySelectorAll('div');
    const newFileBtn = Array.from(newFileItems).find(
      (el) => el.textContent === "新建文件",
    );
    if (newFileBtn) {
      fireEvent.click(newFileBtn);
    }

    // 内联输入框输入文件名并确认
    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    if (newFileInput) {
      fireEvent.change(newFileInput, { target: { value: "newfile.ts" } });
      fireEvent.keyDown(newFileInput, { key: "Enter" });
    }

    // UI 错误横幅应出现
    await waitFor(() => {
      const banner = getByTestId("explorer-error-banner");
      expect(banner.textContent).toContain("新建文件失败");
      expect(banner.textContent).toContain("磁盘空间不足");
    });
  });
});
