// pageGroups — 页组分组模型纯函数族（CP-004/S11 协议单点；ADR-0020 页内分屏修订）
//
// 共享宿主架构：单一 DockviewReact 内每操作页面 = 宿主内一组网格叶组；
// 主组 id = page-{pageId}（恢复锚点 + Watermark 载体）；面板 id 「页前缀 +
// localId」全量协议（panelId = "{pageId}:{localId}"）——宿主内全局唯一 +
// 页归属可语法解析。
//
// ADR-0020 起支持页内分屏（单页多组）：拖拽拆分产物 = dockview 自增 id 自生组
// （无 page- 前缀）——组页归属不从组 id 断言，经 pageIdOfGroup 派生（id 快车道
// ?? 组内首面板页前缀）。混组判定/组查询/新增落组解析全部收敛本模块（纯函数，
// 零 React/零 DOM，L2 直测）。页内编号仍 local（terminal-N 每页从 0 起计数契约
// 不变）。

import type { DockviewApi, IDockviewGroupPanel, IDockviewPanel } from "dockview-react";
import { makeTerminalPanelId } from "../lib/panelId";

/** 页组 id 前缀（主组 id = page-{pageId}；与面板 id 的 ":" 前缀协议互不相干） */
const PAGE_GROUP_PREFIX = "page-";

/**
 * 页面 → 宿主内主组 id（每操作页面一个主组——恢复锚点 + Watermark 载体）。
 * 例：page-1788531106918-2 → page-page-1788531106918-2
 */
export const pageGroupId = (pageId: string): string => `${PAGE_GROUP_PREFIX}${pageId}`;

/** 主组 id → 所属 pageId（非主组 id 返回 null——页内分屏自生组无 id 页归属） */
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
 * 组对象 → 派生属主 pageId（ADR-0020 页内分屏核心谓词）：
 * 1. 快车道：组 id page- 前缀（主组）→ 协议解析（空主组归属仍成立——
 *    Watermark 载体）；
 * 2. 慢车道：组内首个页前缀协议面板的 pageOfPanelId（分屏自生组）；
 * 3. 空组且无协议 id → null（无归属——stray 空壳由 auditGroupMembership 清理）。
 */
export function pageIdOfGroup(group: IDockviewGroupPanel): string | null {
  const fast = pageIdOfGroupId(group.id);
  if (fast !== null) return fast;
  for (const panel of group.panels) {
    const pid = pageOfPanelId(panel.id);
    if (pid !== null) return pid;
  }
  return null;
}

/**
 * 查询页内全部组（api.groups 按派生归属过滤——含主组与页内分屏自生组；
 * 删页逐组移除/可见性遍历的枚举单点）。
 */
export function groupsOfPage(api: DockviewApi, pageId: string): IDockviewGroupPanel[] {
  return api.groups.filter((g) => pageIdOfGroup(g) === pageId);
}

/**
 * 新增面板目标组解析（全部 addPanel 调用点单点）：主组在 → 主组；主组不在
 * （页内分屏后主组面板被拖空自动删除）→ 页内首组；页无任何组 → null——
 * 调用方须显式失败分支（dockview 对无效 referenceGroup 直接 throw，
 * 静默落活跃组会错页）。
 */
export function resolvePageGroupForAdd(
  api: DockviewApi,
  pageId: string,
): IDockviewGroupPanel | null {
  const primary = api.getGroup(pageGroupId(pageId));
  if (primary) return primary;
  return groupsOfPage(api, pageId)[0] ?? null;
}

/**
 * 查询页内全部面板（api.panels 按页前缀过滤；宿主 api 单例下等价
 * 「页面 A 的所有面板」）。
 */
export function panelsOfPage(api: DockviewApi, pageId: string): IDockviewPanel[] {
  return api.panels.filter((p) => pageOfPanelId(p.id) === pageId);
}

/**
 * 面板是否归属于目标组（ADR-0020 派生归属语义）：组的派生属主页 === 面板
 * 页前缀页；无归属组（空壳/无协议面板）→ 恒 false。
 */
export const panelBelongsToGroup = (
  panelId: string,
  group: IDockviewGroupPanel,
): boolean => {
  const owner = pageIdOfGroup(group);
  if (owner === null) return false;
  return pageOfPanelId(panelId) === owner;
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
