// commit-view.test.tsx — Commit 视图 L2 测试
//
// 覆盖：状态机 4 态 / 列表渲染 / 折叠交互 / 双击分派 /
// 去重聚焦 / 普通编辑器与 git 页签共存 / fs-event 刷新 / rootPath 切换。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockGitStatus,
  mockOnFsEventCallback,
} = vi.hoisted(() => {
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
import { openCommitFile, STATUS_PANEL_MAP, getPanelDispatch } from "../features/commit/openCommitFile";
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
  resetStores();
  // 默认 mock dockApi
  window.__dockviewApi = {
    addPanel: vi.fn(),
    getPanel: vi.fn(() => null),
  } as unknown as typeof window.__dockviewApi;
});

afterEach(() => {
  cleanup();
});

describe("CommitView 状态机", () => {
  it("无 rootPath 时显示「选择一个项目以查看变更」", () => {
    // 不种子 stores → rootPath 为 null
    const { container } = render(React.createElement(CommitView));
    const hint = container.querySelector('[data-e2e="commit-view"]');
    expect(hint).toBeTruthy();
    expect(hint!.textContent).toContain("选择一个项目以查看变更");
  });

  it("loading 态显示「加载中…」", () => {
    seedProject("C:/repo");
    // gitStatus 保持 pending
    mockGitStatus.mockReturnValue(new Promise(() => {}));
    const { container } = render(React.createElement(CommitView));
    const hint = container.querySelector('[data-e2e="commit-view"]');
    expect(hint).toBeTruthy();
    expect(hint!.textContent).toContain("加载中…");
  });

  it("gitStatus 失败显示「当前项目并非 git 项目」", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockRejectedValue(new Error("not a git repo"));
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const hint = container.querySelector('[data-e2e="commit-view"]');
      expect(hint!.textContent).toContain("当前项目并非 git 项目");
    });
  });

  it("gitStatus 成功显示两列表", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/b.ts", "untracked"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const changes = container.querySelector('[data-e2e="commit-changes"]');
      expect(changes).toBeTruthy();
      const unversioned = container.querySelector('[data-e2e="commit-unversioned"]');
      expect(unversioned).toBeTruthy();
    });
  });
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

    // 验证关键颜色匹配 GIT_FILE_COLORS（jsdom 将 hex 转为 rgb 格式）
    expect(colors["mod.ts"]).toBe("rgb(104, 151, 187)");
    expect(colors["add.ts"]).toBe("rgb(98, 151, 85)");
    expect(colors["del.ts"]).toBe("rgb(108, 108, 108)");
    expect(colors["ren.ts"]).toBe("rgb(58, 132, 132)");
    expect(colors["conf.ts"]).toBe("rgb(213, 117, 108)");
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

  it("空列表显示「无变更文件」", async () => {
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
      // 折叠时 children 不可见——Reace 不渲染 display:none，而是通过条件渲染
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

describe("openCommitFile 双击分派", () => {
  it("modified 状态打开 diff 面板", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/a.ts", "modified");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
    expect(callArgs.title).toContain("(git diff)");
    expect(callArgs.params.filePath).toBe("C:/repo/src/a.ts");
    expect(callArgs.params.repoPath).toBe("C:/repo");
  });

  it("added 状态打开 editor 面板含 (git add)", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/b.ts", "added");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("editor");
    expect(callArgs.title).toContain("(git add)");
  });

  it("untracked 状态打开 editor 面板含 (git not add)", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/c.ts", "untracked");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("editor");
    expect(callArgs.title).toContain("(git not add)");
  });

  it("deleted 状态打开 gitshow 面板含 (git delete)", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/d.ts", "deleted");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("gitshow");
    expect(callArgs.title).toContain("(git delete)");
  });

  it("renamed 状态传 oldPath 到 params", () => {
    seedProject("C:/repo");
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/new.ts", "renamed", "C:/repo/src/old.ts");

    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
    expect(callArgs.params.oldPath).toBe("C:/repo/src/old.ts");
  });

  it("activePageId 缺失时不执行", () => {
    resetStores();
    // useLayout.activePageId 为 null
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/a.ts", "modified");

    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("dockApi 缺失时不执行", () => {
    seedProject("C:/repo");
    window.__dockviewApi = undefined as unknown as typeof window.__dockviewApi;

    // 不应抛异常
    expect(() => openCommitFile("C:/repo/a.ts", "modified")).not.toThrow();
  });
});

describe("openCommitFile 去重聚焦", () => {
  it("已有同文件同 suffix 面板时聚焦而不创建", () => {
    seedProject("C:/repo");
    titleManager.registerEditor("page-1", "diff-1", "C:/repo/src/a.ts", "(git diff)");

    const mockFocus = vi.fn();
    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => ({ focus: mockFocus })),
    } as unknown as typeof window.__dockviewApi;

    openCommitFile("C:/repo/src/a.ts", "modified");

    expect(mockFocus).toHaveBeenCalled();
    expect(mockAddPanel).not.toHaveBeenCalled();
  });

  it("无 suffix 的普通编辑器不被误匹配（B10）", () => {
    seedProject("C:/repo");
    // 注册一个普通编辑器（无 suffix）
    titleManager.registerEditor("page-1", "editor-1", "C:/repo/src/a.ts");

    const mockAddPanel = vi.fn();
    window.__dockviewApi = {
      addPanel: mockAddPanel,
      getPanel: vi.fn(() => null),
    } as unknown as typeof window.__dockviewApi;

    // 打开 git diff 面板（带 suffix）→ 不应匹配无 suffix 的普通编辑器
    openCommitFile("C:/repo/src/a.ts", "modified");

    // 应创建新面板
    expect(mockAddPanel).toHaveBeenCalledTimes(1);
    const callArgs = mockAddPanel.mock.calls[0][0];
    expect(callArgs.component).toBe("diff");
  });

  it("带 diff suffix 的 git 面板不被普通编辑器去重误匹配（B10 反向）", () => {
    seedProject("C:/repo");
    // 注册一个带 suffix 的 git diff 面板
    titleManager.registerEditor("page-1", "diff-1", "C:/repo/src/a.ts", "(git diff)");

    // 模拟普通编辑器去重（不传 suffix）
    const found = titleManager.findExistingEditor("page-1", "C:/repo/src/a.ts");
    // 不应匹配——普通编辑器 findExistingEditor 不传 suffix 时仅匹配无 suffix 条目
    expect(found).toBeNull();
  });
});

describe("useCommitStatus fs-event 刷新", () => {
  it("fs-event 触发后 200ms debounce 刷新 gitStatus", async () => {
    vi.useFakeTimers();
    seedProject("C:/repo");

    // 首次加载
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
    ]);
    render(React.createElement(CommitView));
    await vi.advanceTimersByTimeAsync(10);
    expect(mockGitStatus).toHaveBeenCalledTimes(1);

    // 触发 fs-event
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/b.ts", "untracked"),
    ]);
    mockOnFsEventCallback.trigger();

    // 200ms 内不应刷新
    await vi.advanceTimersByTimeAsync(100);
    expect(mockGitStatus).toHaveBeenCalledTimes(1);

    // 200ms 后应刷新
    await vi.advanceTimersByTimeAsync(150);
    expect(mockGitStatus).toHaveBeenCalledTimes(2);

    vi.useRealTimers();
  });
});

describe("useCommitStatus rootPath 切换", () => {
  it("rootPath 切换清空旧数据并重载", async () => {
    // 先种子项目 1
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "项目1",
          rootPath: "C:/repo1",
          pages: [{ pageId: "page-1", name: "p1", layout: {}, cwd: undefined, createdAt: 1, lastAccessedAt: 1 }],
          activePageId: "page-1",
          version: 1,
        },
      },
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: { "proj-1": true },
    });
    useLayout.setState({ activePageId: "page-1" });

    // 首次加载
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo1/a.ts", "modified"),
    ]);
    render(React.createElement(CommitView));
    await waitFor(() => {
      expect(mockGitStatus).toHaveBeenCalledWith("C:/repo1");
    });

    // 切换到项目 2
    useProjects.setState({
      projects: {
        "proj-2": {
          projectId: "proj-2",
          name: "项目2",
          rootPath: "C:/repo2",
          pages: [{ pageId: "page-2", name: "p2", layout: {}, cwd: undefined, createdAt: 2, lastAccessedAt: 2 }],
          activePageId: "page-2",
          version: 1,
        },
      },
    });
    useLayout.setState({ activePageId: "page-2" });

    // 等待重载
    await waitFor(() => {
      expect(mockGitStatus).toHaveBeenCalledWith("C:/repo2");
    });
  });

  it("rootPath 变化后旧请求结果不覆盖新状态", async () => {
    vi.useFakeTimers();

    // 种子项目 1
    useProjects.setState({
      projects: {
        "proj-1": {
          projectId: "proj-1",
          name: "项目1",
          rootPath: "C:/repo1",
          pages: [{ pageId: "page-1", name: "p1", layout: {}, cwd: undefined, createdAt: 1, lastAccessedAt: 1 }],
          activePageId: "page-1",
          version: 1,
        },
      },
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: { "proj-1": true },
    });
    useLayout.setState({ activePageId: "page-1" });

    // 首次调用保持 pending（旧请求不 resolve）
    let resolveOld: (v: typeof mockGitStatus extends (...args: infer _A) => infer R ? R : never) => void;
    const oldPromise = new Promise<ReturnType<typeof mockGitStatus>>((r) => { resolveOld = r; });
    mockGitStatus.mockReturnValueOnce(oldPromise);

    render(React.createElement(CommitView));
    await vi.advanceTimersByTimeAsync(10);

    // 切到项目 2，新请求
    useProjects.setState({
      projects: {
        "proj-2": {
          projectId: "proj-2",
          name: "项目2",
          rootPath: "C:/repo2",
          pages: [{ pageId: "page-2", name: "p2", layout: {}, cwd: undefined, createdAt: 2, lastAccessedAt: 2 }],
          activePageId: "page-2",
          version: 1,
        },
      },
    });
    useLayout.setState({ activePageId: "page-2" });

    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo2/x.ts", "modified"),
    ]);

    await vi.advanceTimersByTimeAsync(10);
    expect(mockGitStatus).toHaveBeenCalledTimes(2);
    expect(mockGitStatus).toHaveBeenLastCalledWith("C:/repo2");

    // 旧请求 resolve
    resolveOld!([makeEntry("C:/repo1/old.ts", "modified")] as ReturnType<typeof mockGitStatus>);
    await vi.advanceTimersByTimeAsync(10);

    // 状态应为 repo2 的数据（旧结果被 gen 检查丢弃）
    const commitView = document.querySelector('[data-e2e="commit-view"]');
    // 不应包含旧数据
    expect(commitView?.textContent).not.toContain("old.ts");

    vi.useRealTimers();
  });
});

describe("STATUS_PANEL_MAP 映射表", () => {
  it("所有契约状态均有映射", () => {
    expect(STATUS_PANEL_MAP.added.panelType).toBe("editor");
    expect(STATUS_PANEL_MAP.added.suffix).toBe("(git add)");
    expect(STATUS_PANEL_MAP.untracked.panelType).toBe("editor");
    expect(STATUS_PANEL_MAP.untracked.suffix).toBe("(git not add)");
    expect(STATUS_PANEL_MAP.deleted.panelType).toBe("gitshow");
    expect(STATUS_PANEL_MAP.deleted.suffix).toBe("(git delete)");
    expect(STATUS_PANEL_MAP.modified.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.modified.suffix).toBe("(git diff)");
    expect(STATUS_PANEL_MAP.renamed.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.renamed.suffix).toBe("(git diff)");
    expect(STATUS_PANEL_MAP.conflict.panelType).toBe("diff");
    expect(STATUS_PANEL_MAP.conflict.suffix).toBe("(git diff)");
  });

  it("未知状态返回 null", () => {
    expect(getPanelDispatch("ignored")).toBeNull();
    expect(getPanelDispatch("unknown")).toBeNull();
  });
});
