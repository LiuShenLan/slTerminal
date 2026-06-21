// SidebarTree — 侧栏三级项目树组件
//
// L1: 项目 (Project) → L2: 工作树 (WorktreeInfo) → L3: 操作页面 (OperationPage，按 binding 过滤)
// 点击 L3 切换操作页面；右键菜单支持新建/删除工作树和页面。
// 工具栏提供"添加项目"按钮，通过文件夹对话框选择 git 仓库。

import React, { useState, useCallback, useRef, useEffect } from "react";
import { useProjects, createProjectId, createPageId } from "../../stores/projects";
import type { Project, OperationPage } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { git } from "../../ipc";
import { open } from "../../ipc/dialog";
import { CreateWorktreeDialog, DeleteWorktreeConfirm } from "../worktree";
import type { WorktreeInfo, WorktreeBinding } from "../../types/git";

// ---- CSS 变量（暗色主题） ----
const SIDEBAR_CSS = {
  "--sb-bg": "#1E1E1E",
  "--sb-fg": "#D4D4D4",
  "--sb-hover": "#2A2D2E",
  "--sb-selected": "#37373D",
} as React.CSSProperties;

const SIDEBAR_WIDTH = 250;

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
        background: "#252526",
        border: "1px solid #454545",
        borderRadius: 4,
        padding: "4px 0",
        minWidth: 160,
        zIndex: 1000,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
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
            color: "#D4D4D4",
            fontSize: 13,
            userSelect: "none",
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLDivElement).style.background = "#094771";
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
      borderBottom: "1px solid #333",
      height: 32,
    }}
  >
    <button
      onClick={onAddProject}
      title="添加项目"
      style={{
        background: "none",
        border: "1px solid #444",
        color: "#D4D4D4",
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
    <span style={{ width: 16, fontSize: 10, flexShrink: 0, color: "#808080" }}>
      {expanded ? "▼" : "▶"}
    </span>
    <span style={{ marginRight: 4, flexShrink: 0 }}>📁</span>
    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {project.name}
    </span>
  </div>
);

/** L2 工作树行 */
const WorktreeRow: React.FC<{
  worktree: WorktreeInfo;
  depth: number;
  expanded: boolean;
  onToggle: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}> = ({ worktree, depth, expanded, onToggle, onContextMenu }) => (
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
    <span style={{ width: 16, fontSize: 10, flexShrink: 0, color: "#808080" }}>
      {expanded ? "▼" : "▶"}
    </span>
    <span style={{ marginRight: 4, flexShrink: 0 }}>🌿</span>
    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {worktree.branch}
    </span>
    {worktree.isDetached && (
      <span style={{ marginLeft: 4, fontSize: 10, color: "#CE9178", flexShrink: 0 }}>
        detached
      </span>
    )}
    {worktree.isBare && (
      <span style={{ marginLeft: 4, fontSize: 10, color: "#569CD6", flexShrink: 0 }}>
        bare
      </span>
    )}
  </div>
);

/** L3 操作页面行 */
const PageRow: React.FC<{
  page: OperationPage;
  depth: number;
  selected: boolean;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}> = ({ page, depth, selected, onClick, onContextMenu }) => (
  <div
    onClick={onClick}
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
    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {page.name}
    </span>
  </div>
);

// ---- Props ----

interface SidebarTreeProps {
  /** 切换操作页面（由 Workspace 注入，持有 dockview API） */
  switchToPage: (projectId: string, pageId: string) => void;
}

// ---- 辅助 ----

/** 根据 binding 找页面所属 worktree path */
function pageWorktreePath(page: OperationPage): string | null {
  return page.binding?.worktreePath ?? null;
}

/** 生成新操作页面的默认布局（含一个终端面板） */
function makeDefaultLayout(panelId: string): Record<string, unknown> {
  return {
    grid: {
      root: {
        type: "leaf",
        data: { views: [panelId], activeView: panelId },
      },
    },
    panels: {
      [panelId]: {
        id: panelId,
        component: "terminal",
        params: { panelId },
        renderer: "always",
      },
    },
  };
}

// ---- 主组件 ----

const SidebarTree: React.FC<SidebarTreeProps> = ({ switchToPage }) => {
  const projects = useProjects((s) => s.projects);
  const expandedNodes = useProjects((s) => s.expandedNodes);
  const toggleExpand = useProjects((s) => s.toggleExpand);
  const addProject = useProjects((s) => s.addProject);
  const removeWorktree = useProjects((s) => s.removeWorktree);
  const addPage = useProjects((s) => s.addPage);
  const removePage = useProjects((s) => s.removePage);
  const globalActivePageId = useLayout((s) => s.activePageId);

  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  // H3: CreateWorktreeDialog 状态
  const [worktreeDialogProject, setWorktreeDialogProject] = useState<Project | null>(null);

  // N1: DeleteWorktreeConfirm 状态
  const [deleteTarget, setDeleteTarget] = useState<{
    projectId: string;
    worktree: WorktreeInfo;
  } | null>(null);

  // H3: 项目级新建 worktree
  const handleNewWorktree = useCallback(
    (project: Project) => {
      setWorktreeDialogProject(project);
    },
    [],
  );

  const handleWorktreeCreated = useCallback(
    (projectId: string, worktree: WorktreeInfo, defaultPage: OperationPage) => {
      addPage(projectId, defaultPage);
      useProjects.getState().addWorktree(projectId, worktree);
      setWorktreeDialogProject(null);
    },
    [addPage],
  );

  // "添加项目"按钮
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

      const isRepo = await git.isRepo(dirPath);
      if (!isRepo) {
        // B3 修复：非 git 目录给出用户可见提示
        alert(`"${dirPath}" 不是 git 仓库。\n\n目前仅支持 git 仓库项目——非 git 目录功能正在开发中。`);
        return;
      }

      const rootPath = await git.getRoot(dirPath);
      const worktrees = await git.listWorktrees(rootPath);
      const name = rootPath.split(/[/\\]/).pop() || rootPath;
      const projectId = createProjectId();

      // 为每个 worktree 创建默认操作页面，设置 binding
      const pages: OperationPage[] = worktrees.map((wt) => {
        const binding: WorktreeBinding = {
          worktreePath: wt.path,
          branchName: wt.branch,
        };
        const defaultPanelId = `terminal-${createPageId()}-0`;
        return {
          pageId: createPageId(),
          name: wt.isDetached
            ? `detached:${wt.head.slice(0, 7)}`
            : wt.branch,
          layout: makeDefaultLayout(defaultPanelId),
          binding,
          createdAt: Date.now(),
          lastAccessedAt: Date.now(),
        };
      });

      const project: Project = {
        projectId,
        name,
        rootPath,
        worktrees,
        pages,
        activePageId: pages.length > 0 ? pages[0].pageId : null,
        version: 1,
      };

      addProject(project);
    } catch (err) {
      console.error("[slTerminal] 添加项目失败:", err);
    }
  }, [addProject]);

  // 新建操作页面（绑定到指定 worktree）
  const handleNewPage = useCallback(
    (projectId: string, worktree: WorktreeInfo) => {
      const binding: WorktreeBinding = {
        worktreePath: worktree.path,
        branchName: worktree.branch,
      };
      const pageId = createPageId();
      const defaultPanelId = `terminal-${pageId}-0`;
      const page: OperationPage = {
        pageId,
        name: `页面-${Date.now() % 10000}`,
        layout: makeDefaultLayout(defaultPanelId),
        binding,
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      };
      addPage(projectId, page);
    },
    [addPage],
  );

  // 删除操作页面
  const handleDeletePage = useCallback(
    (projectId: string, pageId: string) => {
      removePage(projectId, pageId);
    },
    [removePage],
  );

  // S1: 重命名操作页面（使用 store.renamePage action）
  const renamePage = useProjects((s) => s.renamePage);

  const handleRenamePage = useCallback(
    (projectId: string, pageId: string) => {
      const newName = prompt("新页面名称:");
      if (!newName?.trim()) return;
      renamePage(projectId, pageId, newName.trim());
    },
    [renamePage],
  );

  // N1: 确认删除 worktree（完整流程：kill 终端 → 清理 store → 后端删除）
  const handleDeleteWorktreeConfirm = useCallback(async () => {
    if (!deleteTarget) return;
    const { projectId, worktree } = deleteTarget;
    const store = useProjects.getState();
    const project = store.projects[projectId];
    if (!project) return;

    try {
      // 1. 后端删除（三级回退）
      await git.removeWorktree(project.rootPath, worktree.path);
    } catch (err) {
      alert(`删除工作树失败: ${err}`);
      setDeleteTarget(null);
      return;
    }

    // 2. 删除绑定到此 worktree 的页面
    for (const page of project.pages) {
      if (page.binding?.worktreePath === worktree.path) {
        removePage(projectId, page.pageId);
      }
    }

    // 3. Store: 移除 worktree
    removeWorktree(projectId, worktree.path);

    // 4. 无剩余页面时创建默认空页面
    const updated = useProjects.getState().projects[projectId];
    if (updated && updated.pages.length === 0) {
      const fallbackPageId = createPageId();
      const fallbackPanelId = `terminal-${fallbackPageId}-0`;
      addPage(projectId, {
        pageId: fallbackPageId,
        name: "默认",
        layout: makeDefaultLayout(fallbackPanelId),
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      });
    }

    setDeleteTarget(null);
  }, [deleteTarget, removeWorktree, removePage, addPage]);

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
        width: SIDEBAR_WIDTH,
        minWidth: SIDEBAR_WIDTH,
        height: "100%",
        background: "var(--sb-bg)",
        borderRight: "1px solid #333",
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
              color: "#6C6C6C",
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
                        label: "新建工作树",
                        action: () => handleNewWorktree(project),
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
                project.worktrees.map((wt) => {
                  // 用 worktree path 作为展开 key
                  const wtNodeId = `${projId}:wt:${wt.path}`;
                  const wtExpanded = isExpanded(wtNodeId);

                  // 过滤出绑定到此 worktree 的页面
                  const wtPages = project.pages.filter(
                    (p) => pageWorktreePath(p) === wt.path,
                  );

                  // worktree 显示名
                  const displayBranch = wt.isDetached
                    ? `(detached:${wt.head.slice(0, 7)})`
                    : wt.branch;

                  return (
                    <div key={wtNodeId}>
                      <WorktreeRow
                        worktree={{ ...wt, branch: displayBranch }}
                        depth={1}
                        expanded={wtExpanded}
                        onToggle={() => toggleExpand(wtNodeId)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          const items: ContextMenuItem[] = [
                            {
                              label: "新建操作页面",
                              action: () => handleNewPage(projId, wt),
                            },
                          ];
                          // S4: 主 worktree 不显示删除选项
                          if (!wt.isMain) {
                            items.push({
                              label: "删除工作树",
                              action: () => setDeleteTarget({ projectId: projId, worktree: wt }),
                            });
                          }
                          setContextMenu({
                            visible: true,
                            x: e.clientX,
                            y: e.clientY,
                            items,
                          });
                        }}
                      />

                      {wtExpanded &&
                        wtPages.map((page) => {
                          const isSelected = page.pageId === globalActivePageId;
                          return (
                            <PageRow
                              key={page.pageId}
                              page={page}
                              depth={2}
                              selected={isSelected}
                              onClick={() => switchToPage(projId, page.pageId)}
                              onContextMenu={(e) => {
                                e.preventDefault();
                                setContextMenu({
                                  visible: true,
                                  x: e.clientX,
                                  y: e.clientY,
                                  items: [
                                    {
                                      label: "重命名操作页面",
                                      action: () =>
                                        handleRenamePage(projId, page.pageId),
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
          );
        })}
      </div>

      {/* 右键菜单 */}
      <ContextMenu state={contextMenu} onClose={closeContextMenu} />

      {/* H3: 项目级新建 worktree 对话框 */}
      {worktreeDialogProject && (
        <CreateWorktreeDialog
          project={worktreeDialogProject}
          onClose={() => setWorktreeDialogProject(null)}
          onCreated={handleWorktreeCreated}
        />
      )}

      {/* N1: 删除 worktree 确认对话框 */}
      {deleteTarget && (() => {
        const affectedPages = (useProjects.getState().projects[deleteTarget.projectId]?.pages ?? [])
          .filter(p => p.binding?.worktreePath === deleteTarget.worktree.path)
          .map(p => p.name);
        return (
          <DeleteWorktreeConfirm
            worktree={deleteTarget.worktree}
            affectedPages={affectedPages}
            onClose={() => setDeleteTarget(null)}
            onConfirm={handleDeleteWorktreeConfirm}
          />
        );
      })()}
    </div>
  );
};

export default SidebarTree;
