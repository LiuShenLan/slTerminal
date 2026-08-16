// navStyles.ts —— 导航树共享样式与纯辅助（NAV-01 行结构契约）
//
// 行结构（UI-501）：chevron 12px fg-3 + 图标 + 名称 + 右侧 11px fg-4 元数据；
// 行高 28（会话行 30）、圆角 5、hover #222227（SIDEBAR_COLORS.hover 槽位）；
// 选中行 rgba(110,159,242,0.13) 底（ACTIVE_SELECTION_BG，hover → SELECTION_HOVER_BG）+ fg-1（UI-502）。
// 层级缩进（UI-503）：每级左缩 15px + 1px 发丝引导线（SIDEBAR_COLORS.treeGuide）。
// 配色全部 theme/colors.ts token（硬约束 #6）；唯一例外 = 项目行彩色文件夹图标
// 六色盘蓝（NavProjectRow 内登记，照 FileIcon 硬编码例外规格）。
//
// fg 层级映射（final-mockup 契约）：fg-1 = SIDEBAR_FG / fg-2 = SIDEBAR_COLORS.fg /
// fg-3 = DIM_FG / fg-4 = PLACEHOLDER_FG。

import type { CSSProperties } from "react";
import {
  ACCENT_FG,
  ACTIVE_SELECTION_BG,
  DIM_FG,
  PLACEHOLDER_FG,
  SELECTION_HOVER_BG,
  SIDEBAR_BG,
  SIDEBAR_COLORS,
} from "../../theme";

/** 普通行高（项目/页面/历史节点行） */
export const ROW_HEIGHT = 28;
/** 会话行高（活跃会话/历史会话行） */
export const SESSION_ROW_HEIGHT = 30;

/**
 * 树行容器基础样式（全部行共用，UI-501/502）：
 * 行高 28（会话行 30）、圆角 5、hover #222227；选中行 accent-dim 底 + hover 0.22。
 * 文字颜色由各行自行设置（项目 fg-1 500 / 页面选中 fg-1 / 会话 fg-1）。
 */
export function rowBaseStyle(
  selected: boolean,
  hovered: boolean,
  height = ROW_HEIGHT,
): CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    height,
    padding: "0 8px",
    borderRadius: 5,
    cursor: "pointer",
    userSelect: "none",
    whiteSpace: "nowrap",
    overflow: "hidden",
    backgroundColor: hovered
      ? selected
        ? SELECTION_HOVER_BG // 选中行 hover：accent-dim-2（UI-502 hover 0.22）
        : SIDEBAR_COLORS.hover // 普通 hover：#222227（UI-501）
      : selected
        ? ACTIVE_SELECTION_BG // 选中/活跃行：rgba(110,159,242,0.13)（UI-502）
        : "transparent",
  };
}

/** 折叠箭头槽位（chevron 12px fg-3——UI-501/IC-05） */
export const chevronStyle: CSSProperties = {
  width: 12,
  height: 12,
  flexShrink: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: DIM_FG, // fg-3
};

/** 子级容器（UI-503：每级左缩 15px + 1px 发丝引导线） */
export const childrenStyle: CSSProperties = {
  marginLeft: 15,
  borderLeft: `1px solid ${SIDEBAR_COLORS.treeGuide}`,
  paddingLeft: 7,
};

/** 名称（flex 1 截断省略） */
export const nameStyle: CSSProperties = {
  flex: 1,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

/** 右侧元数据（11px fg-4——UI-501） */
export const metaStyle: CSSProperties = {
  marginLeft: "auto",
  flexShrink: 0,
  fontSize: 11,
  color: PLACEHOLDER_FG, // fg-4
};

/** 计数 pill（#1a1a1e 底 fg-4、11px——NAV-09 页面计数 / NAV-03 历史计数，UI-505） */
export const countPillStyle: CSSProperties = {
  flexShrink: 0,
  fontSize: 11,
  color: PLACEHOLDER_FG,
  backgroundColor: SIDEBAR_BG,
  borderRadius: 999,
  padding: "0 7px",
};

/** 「当前」pill（accent-dim 底 + accent-fg 字、10px——UI-505/NAV-09） */
export const currentPillStyle: CSSProperties = {
  marginLeft: "auto",
  flexShrink: 0,
  fontSize: 10,
  color: ACCENT_FG,
  backgroundColor: ACTIVE_SELECTION_BG,
  borderRadius: 999,
  padding: "1px 7px",
};
