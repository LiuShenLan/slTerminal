// pageGroups — 页组分组模型纯函数族（CP-004/S11 协议单点）
//
// 共享宿主架构：单一 DockviewReact 内每操作页面 = 宿主内一个顶级页组（group），
// 组 id = page-{pageId}；面板 id 从裸值改为「页前缀 + localId」全量协议
// （panelId = "{pageId}:{localId}"）——宿主内全局唯一 + 页归属可语法解析。
//
// 跨页组拖拽禁止判定 + 页组查询全部收敛本模块（纯函数，零 React/零 DOM，
// L2 直测）。页内编号仍 local（terminal-N 每页从 0 起计数契约不变）。

import type { DockviewApi, IDockviewPanel } from "dockview-react";
import { makeTerminalPanelId } from "../lib/panelId";

/** 页组 id 前缀（group id = page-{pageId}；与面板 id 的 ":" 前缀协议互不相干） */
const PAGE_GROUP_PREFIX = "page-";

/**
 * 页面 → 宿主内页组 id（每操作页面 = 宿主内一个顶级页组）。
 * 例：page-1788531106918-2 → page-page-1788531106918-2
 */
export const pageGroupId = (pageId: string): string => `${PAGE_GROUP_PREFIX}${pageId}`;

/** 页组 id → 所属 pageId（非页组 id 返回 null——dockview 自生组无页归属） */
export const pageIdOfGroupId = (groupId: string): string | null =>
  groupId.startsWith(PAGE_GROUP_PREFIX)
    ? groupId.slice(PAGE_GROUP_PREFIX.length)
    : null;

/** 页前缀面板 id 构造：panelId = "{pageId}:{localId}"（localId 页内唯一即可） */
export const panelIdInPage = (pageId: string, localId: string): string =>
  `${pageId}:${localId}`;

/**
 * 新建终端完整面板 id 单点（workspace 各新建入口 + restoreSession 共用）：
 * local 编号经 makeTerminalPanelId（lib/panelId 每页计数，布局恢复不占号）
 * → 页前缀协议 id "{pageId}:terminal-N"。
 */
export const makeTerminalIdInPage = (pageId: string): string =>
  panelIdInPage(pageId, makeTerminalPanelId(pageId));

/**
 * 面板 id → 所属 pageId。判据 = 首个 ":" 前段（页前缀协议）。
 * 非协议 id（无 ":"——旧格式 terminal-{pageId}-{seq} 形态等）→ null。
 */
export const pageOfPanelId = (panelId: string): string | null =>
  panelId.includes(":") ? panelId.slice(0, panelId.indexOf(":")) : null;

/**
 * 查询页组内全部面板（api.panels 按页前缀过滤；宿主 api 单例下等价
 * 「页面 A 的所有面板」）。
 */
export function panelsOfPage(api: DockviewApi, pageId: string): IDockviewPanel[] {
  return api.panels.filter((p) => pageOfPanelId(p.id) === pageId);
}

/**
 * 面板是否归属于目标页组（跨页组拖拽禁止判定核心谓词）：
 * panel 的页前缀与 group 的 pageId 一致才归属；目标组非页组（dockview
 * 自生组，如拖拽拆分产物）→ 恒 false（任何带页前缀面板都不得落其中）。
 */
export const panelBelongsToGroup = (panelId: string, groupId: string): boolean => {
  const pageId = pageIdOfGroupId(groupId);
  if (pageId === null) return false;
  return pageOfPanelId(panelId) === pageId;
};

/**
 * 设置面板判据（tabClose dirty 守卫/SettingsPanel 自检共享单点——协议形态
 * "{pageId}:settings"；兼容旧 "settings-{pageId}" 形态防历史残留误判）。
 */
export const isSettingsPanelId = (panelId: string | undefined): panelId is string =>
  !!panelId
  && (panelId.endsWith(":settings") || panelId.startsWith("settings-"));

/**
 * 设置面板 → 属主 pageId（协议 id 页前缀解析；旧 settings-{pageId} 形态经
 * 已知页集合前缀匹配兜底）。未命中 → null。
 */
export function pageIdOfSettingsPanel(panelId: string | undefined): string | null {
  if (!panelId) return null;
  const fromProtocol = pageOfPanelId(panelId);
  if (fromProtocol !== null) return fromProtocol;
  return panelId.startsWith("settings-")
    ? panelId.slice("settings-".length)
    : null;
}
