// ActivityBar —— 活动栏组件（40px 宽常驻栏）
//
// 职责：
// - 渲染上区/下区两按钮组，中间 flex:1 间隔
// - 按钮点击 → useSideBar.toggleView(id)（单槽位开关）
// - 按钮 active 态高亮背景 + 左侧 2px 指示条（VS Code 风格）
// - HTML5 原生拖拽（零依赖）：上区 ↔ 下区换区 + 半区内排序
// - 拖拽落点指示线（computeDropTarget 纯函数）
//
// 硬约束 #6：全部颜色引用 theme/colors.ts token，禁止硬编码色值
// 零新依赖：HTML5 DnD 原生，不引入 react-dnd / dnd-kit

import { useState, useCallback } from "react";
import { useSideBar } from "../../stores/sideBar";
import { sideViewRegistry } from "./sideViewRegistry";
import { computeDropTarget } from "./dropTarget";
import type { Zone } from "./sideBarState";
import type { ButtonRect } from "./dropTarget";
import {
  PANEL_BG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
  FOCUS_BORDER,
} from "../../theme/colors";

// ── 样式常量（全部引用 theme/colors.ts token） ──

const containerStyle: React.CSSProperties = {
  width: 40,
  display: "flex",
  flexDirection: "column",
  height: "100%",
  backgroundColor: PANEL_BG,
  borderRight: `1px solid ${SIDEBAR_COLORS.border}`,
  userSelect: "none",
};

const buttonBase: React.CSSProperties = {
  width: 40,
  height: 40,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 20,
  lineHeight: "40px",
  cursor: "pointer",
  background: "transparent",
  border: "none",
  borderLeft: "2px solid transparent",
  color: SIDEBAR_FG,
  padding: 0,
  position: "relative",
  outline: "none",
  transition: "background-color 0.15s",
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

  /** 收集指定 zone 内各按钮的屏幕矩形（供 computeDropTarget 使用） */
  const getButtonRects = useCallback((zone: Zone): ButtonRect[] => {
    const container = document.querySelector(`[data-zone="${zone}"]`);
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

  /** 渲染单个半区按钮组 */
  const renderZone = (zone: Zone) => {
    const ids = zones[zone];
    const defs = ids
      .map((id) => sideViewRegistry.get(id))
      .filter((d): d is NonNullable<typeof d> => d != null);

    return (
      <div
        data-zone={zone}
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          const rects = getButtonRects(zone);
          const target = computeDropTarget(e.clientY, rects, zone);
          setDropIndicator(target);
        }}
        onDragLeave={(e) => {
          // 仅当真正离开该 zone 容器时才清指示线
          if (e.currentTarget === e.target) {
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
        {defs.map((def, index) => {
          const isActive = open[zone] === def.id;
          const isDragging = draggingId === def.id;
          const isHovered = hoveredId === def.id;

          // 计算按钮背景色
          let bg = "transparent";
          if (isActive) {
            bg = SIDEBAR_COLORS.selected;
          } else if (isHovered) {
            bg = SIDEBAR_COLORS.hover;
          }

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
                {def.icon}
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
    <div style={containerStyle}>
      {renderZone("top")}
      <div style={spacerStyle} />
      {renderZone("bottom")}
    </div>
  );
}
