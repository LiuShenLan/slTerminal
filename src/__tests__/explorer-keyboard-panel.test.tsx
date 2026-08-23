// explorer-keyboard-panel.test.tsx — 键盘快捷键在真实 ExplorerPanel 上的动作链路补测（TQ-COV-09）
//
// 背景：explorer-keyboard.test.ts / explorer-delete.test.tsx E6 组已覆盖「命令工厂派发」与
// 「焦点链路建立」，但 v8 报告显示 ExplorerPanel 内实际动作实现仍有未覆盖分支：
//   - handleDeleteSelected 确认删除成功（117-120）/ 失败错误横幅（121-123）
//   - handleOpenSelected 文件打开 / 目录展开（131-148）
//   - handleRenameSelected（155-158）+ handleRenameCancel（308-309）
//   以及 ref 模式动作包装 getSelectedPath/deleteSelected/openSelected（167/169-170）
//
// 本文件经真实焦点链路驱动（照 explorer-delete.test.tsx E6-集成 先例）：
// 单击行 → handleSelect（选中 + container.focus()）→ focusin → usePanelFocus
// pushContext("explorer") + setActiveExplorer —— 不经手动 pushContext/setActiveExplorer。
//
// 每例断言用户可见行为：IPC 调用 + DOM（行高亮/错误横幅/内联输入框）随动作同步。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
import { titleManager } from "../workspace/titleManager";
import { ExplorerPanel } from "../features/explorer";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";
import { createExplorerShortcuts } from "../features/explorer/keyboard";
import { clearActiveExplorer, getActiveExplorer } from "../features/explorer/activeExplorer";
import { makeKeydown } from "./helpers/keyboard";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();
  const mockDeleteEntry = vi.fn();
  const mockRename = vi.fn();
  const mockWriteFile = vi.fn();
  const mockCreateDir = vi.fn();
  const mockConfirmDialog = vi.fn();
  const mockAddPanel = vi.fn();

  return {
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockDeleteEntry,
    mockRename,
    mockWriteFile,
    mockCreateDir,
    mockConfirmDialog,
    mockAddPanel,
    resetAll() {
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockDeleteEntry.mockReset();
      mockRename.mockReset();
      mockWriteFile.mockReset();
      mockCreateDir.mockReset();
      mockConfirmDialog.mockReset();
      mockAddPanel.mockReset();
      mockReadDir.mockResolvedValue([]);
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
      mockDeleteEntry.mockResolvedValue(undefined);
      mockRename.mockResolvedValue(undefined);
      mockWriteFile.mockResolvedValue(undefined);
      mockCreateDir.mockResolvedValue(undefined);
      mockConfirmDialog.mockResolvedValue(false);
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

// ─── 辅助函数 ───

/** 选中态背景（照 explorer-crud-success 附录 A explorerSelectionBg rgba） */
const SELECTION_BG_RGB = "rgba(110, 159, 242, 0.13)";

/** 种子项目 store：单页面 + rootPath */
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

/** 渲染 ExplorerPanel 并等待指定文件名行出现，返回该行文本 span */
async function renderAndFindRow(fileName: string): Promise<HTMLElement> {
  const { findAllByText } = render(React.createElement(ExplorerPanel));
  const items = await findAllByText(fileName);
  return items.find((el) => el.tagName === "SPAN") as HTMLElement;
}

/** 当前行背景色（非选中行透明）——按行容器 testid 限定（TQ-B-05） */
function rowBackground(fileName: string): string {
  const rows = document.querySelectorAll<HTMLElement>('[data-testid="tree-node-row"]');
  const row = Array.from(rows).find((r) => r.textContent?.includes(fileName));
  if (!row) throw new Error(`找不到行: ${fileName}`);
  return row.style.background;
}

/** 建立 explorer 焦点上下文（照 E6-集成：显式 focusIn 防 jsdom 聚焦时序差异，双保险） */
function focusExplorer(): void {
  const treeContainer = document.querySelector('[data-e2e="explorer-tree-container"]');
  if (!treeContainer) throw new Error("explorer 树容器未渲染");
  fireEvent.focusIn(treeContainer as Element);
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.resetAll();
  getShortcutRegistry()._reset();
  getShortcutRegistry().register(createExplorerShortcuts());
  titleManager.reset(); // 打开文件会注册编辑器标题——防测试间泄漏
  cleanup();
  useProjects.setState({ projects: {}, expandedNodes: {} });
  useLayout.setState({ activePageId: null });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: mocks.mockAddPanel,
    getPanel: vi.fn(),
  };
});

afterEach(() => {
  getShortcutRegistry()._reset();
  const a = getActiveExplorer();
  if (a) clearActiveExplorer(a);
  cleanup();
  delete (window as unknown as Record<string, unknown>).__dockviewApi; // 防过期 mock 外泄（TQ-B-15）
});

// =====================================================================
// 键盘 Delete：handleDeleteSelected 确认删除成功 / 失败错误横幅
// =====================================================================

describe("键盘 Delete 动作链路（handleDeleteSelected）", () => {
  it("确认删除 → deleteEntry 调用 + 选中清空 + refresh（readDir 二次）", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
    const fileEntry = { name: "target.ts", path: "C:/test-project/target.ts", isDir: false, size: 64, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const rowSpan = await renderAndFindRow("target.ts");
    fireEvent.click(rowSpan); // 单击行 → 选中 + 容器聚焦
    expect(rowBackground("target.ts")).toBe(SELECTION_BG_RGB);
    focusExplorer();

    window.dispatchEvent(makeKeydown({ code: "Delete" }));

    // 用户可见链路：confirmDialog 确认 → deleteEntry → refresh → 选中清空
    await waitFor(() => {
      expect(mocks.mockDeleteEntry).toHaveBeenCalledWith("C:/test-project/target.ts");
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2); // 初始加载 + refresh
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(rowBackground("target.ts")).not.toBe(SELECTION_BG_RGB); // setSelectedPath(null)
    }, { timeout: 3000 });
    // 成功路径无错误横幅
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });

  it("删除失败 → 错误横幅「删除失败: …」文案 + 关闭钮 dismiss", async () => {
    mocks.mockConfirmDialog.mockResolvedValue(true);
    mocks.mockDeleteEntry.mockRejectedValue(new Error("permission denied"));
    const fileEntry = { name: "locked.ts", path: "C:/test-project/locked.ts", isDir: false, size: 64, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const rowSpan = await renderAndFindRow("locked.ts");
    fireEvent.click(rowSpan);
    focusExplorer();

    window.dispatchEvent(makeKeydown({ code: "Delete" }));

    // 用户可见：错误横幅出现，文案含「删除失败」前缀 + 错误消息
    // 注意：waitFor 回调必须抛错式断言（非 throw 型 waitFor 会立即以 null 返回）
    await waitFor(() => {
      const banner = document.querySelector('[data-testid="explorer-error-banner"]');
      expect(banner).toBeTruthy();
      expect(banner?.textContent).toContain("删除失败");
      expect(banner?.textContent).toContain("permission denied");
    }, { timeout: 3000 });
    const banner = document.querySelector('[data-testid="explorer-error-banner"]') as HTMLElement;

    // 关闭钮（aria-label）点击 → 横幅消失
    fireEvent.click(
      banner!.querySelector('button[aria-label="关闭错误提示"]') as Element,
    );
    expect(document.querySelector('[data-testid="explorer-error-banner"]')).toBeNull();
  });
});

// =====================================================================
// 键盘 Enter：handleOpenSelected 文件打开 / 目录展开
// =====================================================================

describe("键盘 Enter 动作链路（handleOpenSelected）", () => {
  it("选中文件 Enter → addPanel（editor + params.filePath）", async () => {
    const fileEntry = { name: "app.ts", path: "C:/test-project/app.ts", isDir: false, size: 200, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const rowSpan = await renderAndFindRow("app.ts");
    fireEvent.click(rowSpan);
    focusExplorer();

    window.dispatchEvent(makeKeydown({ code: "Enter" }));

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });
    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("editor");
    expect(call.params.filePath).toBe("C:/test-project/app.ts");
    expect(call.id).toMatch(/^editor-/);
  });

  it("选中目录 Enter → 仅切换展开（子树显隐），不触发 addPanel", async () => {
    // src 目录含子文件——展开可观测（行显隐），关闭展开可观测（行消失）
    const srcDir = { name: "src", path: "C:/test-project/src", isDir: true, size: null, modified: 1 };
    const innerFile = { name: "inner.ts", path: "C:/test-project/src/inner.ts", isDir: false, size: 10, modified: 1 };
    mocks.mockReadDir.mockImplementation((dirPath: string) => {
      if (dirPath === "C:/test-project") return Promise.resolve([srcDir]);
      if (dirPath === "C:/test-project/src") return Promise.resolve([innerFile]);
      return Promise.resolve([]);
    });

    seedProject();
    const dirSpan = await renderAndFindRow("src");
    fireEvent.click(dirSpan); // 单击目录行 = 选中 + 展开
    focusExplorer();

    // 用户可见：src 子文件行渲染（目录已展开）
    await waitFor(() => {
      expect(
        Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).some(
          (r) => r.textContent?.includes("inner.ts"),
        ),
      ).toBe(true);
    }, { timeout: 3000 });

    // Enter → handleOpenSelected 命中 isDir → toggleExpand 收起（inner.ts 行消失）
    window.dispatchEvent(makeKeydown({ code: "Enter" }));
    await waitFor(() => {
      expect(
        Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).find(
          (r) => r.textContent?.includes("inner.ts"),
        ),
      ).toBeUndefined();
    }, { timeout: 3000 });
    // 目录打开不产生编辑器面板
    expect(mocks.mockAddPanel).not.toHaveBeenCalled();

    // 嵌套文件 Enter：重新展开 src → 选中 inner.ts → findNode 递归下钻命中（136-141）
    fireEvent.click(
      Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).find(
        (r) => r.textContent?.includes("src"),
      )!,
    );
    await waitFor(() => {
      expect(
        Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).some(
          (r) => r.textContent?.includes("inner.ts"),
        ),
      ).toBe(true);
    }, { timeout: 3000 });
    fireEvent.click(
      Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).find(
        (r) => r.textContent?.includes("inner.ts"),
      )!,
    );
    window.dispatchEvent(makeKeydown({ code: "Enter" }));

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });
    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.params.filePath).toBe("C:/test-project/src/inner.ts");
  });

  it("选中路径已不在树中（重命名后选中残留旧路径）→ findNode 未命中返回 null，仍按原路径打开", async () => {
    const oldEntry = { name: "old.ts", path: "C:/test-project/old.ts", isDir: false, size: 32, modified: 1 };
    const newEntry = { name: "renamed.ts", path: "C:/test-project/renamed.ts", isDir: false, size: 32, modified: 1 };
    mocks.mockReadDir
      .mockResolvedValueOnce([oldEntry]) // 初始加载
      .mockResolvedValueOnce([newEntry]); // 重命名后 refresh

    seedProject();
    const rowSpan = await renderAndFindRow("old.ts");
    fireEvent.click(rowSpan); // 选中 old.ts（selectedPath = 旧路径）
    focusExplorer();

    // F2 重命名成功 → 树刷新：old.ts 行消失、renamed.ts 行出现，选中仍残留旧路径
    window.dispatchEvent(makeKeydown({ code: "F2" }));
    const input = await waitFor(() => {
      const el = document.querySelector('[data-testid="explorer-inline-input"]');
      expect(el).toBeTruthy();
      return el;
    }, { timeout: 3000 });
    fireEvent.change(input as Element, { target: { value: "renamed.ts" } });
    fireEvent.keyDown(input as Element, { key: "Enter" });

    await waitFor(() => {
      expect(
        Array.from(document.querySelectorAll('[data-testid="tree-node-row"]')).some(
          (r) => r.textContent?.includes("renamed.ts"),
        ),
      ).toBe(true);
    }, { timeout: 3000 });

    // Enter → findNode 全树未命中（旧路径已不在树中）→ 防御分支 return null → 仍按选中路径打开
    window.dispatchEvent(makeKeydown({ code: "Enter" }));

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });
    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.params.filePath).toBe("C:/test-project/old.ts");
  });
});

// =====================================================================
// 键盘 F2：handleRenameSelected + handleRenameCancel
// =====================================================================

describe("键盘 F2 动作链路（handleRenameSelected / handleRenameCancel）", () => {
  it("F2 → 重命名 input 预填 basename；Escape → 取消（input 消失）", async () => {
    const fileEntry = { name: "app.ts", path: "C:/test-project/app.ts", isDir: false, size: 200, modified: 1 };
    mocks.mockReadDir.mockResolvedValue([fileEntry]);

    seedProject();
    const rowSpan = await renderAndFindRow("app.ts");
    fireEvent.click(rowSpan);
    focusExplorer();

    // 真实面板 explorerActions.getSelectedPath（ref 模式包装，167）——焦点链路建立后可用
    expect(getActiveExplorer()?.getSelectedPath()).toBe("C:/test-project/app.ts");

    window.dispatchEvent(makeKeydown({ code: "F2" }));

    // 用户可见：内联重命名 input 出现，预填 basename
    // （dispatchEvent 不经 act——React 异步 flush，waitFor 须抛错式断言轮询）
    const input = await waitFor(() => {
      const el = document.querySelector('[data-testid="explorer-inline-input"]');
      expect(el).toBeTruthy();
      return el;
    }, { timeout: 3000 });
    expect((input as HTMLInputElement).value).toBe("app.ts");

    // Escape → handleRenameCancel → input 消失
    fireEvent.keyDown(input as Element, { key: "Escape" });
    expect(document.querySelector('[data-testid="explorer-inline-input"]')).toBeNull();
  });
});
