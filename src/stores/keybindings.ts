// 快捷键自定义绑定状态管理 —— 用户覆盖层，IPC settings 持久化
//
// 职责（仿 fontSize.ts 独立模式）：
// - 存储 overrides（commandId → keystroke 字符串 | null 解绑）
// - setBinding / clearBinding / resetAll 操作
// - loadFromDisk 从 ~/.slterminal/settings.json 的 keybindings 段恢复（sanitize 脏值）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，不覆盖 fontSize 等其他段）
//
// overrides 经 App.tsx 的 wireKeybindings 注入 ShortcutRegistry 构建绑定表。

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import type { KeybindingOverrides } from "../features/shortcuts";
import { PERSIST_DEBOUNCE_MS } from "./projects";

export interface KeybindingsState {
  /** 用户覆盖层：commandId → keystroke 字符串 | null（解绑） */
  overrides: KeybindingOverrides;
  loaded: boolean;
  /** 重绑某命令到指定键；null 表示解绑 */
  setBinding: (id: string, keystroke: string | null) => void;
  /** 移除某命令的覆盖 → 回退默认键 */
  clearBinding: (id: string) => void;
  /** 清空全部覆盖 → 全部回退默认 */
  resetAll: () => void;
  loadFromDisk: () => Promise<void>;
}

/** sanitize：只保留值为 string | null 的项，丢弃脏数据（对象/数字/数组等） */
function sanitize(raw: unknown): KeybindingOverrides {
  const out: KeybindingOverrides = {};
  if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
      if (v === null || typeof v === "string") out[k] = v;
    }
  }
  return out;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useKeybindings = create<KeybindingsState>((set, get) => ({
  overrides: {},
  loaded: false,

  setBinding: (id: string, keystroke: string | null) => {
    set({ overrides: { ...get().overrides, [id]: keystroke } });
  },

  clearBinding: (id: string) => {
    const next = { ...get().overrides };
    delete next[id];
    set({ overrides: next });
  },

  resetAll: () => {
    set({ overrides: {} });
  },

  loadFromDisk: async () => {
    try {
      const saved = await loadSettings();
      if (saved) {
        set({ overrides: sanitize(saved.keybindings) });
      }
    } catch {
      // 首次启动或文件损坏，保持默认（空覆盖）
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
// 后端 save_settings 浅合并 top-level 键，故只写 keybindings 段不会擦除 fontSize 等
useKeybindings.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveSettings({ keybindings: state.overrides }).catch(() => {
      // 静默吞错，非关键数据
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
