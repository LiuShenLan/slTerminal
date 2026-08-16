// openHooksConfig.ts —— 活动栏「配置」钮 → 打开 hooksConfig 面板的公共编排（NAV-05）
//
// 迁移自 SidebarTree「打开 Hooks 配置」右键菜单逻辑（决策 4 入口唯一化——
// 菜单项已随 SidebarTree 退役，配置钮为唯一入口）：
//   - 目标项目：当前活跃页面所属项目优先，兜底第一个项目；无项目 → 无操作
//   - 目标页面：项目已有操作页面 → pages[0]；无 → 新建空布局页面（照 NavTree.handleNewPage 模式）
//   - 先 switchToPageShared 切页（hooksConfig 面板只能在活跃页面打开——
//     useHooksConfig 经 activePageId 推导 rootPath，C13-7），再 openHooksConfigPanel
//   - switchToPageShared 内部完成 setProjectRoot 前置（DBG-5）；目标页面从未初始化时，
//     Workspace 的 activePageId effect 兜底 ensurePageInitialized，openHooksConfigPanel
//     轮询 getPageApi 就绪（100ms×50）——与 SidebarTree 菜单行为等价

import { createPageId, useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import {
  switchToPageShared,
  openHooksConfigPanel,
} from "../../workspace/pageApis";
import { makeEmptyLayout } from "../navTree";

/**
 * 打开 hooks 配置面板（活动栏底部「配置」钮入口）。
 *
 * 无任何项目 → 无操作（hooks 配置依赖项目 rootPath，无法定位配置目标）。
 * 面板打开失败（页面 DockviewApi 5s 未就绪）由 openHooksConfigPanel 内部
 * console.warn 降级，本函数不抛异常（fire-and-forget 语义）。
 */
export async function openHooksConfigFromActivityBar(): Promise<void> {
  const { projects } = useProjects.getState();
  const projectList = Object.values(projects);
  if (projectList.length === 0) return;

  // 目标项目：活跃页面所属项目优先，兜底第一个项目
  const activePageId = useLayout.getState().activePageId;
  const target =
    projectList.find((p) => p.pages.some((pg) => pg.pageId === activePageId)) ??
    projectList[0];

  // 目标页面：已有操作页面 → pages[0]；无 → 新建空布局页面
  let pageId: string;
  if (target.pages.length > 0) {
    pageId = target.pages[0].pageId;
  } else {
    const newPageId = createPageId();
    useProjects.getState().addPage(target.projectId, {
      pageId: newPageId,
      name: `页面-${Date.now() % 10000}`,
      layout: makeEmptyLayout(),
      cwd: target.rootPath,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    });
    pageId = newPageId;
  }

  await switchToPageShared(pageId);
  await openHooksConfigPanel(pageId);
}
