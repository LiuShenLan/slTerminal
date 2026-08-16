// NavProjectRow.tsx —— 项目行（NAV-09 / UI-505）
//
// 500 字重 fg-1 + 彩色文件夹图标；当前活跃项目带「当前」pill
// （accent-dim 底 ACTIVE_SELECTION_BG + #8fb4f5 字 ACCENT_FG、10px）；
// 右侧页面计数 pill（#1a1a1e 底 SIDEBAR_BG、fg-4 PLACEHOLDER_FG）。
// 彩色文件夹图标 = 六色盘蓝（UI-505 执行期决策：与 FileIcon.tsx 六色盘同源色值——
// 该组件已登记「本组件硬编码例外」（IC-04 契约），此处同规格例外，NAV-09 写死）。
// 行整体点击 = 展开/折叠（照 SidebarTree ProjectRow）。

import React, { useState } from "react";
import type { Project } from "../../stores/projects";
import { IconChevronDown, IconChevronRight, IconFolder } from "../../lib/icons";
import { SIDEBAR_FG } from "../../theme";
import {
  chevronStyle,
  countPillStyle,
  currentPillStyle,
  nameStyle,
  rowBaseStyle,
} from "./navStyles";

/** 项目文件夹图标色——六色盘蓝（硬编码例外，见文件头注释） */
const PROJECT_FOLDER_COLOR = "#7fa8e8";

interface NavProjectRowProps {
  project: Project;
  expanded: boolean;
  /** 当前活跃项目（「当前」pill） */
  isCurrent: boolean;
  /** 页面计数（计数 pill） */
  pageCount: number;
  onToggle(): void;
  onContextMenu(e: React.MouseEvent): void;
}

export const NavProjectRow: React.FC<NavProjectRowProps> = ({
  project,
  expanded,
  isCurrent,
  pageCount,
  onToggle,
  onContextMenu,
}) => {
  const [hovered, setHovered] = useState(false);

  return (
    <div
      data-e2e="nav-row-project"
      onClick={onToggle}
      onContextMenu={onContextMenu}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        ...rowBaseStyle(false, hovered),
        color: SIDEBAR_FG, // fg-1（UI-505 项目行文字）
        fontWeight: 500, // 500 字重（UI-505）
        fontSize: 12.5,
      }}
    >
      {/* 树箭头：chevron 12px fg-3（IC-05/icons.tsx 单点） */}
      <span style={chevronStyle}>
        {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
      </span>
      {/* 彩色文件夹图标（六色盘蓝） */}
      <span style={{ flexShrink: 0, display: "flex", color: PROJECT_FOLDER_COLOR }}>
        <IconFolder size={14} />
      </span>
      <span style={nameStyle}>{project.name}</span>
      {isCurrent && <span style={currentPillStyle}>当前</span>}
      {/* 页面计数 pill：非当前项目时承接首个 auto 外边距（当前项目由「当前」pill 承接） */}
      <span style={{ ...countPillStyle, marginLeft: isCurrent ? undefined : "auto" }}>
        {pageCount}
      </span>
    </div>
  );
};
