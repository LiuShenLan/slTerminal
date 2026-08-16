// NavPageRow.tsx —— 操作页面行（NAV-01 行结构 + 内联重命名迁移自 SidebarTree）
//
// 行结构（UI-501）：chevron 12px fg-3 + 图标 + 名称 + 右侧元数据（11px fg-4）。
// 选中行（活跃页面）= accent-dim 底 + fg-1 文字（UI-502），hover → SELECTION_HOVER_BG。
// 页面图标：IconPage（FileText，IC-01 单点）14px fg-3——原等宽占位已替换
// （2026-08 人工验证问题 2：与历史session 行的时钟图标区分）。
// chevron 点击仅切换会话展开（stopPropagation），行点击切换页面（照 SidebarTree）。
// 内联重命名（入口 = 右键菜单「重命名操作页面」）：Enter 确认 / Esc 取消 / blur 确认 /
// 空白或同名取消（行为照 SidebarTree PageRow 不变）。

import React, { useCallback, useEffect, useState } from "react";
import type { OperationPage } from "../../stores/projects";
import { IconChevronDown, IconChevronRight, IconPage } from "../../lib/icons";
import { DIM_FG, FOCUS_BORDER, INPUT_BG, SIDEBAR_COLORS, SIDEBAR_FG } from "../../theme";
import { chevronStyle, metaStyle, nameStyle, rowBaseStyle } from "./navStyles";

interface NavPageRowProps {
  page: OperationPage;
  /** 选中态（活跃页面——accent-dim 底 + fg-1） */
  selected: boolean;
  /** 会话展开态（chevron 方向） */
  expanded: boolean;
  /** 右侧元数据（页面收起时最近会话标题；undefined = 不显示） */
  meta?: string;
  isRenaming: boolean;
  onRename(newName: string): void;
  onCancelRename(): void;
  onClick(): void;
  /** chevron 点击切换会话展开 */
  onToggle(): void;
  onContextMenu(e: React.MouseEvent): void;
}

export const NavPageRow: React.FC<NavPageRowProps> = ({
  page,
  selected,
  expanded,
  meta,
  isRenaming,
  onRename,
  onCancelRename,
  onClick,
  onToggle,
  onContextMenu,
}) => {
  const [editValue, setEditValue] = useState("");
  const [hovered, setHovered] = useState(false);

  // 父组件触发重命名 → 进入编辑模式（迁移自 SidebarTree PageRow）
  useEffect(() => {
    if (isRenaming) setEditValue(page.name);
  }, [isRenaming, page.name]);

  const confirmRename = useCallback(() => {
    const trimmed = editValue.trim();
    if (trimmed && trimmed !== page.name) onRename(trimmed);
    else onCancelRename();
  }, [editValue, page.name, onRename, onCancelRename]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") confirmRename();
      else if (e.key === "Escape") onCancelRename();
    },
    [confirmRename, onCancelRename],
  );

  return (
    <div
      data-e2e="nav-row-page"
      onClick={isRenaming ? undefined : onClick}
      onContextMenu={onContextMenu}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        ...rowBaseStyle(selected, hovered),
        color: selected ? SIDEBAR_FG : SIDEBAR_COLORS.fg, // 选中 fg-1，未选中 fg-2（UI-501/502）
        fontSize: 12.5,
      }}
    >
      {/* 树箭头：chevron 点击只切换会话展开（stopPropagation 防触发行切换） */}
      <span
        style={chevronStyle}
        onClick={(e) => {
          e.stopPropagation();
          onToggle();
        }}
      >
        {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
      </span>
      {/* 页面图标槽位：IconPage（FileText）14px fg-3——与历史session 时钟图标区分 */}
      <span style={{ width: 14, flexShrink: 0, display: "flex", color: DIM_FG }}>
        <IconPage size={14} />
      </span>
      {isRenaming ? (
        <input
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          onBlur={confirmRename}
          onKeyDown={handleKeyDown}
          autoFocus
          onClick={(e) => e.stopPropagation()}
          style={{
            flex: 1,
            minWidth: 0,
            background: INPUT_BG,
            border: `1px solid ${FOCUS_BORDER}`,
            borderRadius: 4,
            color: SIDEBAR_COLORS.fg,
            fontSize: 12.5,
            padding: "0 4px",
            // UI-808：input 键盘可达，去 outline:none 让全局 :focus-visible 环生效
          }}
        />
      ) : (
        <>
          <span style={nameStyle}>{page.name}</span>
          {meta != null && <span style={metaStyle}>{meta}</span>}
        </>
      )}
    </div>
  );
};
