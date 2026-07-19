// alignment.ts — diff 双栏占位对齐纯函数
//
// 根据 DiffHunk[] 计算左右两侧需要插入占位行的位置与数量。
// 零 DOM 访问，纯函数（照 dropTarget 模式），输入→输出无副作用。
//
// 返回的 Map key = 在该行**之后**插入占位，value = 占位行数。
// 规则：
//   纯新增（oldLines=0）→ 左侧插入 newLines 个占位
//   纯删除（newLines=0）→ 右侧插入 oldLines 个占位
//   modified oldLines > newLines → 右侧插入差值个
//   modified newLines > oldLines → 左侧插入差值个
//   等行修改 → 无需占位

import type { DiffHunk } from "../../types/git";

/** 占位对齐结果：left/right 各为 Map<在该行之后插入, 占位行数> */
export interface AlignmentResult {
  left: Map<number, number>;
  right: Map<number, number>;
}

/**
 * 从 DiffHunk[] 计算左右两侧占位对齐。
 *
 * 示例：
 *   - 纯新增 2 行（oldLines=0, newLines=2, newStart=3）→ left.set(2, 2)
 *   - 纯删除 3 行（oldLines=3, newLines=0, newStart=4）→ right.set(3, 3)
 *   - 修改 5→3 行（oldLines=5, newLines=3）→ right.set(newStart+2, 2)
 */
export function computeAlignment(hunks: DiffHunk[]): AlignmentResult {
  const left = new Map<number, number>();
  const right = new Map<number, number>();

  for (const hunk of hunks) {
    if (hunk.oldLines === 0) {
      // 纯新增：右侧有内容，左侧需要占位
      const key = hunk.newStart - 1;
      if (key >= 0) {
        left.set(key, (left.get(key) ?? 0) + hunk.newLines);
      }
    } else if (hunk.newLines === 0) {
      // 纯删除：左侧有内容，右侧需要占位
      const key = hunk.newStart - 1;
      if (key >= 0) {
        right.set(key, (right.get(key) ?? 0) + hunk.oldLines);
      }
    } else if (hunk.oldLines > hunk.newLines) {
      // 修改，old 行数更多：右侧需要占位（在 new 最后一行的后面）
      const key = hunk.newStart + hunk.newLines - 1;
      const diff = hunk.oldLines - hunk.newLines;
      right.set(key, (right.get(key) ?? 0) + diff);
    } else if (hunk.newLines > hunk.oldLines) {
      // 修改，new 行数更多：左侧需要占位（在 old 最后一行的后面）
      const key = hunk.oldStart + hunk.oldLines - 1;
      const diff = hunk.newLines - hunk.oldLines;
      left.set(key, (left.get(key) ?? 0) + diff);
    }
    // oldLines === newLines: 无需占位
  }

  return { left, right };
}
