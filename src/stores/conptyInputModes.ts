// ConPTY 输入模式能力矩阵状态管理（CP-009）——IPC settings 持久化，四开关段形态
//
// 职责：
// - 存储 ConptyInputModes 矩阵四字段（默认 = 后端现状三态等价：inheritCursor/
//   resizeQuirk/win32InputMode 均 true，passthroughMode false——零默认漂移）
// - loadFromDisk 从 settings.json 的 conptyInputModes 段恢复（段形态读取，段缺失
//   /非对象 → 字段缺省 → 默认值）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，写 conptyInputModes
//   段不擦 fontSize/keybindings/sideBar 等段）
//
// 字段值域（与后端 pty::spawn::ConptyInputModes 一一对应，Rust 单源导出至
// src/types/pty.ts）：
// - inheritCursor   ↔ 0x1 INHERIT_CURSOR
// - resizeQuirk     ↔ 0x2 RESIZE_QUIRK
// - win32InputMode  ↔ 0x4 WIN32_INPUT_MODE（后端仍按 build 门控，此处仅表达偏好）
// - passthroughMode ↔ 0x8 PASSTHROUGH_MODE（警示见设置页；开启须实测真实 claude 滚轮）

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import { toast, getErrorMessage } from "../lib";
import { PERSIST_DEBOUNCE_MS } from "./projects";
import type { ConptyInputModes } from "../types/pty";

/** 默认矩阵：与后端现状三态等价（Rust Default 同步口径，零漂移）。
 *  export 仅供本模块取值 + settings-conpty-input-modes 测试断言——生产消费方
 *  经 store state（加载后覆盖默认），无外部直 import，按合法导出登记
 *  knip.json ignoreIssues（2026-09-08）。 */
export const DEFAULT_CONPTY_INPUT_MODES: ConptyInputModes = {
  inheritCursor: true,
  resizeQuirk: true,
  win32InputMode: true,
  passthroughMode: false,
};

export interface ConptyInputModesState {
  modes: ConptyInputModes;
  loaded: boolean;
  /** 单开关切换（字段缺省合并，段保存由 debounce 订阅统一落盘） */
  setMode: (patch: Partial<ConptyInputModes>) => void;
  loadFromDisk: () => Promise<void>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useConptyInputModes = create<ConptyInputModesState>((set) => ({
  modes: DEFAULT_CONPTY_INPUT_MODES,
  loaded: false,

  setMode: (patch: Partial<ConptyInputModes>) => {
    set((s) => ({ modes: { ...s.modes, ...patch } }));
  },

  loadFromDisk: async () => {
    try {
      const { data: saved, corrupted } = await loadSettings();
      // FE-11：配置损坏已回退默认值，toast 告警
      if (corrupted) {
        toast.show("warning", "配置已损坏，已回退默认值");
      }
      if (saved) {
        // 段形态读取：settings.json 的 conptyInputModes 段（与 fontSize 等段先例一致）；
        // 段缺失/非对象 → 字段访问安全返回 undefined → 默认值（后端同口径零漂移）
        const section = saved.conptyInputModes as
          | Partial<ConptyInputModes>
          | undefined;
        const modes: ConptyInputModes = {
          inheritCursor:
            typeof section?.inheritCursor === "boolean"
              ? section.inheritCursor
              : DEFAULT_CONPTY_INPUT_MODES.inheritCursor,
          resizeQuirk:
            typeof section?.resizeQuirk === "boolean"
              ? section.resizeQuirk
              : DEFAULT_CONPTY_INPUT_MODES.resizeQuirk,
          win32InputMode:
            typeof section?.win32InputMode === "boolean"
              ? section.win32InputMode
              : DEFAULT_CONPTY_INPUT_MODES.win32InputMode,
          passthroughMode:
            typeof section?.passthroughMode === "boolean"
              ? section.passthroughMode
              : DEFAULT_CONPTY_INPUT_MODES.passthroughMode,
        };
        set({ modes });
      }
    } catch (err) {
      // 首次启动或 IPC 失败，保持默认值
      console.warn("[slTerminal] conptyInputModes loadFromDisk 失败:", err);
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
useConptyInputModes.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存

  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    // 段形态保存（SEC-11 白名单契约）：顶层键必须是 conptyInputModes 段名——
    // 后端白名单第 7 键（crate::pty::spawn::SETTINGS_KEY 引用，防字面量漂移）
    saveSettings({
      conptyInputModes: state.modes,
    }).catch((err) => {
      // FE-09：保存失败统一 toast 告警（设置未落盘，重启后将丢失）
      toast.show("warning", "设置保存失败，重启后将丢失");
      console.warn("[stores/conptyInputModes] 设置保存失败:", getErrorMessage(err));
    });
  }, PERSIST_DEBOUNCE_MS);
});

/** 取消待执行的 debounced 保存（关闭钩子中避免竞态，照 fontSize 先例） */
export function cancelPendingSave(): void {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}
