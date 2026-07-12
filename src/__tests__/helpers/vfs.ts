// helpers/vfs.ts — 虚拟文件系统测试辅助
//
// 消除 explorer-refresh-preserve.test.tsx 和 explorer-rootpath-clear.test.tsx
// 中重复的 makeVfs / mockEntry / findNode 定义（各约 30 行）。
//
// 用法示例：
//
//   import { makeVfs, mockEntry, findNode } from "./helpers/vfs";
//   import type { DirEntry } from "../types/fs";
//
//   const vfs = makeVfs(readDirMock, {
//     "/root": [mockEntry("file.ts", false, "/root/file.ts")],
//   });
//   // 模拟磁盘变更
//   vfs.set("/root", [...vfs.get("/root")!, mockEntry("new.ts", false, "/root/new.ts")]);

import type { DirEntry } from "../../types/fs";

/** 创建模拟 DirEntry */
export function mockEntry(name: string, isDir: boolean, path: string): DirEntry {
  return {
    name,
    path,
    isDir,
    ...(isDir ? {} : { size: 1024, modified: 1 }),
  };
}

/**
 * 虚拟文件系统：Map<dirPath, DirEntry[]>。
 * 将 mockReadDir 实现绑定到 Map，按传入 dirPath 分派对应子项；
 * 测试可动态增删 entries 模拟磁盘变更。
 */
export function makeVfs(
  mockReadDir: ReturnType<typeof import("vitest").vi.fn>,
  initial: Record<string, DirEntry[]>,
): Map<string, DirEntry[]> {
  const vfs = new Map<string, DirEntry[]>(Object.entries(initial));
  mockReadDir.mockImplementation(async (dirPath: string) => {
    if (!vfs.has(dirPath)) throw new Error(`ENOENT: ${dirPath}`);
    return vfs.get(dirPath)!;
  });
  return vfs;
}

/** TreeNode 最小接口（用于 findNode 递归遍历） */
export interface TreeNodeLike {
  entry: { name: string; path: string; isDir: boolean };
  children: TreeNodeLike[];
}

/** 在树节点数组中按 path 查找节点（递归） */
export function findNode<T extends TreeNodeLike>(nodes: T[], path: string): T | undefined {
  for (const n of nodes) {
    if (n.entry.path === path) return n;
    const found = findNode(n.children as T[], path);
    if (found) return found;
  }
  return undefined;
}
