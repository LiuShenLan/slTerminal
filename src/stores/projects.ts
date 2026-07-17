// projects — 项目/操作页面数据模型存储
//
// 二级模型：Project → OperationPage（面板由 Dockview 管理，不在此 store）
// CAS 锁：deletionLock 用于两阶段删除（标记 → 确认）
// 持久化：Zustand subscribe + 2s debounce 变更即保存

import { create } from "zustand";
import * as fs from "../ipc/fs";
import { setProjectRoot } from "../ipc/fs";

/** 持久化 debounce 间隔（毫秒），供 fontSize/keybindings 等 store 共用 */
export const PERSIST_DEBOUNCE_MS = 2000;

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
          // SEC-01: 通知后端当前项目根路径（路径沙箱边界）
          if (project.rootPath) {
            setProjectRoot(project.rootPath).catch((err) =>
              console.error("[slTerminal] 设置项目根路径失败:", err),
            );
          }
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
        // P2-06: JSON.stringify 当前数据量小（数个 Project / 十数个 Page），
        // 全量序列化开销可忽略。若未来项目数量增长到百级，可考虑增量保存
        // （仅序列化变更项目）或去掉 pretty-print (null, 2) 减少 IO 体积。
        //
        // P2-12 应急恢复：当前直接覆盖写，若写入中途崩溃/磁盘满则文件损坏。
        // 应急方案：改为原子写入模式（先写 .tmp 再 rename，类 POSIX 原子操作）。
        //   1. await fs.writeFile(filePath + ".tmp", data)
        //   2. await fs.rename(filePath + ".tmp", filePath)
        // 启动时 loadFromDisk 增加恢复逻辑：
        //   - 若主文件不存在但 .tmp 存在，尝试从 .tmp 恢复
        //   - 若两者都存在，对比 mtime，取更新的
        await fs.writeFile(
          filePath,
          JSON.stringify({ projects, deletionLock, expandedNodes }, null, 2),
        );
      },
  }));

// ── 持久化连线（H6 修复） ──

const PERSIST_PATH = "slterminal-projects.json";

/** 启动加载：从磁盘恢复项目数据 */
export async function loadAllProjects(): Promise<void> {
  try {
    await useProjects.getState().loadFromDisk(PERSIST_PATH);
  } catch {
    // 首次启动或文件损坏，保持默认空状态
  }
}

/** 保存全部项目数据到磁盘 */
export async function saveAllProjects(): Promise<void> {
  try {
    await useProjects.getState().saveToDisk(PERSIST_PATH);
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
