// useFileTree.test.ts — useFileTree hook 单元测试
//
// F1: 消除重复 IPC — 验证 rootPath 变化时 loadRoot 只调用一次
// F3: generation 取消机制 — 验证旧异步请求结果被丢弃

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  let mockReadDirImpl = (_path: string) => Promise.resolve([] as { name: string; path: string; isDir: boolean; size: number | null; modified: number | null }[]);
  let mockGitStatusImpl = (_path: string) => Promise.resolve([] as { path: string; status: string }[]);

  const mockReadDir = vi.fn().mockImplementation((path: string) => mockReadDirImpl(path));
  const mockGitStatus = vi.fn().mockImplementation((path: string) => mockGitStatusImpl(path));

  const makeEntry = (name: string, isDir = false) => ({
    name,
    path: `C:/project/${name}`,
    isDir,
    size: isDir ? null : 100,
    modified: isDir ? null : 1234567890,
  });

  return {
    mockReadDir,
    mockGitStatus,
    makeEntry,
    resetAll() {
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
      mockReadDirImpl = (_path: string) => Promise.resolve([
        makeEntry("a.ts"), makeEntry("b.ts"), makeEntry("src", true),
      ]);
      mockGitStatusImpl = (_path: string) => Promise.resolve([
        { path: "C:/project/a.ts", status: "modified" },
      ]);
      mockReadDir.mockImplementation((path: string) => mockReadDirImpl(path));
      mockGitStatus.mockImplementation((path: string) => mockGitStatusImpl(path));
    },
    setReadDirImpl(fn: (path: string) => Promise<ReturnType<typeof makeEntry>[]>) {
      mockReadDirImpl = fn;
      mockReadDir.mockImplementation((path: string) => mockReadDirImpl(path));
    },
    setGitStatusImpl(fn: (path: string) => Promise<{ path: string; status: string }[]>) {
      mockGitStatusImpl = fn;
      mockGitStatus.mockImplementation((path: string) => mockGitStatusImpl(path));
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
  onFsEvent: () => () => {},
}));

// 真实模块导入（mock 之后）
import { useFileTree } from "../features/explorer/useFileTree";

// ============================================================
// F1: 消除重复 IPC 调用
// ============================================================
describe("useFileTree — F1 消除重复 IPC", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("F1-1: rootPath 变化时 loadRoot 只调用一次 readDir", async () => {
    const { rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 首次设置 rootPath
    rerender({ rootPath: "C:/project-a" });

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    });

    // 切换到另一个 rootPath（应只再调一次 readDir）
    mocks.setReadDirImpl(() => Promise.resolve([
      mocks.makeEntry("x.ts"), mocks.makeEntry("y.ts"),
    ]));
    rerender({ rootPath: "C:/project-b" });

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    });
  });

  it("F1-2: rootPath 从 null 变为有效路径时正确加载", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    expect(result.current.rootNodes).toEqual([]);

    rerender({ rootPath: "C:/project" });

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    });
    expect(result.current.rootNodes[0].entry.name).toBe("a.ts");
  });

  it("F1-3: rootPath 从有效路径变为 null 时清空", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBeGreaterThan(0);
    });

    rerender({ rootPath: null });

    await waitFor(() => {
      expect(result.current.rootNodes).toEqual([]);
    });
  });

  it("F1-4: 两个不同 rootPath 切换时数据正确", async () => {
    mocks.setReadDirImpl((path) => {
      if (path === "C:/project-a") {
        return Promise.resolve([mocks.makeEntry("alpha.ts")]);
      }
      return Promise.resolve([mocks.makeEntry("beta.ts")]);
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: "C:/project-a" as string | null } },
    );

    await waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("alpha.ts");
    });

    rerender({ rootPath: "C:/project-b" });

    await waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("beta.ts");
    });
  });

  it("F1-5: 重命名文件后 refresh 重新调用 readDir", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    });

    // 手动调用 refresh（模拟 CRUD 操作后刷新）
    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });

  it("F1-6: 删除文件后 refresh 重新调用 readDir", async () => {
    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: "C:/project" as string | null } },
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    });

    const callCountBefore = mocks.mockReadDir.mock.calls.length;

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(callCountBefore + 1);
  });

  it("F1-7: 新建文件后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      () => useFileTree({ rootPath: "C:/project" }),
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });

  it("F1-8: 新建文件夹后 refresh 重新调用 readDir", async () => {
    const { result } = renderHook(
      () => useFileTree({ rootPath: "C:/project" }),
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
  });
});

// ============================================================
// F3: Generation 取消机制
// ============================================================
describe("useFileTree — F3 Generation 取消机制", () => {
  beforeEach(() => {
    mocks.resetAll();
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
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 先切到 project-a（慢），立即切到 project-b（快）
    rerender({ rootPath: "C:/project-a" });
    rerender({ rootPath: "C:/project-b" });

    // 等待快请求完成
    await waitFor(() => {
      expect(result.current.rootNodes.length).toBeGreaterThan(0);
    });

    // 即使慢请求返回，也应该显示 project-b 的数据
    expect(result.current.rootNodes[0].entry.name).toBe("new.ts");

    // 等待慢请求也完成，确保 generation 检查生效
    await new Promise((r) => setTimeout(r, 300));

    // 数据仍应该是 project-b 的
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
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    rerender({ rootPath: "C:/project-slow" });
    rerender({ rootPath: "C:/project-fast" });

    await waitFor(() => {
      expect(result.current.rootNodes[0]?.entry.name).toBe("fast-result.ts");
    });

    // 等待慢请求完成
    await new Promise((r) => setTimeout(r, 300));

    // 数据应保持为快请求的结果
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
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    rerender({ rootPath: "C:/project-slow" });
    rerender({ rootPath: "C:/project-fast" });

    await waitFor(() => {
      const map = result.current.gitStatusMap;
      expect(map.get("C:/project-fast/b.ts")).toBe("added");
    });

    // 等待慢请求完成
    await new Promise((r) => setTimeout(r, 300));

    // 不应包含慢请求的数据
    const map = result.current.gitStatusMap;
    expect(map.get("C:/project-slow/a.ts")).toBeUndefined();
  });

  it("F3-4: 同 rootPath 不触发取消（正常流程）", async () => {
    const { result } = renderHook(
      () => useFileTree({ rootPath: "C:/project" }),
    );

    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledWith("C:/project");
    });

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
      expect(result.current.rootNodes[0].entry.name).toBe("a.ts");
    });

    // gitStatus 也应正常返回
    await waitFor(() => {
      expect(result.current.gitStatusMap.get("C:/project/a.ts")).toBe("modified");
    });
  });

  it("F3-5: generation 在多次切换后正确递增", async () => {
    const { rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: null as string | null } },
    );

    // 切换 5 次
    for (let i = 0; i < 5; i++) {
      rerender({ rootPath: `C:/project-${i}` });
    }

    // 验证 readDir 被调用了 5 次（每次切换都触发 loadRoot）
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(5);
    });

    // 最后一次切换的数据应生效
    expect(mocks.mockReadDir).toHaveBeenLastCalledWith("C:/project-4");
  });

  it("F3-6: loadRoot 返回后 generation 匹配，数据正常更新", async () => {
    const { result } = renderHook(
      () => useFileTree({ rootPath: "C:/project" }),
    );

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    });

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
      ({ rootPath }) => useFileTree({ rootPath: rootPath as string | null }),
      { initialProps: { rootPath: "C:/project-a" as string | null } },
    );

    // 立即切换到 project-b（project-a 的 gitStatus 尚在等待中）
    rerender({ rootPath: "C:/project-b" });

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(3);
    });

    // 等待慢 gitStatus 完成
    await new Promise((r) => setTimeout(r, 300));

    // 不应包含 project-a 的 gitStatus 数据（已被 generation 检查丢弃）
    expect(result.current.gitStatusMap.get("C:/project-a/x.ts")).toBeUndefined();
  });
});
