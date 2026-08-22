// 字体大小状态管理 —— 终端/编辑器独立字体大小，IPC settings 持久化
//
// 职责：
// - 存储 terminalFontSize / editorFontSize（默认 14，范围 [8, 32]）
// - setter 内部 clamp
// - loadFromDisk 从 settings.json 的 fontSize 段恢复（exe 同级应用数据目录）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，写 fontSize 段不擦 keybindings/sideBar 段）

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
        // 段形态读取：settings.json 的 fontSize 段（与 sideBar/keybindings 各写各的段一致）；
        // 段缺失/非对象（section 为 null/string/数组等）→ 字段访问安全返回 undefined → 默认值
        const section = saved.fontSize as
          | { terminalFontSize?: unknown; editorFontSize?: unknown }
          | undefined;
        const terminal = typeof section?.terminalFontSize === "number"
          ? clamp(section.terminalFontSize) : FONT_SIZE_DEFAULT;
        const editor = typeof section?.editorFontSize === "number"
          ? clamp(section.editorFontSize) : FONT_SIZE_DEFAULT;
        set({ terminalFontSize: terminal, editorFontSize: editor });
      }
    } catch (err) {
      // 首次启动或 IPC 失败，保持默认值
      console.warn("[slTerminal] fontSize loadFromDisk 失败:", err);
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
useFontSize.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存

  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    // 段形态保存（SEC-11 白名单契约）：顶层键必须是 fontSize 段名——
    // 平铺 terminalFontSize/editorFontSize 会被后端白名单拒绝（契约断链先例已用测试锁死）
    saveSettings({
      fontSize: {
        terminalFontSize: state.terminalFontSize,
        editorFontSize: state.editorFontSize,
      },
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
