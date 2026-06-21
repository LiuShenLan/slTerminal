// projects — 项目/操作页三层数据模型存储
//
// 三层模型：Project → OperationPage → Panel（面板由 Dockview 管理，不在此 store）
// CAS 锁：deletionLock 用于两阶段删除（标记 → 确认）
// 持久化：persist 中间件暂用内存存储桶，loadFromDisk/saveToDisk 供启动/退出时调用

import { create } from "zustand";
import type { WorktreeInfo, WorktreeBinding } from "../types/git";
import * as fs from "../ipc/fs";

// ── 数据模型 ──────────────────────────────────────────────

export interface Project {
  projectId: string;
  name: string;
  rootPath: string;
  worktrees: WorktreeInfo[];
  pages: OperationPage[];
  activePageId: string | null;
  version: number;
}

export interface OperationPage {
  pageId: string;
  name: string;
  layout: Record<string, unknown>;
  binding?: WorktreeBinding;
  createdAt: number;
  lastAccessedAt: number;
}

interface DeletionLock {
  pendingDelete: string | null;
  acquiredAt: number | null;
}

// ── ID 生成 ────────────────────────────────────────────────

let idCounter = 0;
function nextId(prefix: string): string {
  idCounter++;
  return `${prefix}-${Date.now()}-${idCounter}`;
}

// ── Store ──────────────────────────────────────────────────

interface ProjectsState {
  projects: Record<string, Project>;
  deletionLock: DeletionLock;
  /** 树节点展开状态（nodeId → 是否展开） */
  expandedNodes: Record<string, boolean>;

  addProject: (project: Project) => void;
  removeProject: (projectId: string) => void;
  addWorktree: (projectId: string, worktree: WorktreeInfo) => void;
  removeWorktree: (projectId: string, worktreePath: string) => void;
  addPage: (projectId: string, page: OperationPage) => void;
  removePage: (projectId: string, pageId: string) => void;
  switchToPage: (projectId: string, pageId: string) => void;
  renamePage: (projectId: string, pageId: string, newName: string) => void;
  updatePageLayout: (
    projectId: string,
    pageId: string,
    layout: Record<string, unknown>,
  ) => void;
  toggleExpand: (nodeId: string) => void;

  /** 从磁盘加载项目数据（供启动时调用） */
  loadFromDisk: (filePath: string) => Promise<void>;
  /** 保存项目数据到磁盘（供退出/自动保存时调用） */
  saveToDisk: (filePath: string) => Promise<void>;
}

export const useProjects = create<ProjectsState>()((set, get) => ({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},

      // ── Project CRUD ─────────────────────────────────────

      addProject: (project) =>
        set((state) => ({
          projects: { ...state.projects, [project.projectId]: project },
          expandedNodes: { ...state.expandedNodes, [project.projectId]: true },
        })),

      removeProject: (projectId) =>
        set((state) => {
          const next = { ...state.projects };
          delete next[projectId];
          const nextExpanded = { ...state.expandedNodes };
          delete nextExpanded[projectId];
          return {
            projects: next,
            expandedNodes: nextExpanded,
            deletionLock: { pendingDelete: null, acquiredAt: null },
          };
        }),

      // ── Worktree ─────────────────────────────────────────

      addWorktree: (projectId, worktree) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          // 去重：已存在同路径 worktree 则不添加
          if (project.worktrees.some((w) => w.path === worktree.path))
            return state;
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                worktrees: [...project.worktrees, worktree],
                version: project.version + 1,
              },
            },
          };
        }),

      removeWorktree: (projectId, worktreePath) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                worktrees: project.worktrees.filter(
                  (w) => w.path !== worktreePath,
                ),
                version: project.version + 1,
              },
            },
          };
        }),

      // ── Page ─────────────────────────────────────────────

      addPage: (projectId, page) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          const pages = [...project.pages, page];
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                pages,
                // 首个页面自动激活
                activePageId: project.activePageId ?? page.pageId,
                version: project.version + 1,
              },
            },
            expandedNodes: { ...state.expandedNodes, [page.pageId]: true },
          };
        }),

      removePage: (projectId, pageId) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          const pages = project.pages.filter((p) => p.pageId !== pageId);
          let nextActive = project.activePageId;
          if (project.activePageId === pageId) {
            nextActive = pages.length > 0 ? pages[0].pageId : null;
          }
          const nextExpanded = { ...state.expandedNodes };
          delete nextExpanded[pageId];
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                pages,
                activePageId: nextActive,
                version: project.version + 1,
              },
            },
            expandedNodes: nextExpanded,
          };
        }),

      switchToPage: (projectId, pageId) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                activePageId: pageId,
                pages: project.pages.map((p) =>
                  p.pageId === pageId
                    ? { ...p, lastAccessedAt: Date.now() }
                    : p,
                ),
                version: project.version + 1,
              },
            },
          };
        }),

      renamePage: (projectId, pageId, newName) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                pages: project.pages.map((p) =>
                  p.pageId === pageId ? { ...p, name: newName } : p,
                ),
                version: project.version + 1,
              },
            },
          };
        }),

      updatePageLayout: (projectId, pageId, layout) =>
        set((state) => {
          const project = state.projects[projectId];
          if (!project) return state;
          return {
            projects: {
              ...state.projects,
              [projectId]: {
                ...project,
                pages: project.pages.map((p) =>
                  p.pageId === pageId ? { ...p, layout } : p,
                ),
                version: project.version + 1,
              },
            },
          };
        }),

      toggleExpand: (nodeId) =>
        set((state) => ({
          expandedNodes: {
            ...state.expandedNodes,
            [nodeId]: !state.expandedNodes[nodeId],
          },
        })),

      // ── 磁盘持久化 ──────────────────────────────────────

      loadFromDisk: async (filePath: string) => {
        try {
          const raw = await fs.readFile(filePath);
          const data: {
            projects?: Record<string, Project>;
            deletionLock?: DeletionLock;
            expandedNodes?: Record<string, boolean>;
          } = JSON.parse(raw);
          set({
            projects: data.projects ?? {},
            deletionLock: data.deletionLock ?? {
              pendingDelete: null,
              acquiredAt: null,
            },
            expandedNodes: data.expandedNodes ?? {},
          });
        } catch {
          // 首次启动或文件损坏，保持默认状态
        }
      },

      saveToDisk: async (filePath: string) => {
        const { projects, deletionLock, expandedNodes } = get();
        await fs.writeFile(
          filePath,
          JSON.stringify({ projects, deletionLock, expandedNodes }, null, 2),
        );
      },
  }));

// ── ID 工具函数（供外部创建节点时生成 ID） ──

export function createProjectId(): string {
  return nextId("proj");
}

export function createPageId(): string {
  return nextId("page");
}
