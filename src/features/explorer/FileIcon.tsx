// FileIcon.tsx — 文件类型图标 + git 状态色叠加
//
// 用 Unicode 符号表示文件/文件夹类型，git 状态色覆盖前景色。

import React from "react";
import { GIT_FILE_COLORS, EXPLORER_COLORS } from "../../theme";

/** git 状态到颜色 token 的映射 */
const statusColorMap: Record<string, string> = {
  modified: GIT_FILE_COLORS.modified,
  added: GIT_FILE_COLORS.added,
  untracked: GIT_FILE_COLORS.untracked,
  deleted: GIT_FILE_COLORS.deleted,
  renamed: GIT_FILE_COLORS.renamed,
  conflict: GIT_FILE_COLORS.conflict,
  ignored: GIT_FILE_COLORS.ignored,
};

/** 文件扩展名 → Unicode 图标 */
function fileIcon(ext: string): string {
  switch (ext) {
    case ".ts":
    case ".tsx":
      return "\u{1F596}"; // TS 蓝
    case ".js":
    case ".jsx":
    case ".mjs":
    case ".cjs":
      return "\u{1F4DC}"; // JS 黄
    case ".rs":
      return "\u{2699}\u{FE0F}"; // Rust
    case ".py":
    case ".pyw":
      return "\u{1F40D}"; // Python
    case ".json":
    case ".jsonc":
      return "\u{1F4CB}"; // JSON
    case ".md":
    case ".markdown":
      return "\u{1F4DD}"; // Markdown
    case ".html":
    case ".htm":
      return "\u{1F310}"; // HTML
    case ".css":
    case ".scss":
    case ".less":
      return "\u{1F3A8}"; // CSS
    case ".xml":
    case ".svg":
      return "\u{1F4C4}"; // XML
    case ".toml":
    case ".yaml":
    case ".yml":
      return "\u{2699}\u{FE0F}"; // 配置
    case ".gitignore":
    case ".gitattributes":
      return "\u{1F4E6}"; // Git
    default:
      return "\u{1F4C4}"; // 默认文件
  }
}

interface FileIconProps {
  name: string;
  isDir: boolean;
  gitStatus?: string;
}

export const FileIcon: React.FC<FileIconProps> = ({
  name,
  isDir,
  gitStatus,
}) => {
  const color = gitStatus ? statusColorMap[gitStatus] : EXPLORER_COLORS.fg;

  if (isDir) {
    return (
      <span style={{ color: EXPLORER_COLORS.fg, fontSize: 14 }}>
        {"\u{1F4C1}"}
      </span>
    );
  }

  const ext = name.includes(".")
    ? name.slice(name.lastIndexOf(".")).toLowerCase()
    : "";

  return (
    <span style={{ color, fontSize: 14, filter: "grayscale(0.3)" }}>
      {fileIcon(ext)}
    </span>
  );
};
