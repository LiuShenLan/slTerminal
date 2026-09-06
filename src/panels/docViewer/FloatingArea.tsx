// FloatingArea.tsx — 面板根右上悬浮区（docViewer 共享悬浮 UI 单点）
//
// 形态切换条（上）与缩放 HUD 气泡（下）恒列排的承载容器——2026-09-06 收敛：
// 原实现 HUD 避让取决于 PreviewFrame 是否收到 overlay（paddingTop 44 hack），
// md 面板切换条绕开 overlay 槽改由面板根自摆 → PreviewFrame 不知情 → HUD 与
// 切换条同坐标重叠（md 专属缺陷，html 因切换条进 overlay 槽而幸免——双轨
// 分裂实证）。收敛后两面板全形态同构：内容区 + 本悬浮区；HUD 与切换条的空间
// 协调 = 单容器纵向布局，不再依赖几何 hack。
//
// 定位：面板根 absolute 右上（top 8 / right 8 / zIndex 20——悬浮于 Allotment/
// PreviewFrame/CM 之上）。容器 pointerEvents: "none" 透传，子项各自 auto
//（切换条 hover 提不透明度、HUD 重置可点）。data-e2e 前缀由面板传入
//（html-zoom-hud / markdown-zoom-hud，保持历史命名兼容）。

import React from "react";
import { ACCENT_FG, SECONDARY_BG, SEPARATOR_BG, SIDEBAR_FG } from "../../theme";
import { formatPercent } from "./previewMessages";

/** HUD 数据（缩放显示/复位命令），由面板经 useZoomHud + PreviewFrame ref 组装 */
export interface FloatingHud {
  zoom: number;
  visible: boolean;
  /** 重置点击：下行复位 + 立即隐藏（面板侧组装：frameRef.resetZoom + hudApi.hide） */
  onReset: () => void;
}

export interface FloatingAreaProps {
  /** 常驻悬浮切换条（形态切换 UI；无则不渲染） */
  switcher?: React.ReactNode;
  /** 缩放 HUD 数据（null = 无预览框形态无缩放源，仅切换条） */
  hud?: FloatingHud | null;
  /** e2e 探针前缀（默认 "doc"） */
  dataE2ePrefix?: string;
}

/** 悬浮区容器：absolute 右上、纵向列排（上=切换条 下=HUD）、不拦交互 */
const areaStyle: React.CSSProperties = {
  position: "absolute",
  top: 8,
  right: 8,
  zIndex: 20,
  display: "flex",
  flexDirection: "column",
  alignItems: "flex-end",
  gap: 6,
  pointerEvents: "none",
};

/** HUD 气泡本体：百分比 + 重置按钮（瞬态，缩放停止 3s 后消失） */
const hudChipStyle: React.CSSProperties = {
  pointerEvents: "auto",
  display: "flex",
  alignItems: "center",
  gap: 4,
  background: SECONDARY_BG,
  border: `1px solid ${SEPARATOR_BG}`,
  borderRadius: 6,
  padding: "2px 4px 2px 10px",
  fontSize: 12,
  color: SIDEBAR_FG,
};

/** 重置按钮（去浏览器默认样式，accent 色标识可操作） */
const resetBtnStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  cursor: "pointer",
  color: ACCENT_FG,
  fontSize: 12,
  padding: "2px 6px",
};

export const FloatingArea: React.FC<FloatingAreaProps> = ({
  switcher,
  hud,
  dataE2ePrefix = "doc",
}) => (
  <div style={areaStyle}>
    {switcher !== undefined && <div style={{ pointerEvents: "auto" }}>{switcher}</div>}
    {hud !== null && hud !== undefined && hud.visible && (
      <div data-e2e={`${dataE2ePrefix}-zoom-hud`} style={hudChipStyle} title="缩放比例">
        <span>{formatPercent(hud.zoom)}</span>
        <button
          data-e2e={`${dataE2ePrefix}-zoom-reset`}
          style={resetBtnStyle}
          onClick={hud.onReset}
          title="重置为 100%"
        >
          重置
        </button>
      </div>
    )}
  </div>
);
