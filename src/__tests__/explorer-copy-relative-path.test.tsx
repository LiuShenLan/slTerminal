// explorer-copy-relative-path.test.tsx — 文件浏览器右键「复制相对路径」
//
// 覆盖：
//   C1 组：FileTree 独立组件——文件/文件夹右键菜单含「复制相对路径」且居首；
//          点击写入剪贴板相对项目根路径（Unix 正斜杠）
//   C2 组：越界/退化兜底——目标不在项目根内 / 未提供 projectRootPath → 复制绝对路径
//   C3 组：边界——根级空白菜单无此菜单项；writeText 失败仅 console.error
//   C4 组：ExplorerPanel 集成——浏览根(cwd)≠ 项目根时基准仍为项目根

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockWriteText = vi.fn();
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockAddPanel = vi.fn();
  const mockGetPanel = vi.fn();

  return {
    mockWriteText,
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockAddPanel,
    mockGetPanel,
    resetAll() {
      mockWriteText.mockReset();
      mockWriteText.mockResolvedValue(undefined);
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

vi.mock("../ipc/clipboard", () => ({
  writeText: mocks.mockWriteText,
  readText: vi.fn().mockResolvedValue(""),
}));

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
  stopWatch: vi.fn().mockResolvedValue(undefined),
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

function makeDirNode(name: string, path: string): TreeNode {
  return {
    entry: { name, path, isDir: true, size: null, modified: null },
    expanded: false,
    children: [],
    loading: false,
  };
}

/** 渲染 FileTree（独立组件；projectRootPath 为「复制相对路径」基准） */
function renderFileTree(
  nodes: TreeNode[],
  overrides: Partial<{ projectRootPath: string; rootPath: string }> = {},
) {
  const defaultProps = {
    nodes,
    depth: 0,
    gitStatusMap: new Map<string, string>(),
    onToggleExpand: vi.fn(),
    onOpenFile: vi.fn(),
    onOpenInTerminal: vi.fn(),
    onRename: vi.fn(),
    onDelete: vi.fn(),
    onNewFile: vi.fn(),
    onNewFolder: vi.fn(),
    projectRootPath: overrides.projectRootPath,
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

/** 触发某节点的右键并返回 testing-library 查询集 */
function openRowContextMenu(getAllByText: (t: string) => HTMLElement[], rowLabel: string) {
  fireEvent.contextMenu(getAllByText(rowLabel)[0]);
}

/** 断言「复制相对路径」行位于同一菜单内其它项之前（items 顺序 = 渲染顺序） */
function expectCopyItemBefore(
  getAllByText: (t: string) => HTMLElement[],
  otherLabel: string,
): HTMLElement {
  const copyItems = getAllByText("复制相对路径");
  expect(copyItems.length).toBeGreaterThanOrEqual(1);
  const copyRow = copyItems[0];
  const menu = copyRow.parentElement;
  expect(menu).not.toBeNull();
  const otherRow = getAllByText(otherLabel).find(
    (el) => el.parentElement === menu,
  );
  expect(otherRow).toBeTruthy();
  // 按 DOM 顺序断言复制项在前（DOCUMENT_POSITION_FOLLOWING = 4）
  expect(
    copyRow.compareDocumentPosition(otherRow!) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  return copyRow;
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
// C1 组：文件/文件夹右键菜单含「复制相对路径」，点击写相对项目根路径
// =====================================================================

describe("FileTree 右键「复制相对路径」", () => {
  it("C1: 文件右键菜单含该菜单项（位于「打开」前），点击 → writeText(相对项目根路径)", () => {
    const fileNode = makeFileNode("a.ts", "C:/proj/src/a.ts");
    const { getAllByText } = renderFileTree([fileNode], {
      projectRootPath: "C:/proj",
      rootPath: "C:/proj",
    });

    openRowContextMenu(getAllByText, "a.ts");
    const copyRow = expectCopyItemBefore(getAllByText, "打开");
    fireEvent.click(copyRow);

    expect(mocks.mockWriteText).toHaveBeenCalledTimes(1);
    expect(mocks.mockWriteText).toHaveBeenCalledWith("src/a.ts"); // Unix 正斜杠相对路径
  });

  it("C2: 文件夹右键菜单含该菜单项，点击 → writeText(文件夹相对路径)", () => {
    const dirNode = makeDirNode("src", "C:/proj/src");
    const { getAllByText } = renderFileTree([dirNode], {
      projectRootPath: "C:/proj",
      rootPath: "C:/proj",
    });

    openRowContextMenu(getAllByText, "src");
    fireEvent.click(expectCopyItemBefore(getAllByText, "在终端中打开"));

    expect(mocks.mockWriteText).toHaveBeenCalledWith("src");
  });

  it("C3: 深层嵌套文件 → 多级相对路径完整保留", () => {
    const fileNode = makeFileNode("b.ts", "C:/proj/a/b/c/b.ts");
    const { getAllByText } = renderFileTree([fileNode], {
      projectRootPath: "C:/proj",
      rootPath: "C:/proj",
    });

    openRowContextMenu(getAllByText, "b.ts");
    fireEvent.click(getAllByText("复制相对路径")[0]);

    expect(mocks.mockWriteText).toHaveBeenCalledWith("a/b/c/b.ts");
  });
});

// =====================================================================
// C2 组：越界 / 未提供项目根 → 兜底复制绝对路径
// =====================================================================

describe("兜底：目标不在项目根内或未提供项目根", () => {
  it("C4: 目标在项目根外 → writeText(完整绝对路径)", () => {
    const fileNode = makeFileNode("x.ts", "D:/other/lib/x.ts");
    const { getAllByText } = renderFileTree([fileNode], {
      projectRootPath: "C:/proj",
      rootPath: "C:/proj",
    });

    openRowContextMenu(getAllByText, "x.ts");
    fireEvent.click(getAllByText("复制相对路径")[0]);

    expect(mocks.mockWriteText).toHaveBeenCalledWith("D:/other/lib/x.ts");
  });

  it("C5: 未传 projectRootPath → 菜单项仍在，点击兜底绝对路径", () => {
    const fileNode = makeFileNode("a.ts", "C:/proj/a.ts");
    const { getAllByText } = renderFileTree([fileNode]);

    openRowContextMenu(getAllByText, "a.ts");
    fireEvent.click(getAllByText("复制相对路径")[0]);

    expect(mocks.mockWriteText).toHaveBeenCalledWith("C:/proj/a.ts");
  });

  it("C6: projectRootPath 为 undefined 时相对化不抛异常（path.ts 空输入契约）", () => {
    const dirNode = makeDirNode("src", "C:/proj/src");
    const { getAllByText } = renderFileTree([dirNode], { rootPath: "C:/proj" });

    openRowContextMenu(getAllByText, "src");
    expect(() => {
      fireEvent.click(getAllByText("复制相对路径")[0]);
    }).not.toThrow();
    expect(mocks.mockWriteText).toHaveBeenCalledWith("C:/proj/src");
  });
});

// =====================================================================
// C3 组：边界——根级空白菜单无此项；writeText 失败仅 console.error
// =====================================================================

describe("边界", () => {
  it("C7: 根级空白区域右键菜单（新建文件/新建文件夹）不含「复制相对路径」", () => {
    const { container, getAllByText, queryAllByText } = renderFileTree([], {
      rootPath: "C:/proj",
    });

    // wrapper（depth === 0 顶层 div）承载根级空白右键
    const wrapper = container.firstChild as Element;
    fireEvent.contextMenu(wrapper);

    // 根菜单信号：出现「新建文件夹」
    expect(getAllByText("新建文件夹").length).toBeGreaterThanOrEqual(1);
    expect(queryAllByText("复制相对路径").length).toBe(0);
  });

  it("C8: writeText 被拒 → console.error 且无未处理拒绝", async () => {
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.mockWriteText.mockRejectedValueOnce(new Error("clipboard denied"));
    const fileNode = makeFileNode("a.ts", "C:/proj/src/a.ts");
    const { getAllByText } = renderFileTree([fileNode], {
      projectRootPath: "C:/proj",
      rootPath: "C:/proj",
    });

    openRowContextMenu(getAllByText, "a.ts");
    fireEvent.click(getAllByText("复制相对路径")[0]);

    // 失败被吞并记录（无 unhandled rejection 即通过；await 让微任务落定）
    await waitFor(() => {
      expect(consoleErrorSpy).toHaveBeenCalled();
    });
    expect(consoleErrorSpy.mock.calls[0][0]).toContain("复制相对路径失败");
    consoleErrorSpy.mockRestore();
  });
});

// =====================================================================
// C4 组：ExplorerPanel 集成——浏览根(cwd) ≠ 项目根时基准仍为项目根
// =====================================================================

describe("ExplorerPanel 集成", () => {
  it("C9: 页面 cwd 为项目根子目录时，复制结果仍相对项目根（含子目录前缀）", async () => {
    mocks.mockReadDir.mockResolvedValue([
      { name: "a.ts", path: "C:/proj/sub/a.ts", isDir: false, size: 64, modified: 1 },
    ]);
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "测试项目",
          rootPath: "C:/proj",
          pages: [
            {
              pageId: "page-1",
              name: "页面 1",
              layout: {},
              cwd: "C:/proj/sub", // 浏览根 ≠ 项目根
              createdAt: 1,
              lastAccessedAt: 1,
            },
          ],
          activePageId: "page-1",
          version: 1,
        },
      },
      expandedNodes: { "proj-1": true },
      deletionLock: { pendingDelete: null, acquiredAt: null },
    });
    useLayout.setState({ activePageId: "page-1" });

    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("a.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("a.ts")[0]);
    fireEvent.click(getAllByText("复制相对路径")[0]);

    // 基准是项目根 C:/proj，不是浏览根 C:/proj/sub
    expect(mocks.mockWriteText).toHaveBeenCalledWith("sub/a.ts");
  });
});
