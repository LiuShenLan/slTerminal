// ExplorerPanel.tsx — 文件浏览器侧栏面板容器
//
// 职责：
// - 展示文件树（跟随活跃项目根路径）
// - 双击文件 → 在焦点操作页面打开编辑器面板
// - 右键菜单 CRUD 操作
// - 键盘快捷键（Del/Enter/F2）经 ShortcutRegistry + active pointer 派发

import React, { useEffect, useCallback, useState, useRef, useMemo } from "react";
import { useFileTree } from "./useFileTree";
import { FileTree } from "./FileTree";
import { createDir, deleteEntry, rename, writeFile } from "../../ipc/fs";
import { startWatch } from "../../ipc/notify";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { titleManager } from "../../workspace/titleManager";
import {
  EXPLORER_COLORS,
  SEPARATOR_BG,
  INPUT_BORDER,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
} from "../../theme";
import { PANEL_TERMINAL, PANEL_EDITOR, isAlwaysRenderPanel } from "../../panelRegistry";
import { fileViewerRegistry } from "../fileViewers";
import { usePanelFocus } from "../shortcuts/usePanelFocus";
import { setActiveExplorer, clearActiveExplorer } from "./activeExplorer";
import { basename } from "../../lib/path";
import { ask } from "../../ipc/dialog";

/** 操作失败错误提示自动消失时间（ms） */
const ERROR_AUTO_DISMISS_MS = 5000;

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

  // --- 选中模型 ---
  const [selectedPath, setSelectedPath] = useState<string | null>(null);

  // --- 重命名状态（从 FileTree 上提） ---
  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  // --- 焦点管理 ---
  const containerRef = useRef<HTMLDivElement | null>(null);

  // 操作失败内联错误提示
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showError = useCallback((msg: string) => {
    setErrorMsg(msg);
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    errorTimerRef.current = setTimeout(() => setErrorMsg(null), ERROR_AUTO_DISMISS_MS);
  }, []);

  // 清理定时器
  useEffect(() => {
    return () => {
      if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    };
  }, []);

  // --- Active explorer actions（供快捷键 handler 派发） ---
  // 对齐 terminal/editor 的 ref 模式：useMemo 空 deps，所有数据通过 ref 间接访问，
  // 确保 actions 对象引用稳定——active pointer 中永不持有过期闭包。

  const selectedPathRef = useRef<string | null>(null);
  selectedPathRef.current = selectedPath; // 每次渲染同步最新值

  const isRenamingRef = useRef<() => boolean>(() => false);
  isRenamingRef.current = () => renamingPath !== null;

  const handleDeleteSelected = useCallback(async () => {
    const path = selectedPathRef.current;
    if (!path) return;
    const name = basename(path);
    const ok = await ask(`确定删除 "${name}"？此操作不可撤销。`, {
      title: "确认删除",
      kind: "warning",
    });
    if (!ok) return;
    try {
      await deleteEntry(path);
      setSelectedPath(null);
      refresh();
    } catch (err) {
      console.error("删除失败:", err);
      showError(`删除失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [refresh, showError]);

  const deleteSelectedRef = useRef(handleDeleteSelected);
  deleteSelectedRef.current = handleDeleteSelected;

  const handleOpenSelected = useCallback(() => {
    const path = selectedPathRef.current;
    if (!path) return;
    const findNode = (nodes: typeof rootNodes, targetPath: string): boolean | null => {
      for (const n of nodes) {
        if (n.entry.path === targetPath) return n.entry.isDir;
        if (n.children.length > 0) {
          const found = findNode(n.children, targetPath);
          if (found !== null) return found;
        }
      }
      return null;
    };
    const isDir = findNode(rootNodes, path);
    if (isDir) {
      toggleExpand(path);
      return;
    }
    handleOpenFile(path);
  }, [rootNodes, toggleExpand]);

  const openSelectedRef = useRef(handleOpenSelected);
  openSelectedRef.current = handleOpenSelected;

  const handleRenameSelected = useCallback(() => {
    const path = selectedPathRef.current;
    if (!path) return;
    setRenamingPath(path);
    setRenameValue(basename(path));
  }, []);

  const renameSelectedRef = useRef(handleRenameSelected);
  renameSelectedRef.current = handleRenameSelected;

  // eslint-disable-next-line react-hooks/exhaustive-deps -- 空依赖：所有数据通过 ref 访问，对象引用永久稳定
  const explorerActions = useMemo(
    () => ({
      getSelectedPath: () => selectedPathRef.current,
      deleteSelected: async () => { await deleteSelectedRef.current(); },
      openSelected: () => { openSelectedRef.current(); },
      renameSelected: () => { renameSelectedRef.current(); },
      isRenaming: () => isRenamingRef.current(),
    }),
    [],
  );

  const activate = useCallback(() => setActiveExplorer(explorerActions), [explorerActions]);
  const deactivate = useCallback(() => clearActiveExplorer(explorerActions), [explorerActions]);

  usePanelFocus("explorer", containerRef.current, activate, deactivate);

  // 当项目根路径变化时启动文件监听（后端 emit fs-event → 前端增量刷新）
  useEffect(() => {
    if (projectRootPath) {
      startWatch(projectRootPath).catch((err) =>
        console.error("启动文件监听失败:", err),
      );
    }
  }, [projectRootPath]);

  // rootPath 变化时重置选中和重命名状态
  useEffect(() => {
    setSelectedPath(null);
    setRenamingPath(null);
  }, [rootPath]);

  /** 双击文件 → 打开编辑器面板 */
  const handleOpenFile = useCallback(
    (filePath: string) => {
      if (!activePageId) return;
      const dockApi = window.__dockviewApi;
      if (!dockApi) return;

      // 去重：相同文件路径不重复打开，聚焦已有面板
      const existingPanelId = titleManager.findExistingEditor(
        activePageId,
        filePath,
      );
      if (existingPanelId) {
        const existingPanel = dockApi.getPanel(existingPanelId);
        if (existingPanel) {
          existingPanel.focus();
          return;
        }
      }

      // 通过 FileViewerRegistry 决定面板类型（未知类型回退 editor）
      const panelType = fileViewerRegistry.resolve(filePath) ?? PANEL_EDITOR;

      // 计算标题（无闪烁——addPanel 时直接传入）
      const root = projectRootPath || rootPath || "";
      const title = root
        ? titleManager.getFileEditorTitle(activePageId, root, filePath)
        : titleManager.getFileEditorTitle(activePageId, "", filePath);

      const panelId = `${panelType}-${Date.now()}`;
      // 文件预览类面板（htmlviewer 等）使用 renderer: "always" 保持 iframe/canvas
      // browsing context 存活，避免页签切换/分屏时 DOM 移除导致白屏闪屏
      const renderer = isAlwaysRenderPanel(panelType) ? ("always" as const) : undefined;

      // addPanel 可能抛异常（如布局状态不一致），try-catch 防止 titleManager 状态污染
      try {
        dockApi.addPanel({
          id: panelId,
          component: panelType,
          title,
          params: { panelId, filePath },
          ...(renderer ? { renderer } : {}),
        });
      } catch {
        // 面板创建失败，跳过标题注册（titleManager 与 DOM 保持无孤记录）
        return;
      }

      // 仅在 addPanel 成功后注册到标题管理器（保持两状态一致）
      titleManager.registerEditor(activePageId, panelId, filePath);

      // 新文件打开后重算整个页面标题（可能触发既有面板的冲突更新）
      if (root) {
        const apiForUpdates = window.__dockviewApi;
        if (apiForUpdates) {
          const updates = titleManager.recomputeTitles(activePageId, root);
          for (const { panelId: pid, title: t } of updates) {
            const p = apiForUpdates.getPanel(pid);
            if (p) p.api.setTitle(t);
          }
        }
      }
    },
    [activePageId, projectRootPath, rootPath],
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
        const title = activePageId
          ? titleManager.getTerminalTitle(activePageId)
          : "terminal";
        dockApi.addPanel({
          id: panelId,
          component: PANEL_TERMINAL,
          title,
          params: { panelId, cwd: dir },
          renderer: "always",
        });
      }
    },
    [activePageId],
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
        setRenamingPath(null);
        setRenameValue("");
        refresh();
      } catch (err) {
        console.error("重命名失败:", err);
        showError(`重命名失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    },
    [refresh, showError],
  );

  /** 取消重命名 */
  const handleRenameCancel = useCallback(() => {
    setRenamingPath(null);
    setRenameValue("");
  }, []);

  /** 删除（保留右键菜单使用） */
  const handleDelete = useCallback(
    async (filePath: string) => {
      try {
        await deleteEntry(filePath);
        if (selectedPath === filePath) setSelectedPath(null);
        refresh();
      } catch (err) {
        console.error("删除失败:", err);
        showError(`删除失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    },
    [refresh, showError, selectedPath],
  );

  /** 新建文件 */
  const handleNewFile = useCallback(
    async (path: string) => {
      try {
        await writeFile(path, "");
        refresh();
      } catch (err) {
        console.error("新建文件失败:", err);
        showError(`新建文件失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    },
    [refresh, showError],
  );

  /** 新建文件夹 */
  const handleNewFolder = useCallback(
    async (path: string) => {
      try {
        await createDir(path);
        refresh();
      } catch (err) {
        console.error("新建文件夹失败:", err);
        showError(`新建文件夹失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    },
    [refresh, showError],
  );

  /** 单击行 → 选中 + 聚焦容器 */
  const handleSelect = useCallback(
    (path: string | null) => {
      setSelectedPath(path);
      // 单击即聚焦容器（建立 explorer context）
      if (containerRef.current) {
        containerRef.current.focus();
      }
    },
    [],
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
          borderBottom: `1px solid ${SEPARATOR_BG}`,
          height: 28,
          fontSize: 11,
          color: INPUT_BORDER,
          textTransform: "uppercase",
          letterSpacing: 1,
          userSelect: "none",
          flexShrink: 0,
        }}
      >
        文件浏览器
      </div>

      {/* 操作失败内联错误提示 */}
      {errorMsg && (
        <div
          data-testid="explorer-error-banner"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "4px 8px",
            background: ERROR_BANNER_BG,
            borderBottom: `1px solid ${ERROR_BANNER_BORDER}`,
            color: ERROR_BANNER_FG,
            fontSize: 12,
            flexShrink: 0,
            minHeight: 24,
          }}
        >
          <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {errorMsg}
          </span>
          <button
            onClick={() => setErrorMsg(null)}
            style={{
              background: "none",
              border: "none",
              color: ERROR_BANNER_FG,
              cursor: "pointer",
              fontSize: 14,
              padding: "0 4px",
              lineHeight: 1,
            }}
            aria-label="关闭错误提示"
          >
            ×
          </button>
        </div>
      )}

      {/* 文件树容器（tabIndex 使容器可聚焦，usePanelFocus 监听 focusin/focusout） */}
      <div
        ref={containerRef}
        tabIndex={-1}
        style={{
          flex: 1,
          overflowY: "auto",
          overflowX: "hidden",
          padding: "2px 0",
          outline: "none",
        }}
        data-e2e="explorer-tree-container"
      >
        {rootPath ? (
          <FileTree
            rootPath={rootPath}
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
            // 新增 props：选中模型
            selectedPath={selectedPath}
            onSelect={handleSelect}
            // 新增 props：重命名状态上提
            renamingPath={renamingPath}
            renameValue={renameValue}
            onRenameStart={(path: string, name: string) => {
              setRenamingPath(path);
              setRenameValue(name);
            }}
            onRenameCancel={handleRenameCancel}
          />
        ) : (
          <div
            style={{
              padding: 16,
              color: INPUT_BORDER,
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
