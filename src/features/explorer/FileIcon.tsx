// FileIcon.tsx — 文件/文件夹类型图标（UI-602：emoji → 描边 SVG 重构）+ git 状态色叠加
//
// 文件夹 = 描边款 SVG（src/lib/icons.tsx 的 IconFolder，IC-01 图标单点，禁止直接用 lucide）；
//   颜色 = git 状态色（与文件分支同一映射），无状态/未命中映射回退 EXPLORER_COLORS.fg
// 文件   = 描边 + 小色块款自绘 SVG：
//   - 轮廓/折角描边色 = 当前色（git 状态色，无则 EXPLORER_COLORS.fg）
//   - 左缘小色块 fill = 扩展名映射色（六色盘，见下方映射表）
//   - 色块与描边分层：git 状态色覆盖描边，类型色块保留——两者叠加互不遮蔽
//
// 扩展名 → 色系映射表（执行期定；彩色严格限六色盘 FILE_COLORS）：
//   ts/tsx         → 蓝 #7fa8e8
//   js/jsx/mjs/cjs → 黄 #d6b25e
//   py/pyw         → 绿 #93b573
//   rs             → 红 #d9706b
//   json/jsonc     → 黄 #d6b25e（同 JS）
//   md/markdown    → 紫 #b48ce0
//   html/htm       → 青 #6fbfc4
//   css/scss/less  → 蓝 #7fa8e8（同 TS）
//   配置（xml/svg/toml/yaml/yml/.gitignore/.gitattributes）→ 灰青（主题灰 token 表达）
//   默认           → 灰（无色块，仅描边轮廓）

import React from "react";
import { GIT_FILE_COLORS, EXPLORER_COLORS } from "../../theme";
import { IconFolder } from "../../lib/icons";

/** git 状态到颜色 token 的映射（GIT_FILE_COLORS 逻辑不变） */
const statusColorMap: Record<string, string> = {
  modified: GIT_FILE_COLORS.modified,
  added: GIT_FILE_COLORS.added,
  untracked: GIT_FILE_COLORS.untracked,
  deleted: GIT_FILE_COLORS.deleted,
  renamed: GIT_FILE_COLORS.renamed,
  conflict: GIT_FILE_COLORS.conflict,
  ignored: GIT_FILE_COLORS.ignored,
};

/** 六色盘（UI-602 checklist 指定色值——本组件硬编码例外，映射表见文件头注释） */
const FILE_COLORS = {
  blue: "#7fa8e8", // 蓝
  yellow: "#d6b25e", // 黄
  green: "#93b573", // 绿
  red: "#d9706b", // 红
  purple: "#b48ce0", // 紫
  cyan: "#6fbfc4", // 青
} as const;

/** 文件扩展名 → 小色块颜色；无映射（默认文件）返回 null——不画色块，仅描边 */
function fileColor(ext: string): string | null {
  switch (ext) {
    case ".ts":
    case ".tsx":
      return FILE_COLORS.blue; // TS 蓝
    case ".js":
    case ".jsx":
    case ".mjs":
    case ".cjs":
      return FILE_COLORS.yellow; // JS 黄
    case ".py":
    case ".pyw":
      return FILE_COLORS.green; // Python 绿
    case ".rs":
      return FILE_COLORS.red; // Rust 红
    case ".json":
    case ".jsonc":
      return FILE_COLORS.yellow; // JSON 黄（同 JS）
    case ".md":
    case ".markdown":
      return FILE_COLORS.purple; // Markdown 紫
    case ".html":
    case ".htm":
      return FILE_COLORS.cyan; // HTML 青
    case ".css":
    case ".scss":
    case ".less":
      return FILE_COLORS.blue; // CSS 蓝（同 TS）
    case ".xml":
    case ".svg":
    case ".toml":
    case ".yaml":
    case ".yml":
    case ".gitignore":
    case ".gitattributes":
      return EXPLORER_COLORS.fg; // 配置 → 灰青（主题灰 token）
    default:
      return null; // 默认 → 灰（无色块）
  }
}

interface FileIconProps {
  name: string;
  isDir: boolean;
  gitStatus?: string;
}

/** 文件描边宽度 (px)，与图标体系 IC-01 的 1.5px 描边规格对齐（14px 小尺寸取 1.4） */
const STROKE_WIDTH = 1.4;

export const FileIcon: React.FC<FileIconProps> = ({
  name,
  isDir,
  gitStatus,
}) => {
  // git 状态色（文件/文件夹通用同一映射）；无状态/未命中映射回退默认前景色
  const color = statusColorMap[gitStatus ?? ""] ?? EXPLORER_COLORS.fg;

  if (isDir) {
    return (
      <span
        style={{
          color,
          display: "inline-flex",
          alignItems: "center",
        }}
      >
        <IconFolder size={14} />
      </span>
    );
  }

  const ext = name.includes(".")
    ? name.slice(name.lastIndexOf(".")).toLowerCase()
    : "";
  const typeColor = fileColor(ext);

  return (
    <svg
      width={14}
      height={14}
      viewBox="0 0 14 14"
      aria-hidden="true"
      style={{ display: "block", flexShrink: 0 }}
    >
      {/* 文件轮廓（右上折角） */}
      <path
        d="M3.5 1.5H8L11 4.5v6.5a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 2 11V3a1.5 1.5 0 0 1 1.5-1.5Z"
        fill="none"
        stroke={color}
        strokeWidth={STROKE_WIDTH}
        strokeLinejoin="round"
      />
      {/* 折角线 */}
      <path
        d="M8 1.5v3h3"
        fill="none"
        stroke={color}
        strokeWidth={STROKE_WIDTH}
        strokeLinejoin="round"
      />
      {/* 左缘小色块：类型色（默认文件无映射则省略） */}
      {typeColor != null && (
        <rect x={2.9} y={3.8} width={1.5} height={6.4} rx={0.75} fill={typeColor} />
      )}
    </svg>
  );
};
