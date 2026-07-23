// keyboard.ts — 文件浏览器快捷键命令工厂
//
// 职责：返回 explorer 上下文的 Command[]，在 App.tsx 一次性注册（不随 explorer 实例增删）。
// handler 经 getActiveExplorer() 派发到**当前聚焦** explorer 实例。
// 元数据（默认键/优先级）来自 commandCatalog，本文件只提供 handler。

import type { Command } from "../shortcuts";
import { commandFromMeta } from "../shortcuts";
import { getActiveExplorer } from "./activeExplorer";

/**
 * 创建文件浏览器面板的快捷键命令列表（一次性注册，handler 派发到聚焦 explorer）。
 * 无聚焦 explorer 时 handler 返回 false（透传）。
 * 重命名 input 活跃时透传（让 input 处理 Enter/Escape）。
 */
export function createExplorerShortcuts(): Command[] {
  return [
    commandFromMeta("explorer.delete", () => {
      const e = getActiveExplorer();
      if (!e) return false;
      if (e.isRenaming()) return false; // 重命名中透传
      e.deleteSelected();
      return true;
    }),
    commandFromMeta("explorer.open", () => {
      const e = getActiveExplorer();
      if (!e) return false;
      if (e.isRenaming()) return false; // 重命名中透传给 input
      e.openSelected();
      return true;
    }),
    commandFromMeta("explorer.rename", () => {
      const e = getActiveExplorer();
      if (!e) return false;
      if (e.isRenaming()) return false; // 已在重命名不透传
      e.renameSelected();
      return true;
    }),
  ];
}
