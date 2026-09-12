// pageApis — 页面 API 注册表 + 共享页面切换（CP-004/S11 单宿主语义）
//
// 共享宿主架构下全局只存在一个 DockviewApi（单一 DockviewReact）；「页面 API」
// 语义收敛为「宿主 API + 页组挂载标记」：
// - registerHostApi：宿主 onReady 注册（宿主唯一，重复注册 = 重建/测试重置）；
// - markPageGroupMounted(pageId) / 组移除注销：getPageApi(pageId) 仅在宿主就绪
//   且该页页组已挂载时返回宿主 API（页未挂载 = 旧架构「页面未初始化」语义）。
// - window.__dockviewApi 指向宿主（唯一，不再随切页重指）。
//
// 切页 = setProjectRoot 前置 → setActivePage → 页组可见性由 Workspace 宿主
// 订阅 activePageId 统一刷新（页组 = 宿主内顶级组，切页即页组容器显隐——
// dockview 叶可见性切换，见 workspace/CLAUDE.md「页组模型」节）。

import type { DockviewApi } from "dockview-react";
import { slicePageLayout } from "./layoutSerde";
import { saveLayout as saveLayoutApi } from "./layoutSerde";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { setProjectRoot } from "../ipc/fs";
import { toast } from "../lib";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { basename } from "../lib/path";
import { keyOf } from "../features/agentHistory/historyModel";
import { pageOfPanelId, pageGroupId, panelIdInPage } from "./pageGroups";

/** 模块级宿主 API（单例——唯一 DockviewReact 实例就绪时注册） */
let hostApi: DockviewApi | null = null;

/** 已挂载页组集合（pageId——宿主就绪后逐页组标记，防「页未挂载即查询」竞态） */
const mountedPageGroups = new Set<string>();

/** 页面 DockviewApi 就绪事件名（CP-042 保留：页组挂载时派发；detail = pageId） */
export const PAGE_API_READY_EVENT = "slterm:page-api-ready";

/**
 * 注册宿主 DockviewApi（Workspace 宿主 onReady 调用——唯一注册点；
 * 宿主重建（测试/重挂载）时幂等覆盖）。
 */
export function registerHostApi(api: DockviewApi): void {
  hostApi = api;
}

/** 标记页组已挂载（宿主 restore/页组并入后逐页调用）——派发就绪事件 */
export function markPageGroupMounted(pageId: string): void {
  mountedPageGroups.add(pageId);
  window.dispatchEvent(
    new CustomEvent(PAGE_API_READY_EVENT, { detail: pageId }),
  );
}

/** 页组移除（页面删除）——同步注销挂载标记（与 mounted 对称） */
export function unregisterPageGroup(pageId: string): void {
  mountedPageGroups.delete(pageId);
}

/** 注销宿主（宿主组件卸载——页面 API 层回到未就绪态） */
export function unregisterHostApi(): void {
  hostApi = null;
  mountedPageGroups.clear();
}

/** 宿主是否已注册（Workspace 宿主 onReady 之前为 false） */
export function isHostReady(): boolean {
  return hostApi !== null;
}

/**
 * 页面所属项目 rootPath 现取（经 stores）——新建终端 cwd 默认值的唯一来源
 * （决策：新建终端 cwd 恒 = 项目根，不跟随活动终端/页面 cwd）；
 * 标题重算/复制相对路径基准共用。未命中项目 → null。
 */
export function projectRootOfPage(pageId: string): string | null {
  const { projects } = useProjects.getState();
  for (const [, proj] of Object.entries(projects)) {
    if (proj.pages.some((p) => p.pageId === pageId)) return proj.rootPath ?? null;
  }
  return null;
}

/**
 * 宿主全量布局 → store 各页切片写回（硬约束 #7 的宿主侧消费单点：
 * saveLayout 全量 toJSON → slicePageLayout 逐页切分 → updatePageLayout，
 * 切片无变化跳过——拖拽/缩放期间的 onDidLayoutChange 高频事件零冗余写）。
 */
export function syncHostLayoutToStore(api: DockviewApi): void {
  const full = saveLayoutApi(api);
  const { projects, updatePageLayout } = useProjects.getState();
  for (const [projId, proj] of Object.entries(projects)) {
    for (const page of proj.pages) {
      const slice = slicePageLayout(page.pageId, full);
      if (JSON.stringify(slice) !== JSON.stringify(page.layout)) {
        updatePageLayout(projId, page.pageId, slice);
      }
    }
  }
}

/**
 * 获取页面 DockviewApi（旧「每页一实例注册表」语义的页组查询替代）：
 * 宿主就绪且该页页组已挂载 → 宿主 API；否则 undefined（调用方轮询——与旧
 * 惰性初始化页面「未就绪」语义等价）。
 */
export function getPageApi(pageId: string): DockviewApi | undefined {
  return hostApi !== null && mountedPageGroups.has(pageId) ? hostApi : undefined;
}

/** 宿主 API 直取（无页挂载条件——宿主内部/遍历场景用） */
export function getHostApi(): DockviewApi | null {
  return hostApi;
}

/**
 * 遍历全部页面 DockviewApi（单宿主 = [宿主]——调用方自行按面板过滤；
 * E2E 兜底清理隐藏页面残留面板用：隐藏页组面板仍在宿主 panels 内）。
 */
export function getAllPageApis(): DockviewApi[] {
  return hostApi !== null ? [hostApi] : [];
}

/**
 * 切换活跃页面——setProjectRoot 前置 await → setActivePage（宿主订阅刷新
 * 页组显隐）。window.__dockviewApi 已收敛为宿主常量，不再于本函数重指。
 *
 * - activePageId 已为目标 pageId 时直接返回（幂等）
 * - 经 useProjects.getState() 查 pageId 所属项目 rootPath，await setProjectRoot
 *   （失败 console.error 降级继续）
 * - useLayout.getState().setActivePage(pageId)——页组容器显隐/标题刷新生效点
 *   在 Workspace 宿主订阅（本函数不直接触碰 dockview，切页编排单点不破）
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

  // 轮询面板挂载——页组挂载标记就绪后宿主 API 可查，等面板渲染落定
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
 * 打开设置中心面板（同页单例，F11 语义不变）——调用方须先切到目标页
 * （本函数不切页，见 features/settingsCenter/openSettings.ts 编排）。
 *
 * 面板 id = panelIdInPage(pageId, "settings")（页前缀协议——每页一个设置面板，
 * dirtyRegistry 键同 params.panelId）；getPanel 命中 → focus 返回 true（同页
 * 单例），未命中 → addPanel（component "settings"，renderer "always"——
 * CP-017；settingsPageId 深链时注入 params.selectedPage），显式落目标页组。
 * 页面就绪改事件驱动等待（CP-042 保留）：页组挂载派发 `slterm:page-api-ready`，
 * 5s 超时仅作防御底线——超时经 toast 可观测化后返回 false。
 * @param settingsPageId 可选深链目标配置页 id（壳据此选中该配置页）
 * @returns 面板打开成功与否（超时返回 false）
 */
export async function openSettingsPanel(
  pageId: string,
  settingsPageId?: string,
): Promise<boolean> {
  const api = await waitPageApi(pageId, 5000);
  if (!api) {
    console.warn(
      `[slTerminal] 页面 ${pageId} 的 DockviewApi 在 5s 内未就绪，无法打开设置中心`,
    );
    toast.show("warning", "设置中心打开失败:操作页面尚未就绪,请重试");
    return false;
  }
  // 设置面板 id = {pageId}:settings（页前缀协议——每页一个设置面板的 F11
  // 单例键；dirtyRegistry 键同 params.panelId；tabClose 守卫判据见 tabClose.ts）
  const panelId = panelIdInPage(pageId, "settings");
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
    position: { referenceGroup: pageGroupId(pageId) },
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
 * panelId → 属主 pageId（CP-004 页前缀协议优先；旧恢复格式兜底）：
 * 1. pageOfPanelId（协议 id "{pageId}:localId"——新形态快速路径）；
 * 2. 已知页面集合前缀匹配——旧格式（terminal-{pageId}-{DateNow}-{seq} 等）
 *    pageId 含数字段，语法切分不可靠，前缀匹配对旧格式可靠（布局迁移兜底
 *    前的老面板）；均未命中 → null。
 */
export function findPageIdForPanelId(panelId: string): string | null {
  const fromProtocol = pageOfPanelId(panelId);
  if (fromProtocol !== null) return fromProtocol;
  const { projects } = useProjects.getState();
  for (const project of Object.values(projects)) {
    for (const page of project.pages) {
      if (
        panelId.startsWith(`terminal-${page.pageId}-`)
        || panelId.startsWith(`settings-${page.pageId}`)
      ) {
        return page.pageId;
      }
    }
  }
  return null;
}

/** 测试专用：重置宿主/挂载集合（宿主级单例重置——用例隔离） */
export function _resetHostApi(): void {
  hostApi = null;
  mountedPageGroups.clear();
}
