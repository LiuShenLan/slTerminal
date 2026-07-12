// testMocks/explorerMocks.ts — 文件浏览器测试共享 mock 接口定义（参考文档）
//
// 实际实现在 setup.ts 中注册为全局函数（__createFsMocks/__createGitMocks/__createNotifyMocks），
// 供 vi.hoisted() 回调使用（vi.hoisted 运行在 ESM import 之前，无法通过 import 加载外部模块）。
// 本文件保留接口定义和用法文档。
//
// 用法：
//   const mocks = vi.hoisted(() => {
//     const fs = __createFsMocks();      // readDir 默认 mockResolvedValue([])
//     const git = __createGitMocks();    // gitStatus 默认 mockResolvedValue([])
//     const notify = __createNotifyMocks(); // startWatch 默认 mockResolvedValue(undefined)
//     return {
//       get mockReadDir() { return fs.readDir; },
//       get mockGitStatus() { return git.gitStatus; },
//       get mockOnFsEvent() { return notify.onFsEvent; },
//       get triggerFsEvent() { return notify.triggerFsEvent; },
//       resetAll() { fs.readDir.mockReset(); git.gitStatus.mockReset(); notify.onFsEvent.mockClear(); },
//     };
//   });
//
//   vi.mock("../ipc/fs", () => ({ readDir: mocks.mockReadDir, createDir: vi.fn(), ... }));
//   vi.mock("../ipc/git", () => ({ gitStatus: mocks.mockGitStatus }));
//   vi.mock("../ipc/notify", () => ({ startWatch: vi.fn().mockResolvedValue(undefined), onFsEvent: mocks.mockOnFsEvent }));
//
// 如需自定义 mock 行为（如特定文件系统的 readDir 实现），在 beforeEach/it 中调用：
//   - makeVfs(mocks.mockReadDir, { "/root": [...] })  — 绑定虚拟文件系统
//   - mocks.mockReadDir.mockRejectedValue(new Error("..."))
//   - mocks.mockGitStatus.mockResolvedValue([...])
//
// 注意：resetAll() 使用 mockReset（清空 calls + implementation），之后需重新设置 mock 行为。

import { vi } from "vitest";

// ─── FS mocks ───

export interface FsMockOverrides {
  readDir?: ReturnType<typeof vi.fn>;
  readFile?: ReturnType<typeof vi.fn>;
  createDir?: ReturnType<typeof vi.fn>;
  deleteEntry?: ReturnType<typeof vi.fn>;
  rename?: ReturnType<typeof vi.fn>;
  writeFile?: ReturnType<typeof vi.fn>;
}

/** 创建 fs IPC mock 函数集。readDir 默认返回 [] 以避免 ExplorerPanel 挂载时未设置 mock 抛错。 */
export function createFsMocks(overrides?: FsMockOverrides) {
  return {
    readDir: overrides?.readDir ?? vi.fn().mockResolvedValue([]),
    readFile: overrides?.readFile ?? vi.fn(),
    createDir: overrides?.createDir ?? vi.fn(),
    deleteEntry: overrides?.deleteEntry ?? vi.fn(),
    rename: overrides?.rename ?? vi.fn(),
    writeFile: overrides?.writeFile ?? vi.fn(),
  };
}

// ─── Git mocks ───

export interface GitMockOverrides {
  gitStatus?: ReturnType<typeof vi.fn>;
}

/** 创建 git IPC mock 函数集。gitStatus 默认返回 [] 以避免 ExplorerPanel 挂载时未设置 mock 抛错。 */
export function createGitMocks(overrides?: GitMockOverrides) {
  return {
    gitStatus: overrides?.gitStatus ?? vi.fn().mockResolvedValue([]),
  };
}

// ─── Notify mocks ───

export interface NotifyMockOverrides {
  startWatch?: ReturnType<typeof vi.fn>;
  onFsEvent?: ReturnType<typeof vi.fn>;
}

/** 创建 notify IPC mock 函数集（含 triggerFsEvent 辅助）。startWatch 默认 resolve。 */
export function createNotifyMocks(overrides?: NotifyMockOverrides) {
  let fsEventCallback: (() => void) | null = null;

  const defaultOnFsEvent = vi.fn((cb: () => void) => {
    fsEventCallback = cb;
    return () => {
      fsEventCallback = null;
    };
  });

  const defaultStartWatch = vi.fn().mockResolvedValue(undefined);

  return {
    startWatch: overrides?.startWatch ?? defaultStartWatch,
    onFsEvent: overrides?.onFsEvent ?? defaultOnFsEvent,
    /** 手动触发 fs-event 回调（模拟后端文件变更通知） */
    triggerFsEvent() {
      fsEventCallback?.();
    },
  };
}
