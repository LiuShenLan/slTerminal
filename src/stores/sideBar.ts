// 侧栏视图状态管理 —— 活动栏+侧栏区持久化 store
//
// 职责（仿 fontSize.ts 独立持久化模式）：
// - 存储 zones/open/width/splitRatio（SideBarSlice）
// - toggleView/moveButton 委托 sideBarState 纯函数
// - setWidth/setSplitRatio 内部 clamp
// - loadFromDisk 从 ~/.slterminal/settings.json 的 sideBar 段恢复
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，不覆盖 fontSize/keybindings 段）
//
// 硬约束 #1：不 import invoke / @tauri-apps/api——只经 ../ipc/settings

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import { PERSIST_DEBOUNCE_MS } from "./projects";
import {
  type Zone,
  type Zones,
  type OpenState,
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  WIDTH_DEFAULT,
  WIDTH_MIN,
  WIDTH_MAX,
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
  toggleViewPure,
  moveButtonPure,
  reconcileZones,
  sanitizeSideBar,
} from "../features/sideViews/sideBarState";
import { sideViewRegistry } from "../features/sideViews/sideViewRegistry";

// ── 类型 ──

/** 侧栏 store 完整状态（继承持久化 slice + 操作方法） */
export interface SideBarState {
  zones: Zones;
  open: OpenState;
  width: number;
  splitRatio: number;
  loaded: boolean;
  /** 切换视图开关（委托 toggleViewPure） */
  toggleView: (id: string) => void;
  /** 移动按钮归属（委托 moveButtonPure） */
  moveButton: (id: string, zone: Zone, index: number) => void;
  /** 设置侧栏区宽度，内部 clamp 到 [WIDTH_MIN, WIDTH_MAX] */
  setWidth: (w: number) => void;
  /** 设置上下区分割比例，内部 clamp 到 [SPLIT_MIN, SPLIT_MAX] */
  setSplitRatio: (r: number) => void;
  /** 从磁盘恢复持久化状态 */
  loadFromDisk: () => Promise<void>;
}

// ── 内部辅助 ──

/** 将值 clamp 到 [min, max]，非有限数回退 min */
function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, value));
}

// ── 持久化 timer（模块级，仿 fontSize.ts） ──

let saveTimer: ReturnType<typeof setTimeout> | null = null;

// ── Store ──

export const useSideBar = create<SideBarState>((set, get) => ({
  // 默认值取自契约 [A] 常量
  zones: { ...DEFAULT_ZONES },
  open: { ...DEFAULT_OPEN },
  width: WIDTH_DEFAULT,
  splitRatio: SPLIT_DEFAULT,
  loaded: false,

  toggleView: (id: string) => {
    const { zones, open } = get();
    const newOpen = toggleViewPure(zones, open, id);
    set({ open: newOpen });
  },

  moveButton: (id: string, zone: Zone, index: number) => {
    const { zones, open } = get();
    const result = moveButtonPure(zones, open, id, zone, index);
    set({ zones: result.zones, open: result.open });
  },

  setWidth: (w: number) => {
    set({ width: clamp(w, WIDTH_MIN, WIDTH_MAX) });
  },

  setSplitRatio: (r: number) => {
    set({ splitRatio: clamp(r, SPLIT_MIN, SPLIT_MAX) });
  },

  loadFromDisk: async () => {
    try {
      const saved = await loadSettings();
      if (saved) {
        // sanitizeSideBar 处理原始数据——非法/缺失回退默认值 + clamp
        const slice = sanitizeSideBar(saved["sideBar"]);
        // reconcileZones 对齐注册表——过滤未注册 id + 补全缺失按钮
        const registeredIds = sideViewRegistry.getAll().map((d) => d.id);
        const reconciled = reconcileZones(
          slice.zones,
          slice.open,
          registeredIds,
        );
        set({
          zones: reconciled.zones,
          open: reconciled.open,
          width: slice.width,
          splitRatio: slice.splitRatio,
        });
      }
    } catch {
      // 首次启动或文件损坏，保持默认值
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
// 后端 save_settings 浅合并 top-level 键，故只写 sideBar 段不擦 fontSize/keybindings 等
useSideBar.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveSettings({
      sideBar: {
        zones: state.zones,
        open: state.open,
        width: state.width,
        splitRatio: state.splitRatio,
      },
    }).catch(() => {
      // 静默吞错，侧栏状态非关键数据
    });
  }, PERSIST_DEBOUNCE_MS);
});

/** 取消待执行的 debounced 保存（关闭钩子中避免竞态） */
export function cancelPendingSave(): void {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}
