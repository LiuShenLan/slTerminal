// diff-alignment.test.ts — computeAlignment 纯函数全分支测试
//
// 覆盖：纯增/纯删/等行修改/old>new/new>old/多 hunk/空/同位置合并

import { describe, it, expect } from "vitest";
import { computeAlignment } from "../panels/diff/alignment";
import type { DiffHunk } from "../types/git";

/** 构造 DiffHunk 辅助 */
function h(
  oldStart: number,
  oldLines: number,
  newStart: number,
  newLines: number,
): DiffHunk {
  return { oldStart, oldLines, newStart, newLines };
}

/** 将 Map 转为排序后的键值对数组便于断言 */
function sorted(m: Map<number, number>): [number, number][] {
  return [...m.entries()].sort((a, b) => a[0] - b[0]);
}

describe("computeAlignment", () => {
  // ── 纯新增 ──────────────────────────────────────────

  it("纯新增 2 行——左侧在 newStart-1 之后插入占位", () => {
    // HEAD: 1 2 3 4 5, workdir: 1 2 A B 3 4 5
    // hunk: oldLines=0, newLines=2, newStart=3
    const result = computeAlignment([h(3, 0, 3, 2)]);
    expect(sorted(result.left)).toEqual([[2, 2]]);
    expect(result.right.size).toBe(0);
  });

  it("纯新增 1 行在文件头——左侧 key=0 之后插入", () => {
    // HEAD: 1 2 3, workdir: A 1 2 3
    const result = computeAlignment([h(1, 0, 1, 1)]);
    expect(sorted(result.left)).toEqual([[0, 1]]);
    expect(result.right.size).toBe(0);
  });

  it("多个纯新增 hunk——同位置占位合并", () => {
    // 同一位置的两个新增 hunk（连续）
    const result = computeAlignment([h(3, 0, 3, 2), h(3, 0, 3, 3)]);
    expect(sorted(result.left)).toEqual([[2, 5]]); // 2+3 = 5
    expect(result.right.size).toBe(0);
  });

  // ── 纯删除 ──────────────────────────────────────────

  it("纯删除 3 行——右侧在 newStart-1 之后插入占位", () => {
    // HEAD: 1 2 D D D 3 4, workdir: 1 2 3 4
    // hunk: oldLines=3, newLines=0, newStart=3
    const result = computeAlignment([h(3, 3, 3, 0)]);
    expect(result.left.size).toBe(0);
    expect(sorted(result.right)).toEqual([[2, 3]]);
  });

  it("纯删除 2 行在文件头——右侧 key=0 之后插入", () => {
    // HEAD: D D 1 2 3, workdir: 1 2 3
    const result = computeAlignment([h(1, 2, 1, 0)]);
    expect(result.left.size).toBe(0);
    expect(sorted(result.right)).toEqual([[0, 2]]);
  });

  // ── 等行修改 ────────────────────────────────────────

  it("等行修改（oldLines === newLines）——无占位", () => {
    const result = computeAlignment([h(3, 4, 3, 4)]);
    expect(result.left.size).toBe(0);
    expect(result.right.size).toBe(0);
  });

  // ── oldLines > newLines ─────────────────────────────

  it("oldLines > newLines（5→3）——右侧在 new 最后行之后插入差值", () => {
    // oldStart=3, oldLines=5, newStart=3, newLines=3
    // 右侧 new 行号 3,4,5，old 还有 6,7（HEAD 多 2 行）
    // 右侧 key = newStart + newLines - 1 = 5, value = 2
    const result = computeAlignment([h(3, 5, 3, 3)]);
    expect(result.left.size).toBe(0);
    expect(sorted(result.right)).toEqual([[5, 2]]);
  });

  it("oldLines > newLines（3→1）", () => {
    const result = computeAlignment([h(5, 3, 5, 1)]);
    expect(result.left.size).toBe(0);
    expect(sorted(result.right)).toEqual([[5, 2]]); // key = 5+1-1 = 5
  });

  // ── newLines > oldLines ─────────────────────────────

  it("newLines > oldLines（3→5）——左侧在 old 最后行之后插入差值", () => {
    // oldStart=3, oldLines=3, newStart=3, newLines=5
    // 左侧 old 行号 3,4,5，右侧 new 还有 6,7
    // 左侧 key = oldStart + oldLines - 1 = 5, value = 2
    const result = computeAlignment([h(3, 3, 3, 5)]);
    expect(sorted(result.left)).toEqual([[5, 2]]);
    expect(result.right.size).toBe(0);
  });

  it("newLines > oldLines（1→4）", () => {
    const result = computeAlignment([h(7, 1, 7, 4)]);
    expect(sorted(result.left)).toEqual([[7, 3]]); // key = 7+1-1 = 7
    expect(result.right.size).toBe(0);
  });

  // ── 多 hunk 混合 ────────────────────────────────────

  it("多 hunk 混合——两侧均有占位", () => {
    // hunk 1: 纯新增 2 行 at newStart=3 → left key=2, value=2
    // hunk 2: 纯删除 1 行 at newStart=5 → right key=4, value=1
    // hunk 3: oldLines=4 > newLines=2, newStart=7 → right key=8, value=2
    const hunks = [
      h(3, 0, 3, 2),   // 纯增
      h(5, 1, 5, 0),   // 纯删
      h(7, 4, 7, 2),   // old>new
    ];
    const result = computeAlignment(hunks);
    expect(sorted(result.left)).toEqual([[2, 2]]);
    expect(sorted(result.right)).toEqual([
      [4, 1],
      [8, 2],
    ]);
  });

  it("多 hunk 混合——含等行修改", () => {
    const hunks = [
      h(3, 2, 3, 2),   // 等行（无占位）
      h(5, 0, 5, 3),   // 新增 3 → left key=4, 3
      h(8, 2, 8, 1),   // old>new → right key=8, 1
    ];
    const result = computeAlignment(hunks);
    expect(sorted(result.left)).toEqual([[4, 3]]);
    expect(sorted(result.right)).toEqual([[8, 1]]);
  });

  // ── 边界 ────────────────────────────────────────────

  it("空 hunks 返回空 Map", () => {
    const result = computeAlignment([]);
    expect(result.left.size).toBe(0);
    expect(result.right.size).toBe(0);
  });

  it("newStart=1 且 oldLines=0 → left key=0", () => {
    // 文件头新增
    const result = computeAlignment([h(1, 0, 1, 4)]);
    expect(sorted(result.left)).toEqual([[0, 4]]);
  });

  it("newStart=1 且 newLines=0 → right key=0", () => {
    // 文件头删除
    const result = computeAlignment([h(1, 3, 1, 0)]);
    expect(sorted(result.right)).toEqual([[0, 3]]);
  });

  it("同位置 left 和 right 各有占位——独立不混淆", () => {
    // 同一个 newStart=3: left 新增 + right 删除
    const hunks = [
      h(3, 0, 3, 2),  // left key=2, 2
      h(3, 1, 3, 0),  // right key=2, 1
    ];
    const result = computeAlignment(hunks);
    expect(sorted(result.left)).toEqual([[2, 2]]);
    expect(sorted(result.right)).toEqual([[2, 1]]);
  });
});
