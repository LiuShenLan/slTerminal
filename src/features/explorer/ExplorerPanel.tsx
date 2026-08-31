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
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { titleManager } from "../../workspace/titleManager";
import {
  EXPLORER_COLORS,
  SEPARATOR_BG,
  PLACEHOLDER_FG,
  DIM_FG,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
} from "../../theme";
import { PANEL_TERMINAL, PANEL_EDITOR, isAlwaysRenderPanel } from "../../panelRegistry";
import { fileViewerRegistry } from "../fileViewers";
import { usePanelFocus } from "../shortcuts/usePanelFocus";
import { setActiveExplorer, clearActiveExplorer } from "./activeExplorer";
import { basename } from "../../lib/path";
import { confirmDialog } from "../../lib/ConfirmDialog";
import { getErrorMessage } from "../../lib";
import { IconClose, IconEmptyBox, IconAlertTriangle } from "../../lib/icons";

/** 操作失败错误提示自动消失时间（ms） */
const ERROR_AUTO_DISMISS_MS = 5000;

/**
 * handleOpenFile 前置守卫：无活跃操作页或无 Dockview API 时禁止打开面板。
 * 导出供单测直测——UI 路径上 activePageId 为 null 时 rootPath 亦为 null、
 * FileTree 不渲染，双击路径不可达（防御性代码仍需锁定行为）。
 * type predicate 使调用处 activePageId 自动收窄为非 null。
 */
export const canOpenFile = (
  activePageId: string | null,
  dockviewApi: unknown,
): activePageId is string => !!activePageId && !!dockviewApi;

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

  const { rootNodes, gitStatusMap, rootError, toggleExpand, refresh } = useFileTree({ rootPath });

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
    const ok = await confirmDialog({
      title: "确认删除",
      message: `确定删除 "${name}"？此操作不可撤销。`,
      kind: "warning",
      danger: true,
    });
    if (!ok) return;
    try {
      await deleteEntry(path);
      setSelectedPath(null);
      refresh();
    } catch (err) {
      console.error("删除失败:", err);
      showError(`删除失败: ${getErrorMessage(err)}`);
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

  // 空依赖：所有数据通过 ref 访问，对象引用永久稳定
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

  // 文件监听已上提至 Workspace 项目激活层（SEC-01 effect）——fs-event 消费方
  // （编辑器外部修改 reload/commit 面板）不依赖 explorer 视图打开；本组件仅消费
  // onFsEvent 增量刷新，不再管理 watcher 生命周期（防双管理互停）。

  // rootPath 变化时重置选中和重命名状态
  useEffect(() => {
    setSelectedPath(null);
    setRenamingPath(null);
  }, [rootPath]);

  /** 双击文件 → 打开编辑器面板 */
  const handleOpenFile = useCallback(
    (filePath: string) => {
      const dockApi = window.__dockviewApi;
      // 前置守卫：无活跃操作页或无 Dockview API 时直接返回
      //（canOpenFile 导出供单测直测，predicate 收窄 activePageId 非 null）
      if (!dockApi) return;
      if (!canOpenFile(activePageId, dockApi)) return;

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
      // 兜底短路：新旧路径相同（防未来其他调用方直传原名）→ 静默退出编辑态，
      // 不发 IPC——同名提交会触发后端 src==dst 覆盖分支误删源文件。
      // 正常同名已由 FileTree.confirmRename 拦截，此处为防御层（不调 refresh，
      // 磁盘未变；不引 handleRenameCancel——其定义在本函数之后，闭包有 TDZ 风险）。
      if (oldPath === newPath) {
        setRenamingPath(null);
        setRenameValue("");
        return;
      }
      try {
        await rename(oldPath, newPath);
        setRenamingPath(null);
        setRenameValue("");
        refresh();
      } catch (err) {
        console.error("重命名失败:", err);
        showError(`重命名失败: ${getErrorMessage(err)}`);
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
        showError(`删除失败: ${getErrorMessage(err)}`);
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
        showError(`新建文件失败: ${getErrorMessage(err)}`);
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
        showError(`新建文件夹失败: ${getErrorMessage(err)}`);
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
              display: "flex",
              alignItems: "center",
              padding: "0 4px",
              lineHeight: 1,
            }}
            aria-label="关闭错误提示"
          >
            <IconClose size={14} />
          </button>
        </div>
      )}

      {/* 文件树容器（tabIndex 使容器可聚焦，usePanelFocus 监听 focusin/focusout；不设 outline 抑制——全局 :focus-visible 环接管，鼠标点击不匹配 :focus-visible，键盘编程聚焦时可见，UI-808） */}
      <div
        ref={containerRef}
        tabIndex={-1}
        style={{
          flex: 1,
          overflowY: "auto",
          overflowX: "hidden",
          padding: "2px 0",
        }}
        data-e2e="explorer-tree-container"
      >
        {rootPath ? (
          rootError ? (
            // FE-07: 根目录加载失败 → 错误占位（错误消息 + 重试按钮），不再伪装空目录
            <div
              data-testid="explorer-load-error"
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                gap: 8,
                padding: 16,
                fontSize: 12,
                color: DIM_FG, // 说明文字 fg-3
                textAlign: "center",
                userSelect: "none",
              }}
            >
              <span style={{ color: ERROR_BANNER_FG, display: "flex" }}>
                <IconAlertTriangle size={15} />
              </span>
              <span style={{ color: ERROR_BANNER_FG }}>文件树加载失败</span>
              <span style={{ wordBreak: "break-all" }}>{rootError}</span>
              <button
                data-testid="explorer-load-retry"
                onClick={() => {
                  refresh();
                }}
                style={{
                  marginTop: 4,
                  background: "none",
                  border: `1px solid ${ERROR_BANNER_BORDER}`,
                  borderRadius: 4,
                  color: ERROR_BANNER_FG,
                  fontSize: 12,
                  padding: "4px 14px",
                  cursor: "pointer",
                }}
              >
                重试
              </button>
            </div>
          ) : (
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
          )
        ) : (
          // GL-05：空文件树统一空态——15px 线性图标 fg-4 + 说明文字 fg-3，居中
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 8,
              padding: 16,
              fontSize: 12,
              color: DIM_FG, // 说明文字 fg-3
              textAlign: "center",
              userSelect: "none",
            }}
          >
            <span style={{ color: PLACEHOLDER_FG, display: "flex" }}>
              <IconEmptyBox size={15} />
            </span>
            <span>选择一个项目以浏览文件</span>
          </div>
        )}
      </div>
    </div>
  );
};
