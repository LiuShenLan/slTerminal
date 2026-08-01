// pageApis — 页面 API 注册表 + 共享页面切换
//
// 模块级 Map<pageId, DockviewApi>，管理每个页面的 DockviewApi 实例。
// 提供 register/unregister/get 操作，以及共享切换函数 switchToPageShared /
// switchToPageAndFocus / openHooksConfigPanel。
//
// 不变量：window.__dockviewApi 重指向只允许出现在三站点——
//   switchToPageShared（本文件）、Workspace.onDeletePage、Workspace.handlePageApiReady
//
// 契约 C1：详见 docs/hooks-dev/phase2-fix/stages.md

import type { DockviewApi } from "dockview-react";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { setProjectRoot } from "../ipc/fs";

/** 模块级页面 API 注册表 */
const pageApiMap = new Map<string, DockviewApi>();

/** 注册页面 DockviewApi */
export function registerPageApi(pageId: string, api: DockviewApi): void {
  pageApiMap.set(pageId, api);
}

/** 注销页面 DockviewApi */
export function unregisterPageApi(pageId: string): void {
  pageApiMap.delete(pageId);
}

/** 获取页面 DockviewApi */
export function getPageApi(pageId: string): DockviewApi | undefined {
  return pageApiMap.get(pageId);
}

/**
 * 切换活跃页面——setProjectRoot 前置 await → setActivePage → 重指向 __dockviewApi。
 *
 * - activePageId 已为目标 pageId 时直接返回（幂等）
 * - 经 useProjects.getState() 查 pageId 所属项目 rootPath，await setProjectRoot（失败 console.error 降级继续）
 * - useLayout.getState().setActivePage(pageId)
 * - getPageApi(pageId) 命中 → window.__dockviewApi = api
 *   （未初始化页面由 Workspace.handlePageApiReady 兜底重指向）
 */
export async function switchToPageShared(pageId: string): Promise<void> {
  const layoutStore = useLayout.getState();
  if (layoutStore.activePageId === pageId) return;

  // 查找 pageId 所属项目 rootPath 并同步到后端（路径沙箱前置条件）
  const { projects: currentProjects } = useProjects.getState();
  for (const [, proj] of Object.entries(currentProjects)) {
    if (proj.pages.some((p) => p.pageId === pageId)) {
      if (proj.rootPath) {
        try {
          await setProjectRoot(proj.rootPath);
        } catch (err) {
          console.error("[slTerminal] 设置项目根路径失败:", err);
        }
      }
      break;
    }
  }

  layoutStore.setActivePage(pageId);

  // 已初始化页面的 DockviewApi 立即重指向（未初始化页面由 handlePageApiReady 兜底）
  const api = getPageApi(pageId);
  if (api) window.__dockviewApi = api;
}

/**
 * 切换页面并聚焦面板——await switchToPageShared → 有限轮询面板挂载 → focus()。
 *
 * 轮询 getPageApi(pageId)?.getPanel(panelId)，100ms×50=5s 上限。
 * 超时后 console.warn 降级（不抛异常）。
 */
export async function switchToPageAndFocus(
  pageId: string,
  panelId: string,
): Promise<void> {
  await switchToPageShared(pageId);

  // 轮询面板挂载——页面可能尚未初始化，等 handlePageApiReady 注册 api 后再查
  for (let i = 0; i < 50; i++) {
    const panel = getPageApi(pageId)?.getPanel(panelId);
    if (panel) {
      panel.focus();
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  console.warn(
    `[slTerminal] 面板 ${panelId} 在 5s 内未就绪，无法聚焦`,
  );
}

/**
 * 打开 hooks 配置面板（同页单例，契约 C13-7）——调用方须先切到目标页（本函数不切页）。
 *
 * 面板只能在活跃页面打开（useHooksConfig 经 activePageId 推导 rootPath），
 * 故调用方须先 switchToPage 再调用本函数（见 SidebarTree「打开 Hooks 配置」菜单 action）。
 *
 * 轮询 getPageApi(pageId) 就绪——首次挂载页面的 Dockview API 在 React commit 后
 * 经 Workspace.handlePageApiReady 异步注册，100ms×50=5s 上限（照 switchToPageAndFocus）。
 * 就绪后 getPanel 查重（同页单例：命中 focus、未命中 addPanel）。
 * 超时 console.warn 降级（不抛异常）。
 * @returns 面板打开成功与否（超时返回 false）
 */
export async function openHooksConfigPanel(pageId: string): Promise<boolean> {
  const panelId = `hooksConfig-${pageId}`;
  for (let i = 0; i < 50; i++) {
    const api = getPageApi(pageId);
    if (api) {
      const existing = api.getPanel(panelId);
      if (existing) {
        existing.focus();
        return true;
      }
      api.addPanel({
        id: panelId,
        component: "hooksConfig",
        title: "Hooks 配置",
        params: { panelId },
      });
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  console.warn(
    `[slTerminal] 页面 ${pageId} 的 DockviewApi 在 5s 内未就绪，无法打开 Hooks 配置`,
  );
  return false;
}
