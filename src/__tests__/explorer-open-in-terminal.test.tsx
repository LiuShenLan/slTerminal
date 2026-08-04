// explorer-open-in-terminal.test.tsx — 「在终端中打开」测试（EXP-01）
//
// 覆盖：
//   O1 组：FileTree 独立组件——右键菜单含「在终端中打开」项，点击回调 onOpenInTerminal
//   O2 组：ExplorerPanel 集成——addPanel 参数（component/cwd/panelId/renderer/title）

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockAddPanel = vi.fn();
  const mockGetPanel = vi.fn();

  return {
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockAddPanel,
    mockGetPanel,
    resetAll() {
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockAddPanel.mockReset();
      mockGetPanel.mockReset();
      mockReadDir.mockResolvedValue([]);
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
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
import { PANEL_TERMINAL } from "../panelRegistry";

// ─── 辅助函数 ───

function makeFileNode(name: string, path: string): TreeNode {
  return {
    entry: { name, path, isDir: false, size: 100, modified: Date.now() },
    expanded: false,
    children: [],
    loading: false,
  };
}

function makeDirNode(name: string, path: string): TreeNode {
  return {
    entry: { name, path, isDir: true, size: undefined, modified: undefined },
    expanded: false,
    children: [],
    loading: false,
  };
}

/** 渲染 FileTree（独立组件，控制 onOpenInTerminal 回调） */
function renderFileTree(
  nodes: TreeNode[],
  overrides: Partial<{ onOpenInTerminal: (path: string) => void; rootPath: string }> = {},
) {
  const defaultProps = {
    nodes,
    depth: 0,
    gitStatusMap: new Map<string, string>(),
    onToggleExpand: vi.fn(),
    onOpenFile: vi.fn(),
    onOpenInTerminal: overrides.onOpenInTerminal ?? vi.fn(),
    onRename: vi.fn(),
    onDelete: vi.fn(),
    onNewFile: vi.fn(),
    onNewFolder: vi.fn(),
    rootPath: overrides.rootPath,
    selectedPath: null,
    onSelect: vi.fn(),
    renamingPath: null,
    renameValue: "",
    onRenameStart: vi.fn(),
    onRenameCancel: vi.fn(),
  };
  return render(React.createElement(FileTree, defaultProps));
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
  cleanup();
  useProjects.setState({ projects: {}, expandedNodes: {} });
  useLayout.setState({ activePageId: null });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: mocks.mockAddPanel,
    getPanel: mocks.mockGetPanel,
  };
});

// =====================================================================
// O1 组：FileTree 右键菜单「在终端中打开」入口
// =====================================================================

describe("FileTree 右键菜单「在终端中打开」", () => {
  it("O1: 文件右键菜单含「在终端中打开」，点击回调 onOpenInTerminal(文件路径)", () => {
    const onOpenInTerminal = vi.fn();
    const fileNode = makeFileNode("a.ts", "C:/proj/a.ts");
    const { getAllByText } = renderFileTree([fileNode], { onOpenInTerminal, rootPath: "C:/proj" });

    fireEvent.contextMenu(getAllByText("a.ts")[0]);
    // 文件菜单含「在终端中打开」项
    const openItems = getAllByText("在终端中打开");
    expect(openItems.length).toBeGreaterThanOrEqual(1);
    fireEvent.click(openItems[0]);

    expect(onOpenInTerminal).toHaveBeenCalledWith("C:/proj/a.ts");
    expect(onOpenInTerminal).toHaveBeenCalledTimes(1);
  });

  it("O2: 文件夹右键菜单含「在终端中打开」，点击回调 onOpenInTerminal(文件夹路径)", () => {
    const onOpenInTerminal = vi.fn();
    const dirNode = makeDirNode("src", "C:/proj/src");
    const { getAllByText } = renderFileTree([dirNode], { onOpenInTerminal, rootPath: "C:/proj" });

    fireEvent.contextMenu(getAllByText("src")[0]);
    const openItems = getAllByText("在终端中打开");
    expect(openItems.length).toBeGreaterThanOrEqual(1);
    fireEvent.click(openItems[0]);

    expect(onOpenInTerminal).toHaveBeenCalledWith("C:/proj/src");
    expect(onOpenInTerminal).toHaveBeenCalledTimes(1);
  });
});

// =====================================================================
// O2 组：ExplorerPanel 集成 — addPanel 参数断言
// =====================================================================

describe("ExplorerPanel 在终端中打开 — addPanel 参数", () => {
  it("O3: 文件 → cwd 取父目录，component=terminal，renderer=always，panelId 格式 terminal-open-*", async () => {
    mocks.mockReadDir.mockResolvedValue([
      { name: "config.json", path: "C:/test-project/config.json", isDir: false, size: 64, modified: 1 },
    ]);
    seedProject();

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("config.json").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("config.json")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe(PANEL_TERMINAL);
    // 文件 → cwd 取父目录
    expect(call.params.cwd).toBe("C:/test-project");
    // panelId 格式：terminal-open-{时间戳}
    expect(call.id).toMatch(/^terminal-open-\d+$/);
    expect(call.params.panelId).toBe(call.id);
    // renderer 恒为 "always"（页签切换终端不白屏）
    expect(call.renderer).toBe("always");
  });

  it("O4: 文件夹 → cwd 取父目录（当前实现，待产品决策）", async () => {
    // ⚠️ 行为锁定说明：当前实现 `handleOpenInTerminal` 对任何路径统一取
    // `lastIndexOf("/")` 前的父级——文件夹 "C:/test-project/src" 的 cwd 实际为
    // 其父目录 "C:/test-project"。checklist EXP-01 语义为「文件取父目录」，
    // 文件夹自身即目录，语义上应打开在文件夹内（待产品决策，本用例锁当前行为）。
    mocks.mockReadDir.mockResolvedValue([
      { name: "src", path: "C:/test-project/src", isDir: true, size: undefined, modified: undefined },
    ]);
    seedProject();

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("src").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("src")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    // 当前实现：文件夹也取父目录（与 VS Code「在终端中打开」打开在文件夹内的语义存在偏差）
    expect(call.params.cwd).toBe("C:/test-project");
  });

  it("O5: 无斜杠路径（根级单名文件）→ cwd 取路径自身", async () => {
    mocks.mockReadDir.mockResolvedValue([
      { name: "readme.txt", path: "readme.txt", isDir: false, size: 10, modified: 1 },
    ]);
    seedProject("C:/test-project");

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("readme.txt").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("readme.txt")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    // lastIndexOf("/") < 0 → dir 取路径自身
    expect(call.params.cwd).toBe("readme.txt");
  });

  it("O6: 标题使用 getTerminalTitle（活跃页存在时）", async () => {
    mocks.mockReadDir.mockResolvedValue([
      { name: "a.ts", path: "C:/test-project/a.ts", isDir: false, size: 10, modified: 1 },
    ]);
    seedProject();

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("a.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("a.ts")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    // 与 titleManager 的 terminal-N 约定一致
    expect(call.title).toMatch(/^terminal-\d+$/);
  });

  it("O7: 重复「在终端中打开」→ 每次均 addPanel（终端打开无去重语义）", async () => {
    mocks.mockReadDir.mockResolvedValue([
      { name: "a.ts", path: "C:/test-project/a.ts", isDir: false, size: 10, modified: 1 },
    ]);
    seedProject();

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("a.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 连续两次触发
    fireEvent.contextMenu(getAllByText("a.ts")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);
    fireEvent.contextMenu(getAllByText("a.ts")[0]);
    fireEvent.click(getAllByText("在终端中打开")[0]);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // 两次 panelId 各自独立（无去重聚焦）
    const id1 = mocks.mockAddPanel.mock.calls[0][0].id;
    const id2 = mocks.mockAddPanel.mock.calls[1][0].id;
    expect(id1).not.toBe(id2);
  });
});
