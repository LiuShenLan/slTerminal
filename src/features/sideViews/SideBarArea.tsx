// SideBarArea — 侧栏区组件
//
// 活动栏与主区之间的共享展示区域，垂直划分为上区与下区两个半区。
// 每半区一槽位，视图通过 display:none/flex 切换保挂载。
// 换区重建为已知行为（ADR-0001）：组件随 zones 跨 pane 移动即卸载重建。

import React, { useEffect, useRef } from "react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import { useSideBar } from "../../stores/sideBar";
import { sideViewRegistry } from "./sideViewRegistry";
import {
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
} from "./sideBarState";
import { PANEL_BG } from "../../theme/colors";

/** SideBarArea 外部注入 props——与 SidebarTree props 精确匹配 */
export interface SideBarAreaProps {
  /** 切换操作页面（async——切换完成后再开面板） */
  switchToPage: (projectId: string, pageId: string) => Promise<void>;
  /** 删除操作页面 */
  onDeletePage: (projectId: string, pageId: string) => void;
}

/**
 * 侧栏区组件
 *
 * 结构：<Allotment vertical proportionalLayout> 两 pane
 * - 上 pane visible={!!open.top} preferredSize={splitRatio * 100}
 * - 下 pane visible={!!open.bottom} preferredSize={(1 - splitRatio) * 100}
 * - 每 pane 内：zones[zone].map(id → registry.get(id)) 过滤 undefined →
 *   每视图一槽 display: open==id ? "flex" : "none"，height: 100%
 * - onChange 仅双开时换算 ratio 写回 store
 *
 * 硬约束 #6：全部颜色引用 theme/colors.ts token，禁止硬编码色值
 */
export const SideBarArea: React.FC<SideBarAreaProps> = ({
  switchToPage,
  onDeletePage,
}) => {
  const zones = useSideBar((s) => s.zones);
  const open = useSideBar((s) => s.open);
  const splitRatio = useSideBar((s) => s.splitRatio);
  const setSplitRatio = useSideBar((s) => s.setSplitRatio);

  // 各半区已注册视图定义（过滤持久化中可能已取消注册的 id）
  const topDefs = zones.top
    .map((id) => sideViewRegistry.get(id))
    .filter((d): d is NonNullable<typeof d> => d !== undefined);

  const bottomDefs = zones.bottom
    .map((id) => sideViewRegistry.get(id))
    .filter((d): d is NonNullable<typeof d> => d !== undefined);

  const topOpen = open.top !== null;
  const bottomOpen = open.bottom !== null;
  const bothOpen = topOpen && bottomOpen;

  // 从单视图过渡到双视图时，仅当 splitRatio 为默认值（无持久化值，首次进入双视图）
  // 或越界（出 [SPLIT_MIN, SPLIT_MAX]）才回退默认 0.5；
  // 用户调节过的合法比例在正常单↔双切换中保留（FE-19）
  const prevBothOpen = useRef(bothOpen);
  useEffect(() => {
    if (bothOpen && !prevBothOpen.current) {
      const outOfRange =
        splitRatio < SPLIT_MIN || splitRatio > SPLIT_MAX;
      const noPersistedValue = splitRatio === SPLIT_DEFAULT;
      if (outOfRange || noPersistedValue) {
        setSplitRatio(SPLIT_DEFAULT);
      }
    }
    prevBothOpen.current = bothOpen;
  }, [bothOpen, splitRatio, setSplitRatio]);

  return (
    <div
      style={{
        height: "100%",
        background: PANEL_BG,
      }}
    >
      <Allotment
        vertical
        proportionalLayout={true}
        onChange={(sizes) => {
          // 仅双开时换算 ratio 写回——单开时另一 pane size 为 0，除零守卫
          if (!bothOpen || sizes.length < 2) return;
          const total = sizes[0] + sizes[1];
          if (total <= 0) return;
          const ratio = sizes[0] / total;
          setSplitRatio(ratio);
        }}
      >
        {/* 上区 pane */}
        <Allotment.Pane
          visible={topOpen}
          preferredSize={splitRatio * 100}
        >
          <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
            {topDefs.map((def) => (
              <div
                key={def.id}
                style={{
                  display: open.top === def.id ? "flex" : "none",
                  height: "100%",
                  flexDirection: "column",
                }}
                data-e2e={`sidebar-slot-top-${def.id}`}
              >
                <def.component
                  switchToPage={switchToPage}
                  onDeletePage={onDeletePage}
                />
              </div>
            ))}
          </div>
        </Allotment.Pane>

        {/* 下区 pane */}
        <Allotment.Pane
          visible={bottomOpen}
          preferredSize={(1 - splitRatio) * 100}
        >
          <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
            {bottomDefs.map((def) => (
              <div
                key={def.id}
                style={{
                  display: open.bottom === def.id ? "flex" : "none",
                  height: "100%",
                  flexDirection: "column",
                }}
                data-e2e={`sidebar-slot-bottom-${def.id}`}
              >
                <def.component
                  switchToPage={switchToPage}
                  onDeletePage={onDeletePage}
                />
              </div>
            ))}
          </div>
        </Allotment.Pane>
      </Allotment>
    </div>
  );
};
