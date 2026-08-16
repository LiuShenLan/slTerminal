// NavHistoryNode.tsx —— 历史会话折叠节点（NAV-03 / UI-303/505）
//
// 挂项目下（决策 5：历史折叠节点挂项目下，cwd 归属）：时钟图标（IconHistory）
// + 「历史session」+ 计数 pill（#1a1a1e 底 SIDEBAR_BG、fg-4 PLACEHOLDER_FG）；
// 名称 fg-3（mockup .row.hist .nm 契约）。文案 2026-08 人工验证修订：
// 「历史」→「历史session」，并与操作页面同级（NavTree 外包 childrenStyle 容器）。
// 结构（NAV-10 契约：历史行须嵌套于 nav-history-node 元素内——测试经
// node.querySelectorAll 定位行内容）：外层容器承载 data-e2e 与整体点击，
// 内层头部行持行样式，展开时在头部下渲染子级容器（历史行/空态）。
// 子级容器点击 stopPropagation——行内点击不误触发节点折叠。
// 节点常驻项目下（不随项目展开态隐藏，NAV-10 契约）。

import React, { useState } from "react";
import type { ReactNode } from "react";
import { IconChevronDown, IconChevronRight, IconHistory } from "../../lib/icons";
import { DIM_FG } from "../../theme";
import { chevronStyle, childrenStyle, countPillStyle, nameStyle, rowBaseStyle } from "./navStyles";

interface NavHistoryNodeProps {
  /** 项目全部历史会话数（计数 pill） */
  total: number;
  expanded: boolean;
  onToggle(): void;
  /** 展开时渲染的子内容（历史行/空态，NAV-03） */
  children?: ReactNode;
}

export const NavHistoryNode: React.FC<NavHistoryNodeProps> = ({
  total,
  expanded,
  onToggle,
  children,
}) => {
  const [hovered, setHovered] = useState(false);

  return (
    <div
      data-e2e="nav-history-node"
      onClick={onToggle}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* 头部行：chevron + 时钟 + 「历史session」+ 计数 pill */}
      <div
        style={{
          ...rowBaseStyle(false, hovered),
          color: DIM_FG, // 名称 fg-3（mockup .row.hist .nm）
          fontSize: 12.5,
        }}
      >
        <span style={chevronStyle}>
          {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
        </span>
        <span style={{ flexShrink: 0, display: "flex", color: DIM_FG }}>
          <IconHistory size={14} />
        </span>
        <span style={nameStyle}>历史session</span>
        <span style={countPillStyle}>{total}</span>
      </div>
      {/* 子级容器：历史行/空态（点击不冒泡到节点整体点击） */}
      {expanded && (
        <div style={childrenStyle} onClick={(e) => e.stopPropagation()}>
          {children}
        </div>
      )}
    </div>
  );
};
