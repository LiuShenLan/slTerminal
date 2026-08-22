// projects — 项目/操作页面数据模型存储
//
// 二级模型：Project → OperationPage（面板由 Dockview 管理，不在此 store）
// CAS 锁：deletionLock 用于两阶段删除（标记 → 确认）
// 持久化：Zustand subscribe + 2s debounce 变更即保存

import { create } from "zustand";
import * as projectsIpc from "../ipc/projects";
import { toast } from "../lib";

/** 持久化 debounce 间隔（毫秒），供 fontSize/keybindings 等 store 共用 */
export const PERSIST_DEBOUNCE_MS = 2000;

/** 页面总数上限（FE-01/D1 契约）——多 Dockview 实例架构每页一实例，上限防内存/DOM 线性增长 */
export const MAX_PAGES = 20;

// ── 数据模型 ──────────────────────────────────────────────

export interface Project {
  projectId: string;
  name: string;
  rootPath: string;
  pages: OperationPage[];
  activePageId: string | null;
  version: number;
}

export interface OperationPage {
  pageId: string;
  name: string;
  layout: Record<string, unknown>;
  /** 终端工作目录（项目根路径） */
  cwd?: string;
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

  /** 从磁盘加载项目数据（供启动时调用，路径由 Rust 端解析） */
  loadFromDisk: () => Promise<void>;
  /** 保存项目数据到磁盘（供退出/自动保存时调用，路径由 Rust 端解析） */
  saveToDisk: () => Promise<void>;
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

      // ── Page ─────────────────────────────────────────────

      addPage: (projectId, page) => {
        // FE-01（D1 契约）：页面总数上限 MAX_PAGES——超限拒绝新增 + toast 告警。
        // 多 Dockview 实例架构每页一实例，上限防内存/DOM 线性增长（豁免登记 S19）；
        // FE-36 全局化：上限按跨项目全局页面总数计数（原按项目计数）
        const project = get().projects[projectId];
        if (!project) return;
        // FE-36（D1 契约名实相符）：页面总数上限 = 跨项目全局计数
        // （原按项目计数——多项目下 Dockview 实例仍可无界增长）
        const totalPages = Object.values(get().projects).flatMap((p) => p.pages).length;
        if (totalPages >= MAX_PAGES) {
          toast.show("warning", "页面数已达上限");
          return;
        }
        set((state) => {
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
        });
      },

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
        // FE-37（D18）：纯状态转换——setProjectRoot 已上提调用方
        // switchToPageShared（src/workspace/pageApis.ts），store 不触 IPC（硬约束 #12）
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

      loadFromDisk: async () => {
        try {
          const { data: raw, corrupted } = await projectsIpc.loadProjects();
          // FE-11：损坏时后端已回退默认数据（data 为默认形态），toast 告警
          if (corrupted) {
            toast.show("warning", "配置已损坏，已回退默认值");
          }
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
        } catch (err) {
          // 首次启动或 IPC 失败，保持默认状态
          console.warn("[slTerminal] projects loadFromDisk 失败:", err);
        }
      },

      saveToDisk: async () => {
        const { projects, deletionLock, expandedNodes } = get();
        await projectsIpc.saveProjects(
          JSON.stringify({ projects, deletionLock, expandedNodes }, null, 2),
        );
      },
  }));

// ── 持久化连线（H6 修复） ──
// 项目数据文件路径由 Rust 端解析为 exe 同级目录（便携分发），
// 详见 src-tauri/src/projects.rs

/** 启动加载：从磁盘恢复项目数据 */
export async function loadAllProjects(): Promise<void> {
  try {
    await useProjects.getState().loadFromDisk();
  } catch (err) {
    // 首次启动或文件损坏，保持默认空状态
    console.warn("[slTerminal] loadAllProjects loadFromDisk 失败:", err);
  }
}

/** 保存全部项目数据到磁盘 */
export async function saveAllProjects(): Promise<void> {
  try {
    await useProjects.getState().saveToDisk();
  } catch (err) {
    console.error("[slTerminal] 保存项目数据失败:", err);
  }
}

// 变更即保存（2s debounce）—— 唯一抵抗 taskkill/关机的手段
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let initialized = false;

// 标记初始化完成（loadFromDisk 调用后），避免首次加载触发保存
export function markPersistenceReady(): void {
  initialized = true;
}

/** 取消待执行的 debounced 保存（关闭钩子中避免竞态） */
export function cancelPendingSave(): void {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}

/** 仅测试用：重置持久化状态（清 timer + 重置 initialized 标记） */
export function _resetPersistence(): void {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  initialized = false;
}

useProjects.subscribe(() => {
  if (!initialized) return;
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveAllProjects();
  }, PERSIST_DEBOUNCE_MS);
});

// ── ID 工具函数（供外部创建节点时生成 ID） ──

export function createProjectId(): string {
  return nextId("proj");
}

export function createPageId(): string {
  return nextId("page");
}
