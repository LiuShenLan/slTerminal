// titleManager.ts — 集中标题管理
//
// 非 React 的纯逻辑模块（闭包状态 + 返回函数），负责：
// - 终端/编辑器序号计数器（每页独立）
// - 编辑器面板注册表（panelId → filePath）
// - 标题计算（basename / 相对路径 / 冲突检测）
// - 面板关闭后批量重算标题（recomputeTitles）

import { normalizePath, basename, relativePath } from "../lib/path";

/** 编辑器注册条目 */
export interface EditorEntry {
  filePath?: string;
}

/** 标题更新指令（调用方通过 DockviewApi 执行） */
export interface TitleUpdate {
  panelId: string;
  title: string;
}

/**
 * 创建标题管理器实例。
 * 每个 Workspace 实例应创建一个管理器。
 */
export function createTitleManager() {
  /** Map<pageId, Map<panelId, EditorEntry>> — 编辑器注册表 */
  const registry = new Map<string, Map<string, EditorEntry>>();

  /** Map<pageId, { terminal: number }> — 序号计数器 */
  const counters = new Map<string, { terminal: number }>();

  function getCounter(pageId: string) {
    let c = counters.get(pageId);
    if (!c) {
      c = { terminal: 0 };
      counters.set(pageId, c);
    }
    return c;
  }

  function getPageRegistry(pageId: string) {
    let page = registry.get(pageId);
    if (!page) {
      page = new Map();
      registry.set(pageId, page);
    }
    return page;
  }

  /** 终端页签标题 "terminal-N"，每页独立，关闭不重算 */
  function getTerminalTitle(pageId: string): string {
    const c = getCounter(pageId);
    return `terminal-${c.terminal++}`;
  }

  /**
   * 注册编辑器面板。
   * 在 addPanel 之后调用。
   */
  function registerEditor(
    pageId: string,
    panelId: string,
    filePath?: string,
  ): void {
    const page = getPageRegistry(pageId);
    page.set(panelId, { filePath });
  }

  /** 注销编辑器面板。在面板关闭时调用。 */
  function unregisterEditor(pageId: string, panelId: string): void {
    const page = registry.get(pageId);
    if (page) page.delete(panelId);
  }

  /**
   * 查找是否已有相同 filePath 的编辑器面板。
   * 返回 panelId 或 null。用于重复文件去重聚焦。
   */
  function findExistingEditor(
    pageId: string,
    filePath: string,
  ): string | null {
    const page = registry.get(pageId);
    if (!page) return null;
    const target = normalizePath(filePath);
    for (const [panelId, entry] of page) {
      if (entry.filePath && normalizePath(entry.filePath) === target) {
        return panelId;
      }
    }
    return null;
  }

  /**
   * 计算新打开文件的初始标题（addPanel 时传入，避免闪烁）。
   * 只检测已注册编辑器的冲突，不修改既有面板标题。
   */
  function getFileEditorTitle(
    pageId: string,
    rootPath: string,
    filePath: string,
  ): string {
    const name = basename(filePath);
    const page = getPageRegistry(pageId);
    const normalizedNew = normalizePath(filePath);

    // 检测同名冲突（排除自己）
    let hasConflict = false;
    for (const [, entry] of page) {
      if (!entry.filePath) continue;
      if (normalizePath(entry.filePath) === normalizedNew) continue;
      if (basename(entry.filePath) === name) {
        hasConflict = true;
        break;
      }
    }

    if (!hasConflict) return name;

    // 有冲突 → 显示相对路径，不在项目树中则显示绝对路径
    const rel = relativePath(filePath, rootPath);
    return rel ?? normalizedNew;
  }

  /**
   * 重新计算指定页面所有编辑器面板的标题。
   * 用于面板关闭后、或新建面板注册后批量更新。
   * 调用方负责通过 DockviewApi 执行 setTitle。
   */
  function recomputeTitles(
    pageId: string,
    rootPath: string,
  ): TitleUpdate[] {
    const page = registry.get(pageId);
    if (!page || page.size === 0) return [];

    // 第一遍：统计每个 basename 的出现次数
    const nameCounts = new Map<string, number>();
    for (const [, entry] of page) {
      if (entry.filePath) {
        const name = basename(entry.filePath);
        nameCounts.set(name, (nameCounts.get(name) || 0) + 1);
      }
    }

    // 第二遍：为每个有文件的编辑器生成标题
    const updates: TitleUpdate[] = [];
    for (const [panelId, entry] of page) {
      if (!entry.filePath) continue;

      const name = basename(entry.filePath);
      const count = nameCounts.get(name) || 0;

      if (count <= 1) {
        updates.push({ panelId, title: name });
      } else {
        const rel = relativePath(entry.filePath, rootPath);
        updates.push({
          panelId,
          title: rel ?? normalizePath(entry.filePath),
        });
      }
    }

    return updates;
  }

  /**
   * 页面删除时清理该页所有标题管理状态。
   * 应在 Workspace 删除页面回调中调用。
   */
  function onDeletePage(pageId: string): void {
    registry.delete(pageId);
    counters.delete(pageId);
  }

  /**
   * 另存为后更新文件路径，并返回该页面所有编辑器的标题更新。
   * 调用方负责通过 DockviewApi 执行 setTitle。
   */
  function handleSaveAs(
    pageId: string,
    panelId: string,
    newFilePath: string,
    rootPath: string,
  ): TitleUpdate[] {
    const page = getPageRegistry(pageId);
    page.set(panelId, { filePath: newFilePath });
    return recomputeTitles(pageId, rootPath);
  }

  /**
   * 重置所有内部状态（仅用于测试）。
   */
  function reset(): void {
    registry.clear();
    counters.clear();
  }

  return {
    getTerminalTitle,
    getFileEditorTitle,
    registerEditor,
    unregisterEditor,
    findExistingEditor,
    recomputeTitles,
    handleSaveAs,
    onDeletePage,
    reset,
  };
}

/** 全局单例（简化跨组件共享） */
export const titleManager = createTitleManager();
