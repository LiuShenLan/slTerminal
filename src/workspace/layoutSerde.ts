// layoutSerde — 布局序列化/反序列化
//
// 布局单点：操作页面布局只经此处用 Dockview toJSON/fromJSON 存取（硬约束 #7）。
// fromJSON 前修补旧格式（缺少 leaf.data.id / activeGroup），返回 boolean 供调用方回退。

import type { DockviewApi } from "dockview-react";
import { isValidPanelType } from "../panelRegistry";

/** 从 Dockview API 导出布局 JSON */
export function saveLayout(api: DockviewApi): object {
  return api.toJSON();
}

/**
 * 修补旧版布局格式（makeDefaultLayout 早期版本可能缺少字段）
 *
 * 旧格式特征：leaf.data 缺少 id（group id），顶层缺少 activeGroup。
 * 新格式要求：leaf.data.id 必须为 string，activeGroup 必须匹配。
 */
function patchLegacyLayout(layout: Record<string, unknown>): void {
  try {
    // 修补 panels 中的 component→contentComponent 迁移
    if (layout.panels && typeof layout.panels === "object") {
      const panels = layout.panels as Record<string, Record<string, unknown>>;
      for (const key of Object.keys(panels)) {
        const panel = panels[key];
        // 旧格式用 component，新格式用 contentComponent
        if (panel.component && !panel.contentComponent) {
          panel.contentComponent = panel.component;
          delete panel.component;
        }
      }
    }

    // 修补 grid.root：leaf 节点缺少 id 则补上
    const grid = layout.grid as Record<string, unknown> | undefined;
    if (!grid || !grid.root) return;

    // 修补 grid 缺少 orientation → 默认 HORIZONTAL
    if (typeof grid.orientation !== "string") {
      grid.orientation = "HORIZONTAL";
    }

    const root = grid.root as Record<string, unknown>;
    if (root.type !== "branch" || !Array.isArray(root.data)) return;

    const leaves = root.data as Record<string, unknown>[];
    for (const leaf of leaves) {
      if (leaf.type !== "leaf") continue;
      const leafData = leaf.data as Record<string, unknown> | undefined;
      if (!leafData) continue;

      // 缺少 id → 根据 views[0] 生成
      if (typeof leafData.id !== "string") {
        const views = leafData.views as string[] | undefined;
        leafData.id = views?.[0] ? `group-${views[0]}` : "group-default";
      }

      // 顶层缺少 activeGroup → 用第一个 leaf 的 id
      if (typeof layout.activeGroup !== "string") {
        layout.activeGroup = leafData.id;
      }
    }
  } catch {
    // 修补失败不阻塞，fromJSON 会自然报错
  }
}

/**
 * 恢复布局到 Dockview，返回是否成功
 *
 * 失败原因包括：旧格式不兼容、组件白名单过滤后无剩余面板、Dockview 内部异常。
 * 调用方应在失败时回退到默认布局（创建默认终端）。
 */
export function loadLayout(
  api: DockviewApi,
  saved: object,
): boolean {
  try {
    // 深度拷贝（避免修改原始 store 数据）
    const layout = JSON.parse(JSON.stringify(saved)) as Record<string, unknown>;

    // 白名单校验：过滤无效组件名
    if (layout.panels && typeof layout.panels === "object") {
      const panels = layout.panels as Record<string, { component?: string; contentComponent?: string }>;
      for (const key of Object.keys(panels)) {
        const id = panels[key].contentComponent ?? panels[key].component;
        if (id && !isValidPanelType(id)) {
          delete panels[key];
        }
      }
    }

    // 修补旧格式（缺少 leaf.data.id / activeGroup）
    patchLegacyLayout(layout);

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    api.fromJSON(layout as any, { reuseExistingPanels: true });
    return true;
  } catch (err) {
    console.error("布局恢复失败:", err);
    return false;
  }
}
