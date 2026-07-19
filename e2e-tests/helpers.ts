/**
 * E2E 测试辅助——所有 E2E helper 在 DEV 模式下统一挂载。
 * 生产构建中此文件完全不被打包（main.tsx 通过动态 import 加载，
 * Vite 在 production 模式下 tree-shake 掉 import.meta.env.DEV 分支）。
 *
 * 命名约定：
 * - __slterm_e2e_* — window 全局 helper（测试脚本直接调用）
 * - __e2e_*       — 终端容器 DOM 元素 helper（随面板挂载/卸载）
 */

import { writeText } from "../src/ipc/clipboard";
import { setProjectRoot } from "../src/ipc/fs";
import { getShortcutRegistry } from "../src/features/shortcuts";
import { useProjects, createProjectId, createPageId } from "../src/stores/projects";
import type { OperationPage, Project } from "../src/stores/projects";
import { useLayout } from "../src/stores/layout";
import { makeEmptyLayout } from "../src/features/sidebar/SidebarTree";
import { titleManager } from "../src/workspace/titleManager";
import { useSideBar } from "../src/stores/sideBar";

// ── Window 全局类型扩展 ──

declare global {
  interface Window {
    // 工作区（Workspace 设置）
    __slterm_e2e_workspaceReady?: boolean;
    // 剪贴板
    __slterm_e2e_writeClipboard?: (text: string) => Promise<void>;
    // 诊断
    __slterm_e2e_shortcutDebug?: () => { stack: string[]; commands: string[] };
    // 项目管理
    __slterm_e2e_createProject?: (dirPath: string) => Promise<string>;
    __slterm_e2e_getProjectIdForPage?: (pageId: string) => string | null;
    __slterm_e2e_addPage?: (projectId: string, name: string, rootPath: string) => string;
    __slterm_e2e_switchToPage?: (pageId: string) => Promise<void>;
    // 页签标题
    __slterm_e2e_registerAndRecompute?: (
      pageId: string, rootPath: string, panelId: string, filePath?: string
    ) => void;
    __slterm_e2e_getActivePageInfo?: () => { pageId: string; rootPath: string } | null;
    // 初始化竞态协调
    __slterm_e2e_projectPending?: boolean;
    // 侧栏视图（SB-25）
    __slterm_e2e_getSideBarState?: () => SideBarSnapshot | null;
    __slterm_e2e_toggleSideView?: (id: string) => void;
    __slterm_e2e_moveSideViewButton?: (id: string, zone: string, index: number) => void;
  }
}

/** useSideBar.getState() 的纯数据快照（去函数键，供 browser.execute 序列化） */
interface SideBarSnapshot {
  zones: { top: string[]; bottom: string[] };
  open: { top: string | null; bottom: string | null };
  width: number;
  splitRatio: number;
  loaded: boolean;
}

// ── installAllE2eHelpers —— 主入口 ──

/** 在 window 上安装全部 E2E 辅助函数（不含终端容器级 helper） */
export function installAllE2eHelpers(): void {
  installClipboard();
  installShortcutDebug();
  installProjectHelpers();
  installTitleHelpers();
  installSideBarHelpers();

  // 标记 Workspace 就绪（Workspace 组件渲染时同步设置）
  window.__slterm_e2e_workspaceReady = false;
}

/** Workspace 就绪标记——由 Workspace 组件渲染阶段调用 */
export function markWorkspaceReady(): void {
  window.__slterm_e2e_workspaceReady = true;
}

// ── 终端容器级 helper ──

/** useTerminalInstance 调用的终端 E2E 上下文 */
export interface TerminalE2eContext {
  writeToTerminal: (text: string) => void;
  getTerminalText: () => string;
}

/**
 * 在终端容器 DOM 上安装 __e2e_writeToTerminal / __e2e_getTerminalText。
 * 由 useTerminalInstance 在 DEV 模式下调用。
 */
export function initTerminalE2e(container: HTMLElement, ctx: TerminalE2eContext): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const h = container as any;
  h.__e2e_writeToTerminal = ctx.writeToTerminal;
  h.__e2e_getTerminalText = ctx.getTerminalText;
}

/**
 * 在终端容器上安装 __e2e_writeToPty。
 * 由 useXterm 在 DEV 模式下调用。
 */
export function installTerminalWriteToPty(
  container: HTMLElement,
  writeFn: (data: string) => void,
): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_writeToPty = writeFn;
}

/**
 * 设置终端 session 就绪标记 __e2e_sessionReady 和 __e2e_error。
 * 由 useXterm 在 DEV 模式下调用。
 */
export function setTerminalSessionReady(container: HTMLElement, ready: boolean): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_sessionReady = ready;
}

/** 设置终端 spawn 错误信息 */
export function setTerminalSessionError(container: HTMLElement, error: string): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_error = error;
}

// ── 私有实现 ──

/** __slterm_e2e_writeClipboard */
function installClipboard(): void {
  window.__slterm_e2e_writeClipboard = writeText;
}

/** __slterm_e2e_shortcutDebug */
function installShortcutDebug(): void {
  window.__slterm_e2e_shortcutDebug = () => {
    const r = getShortcutRegistry();
    return { stack: r._contextStack(), commands: r.listCommands().map((c) => c.id) };
  };
}

/** __slterm_e2e_createProject / __slterm_e2e_addPage / __slterm_e2e_switchToPage / __slterm_e2e_getProjectIdForPage */
function installProjectHelpers(): void {
  // __slterm_e2e_createProject —— 程序化创建测试项目（绕过原生对话框）
  window.__slterm_e2e_createProject = async (dirPath: string) => {
    window.__slterm_e2e_projectPending = true; // 阻止 localStorage 恢复覆盖
    const name = dirPath.split(/[/\\]/).pop() || dirPath;
    const projectId = createProjectId();
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

    useProjects.getState().addProject(project);
    // DBG-8: setProjectRoot 必须在 setActivePage 之前（路径沙箱前置条件）
    try {
      await setProjectRoot(dirPath);
    } catch (err) {
      console.error("[slTerminal e2e] 设置项目根路径失败:", err);
    }
    useLayout.getState().setActivePage(pageId);
    return pageId;
  };

  // __slterm_e2e_getProjectIdForPage
  window.__slterm_e2e_getProjectIdForPage = (pageId: string) => {
    const { projects } = useProjects.getState();
    for (const [projId, proj] of Object.entries(projects)) {
      if (proj.pages.some((p) => p.pageId === pageId)) {
        return projId;
      }
    }
    return null;
  };

  // __slterm_e2e_addPage —— 在已有项目中新增操作页面（H6 跨页面存活测试）
  window.__slterm_e2e_addPage = (projectId: string, name: string, rootPath: string) => {
    const pageId = createPageId();
    const page: OperationPage = {
      pageId,
      name,
      layout: makeEmptyLayout(),
      cwd: rootPath,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };
    useProjects.getState().addPage(projectId, page);
    return pageId;
  };

  // __slterm_e2e_switchToPage —— 切换活跃页面（H6 跨页面切换验证）
  window.__slterm_e2e_switchToPage = async (pageId: string) => {
    // DBG-8: setActivePage 前先同步项目根路径到后端（路径沙箱前置条件）
    const { projects } = useProjects.getState();
    for (const [, proj] of Object.entries(projects)) {
      if (proj.pages.some((p) => p.pageId === pageId)) {
        if (proj.rootPath) {
          try {
            await setProjectRoot(proj.rootPath);
          } catch (err) {
            console.error("[slTerminal e2e] 切换页面—设置项目根路径失败:", err);
          }
        }
        break;
      }
    }
    useLayout.getState().setActivePage(pageId);
  };
}

/** __slterm_e2e_registerAndRecompute / __slterm_e2e_getActivePageInfo */
function installTitleHelpers(): void {
  // __slterm_e2e_registerAndRecompute —— 注册编辑器并重算标题
  window.__slterm_e2e_registerAndRecompute = (
    pageId: string,
    rootPath: string,
    panelId: string,
    filePath?: string,
  ) => {
    titleManager.registerEditor(pageId, panelId, filePath);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const api = window.__dockviewApi as any;
    if (api && rootPath) {
      const updates = titleManager.recomputeTitles(pageId, rootPath);
      for (const { panelId: pid, title } of updates) {
        const p = api.getPanel(pid);
        if (p) p.api.setTitle(title);
      }
    }
  };

  // __slterm_e2e_getActivePageInfo —— 获取活跃页面信息
  window.__slterm_e2e_getActivePageInfo = () => {
    const state = useProjects.getState();
    const activeId = useLayout.getState().activePageId;
    if (!activeId) return null;
    for (const [, proj] of Object.entries(state.projects)) {
      for (const page of proj.pages) {
        if (page.pageId === activeId) {
          return {
            pageId: page.pageId,
            rootPath: proj.rootPath,
          };
        }
      }
    }
    return null;
  };
}

/** __slterm_e2e_getSideBarState / __slterm_e2e_toggleSideView / __slterm_e2e_moveSideViewButton */
function installSideBarHelpers(): void {
  // __slterm_e2e_getSideBarState —— 返回 useSideBar 纯数据快照（去函数键，可安全经 browser.execute 序列化）
  window.__slterm_e2e_getSideBarState = () => {
    const state = useSideBar.getState();
    return {
      zones: { top: [...state.zones.top], bottom: [...state.zones.bottom] },
      open: { top: state.open.top, bottom: state.open.bottom },
      width: state.width,
      splitRatio: state.splitRatio,
      loaded: state.loaded,
    };
  };

  // __slterm_e2e_toggleSideView —— 等价点击活动栏按钮，走 store.toggleView（委托 toggleViewPure）
  window.__slterm_e2e_toggleSideView = (id: string) => {
    useSideBar.getState().toggleView(id);
  };

  // __slterm_e2e_moveSideViewButton —— 等价拖拽落点，走 store.moveButton（委托 moveButtonPure）
  // zone 类型为 "top" | "bottom"，调用方保证传入合法值
  window.__slterm_e2e_moveSideViewButton = (id: string, zone: string, index: number) => {
    useSideBar.getState().moveButton(id, zone as "top" | "bottom", index);
  };
}
