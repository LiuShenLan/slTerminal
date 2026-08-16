// commit-view-list.test.tsx — CommitView 列表渲染与折叠交互 L2 测试
//
// 覆盖：文件名状态色 / 标题计数 / 空态 / 字母序排序 / 目录后缀 / 状态分组 / COMMIT 标题 / 折叠展开。
// 拆分自原 commit-view.test.tsx（SVC-14）。
// 状态机 4 态 → commit-view.test.tsx；useCommitStatus 计时 → commit-view-status.test.ts；
// openCommitFile 分派 → commit-open-file.test.ts；右键菜单 UI → commit-context-menu-ui.test.tsx。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockGitStatus, mockOnFsEventCallback } = vi.hoisted(() => {
  let onFsEventCb: (() => void) | null = null;
  return {
    mockGitStatus: vi.fn(),
    // 保存 onFsEvent 注册的回调引用，供测试手动触发 fs-event
    mockOnFsEventCallback: {
      set cb(fn: (() => void) | null) { onFsEventCb = fn; },
      get cb() { return onFsEventCb; },
      trigger() { onFsEventCb?.(); },
    },
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

import React from "react";
import { render, cleanup, fireEvent, waitFor } from "@testing-library/react";
import { CommitView } from "../features/commit/CommitView";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { DIM_FG } from "../theme";

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 去除内空白后比对） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  return hex;
}

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
  resetStores();
});

afterEach(() => {
  cleanup();
});

describe("CommitView 列表渲染", () => {
  it("文件名使用 GIT_FILE_COLORS 对应状态色", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/mod.ts", "modified"),
      makeEntry("C:/repo/add.ts", "added"),
      makeEntry("C:/repo/del.ts", "deleted"),
      makeEntry("C:/repo/ren.ts", "renamed", "C:/repo/old.ts"),
      makeEntry("C:/repo/conf.ts", "conflict"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
      expect(items.length).toBe(5);
    });

    const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
    const colors: Record<string, string> = {};
    items.forEach((el) => {
      const nameSpan = el.querySelector("span");
      if (nameSpan) {
        const text = nameSpan.textContent || "";
        const color = nameSpan.getAttribute("style")?.match(/color:\s*([^;]+)/)?.[1] || "";
        colors[text] = color;
      }
    });

    // 验证关键颜色匹配 GIT_FILE_COLORS（jsdom 将 hex 转为 rgb 格式；值 = 附录 A）
    expect(colors["mod.ts"]).toBe("rgb(214, 178, 94)");
    expect(colors["add.ts"]).toBe("rgb(134, 187, 122)");
    expect(colors["del.ts"]).toBe("rgb(217, 112, 107)");
    expect(colors["ren.ts"]).toBe("rgb(110, 159, 242)");
    expect(colors["conf.ts"]).toBe("rgb(217, 112, 107)");
  });

  it("标题栏显示计数", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/b.ts", "modified"),
      makeEntry("C:/repo/c.ts", "untracked"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const changes = container.querySelector('[data-e2e="commit-changes"]');
      expect(changes!.textContent).toContain("Changes (2)");
      const unversioned = container.querySelector('[data-e2e="commit-unversioned"]');
      expect(unversioned!.textContent).toContain("Unversioned Files (1)");
    });
  });

  it("空列表显示「无变更文件」（空态色 DIM_FG——人工验证问题 4）", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      // 两个列表都应展开并显示空态
      const emptyHints = container.querySelectorAll('[data-e2e="commit-view"]');
      expect(emptyHints.length).toBeGreaterThan(0);
    });
    // 检查无变更文件文本
    await waitFor(() => {
      expect(container.textContent).toContain("无变更文件");
    });
    // 空态文字色 = fg-3（DIM_FG）——原 INPUT_BORDER 边框 token 近乎不可见
    //（textContent 精确匹配可能命中祖先容器——取带内联 color 的提示 div 本身）
    const hintDiv = Array.from(container.querySelectorAll("div")).find(
      (d) => d.textContent === "无变更文件" && d.style.color !== "",
    );
    expect(hintDiv).toBeTruthy();
    expect((hintDiv as HTMLElement).style.color).toBe(hexToRgb(DIM_FG));
  });

  it("文件按完整相对路径字母序排序", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/z.ts", "modified"),
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/m.ts", "modified"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
      expect(items.length).toBe(3);
    });
    const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
    const names = Array.from(items).map(
      (el) => el.querySelector("span")?.textContent || "",
    );
    expect(names).toEqual(["a.ts", "m.ts", "z.ts"]);
  });

  it("灰色父目录相对路径后缀", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/src/sub/a.ts", "modified"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
      expect(items.length).toBe(1);
    });
    const item = container.querySelector('[data-e2e="commit-file-item"]')!;
    // 文件名 span 后应有灰色目录路径 span
    const spans = item.querySelectorAll("span");
    expect(spans.length).toBeGreaterThanOrEqual(2);
    const dirSpan = spans[1];
    expect(dirSpan.textContent).toBe("src/sub");
  });

  it("Changes 只含 added/modified/deleted/renamed/conflict", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/b.ts", "untracked"),
      makeEntry("C:/repo/c.ts", "added"),
      makeEntry("C:/repo/d.ts", "ignored"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const changes = container.querySelector('[data-e2e="commit-changes"]');
      // Changes 应只含 modified + added = 2 条
      expect(changes!.textContent).toContain("Changes (2)");
      const unversioned = container.querySelector('[data-e2e="commit-unversioned"]');
      expect(unversioned!.textContent).toContain("Unversioned Files (1)");
    });
  });

  it("标题栏 COMMIT 样式正确", () => {
    seedProject("C:/repo");
    mockGitStatus.mockReturnValue(new Promise(() => {}));
    const { container } = render(React.createElement(CommitView));
    const header = container.querySelector('[data-e2e="commit-view"] > div');
    expect(header).toBeTruthy();
    expect(header!.textContent).toBe("COMMIT");
  });
});

describe("CommitView 折叠交互", () => {
  it("点击标题折叠再展开", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);
    const { container } = render(React.createElement(CommitView));

    // 等待渲染完成
    await waitFor(() => {
      const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
      expect(items.length).toBe(1);
    });

    // 点击 Changes 标题折叠
    const changesHeader = container.querySelector('[data-e2e="commit-changes"] > div');
    expect(changesHeader).toBeTruthy();
    fireEvent.click(changesHeader!);

    // 折叠后文件项应消失（但空态"无变更文件"只在 entries.length===0 时显示，这里 entries 有内容，折叠后是 display:none）
    // 对于有内容的列表，折叠后 items 的父 div display:none
    await waitFor(() => {
      // 折叠后子 div 不可见
      const changesContainer = container.querySelector('[data-e2e="commit-changes"]');
      // 第一个子 div 是标题栏，第二个是内容 div
      const contentDiv = changesContainer!.querySelectorAll(":scope > div")[1];
      // 折叠时 children 不可见——React 不渲染 display:none，而是通过条件渲染
      // 实际上代码是 {!collapsed && <div>...}，所以折叠后内容 div 直接不存在
      expect(contentDiv).toBeFalsy();
    });

    // 再次点击展开
    fireEvent.click(changesHeader!);
    await waitFor(() => {
      const items = container.querySelectorAll('[data-e2e="commit-file-item"]');
      expect(items.length).toBe(1);
    });
  });

  it("展开时空态显示「无变更文件」", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      expect(container.textContent).toContain("无变更文件");
    });
  });
});
