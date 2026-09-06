// persistPanelParams — 面板 params 持久化共享辅助（docViewer 面板用）
//
// SettingsPanel.persistParams 先例的通用化：文件面板 panelId 无 settings- 前缀
// （pageId 不可从 panelId 解出），交互必发生在活跃页——pageId 从 useLayout
// 现取（openFileInActivePage 同源近似，workspace/CLAUDE.md「__dockviewApi 重指
// 不变量」论证）。
//
// 语义 = updateParameters(patch) + 显式布局落盘：updateParameters 不触发
// onDidLayoutChange，必须经 saveLayout 显式保存（F8/SettingsPanel 先例）；
// 布局 JSON 本身仍只经 layoutSerde.saveLayout 序列化（布局单点 #7 不破）。

import type { DockviewApi } from "dockview-react";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { saveLayout } from "./layoutSerde";

/**
 * 更新面板 params 并显式持久化布局。
 *
 * @param containerApi Dockview 容器 API（布局序列化源）
 * @param panelApi 目标面板 API（updateParameters 写入者）
 * @param patch 要合并写入的 params 增量
 */
export function persistPanelParams(
  containerApi: DockviewApi,
  panelApi: { updateParameters(patch: Record<string, unknown>): void },
  patch: Record<string, unknown>,
): void {
  panelApi.updateParameters(patch);

  // pageId 现取（面板交互发生在活跃页；无活跃页 = 启动瞬态，跳过落盘无害——
  // 布局 JSON 恢复时 params 已含旧值）
  const activePageId = useLayout.getState().activePageId;
  if (!activePageId) return;
  const { projects } = useProjects.getState();
  for (const [projId, proj] of Object.entries(projects)) {
    if (proj.pages.some((p) => p.pageId === activePageId)) {
      useProjects
        .getState()
        .updatePageLayout(projId, activePageId, saveLayout(containerApi) as Record<string, unknown>);
      break;
    }
  }
}
