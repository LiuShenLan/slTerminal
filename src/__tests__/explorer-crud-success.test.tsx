// explorer-crud-success.test.tsx — CRUD 成功路径断言（EXP-02）
//
// 覆盖：删除/重命名/新建文件/新建文件夹成功后——
//   IPC 调用、refresh()（readDir 二次调用）、状态重置（选中清空/输入框消失）
// 防回归：静默失败不红（如某次重构误删 refresh() 或状态重置）

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockConfirmDialog = vi.fn();
  const mockDeleteEntry = vi.fn();
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockCreateDir = vi.fn();
  const mockRename = vi.fn();
  const mockWriteFile = vi.fn();

  return {
    mockConfirmDialog,
    mockDeleteEntry,
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockCreateDir,
    mockRename,
    mockWriteFile,
    resetAll() {
      mockConfirmDialog.mockReset();
      mockDeleteEntry.mockReset();
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockCreateDir.mockReset();
      mockRename.mockReset();
      mockWriteFile.mockReset();
      mockReadDir.mockResolvedValue([]);
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
      mockDeleteEntry.mockResolvedValue(undefined);
      mockRename.mockResolvedValue(undefined);
      mockWriteFile.mockResolvedValue(undefined);
      mockCreateDir.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: mocks.mockConfirmDialog,
}));

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
  stopWatch: vi.fn().mockResolvedValue(undefined),
  onFsEvent: () => () => {},
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { resetProjectStores } from "./helpers/workspace-setup";
import { ExplorerPanel } from "../features/explorer";

// ─── 辅助函数 ───

/** 根目录文件条目（选中背景色断言，值 = 附录 A explorerSelectionBg rgba） */
const SELECTION_BG_RGB = "rgba(110, 159, 242, 0.13)";

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

/** 当前行背景色（非选中行透明） */
function rowBackground(fileName: string): string {
  // 按行容器 testid 限定（虚拟化 DOM 下 closest("div") 可能取到包裹层——TQ-B-05）
  const rows = document.querySelectorAll<HTMLElement>('[data-testid="tree-node-row"]');
  const row = Array.from(rows).find((r) => r.textContent?.includes(fileName));
  if (!row) throw new Error(`找不到行: ${fileName}`);
  return row.style.background;
}

// ─── 通用 setup ───
beforeEach(() => {
  vi.clearAllMocks();
  mocks.resetAll();
  mocks.mockConfirmDialog.mockResolvedValue(false);
  cleanup();
  resetProjectStores(); // 共享重置：projects/layout/sideBar/keybindings 全量（TQ-B-10）
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: vi.fn(),
    getPanel: vi.fn(),
  };
});

afterEach(() => {
  cleanup();
  delete (window as unknown as Record<string, unknown>).__dockviewApi; // 防过期 mock 外泄（TQ-B-15）
});

// =====================================================================
// C1 组：删除成功路径
// =====================================================================

describe("ExplorerPanel 删除成功路径", () => {
  it("C1: 删除成功 → deleteEntry 调用 + refresh（readDir 二次）+ 选中态清空", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
    const fileEntry = { name: "config.json", path: "C:/test-project/config.json", isDir: false, size: 64, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("config.json").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 先单击选中该行（建立选中态）
    fireEvent.click(getAllByText("config.json")[0]);
    expect(rowBackground("config.json")).toBe(SELECTION_BG_RGB);

    // 右键删除 → 确认
    fireEvent.contextMenu(getAllByText("config.json")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    await waitFor(() => {
      expect(mocks.mockDeleteEntry).toHaveBeenCalledWith("C:/test-project/config.json");
    }, { timeout: 3000 });

    // refresh：readDir 第二次调用
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // 选中态清空（setSelectedPath(null)）——refresh 后行仍渲染（mock 返回同一列表）
    expect(rowBackground("config.json")).not.toBe(SELECTION_BG_RGB);
    // 无错误横幅
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });
});

// =====================================================================
// C2 组：重命名成功路径
// =====================================================================

describe("ExplorerPanel 重命名成功路径", () => {
  it("C2: 重命名成功 → rename 调用 + refresh + renamingPath 清空（输入框消失）", async () => {
    const fileEntry = { name: "old.ts", path: "C:/test-project/old.ts", isDir: false, size: 32, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("old.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 右键 → 重命名 → 输入框出现
    fireEvent.contextMenu(getAllByText("old.ts")[0]);
    fireEvent.click(getAllByText("重命名")[0]);
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(1);

    // 修改名称并回车确认（testid 定位——querySelectorAll('input') 取最后一个不可靠，TQ-B-17）
    const renameInput = document.querySelector(
      '[data-testid="explorer-inline-input"]',
    ) as HTMLInputElement;
    fireEvent.change(renameInput, { target: { value: "new-name.ts" } });
    fireEvent.keyDown(renameInput, { key: "Enter" });

    // IPC 调用参数
    await waitFor(() => {
      expect(mocks.mockRename).toHaveBeenCalledWith(
        "C:/test-project/old.ts",
        "C:/test-project/new-name.ts",
      );
    }, { timeout: 3000 });

    // refresh：readDir 第二次调用
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // renamingPath 清空：输入框消失（TQ-B-17 testid 计数）
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(0);
    // 无错误横幅
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });

  it("C5: 重命名不改名直接 Enter → rename 不被调用 + 输入框消失 + 无错误横幅", async () => {
    // 防复发：修复前同名提交会经 fs_rename(src==dst) 触发后端覆盖分支误删源文件，
    // mockRename 被以 (old.ts, "old.ts") 即 src==dst 调用；修复后应静默取消。
    const fileEntry = { name: "old.ts", path: "C:/test-project/old.ts", isDir: false, size: 32, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const { getAllByText } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(getAllByText("old.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 右键 → 重命名 → 输入框出现
    fireEvent.contextMenu(getAllByText("old.ts")[0]);
    fireEvent.click(getAllByText("重命名")[0]);
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(1);

    // 不改值直接 Enter（非受控 input，defaultValue 即原名）
    const renameInput = document.querySelector(
      '[data-testid="explorer-inline-input"]',
    ) as HTMLInputElement;
    fireEvent.keyDown(renameInput, { key: "Enter" });

    // 同名视为取消：rename IPC 绝不调用
    await waitFor(() => {
      expect(mocks.mockRename).not.toHaveBeenCalled();
    }, { timeout: 3000 });
    // 退出编辑态：输入框消失
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(0);
    // 无错误横幅
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });
});

// =====================================================================
// C3 组：新建文件成功路径
// =====================================================================

describe("ExplorerPanel 新建文件成功路径", () => {
  it("C3: 新建文件成功 → writeFile 调用 + refresh + 输入框消失", async () => {
    mocks.mockReadDir.mockResolvedValue([]);
    mocks.mockWriteFile.mockResolvedValue(undefined);

    seedProject();
    const { container } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(container.textContent).toContain("空目录");
    }, { timeout: 3000 });

    // 右键空白区域 → 新建文件
    const wrapper = container.querySelector('[style*="min-height: 100%"]') as HTMLElement;
    fireEvent.contextMenu(wrapper);
    const newFileItems = Array.from(document.querySelectorAll("div")).find(
      (el) => el.textContent === "新建文件",
    );
    fireEvent.click(newFileItems!);

    const newFileInput = document.querySelector(
      '[data-testid="explorer-inline-input"]',
    ) as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "newfile.ts" } });
    fireEvent.keyDown(newFileInput, { key: "Enter" });

    // IPC：writeFile 空内容
    await waitFor(() => {
      expect(mocks.mockWriteFile).toHaveBeenCalledWith("C:/test-project/newfile.ts", "");
    }, { timeout: 3000 });

    // refresh：readDir 第二次调用
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // newFileName 清空：输入框消失（TQ-B-17 testid 计数）
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(0);
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });
});

// =====================================================================
// C4 组：新建文件夹成功路径
// =====================================================================

describe("ExplorerPanel 新建文件夹成功路径", () => {
  it("C4: 新建文件夹成功 → createDir 调用 + refresh + 输入框消失", async () => {
    mocks.mockReadDir.mockResolvedValue([]);
    mocks.mockCreateDir.mockResolvedValue(undefined);

    seedProject();
    const { container } = render(React.createElement(ExplorerPanel));

    await waitFor(() => {
      expect(container.textContent).toContain("空目录");
    }, { timeout: 3000 });

    // 右键空白区域 → 新建文件夹
    const wrapper = container.querySelector('[style*="min-height: 100%"]') as HTMLElement;
    fireEvent.contextMenu(wrapper);
    const newFolderItems = Array.from(document.querySelectorAll("div")).find(
      (el) => el.textContent === "新建文件夹",
    );
    fireEvent.click(newFolderItems!);

    const newFolderInput = document.querySelector(
      '[data-testid="explorer-inline-input"]',
    ) as HTMLInputElement;
    fireEvent.change(newFolderInput, { target: { value: "src" } });
    fireEvent.keyDown(newFolderInput, { key: "Enter" });

    await waitFor(() => {
      expect(mocks.mockCreateDir).toHaveBeenCalledWith("C:/test-project/src");
    }, { timeout: 3000 });

    // refresh：readDir 第二次调用
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // newFolderName 清空：输入框消失（TQ-B-17 testid 计数）
    expect(document.querySelectorAll('[data-testid="explorer-inline-input"]').length).toBe(0);
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });
});
