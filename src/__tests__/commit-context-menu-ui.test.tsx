// commit-context-menu-ui.test.tsx — CommitFileList 右键菜单 UI L2 测试
//
// 覆盖：右键菜单打开/无菜单项不弹/外点关闭/项点击执行 action、
// 菜单项 hover 高亮背景切换、危险项 ERROR_FG 着色、renamed oldPath 回退传递。
// 拆分自原 commit-view.test.tsx（SVC-14）。
// 菜单策略逻辑（confirmDialog → IPC → refresh）→ commit-context-menu.test.ts（纯逻辑层）。
// openCommitFile 分派逻辑 → commit-open-file.test.ts。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockGitStatus, mockOnFsEventCallback, mockGetContextMenuItems, mockOpenCommitFile } = vi.hoisted(() => {
  let onFsEventCb: (() => void) | null = null;
  return {
    mockGitStatus: vi.fn(),
    // 保存 onFsEvent 注册的回调引用，供测试手动触发 fs-event
    mockOnFsEventCallback: {
      set cb(fn: (() => void) | null) { onFsEventCb = fn; },
      get cb() { return onFsEventCb; },
      trigger() { onFsEventCb?.(); },
    },
    mockGetContextMenuItems: vi.fn(),
    // 双击分派 mock——本文件只验证 oldPath 参数传递，分派行为在 commit-open-file.test.ts
    mockOpenCommitFile: vi.fn(),
  };
});

// mock IPC git —— gitStatus 返回模拟数据
vi.mock("../ipc/git", () => ({
  gitStatus: mockGitStatus,
}));

// mock IPC notify —— onFsEvent 保存回调引用
vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn((cb: () => void) => {
    mockOnFsEventCallback.cb = cb;
    return () => { mockOnFsEventCallback.cb = null; };
  }),
}));

// mock commitContextMenu —— 菜单项由测试注入（策略逻辑在 commit-context-menu.test.ts）
vi.mock("../features/commit/commitContextMenu", () => ({
  getContextMenuItems: mockGetContextMenuItems,
}));

// mock openCommitFile —— 双击分派验证 oldPath 参数传递
vi.mock("../features/commit/openCommitFile", () => ({
  openCommitFile: mockOpenCommitFile,
}));

import React from "react";
import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";
import { CommitView } from "../features/commit/CommitView";
import { CommitFileList } from "../features/commit/CommitFileList";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ── 辅助函数：种子 stores ──
function seedProject(rootPath: string) {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: undefined,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { "proj-1": true },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStores() {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  titleManager.reset();
}

// ── 辅助：构造 GitStatusEntry ──
function makeEntry(
  path: string,
  status: string,
  oldPath: string | null = null,
) {
  return { path, status, oldPath };
}

beforeEach(() => {
  mockGitStatus.mockReset();
  mockOnFsEventCallback.cb = null;
  mockGetContextMenuItems.mockReset();
  mockGetContextMenuItems.mockReturnValue([]); // 默认无菜单项
  mockOpenCommitFile.mockReset();
  resetStores();
});

afterEach(() => {
  cleanup();
});

/** 渲染 CommitView 并等待文件项出现 */
async function renderReady(entries: ReturnType<typeof makeEntry>[]) {
  mockGitStatus.mockResolvedValue(entries);
  const { container } = render(React.createElement(CommitView));
  await waitFor(() => {
    const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
    expect(items.length).toBe(entries.length);
  });
  return { container };
}

/** 定位当前弹出的右键菜单（position: fixed 浮层） */
function getMenuEl(): HTMLDivElement {
  const menus = document.querySelectorAll('div[style*="position: fixed"]');
  return menus[menus.length - 1] as HTMLDivElement;
}

describe("CommitFileList 右键菜单", () => {
  it("右键文件触发 ContextMenu 显示", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", action: vi.fn() },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    // ContextMenu 应在 (100, 200) 显示
    const menuEl = getMenuEl();
    expect(menuEl.style.left).toBe("100px");
    expect(menuEl.style.top).toBe("200px");
    expect(menuEl.textContent).toContain("回滚");
  });

  it("无菜单项不弹 ContextMenu", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    // ContextMenu 不应出现
    const menus = document.querySelectorAll('div[style*="position: fixed"]');
    expect(menus.length).toBe(0);
  });

  it("菜单外点击关闭 ContextMenu", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", action: vi.fn() },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    // 菜单出现
    const menusBefore = document.querySelectorAll(
      'div[style*="position: fixed"]',
    );
    expect(menusBefore.length).toBeGreaterThan(0);

    // 点击 document 外部
    fireEvent.mouseDown(document.body);

    // 菜单消失
    await waitFor(() => {
      const menusAfter = document.querySelectorAll(
        'div[style*="position: fixed"]',
      );
      expect(menusAfter.length).toBe(0);
    });
  });

  it("点击菜单项执行 action + 关闭菜单", async () => {
    seedProject("C:/repo");
    const mockAction = vi.fn();
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", action: mockAction },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    // 点击菜单项
    const menuItem = document.querySelector(
      'div[style*="position: fixed"] div',
    )!;
    fireEvent.click(menuItem);

    expect(mockAction).toHaveBeenCalledTimes(1);

    // 菜单应关闭
    await waitFor(() => {
      const menusAfter = document.querySelectorAll(
        'div[style*="position: fixed"]',
      );
      expect(menusAfter.length).toBe(0);
    });
  });

  it("菜单项 hover 高亮背景、移出恢复透明", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", action: vi.fn() },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    const menuItem = document.querySelector(
      'div[style*="position: fixed"] div',
    ) as HTMLDivElement;
    // hover → SIDEBAR_COLORS.hover（#222227，UI-802；jsdom 转 rgb 形态）
    fireEvent.mouseEnter(menuItem);
    expect(menuItem.style.background).toBe("rgb(34, 34, 39)");
    // 移出 → 恢复透明
    fireEvent.mouseLeave(menuItem);
    expect(menuItem.style.background).toBe("transparent");
  });

  it("菜单项 28px 高 + 圆角 5（UI-802 项规格）", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", action: vi.fn() },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    const menuItem = document.querySelector(
      'div[style*="position: fixed"] div',
    ) as HTMLDivElement;
    expect(menuItem.style.height).toBe("28px");
    expect(menuItem.style.borderRadius).toBe("5px");
  });

  it("危险项 ERROR_FG 着色、普通项 SIDEBAR_FG（UI-802）", async () => {
    seedProject("C:/repo");
    mockGetContextMenuItems.mockReturnValue([
      { label: "回滚", danger: true, action: vi.fn() },
    ]);
    const { container } = await renderReady([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);

    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });

    const menuItem = document.querySelector(
      'div[style*="position: fixed"] div',
    ) as HTMLDivElement;
    // 危险项 → ERROR_FG（#d9706b，UI-802；jsdom 转 rgb 形态）
    expect(menuItem.style.color).toBe("rgb(217, 112, 107)");

    // 普通项 → SIDEBAR_FG（#ece9e4）
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });
    mockGetContextMenuItems.mockReturnValue([
      { label: "普通项", action: vi.fn() },
    ]);
    // 重新打开菜单（state 更新后再查当前菜单项）
    fireEvent.mouseDown(document.body);
    fireEvent.contextMenu(item, { clientX: 100, clientY: 200 });
    const normalItem = Array.from(
      document.querySelectorAll('div[style*="position: fixed"] div'),
    ).find((el) => el.textContent === "普通项") as HTMLDivElement;
    expect(normalItem.style.color).toBe("rgb(236, 233, 228)");
  });

  it("CommitFileList 独立渲染时 onRefresh 可用", () => {
    const { container } = render(
      React.createElement(CommitFileList, {
        title: "Test",
        entries: [makeEntry("C:/repo/a.ts", "modified")],
        rootPath: "C:/repo",
        e2eId: "test-list",
        onRefresh: vi.fn(),
      }),
    );

    const list = container.querySelector('[data-e2e="test-list"]');
    expect(list).toBeTruthy();
    expect(list!.textContent).toContain("Test (1)");
  });
});

describe("CommitFileList 双击 oldPath 回退", () => {
  it("renamed 无 oldPath 时双击传 oldPath 为 undefined", () => {
    render(
      React.createElement(CommitFileList, {
        title: "Changes",
        entries: [makeEntry("C:/repo/new.ts", "renamed", null)],
        rootPath: "C:/repo",
        e2eId: "changes",
        onRefresh: vi.fn(),
      }),
    );

    const item = document.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.doubleClick(item);

    // oldPath ?? undefined 回退：CommitFileList 传 undefined 给 openCommitFile
    expect(mockOpenCommitFile).toHaveBeenCalledWith(
      "C:/repo/new.ts",
      "renamed",
      undefined,
    );
  });

  it("renamed 有 oldPath 时双击原样传递", () => {
    render(
      React.createElement(CommitFileList, {
        title: "Changes",
        entries: [makeEntry("C:/repo/new.ts", "renamed", "C:/repo/old.ts")],
        rootPath: "C:/repo",
        e2eId: "changes",
        onRefresh: vi.fn(),
      }),
    );

    const item = document.querySelector('[data-e2e="commit-file-item"]')!;
    fireEvent.doubleClick(item);

    expect(mockOpenCommitFile).toHaveBeenCalledWith(
      "C:/repo/new.ts",
      "renamed",
      "C:/repo/old.ts",
    );
  });
});
