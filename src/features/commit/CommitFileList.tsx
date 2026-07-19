// CommitFileList.tsx — commit 视图可折叠文件列表
//
// 渲染可折叠标题栏 + 文件条目列表。
// 颜色全部引用 theme/colors.ts token（硬约束 #6）。

import React, { useState } from "react";
import { basename, relativePath } from "../../lib/path";
import { GIT_FILE_COLORS, EXPLORER_COLORS, INPUT_BORDER } from "../../theme";
import { openCommitFile } from "./openCommitFile";
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

interface CommitFileListProps {
  /** 列表标题，如 "Changes" 或 "Unversioned Files" */
  title: string;
  /** 文件条目列表 */
  entries: GitStatusEntry[];
  /** 项目根路径（用于计算相对路径后缀） */
  rootPath: string;
  /** 列表 data-e2e 标识 */
  e2eId: string;
}

export const CommitFileList: React.FC<CommitFileListProps> = ({
  title,
  entries,
  rootPath,
  e2eId,
}) => {
  const [collapsed, setCollapsed] = useState(false);

  const toggleCollapse = () => setCollapsed((c) => !c);

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
              />
            ))
          )}
        </div>
      )}
    </div>
  );
};

/** 单个文件条目 */
interface CommitFileItemProps {
  entry: GitStatusEntry;
  rootPath: string;
}

const CommitFileItem: React.FC<CommitFileItemProps> = ({
  entry,
  rootPath,
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
