// useFileTree.test.ts — useFileTree hook 单元测试
//
// F1: 消除重复 IPC — 验证 rootPath 变化时 loadRoot 只调用一次
// F3: generation 取消机制 — 验证旧异步请求结果被丢弃

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { makeVfs, mockEntry } from "./helpers/vfs";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  type DirEntry = { name: string; path: string; isDir: boolean; size: number | null; modified: number | null };
  type GitStatusEntry = { path: string; status: string };

  type ReadDirPage = { entries: DirEntry[]; nextCursor: string | null };

  let mockReadDirImpl: (path: string) => Promise<DirEntry[]> = () => Promise.resolve([]);
  let mockGitStatusImpl: (path: string) => Promise<GitStatusEntry[]> = () => Promise.resolve([]);
  // CP-016：捕获 onFsEvent 处理器——测试可手动触发 fs-event（200ms 去抖 → refreshExpanded）
  let fsEventHandler: (() => void) | null = null;

  // CP-006：mock 兑现分页契约——便利接口（整表 DirEntry[]）结果作为末页单页返回
  const mockReadDir = vi
    .fn<(path: string, cursor?: string | null) => Promise<ReadDirPage>>()
    .mockImplementation(async (path) => {
      const entries = await mockReadDirImpl(path);
      return { entries, nextCursor: null };
    });
  const mockGitStatus = vi.fn<(path: string) => Promise<GitStatusEntry[]>>().mockImplementation((path) => mockGitStatusImpl(path));

  const makeEntry = (name: string, isDir = false): DirEntry => ({
    name,
    path: `C:/project/${name}`,
    isDir,
    size: isDir ? null : 100,
    modified: isDir ? null : 1234567890,
  });

  /** onFsEvent 注册处理器捕获（CP-016 场景④手动触发） */
  const mockOnFsEvent = vi.fn((handler: () => void) => {
    fsEventHandler = handler;
    return () => {};
  });

  return {
    mockReadDir,
    mockGitStatus,
    mockOnFsEvent,
    makeEntry,
    /** 手动触发一次 fs-event（hooks 挂载时注册的处理器） */
    fireFsEvent() {
      fsEventHandler?.();
    },
    resetAll() {
      fsEventHandler = null;
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
      mockReadDirImpl = () => Promise.resolve([
        makeEntry("a.ts"), makeEntry("b.ts"), makeEntry("src", true),
      ]);
      mockGitStatusImpl = () => Promise.resolve([
        { path: "C:/project/a.ts", status: "modified" },
      ]);
      mockReadDir.mockImplementation(async (path) => {
        const entries = await mockReadDirImpl(path);
        return { entries, nextCursor: null };
      });
      mockGitStatus.mockImplementation((path) => mockGitStatusImpl(path));
    },
    setReadDirImpl(fn: (path: string) => Promise<DirEntry[]>) {
      mockReadDirImpl = fn;
      mockReadDir.mockImplementation(async (path) => {
        const entries = await mockReadDirImpl(path);
        return { entries, nextCursor: null };
      });
    },
    setGitStatusImpl(fn: (path: string) => Promise<GitStatusEntry[]>) {
      mockGitStatusImpl = fn;
      mockGitStatus.mockImplementation((path) => mockGitStatusImpl(path));
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readDirPage: mocks.mockReadDir,
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

// 真实模块导入（mock 之后）
import { useFileTree } from "../features/explorer/useFileTree";
import type { FileTreeViewState } from "../features/explorer/useFileTree";

// ============================================================
// F1: 消除重复 IPC 调用
// ============================================================
describe("useFileTree — F1 消除重复 IPC", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("F1-1: rootPath 变化时 loadRoot 只调用一次 readDir", async () => {
    const { rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 首次设置 rootPath
    rerender({ rootPath: "C:/project-a" });

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // 切换到另一个 rootPath（应只再调一次 readDir）
    mocks.setReadDirImpl(() => Promise.resolve([
      mocks.makeEntry("x.ts"), mocks.makeEntry("y.ts"),
    ]));
    rerender({ rootPath: "C:/project-b" });

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });
  });

  it("F1-2: rootPath 从 null 变为有效路径时正确加载", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    expect(result.current.rootNodes).toEqual([]);

    rerender({ rootPath: "C:/project" });

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    }, { timeout: 3000 });
    expect(result.current.rootNodes[0].entry.name).toBe("a.ts");
  });

  it("F1-3: rootPath 从有效路径变为 null 时清空", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    rerender({ rootPath: null });

    await waitFor(() => {
      expect(result.current.rootNodes).toEqual([]);
    }, { timeout: 3000 });
  });

  it("F1-4: 两个不同 rootPath 切换时数据正确", async () => {
    mocks.setReadDirImpl((path) => {
      if (path === "C:/project-a") {
        return Promise.resolve([mocks.makeEntry("alpha.ts")]);
      }
      return Promise.resolve([mocks.makeEntry("beta.ts")]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project-a" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("alpha.ts");
    }, { timeout: 3000 });

    rerender({ rootPath: "C:/project-b" });

    await waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("beta.ts");
    }, { timeout: 3000 });
  });

  it("F1-5: 重命名文件后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // 手动调用 refresh（模拟 CRUD 操作后刷新）
    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });

  it("F1-6: 删除文件后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    const callCountBefore = mocks.mockReadDir.mock.calls.length;

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(callCountBefore + 1);
  });

  it("F1-7: 新建文件后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      () =>
        useFileTree({
          rootPath: "C:/project",
          viewState: undefined,
          onViewStateChange: undefined,
        }),
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });

  it("F1-8: 新建文件夹后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      () =>
        useFileTree({
          rootPath: "C:/project",
          viewState: undefined,
          onViewStateChange: undefined,
        }),
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });
});

// ============================================================
// CP-006: loadRoot 首帧 + 游标续页拼接
// ============================================================
describe("useFileTree — CP-006 首帧续页拼接", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("P1: 首页超限时首帧渲染首页，随后按游标续页拼接为全量", async () => {
    // 仿后端分页：首帧 500 条 + nextCursor，续页 1 条即末页。
    // 续页门闩延迟 resolve（两阶段）——若两页即时返回，后台续页会在 waitFor
    // 首帧轮询前拼入（瞬态 500 不可观察），断言失去意义
    let releasePage2: (() => void) | undefined;
    const page2Gate = new Promise<void>((resolve) => {
      releasePage2 = resolve;
    });
    mocks.mockReadDir.mockImplementation(
      async (_path: string, cursor?: string | null) => {
        if (cursor) {
          await page2Gate; // 挂起至首帧 500 断言完成，再由 act 放行
          return { entries: [mocks.makeEntry("z-last.ts")], nextCursor: null };
        }
        const entries = Array.from({ length: 500 }, (_, i) =>
          mocks.makeEntry(`f${i}.ts`),
        );
        return { entries, nextCursor: "c1" };
      },
    );

    const { result } = renderHook(() =>
      useFileTree({
        rootPath: "C:/project",
        viewState: undefined,
        onViewStateChange: undefined,
      }),
    );

    // 首帧 500 条先落地（不等续页——超大目录首屏不阻塞）
    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(500);
    }, { timeout: 3000 });

    // 续页请求已在首帧后即时发起（mock 挂起等门闩）——调用序列契约先行断言：
    // 首帧仅 path（cursor 省略）；续页带上一页返回的游标
    expect(mocks.mockReadDir).toHaveBeenNthCalledWith(1, "C:/project");
    expect(mocks.mockReadDir).toHaveBeenNthCalledWith(2, "C:/project", "c1");

    // 放行续页：拼接后 = 501 条（首页 + 末页），末位 = 续页条目
    await act(async () => {
      releasePage2?.();
    });
    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(501);
    }, { timeout: 3000 });
    expect(result.current.rootNodes[500].entry.name).toBe("z-last.ts");
  });

  it("P2: 首页即末页（nextCursor null）→ 不发起续页调用", async () => {
    const { result } = renderHook(() =>
      useFileTree({
        rootPath: "C:/project",
        viewState: undefined,
        onViewStateChange: undefined,
      }),
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    }, { timeout: 3000 });
    // resetAll 默认 mock：整表一页返回——仅 1 次 readDirPage 调用（无续页）
    expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
  });
});

// ============================================================
// F3: Generation 取消机制
// ============================================================
describe("useFileTree — F3 Generation 取消机制", () => {
  beforeEach(() => {
    mocks.resetAll();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("F3-1: 快速切换 rootPath 两次，只有最后一次生效", async () => {
    // 让第一次 readDir 延迟 200ms 返回
    mocks.setReadDirImpl((path) => {
      if (path === "C:/project-a") {
        return new Promise((resolve) => {
          setTimeout(() => resolve([mocks.makeEntry("old.ts")]), 200);
        });
      }
      return Promise.resolve([mocks.makeEntry("new.ts")]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 先切到 project-a（慢），立即切到 project-b（快）
    rerender({ rootPath: "C:/project-a" });
    rerender({ rootPath: "C:/project-b" });

    // vi.waitFor 与 fake timers 兼容，自动推进时间轮询
    await vi.waitFor(() => {
      expect(result.current.rootNodes.length).toBeGreaterThan(0);
    }, { timeout: 3000 });

    expect(result.current.rootNodes[0].entry.name).toBe("new.ts");

    // 推进时间触发慢请求的 setTimeout(200ms)
    await vi.advanceTimersByTimeAsync(300);
    // generation 检查生效，仍是快结果
    expect(result.current.rootNodes[0].entry.name).toBe("new.ts");
  });

  it("F3-2: 旧请求的 setRootNodes 不覆盖新请求（慢先快后）", async () => {
    mocks.setReadDirImpl((path) => {
      if (path === "C:/project-slow") {
        return new Promise((resolve) => {
          setTimeout(() => resolve([mocks.makeEntry("slow-result.ts")]), 200);
        });
      }
      return Promise.resolve([mocks.makeEntry("fast-result.ts")]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    rerender({ rootPath: "C:/project-slow" });
    rerender({ rootPath: "C:/project-fast" });

    await vi.waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("fast-result.ts");
    }, { timeout: 3000 });

    // 推进时间触发慢请求
    await vi.advanceTimersByTimeAsync(300);
    expect(result.current.rootNodes[0]?.entry.name).toBe("fast-result.ts");
  });

  it("F3-3: 旧请求的 setGitStatusMap 不覆盖新请求", async () => {
    mocks.setGitStatusImpl((path) => {
      if (path === "C:/project-slow") {
        return new Promise((resolve) => {
          setTimeout(() => resolve([{ path: "C:/project-slow/a.ts", status: "modified" }]), 200);
        });
      }
      return Promise.resolve([{ path: "C:/project-fast/b.ts", status: "added" }]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    rerender({ rootPath: "C:/project-slow" });
    rerender({ rootPath: "C:/project-fast" });

    await vi.waitFor(() => {
      const map = result.current.gitStatusMap;
      expect(map.get("C:/project-fast/b.ts")).toBe("added");
    }, { timeout: 3000 });

    // 推进时间触发慢请求
    await vi.advanceTimersByTimeAsync(300);
    const map = result.current.gitStatusMap;
    expect(map.get("C:/project-slow/a.ts")).toBeUndefined();
  });

  it("F3-4: 同 rootPath 不触发取消（正常流程）", async () => {
    const { result } = renderHook(
      () =>
        useFileTree({
          rootPath: "C:/project",
          viewState: undefined,
          onViewStateChange: undefined,
        }),
    );

    await vi.waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledWith("C:/project");
    }, { timeout: 3000 });

    await vi.waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
      expect(result.current.rootNodes[0].entry.name).toBe("a.ts");
    }, { timeout: 3000 });

    // gitStatus 也应正常返回
    await vi.waitFor(() => {
      expect(result.current.gitStatusMap.get("C:/project/a.ts")).toBe("modified");
    }, { timeout: 3000 });
  });

  it("F3-5: generation 在多次切换后正确递增", async () => {
    const { rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 切换 5 次
    for (let i = 0; i < 5; i++) {
      rerender({ rootPath: `C:/project-${i}` });
    }

    // 验证 readDir 被调用了 5 次（每次切换都触发 loadRoot）
    await vi.waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(5);
    }, { timeout: 3000 });

    // 最后一次切换的数据应生效
    expect(mocks.mockReadDir).toHaveBeenLastCalledWith("C:/project-4");
  });

  it("F3-6: loadRoot 返回后 generation 匹配，数据正常更新", async () => {
    const { result } = renderHook(
      () =>
        useFileTree({
          rootPath: "C:/project",
          viewState: undefined,
          onViewStateChange: undefined,
        }),
    );

    await vi.waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    }, { timeout: 3000 });

    expect(result.current.rootNodes[0].entry.name).toBe("a.ts");
    expect(result.current.rootNodes[1].entry.name).toBe("b.ts");
    expect(result.current.rootNodes[2].entry.name).toBe("src");
  });

  it("F3-7: gitStatus 的 .then 回调中 generation 检查有效", async () => {
    // project-a 的 gitStatus 延迟返回，project-b 立即返回
    mocks.setGitStatusImpl((path) => {
      if (path === "C:/project-a") {
        return new Promise((resolve) => {
          setTimeout(() => resolve([{ path: "C:/project-a/x.ts", status: "modified" }]), 200);
        });
      }
      return Promise.resolve([]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project-a" as string | null } },
    );

    // 立即切换到 project-b（project-a 的 gitStatus 尚在等待中）
    rerender({ rootPath: "C:/project-b" });

    await vi.waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    }, { timeout: 3000 });

    // 推进时间触发慢 gitStatus
    await vi.advanceTimersByTimeAsync(300);
    expect(result.current.gitStatusMap.get("C:/project-a/x.ts")).toBeUndefined();
  });

  it("F3-8: rootPath 切换后，旧代际的续页结果被丢弃（CP-006 续页 gen 校验）", async () => {
    // project-a：首帧带游标、续页延迟 200ms；project-b：立即末页
    mocks.mockReadDir.mockImplementation(
      async (path: string, cursor?: string | null) => {
        if (path === "C:/project-b") {
          return { entries: [mocks.makeEntry("b.ts")], nextCursor: null };
        }
        if (!cursor) {
          return { entries: [mocks.makeEntry("a1.ts")], nextCursor: "c1" };
        }
        return new Promise((resolve) =>
          setTimeout(
            () =>
              resolve({
                entries: [mocks.makeEntry("a2.ts")],
                nextCursor: null,
              }),
            200,
          ),
        );
      },
    );

    const { result, rerender } = renderHook(
      ({ rootPath }) =>
        useFileTree({
          rootPath: rootPath as string | null,
          viewState: undefined,
          onViewStateChange: undefined,
        }),
      { initialProps: { rootPath: "C:/project-a" as string | null } },
    );

    // 先让 a 的首帧渲染，随后立即切到 b（a 的续页尚在等待）
    await vi.waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("a1.ts");
    }, { timeout: 3000 });
    rerender({ rootPath: "C:/project-b" });

    await vi.waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("b.ts");
    }, { timeout: 3000 });

    // 推进时间触发旧代际续页返回 → gen 校验丢弃，不污染新树
    await vi.advanceTimersByTimeAsync(300);
    expect(result.current.rootNodes.length).toBe(1);
    expect(result.current.rootNodes[0].entry.name).toBe("b.ts");
  });
});

// ============================================================
// FE-41: refreshSubtreeAt 目标已删空目录行移除
// ============================================================
describe("useFileTree — FE-41 目标已删目录行移除", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("FE-41-1: 已展开目录被删除后 file-saved 子树刷新 → 该目录行从树中移除", async () => {
    // vfs：根含 a.ts + src 目录，src 已展开（含 b.ts + sub 子目录）
    const vfs = makeVfs(mocks.mockReadDir, {
      "C:/project": [
        mockEntry("a.ts", false, "C:/project/a.ts"),
        mockEntry("src", true, "C:/project/src"),
      ],
      "C:/project/src": [
        mockEntry("b.ts", false, "C:/project/src/b.ts"),
        mockEntry("sub", true, "C:/project/src/sub"),
      ],
      "C:/project/src/sub": [],
    });

    const { result } = renderHook(
      () =>
        useFileTree({
          rootPath: "C:/project",
          viewState: undefined,
          onViewStateChange: undefined,
        }),
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(2);
    }, { timeout: 3000 });

    // 展开 src，加载其子项
    await act(async () => {
      await result.current.toggleExpand("C:/project/src");
    });
    await waitFor(() => {
      const src = result.current.rootNodes.find(
        (n) => n.entry.path === "C:/project/src",
      );
      expect(src?.expanded).toBe(true);
      expect(src?.children.length).toBe(2);
    }, { timeout: 3000 });

    // 磁盘上删除 src 目录（vfs.delete 后 readDir 抛 ENOENT），
    // 再模拟其内部文件保存事件触发子树刷新（300ms 去抖）
    vfs.delete("C:/project/src");
    window.dispatchEvent(
      new CustomEvent("slterm:file-saved", {
        detail: { path: "C:/project/src/b.ts" },
      }),
    );

    // 刷新目标（src）readDir 抛错 → 判定「目标已删除」→ 从父层移除该目录行
    await waitFor(() => {
      expect(
        result.current.rootNodes.find((n) => n.entry.path === "C:/project/src"),
      ).toBeUndefined();
    }, { timeout: 3000 });
    expect(result.current.rootNodes.map((n) => n.entry.path)).toEqual([
      "C:/project/a.ts",
    ]);
  });
});

// ============================================================
// CP-016: 展开态真值源外移——注册表状态槽恢复/提交
// ============================================================
describe("useFileTree — CP-016 恢复与提交", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  /** 最近一次上呼 payload（onViewStateChange 末次调用参数；lib=ES2020 无 Array.at） */
  const lastPayload = (spy: ReturnType<typeof vi.fn>) => {
    const calls = spy.mock.calls;
    const last = calls[calls.length - 1];
    return last?.[0] as FileTreeViewState | undefined;
  };

  it("CP-016-1: 匹配 rootPath 的 expandedPaths 恢复展开且子节点已加载", async () => {
    // vfs：根含 a.ts + src 目录（src 含 b.ts）——恢复需 src 子项加载
    makeVfs(mocks.mockReadDir, {
      "C:/project": [
        mockEntry("a.ts", false, "C:/project/a.ts"),
        mockEntry("src", true, "C:/project/src"),
      ],
      "C:/project/src": [mockEntry("b.ts", false, "C:/project/src/b.ts")],
    });
    const onViewStateChange = vi.fn();

    const { result } = renderHook(() =>
      useFileTree({
        rootPath: "C:/project",
        viewState: {
          rootPath: "C:/project",
          expandedPaths: ["C:/project/src"],
        },
        onViewStateChange,
      }),
    );

    // 初始加载完成后恢复展开（含异步加载子节点 b.ts）
    await waitFor(() => {
      const src = result.current.rootNodes.find(
        (n) => n.entry.path === "C:/project/src",
      );
      expect(src?.expanded).toBe(true);
    }, { timeout: 3000 });
    await waitFor(() => {
      const src = result.current.rootNodes.find(
        (n) => n.entry.path === "C:/project/src",
      );
      expect(src?.children.map((c) => c.entry.name)).toContain("b.ts");
    }, { timeout: 3000 });
    // 恢复完成的提交集含该路径（渲染落定后的最新提交）
    await waitFor(() => {
      expect(lastPayload(onViewStateChange)?.expandedPaths).toContain(
        "C:/project/src",
      );
    }, { timeout: 3000 });
  });

  it("CP-016-2: viewState.rootPath 与当前不符 → 不恢复（域键作废）", async () => {
    makeVfs(mocks.mockReadDir, {
      "C:/project": [
        mockEntry("a.ts", false, "C:/project/a.ts"),
        mockEntry("src", true, "C:/project/src"),
      ],
      "C:/project/src": [mockEntry("b.ts", false, "C:/project/src/b.ts")],
    });
    const onViewStateChange = vi.fn();

    const { result } = renderHook(() =>
      useFileTree({
        rootPath: "C:/project",
        viewState: {
          rootPath: "C:/other-project", // 域键不符——整份作废不恢复
          expandedPaths: ["C:/project/src"],
        },
        onViewStateChange,
      }),
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(2);
    }, { timeout: 3000 });

    const src = result.current.rootNodes.find(
      (n) => n.entry.path === "C:/project/src",
    );
    expect(src?.expanded).toBe(false);
    expect(src?.children.length).toBe(0);
    // 提交集不包含失效恢复路径
    expect(lastPayload(onViewStateChange)?.expandedPaths).not.toContain(
      "C:/project/src",
    );
  });

  it("CP-016-3: toggleExpand 后上呼含新展开路径，折叠后移除", async () => {
    makeVfs(mocks.mockReadDir, {
      "C:/project": [
        mockEntry("a.ts", false, "C:/project/a.ts"),
        mockEntry("src", true, "C:/project/src"),
      ],
      "C:/project/src": [mockEntry("b.ts", false, "C:/project/src/b.ts")],
    });
    const onViewStateChange = vi.fn();

    const { result } = renderHook(() =>
      useFileTree({
        rootPath: "C:/project",
        viewState: undefined,
        onViewStateChange,
      }),
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(2);
    }, { timeout: 3000 });
    onViewStateChange.mockClear();

    // 展开 → 上呼含新展开路径
    await act(async () => {
      await result.current.toggleExpand("C:/project/src");
    });
    await waitFor(() => {
      const src = result.current.rootNodes.find(
        (n) => n.entry.path === "C:/project/src",
      );
      expect(src?.expanded).toBe(true);
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(lastPayload(onViewStateChange)?.expandedPaths).toContain(
        "C:/project/src",
      );
    }, { timeout: 3000 });

    // 折叠 → 上呼移除该路径
    await act(async () => {
      await result.current.toggleExpand("C:/project/src");
    });
    await waitFor(() => {
      expect(lastPayload(onViewStateChange)?.expandedPaths).not.toContain(
        "C:/project/src",
      );
    }, { timeout: 3000 });
  });

  it("CP-016-4: 磁盘删除已展开目录经 fs-event 刷新后提交集收缩", async () => {
    vi.useFakeTimers();
    try {
      const vfs = makeVfs(mocks.mockReadDir, {
        "C:/project": [
          mockEntry("a.ts", false, "C:/project/a.ts"),
          mockEntry("src", true, "C:/project/src"),
        ],
        "C:/project/src": [mockEntry("b.ts", false, "C:/project/src/b.ts")],
      });
      const onViewStateChange = vi.fn();

      const { result } = renderHook(() =>
        useFileTree({
          rootPath: "C:/project",
          viewState: {
            rootPath: "C:/project",
            expandedPaths: ["C:/project/src"],
          },
          onViewStateChange,
        }),
      );

      // 先恢复展开 src
      await vi.waitFor(() => {
        const src = result.current.rootNodes.find(
          (n) => n.entry.path === "C:/project/src",
        );
        expect(src?.expanded).toBe(true);
      }, { timeout: 3000 });
      onViewStateChange.mockClear();

      // 磁盘删除 src + 手动 fs-event → 200ms 去抖 → refreshExpanded
      // （reloadPreservingExpanded 重建后 src 消失）→ 提交集收缩
      // 完整模拟磁盘删除：既删目录自身键（子目录 readDir → ENOENT），
      // 也从父层列表移除条目（父层 readDir 不再返回该项——行才真正消失，
      // 见 explorer/CLAUDE.md「已展开目录被磁盘删除 → 父层 readDir 不再返回该项」）
      vfs.set(
        "C:/project",
        (vfs.get("C:/project") ?? []).filter(
          (e) => e.path !== "C:/project/src",
        ),
      );
      vfs.delete("C:/project/src");
      await act(async () => {
        mocks.fireFsEvent();
        await vi.advanceTimersByTimeAsync(300);
      });

      await vi.waitFor(() => {
        expect(
          result.current.rootNodes.find(
            (n) => n.entry.path === "C:/project/src",
          ),
        ).toBeUndefined();
      }, { timeout: 3000 });
      expect(lastPayload(onViewStateChange)?.expandedPaths).not.toContain(
        "C:/project/src",
      );
    } finally {
      vi.useRealTimers();
    }
  });
});
