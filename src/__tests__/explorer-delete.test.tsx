// explorer-delete.test.tsx — 文件浏览器右键删除确认自动化测试
//
// 覆盖：
//   E1 组：FileTree 文件/文件夹删除 — confirmDialog 调用与 onDelete 回调
//   E2 组：confirmDialog 参数验证 — 消息/title/kind/danger
//   E3 组：ExplorerPanel 集成 — deleteEntry → refresh 链路
//   E4 组：边界条件 — 右键菜单包含"删除"项
//   E5 组：操作失败 UI 通知 — 失败 → 内联错误横幅 + 横幅 dismiss/自动消失/卸载清理（EXP-04）
//   E6 组：键盘 Del 删除 — ShortcutRegistry 路径（编号 17-22 与全文连续，EXP-11）
//   E7 组：右键菜单视觉规格（UI-802）— 项 28px/圆角 5/hover token/危险项 ERROR_FG

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup, act } from "@testing-library/react";
import {
  SIDEBAR_BG,
  SIDEBAR_FG,
  SECONDARY_BG,
  ERROR_FG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
} from "../theme";

// ── 测试辅助 ──

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  return hex;
}

/** rgba 输入 jsdom 可能在逗号后补空格——比对前去空白 */
function normColor(c: string): string {
  return c.replace(/\s/g, "");
}

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
      mockConfirmDialog.mockClear();
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

// Mock ConfirmDialog（FileTree/ExplorerPanel 经 ../../lib/ConfirmDialog 的 confirmDialog 引用，OV-02）
vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: mocks.mockConfirmDialog,
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
  onFsEvent: () => () => {},
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { FileTree } from "../features/explorer/FileTree";
import { ExplorerPanel } from "../features/explorer";
import type { TreeNode } from "../features/explorer/useFileTree";
import type { ExplorerActions } from "../features/explorer/activeExplorer";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";
import { setActiveExplorer, clearActiveExplorer, getActiveExplorer } from "../features/explorer/activeExplorer";
import { createExplorerShortcuts } from "../features/explorer/keyboard";

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
    selectedPath: null,
    onSelect: vi.fn(),
    renamingPath: null,
    renameValue: "",
    onRenameStart: vi.fn(),
    onRenameCancel: vi.fn(),
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
  mocks.mockConfirmDialog.mockResolvedValue(false); // 默认取消
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

describe("FileTree 删除确认 — confirmDialog 弹窗分支", () => {
  it("1. 文件删除 + 用户确认 → onDelete 调用传入路径", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
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
    }, { timeout: 3000 });
  });

  it("2. 文件删除 + 用户取消 → onDelete 不调用", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(false);
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
    mocks.mockConfirmDialog.mockResolvedValue(true);
    const onDelete = vi.fn();
    const dirNode = makeDirNode("src", "C:/project/src");

    const { getAllByText } = renderFileTree([dirNode], { onDelete });

    fireEvent.contextMenu(getAllByText("src")[0]);
    const deleteItems = getAllByText("删除");
    fireEvent.click(deleteItems[0]);

    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith("C:/project/src");
    }, { timeout: 3000 });
  });

  it("4. 文件夹删除 + 用户取消 → onDelete 不调用", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(false);
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
// E2 组：confirmDialog 参数验证
// =====================================================================

describe("FileTree 删除确认 — confirmDialog 参数", () => {
  it("5. 文件删除 confirmDialog 参数：消息含文件名+不可撤销，title/kind/danger 正确", () => {
    const onDelete = vi.fn();
    const fileNode = makeFileNode("main.tsx", "C:/project/main.tsx");

    const { getAllByText } = renderFileTree([fileNode], { onDelete });

    fireEvent.contextMenu(getAllByText("main.tsx")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    expect(mocks.mockConfirmDialog).toHaveBeenCalledTimes(1);
    const [opts] = mocks.mockConfirmDialog.mock.calls[0];

    expect(opts).toEqual({
      title: "确认删除",
      message: `确定删除 "main.tsx"？此操作不可撤销。`,
      kind: "warning",
      danger: true,
    });
  });

  it("6. 文件夹删除 confirmDialog 参数：消息含文件夹名+不可撤销，title/kind/danger 正确", () => {
    const onDelete = vi.fn();
    const dirNode = makeDirNode("components", "C:/project/components");

    const { getAllByText } = renderFileTree([dirNode], { onDelete });

    fireEvent.contextMenu(getAllByText("components")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    expect(mocks.mockConfirmDialog).toHaveBeenCalledTimes(1);
    const [opts] = mocks.mockConfirmDialog.mock.calls[0];

    expect(opts).toEqual({
      title: "确认删除",
      message: `确定删除文件夹 "components"？此操作不可撤销。`,
      kind: "warning",
      danger: true,
    });
  });
});

// =====================================================================
// E3 组：ExplorerPanel 集成 — deleteEntry → refresh
// =====================================================================

describe("ExplorerPanel 删除集成", () => {
  it("7. 删除成功 → deleteEntry 调用并触发 refresh", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
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
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("config.json")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    await waitFor(() => {
      expect(mocks.mockDeleteEntry).toHaveBeenCalledWith("C:/test-project/config.json");
    }, { timeout: 3000 });

    // refresh 会再次调用 readDir
    // 初始化时 useFileTree.loadRoot 调用一次，删除后 refresh 再调一次 = 2
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });
  });

  it("8. 删除失败 → UI 错误横幅显示", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
    mocks.mockDeleteEntry.mockRejectedValue(new Error("权限不足"));

    mocks.mockReadDir.mockResolvedValue([
      { name: "readonly.txt", path: "C:/test-project/readonly.txt", isDir: false, size: 32, modified: 1 },
    ]);

    seedProject();

    const { getAllByText, getByTestId } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("readonly.txt").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("readonly.txt")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    // UI 错误横幅应出现，包含错误消息
    await waitFor(() => {
      const banner = getByTestId("explorer-error-banner");
      expect(banner.textContent).toContain("删除失败");
      expect(banner.textContent).toContain("权限不足");
    }, { timeout: 3000 });
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

// =====================================================================
// E7 组：右键菜单视觉规格（UI-802）— 项 28px/圆角 5/hover token/危险项 ERROR_FG
// =====================================================================

describe("FileTree 右键菜单视觉规格（UI-802）", () => {
  it("23. 菜单容器：SIDEBAR_BG 底 + CONTEXT_MENU_BORDER 描边 + 圆角 5 + contextMenuShadow 阴影", () => {
    const fileNode = makeFileNode("index.ts", "C:/project/index.ts");

    const { getAllByText } = renderFileTree([fileNode]);
    fireEvent.contextMenu(getAllByText("index.ts")[0]);

    const menu = document.querySelector('[style*="position: fixed"]') as HTMLElement;
    expect(menu).toBeTruthy();
    expect(menu.style.borderRadius).toBe("5px");
    expect(menu.style.background).toBe(hexToRgb(SIDEBAR_BG));
    expect(normColor(menu.style.border)).toBe(
      normColor(`1px solid ${CONTEXT_MENU_BORDER}`),
    );
    expect(normColor(menu.style.boxShadow)).toBe(
      normColor(SIDEBAR_COLORS.contextMenuShadow),
    );
  });

  it("24. 菜单项：28px 高 + 圆角 5 + hover 变 SECONDARY_BG（#222227）", () => {
    const fileNode = makeFileNode("index.ts", "C:/project/index.ts");

    const { getAllByText } = renderFileTree([fileNode]);
    fireEvent.contextMenu(getAllByText("index.ts")[0]);

    const item = getAllByText("打开")[0] as HTMLElement;
    expect(item.style.height).toBe("28px");
    expect(item.style.borderRadius).toBe("5px");
    expect(item.style.color).toBe(hexToRgb(SIDEBAR_FG));

    fireEvent.mouseEnter(item);
    expect(item.style.background).toBe(hexToRgb(SECONDARY_BG));
    fireEvent.mouseLeave(item);
    expect(item.style.background).toBe("transparent");
  });

  it("25. 危险项「删除」ERROR_FG 着色、普通项 SIDEBAR_FG（UI-802）", () => {
    const fileNode = makeFileNode("index.ts", "C:/project/index.ts");

    const { getAllByText } = renderFileTree([fileNode]);
    fireEvent.contextMenu(getAllByText("index.ts")[0]);

    const deleteItem = getAllByText("删除")[0] as HTMLElement;
    expect(deleteItem.style.color).toBe(hexToRgb(ERROR_FG));
    const openItem = getAllByText("打开")[0] as HTMLElement;
    expect(openItem.style.color).toBe(hexToRgb(SIDEBAR_FG));
  });
});

// =====================================================================
// E5 组：操作失败 UI 通知 — 删除/重命名/新建文件/新建文件夹失败 → 内联错误横幅
// =====================================================================

describe("ExplorerPanel 操作失败 UI 通知", () => {
  it("11. 重命名失败 → UI 错误横幅显示", async () => {
    mocks.mockRename.mockRejectedValue(new Error("文件被锁定"));
    mocks.mockReadDir.mockResolvedValue([
      { name: "locked.ts", path: "C:/test-project/locked.ts", isDir: false, size: 32, modified: 1 },
    ]);

    seedProject();

    const { getAllByText, getByTestId } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("locked.ts").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 右键 → 重命名
    fireEvent.contextMenu(getAllByText("locked.ts")[0]);
    fireEvent.click(getAllByText("重命名")[0]);

    // 内联输入框出现，修改文件名并回车确认
    const inputs = document.querySelectorAll('input');
    const renameInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(renameInput, { target: { value: "new-name.ts" } });
    fireEvent.keyDown(renameInput, { key: "Enter" });

    // UI 错误横幅应出现
    await waitFor(() => {
      const banner = getByTestId("explorer-error-banner");
      expect(banner.textContent).toContain("重命名失败");
      expect(banner.textContent).toContain("文件被锁定");
    }, { timeout: 3000 });
  });

  it("12. 新建文件失败 → UI 错误横幅显示", async () => {
    mocks.mockWriteFile.mockRejectedValue(new Error("磁盘空间不足"));
    mocks.mockReadDir.mockResolvedValue([
      { name: "src", path: "C:/test-project/src", isDir: true, size: undefined, modified: undefined },
    ]);
    mocks.mockConfirmDialog.mockResolvedValue(false);

    seedProject();

    const { getAllByText, getByTestId } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("src").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 右键文件夹 → 新建文件
    fireEvent.contextMenu(getAllByText("src")[0]);
    fireEvent.click(getAllByText("新建文件")[0]);

    // 内联输入框出现，输入文件名并回车确认
    const inputs = document.querySelectorAll('input');
    const newFileInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFileInput, { target: { value: "newfile.ts" } });
    fireEvent.keyDown(newFileInput, { key: "Enter" });

    // UI 错误横幅应出现
    await waitFor(() => {
      const banner = getByTestId("explorer-error-banner");
      expect(banner.textContent).toContain("新建文件失败");
      expect(banner.textContent).toContain("磁盘空间不足");
    }, { timeout: 3000 });
  });

  it("13. 新建文件夹失败 → UI 错误横幅显示", async () => {
    mocks.mockCreateDir.mockRejectedValue(new Error("权限不足"));
    mocks.mockReadDir.mockResolvedValue([
      { name: "lib", path: "C:/test-project/lib", isDir: true, size: undefined, modified: undefined },
    ]);
    mocks.mockConfirmDialog.mockResolvedValue(false);

    seedProject();

    const { getAllByText, getByTestId } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("lib").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    // 右键文件夹 → 新建文件夹
    fireEvent.contextMenu(getAllByText("lib")[0]);
    fireEvent.click(getAllByText("新建文件夹")[0]);

    // 内联输入框出现，输入文件夹名并回车确认
    const inputs = document.querySelectorAll('input');
    const newFolderInput = inputs[inputs.length - 1] as HTMLInputElement;
    fireEvent.change(newFolderInput, { target: { value: "newfolder" } });
    fireEvent.keyDown(newFolderInput, { key: "Enter" });

    // UI 错误横幅应出现
    await waitFor(() => {
      const banner = getByTestId("explorer-error-banner");
      expect(banner.textContent).toContain("新建文件夹失败");
      expect(banner.textContent).toContain("权限不足");
    }, { timeout: 3000 });
  });

  it("14. 错误横幅 × 按钮点击 → 横幅立即消失", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
    mocks.mockDeleteEntry.mockRejectedValue(new Error("权限不足"));
    mocks.mockReadDir.mockResolvedValue([
      { name: "readonly.txt", path: "C:/test-project/readonly.txt", isDir: false, size: 32, modified: 1 },
    ]);

    seedProject();

    const { getAllByText, getByTestId, queryByTestId, container } = renderExplorerPanel();

    await waitFor(() => {
      expect(getAllByText("readonly.txt").length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    fireEvent.contextMenu(getAllByText("readonly.txt")[0]);
    fireEvent.click(getAllByText("删除")[0]);

    // 错误横幅出现
    await waitFor(() => {
      expect(getByTestId("explorer-error-banner")).toBeTruthy();
    }, { timeout: 3000 });

    // 点击 × 关闭按钮（aria-label="关闭错误提示"）
    const closeBtn = container.querySelector('[aria-label="关闭错误提示"]')!;
    fireEvent.click(closeBtn);

    expect(queryByTestId("explorer-error-banner")).toBeNull();
  });

  it("15. 错误横幅 5 秒后自动消失（fake timers）", async () => {
    vi.useFakeTimers();
    try {
      mocks.mockConfirmDialog.mockResolvedValue(true);
      mocks.mockDeleteEntry.mockRejectedValue(new Error("权限不足"));
      mocks.mockReadDir.mockResolvedValue([
        { name: "readonly.txt", path: "C:/test-project/readonly.txt", isDir: false, size: 32, modified: 1 },
      ]);

      seedProject();
      const { getAllByText, getByTestId, queryByTestId } = renderExplorerPanel();

      // 冲刷初始加载微任务（readDir/gitStatus）
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(getAllByText("readonly.txt").length).toBeGreaterThan(0);

      fireEvent.contextMenu(getAllByText("readonly.txt")[0]);
      fireEvent.click(getAllByText("删除")[0]);

      // 冲刷 ask → deleteEntry reject → showError 微任务链
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(getByTestId("explorer-error-banner")).toBeTruthy();

      // 4.999s 未到边界 → 横幅仍在
      act(() => {
        vi.advanceTimersByTime(4999);
      });
      expect(queryByTestId("explorer-error-banner")).toBeTruthy();

      // 跨过 5s 边界（ERROR_AUTO_DISMISS_MS=5000）→ 横幅消失
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(queryByTestId("explorer-error-banner")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("16. 错误横幅卸载 → 自动消失定时器被清理（无残留 timer）", async () => {
    vi.useFakeTimers();
    try {
      mocks.mockConfirmDialog.mockResolvedValue(true);
      mocks.mockDeleteEntry.mockRejectedValue(new Error("权限不足"));
      mocks.mockReadDir.mockResolvedValue([
        { name: "readonly.txt", path: "C:/test-project/readonly.txt", isDir: false, size: 32, modified: 1 },
      ]);

      seedProject();
      const { getAllByText, getByTestId, unmount } = renderExplorerPanel();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(getAllByText("readonly.txt").length).toBeGreaterThan(0);

      fireEvent.contextMenu(getAllByText("readonly.txt")[0]);
      fireEvent.click(getAllByText("删除")[0]);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(getByTestId("explorer-error-banner")).toBeTruthy();
      // 自动消失定时器已注册
      expect(vi.getTimerCount()).toBe(1);

      unmount();
      // 卸载清理应清掉定时器（useEffect cleanup → clearTimeout）
      expect(vi.getTimerCount()).toBe(0);

      // 推进时间不抛错（定时器已清理，无卸载后 setState）
      expect(() => act(() => vi.advanceTimersByTime(6000))).not.toThrow();
    } finally {
      vi.useRealTimers();
    }
  });
});

// =====================================================================
// E6 组：键盘 Del 删除 — ShortcutRegistry 路径
// =====================================================================

function makeKeyboardEvent(code: string): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    code,
    key: code,
    bubbles: true,
    cancelable: true,
  });
}

function makeExplorerActions(overrides: Partial<ExplorerActions> = {}): ExplorerActions {
  return {
    getSelectedPath: () => "/a/test.ts",
    deleteSelected: vi.fn().mockResolvedValue(undefined),
    openSelected: vi.fn(),
    renameSelected: vi.fn(),
    isRenaming: () => false,
    ...overrides,
  };
}

describe("键盘 Del 删除 (ShortcutRegistry)", () => {
  beforeEach(() => {
    getShortcutRegistry()._reset();
    getShortcutRegistry().register(createExplorerShortcuts());
    const a = getActiveExplorer();
    if (a) clearActiveExplorer(a);
  });

  afterEach(() => {
    getShortcutRegistry()._reset();
    const a = getActiveExplorer();
    if (a) clearActiveExplorer(a);
  });

  it("17. ShortcutRegistry 含 explorer.delete 命令", () => {
    const cmds = getShortcutRegistry().listCommands();
    const ids = cmds.map((c: { id: string }) => c.id);
    expect(ids).toContain("explorer.delete");
  });

  it("18. Del 键盘事件 + explorer context → deleteSelected 调用", () => {
    const actions = makeExplorerActions();
    setActiveExplorer(actions);
    getShortcutRegistry().pushContext("explorer");

    const event = makeKeyboardEvent("Delete");
    window.dispatchEvent(event);

    expect(actions.deleteSelected).toHaveBeenCalledOnce();
  });

  it("19. 无选中 + Del → handler 仍派发 deleteSelected（空选中判空在 action 实现内）", () => {
    // 标题与断言对齐（EXP-11）：keyboard.ts 的 handler 不做选中判空——
    // 总是调用 e.deleteSelected()，判空发生在 ExplorerPanel.handleDeleteSelected 内部
    // （selectedPathRef.current 为 null 时直接 return）。此处锁死「handler 派发」语义。
    const actions = makeExplorerActions({ getSelectedPath: () => null, deleteSelected: vi.fn().mockResolvedValue(undefined) });
    setActiveExplorer(actions);
    getShortcutRegistry().pushContext("explorer");

    const event = makeKeyboardEvent("Delete");
    window.dispatchEvent(event);

    expect(actions.deleteSelected).toHaveBeenCalledOnce();
  });

  it("20. explorer 无焦点 + Del → 不匹配（context 不在栈中）", () => {
    const actions = makeExplorerActions();
    setActiveExplorer(actions);
    // 不 pushContext("explorer")

    const event = makeKeyboardEvent("Delete");
    window.dispatchEvent(event);

    expect(actions.deleteSelected).not.toHaveBeenCalled();
  });

  it("21. isRenaming=true + Del → handler 返回 false（透传）", () => {
    const actions = makeExplorerActions({ isRenaming: () => true });
    setActiveExplorer(actions);
    getShortcutRegistry().pushContext("explorer");

    const event = makeKeyboardEvent("Delete");
    window.dispatchEvent(event);

    expect(actions.deleteSelected).not.toHaveBeenCalled();
  });

  it("22. 非 Del 键 → deleteSelected 不调用", () => {
    const actions = makeExplorerActions();
    setActiveExplorer(actions);
    getShortcutRegistry().pushContext("explorer");

    const event = makeKeyboardEvent("KeyA");
    window.dispatchEvent(event);

    expect(actions.deleteSelected).not.toHaveBeenCalled();
  });
});
