// 字体大小状态管理 —— 终端/编辑器独立字体大小，IPC settings 持久化
//
// 职责：
// - 存储 terminalFontSize / editorFontSize（默认 14，范围 [8, 32]）
// - setter 内部 clamp
// - loadFromDisk 从 ~/.slterminal/settings.json 恢复
// - 变更后 2s debounce 自动保存

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import { toast, getErrorMessage } from "../lib";
import { PERSIST_DEBOUNCE_MS } from "./projects";

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
      const { data: saved, corrupted } = await loadSettings();
      // FE-11：配置损坏已回退默认值，toast 告警
      if (corrupted) {
        toast.show("warning", "配置已损坏，已回退默认值");
      }
      if (saved) {
        const terminal = typeof saved.terminalFontSize === "number"
          ? clamp(saved.terminalFontSize) : FONT_SIZE_DEFAULT;
        const editor = typeof saved.editorFontSize === "number"
          ? clamp(saved.editorFontSize) : FONT_SIZE_DEFAULT;
        set({ terminalFontSize: terminal, editorFontSize: editor });
      }
    } catch {
      // 首次启动或 IPC 失败，保持默认值
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
    }).catch((err) => {
      // FE-09：保存失败统一 toast 告警（设置未落盘，重启后将丢失）；错误详情统一经 getErrorMessage
      toast.show("warning", "设置保存失败，重启后将丢失");
      console.warn("[stores/fontSize] 设置保存失败:", getErrorMessage(err));
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
