// ExplorerPanel.tsx — 文件浏览器侧栏面板容器（薄壳）
//
// 职责（薄壳化后）：
// - 标题栏「文件浏览器」
// - 活跃项目 rootPath 推导（projects + activePageId）
// - 树交互全部委托 FileTreeExplorer（共享组件——选中/重命名/CRUD/打开/焦点/错误横幅）
//
// 红线：文件浏览器行为零回归——交互逻辑平移至 FileTreeExplorer 原样承载。

import React from "react";
import { FileTreeExplorer } from "./FileTreeExplorer";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { EXPLORER_COLORS, SEPARATOR_BG, DIM_FG } from "../../theme";
import type { SideViewComponentProps } from "../sideViews/sideViewRegistry";

/**
 * canOpenFile re-export：打开链路单点已迁 src/workspace/openFile.ts
 * （explorer/index.ts 导出面与既有单测兼容，勿在此另定义）。
 */
export { canOpenFile } from "../../workspace/openFile";

export const ExplorerPanel: React.FC<SideViewComponentProps> = ({
  viewState,
  onViewStateChange,
}) => {
  const projects = useProjects((s) => s.projects);
  const activePageId = useLayout((s) => s.activePageId);

  // 查找活跃项目的根路径
  let rootPath: string | null = null;
  let projectRootPath: string | null = null;
  if (activePageId) {
    for (const [, proj] of Object.entries(projects)) {
      const activePage = proj.pages.find(
        (p) => p.pageId === activePageId,
      );
      if (activePage) {
        rootPath = proj.rootPath;
        projectRootPath = proj.rootPath;
        break;
      }
    }
  }

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: EXPLORER_COLORS.bg,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* 标题栏 */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          padding: "4px 8px",
          borderBottom: `1px solid ${SEPARATOR_BG}`,
          height: 28,
          fontSize: 11,
          // UI-206：分组标题 fg-3（DIM_FG）+ 字距 0.08em
          color: DIM_FG,
          textTransform: "uppercase",
          letterSpacing: "0.08em",
          userSelect: "none",
          flexShrink: 0,
        }}
      >
        文件浏览器
      </div>

      <FileTreeExplorer
        rootPath={rootPath}
        projectRootPath={projectRootPath}
        viewState={viewState}
        onViewStateChange={onViewStateChange}
      />
    </div>
  );
};
