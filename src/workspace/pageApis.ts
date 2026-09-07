// pageApis — 页面 API 注册表 + 共享页面切换
//
// 模块级 Map<pageId, DockviewApi>，管理每个页面的 DockviewApi 实例。
// 提供 register/unregister/get 操作，以及共享切换函数 switchToPageShared /
// switchToPageAndFocus / openSettingsPanel，外加会话/面板反查
// findPanelForSession / findPageIdForPanelId（FE-09 自 NavTree 上提）。
//
// 不变量：window.__dockviewApi 重指向只允许出现在三站点——
//   switchToPageShared（本文件）、Workspace.onDeletePage、Workspace.handlePageApiReady
//
// 契约要点见 src/features/settingsCenter/CLAUDE.md（openSettingsPanel 同页单例）

import type { DockviewApi } from "dockview-react";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { setProjectRoot } from "../ipc/fs";
import { toast } from "../lib";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { parseTerminalPageId } from "../lib/panelId";
import { basename } from "../lib/path";
import { keyOf } from "../features/agentHistory/historyModel";

/** 模块级页面 API 注册表 */
const pageApiMap = new Map<string, DockviewApi>();

/** 页面 DockviewApi 就绪事件名（CP-042：openSettingsPanel 事件驱动等待；detail = pageId） */
export const PAGE_API_READY_EVENT = "slterm:page-api-ready";

/** 注册页面 DockviewApi（就绪时派发 window CustomEvent——事件驱动替代轮询） */
export function registerPageApi(pageId: string, api: DockviewApi): void {
  pageApiMap.set(pageId, api);
  window.dispatchEvent(
    new CustomEvent(PAGE_API_READY_EVENT, { detail: pageId }),
  );
}

/** 注销页面 DockviewApi */
export function unregisterPageApi(pageId: string): void {
  pageApiMap.delete(pageId);
}

/** 获取页面 DockviewApi */
export function getPageApi(pageId: string): DockviewApi | undefined {
  return pageApiMap.get(pageId);
}

/** 遍历全部页面 DockviewApi（含隐藏页面——E2E 兜底清理隐藏页面残留面板用） */
export function getAllPageApis(): DockviewApi[] {
  return Array.from(pageApiMap.values());
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
          // BE-23：与 FE-04 三处一致——失败 toast 可感知（原仅 console.error）
          toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
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
 * @param signal FE-26: 可选 AbortSignal——调用方卸载/再次点击时 abort，
 *   中止后轮询静默退出（不 focus、不 warn；避免过期聚焦动作落到已切换的页面）
 */
export async function switchToPageAndFocus(
  pageId: string,
  panelId: string,
  signal?: AbortSignal,
): Promise<void> {
  await switchToPageShared(pageId);

  // 轮询面板挂载——页面可能尚未初始化，等 handlePageApiReady 注册 api 后再查
  for (let i = 0; i < 50; i++) {
    if (signal?.aborted) return; // FE-26: abort 后停止轮询
    const panel = getPageApi(pageId)?.getPanel(panelId);
    if (panel) {
      panel.focus();
      return;
    }
    // FE-48：abort 感知轮询——abort 时立即 clearTimeout + resolve，不等下一 tick
    //（循环顶部 signal?.aborted 检查在下一轮退出——abort 后 resolve 落入顶部即返回）
    await new Promise<void>((resolve) => {
      const timer = setTimeout(resolve, 100);
      signal?.addEventListener(
        "abort",
        () => { clearTimeout(timer); resolve(); },
        { once: true },
      );
    });
  }

  console.warn(
    `[slTerminal] 面板 ${panelId} 在 5s 内未就绪，无法聚焦`,
  );
}

/**
 * 打开设置中心面板（同页单例）——调用方须先切到目标页
 * （本函数不切页，见 features/settingsCenter/openSettings.ts 编排）。
 *
 * 面板 id = `settings-{pageId}`；getPanel 命中 → focus 返回 true（同页单例），
 * 未命中 → addPanel（component "settings"，renderer "always"——CP-017；
 * settingsPageId 深链时注入 params.selectedPage）。
 * 页面 api 就绪改事件驱动等待（CP-042）：registerPageApi 派发
 * `slterm:page-api-ready`，5s 超时仅作防御底线——超时经 toast 可观测化
 * （原仅 console.warn 静默降级），返回 false 不抛异常。
 * @param settingsPageId 可选深链目标配置页 id（壳据此选中该配置页）
 * @returns 面板打开成功与否（超时返回 false）
 */
export async function openSettingsPanel(
  pageId: string,
  settingsPageId?: string,
): Promise<boolean> {
  const panelId = `settings-${pageId}`;
  const api = await waitPageApi(pageId, 5000);
  if (!api) {
    console.warn(
      `[slTerminal] 页面 ${pageId} 的 DockviewApi 在 5s 内未就绪，无法打开设置中心`,
    );
    toast.show("warning", "设置中心打开失败:操作页面尚未就绪,请重试");
    return false;
  }
  const existing = api.getPanel(panelId);
  if (existing) {
    existing.focus?.();
    return true;
  }
  api.addPanel({
    id: panelId,
    component: "settings",
    title: "设置",
    renderer: "always",
    params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) },
  });
  return true;
}

/** 等待页面 DockviewApi 注册——事件驱动（PAGE_API_READY_EVENT）+ 超时防御底线 */
function waitPageApi(
  pageId: string,
  timeoutMs: number,
): Promise<DockviewApi | undefined> {
  const existing = getPageApi(pageId);
  if (existing) return Promise.resolve(existing);
  return new Promise((resolve) => {
    const onReady = (e: Event) => {
      if ((e as CustomEvent<string>).detail !== pageId) return;
      cleanup();
      resolve(getPageApi(pageId));
    };
    const timer = setTimeout(() => {
      cleanup();
      resolve(undefined);
    }, timeoutMs);
    const cleanup = () => {
      clearTimeout(timer);
      window.removeEventListener(PAGE_API_READY_EVENT, onReady);
    };
    window.addEventListener(PAGE_API_READY_EVENT, onReady);
  });
}

// ---- 会话/面板反查（FE-09 自 NavTree 上提——双击弹窗「切换到该会话操作页面」用） ----

/**
 * 反查运行中会话所在终端面板：复合键 `cliId|sessionId` 精确匹配（MC-313——与
 * deriveActiveSessionStatuses 同键形态），两侧键构造均经 keyOf 单点
 * （cliId 缺省回退 CLAUDE_CLI_ID + 转义，ZQ-1）；未命中 → undefined。
 */
export function findPanelForSession(
  cliId: string,
  sessionId: string,
): string | undefined {
  const key = keyOf(cliId, sessionId);
  for (const [panelId, entry] of TerminalRegistry.getAll()) {
    const cs = entry.agentSession;
    if (!cs) continue;
    let id = cs.sessionId;
    if (!id && cs.usageSourcePath) {
      const base = basename(cs.usageSourcePath);
      id = base.endsWith(".jsonl") ? base.slice(0, -".jsonl".length) : base;
    }
    if (!id) continue;
    if (keyOf(cs.cliId, id) === key) return panelId;
  }
  return undefined;
}

/**
 * panelId → 属主 pageId（B14 防御分层）：先按已知页面集合做前缀匹配——旧恢复格式
 * （terminal-{pageId}-{Date.now}-{seq}）的 pageId 含数字段，语法切分会把 Date.now
 * 段误并入 pageId 得到幽灵页面；前缀匹配对旧格式可靠。兜底 parseTerminalPageId
 * （新格式）；均未命中 → null。
 */
export function findPageIdForPanelId(panelId: string): string | null {
  const { projects } = useProjects.getState();
  for (const project of Object.values(projects)) {
    for (const page of project.pages) {
      if (panelId.startsWith(`terminal-${page.pageId}-`)) {
        return page.pageId;
      }
    }
  }
  return parseTerminalPageId(panelId);
}
