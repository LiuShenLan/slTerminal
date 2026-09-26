// FileTreeExplorer.tsx — 文件树浏览共享组件（自 ExplorerPanel 抽取）
//
// 职责：文件树的全部交互能力——树渲染/选中模型/重命名/CRUD 右键菜单/双击打开/
// 错误横幅/焦点管理（usePanelFocus + activeExplorer 指针）。
// 「文件浏览器」（ExplorerPanel 薄壳：标题栏 + 活跃项目 rootPath 推导）与
// 「Agent 全局文件」（AgentFilesPanel：rootFilter/eventPathFilter/定制打开守卫）
// 各为薄壳消费本组件——同一逻辑不维护两处（架构约束：文件树单点复用）。
//
// 焦点语义：多实例共存时（explorer + agent 视图同开）explorer.* 快捷键经
// activeExplorer 指针派发给最后聚焦实例——两侧共用 "explorer" context，语义自洽。

import React, { useEffect, useCallback, useState, useRef, useMemo } from "react";
import { useFileTree } from "./useFileTree";
import { FileTree } from "./FileTree";
import { createDir, deleteEntry, rename, writeFile } from "../../ipc/fs";
import { useLayout } from "../../stores/layout";
import { panelIdInPage, resolveFocusedGroupForAdd } from "../../workspace/pageGroups";
import { markPanelFocusIntent } from "../../workspace/panelFocusIntent";
import { titleManager } from "../../workspace/titleManager";
import { openFileInPage } from "../../workspace/openFile";
import {
  PLACEHOLDER_FG,
  DIM_FG,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
} from "../../theme";
import { PANEL_TERMINAL } from "../../panelRegistry";
import { usePanelFocus } from "../shortcuts/usePanelFocus";
import { setActiveExplorer, clearActiveExplorer } from "./activeExplorer";
import { basename } from "../../lib/path";
import { confirmDialog } from "../../lib/ConfirmDialog";
import { getErrorMessage } from "../../lib";
import { IconClose, IconEmptyBox, IconAlertTriangle } from "../../lib/icons";

/** 操作失败错误提示自动消失时间（ms） */
const ERROR_AUTO_DISMISS_MS = 5000;

export interface FileTreeExplorerProps {
  /** 树根绝对路径；null → 渲染 emptyState */
  rootPath: string | null;
  /** 打开文件链路的项目根（标题相对路径计算基准）；agent 视图传 null（标题 = basename） */
  projectRootPath?: string | null;
  /** 展开态跨挂载快照（CP-016 槽位回填） */
  viewState?: unknown;
  /** 展开态变化上呼（CP-016 槽位提交） */
  onViewStateChange?: (state: unknown) => void;
  /** 根层条目过滤器（透传 useFileTree；须 useCallback 稳定引用） */
  rootFilter?: (name: string) => boolean;
  /** fs-event 路径二次过滤器（透传 useFileTree；须 useCallback 稳定引用） */
  eventPathFilter?: (absPath: string) => boolean;
  /** 双击打开文件（缺省 = openFileInPage 共享链路；agent 视图传定制版做无项目 toast 守卫） */
  onOpenFile?: (filePath: string) => void;
  /** rootPath 为 null 时的空态内容（缺省 = 「选择一个项目以浏览文件」统一空态） */
  emptyState?: React.ReactNode;
  /** data-testid/data-e2e 前缀（缺省 "explorer" 保持文件浏览器选择器零回归；
   *  agent 视图传 "agent-files" 防同页多实例撞名） */
  testIdPrefix?: string;
}

export const FileTreeExplorer: React.FC<FileTreeExplorerProps> = ({
  rootPath,
  projectRootPath = null,
  viewState,
  onViewStateChange,
  rootFilter,
  eventPathFilter,
  onOpenFile,
  emptyState,
  testIdPrefix = "explorer",
}) => {
  const activePageId = useLayout((s) => s.activePageId);

  const { rootNodes, gitStatusMap, rootError, toggleExpand, refresh } = useFileTree({
    rootPath,
    viewState,
    onViewStateChange,
    rootFilter,
    eventPathFilter,
  });

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

  /** 默认打开链路：双击文件 → 经 openFileInPage 在焦点操作页面打开面板 */
  const defaultOpenFile = useCallback(
    (filePath: string) => {
      openFileInPage(
        {
          activePageId,
          dockApi: window.__dockviewApi,
          rootPath,
          projectRootPath,
        },
        filePath,
      );
    },
    [activePageId, projectRootPath, rootPath],
  );
  // ref 承载最终打开实现（onOpenFile 定制覆盖；handleOpenSelected 经 ref 读取防闭包过期）
  const openFileRef = useRef(onOpenFile ?? defaultOpenFile);
  openFileRef.current = onOpenFile ?? defaultOpenFile;
  const handleOpenFile = useCallback(
    (filePath: string) => openFileRef.current(filePath),
    [],
  );

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
    openFileRef.current(path);
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

  // rootPath 变化时重置选中和重命名状态
  useEffect(() => {
    setSelectedPath(null);
    setRenamingPath(null);
  }, [rootPath]);

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
        if (!activePageId) return;
        // CP-004：terminal localId 页前缀协议（terminal-open-{ts} 为免撞号 local
        // 形态——不占页组 seq 计数）；ADR-0020 落组经 resolveFocusedGroupForAdd
        // （聚焦组优先 ?? 主组 ?? 页内首组——分屏后主组可能被拖空删除；
        // 页无组解析 null → 不落活跃组防错页）
        const group = resolveFocusedGroupForAdd(dockApi, activePageId);
        if (!group) return;
        const localId = `terminal-open-${Date.now()}`;
        const panelId = panelIdInPage(activePageId, localId);
        const title = titleManager.getTerminalTitle(activePageId);
        // C4：键盘聚焦意图（新建面板挂载即激活路径的焦点驱动）
        markPanelFocusIntent(panelId);
        dockApi.addPanel({
          id: panelId,
          component: PANEL_TERMINAL,
          title,
          params: { panelId, cwd: dir },
          renderer: "always",
          position: { referenceGroup: group.id },
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
    <>
      {/* 操作失败内联错误提示 */}
      {errorMsg && (
        <div
          data-testid={`${testIdPrefix}-error-banner`}
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
        data-e2e={`${testIdPrefix}-tree-container`}
      >
        {rootPath ? (
          rootError ? (
            // FE-07: 根目录加载失败 → 错误占位（错误消息 + 重试按钮），不再伪装空目录
            <div
              data-testid={`${testIdPrefix}-load-error`}
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
                data-testid={`${testIdPrefix}-load-retry`}
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
              projectRootPath={projectRootPath ?? undefined}
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
          emptyState ?? (
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
          )
        )}
      </div>
    </>
  );
};
