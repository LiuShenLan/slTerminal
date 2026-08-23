// explorer-race-cleanup.test.tsx — useFileTree 竞态与清理分支测试（EXP-07）
//
// 覆盖：
//   G1：旧 loadRoot 延迟 resolve + rootPath 切 null → gen 检查丢弃，不抛错
//   G2：reloadPreservingExpanded rootPathRef 为 null → 清空不抛错
//   G3：fs-event 去抖中卸载 → 定时器清理，不触发刷新
//   G4：slterm:file-saved 缺 path（detail 无 path）→ 仍刷新
//   G5：slterm:file-saved 卸载清理 → 卸载后分发不刷新
//   G6：gitStatus 旧请求延迟 resolve → gen 检查丢弃，保留新项目数据

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";

// ─── 共享 mock 工厂（setup.ts 注入 globalThis）───
import { makeVfs, mockEntry } from "./helpers/vfs";

const mocks = vi.hoisted(() => {
  const fs = __createFsMocks();
  const git = __createGitMocks();
  const notify = __createNotifyMocks();

  return {
    get mockReadDir() { return fs.readDir; },
    get mockGitStatus() { return git.gitStatus; },
    get mockOnFsEvent() { return notify.onFsEvent; },
    get triggerFsEvent() { return notify.triggerFsEvent; },
    resetAll() {
      fs.readDir.mockReset();
      git.gitStatus.mockReset();
      notify.onFsEvent.mockClear();
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
  startWatch: vi.fn().mockResolvedValue(undefined),
  onFsEvent: mocks.mockOnFsEvent,
}));

import { useFileTree } from "../features/explorer/useFileTree";
import type { DirEntry } from "../types/fs";

// ─── 辅助 ───

/** 手动控制 resolve 的 deferred promise */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  cleanup();
  mocks.resetAll();
  mocks.mockReadDir.mockResolvedValue([]);
  mocks.mockGitStatus.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
});

// =====================================================================
// G1：旧 loadRoot 延迟 resolve + rootPath 切 null
// =====================================================================

describe("useFileTree 竞态清理 — G1 loadRoot 过期回调", () => {
  it("G1: rootPath 切 null 后旧 loadRoot 回调 resolve → 丢弃不抛错、树保持空", async () => {
    const d = deferred<DirEntry[]>();
    mocks.mockReadDir.mockImplementation(() => d.promise);

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    // loadRoot("/proj-a") 挂起中（readDir pending），rootNodes 尚未填充
    expect(result.current.rootNodes).toEqual([]);

    // 切换到 null → 清空 + gen 递增
    rerender({ rootPath: null });
    expect(result.current.rootNodes).toEqual([]);

    // 旧回调此刻才 resolve：gen 已过期 → 结果被丢弃，不 setState、不抛错
    await act(async () => {
      d.resolve([mockEntry("stale.ts", false, "/proj-a/stale.ts")]);
    });

    expect(result.current.rootNodes).toEqual([]);
  });

  it("G2: refresh() 时 rootPathRef 为 null → 清空不抛错", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    // 切换到 null 后再调 refresh（reloadPreservingExpanded 的 null 分支）
    rerender({ rootPath: null });
    mocks.mockReadDir.mockClear();
    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.rootNodes).toEqual([]);
    // null 分支不发起任何 IPC（清空后无 readDir 调用）
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });
});

// =====================================================================
// G3：fs-event 去抖中卸载 → 定时器清理
// =====================================================================

describe("useFileTree 竞态清理 — G3 fs-event 去抖清理", () => {
  it("G3: fs-event 去抖窗口内卸载 → 定时器清理，不触发刷新", async () => {
    vi.useFakeTimers();
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result, unmount } = renderHook(() => useFileTree({ rootPath: "/proj" }));

    // 等初始加载真实完成（微任务链时长不定，0ms 推进假设过强——TQ-B-04）。
    // fake timers 下用 vi.waitFor（自动推进假定时器）
    await vi.waitFor(() => expect(result.current.rootNodes.length).toBe(1), { timeout: 3000 });

    // 清空计数，触发 fs-event → 200ms 去抖挂起
    mocks.mockReadDir.mockClear();
    mocks.mockGitStatus.mockClear();
    act(() => {
      mocks.triggerFsEvent();
    });
    // 去抖未到点：不刷新
    expect(mocks.mockReadDir).not.toHaveBeenCalled();

    // 卸载：cleanup 应清掉去抖定时器
    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });
});

// =====================================================================
// G4/G5：slterm:file-saved 事件缺 path / 卸载清理
// =====================================================================

describe("useFileTree 竞态清理 — slterm:file-saved", () => {
  it("G4: file-saved 事件 detail 无 path → 仍触发刷新（gitStatus 重调）", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(1);
    }, { timeout: 3000 });
    expect(mocks.mockGitStatus).toHaveBeenCalledTimes(1);

    // detail 存在但无 path → savedPath 为 undefined → 仍走 refreshExpanded
    window.dispatchEvent(new CustomEvent("slterm:file-saved", { detail: {} }));

    await waitFor(() => {
      expect(mocks.mockGitStatus).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });
  });

  it("G5: file-saved 监听卸载清理 → 卸载后分发不触发刷新", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result, unmount } = renderHook(() => useFileTree({ rootPath: "/proj" }));

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(1);
    }, { timeout: 3000 });
    expect(mocks.mockGitStatus).toHaveBeenCalledTimes(1);

    unmount();
    window.dispatchEvent(new CustomEvent("slterm:file-saved", { detail: {} }));

    // 冲刷微任务后 gitStatus 不应被再次调用（监听已移除）
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.mockGitStatus).toHaveBeenCalledTimes(1);
  });
});

// =====================================================================
// G6：gitStatus 旧请求延迟 resolve → gen 丢弃
// =====================================================================

describe("useFileTree 竞态清理 — G6 gitStatus 过期", () => {
  it("G6: 旧项目 gitStatus 延迟 resolve → 被丢弃，保留新项目数据", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
      "/proj-c": [mockEntry("c.ts", false, "/proj-c/c.ts")],
    });

    // 手动控制：/proj-a 的 gitStatus 延迟，/proj-c 即时返回
    const d = deferred<Array<{ path: string; status: string }>>();
    mocks.mockGitStatus.mockImplementation((path: string) => {
      if (path === "/proj-a") return d.promise;
      return Promise.resolve([{ path: "/proj-c/c.ts", status: "added" }]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(1);
    }, { timeout: 3000 });
    // /proj-a 的 gitStatus 挂起中 → map 仍空
    expect(result.current.gitStatusMap.size).toBe(0);

    // 切换到 /proj-c → 新 gitStatus 即时 resolve
    rerender({ rootPath: "/proj-c" });
    await waitFor(() => {
      expect(result.current.gitStatusMap.get("/proj-c/c.ts")).toBe("added");
    }, { timeout: 3000 });

    // /proj-a 的旧 gitStatus 此刻 resolve → gen 过期被丢弃
    await act(async () => {
      d.resolve([{ path: "/proj-a/a.ts", status: "modified" }]);
    });

    // map 仍为 /proj-c 的数据，不被旧项目污染
    expect(result.current.gitStatusMap.get("/proj-c/c.ts")).toBe("added");
    expect(result.current.gitStatusMap.has("/proj-a/a.ts")).toBe(false);
    expect(result.current.gitStatusMap.size).toBe(1);
  });
});
