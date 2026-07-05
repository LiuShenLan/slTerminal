// globalCommands.ts — 全局快捷键命令工厂
//
// 注册独立于面板的全局快捷键（context: "global"），
// 在 App.tsx 挂载时注册一次，整个应用生命周期有效。

import type { ShortcutCommand } from "./types";
import type { DockviewApi } from "dockview-react";

/**
 * 创建全局快捷键命令列表。
 * @param getDockviewApi 获取当前活跃页面 DockviewApi 的函数
 */
export function createGlobalShortcuts(
  getDockviewApi: () => DockviewApi | undefined,
): ShortcutCommand[] {
  return [
    {
      id: "global.closeTab",
      keystroke: {
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
        code: "KeyW",
      },
      context: "global",
      priority: 10, // 低优先，面板级命令（priority 100+）可覆盖
      handler: () => {
        const api = getDockviewApi();
        const activePanel = api?.activePanel;
        if (activePanel) {
          activePanel.api.close();
          return true;
        }
        // 无活跃面板 → 透传（xterm.js 可接收 \x17 用于 bash readline）
        return false;
      },
    },
  ];
}
