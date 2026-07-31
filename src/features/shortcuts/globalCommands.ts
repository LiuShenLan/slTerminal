// globalCommands.ts — 全局快捷键命令工厂
//
// 注册独立于面板的全局快捷键（context: "global"），
// 在 App.tsx 挂载时注册一次，整个应用生命周期有效。
// 元数据（默认键/优先级）来自 commandCatalog，本文件只提供 handler。

import type { Command } from "./types";
import { commandFromMeta } from "./commandCatalog";
import type { DockviewApi } from "dockview-react";
import { useLayout } from "../../stores/layout";

/**
 * 创建全局快捷键命令列表。
 * @param getDockviewApi 获取当前活跃页面 DockviewApi 的函数
 */
export function createGlobalShortcuts(
  getDockviewApi: () => DockviewApi | undefined,
): Command[] {
  return [
    commandFromMeta("global.closeTab", () => {
      const api = getDockviewApi();
      const activePanel = api?.activePanel;
      if (activePanel) {
        activePanel.api.close();
        return true;
      }
      // 无活跃面板 → 透传（xterm.js 可接收 \x17 用于 bash readline）
      return false;
    }),
    commandFromMeta("global.openHooksConfig", () => {
      // 同页单例（契约 C13-7）：面板 id = hooksConfig-{activePageId}，命中聚焦、未命中新建
      const { activePageId } = useLayout.getState();
      if (!activePageId) return false; // 无活跃页面 → 透传
      const panelId = `hooksConfig-${activePageId}`;
      const api = getDockviewApi();
      if (!api) return false; // 无 DockviewApi → 透传
      const existing = api.getPanel(panelId);
      if (existing) {
        existing.focus();
        return true;
      }
      api.addPanel({
        id: panelId,
        component: "hooksConfig",
        title: "Hooks 配置",
        params: { panelId },
      });
      return true;
    }),
  ];
}
