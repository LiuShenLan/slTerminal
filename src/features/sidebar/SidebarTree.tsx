// SidebarTree — 侧栏二级项目树组件
//
// L1: 项目 (Project) → L2: 操作页面 (OperationPage)
// 点击 L2 切换操作页面；右键菜单支持新建/删除/重命名操作页面。
// 工具栏提供"+添加项目"按钮，通过文件夹对话框选择任意文件夹（不限制 git 仓库）。

import React, { useState, useCallback, useRef, useEffect } from "react";
import { useProjects, createProjectId, createPageId } from "../../stores/projects";
import type { Project, OperationPage } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { open } from "../../ipc/dialog";
import {
  PANEL_BG,
  SIDEBAR_COLORS,
  ACTIVE_SELECTION_BG,
  INPUT_BG,
  FOCUS_BORDER,
  PLACEHOLDER_FG,
  INPUT_BORDER,
} from "../../theme";

// ---- CSS 变量（暗色主题） ----
const SIDEBAR_CSS = {
  "--sb-bg": PANEL_BG,
  "--sb-fg": SIDEBAR_COLORS.fg,
  "--sb-hover": SIDEBAR_COLORS.hover,
  "--sb-selected": SIDEBAR_COLORS.selected,
} as React.CSSProperties;
// ---- 右键菜单类型 ----

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuItem {
  label: string;
  action: () => void;
}

// ---- 右键菜单组件 ----

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
        background: SIDEBAR_COLORS.bg,
        border: `1px solid ${SIDEBAR_COLORS.contextMenuBorder}`,
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
            color: SIDEBAR_COLORS.fg,
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

// ---- 工具栏 ----

const Toolbar: React.FC<{ onAddProject: () => void }> = ({ onAddProject }) => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      padding: "4px 8px",
      borderBottom: `1px solid ${SIDEBAR_COLORS.border}`,
      height: 32,
    }}
  >
    <button
      onClick={onAddProject}
      title="添加项目"
      style={{
        background: "none",
        border: `1px solid ${SIDEBAR_COLORS.border}`,
        color: SIDEBAR_COLORS.fg,
        cursor: "pointer",
        fontSize: 12,
        padding: "2px 8px",
        borderRadius: 3,
        width: "100%",
      }}
    >
      + 添加项目
    </button>
  </div>
);

// ---- 树节点子组件 ----

/** L1 项目行 */
const ProjectRow: React.FC<{
  project: Project;
  depth: number;
  expanded: boolean;
  onToggle: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}> = ({ project, depth, expanded, onToggle, onContextMenu }) => (
  <div
    onClick={onToggle}
    onContextMenu={onContextMenu}
    style={{
      display: "flex",
      alignItems: "center",
      padding: "2px 8px",
      paddingLeft: 8 + depth * 16,
      cursor: "pointer",
      userSelect: "none",
      fontSize: 13,
      color: "var(--sb-fg)",
      height: 26,
    }}
    onMouseEnter={(e) => {
      (e.target as HTMLDivElement).style.background = "var(--sb-hover)";
    }}
    onMouseLeave={(e) => {
      (e.target as HTMLDivElement).style.background = "transparent";
    }}
  >
    <span style={{ width: 16, fontSize: 10, flexShrink: 0, color: PLACEHOLDER_FG }}>
      {expanded ? "▼" : "▶"}
    </span>
    <span style={{ marginRight: 4, flexShrink: 0 }}>📁</span>
    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {project.name}
    </span>
  </div>
);

/** L2 操作页面行（支持内联重命名） */
const PageRow: React.FC<{
  page: OperationPage;
  depth: number;
  selected: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  isRenaming: boolean;
  onRename: (newName: string) => void;
  onCancelRename: () => void;
}> = ({
  page,
  depth,
  selected,
  onClick,
  onContextMenu,
  isRenaming,
  onRename,
  onCancelRename,
}) => {
  const [editValue, setEditValue] = useState("");

  // 父组件触发重命名 → 进入编辑模式
  useEffect(() => {
    if (isRenaming) {
      setEditValue(page.name);
    }
  }, [isRenaming, page.name]);

  const confirmRename = useCallback(() => {
    const trimmed = editValue.trim();
    if (trimmed && trimmed !== page.name) {
      onRename(trimmed);
    } else {
      onCancelRename();
    }
  }, [editValue, page.name, onRename, onCancelRename]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        confirmRename();
      } else if (e.key === "Escape") {
        onCancelRename();
      }
    },
    [confirmRename, onCancelRename],
  );

  return (
    <div
      onClick={isRenaming ? undefined : onClick}
      onContextMenu={onContextMenu}
      style={{
        display: "flex",
        alignItems: "center",
        padding: "2px 8px",
        paddingLeft: 8 + depth * 16,
        cursor: "pointer",
        userSelect: "none",
        fontSize: 13,
        color: "var(--sb-fg)",
        background: selected ? "var(--sb-selected)" : "transparent",
        height: 26,
      }}
      onMouseEnter={(e) => {
        if (!selected)
          (e.target as HTMLDivElement).style.background = "var(--sb-hover)";
      }}
      onMouseLeave={(e) => {
        if (!selected)
          (e.target as HTMLDivElement).style.background = "transparent";
      }}
    >
      <span style={{ width: 16, flexShrink: 0 }} />
      <span style={{ marginRight: 4, flexShrink: 0 }}>📄</span>
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
            background: INPUT_BG,
            border: `1px solid ${FOCUS_BORDER}`,
            color: SIDEBAR_COLORS.fg,
            fontSize: 13,
            padding: "0 4px",
            outline: "none",
            borderRadius: 2,
            minWidth: 0,
          }}
        />
      ) : (
        <span
          style={{
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {page.name}
        </span>
      )}
    </div>
  );
};

// ---- Props ----

interface SidebarTreeProps {
  /** 切换操作页面（由 Workspace 注入，持有 dockview API） */
  switchToPage: (projectId: string, pageId: string) => void;
  /** 删除操作页面（由 Workspace 层编排，区分当前/非当前页面） */
  onDeletePage: (projectId: string, pageId: string) => void;
}

// ---- 辅助 ----

/** 生成新操作页面的空白布局（不含任何默认面板）。
 *
 * 新页面显示 Watermark 组件（"打开终端或编辑器开始工作"），
 * 用户通过 Watermark 按钮或页签 "+" 按钮手动创建终端。 */
export function makeEmptyLayout(): Record<string, unknown> {
  return {};
}

// ---- 主组件 ----

const SidebarTree: React.FC<SidebarTreeProps> = ({ switchToPage, onDeletePage }) => {
  const projects = useProjects((s) => s.projects);
  const expandedNodes = useProjects((s) => s.expandedNodes);
  const toggleExpand = useProjects((s) => s.toggleExpand);
  const addProject = useProjects((s) => s.addProject);
  const addPage = useProjects((s) => s.addPage);
  const renamePage = useProjects((s) => s.renamePage);
  const globalActivePageId = useLayout((s) => s.activePageId);

  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  /** 当前正在内联重命名的页面 ID（null = 无） */
  const [renamingPageId, setRenamingPageId] = useState<string | null>(null);

  // "添加项目"按钮 — 选择任意文件夹，无需 git 检查
  const handleAddProject = useCallback(async () => {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "选择项目文件夹",
      });
      if (!result) return;
      const dirPath = Array.isArray(result) ? result[0] : result;
      if (!dirPath) return;

      const name = dirPath.split(/[/\\]/).pop() || dirPath;
      const projectId = createProjectId();

      // 创建默认操作页面（cwd = 所选目录路径）
      const pageId = createPageId();
      const page: OperationPage = {
        pageId,
        name,
        layout: makeEmptyLayout(),
        cwd: dirPath,
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      };

      const project: Project = {
        projectId,
        name,
        rootPath: dirPath,
        pages: [page],
        activePageId: pageId,
        version: 1,
      };

      addProject(project);
    } catch (err) {
      console.error("[slTerminal] 添加项目失败:", err);
    }
  }, [addProject]);

  // 新建操作页面（绑定到项目根目录）
  const handleNewPage = useCallback(
    (projectId: string, cwd: string) => {
      const pageId = createPageId();
      const page: OperationPage = {
        pageId,
        name: `页面-${Date.now() % 10000}`,
        layout: makeEmptyLayout(),
        cwd,
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      };
      addPage(projectId, page);
    },
    [addPage],
  );

  // 删除操作页面（委托 Workspace 层编排：清空 Dockview + store 移除 + 页面切换）
  const handleDeletePage = useCallback(
    (projectId: string, pageId: string) => {
      onDeletePage(projectId, pageId);
    },
    [onDeletePage],
  );

  const closeContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  const isExpanded = useCallback(
    (nodeId: string) => expandedNodes[nodeId] === true,
    [expandedNodes],
  );

  const projectList = Object.values(projects);

  return (
    <div
      style={{
        ...SIDEBAR_CSS,
        width: "100%",
        minWidth: 0,
        height: "100%",
        background: "var(--sb-bg)",
        borderRight: `1px solid ${SIDEBAR_COLORS.border}`,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      <Toolbar onAddProject={handleAddProject} />

      {/* 树区域 */}
      <div style={{ flex: 1, overflowY: "auto", overflowX: "hidden" }}>
        {projectList.length === 0 && (
          <div
            style={{
              padding: 16,
              color: INPUT_BORDER,
              fontSize: 12,
              textAlign: "center",
            }}
          >
            暂无项目，点击 "+ 添加项目" 开始
          </div>
        )}

        {projectList.map((project) => {
          const projId = project.projectId;
          const projExpanded = isExpanded(projId);

          return (
            <div key={projId}>
              <ProjectRow
                project={project}
                depth={0}
                expanded={projExpanded}
                onToggle={() => toggleExpand(projId)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setContextMenu({
                    visible: true,
                    x: e.clientX,
                    y: e.clientY,
                    items: [
                      {
                        label: "新建操作页面",
                        action: () => handleNewPage(projId, project.rootPath),
                      },
                      {
                        label: "删除项目",
                        action: () => {
                          if (window.confirm(`确定删除项目 "${project.name}"？`)) {
                            useProjects.getState().removeProject(projId);
                          }
                        },
                      },
                    ],
                  });
                }}
              />

              {projExpanded &&
                project.pages.map((page) => {
                  const isSelected = page.pageId === globalActivePageId;
                  return (
                    <PageRow
                      key={page.pageId}
                      page={page}
                      depth={1}
                      selected={isSelected}
                      onClick={() => switchToPage(projId, page.pageId)}
                      isRenaming={renamingPageId === page.pageId}
                      onRename={(newName) => {
                        renamePage(projId, page.pageId, newName);
                        setRenamingPageId(null);
                      }}
                      onCancelRename={() => setRenamingPageId(null)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setContextMenu({
                          visible: true,
                          x: e.clientX,
                          y: e.clientY,
                          items: [
                            {
                              label: "重命名操作页面",
                              action: () => setRenamingPageId(page.pageId),
                            },
                            {
                              label: "删除操作页面",
                              action: () =>
                                handleDeletePage(projId, page.pageId),
                            },
                          ],
                        });
                      }}
                    />
                  );
                })}
            </div>
          );
        })}
      </div>

      {/* 右键菜单 */}
      <ContextMenu state={contextMenu} onClose={closeContextMenu} />
    </div>
  );
};

export default SidebarTree;
