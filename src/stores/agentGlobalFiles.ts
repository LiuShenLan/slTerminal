// 「Agent 全局文件」视图展示配置状态管理 —— settings.json `agentGlobalFiles` 段
//（纯透传段，ADR-0014 先例；settings 类 store 家族模板照 cliAliases.ts）
//
// 职责：
// - 存储 configs（cliId → {mode, customNames, showRuntimeFiles}）
// - setMode / addCustomName / removeCustomName / setShowRuntimeFiles 纯状态转换
//   （名单校验在调用方先做——校验/净化纯函数归 features/agentFiles/filtering.ts，硬约束 #12）
// - loadFromDisk 从 settings.json 恢复（sanitize 脏值：孤儿 cliId/非法字段回退）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，只写 agentGlobalFiles 段）
//
// 依赖注记：loadFromDisk 的 sanitize 需 profile 已注册（孤儿判定）——生产由
// App 静态 import 链（Workspace → profiles/index.ts 注册）先于启动加载完成保证；
// 测试须先注册 profile，否则全部键按孤儿丢弃（同 cliAliases 先例）。

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import { toast, getErrorMessage } from "../lib";
import {
  defaultConfig,
  sanitizeAgentGlobalFiles,
} from "../features/agentFiles/filtering";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import type {
  AgentGlobalFilesMap,
  AgentGlobalFilesMode,
} from "../types/local";
import { PERSIST_DEBOUNCE_MS } from "./projects";

export interface AgentGlobalFilesState {
  /** cliId → 展示配置（未配置的 agent 缺省即 defaultConfig，消费方经 getConfig 回退） */
  configs: AgentGlobalFilesMap;
  loaded: boolean;
  /** 切换展示模式（all/custom）；无既有配置时从缺省起步 */
  setMode: (cliId: string, mode: AgentGlobalFilesMode) => void;
  /** 追加自定义名单项（调用方须先过 validateCustomName；重复防御照 cliAliases 先例） */
  addCustomName: (cliId: string, name: string) => void;
  /** 移除自定义名单项 */
  removeCustomName: (cliId: string, name: string) => void;
  /** 设置「显示运行时文件」开关（值持久保留；custom 模式下 UI 禁用不生效） */
  setShowRuntimeFiles: (cliId: string, show: boolean) => void;
  loadFromDisk: () => Promise<void>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** 取某 cliId 配置（无则缺省起步并入表——形态稳定，消费方无需判空分支） */
function ensureConfig(
  configs: AgentGlobalFilesMap,
  cliId: string,
): AgentGlobalFilesMap {
  if (configs[cliId]) return configs;
  return { ...configs, [cliId]: defaultConfig() };
}

export const useAgentGlobalFiles = create<AgentGlobalFilesState>((set, get) => ({
  configs: {},
  loaded: false,

  setMode: (cliId, mode) => {
    const configs = ensureConfig(get().configs, cliId);
    set({ configs: { ...configs, [cliId]: { ...configs[cliId], mode } } });
  },

  addCustomName: (cliId, name) => {
    const trimmed = name.trim();
    if (trimmed === "") return;
    const configs = ensureConfig(get().configs, cliId);
    const current = configs[cliId].customNames;
    // 防御：store 是纯转换不重跑校验，但拒绝破坏不变量（大小写不敏感重复）
    if (current.some((n) => n.toLowerCase() === trimmed.toLowerCase())) return;
    set({
      configs: {
        ...configs,
        [cliId]: { ...configs[cliId], customNames: [...current, trimmed] },
      },
    });
  },

  removeCustomName: (cliId, name) => {
    const current = get().configs[cliId];
    if (current === undefined) return;
    set({
      configs: {
        ...get().configs,
        [cliId]: {
          ...current,
          customNames: current.customNames.filter((n) => n !== name),
        },
      },
    });
  },

  setShowRuntimeFiles: (cliId, show) => {
    const configs = ensureConfig(get().configs, cliId);
    set({
      configs: {
        ...configs,
        [cliId]: { ...configs[cliId], showRuntimeFiles: show },
      },
    });
  },

  loadFromDisk: async () => {
    try {
      const { data: saved, corrupted } = await loadSettings();
      // FE-11：配置损坏已回退默认值，toast 告警
      if (corrupted) {
        toast.show("warning", "配置已损坏，已回退默认值");
      }
      if (saved) {
        const configs = sanitizeAgentGlobalFiles(
          saved.agentGlobalFiles,
          cliProfileRegistry.getAll(),
        );
        set({ configs });
      }
    } catch (err) {
      // 首次启动或 IPC 失败，保持默认（空配置表）
      console.warn("[slTerminal] agentGlobalFiles loadFromDisk 失败:", err);
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
// 后端 save_settings 浅合并 top-level 键，故只写 agentGlobalFiles 段不会擦除其它段
useAgentGlobalFiles.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveSettings({ agentGlobalFiles: state.configs }).catch((err) => {
      // FE-09：保存失败统一 toast 告警（设置未落盘，重启后将丢失）
      toast.show("warning", "设置保存失败，重启后将丢失");
      console.warn("[stores/agentGlobalFiles] 设置保存失败:", getErrorMessage(err));
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
