// openFile.ts — 文件打开共享链路（workspace 层单点）
//
// 由 ExplorerPanel.handleOpenFile（双击/Enter/右键打开）核心逻辑抽取：文件浏览器
// 与 docViewer 预览链接点击共用同一「打开分发」——去重聚焦 → FileViewerRegistry
// 分派面板类型 → addPanel（isAlwaysRenderPanel 决定 renderer）→ 标题注册与重算。
// 复制即双源漂移（注册表家族契约），凡「应用内打开文件」一律走本模块。
//
// 两层入口：
// - openFileInPage(ctx, filePath)：纯逻辑 + 注入依赖，L2 直测；
// - openFileInActivePage(filePath)：从 stores + window.__dockviewApi 现取上下文
//   （__dockviewApi 恒指活跃操作页——workspace/CLAUDE.md「__dockviewApi 重指
//   不变量」；交互必发生在活跃页）。
//
// 失败语义：守卫不通过/面板创建失败 → 返回 false（不抛错、不弹窗——
// 预览链接点击场景静默忽略，ExplorerPanel 双击场景行为与抽取前逐字等价）。

import type { DockviewApi } from "dockview-react";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { titleManager } from "./titleManager";
import { panelIdInPage, pageGroupId } from "./pageGroups";
import { PANEL_EDITOR, isAlwaysRenderPanel } from "../panelRegistry";
import { fileViewerRegistry } from "../features/fileViewers";

/**
 * handleOpenFile 前置守卫：无活跃操作页或无 Dockview API 时禁止打开面板。
 * type predicate 使通过后 activePageId 自动收窄为非 null。
 */
export const canOpenFile = (
  activePageId: string | null,
  dockviewApi: unknown,
): activePageId is string => !!activePageId && !!dockviewApi;

/** openFileInPage 的上下文（活跃页/API/路径信息，由调用方或便捷入口现取） */
export interface OpenFileContext {
  activePageId: string | null;
  dockApi: unknown;
  /** 项目 rootPath（标题计算相对根） */
  rootPath: string | null;
  /** 项目根路径 */
  projectRootPath: string | null;
}

/** 取活跃项目/页面路径信息（与 ExplorerPanel 渲染期提取同源遍历逻辑） */
function resolveActivePaths(activePageId: string | null): Pick<
  OpenFileContext,
  "rootPath" | "projectRootPath"
> {
  let rootPath: string | null = null;
  let projectRootPath: string | null = null;
  if (activePageId) {
    const projects = useProjects.getState().projects;
    for (const [, proj] of Object.entries(projects)) {
      const activePage = proj.pages.find((p) => p.pageId === activePageId);
      if (activePage) {
        rootPath = proj.rootPath;
        projectRootPath = proj.rootPath;
        break;
      }
    }
  }
  return { rootPath, projectRootPath };
}

/**
 * 在指定页打开文件（面板分派核心链路）。
 *
 * @returns 是否成功打开（false = 守卫不通过 / 已存在聚焦 / addPanel 异常，不弹错）
 */
export function openFileInPage(
  ctx: OpenFileContext,
  filePath: string,
): boolean {
  // 前置守卫：无活跃操作页或无 Dockview API 时直接返回
  const { activePageId } = ctx;
  if (!canOpenFile(activePageId, ctx.dockApi)) return false;
  const dockApi = ctx.dockApi as DockviewApi;

  // 去重：相同文件路径不重复打开，聚焦已有面板
  const existingPanelId = titleManager.findExistingEditor(activePageId, filePath);
  if (existingPanelId) {
    const existingPanel = dockApi.getPanel(existingPanelId);
    if (existingPanel) {
      existingPanel.focus();
      return true;
    }
  }

  // 通过 FileViewerRegistry 决定面板类型（未知类型回退 editor）
  const panelType = fileViewerRegistry.resolve(filePath) ?? PANEL_EDITOR;

  // 计算标题（无闪烁——addPanel 时直接传入）
  const root = ctx.projectRootPath || ctx.rootPath || "";
  const title = root
    ? titleManager.getFileEditorTitle(activePageId, root, filePath)
    : titleManager.getFileEditorTitle(activePageId, "", filePath);

  // CP-004：面板 id 页前缀协议（localId 免撞号——页前缀保证宿主内全局唯一）
  const localId = `${panelType}-${Date.now()}`;
  const panelId = panelIdInPage(activePageId, localId);
  // 文件预览类面板（htmlviewer 等）使用 renderer: "always" 保持 iframe/canvas
  // browsing context 存活，避免页签切换/分屏时 DOM 移除导致白屏闪屏
  const renderer = isAlwaysRenderPanel(panelType) ? ("always" as const) : undefined;

  // addPanel 可能抛异常（如布局状态不一致），try-catch 防止 titleManager 状态污染；
  // options.group 显式指定目标页组（生命周期契约）
  try {
    dockApi.addPanel({
      id: panelId,
      component: panelType,
      title,
      params: { panelId, filePath },
      position: { referenceGroup: pageGroupId(activePageId) },
      ...(renderer ? { renderer } : {}),
    });
  } catch {
    // 面板创建失败，跳过标题注册（titleManager 与 DOM 保持无孤记录）
    return false;
  }

  // 仅在 addPanel 成功后注册到标题管理器（保持两状态一致）
  titleManager.registerEditor(activePageId, panelId, filePath);

  // 新文件打开后重算整个页面标题（可能触发既有面板的冲突更新）
  if (root) {
    const updates = titleManager.recomputeTitles(activePageId, root);
    for (const { panelId: pid, title: t } of updates) {
      const p = dockApi.getPanel(pid);
      if (p) p.api.setTitle(t);
    }
  }
  return true;
}

/**
 * 零上下文便捷入口：从 stores + window.__dockviewApi 现取活跃页上下文后打开。
 * docViewer 预览相对链接点击、其它 features 的「应用内打开文件」用此入口。
 */
export function openFileInActivePage(filePath: string): boolean {
  const dockApi = window.__dockviewApi;
  const activePageId = useLayout.getState().activePageId;
  const paths = resolveActivePaths(activePageId);
  return openFileInPage({ activePageId, dockApi, ...paths }, filePath);
}
