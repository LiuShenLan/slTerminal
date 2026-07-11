// wireKeybindings.ts — 把用户覆盖层接线到注册表
//
// 抽成独立纯 helper（而非埋在 App.tsx）以便单测：不依赖 React 渲染，
// 只需 registry.setOverrides 与一个提供 getState/subscribe 的 store。
//
// 语义：立即应用一次当前覆盖（覆盖"加载已完成"场景），并订阅后续变更重新应用。

import type { KeybindingOverrides, ShortcutRegistryAPI } from "./types";

/** wireKeybindings 所需的最小 store 结构（Zustand store 天然满足） */
interface OverridesStore {
  getState(): { overrides: KeybindingOverrides };
  subscribe(listener: () => void): () => void;
}

/**
 * 将 store.overrides 持续同步到 registry。
 * @returns 取消订阅函数
 */
export function wireKeybindings(
  registry: Pick<ShortcutRegistryAPI, "setOverrides">,
  store: OverridesStore,
): () => void {
  const apply = () => registry.setOverrides(store.getState().overrides);
  apply(); // 立即应用（load 可能已完成）
  return store.subscribe(apply); // load set() + 未来编辑时重新应用
}
