// commit-view-status.test.ts — useCommitStatus 数据 hook L2 测试
//
// 覆盖：fs-event 200ms 去抖刷新 / 连续触发合并 / 去抖 timer unmount 清理 /
// rootPath 切换清空重载 / 旧请求 gen 丢弃 / refresh 立即重载 / rootPath 为 null 无操作。
// 拆分自原 commit-view.test.tsx（SVC-14）。
// 状态机 4 态 → commit-view.test.tsx；列表渲染/折叠交互 → commit-view-list.test.tsx。

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
import { render, cleanup, waitFor, renderHook, act } from "@testing-library/react";
import { CommitView } from "../features/commit/CommitView";
import { useCommitStatus } from "../features/commit/useCommitStatus";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import type { GitStatusEntry } from "../types/git";

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

  it("连续 fs-event 200ms 去抖合并为 1 次 gitStatus", async () => {
    vi.useFakeTimers();
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([makeEntry("C:/repo/a.ts", "modified")]);
    render(React.createElement(CommitView));
    await vi.advanceTimersByTimeAsync(10);
    mockGitStatus.mockClear();

    // 200ms 内连续触发 3 次 → timer 逐次重置，只保留最后一个
    mockOnFsEventCallback.trigger();
    mockOnFsEventCallback.trigger();
    mockOnFsEventCallback.trigger();

    // 100ms 未到 200ms 阈值 → 不刷新
    await vi.advanceTimersByTimeAsync(100);
    expect(mockGitStatus).not.toHaveBeenCalled();

    // 跨过阈值 → 仅 1 次 gitStatus（去抖合并）
    await vi.advanceTimersByTimeAsync(150);
    expect(mockGitStatus).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });

  it("去抖 timer 激活后 unmount 清理（clearTimeout）", async () => {
    vi.useFakeTimers();
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([]);
    const { unmount } = render(React.createElement(CommitView));
    await vi.advanceTimersByTimeAsync(10);
    mockGitStatus.mockClear();

    // 触发 fs-event 激活去抖 timer
    mockOnFsEventCallback.trigger();

    // timer 已激活：199ms 未到阈值仍不刷新
    await vi.advanceTimersByTimeAsync(199);
    expect(mockGitStatus).not.toHaveBeenCalled();

    // unmount → effect 清理 clearTimeout
    unmount();
    await vi.advanceTimersByTimeAsync(100);
    // timer 已清 → 不刷新
    expect(mockGitStatus).not.toHaveBeenCalled();

    vi.useRealTimers();
  });
});

describe("useCommitStatus rootPath 切换", () => {
  it("rootPath 切换清空旧数据并重载", async () => {
    vi.useFakeTimers();
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
    await vi.advanceTimersByTimeAsync(10);
    expect(mockGitStatus).toHaveBeenCalledWith("C:/repo1");

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
    await vi.advanceTimersByTimeAsync(10);
    expect(mockGitStatus).toHaveBeenCalledWith("C:/repo2");

    vi.useRealTimers();
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
    let resolveOld!: (entries: GitStatusEntry[]) => void;
    const oldPromise = new Promise<GitStatusEntry[]>((r) => { resolveOld = r; });
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
    resolveOld([makeEntry("C:/repo1/old.ts", "modified")]);

    // 轮询确认渲染稳定后旧数据仍不落地（单次 10ms 推进抓不住闪屏——TQ-B-06）
    await vi.waitFor(() => {
      const view = document.querySelector('[data-e2e="commit-view"]');
      expect(view?.textContent).toContain("x.ts");
    }, { timeout: 3000 });

    // 稳定期复查：状态应为 repo2 的数据（旧结果被 gen 检查丢弃）
    const commitView = document.querySelector('[data-e2e="commit-view"]');
    // 不应包含旧数据
    expect(commitView?.textContent).not.toContain("old.ts");

    vi.useRealTimers();
  });
});

describe("useCommitStatus refresh", () => {
  it("refresh() 立即触发 gitStatus 重载", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([]);

    const { result } = renderHook(() => useCommitStatus());
    await waitFor(() =>
      expect(result.current.state.kind).toBe("ready"),
    );

    mockGitStatus.mockClear();
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/new.ts", "modified"),
    ]);

    act(() => {
      result.current.refresh();
    });

    await waitFor(() =>
      expect(mockGitStatus).toHaveBeenCalledTimes(1),
    );
  });

  it("rootPath 为 null 时 refresh 无操作", () => {
    resetStores();
    const { result } = renderHook(() => useCommitStatus());

    act(() => {
      result.current.refresh();
    });

    expect(mockGitStatus).not.toHaveBeenCalled();
  });
});
