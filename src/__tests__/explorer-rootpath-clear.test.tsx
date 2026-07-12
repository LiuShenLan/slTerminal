// explorer-rootpath-clear.test.tsx — rootPath 变化时清空旧状态测试
//
// 验证修复：切换项目操作页面时，rootNodes 和 gitStatusMap 立即清空，
// 不残留旧项目的文件树数据。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";

// ─── 共享 mock 工厂（通过 setup.ts 注册到 globalThis，vi.hoisted 中可用）───
import { makeVfs, mockEntry, findNode } from "./helpers/vfs";

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

// ─── Tests ───

describe("useFileTree rootPath 变化清空旧状态", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    mocks.mockGitStatus.mockResolvedValue([]);
  });

  it("T2.1: rootPath 变化时立即清空 rootNodes 和 gitStatusMap", async () => {
    makeVfs(mocks.mockReadDir, {
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
    makeVfs(mocks.mockReadDir, {
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
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });

  it("T2.3: 清空后新项目数据正确加载", async () => {
    makeVfs(mocks.mockReadDir, {
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
    makeVfs(mocks.mockReadDir, {
      "/proj-a": [mockEntry("a.ts", false, "/proj-a/a.ts")],
      "/proj-c": [mockEntry("c.ts", false, "/proj-c/c.ts")],
    });

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

    await waitFor(() =>
      expect(result.current.rootNodes.length).toBe(1),
    );
    expect(result.current.rootNodes[0].entry.name).toBe("c.ts");
  });

  it("T2.5: rootPath 不变时不清空状态", async () => {
    makeVfs(mocks.mockReadDir, {
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
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("a.ts", false, "/proj/a.ts")],
    });

    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));

    // 同步渲染后：清空逻辑已将 rootNodes 设为 []
    expect(result.current.rootNodes).toEqual([]);
    expect(result.current.gitStatusMap.size).toBe(0);
  });
});
