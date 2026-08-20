// explorer-notify.test.tsx — useFileTree 状态转换 + fs-event 消费测试
//
// watcher 生命周期用例（startWatch/stopWatch 时机、projectRootPath 推导、
// 卸载清理）已随 watcher 上提至 Workspace 项目激活层迁移——
// 见 workspace-switch-order.test.tsx「文件监听跟随项目激活（watcher 上提）」；
// 本文件仅保留 useFileTree 状态转换用例（loadRoot/toggleExpand/错误处理）。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ─── 共享 mock（通过 setup.ts 注册到 globalThis，vi.hoisted 中可用）───
import { mockEntry } from "./helpers/vfs";

const mocks = vi.hoisted(() => {
  const fs = __createFsMocks();
  const git = __createGitMocks();
  // 此文件使用 startWatch/stopWatch mock，不依赖 onFsEvent 回调模式
  const startWatch = vi.fn().mockResolvedValue(undefined);
  const stopWatch = vi.fn().mockResolvedValue(undefined);

  return {
    get mockStartWatch() { return startWatch; },
    get mockStopWatch() { return stopWatch; },
    get mockReadDir() { return fs.readDir; },
    get mockGitStatus() { return git.gitStatus; },
    resetAll() {
      startWatch.mockClear();
      stopWatch.mockClear();
      fs.readDir.mockClear();
      git.gitStatus.mockClear();
    },
  };
});

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  stopWatch: mocks.mockStopWatch,
  onFsEvent: () => () => {},
}));

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

// 导入真实 stores + useFileTree（在 IPC mock 之后）
import { useFileTree } from "../features/explorer/useFileTree";

// ─── Tests ───

// watcher 生命周期用例（startWatch/stopWatch 时机、projectRootPath 推导、
// 卸载清理）已随 watcher 上提至 Workspace 项目激活层迁移——
// 见 workspace-switch-order.test.tsx「文件监听跟随项目激活（watcher 上提）」；
// 本文件仅保留 fs-event 消费与 useFileTree 状态转换用例。

// ═══════════════════════════════════════════════════════════
// P2-50: useFileTree hook 测试 — loadRoot / toggleExpand / loadChildren 状态转换
// ═══════════════════════════════════════════════════════════

describe("useFileTree 状态转换", () => {
  beforeEach(() => {
    // mockReset: 清空 calls + implementation（包括 mockResolvedValueOnce 队列，防跨测试泄漏）
    mocks.mockReadDir.mockReset();
    mocks.mockGitStatus.mockReset();
    // 默认 mock：返回 3 个根条目（2 目录 + 1 文件）
    mocks.mockReadDir.mockResolvedValue([
      mockEntry("src", true, "/test/src"),
      mockEntry("docs", true, "/test/docs"),
      mockEntry("README.md", false, "/test/README.md"),
    ]);
    mocks.mockGitStatus.mockResolvedValue([]);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("loadRoot 状态转换", () => {
    it("7. rootPath 非空时 → 调用 readDir 并设置 rootNodes", async () => {
      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      // 初始状态：空数组（异步加载中）
      expect(result.current.rootNodes).toEqual([]);

      await vi.waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      }, { timeout: 3000 });

      expect(mocks.mockReadDir).toHaveBeenCalledWith("/test");
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(1);

      // 验证节点结构
      const node = result.current.rootNodes[0];
      expect(node.entry.name).toBe("src");
      expect(node.entry.isDir).toBe(true);
      expect(node.expanded).toBe(false);
      expect(node.children).toEqual([]);
      expect(node.loading).toBe(false);
    });

    it("8. rootPath 为 null → rootNodes 为空数组", async () => {
      const { result } = renderHook(() => useFileTree({ rootPath: null }));

      // 同步设置为空
      expect(result.current.rootNodes).toEqual([]);

      // 推进时间，确保 readDir 未被调用
      await vi.advanceTimersByTimeAsync(50);
      expect(mocks.mockReadDir).not.toHaveBeenCalled();
    });

    it("9. rootPath 变更时 → 重新调用 loadRoot", async () => {
      const { result, rerender } = renderHook(
        ({ rootPath }) => useFileTree({ rootPath }),
        { initialProps: { rootPath: "/test" as string | null } },
      );

      await vi.waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      }, { timeout: 3000 });
      expect(mocks.mockReadDir).toHaveBeenCalledWith("/test");

      // 变更 rootPath
      mocks.resetAll();
      mocks.mockReadDir.mockResolvedValue([
        mockEntry("lib", true, "/other/lib"),
        mockEntry("main.ts", false, "/other/main.ts"),
      ]);

      rerender({ rootPath: "/other" });

      await vi.waitFor(() => {
        expect(result.current.rootNodes.length).toBe(2);
      }, { timeout: 3000 });
      expect(mocks.mockReadDir).toHaveBeenCalledWith("/other");
    });
  });

  describe("toggleExpand 状态转换", () => {
    it("10. 点击展开目录 → expanded=true → 加载子节点 → loading=false", async () => {
      // 子目录内容
      mocks.mockReadDir
        .mockResolvedValueOnce([
          mockEntry("src", true, "/test/src"),
          mockEntry("docs", true, "/test/docs"),
          mockEntry("README.md", false, "/test/README.md"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
          mockEntry("lib.ts", false, "/test/src/lib.ts"),
        ]);

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      await vi.waitFor(() => {
        expect(result.current.rootNodes.length).toBe(3);
      }, { timeout: 3000 });

      const gitBefore = mocks.mockGitStatus.mock.calls.length;

      // 展开 src 目录
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      // 验证节点状态：expanded=true, loading=false（异步加载完成），children 已填充
      const srcNode = result.current.rootNodes.find(
        (n) => n.entry.path === "/test/src",
      );
      expect(srcNode).toBeDefined();
      expect(srcNode!.expanded).toBe(true);
      expect(srcNode!.loading).toBe(false);
      expect(srcNode!.children.length).toBe(2);
      expect(srcNode!.children[0].entry.name).toBe("index.ts");
      expect(srcNode!.children[1].entry.name).toBe("lib.ts");

      // 验证 readDir 被调用（第二次 call → 加载子目录）
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);

      // toggleExpand 不直接触发 gitStatus
      const gitAfter = mocks.mockGitStatus.mock.calls.length;
      expect(gitAfter).toBe(gitBefore);
    });

    it("11. 点击折叠已展开目录 → expanded=false（toggleExpand 折叠时也会重新 loadChildren）", async () => {
      // toggleExpand 总是调用 loadChildren(nodePath)，不限展开/折叠
      // 因此需要 3 个 once：根加载 + 展开加载 + 折叠时的 re-load
      mocks.mockReadDir
        .mockResolvedValueOnce([
          mockEntry("src", true, "/test/src"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
        ])
        .mockResolvedValueOnce([
          mockEntry("index.ts", false, "/test/src/index.ts"),
        ]);

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      await vi.waitFor(() => {
        expect(result.current.rootNodes.length).toBe(1);
      }, { timeout: 3000 });

      // 展开
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      let srcNode = result.current.rootNodes[0];
      expect(srcNode.expanded).toBe(true);
      expect(srcNode.children.length).toBe(1);

      // 折叠
      await act(async () => {
        await result.current.toggleExpand("/test/src");
      });

      srcNode = result.current.rootNodes[0];
      expect(srcNode.expanded).toBe(false);
      // children 保留（不丢失），但 expanded=false 时不应显示
      expect(srcNode.children.length).toBe(1);
    });
  });

  describe("错误处理", () => {
    it("12. readDir 失败 → rootNodes 保持空数组，不抛错", async () => {
      mocks.mockReadDir.mockRejectedValue(new Error("权限拒绝"));

      const { result } = renderHook(() => useFileTree({ rootPath: "/test" }));

      // loadDirectory 的 catch 分支静默返回 []
      await vi.waitFor(() => {
        expect(mocks.mockReadDir).toHaveBeenCalled();
      }, { timeout: 3000 });

      expect(result.current.rootNodes).toEqual([]);
    });
  });
});
