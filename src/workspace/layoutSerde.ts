// layoutSerde — 布局序列化/反序列化
//
// 布局单点：操作页面布局只经此处用 Dockview toJSON/fromJSON 存取（硬约束 #7）。
// fromJSON 加 try/catch + 白名单校验（防 Dockview #341 损坏 API 状态）。

import type { DockviewApi } from "dockview-react";
import { isValidPanelType } from "./panelRegistry";

/** 从 Dockview API 导出布局 JSON */
export function saveLayout(api: DockviewApi): object {
  return api.toJSON();
}

/** 恢复布局到 Dockview，校验组件名白名单 */
export function loadLayout(
  api: DockviewApi,
  saved: object,
): void {
  try {
    // 白名单校验：过滤无效组件名
    const layout = JSON.parse(JSON.stringify(saved)) as Record<string, unknown>;
    if (layout.panels && typeof layout.panels === "object") {
      const panels = layout.panels as Record<string, { component?: string }>;
      for (const key of Object.keys(panels)) {
        const component = panels[key].component;
        if (component && !isValidPanelType(component)) {
          delete panels[key];
        }
      }
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    api.fromJSON(layout as any);
  } catch (err) {
    console.error("布局恢复失败:", err);
  }
}
