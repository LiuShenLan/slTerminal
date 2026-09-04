// globalCommands.ts — 全局快捷键命令工厂
//
// 注册独立于面板的全局快捷键（context: "global"），
// 在 App.tsx 挂载时注册一次，整个应用生命周期有效。
// 元数据（默认键/优先级）来自 commandCatalog，本文件只提供 handler。
// 全局命令仅 global.closeTab——Hooks 配置入口已迁移到活动栏「配置」钮
// （openSettings，见 features/settingsCenter/openSettings.ts）。

import type { Command } from "./types";
import { commandFromMeta } from "./commandCatalog";
import type { DockviewApi } from "dockview-react";
// 页签关闭守卫统一入口（FE-49）：Ctrl+W 曾直调 activePanel.api.close() 绕过
// settings dirty 确认（× 守卫 F11/SC-FE-07 登记的不对称）——现与 ×/中键/右键
// 菜单「关闭」同走 closeTabGuarded；tabClose 无 React 依赖，shortcuts 层可安全引用
import { closeTabGuarded } from "../../workspace/tabClose";

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
        // 守卫（确认）为异步：handler 仍同步消费按键（返回 true），关闭动作
        // 在 guard 决议后执行——非 dirty 面板零延迟差异（microtask 边界）
        void closeTabGuarded(activePanel.api, activePanel.id);
        return true;
      }
      // 无活跃面板 → 透传（xterm.js 可接收 \x17 用于 bash readline）
      return false;
    }),
  ];
}
