// CommitFileList.tsx — commit 视图可折叠文件列表
//
// 渲染可折叠标题栏 + 文件条目列表。
// 颜色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useState, useCallback, useRef, useEffect } from "react";
import { basename, relativePath } from "../../lib/path";
import {
  GIT_FILE_COLORS,
  EXPLORER_COLORS,
  INPUT_BORDER,
  SIDEBAR_BG,
  SIDEBAR_FG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  ACTIVE_SELECTION_BG,
} from "../../theme";
import { openCommitFile } from "./openCommitFile";
import { getContextMenuItems } from "./commitContextMenu";
import type { CommitMenuItem } from "./commitContextMenu";
import type { GitStatusEntry } from "../../types/git";

/** 折叠箭头样式 */
const arrowStyle: React.CSSProperties = {
  display: "inline-block",
  width: 16,
  fontSize: 10,
  color: EXPLORER_COLORS.arrowClosed,
  textAlign: "center",
  lineHeight: "22px",
  userSelect: "none",
  flexShrink: 0,
};

/** 标题栏样式 */
const sectionHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px",
  height: 22,
  cursor: "pointer",
  userSelect: "none",
  fontSize: 12,
  color: EXPLORER_COLORS.fg,
};

/** 文件行容器 */
const fileRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px 2px 24px",
  height: 22,
  cursor: "pointer",
  fontSize: 12,
  userSelect: "none",
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

/** 空态提示 */
const emptyHintStyle: React.CSSProperties = {
  padding: "4px 8px 4px 24px",
  color: INPUT_BORDER,
  fontSize: 11,
  fontStyle: "italic",
  userSelect: "none",
};

// ── 右键菜单 ──

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  items: CommitMenuItem[];
}

/** 右键菜单浮层（纯渲染，照 FileTree.tsx ContextMenu 模式） */
const ContextMenu: React.FC<{
  state: ContextMenuState;
  onClose: () => void;
}> = ({ state, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null);

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
        borderRadius: 4,
        padding: "4px 0",
        minWidth: 160,
        zIndex: 1000,
        boxShadow: SIDEBAR_COLORS.contextMenuShadow,
      }}
    >
      {state.items.map((item, i) => (
        <div
          key={i}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            padding: "4px 12px",
            cursor: "pointer",
            color: SIDEBAR_FG,
            fontSize: 13,
            userSelect: "none",
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLDivElement).style.background = ACTIVE_SELECTION_BG;
          }}
          onMouseLeave={(e) => {
            (e.target as HTMLDivElement).style.background = "transparent";
          }}
        >
          {item.label}
        </div>
      ))}
    </div>
  );
};

// ── 文件列表 ──

interface CommitFileListProps {
  /** 列表标题，如 "Changes" 或 "Unversioned Files" */
  title: string;
  /** 文件条目列表 */
  entries: GitStatusEntry[];
  /** 项目根路径（用于计算相对路径后缀 + IPC 调用） */
  rootPath: string;
  /** 列表 data-e2e 标识 */
  e2eId: string;
  /** 操作完成后刷新列表的回调 */
  onRefresh: () => void;
}

export const CommitFileList: React.FC<CommitFileListProps> = ({
  title,
  entries,
  rootPath,
  e2eId,
  onRefresh,
}) => {
  const [collapsed, setCollapsed] = useState(false);

  const toggleCollapse = () => setCollapsed((c) => !c);

  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  const closeContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  /** 右键菜单——委托策略注册表，不直接依赖 git/fs IPC */
  const handleContextMenu = useCallback(
    (e: React.MouseEvent, entry: GitStatusEntry) => {
      e.preventDefault();
      e.stopPropagation();

      const items = getContextMenuItems(entry, rootPath, onRefresh);
      if (items.length === 0) return;

      setContextMenu({
        visible: true,
        x: e.clientX,
        y: e.clientY,
        items,
      });
    },
    [rootPath, onRefresh],
  );

  // 按完整相对路径字母序排序
  const sorted = [...entries].sort((a, b) => {
    const ra = relativePath(a.path, rootPath) ?? a.path;
    const rb = relativePath(b.path, rootPath) ?? b.path;
    return ra.localeCompare(rb);
  });

  return (
    <div data-e2e={e2eId}>
      {/* 可折叠标题栏 */}
      <div style={sectionHeaderStyle} onClick={toggleCollapse}>
        <span style={arrowStyle}>
          {collapsed ? "▶" : "▼"}
        </span>
        <span>
          {title} ({entries.length})
        </span>
      </div>

      {/* 展开时显示文件列表或空态 */}
      {!collapsed && (
        <div>
          {sorted.length === 0 ? (
            <div style={emptyHintStyle}>无变更文件</div>
          ) : (
            sorted.map((entry) => (
              <CommitFileItem
                key={entry.path}
                entry={entry}
                rootPath={rootPath}
                onContextMenu={handleContextMenu}
              />
            ))
          )}
        </div>
      )}
      <ContextMenu state={contextMenu} onClose={closeContextMenu} />
    </div>
  );
};

/** 单个文件条目 */
interface CommitFileItemProps {
  entry: GitStatusEntry;
  rootPath: string;
  onContextMenu: (e: React.MouseEvent, entry: GitStatusEntry) => void;
}

const CommitFileItem: React.FC<CommitFileItemProps> = ({
  entry,
  rootPath,
  onContextMenu,
}) => {
  const handleDoubleClick = () => {
    openCommitFile(entry.path, entry.status, entry.oldPath ?? undefined);
  };

  const name = basename(entry.path);
  const relPath = relativePath(entry.path, rootPath);
  // 父目录路径（不含文件名）
  const dirPath = relPath
    ? relPath.slice(0, Math.max(0, relPath.lastIndexOf("/")))
    : "";

  // 文件名颜色 = GIT_FILE_COLORS[status]（硬约束 #6）
  const fileColor =
    GIT_FILE_COLORS[entry.status as keyof typeof GIT_FILE_COLORS] ??
    EXPLORER_COLORS.fg;

  return (
    <div
      data-e2e="commit-file-item"
      style={fileRowStyle}
      onDoubleClick={handleDoubleClick}
      onContextMenu={(e) => onContextMenu(e, entry)}
      title={relPath ?? entry.path}
    >
      <span style={{ color: fileColor, flexShrink: 0 }}>{name}</span>
      {dirPath && (
        <span
          style={{
            color: INPUT_BORDER,
            marginLeft: 6,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {dirPath}
        </span>
      )}
    </div>
  );
};
