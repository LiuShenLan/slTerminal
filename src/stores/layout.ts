// layout — 布局状态存储
//
// 只跟踪当前活动页面和布局切换锁，不持有布局数据（布局数据在 projects store）。

import { create } from "zustand";

interface LayoutState {
  /** 当前活动页面 ID */
  activePageId: string | null;
  /** 布局切换锁（防止并发切换） */
  isLayoutSwitching: boolean;
  setActivePage: (pageId: string | null) => void;
  setLayoutSwitching: (switching: boolean) => void;
}

export const useLayout = create<LayoutState>((set) => ({
  activePageId: null,
  isLayoutSwitching: false,
  setActivePage: (pageId) => set({ activePageId: pageId }),
  setLayoutSwitching: (switching) => set({ isLayoutSwitching: switching }),
}));
