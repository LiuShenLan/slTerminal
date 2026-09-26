// use-file-tree-filter.test.ts — useFileTree 过滤选项单元测试
//
// 覆盖（Agent 全局文件视图依赖的三项能力，ADR-0024）：
// - rootFilter：根层三点过滤（loadRoot 首帧 / 续页 / 子层不受影响）+ 引用变化即时刷新
// - eventPathFilter：fs-event 二次过滤（全部被滤 → 不刷新；任一通过 → 刷新）
// - 内置根前缀过滤：root 外事件不刷新 / root 内刷新 / 空 paths 保守放行

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  type DirEntry = { name: string; path: string; isDir: boolean; size: number | null; modified: number | null };
  type ReadDirPage = { entries: DirEntry[]; nextCursor: string | null };
  type FsEventPayload = { paths: string[]; kind: string; detail: string };

  const ROOT = "C:/agent";

  const makeEntry = (name: string, isDir = false): DirEntry => ({
    name,
    path: `${ROOT}/${name}`,
    isDir,
    size: isDir ? null : 100,
    modified: isDir ? null : 1234567890,
  });

  /** 根层默认条目：两文件 + 一目录（settings.json / projects / CLAUDE.md） */
  const defaultRootEntries = (): DirEntry[] => [
    makeEntry("settings.json"),
    makeEntry("projects", true),
    makeEntry("CLAUDE.md"),
  ];

  let readDirImpl: (path: string) => Promise<DirEntry[]> = async (path) =>
    path === ROOT ? defaultRootEntries() : [];

  // 续页测试用：首页游标 "page2"，第二页末页
  let pagedImpl: ((path: string, cursor?: string | null) => Promise<ReadDirPage>) | null = null;

  const mockReadDir = vi
    .fn<(path: string, cursor?: string | null) => Promise<ReadDirPage>>()
    .mockImplementation(async (path, cursor) => {
      if (pagedImpl) return pagedImpl(path, cursor);
      return { entries: await readDirImpl(path), nextCursor: null };
    });
  const mockGitStatus = vi.fn().mockResolvedValue([]);

  let fsEventHandler: ((payload: FsEventPayload) => void) | null = null;
  const mockOnFsEvent = vi.fn((handler: (payload: FsEventPayload) => void) => {
    fsEventHandler = handler;
    return () => {};
  });

  return {
    ROOT,
    makeEntry,
    mockReadDir,
    mockGitStatus,
    mockOnFsEvent,
    fireFsEvent(payload: FsEventPayload) {
      fsEventHandler?.(payload);
    },
    setReadDirImpl(fn: (path: string) => Promise<DirEntry[]>) {
      readDirImpl = fn;
    },
    setPagedImpl(fn: ((path: string, cursor?: string | null) => Promise<ReadDirPage>) | null) {
      pagedImpl = fn;
    },
    resetAll() {
      fsEventHandler = null;
      pagedImpl = null;
      readDirImpl = async (path) => (path === ROOT ? defaultRootEntries() : []);
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
      mockOnFsEvent.mockClear();
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
  stopWatch: vi.fn().mockResolvedValue(undefined),
  onFsEvent: mocks.mockOnFsEvent,
}));

import { useFileTree } from "../features/explorer/useFileTree";

/** 挂载 hook 并等首帧加载落定 */
async function mountTree(options?: {
  rootFilter?: (name: string) => boolean;
  eventPathFilter?: (absPath: string) => boolean;
}) {
  const hook = renderHook(
    ({ rootFilter, eventPathFilter }) =>
      useFileTree({
        rootPath: mocks.ROOT,
        viewState: undefined,
        onViewStateChange: undefined,
        rootFilter,
        eventPathFilter,
      }),
    {
      initialProps: {
        rootFilter: options?.rootFilter,
        eventPathFilter: options?.eventPathFilter,
      },
    },
  );
  await waitFor(() => {
    expect(hook.result.current.rootNodes.length).toBeGreaterThan(0);
  });
  return hook;
}

describe("useFileTree — rootFilter 根层过滤", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("loadRoot 首帧：仅过滤根层条目（projects 被滤除）", async () => {
    const { result } = await mountTree({
      rootFilter: (name) => name !== "projects",
    });
    const names = result.current.rootNodes.map((n) => n.entry.name);
    expect(names).toEqual(["settings.json", "CLAUDE.md"]);
  });

  it("续页：过滤同样应用于后台拼接的后续页", async () => {
    mocks.setPagedImpl(async (_path, cursor) => {
      if (cursor === undefined || cursor === null) {
        return {
          entries: [mocks.makeEntry("settings.json")],
          nextCursor: "page2",
        };
      }
      return {
        entries: [mocks.makeEntry("projects", true), mocks.makeEntry("CLAUDE.md")],
        nextCursor: null,
      };
    });
    const { result } = await mountTree({
      rootFilter: (name) => name !== "projects",
    });
    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(2);
    });
    const names = result.current.rootNodes.map((n) => n.entry.name);
    expect(names).toEqual(["settings.json", "CLAUDE.md"]);
  });

  it("子层不受影响：展开根下目录时子目录条目不过滤", async () => {
    mocks.setReadDirImpl(async (path) => {
      if (path === mocks.ROOT) {
        return [mocks.makeEntry("projects", true), mocks.makeEntry("settings.json")];
      }
      if (path === `${mocks.ROOT}/projects`) {
        // 子层含 rootFilter 会滤除的「settings.json」——rootFilter 仅根层生效，子层原样返回
        return [mocks.makeEntry("settings.json"), mocks.makeEntry("a.jsonl")];
      }
      return [];
    });
    const { result } = await mountTree({
      rootFilter: (name) => name !== "settings.json",
    });
    // 根层 settings.json 已被滤除，仅剩 projects 目录
    expect(result.current.rootNodes.map((n) => n.entry.name)).toEqual(["projects"]);
    await act(async () => {
      await result.current.toggleExpand(`${mocks.ROOT}/projects`);
    });
    const projectsNode = result.current.rootNodes[0];
    expect(projectsNode.expanded).toBe(true);
    expect(projectsNode.children.map((n) => n.entry.name)).toEqual([
      "settings.json",
      "a.jsonl",
    ]);
  });

  it("rootFilter 引用变化 → 自动 refreshExpanded 重拉（配置即时生效）", async () => {
    const filterA = (name: string) => name !== "projects";
    const { result, rerender } = await mountTree({ rootFilter: filterA });
    expect(result.current.rootNodes.map((n) => n.entry.name)).not.toContain("projects");

    mocks.mockReadDir.mockClear();
    // 换过滤器（放行全部）→ 应触发整树重拉
    rerender({ rootFilter: () => true, eventPathFilter: undefined });
    await waitFor(() => {
      expect(result.current.rootNodes.map((n) => n.entry.name)).toContain("projects");
    });
    expect(mocks.mockReadDir).toHaveBeenCalled();
  });

  it("rootFilter 引用不变 → 不触发额外重拉", async () => {
    const filter = (name: string) => name !== "projects";
    const { result, rerender } = await mountTree({ rootFilter: filter });
    const callsAfterMount = mocks.mockReadDir.mock.calls.length;
    rerender({ rootFilter: filter, eventPathFilter: undefined });
    // 等一拍确认无新调用
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.mockReadDir.mock.calls.length).toBe(callsAfterMount);
    expect(result.current.rootNodes.length).toBe(2);
  });
});

describe("useFileTree — fs-event 内置根前缀过滤", () => {
  beforeEach(() => {
    mocks.resetAll();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const fsPayload = (paths: string[]) => ({ paths, kind: "Any", detail: "Any" });

  it("全部事件路径在 root 外 → 跳过刷新（无 readDir 追加调用）", async () => {
    await mountTree();
    mocks.mockReadDir.mockClear();
    // fake timers 须在挂载（real-timer waitFor）落定后开启，否则 waitFor 轮询不推进
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(fsPayload(["C:/other-project/a.ts", "C:/other/b.ts"]));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });

  it("任一路径在 root 内 → 200ms 去抖后整树刷新", async () => {
    await mountTree();
    mocks.mockReadDir.mockClear();
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(fsPayload(["C:/other/a.ts", `${mocks.ROOT}/settings.json`]));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).toHaveBeenCalledWith(mocks.ROOT);
  });

  it("事件路径 = 监听根本身（need_rescan 形态）→ 放行刷新", async () => {
    await mountTree();
    mocks.mockReadDir.mockClear();
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(fsPayload([mocks.ROOT]));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).toHaveBeenCalledWith(mocks.ROOT);
  });

  it("空 paths → 保守放行刷新", async () => {
    await mountTree();
    mocks.mockReadDir.mockClear();
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(fsPayload([]));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).toHaveBeenCalledWith(mocks.ROOT);
  });
});

describe("useFileTree — eventPathFilter 二次过滤", () => {
  beforeEach(() => {
    mocks.resetAll();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const fsPayload = (paths: string[]) => ({ paths, kind: "Any", detail: "Any" });
  /** 滤掉 projects 首段（运行时文件语义） */
  const noProjects = (absPath: string) =>
    !absPath.toLowerCase().startsWith(`${mocks.ROOT}/projects`.toLowerCase());

  it("全部路径被 eventPathFilter 滤除 → 不刷新", async () => {
    await mountTree({ eventPathFilter: noProjects });
    mocks.mockReadDir.mockClear();
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(fsPayload([`${mocks.ROOT}/projects/sess/a.jsonl`]));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).not.toHaveBeenCalled();
  });

  it("任一路径通过 eventPathFilter → 放行刷新（事件粒度保守，不逐路径裁剪）", async () => {
    await mountTree({ eventPathFilter: noProjects });
    mocks.mockReadDir.mockClear();
    vi.useFakeTimers();
    await act(async () => {
      mocks.fireFsEvent(
        fsPayload([`${mocks.ROOT}/projects/a.jsonl`, `${mocks.ROOT}/settings.json`]),
      );
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.mockReadDir).toHaveBeenCalledWith(mocks.ROOT);
  });
});
