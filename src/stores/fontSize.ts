// 字体大小状态管理 —— 终端/编辑器独立字体大小，IPC settings 持久化
//
// 职责：
// - 存储 terminalFontSize / editorFontSize（默认 14，范围 [8, 32]）
// - setter 内部 clamp
// - loadFromDisk 从 ~/.slterminal/settings.json 恢复
// - 变更后 2s debounce 自动保存

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";

/** 字体大小范围 */
export const FONT_SIZE_MIN = 8;
export const FONT_SIZE_MAX = 32;
/** 默认字体大小 */
export const FONT_SIZE_DEFAULT = 14;

function clamp(size: number): number {
  return Math.max(FONT_SIZE_MIN, Math.min(FONT_SIZE_MAX, Math.round(size)));
}

export interface FontSizeState {
  terminalFontSize: number;
  editorFontSize: number;
  loaded: boolean;
  setTerminalFontSize: (size: number) => void;
  setEditorFontSize: (size: number) => void;
  loadFromDisk: () => Promise<void>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useFontSize = create<FontSizeState>((set) => ({
  terminalFontSize: FONT_SIZE_DEFAULT,
  editorFontSize: FONT_SIZE_DEFAULT,
  loaded: false,

  setTerminalFontSize: (size: number) => {
    const clamped = clamp(size);
    set({ terminalFontSize: clamped });
  },

  setEditorFontSize: (size: number) => {
    const clamped = clamp(size);
    set({ editorFontSize: clamped });
  },

  loadFromDisk: async () => {
    try {
      const saved = await loadSettings();
      if (saved) {
        const terminal = typeof saved.terminalFontSize === "number"
          ? clamp(saved.terminalFontSize) : FONT_SIZE_DEFAULT;
        const editor = typeof saved.editorFontSize === "number"
          ? clamp(saved.editorFontSize) : FONT_SIZE_DEFAULT;
        set({ terminalFontSize: terminal, editorFontSize: editor });
      }
    } catch {
      // 首次启动或文件损坏，保持默认值
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
useFontSize.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存

  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveSettings({
      terminalFontSize: state.terminalFontSize,
      editorFontSize: state.editorFontSize,
    }).catch(() => {
      // 静默吞错，fontsSize 非关键数据
    });
  }, 2000);
});
