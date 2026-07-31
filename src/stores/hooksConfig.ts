// hooks 配置禁用状态管理 —— 单条启停禁用记录（ADR-0002），IPC settings 持久化
//
// 职责（仿 keybindings.ts 模式）：
// - 存储 disabledHooks（DisabledHookKey 四元组数组：层级 + 事件 + matcher + command）
// - disableHook / enableHook / isDisabled 操作
// - loadFromDisk 从 ~/.slterminal/settings.json 的 disabledHooks 段恢复（sanitize 脏数据）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，不覆盖 fontSize 等其他段）
//
// 注意：本 store 不在 App init 中加载，由 useHooksConfig 在面板挂载时调用 loadFromDisk。

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import type { DisabledHookKey, HooksLayer } from "../types/hooksConfig";
import { PERSIST_DEBOUNCE_MS } from "./projects";

const LAYERS: HooksLayer[] = ["user", "project", "local"];

/** sanitize：只保留四元组结构合法的元素，丢弃脏数据；matcher 缺失（undefined）归 null（全匹配语义） */
function sanitize(raw: unknown): DisabledHookKey[] {
  if (!Array.isArray(raw)) return [];
  const out: DisabledHookKey[] = [];
  for (const item of raw) {
    if (!item || typeof item !== "object" || Array.isArray(item)) continue;
    const { layer, event, matcher, command } = item as Record<string, unknown>;
    const m = matcher as string | null | undefined;
    if (
      LAYERS.includes(layer as HooksLayer) &&
      typeof event === "string" &&
      (m === undefined || m === null || typeof m === "string") &&
      typeof command === "string"
    ) {
      out.push({ layer: layer as HooksLayer, event, matcher: m ?? null, command });
    }
  }
  return out;
}

/** 四元组全等比较（isDisabled / enableHook 匹配依据） */
function sameKey(a: DisabledHookKey, b: DisabledHookKey): boolean {
  return (
    a.layer === b.layer &&
    a.event === b.event &&
    a.matcher === b.matcher &&
    a.command === b.command
  );
}

export interface HooksConfigState {
  /** 单条启停禁用记录（ADR-0002），空数组 = 全部启用 */
  disabledHooks: DisabledHookKey[];
  loaded: boolean;
  /** 禁用某条 hook；已存在则忽略 */
  disableHook: (key: DisabledHookKey) => void;
  /** 启用某条 hook；不存在则忽略 */
  enableHook: (key: DisabledHookKey) => void;
  /** 判断某条 hook 是否被禁用 */
  isDisabled: (key: DisabledHookKey) => boolean;
  loadFromDisk: () => Promise<void>;
  /** 立即写盘（debounce 自动保存调用；也可显式调用） */
  saveToDisk: () => Promise<void>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useHooksConfig = create<HooksConfigState>((set, get) => ({
  disabledHooks: [],
  loaded: false,

  disableHook: (key) => {
    if (!get().disabledHooks.some((k) => sameKey(k, key))) {
      set({ disabledHooks: [...get().disabledHooks, key] });
    }
  },

  enableHook: (key) => {
    set({ disabledHooks: get().disabledHooks.filter((k) => !sameKey(k, key)) });
  },

  isDisabled: (key) => get().disabledHooks.some((k) => sameKey(k, key)),

  loadFromDisk: async () => {
    try {
      const saved = await loadSettings();
      if (saved) {
        set({ disabledHooks: sanitize(saved.disabledHooks) });
      }
    } catch {
      // 首次启动或文件损坏，保持默认（空列表）
    }
    set({ loaded: true });
  },

  saveToDisk: async () => {
    await saveSettings({ disabledHooks: get().disabledHooks });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
// 后端 save_settings 浅合并 top-level 键，故只写 disabledHooks 段不会擦除 fontSize 等
useHooksConfig.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    useHooksConfig.getState().saveToDisk().catch(() => {
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
