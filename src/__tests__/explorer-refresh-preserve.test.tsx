// explorer-refresh-preserve.test.tsx — useFileTree 刷新保留展开状态测试
//
// 验证修复：文件变更（Ctrl+S / fs-event / CRUD refresh）后，文件树保留任意深度
// 的已展开状态，同时反映子目录内文件增删、git 着色仍刷新。
//
// 覆盖矩阵（对应计划 A–E）：
//   A. reloadPreservingExpanded 递归重建正确性：R1/R2/R3/R4/R7/R8/R10
//   B. 边界与容错分支：R6/R11/R9/R12
//   C. gitStatus 刷新路径：R5/R13
//   D. 三条触发路径均保留展开：R14/R15/R16
//   E. 竞态：R17

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";

// ─── 共享 mock（通过 setup.ts 注册到 globalThis，vi.hoisted 中可用）───
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

// ─── 辅助 ───

/** 展开指定路径（驱动 toggleExpand + 等待异步 children 加载） */
async function expand(
  result: { current: ReturnType<typeof useFileTree> },
  path: string,
) {
  await act(async () => {
    await result.current.toggleExpand(path);
  });
  await waitFor(() => {
    expect(findNode(result.current.rootNodes, path)?.expanded).toBe(true);
    expect(findNode(result.current.rootNodes, path)?.loading).toBe(false);
  });
}

// ─── Tests ───

describe("useFileTree 刷新保留展开状态", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    mocks.mockGitStatus.mockResolvedValue([]);
  });

  // ═══ A. reloadPreservingExpanded 递归重建正确性 ═══

  it("R1: 单层展开目录 → refresh() 后 expanded 保留且 children 保留", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));

    await expand(result, "/proj/src");
    expect(findNode(result.current.rootNodes, "/proj/src")?.children.length).toBe(1);

    await act(async () => {
      await result.current.refresh();
    });

    const src = findNode(result.current.rootNodes, "/proj/src");
    expect(src?.expanded).toBe(true);
    expect(src?.children.map((c) => c.entry.name)).toEqual(["a.ts"]);
  });

  it("R2: 展开目录磁盘新增文件 → refresh 后 children 含新文件（折叠态）", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    // 磁盘新增 b.ts
    vfs.set("/proj/src", [
      mockEntry("a.ts", false, "/proj/src/a.ts"),
      mockEntry("b.ts", false, "/proj/src/b.ts"),
    ]);

    await act(async () => {
      await result.current.refresh();
    });

    const src = findNode(result.current.rootNodes, "/proj/src");
    expect(src?.expanded).toBe(true);
    expect(src?.children.map((c) => c.entry.name)).toEqual(["a.ts", "b.ts"]);
  });

  it("R3: 展开目录磁盘删除文件 → refresh 后 children 不含该文件", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [
        mockEntry("a.ts", false, "/proj/src/a.ts"),
        mockEntry("b.ts", false, "/proj/src/b.ts"),
      ],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    // 磁盘删除 b.ts
    vfs.set("/proj/src", [mockEntry("a.ts", false, "/proj/src/a.ts")]);

    await act(async () => {
      await result.current.refresh();
    });

    const src = findNode(result.current.rootNodes, "/proj/src");
    expect(src?.children.map((c) => c.entry.name)).toEqual(["a.ts"]);
  });

  it("R4: 多层嵌套展开（root→src→components）→ refresh 后三层 expanded 全保留", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("components", true, "/proj/src/components")],
      "/proj/src/components": [
        mockEntry("Button.tsx", false, "/proj/src/components/Button.tsx"),
      ],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));

    await expand(result, "/proj/src");
    await expand(result, "/proj/src/components");

    await act(async () => {
      await result.current.refresh();
    });

    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
    expect(
      findNode(result.current.rootNodes, "/proj/src/components")?.expanded,
    ).toBe(true);
    expect(
      findNode(result.current.rootNodes, "/proj/src/components")?.children.map(
        (c) => c.entry.name,
      ),
    ).toEqual(["Button.tsx"]);
  });

  it("R7: 未展开兄弟目录 refresh 后仍折叠（不误展开）", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [
        mockEntry("src", true, "/proj/src"),
        mockEntry("docs", true, "/proj/docs"),
      ],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
      "/proj/docs": [mockEntry("readme.md", false, "/proj/docs/readme.md")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(2));

    await expand(result, "/proj/src"); // 仅展开 src

    await act(async () => {
      await result.current.refresh();
    });

    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
    const docs = findNode(result.current.rootNodes, "/proj/docs");
    expect(docs?.expanded).toBe(false);
    expect(docs?.children).toEqual([]);
  });

  it("R8: 空目录展开后 refresh → 保持 expanded、children=[]", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("empty", true, "/proj/empty")],
      "/proj/empty": [],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/empty");

    await act(async () => {
      await result.current.refresh();
    });

    const empty = findNode(result.current.rootNodes, "/proj/empty");
    expect(empty?.expanded).toBe(true);
    expect(empty?.children).toEqual([]);
  });

  it("R10: 旧节点 expanded 但新一层同 path 变为文件 → 不递归、不报错", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("thing", true, "/proj/thing")],
      "/proj/thing": [mockEntry("x.ts", false, "/proj/thing/x.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/thing");

    // 磁盘上 thing 从目录变成文件（同名 path，isDir 翻转）
    vfs.set("/proj", [mockEntry("thing", false, "/proj/thing")]);

    await act(async () => {
      await result.current.refresh();
    });

    const thing = findNode(result.current.rootNodes, "/proj/thing");
    expect(thing?.entry.isDir).toBe(false);
    expect(thing?.expanded).toBe(false); // isDir 为 false → 不保留展开
    expect(thing?.children).toEqual([]);
  });

  // ═══ B. 边界与容错分支 ═══

  it("R6: 已展开目录被磁盘删除 → 节点连子树消失、不抛错", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [
        mockEntry("src", true, "/proj/src"),
        mockEntry("keep", true, "/proj/keep"),
      ],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
      "/proj/keep": [mockEntry("k.ts", false, "/proj/keep/k.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(2));
    await expand(result, "/proj/src");

    // 磁盘删除 src（父层不再返回该项）
    vfs.set("/proj", [mockEntry("keep", true, "/proj/keep")]);
    vfs.delete("/proj/src");

    await act(async () => {
      await result.current.refresh();
    });

    expect(findNode(result.current.rootNodes, "/proj/src")).toBeUndefined();
    expect(findNode(result.current.rootNodes, "/proj/keep")).toBeDefined();
  });

  it("R11: 已展开子目录 readDir 抛错 → 该目录 children 为空、不冒泡异常", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    // src 仍在父层返回，但其自身 readDir 抛错（权限/瞬时错误）
    mocks.mockReadDir.mockImplementation(async (dirPath: string) => {
      if (dirPath === "/proj/src") throw new Error("EACCES");
      if (!vfs.has(dirPath)) throw new Error(`ENOENT: ${dirPath}`);
      return vfs.get(dirPath)!;
    });

    await act(async () => {
      await result.current.refresh();
    });

    const src = findNode(result.current.rootNodes, "/proj/src");
    expect(src?.expanded).toBe(true);
    expect(src?.children).toEqual([]); // loadDirectory catch → []
  });

  it("R9: rootPath=null → refresh() 立即返回，不调 readDir/gitStatus", async () => {
    const { result } = renderHook(() => useFileTree({ rootPath: null }));
    await new Promise((r) => setTimeout(r, 30));
    mocks.mockReadDir.mockClear();
    mocks.mockGitStatus.mockClear();

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockReadDir).not.toHaveBeenCalled();
    expect(mocks.mockGitStatus).not.toHaveBeenCalled();
  });

  it("R12: 根目录被删（readDir 抛错）→ rootNodes 置空、不抛错", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    // 根目录被删
    vfs.delete("/proj");

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.rootNodes).toEqual([]);
  });

  // ═══ C. gitStatus 刷新路径 ═══

  it("R5: refresh 后 gitStatus 被调用、gitStatusMap 更新", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    mocks.mockGitStatus.mockClear();
    mocks.mockGitStatus.mockResolvedValueOnce([
      { path: "/proj/src/a.ts", status: "modified" },
    ]);

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.mockGitStatus).toHaveBeenCalledWith("/proj");
    await waitFor(() => {
      expect(result.current.gitStatusMap.get("/proj/src/a.ts")).toBe("modified");
    });
  });

  it("R13: gitStatus 抛错 → gitStatusMap 降级为空 Map、不冒泡", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    // 初始 gitStatus 返回一个值
    mocks.mockGitStatus.mockResolvedValueOnce([
      { path: "/proj/src/a.ts", status: "modified" },
    ]);
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    // refresh 时 gitStatus 抛错
    mocks.mockGitStatus.mockRejectedValueOnce(new Error("not a git repo"));

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.gitStatusMap.size).toBe(0);
    // 展开状态不受 gitStatus 失败影响
    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
  });

  // ═══ D. 三条触发路径均保留展开 ═══

  it("R14: dispatch slterm:file-saved → 展开状态保留（Ctrl+S 路径）", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    await act(async () => {
      window.dispatchEvent(
        new CustomEvent("slterm:file-saved", {
          detail: { path: "/proj/src/a.ts" },
        }),
      );
    });

    await waitFor(() => {
      expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
    });
  });

  it("R15: fs-event 触发（200ms 去抖）→ 展开状态保留", async () => {
    vi.useFakeTimers();
    try {
      makeVfs(mocks.mockReadDir, {
        "/proj": [mockEntry("src", true, "/proj/src")],
        "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
      });
      const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
      await vi.waitFor(() => expect(result.current.rootNodes.length).toBe(1));

      await act(async () => {
        await result.current.toggleExpand("/proj/src");
      });
      await vi.waitFor(() =>
        expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true),
      );

      // 触发 fs-event → 200ms 去抖后 refreshExpanded
      await act(async () => {
        mocks.triggerFsEvent();
        await vi.advanceTimersByTimeAsync(250);
      });

      expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it("R16: 直接调 refresh()（CRUD 路径）→ 展开状态保留", async () => {
    makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    await act(async () => {
      await result.current.refresh();
    });

    expect(findNode(result.current.rootNodes, "/proj/src")?.expanded).toBe(true);
  });

  // ═══ E. 竞态 ═══

  it("R17: 连续两次 refresh（后者磁盘不同）→ last-write-wins 且展开保留", async () => {
    const vfs = makeVfs(mocks.mockReadDir, {
      "/proj": [mockEntry("src", true, "/proj/src")],
      "/proj/src": [mockEntry("a.ts", false, "/proj/src/a.ts")],
    });
    const { result } = renderHook(() => useFileTree({ rootPath: "/proj" }));
    await waitFor(() => expect(result.current.rootNodes.length).toBe(1));
    await expand(result, "/proj/src");

    await act(async () => {
      const first = result.current.refresh();
      // 第二次 refresh 前磁盘发生变更
      vfs.set("/proj/src", [
        mockEntry("a.ts", false, "/proj/src/a.ts"),
        mockEntry("c.ts", false, "/proj/src/c.ts"),
      ]);
      const second = result.current.refresh();
      await Promise.all([first, second]);
    });

    const src = findNode(result.current.rootNodes, "/proj/src");
    expect(src?.expanded).toBe(true);
    expect(src?.children.map((c) => c.entry.name)).toEqual(["a.ts", "c.ts"]);
  });
});
