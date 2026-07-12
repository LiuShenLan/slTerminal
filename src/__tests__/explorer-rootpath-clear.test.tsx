// explorer-rootpath-clear.test.tsx — rootPath 变化时清空旧状态测试
//
// 验证修复：切换项目操作页面时，rootNodes 和 gitStatusMap 立即清空，
// 不残留旧项目的文件树数据。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";
import type { DirEntry } from "../types/fs";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  let fsEventCallback: (() => void) | null = null;
  const mockOnFsEvent = vi.fn((cb: () => void) => {
    fsEventCallback = cb;
    return () => {
      fsEventCallback = null;
    };
  });
  return {
    mockReadDir,
    mockGitStatus,
    mockOnFsEvent,
    triggerFsEvent() {
      fsEventCallback?.();
    },
    resetAll() {
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockOnFsEvent.mockClear();
      fsEventCallback = null;
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

// ─── 辅助 ───

/** 创建模拟 DirEntry */
function mockEntry(name: string, isDir: boolean, path: string): DirEntry {
  return {
    name,
    path,
    isDir,
    ...(isDir ? {} : { size: 1024, modified: 1 }),
  };
}

/**
 * 虚拟文件系统：Map<dirPath, DirEntry[]>。
 * mockReadDir 按传入 dirPath 分派对应子项。
 */
function makeVfs(initial: Record<string, DirEntry[]>) {
  const vfs = new Map<string, DirEntry[]>(Object.entries(initial));
  mocks.mockReadDir.mockImplementation(async (dirPath: string) => {
    if (!vfs.has(dirPath)) throw new Error(`ENOENT: ${dirPath}`);
    return vfs.get(dirPath)!;
  });
  return vfs;
}

/** 在 rootNodes 中按 path 查找节点（递归） */
function findNode(
  nodes: ReturnType<typeof useFileTree>["rootNodes"],
  path: string,
): ReturnType<typeof useFileTree>["rootNodes"][number] | undefined {
  for (const n of nodes) {
    if (n.entry.path === path) return n;
    const found = findNode(n.children, path);
    if (found) return found;
  }
  return undefined;
}

// ─── Tests ───

describe("useFileTree rootPath 变化清空旧状态", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    mocks.mockGitStatus.mockResolvedValue([]);
  });

  it("T2.1: rootPath 变化时立即清空 rootNodes 和 gitStatusMap", async () => {
    makeVfs({
      "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
      "/proj-b": [mockEntry("b.ts", false, "/proj-b/b.ts")],
    });
    mocks.mockGitStatus.mockResolvedValue([
      { path: "/proj-a/a.ts", status: "modified" },
    ]);

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    // 等待 proj-a 加载完成
    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );
    await waitFor(() =>
      expect(result.current.gitStatusMap.get("/proj-a/a.ts")).toBe("modified"),
    );

    // 切换到 proj-b：应同步清空（在 React 同一批次中）
    mocks.mockGitStatus.mockResolvedValue([
      { path: "/proj-b/b.ts", status: "untracked" },
    ]);
    rerender({ rootPath: "/proj-b" });

    // 同步检查：rootNodes 和 gitStatusMap 应已清空
    expect(result.current.rootNodes).toEqual([]);
    expect(result.current.gitStatusMap.size).toBe(0);

    // 异步等待新数据加载
    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );
    expect(result.current.rootNodes[0].entry.name).toBe("b.ts");
  });

  it("T2.2: rootPath 为 null 时清空状态且无 IPC 调用", async () => {
    makeVfs({
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj" as string | null } },
    );

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );

    mocks.mockReadDir.mockClear();
    mocks.mockGitStatus.mockClear();

    // 切换到 null
    rerender({ rootPath: null });

    expect(result.current.rootNodes).toEqual([]);
    expect(result.current.gitStatusMap.size).toBe(0);

    // 确保没有额外的 IPC 调用（loadRoot 对 null path 直接 return）
    // 注意：mockReadDir 可能在 rerender 触发的 effect 之前被调用一次，
    // 但因为我们先 setRootNodes([]) 再早期 return，不应该有 IPC 调用
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });

  it("T2.3: 清空后新项目数据正确加载", async () => {
    makeVfs({
      "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
      "/proj-b": [
        mockEntry("src", true, "/proj-b/src"),
        mockEntry("README.md", false, "/proj-b/README.md"),
      ],
      "/proj-b/src": [mockEntry("main.ts", false, "/proj-b/src/main.ts")],
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );

    // 切换到 proj-b
    mocks.mockGitStatus.mockResolvedValue([
      { path: "/proj-b/src/main.ts", status: "added" },
    ]);
    rerender({ rootPath: "/proj-b" });

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(2),
    );

    // 验证 proj-b 的数据（不是 proj-a 的残留）
    const names = result.current.rootNodes.map((n) => n.entry.name);
    expect(names).toEqual(["src", "README.md"]);

    // gitStatusMap 应为 proj-b 的数据
    await waitFor(() =>
      expect(result.current.gitStatusMap.get("/proj-b/src/main.ts")).toBe("added"),
    );
  });

  it("T2.4: 快速连续切换只保留最终结果（gen 检查防竞态）", async () => {
    // 三个项目：A（即时返回）、B（延迟返回）、C（即时返回）
    makeVfs({
      "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
      "/proj-c": [mockEntry("c.ts", false, "/proj-c/c.ts")],
    });

    // B 的 vfs 没有注册 — mockReadDir 会 throw ENOENT，
    // 但 loadDirectory 内部 catch 返回 []，所以超时后 rootNodes 仍为 []

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj-a" as string | null } },
    );

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );

    // 快速切换 A→B→C（B 路径无 vfs → loadDirectory 抛错返回 []）
    rerender({ rootPath: "/proj-b" });
    rerender({ rootPath: "/proj-c" });

    // gen 应为 3（A=1, B=2, C=3），C 的 loadRoot 使用 gen=3
    // C 的数据应最终加载
    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );
    expect(result.current.rootNodes[0].entry.name).toBe("c.ts");
  });

  it("T2.5: rootPath 不变时不清空状态", async () => {
    makeVfs({
      "/proj": [
        mockEntry("src", true, "/proj/src"),
      ],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });

    const { result, rerender } = renderHook(
      ({ rootPath }) => useFileTree({ rootPath }),
      { initialProps: { rootPath: "/proj" as string | null } },
    );

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );

    // 展开 src
    await act(async () => {
      await result.current.toggleExpand("/proj/src");
    });
    await waitFor(() => {
      expect(findNode(result.current.rootNodes, "/proj/src")?.loading).toBe(false);
    });
    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);

    // 同值 rerender
    rerender({ rootPath: "/proj" });

    // rootNodes 不应被清空（effect 不触发，因为依赖未变化）
    expect(result.current.rootNodes.length).toBe(1);
    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
    expect(findNode(result.current.rootNodes, "/proj/src")?.children.length).toBe(1);
  });

  it("T2.6: 首次挂载时 rootNodes 初始为空", () => {
    makeVfs({
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

    // 同步渲染后：清空逻辑已将 rootNodes 设为 []
    expect(result.current.rootNodes).toEqual([]);
    expect(result.current.gitStatusMap.size).toBe(0);
  });
});
