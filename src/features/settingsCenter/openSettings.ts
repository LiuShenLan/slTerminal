// openSettings.ts —— 活动栏「配置」钮 → 打开设置中心面板的公共编排（F11）
//
// 迁移自 openHooksConfigFromActivityBar 编排（设置中心取代 hooks 配置面板成为
// 配置钮唯一入口）：
//   - 目标项目：当前活跃页面所属项目优先，兜底第一个项目；
//     无项目 → toast「请先创建项目」+ return（R1 修订——原编排静默 return，
//     但设置面板无 Dockview 宿主可挂，静默不可感知）
//   - 目标页面：项目已有操作页面 → pages[0]；无 → 新建空布局页面
//     （照 NavTree.handleNewPage 模式）
//   - 先 switchToPageShared 切页（面板只能在活跃页面打开），再 openSettingsPanel
//   - switchToPageShared 内部完成 setProjectRoot 前置（DBG-5）；目标页面从未初始化时，
//     Workspace 的 activePageId effect 兜底 ensurePageInitialized，openSettingsPanel
//     轮询 getPageApi 就绪（100ms×50）——与旧编排行为等价

import { createPageId, useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import {
  switchToPageShared,
  openSettingsPanel,
} from "../../workspace/pageApis";
import { makeEmptyLayout } from "../navTree";
import { toast } from "../../lib";

/**
 * 打开设置中心面板（活动栏底部「配置」钮入口）。
 *
 * 无任何项目 → toast「请先创建项目」且不切页（R1：面板无 Dockview 宿主可挂）。
 * 面板打开失败（页面 DockviewApi 5s 未就绪）由 openSettingsPanel 内部
 * console.warn 降级，本函数不抛异常（fire-and-forget 语义）。
 * @param settingsPageId 可选深链目标配置页 id（直接选中该配置页）
 */
export async function openSettings(settingsPageId?: string): Promise<void> {
  const { projects } = useProjects.getState();
  const projectList = Object.values(projects);
  if (projectList.length === 0) {
    toast.show("warning", "请先创建项目");
    return;
  }

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
  await openSettingsPanel(pageId, settingsPageId);
}
