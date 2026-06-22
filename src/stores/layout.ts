// layout — 布局状态存储
//
// 只跟踪当前活动页面 ID，不持有布局数据（布局数据在 projects store）。

import { create } from "zustand";

interface LayoutState {
  /** 当前活动页面 ID */
  activePageId: string | null;
  setActivePage: (pageId: string | null) => void;
}

export const useLayout = create<LayoutState>((set) => ({
  activePageId: null,
  setActivePage: (pageId) => set({ activePageId: pageId }),
}));
