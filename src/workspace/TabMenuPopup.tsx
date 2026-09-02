// TabMenuPopup.tsx — 页签右键菜单弹层（自研，UI-802 规格）
//
// dockview 8.1 free core 无 contextMenuService（getTabContextMenuItems 路径恒短路），
// 页签右键菜单机制自绘于此：DefaultTab onContextMenu → PageDockview 中心状态 →
// TabMenuPopup fixed 弹层。与全仓自研菜单（NavContextMenu 等）同构，额外支持
// "separator" 分隔线令牌。
//
// 菜单项/容器样式为旧 TabContextMenuItem + TAB_CONTEXT_MENU_CSS 规格的逐像素迁移：
// 项高 28px、hover SECONDARY_BG、danger ERROR_FG、disabled 置灰 0.4 且不响应；
// 容器 SIDEBAR_BG 底 + CONTEXT_MENU_BORDER 描边 + 圆角 5 + contextMenuShadow。
// 全部 token 引用（硬约束 #6），不注入 <style>。

import React, { useEffect, useRef, useState } from "react";
import {
  SIDEBAR_BG,
  SIDEBAR_FG,
  SECONDARY_BG,
  ERROR_FG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
} from "../theme";

/** 页签右键菜单项（通用自研形态；"separator" 为分隔线令牌） */
export type TabMenuItem =
  | { label: string; danger?: boolean; disabled?: boolean; action: () => void }
  | "separator";

/** 菜单状态：fixed 定位坐标 + 项列表 */
export interface TabMenuState {
  x: number;
  y: number;
  items: TabMenuItem[];
}

/**
 * 页签右键菜单弹层（UI-802）。props.menu 为 null 即关闭（不渲染）。
 * 关闭机制：点击项（先 action 后关）、菜单外 document mousedown、Escape——
 * 监听仅在菜单开启期间挂载，成对清理。
 */
export const TabMenuPopup: React.FC<{
  menu: TabMenuState | null;
  onClose: () => void;
}> = ({ menu, onClose }) => {
  const menuRef = useRef<HTMLDivElement | null>(null);
  const [hoveredIdx, setHoveredIdx] = useState(-1);

  useEffect(() => {
    if (!menu) return;
    // 换菜单（新右键）时不残留旧 hover 高亮
    setHoveredIdx(-1);
    const onDocMouseDown = (e: MouseEvent) => {
      // 点在菜单容器外 → 关闭（右键另一页签 = mousedown 先关、随后新 contextmenu 重开）
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDocMouseDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onDocMouseDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [menu, onClose]);

  if (!menu) return null;

  return (
    <div
      ref={menuRef}
      data-e2e="tab-menu"
      role="menu"
      style={{
        position: "fixed",
        left: menu.x,
        top: menu.y,
        zIndex: 1000,
        background: SIDEBAR_BG,
        border: `1px solid ${CONTEXT_MENU_BORDER}`,
        borderRadius: 5,
        padding: "4px 0",
        minWidth: 160,
        boxShadow: SIDEBAR_COLORS.contextMenuShadow,
      }}
    >
      {menu.items.map((item, i) =>
        item === "separator" ? (
          <div
            key={i}
            style={{
              height: 1,
              background: CONTEXT_MENU_BORDER,
              margin: "4px 0",
            }}
          />
        ) : (
          <div
            key={i}
            role="menuitem"
            onMouseEnter={() => setHoveredIdx(i)}
            onMouseLeave={() => setHoveredIdx(-1)}
            onClick={() => {
              if (item.disabled) return;
              item.action();
              onClose(); // 先执行 action 再关菜单（照库/旧实现语义）
            }}
            style={{
              height: 28,
              margin: "0 4px",
              borderRadius: 5,
              display: "flex",
              alignItems: "center",
              padding: "0 12px",
              cursor: item.disabled ? "default" : "pointer",
              fontSize: 13,
              whiteSpace: "nowrap",
              userSelect: "none",
              background:
                hoveredIdx === i && !item.disabled
                  ? SECONDARY_BG
                  : "transparent",
              color: item.danger === true ? ERROR_FG : SIDEBAR_FG,
              opacity: item.disabled ? 0.4 : 1,
              pointerEvents: item.disabled ? "none" : "auto",
            }}
          >
            {item.label}
          </div>
        ),
      )}
    </div>
  );
};
