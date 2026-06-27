// ExplorerPanel.tsx — 文件浏览器侧栏面板容器
//
// 职责：
// - 展示文件树（跟随活跃项目根路径）
// - 双击文件 → 在焦点操作页面打开编辑器面板
// - 右键菜单 CRUD 操作

import React, { useEffect, useCallback } from "react";
import { useFileTree } from "./useFileTree";
import { FileTree } from "./FileTree";
import { createDir, deleteEntry, rename, writeFile } from "../../ipc/fs";
import { startWatch } from "../../ipc/notify";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { EXPLORER_COLORS } from "../../theme";

export const ExplorerPanel: React.FC = () => {
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
        rootPath = activePage.cwd || proj.rootPath;
        projectRootPath = proj.rootPath;
        break;
      }
    }
  }

  const { rootNodes, gitStatusMap, toggleExpand, refresh } = useFileTree({ rootPath });

  // 当活跃项目变化时刷新
  useEffect(() => {
    refresh();
  }, [rootPath, refresh]);

  // 当项目根路径变化时启动文件监听（后端 emit fs-event → 前端增量刷新）
  useEffect(() => {
    if (projectRootPath) {
      startWatch(projectRootPath).catch((err) =>
        console.error("启动文件监听失败:", err),
      );
    }
  }, [projectRootPath]);

  /** 双击文件 → 打开编辑器面板 */
  const handleOpenFile = useCallback(
    (filePath: string) => {
      if (!activePageId) return;
      // 通过全局 dockview API 在活跃页面中打开编辑器
      const dockApi = window.__dockviewApi;
      if (dockApi) {
        const panelId = `editor-${Date.now()}`;
        dockApi.addPanel({
          id: panelId,
          component: "editor",
          params: { panelId, filePath },
        });
      }
    },
    [activePageId],
  );

  /** 在终端中打开（打开文件所在目录的终端） */
  const handleOpenInTerminal = useCallback(
    (path: string) => {
      const dockApi = window.__dockviewApi;
      if (dockApi) {
        // 获取文件所在目录
        const dir =
          path.lastIndexOf("/") >= 0
            ? path.slice(0, path.lastIndexOf("/"))
            : path;
        const panelId = `terminal-open-${Date.now()}`;
        dockApi.addPanel({
          id: panelId,
          component: "terminal",
          params: { panelId, cwd: dir },
          renderer: "always",
        });
      }
    },
    [],
  );

  /** 重命名 */
  const handleRename = useCallback(
    async (oldPath: string, newName: string) => {
      const parentDir =
        oldPath.lastIndexOf("/") >= 0
          ? oldPath.slice(0, oldPath.lastIndexOf("/"))
          : "";
      const newPath = parentDir ? `${parentDir}/${newName}` : newName;
      try {
        await rename(oldPath, newPath);
        refresh();
      } catch (err) {
        console.error("重命名失败:", err);
      }
    },
    [refresh],
  );

  /** 删除 */
  const handleDelete = useCallback(
    async (filePath: string) => {
      try {
        await deleteEntry(filePath);
        refresh();
      } catch (err) {
        console.error("删除失败:", err);
      }
    },
    [refresh],
  );

  /** 新建文件 */
  const handleNewFile = useCallback(
    async (path: string) => {
      try {
        await writeFile(path, "");
        refresh();
      } catch (err) {
        console.error("新建文件失败:", err);
      }
    },
    [refresh],
  );

  /** 新建文件夹 */
  const handleNewFolder = useCallback(
    async (path: string) => {
      try {
        await createDir(path);
        refresh();
      } catch (err) {
        console.error("新建文件夹失败:", err);
      }
    },
    [refresh],
  );

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
          borderBottom: "1px solid #333",
          height: 28,
          fontSize: 11,
          color: "#6C6C6C",
          textTransform: "uppercase",
          letterSpacing: 1,
          userSelect: "none",
          flexShrink: 0,
        }}
      >
        文件浏览器
      </div>

      {/* 文件树 */}
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          overflowX: "hidden",
          padding: "2px 0",
        }}
      >
        {rootPath ? (
          rootNodes.length === 0 ? (
            <div
              style={{
                padding: 16,
                color: "#6C6C6C",
                fontSize: 12,
                textAlign: "center",
              }}
            >
              空目录
            </div>
          ) : (
            <FileTree
              nodes={rootNodes}
              depth={0}
              gitStatusMap={gitStatusMap}
              onToggleExpand={toggleExpand}
              onOpenFile={handleOpenFile}
              onOpenInTerminal={handleOpenInTerminal}
              onRename={handleRename}
              onDelete={handleDelete}
              onNewFile={handleNewFile}
              onNewFolder={handleNewFolder}
            />
          )
        ) : (
          <div
            style={{
              padding: 16,
              color: "#6C6C6C",
              fontSize: 12,
              textAlign: "center",
            }}
          >
            选择一个项目以浏览文件
          </div>
        )}
      </div>
    </div>
  );
};
