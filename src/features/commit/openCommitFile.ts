// openCommitFile.ts — commit 视图双击分派
//
// 状态→面板类型映射（策略模式，独立导出）。
// 流程照 ExplorerPanel.handleOpenFile：去重聚焦 → addPanel → registerEditor → recomputeTitles。

import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { titleManager } from "../../workspace/titleManager";

/** 文件面板分派结果 */
export interface PanelDispatch {
  panelType: string;
  suffix: string;
}

/**
 * 状态→面板映射表（独立导出，策略模式）。
 * added → editor + "(git add)"
 * untracked → editor + "(git not add)"
 * deleted → gitshow + "(git delete)"
 * modified / renamed / conflict → diff + "(git diff)"
 */
export const STATUS_PANEL_MAP: Record<string, PanelDispatch> = {
  added: { panelType: "editor", suffix: "(git add)" },
  untracked: { panelType: "editor", suffix: "(git not add)" },
  deleted: { panelType: "gitshow", suffix: "(git delete)" },
  modified: { panelType: "diff", suffix: "(git diff)" },
  renamed: { panelType: "diff", suffix: "(git diff)" },
  conflict: { panelType: "diff", suffix: "(git diff)" },
};

/** 根据 git 状态获取面板分派信息，未知状态返回 null */
export function getPanelDispatch(status: string): PanelDispatch | null {
  return STATUS_PANEL_MAP[status] ?? null;
}

/**
 * 双击 commit 文件列表项 → 打开对应面板。
 * 流程照 ExplorerPanel.handleOpenFile。
 */
export function openCommitFile(
  filePath: string,
  status: string,
  oldPath?: string,
): void {
  const dispatch = getPanelDispatch(status);
  if (!dispatch) return;

  const activePageId = useLayout.getState().activePageId;
  if (!activePageId) return;

  const dockApi = window.__dockviewApi;
  if (!dockApi) return;

  // 推导 rootPath（同 ExplorerPanel）
  const projects = useProjects.getState().projects;
  let rootPath: string | null = null;
  for (const [, proj] of Object.entries(projects)) {
    const activePage = proj.pages.find((p) => p.pageId === activePageId);
    if (activePage) {
      rootPath = activePage.cwd || proj.rootPath;
      break;
    }
  }
  if (!rootPath) return;

  const { panelType, suffix } = dispatch;

  // 去重：查找已有同文件 + 同 suffix 的面板
  const existingPanelId = titleManager.findExistingEditor(
    activePageId,
    filePath,
    suffix,
  );
  if (existingPanelId) {
    const existingPanel = dockApi.getPanel(existingPanelId);
    if (existingPanel) {
      existingPanel.focus();
      return;
    }
  }

  // 计算标题（含 suffix）
  const title = titleManager.getFileEditorTitle(
    activePageId,
    rootPath,
    filePath,
    suffix,
  );

  const panelId = `${panelType}-${Date.now()}`;

  // 构造 params：renamed 时传 oldPath
  const params: Record<string, unknown> = {
    panelId,
    filePath,
    repoPath: rootPath,
  };
  if (status === "renamed" && oldPath) {
    params.oldPath = oldPath;
  }

  // addPanel 可能抛异常，try-catch 防止 titleManager 状态污染
  try {
    dockApi.addPanel({
      id: panelId,
      component: panelType,
      title,
      params,
    });
  } catch {
    return;
  }

  // 注册到标题管理器（含 suffix）
  titleManager.registerEditor(activePageId, panelId, filePath, suffix);

  // 重算冲突标题
  const updates = titleManager.recomputeTitles(activePageId, rootPath);
  for (const { panelId: pid, title: t } of updates) {
    const p = dockApi.getPanel(pid);
    if (p) p.api.setTitle(t);
  }
}
