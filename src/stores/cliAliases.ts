// CLI 启动别名状态管理 —— 用户配置层，IPC settings 持久化（settings 类 store 家族）
//
// 职责（仿 keybindings.ts 独立模式）：
// - 存储 aliases（cliId → 别名数组）
// - addAlias / removeAlias 操作（纯状态转换；语法/D3 唯一性校验在调用方先做——
//   校验纯函数归 cliProfiles 域 aliasValidation.ts，硬约束 #12）
// - loadFromDisk 从 settings.json 的 cliAliases 段恢复（sanitize 脏值：孤儿 cliId/
//   违例条目/重复——settings.json 可被手改或版本残留，后端纯透传不校验）
// - 变更后 2s debounce 自动保存（后端 save_settings 浅合并，只写 cliAliases 段）
//
// 别名消费：aliases 经 App.tsx 快照同步 effect 注入 cliProfileRegistry.setAliases
// （仿 wireKeybindings 编排），注册表 matchByCommand 内置未命中后查别名快照——
// 内存态即时生效（D4），不依赖 debounce 落盘完成。
// 依赖注记：loadFromDisk 的 sanitize 需 profile 已注册（孤儿判定）——生产由
// App 静态 import 链（Workspace → profiles/index.ts 注册）先于启动加载完成保证；
// 测试须先 registerMockCliProfile，否则全部键按孤儿丢弃。

import { create } from "zustand";
import { loadSettings, saveSettings } from "../ipc/settings";
import { toast, getErrorMessage } from "../lib";
import { sanitizeCliAliases } from "../features/cliProfiles/aliasValidation";
import { cliProfileRegistry } from "../features/cliProfiles/cliProfileRegistry";
import { PERSIST_DEBOUNCE_MS } from "./projects";

export interface CliAliasesState {
  /** cliId → 别名数组（删光后键保留为空数组，形态稳定防抖动） */
  aliases: Record<string, string[]>;
  loaded: boolean;
  /** 追加别名（调用方须先过 validateCliAlias；大小写敏感去重防御） */
  addAlias: (cliId: string, alias: string) => void;
  /** 移除别名 */
  removeAlias: (cliId: string, alias: string) => void;
  loadFromDisk: () => Promise<void>;
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

export const useCliAliases = create<CliAliasesState>((set, get) => ({
  aliases: {},
  loaded: false,

  addAlias: (cliId: string, alias: string) => {
    const current = get().aliases[cliId] ?? [];
    // 防御：store 是纯转换不重跑校验，但拒绝破坏不变量（同值重复）
    if (current.includes(alias)) return;
    set({
      aliases: {
        ...get().aliases,
        [cliId]: [...current, alias],
      },
    });
  },

  removeAlias: (cliId: string, alias: string) => {
    const current = get().aliases[cliId];
    if (current === undefined) return;
    const next = current.filter((a) => a !== alias);
    set({ aliases: { ...get().aliases, [cliId]: next } });
  },

  loadFromDisk: async () => {
    try {
      const { data: saved, corrupted } = await loadSettings();
      // FE-11：配置损坏已回退默认值，toast 告警
      if (corrupted) {
        toast.show("warning", "配置已损坏，已回退默认值");
      }
      if (saved) {
        const aliases = sanitizeCliAliases(
          saved.cliAliases,
          cliProfileRegistry.getAll(),
        );
        set({ aliases });
      }
    } catch (err) {
      // 首次启动或 IPC 失败，保持默认（空别名）
      console.warn("[slTerminal] cliAliases loadFromDisk 失败:", err);
    }
    set({ loaded: true });
  },
}));

// 持久化订阅：变更后 2s debounce 写入磁盘
// 后端 save_settings 浅合并 top-level 键，故只写 cliAliases 段不会擦除 fontSize 等
useCliAliases.subscribe((state) => {
  if (!state.loaded) return; // loaded 守卫：启动加载阶段不触发保存
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveSettings({ cliAliases: state.aliases }).catch((err) => {
      // FE-09：保存失败统一 toast 告警（设置未落盘，重启后将丢失）
      toast.show("warning", "设置保存失败，重启后将丢失");
      console.warn("[stores/cliAliases] 设置保存失败:", getErrorMessage(err));
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
