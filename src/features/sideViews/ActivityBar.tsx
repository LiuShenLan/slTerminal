// ActivityBar —— 活动栏组件（NAV-05：46px 宽常驻栏）
//
// 职责：
// - 渲染上区/下区两按钮组，中间 flex:1 间隔
// - 按钮点击 → useSideBar.toggleView(id)（单槽位开关）
// - 按钮 active 态高亮背景 + 左侧 2px 指示条（VS Code 风格）
// - HTML5 原生拖拽（零依赖）：上区 ↔ 下区换区 + 半区内排序
// - 拖拽落点指示线（computeDropTarget 纯函数）
// - 底部「配置」钮（NAV-05）：id config、IconConfig、data-e2e=activity-btn-config、
//   点击 = openSettings（设置中心）——不入 SideViewRegistry、不参与拖拽/持久化
//
// 规格（NAV-05/GL-04）：栏宽 46px（ACTIVITY_BAR_SIZE 常量，Workspace 同步引用）；
// 按钮 34×34 圆角 6；激活态 = ACTIVE_SELECTION_BG（accent-dim）底 +
// ACCENT_FG 图标 + 左侧 2px FOCUS_BORDER 竖条（沿用现指示条机制）。
//
// 硬约束 #6：全部颜色引用 theme/colors.ts token，禁止硬编码色值
// 零新依赖：HTML5 DnD 原生，不引入 react-dnd / dnd-kit

import { useState, useCallback, useRef } from "react";
import { useSideBar } from "../../stores/sideBar";
import { sideViewRegistry } from "./sideViewRegistry";
import { computeDropTarget } from "./dropTarget";
import { ACTIVITY_BAR_SIZE } from "./sideBarState";
import type { Zone } from "./sideBarState";
import type { ButtonRect } from "./dropTarget";
import { openSettings } from "../settingsCenter/openSettings";
import { IconConfig } from "../../lib/icons";
import {
  PANEL_BG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
  DIM_FG,
  ACCENT_FG,
  FOCUS_BORDER,
  ACTIVE_SELECTION_BG,
} from "../../theme/colors";

// ── 样式常量（全部引用 theme/colors.ts token） ──

const containerStyle: React.CSSProperties = {
  width: ACTIVITY_BAR_SIZE,
  display: "flex",
  flexDirection: "column",
  height: "100%",
  backgroundColor: PANEL_BG,
  borderRight: `1px solid ${SIDEBAR_COLORS.border}`,
  userSelect: "none",
};

const buttonBase: React.CSSProperties = {
  width: 34,
  height: 34,
  margin: "6px auto", // 46px 栏内水平居中 + 上下 6px 间距
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
  background: "transparent",
  border: "none",
  borderLeft: "2px solid transparent",
  borderRadius: 6,
  padding: 0,
  position: "relative",
  // UI-808：button 键盘可达，去 outline:none 让全局 :focus-visible 环生效（鼠标点击不显示）
};

const spacerStyle: React.CSSProperties = {
  flex: 1,
};

const indicatorStyle: React.CSSProperties = {
  height: 2,
  backgroundColor: FOCUS_BORDER,
  width: "100%",
};

/** 活动栏内拖拽 dataTransfer 类型标记 */
const DND_TYPE = "application/x-side-view-id";

// ── 组件 ──

export function ActivityBar() {
  const zones = useSideBar((s) => s.zones);
  const open = useSideBar((s) => s.open);

  /** 当前正在拖拽的按钮 id（null = 无拖拽） */
  const [draggingId, setDraggingId] = useState<string | null>(null);
  /** 拖拽落点指示线位置 */
  const [dropIndicator, setDropIndicator] = useState<{
    zone: Zone;
    index: number;
  } | null>(null);
  /** 当前 hover 的按钮 id（非 active 态才显示 hover 色） */
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  /** 清空拖拽状态 */
  const clearDragState = useCallback(() => {
    setDraggingId(null);
    setDropIndicator(null);
  }, []);

  /** 根据 clientY 在活动栏内的相对位置判定目标 zone——容器垂直中点以上→"top"，以下→"bottom" */
  const resolveTargetZone = useCallback(
    (clientY: number, root: HTMLElement): Zone => {
      const rect = root.getBoundingClientRect();
      const boundary = rect.top + rect.height / 2;
      return clientY >= boundary ? "bottom" : "top";
    },
    [],
  );

  /** 活动栏根容器 ref——供 getButtonRects 限定查询范围 */
  const barRef = useRef<HTMLDivElement>(null);

  /** 收集指定 zone 内各按钮的屏幕矩形（供 computeDropTarget 使用，限定根容器内查询） */
  const getButtonRects = useCallback((zone: Zone): ButtonRect[] => {
    const root = barRef.current;
    if (!root) return [];
    const container = root.querySelector(`[data-zone="${zone}"]`);
    if (!container) return [];
    const buttons = container.querySelectorAll("[data-view-id]");
    const rects: ButtonRect[] = [];
    buttons.forEach((btn) => {
      const id = btn.getAttribute("data-view-id");
      if (!id) return;
      const r = btn.getBoundingClientRect();
      rects.push({ id, top: r.top, height: r.height });
    });
    return rects;
  }, []);

  /** 渲染单个半区按钮组（仅结构+样式，拖拽事件在外层容器统一处理） */
  const renderZone = (zone: Zone) => {
    const ids = zones[zone];
    const defs = ids
      .map((id) => sideViewRegistry.get(id))
      .filter((d): d is NonNullable<typeof d> => d != null);

    return (
      <div data-zone={zone}>
        {defs.map((def, index) => {
          const isActive = open[zone] === def.id;
          const isDragging = draggingId === def.id;
          const isHovered = hoveredId === def.id;

          // 计算按钮背景色
          let bg = "transparent";
          if (isActive) {
            bg = ACTIVE_SELECTION_BG;
          } else if (isHovered) {
            bg = SIDEBAR_COLORS.hover;
          }
          // 图标色（IC-06）：默认 fg-3（DIM_FG）→ hover fg-1（SIDEBAR_FG）→ active accentFg
          const iconColor = isActive
            ? ACCENT_FG
            : isHovered
              ? SIDEBAR_FG
              : DIM_FG;

          const showIndicatorBefore =
            dropIndicator != null &&
            dropIndicator.zone === zone &&
            dropIndicator.index === index;

          return (
            <div key={def.id}>
              {/* 拖拽插入指示线（按钮前方） */}
              {showIndicatorBefore && <div style={indicatorStyle} />}
              <button
                data-view-id={def.id}
                data-e2e={`activity-btn-${def.id}`}
                title={def.title}
                draggable
                style={{
                  ...buttonBase,
                  backgroundColor: bg,
                  borderLeftColor: isActive ? FOCUS_BORDER : "transparent",
                  opacity: isDragging ? 0.5 : 1,
                  color: iconColor,
                }}
                onClick={() => useSideBar.getState().toggleView(def.id)}
                onMouseEnter={() => {
                  if (!isActive) setHoveredId(def.id);
                }}
                onMouseLeave={() => {
                  if (hoveredId === def.id) setHoveredId(null);
                }}
                onDragStart={(e) => {
                  e.dataTransfer.setData(DND_TYPE, def.id);
                  e.dataTransfer.effectAllowed = "move";
                  setDraggingId(def.id);
                }}
                onDragEnd={() => {
                  clearDragState();
                }}
              >
                <def.icon size={15} />
              </button>
            </div>
          );
        })}
        {/* 拖拽插入指示线（zone 末尾） */}
        {dropIndicator != null &&
          dropIndicator.zone === zone &&
          dropIndicator.index >= defs.length && <div style={indicatorStyle} />}
      </div>
    );
  };

  return (
    <div
      ref={barRef}
      data-e2e="activity-bar"
      style={containerStyle}
      onDragOver={(e) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        const root = barRef.current;
        if (!root) return;
        const targetZone = resolveTargetZone(e.clientY, root);
        const rects = getButtonRects(targetZone);
        const target = computeDropTarget(e.clientY, rects, targetZone);
        setDropIndicator(target);
      }}
      onDragLeave={(e) => {
        // 仅当拖拽真正离开活动栏时才清指示线——relatedTarget 仍在本容器内
        // （容器→子元素 / 子元素间转移）视为未离开，不清（FE-23）
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          setDropIndicator(null);
        }
      }}
      onDrop={(e) => {
        e.preventDefault();
        const id = e.dataTransfer.getData(DND_TYPE);
        if (id && dropIndicator) {
          useSideBar.getState().moveButton(
            id,
            dropIndicator.zone,
            dropIndicator.index,
          );
        }
        clearDragState();
      }}
    >
      {renderZone("top")}
      <div style={spacerStyle} />
      {renderZone("bottom")}
      {/* 底部「配置」钮（NAV-05：固定底部、不入注册表、不参与拖拽/持久化） */}
      <button
        data-e2e="activity-btn-config"
        title="配置"
        style={{
          ...buttonBase,
          marginBottom: 8,
          backgroundColor: hoveredId === "config" ? SIDEBAR_COLORS.hover : "transparent",
          color: hoveredId === "config" ? SIDEBAR_FG : DIM_FG,
        }}
        onClick={() => {
          void openSettings();
        }}
        onMouseEnter={() => setHoveredId("config")}
        onMouseLeave={() => {
          if (hoveredId === "config") setHoveredId(null);
        }}
      >
        <IconConfig size={15} />
      </button>
    </div>
  );
}
