// NavContextMenu.tsx —— 导航树右键菜单（UI-802 视觉规范）
//
// 项 28px 高、圆角 5、hover #222227（SECONDARY_BG）、危险项（删除类）ERROR_FG；
// 容器 SIDEBAR_BG 底 + 0.09 描边（CONTEXT_MENU_BORDER）+ 阴影（SIDEBAR_COLORS.contextMenuShadow）。
// 项目/页面/历史行菜单共用（承接 SidebarTree 现菜单项但删除「打开 Hooks 配置」——
// 决策 4 入口唯一化，配置钮移至活动栏底部）。
// 危险项判定由调用方构造 items 时标记（danger: true）；disabled 项灰显不可点。

import React, { useEffect, useRef, useState } from "react";
import {
  CONTEXT_MENU_BORDER,
  ERROR_FG,
  PLACEHOLDER_FG,
  SECONDARY_BG,
  SIDEBAR_BG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
} from "../../theme";

export interface NavMenuItem {
  label: string;
  /** true 时灰显不可点（无点击回调） */
  disabled?: boolean;
  /** 危险项（删除类）——ERROR_FG 着色（UI-802） */
  danger?: boolean;
  action(): void;
}

export interface NavMenuState {
  visible: boolean;
  x: number;
  y: number;
  items: NavMenuItem[];
}

/** 菜单项行样式（项 28px、圆角 5——UI-802） */
const itemStyle: React.CSSProperties = {
  height: 28,
  margin: "0 4px",
  borderRadius: 5,
  display: "flex",
  alignItems: "center",
  padding: "0 12px",
  fontSize: 12,
  userSelect: "none",
};

export const NavContextMenu: React.FC<{
  state: NavMenuState;
  onClose(): void;
}> = ({ state, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  // hover 高亮 React state 驱动（FE-06：不再直改 DOM style.background）
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  // 菜单重开时清空 hover 索引（防上次关闭残留指向错误项）
  useEffect(() => {
    if (state.visible) setHoveredIdx(null);
  }, [state.visible]);

  // 点击菜单外任意处关闭（原 SidebarTree/HistorySessionList（均已退役删除）ContextMenu 同款）
  useEffect(() => {
    if (!state.visible) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [state.visible, onClose]);

  if (!state.visible) return null;

  return (
    <div
      ref={menuRef}
      style={{
        position: "fixed",
        left: state.x,
        top: state.y,
        background: SIDEBAR_BG,
        border: `1px solid ${CONTEXT_MENU_BORDER}`,
        borderRadius: 5,
        padding: "4px 0",
        minWidth: 160,
        zIndex: 1000,
        boxShadow: SIDEBAR_COLORS.contextMenuShadow,
      }}
    >
      {state.items.map((item, i) =>
        item.disabled ? (
          <div key={i} style={{ ...itemStyle, color: PLACEHOLDER_FG, cursor: "default" }}>
            {item.label}
          </div>
        ) : (
          <div
            key={i}
            onClick={() => {
              item.action();
              onClose();
            }}
            style={{
              ...itemStyle,
              cursor: "pointer",
              color: item.danger ? ERROR_FG : SIDEBAR_FG,
              background: hoveredIdx === i ? SECONDARY_BG : "transparent",
            }}
            onMouseEnter={() => setHoveredIdx(i)}
            onMouseLeave={() => setHoveredIdx(null)}
          >
            {item.label}
          </div>
        ),
      )}
    </div>
  );
};
