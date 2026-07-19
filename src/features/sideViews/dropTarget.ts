// 拖拽落点判定纯函数——不碰 DOM（禁止 document/window/getBoundingClientRect）

import type { Zone } from "./sideBarState";

/** 按钮矩形（调用方从 DOM getBoundingClientRect 提供，本函数不碰 DOM） */
export interface ButtonRect {
  id: string;
  top: number;
  height: number;
}

/** 落点结果 */
export interface DropTarget {
  zone: Zone;
  index: number;
}

/**
 * 根据鼠标 clientY 和按钮矩形列表计算拖拽落点
 * - clientY 在按钮上半 → 该按钮 index（插入其前方）
 * - clientY 在按钮下半 → index + 1（插入其后方）
 * - 空白区（无按钮或 clientY 在所有按钮之下） → 数组末尾
 *
 * buttonRects 应为目标 zone 的按钮列表（按当前排列顺序）
 */
export function computeDropTarget(
  clientY: number,
  buttonRects: ButtonRect[],
  zone: Zone
): DropTarget {
  for (let i = 0; i < buttonRects.length; i++) {
    const rect = buttonRects[i];
    const midY = rect.top + rect.height / 2;
    if (clientY < midY) {
      // 在按钮上半 → 插到该按钮前方
      return { zone, index: i };
    }
    // clientY >= midY → 继续检查下一个按钮
  }
  // 所有按钮下半或空白区 → 末尾
  return { zone, index: buttonRects.length };
}
