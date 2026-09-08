/* eslint-disable @typescript-eslint/no-explicit-any */
// layoutSerde — 布局序列化/反序列化（硬约束 #7 单点：操作页面布局只经本模块存取）
//
// CP-004/S11 契约演进（共享宿主 + 页组模型）：
// - saveLayout(api) 不变——单宿主全量 toJSON（宿主 = 全部页面页组）。
// - 存储形态：projects store 的 OperationPage.layout = 该页「页组子树切片」——
//   规范切片 = { grid: { root: branch[leaf(page-{pageId})] }, panels: 页内面板, activeGroup }。
// - composeHostLayout(页切片集合) → 宿主全量 JSON（fromJSON 前汇编单点）。
// - loadPageGroup(api, pageId, saved)：运行期把一页切片并入现有宿主
//   （toJSON 合并 + fromJSON reuseExistingPanels——既有面板实例不重建）。
// - 旧多实例格式迁移（patchLegacyLayout 增补）：无 page- 前缀页组的旧布局 →
//   压平成该页单一页组（多组面板并入同一组，按序遍历）；白名单过滤后
//   无法归组/被剔除的面板丢弃 + console.error（不阻断启动，Watermark 接管空页）。

import type { DockviewApi } from "dockview-react";
import { isValidPanelType } from "../panelRegistry";
import { pageGroupId, panelIdInPage } from "./pageGroups";

/** 从 Dockview API 导出布局 JSON（全量——宿主唯一实例） */
export function saveLayout(api: DockviewApi): object {
  return api.toJSON();
}

// ── 类型形状 ──────────────────────────────────────────────

/** 松散布局结构（读写前 JSON 深度拷贝，防污染 store） */
type JsonLayout = Record<string, any>;

// ── 纯函数工具 ────────────────────────────────────────────

/** 深拷贝（布局 JSON 进出本模块一律深拷贝——fromJSON 前防修改 store 原数据） */
function deepClone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

/** 遍历收集 grid 树全部 leaf（深度优先、data 序——旧多组布局压平顺序语义） */
function collectLeaves(root: JsonLayout | undefined): JsonLayout[] {
  if (!root || typeof root !== "object") return [];
  if (root.type === "leaf") return [root];
  const data = root.data;
  if (!Array.isArray(data)) return [];
  return data.flatMap((child: JsonLayout) => collectLeaves(child));
}

/** 校验面板 component 白名单（非法/未知 → 剔除，返回剔除面板 id 列表） */
function filterInvalidPanels(
  layout: JsonLayout,
  onDiscard?: (panelId: string) => void,
): void {
  if (!layout.panels || typeof layout.panels !== "object") return;
  const panels = layout.panels as Record<string, JsonLayout>;
  for (const key of Object.keys(panels)) {
    const panel = panels[key];
    const id = panel?.contentComponent ?? panel?.component;
    if (id && !isValidPanelType(id)) {
      delete panels[key];
      onDiscard?.(key);
    }
  }
}

/** 旧格式字段修补（component→contentComponent / orientation / leaf id / activeGroup） */
function patchLegacyShape(layout: JsonLayout): void {
  if (layout.panels && typeof layout.panels === "object") {
    const panels = layout.panels as Record<string, JsonLayout>;
    for (const key of Object.keys(panels)) {
      const panel = panels[key];
      if (panel.component && !panel.contentComponent) {
        panel.contentComponent = panel.component;
        delete panel.component;
      }
    }
  }
  const grid = layout.grid as JsonLayout | undefined;
  if (!grid || !grid.root) return;
  if (typeof grid.orientation !== "string") grid.orientation = "HORIZONTAL";
  const root = grid.root as JsonLayout;
  if (root.type !== "branch" || !Array.isArray(root.data)) return;
  for (const leaf of root.data as JsonLayout[]) {
    if (!leaf || leaf.type !== "leaf") continue;
    const leafData = leaf.data as JsonLayout | undefined;
    if (!leafData) continue;
    if (typeof leafData.id !== "string") {
      const views = leafData.views as string[] | undefined;
      leafData.id = views?.[0] ? `group-${views[0]}` : "group-default";
    }
    if (typeof layout.activeGroup !== "string") {
      layout.activeGroup = leafData.id;
    }
  }
}

/** 新格式切片判定：grid 顶层 leaf 中存在 page- 前缀页组（宿主/切片形态） */
function hasPageGroupLeaf(layout: JsonLayout): boolean {
  const root = layout.grid?.root as JsonLayout | undefined;
  if (!root || root.type !== "branch" || !Array.isArray(root.data)) return false;
  return (root.data as JsonLayout[]).some(
    (child) => child?.type === "leaf" && typeof child.data?.id === "string"
      && (child.data.id as string).startsWith("page-"),
  );
}

// ── 页切片（OperationPage.layout 语义）─────────────────────

/**
 * 规范空页切片（页组零面板——dockview 空组渲染 Watermark；`{}` 形态旧占位
 * 布局在 normalize 时同样归此）。
 */
export function emptyPageLayout(pageId: string): Record<string, unknown> {
  const gid = pageGroupId(pageId);
  return {
    grid: {
      orientation: "HORIZONTAL",
      root: { type: "branch", data: [{ type: "leaf", data: { id: gid, views: [] } }] },
    },
    panels: {},
    activeGroup: gid,
  };
}

/**
 * 旧多实例格式 → 页组切片迁移（CP-004）：旧布局（每页一 Dockview 实例产出的
 * 完整 SerializedDockview，组 id 无 page- 前缀）压平为该页单一页组——
 * 全部 leaf 面板按 grid 树序遍历并入 page-{pageId} 组；面板 id 重写为
 * 页前缀协议（{pageId}:{oldId}，params.panelId 同步）；无法归组的面板
 * （无 id 引用/无 panels 条目/白名单剔除）丢弃 + console.error，不阻断启动。
 */
function migrateLegacyToSlice(pageId: string, raw: JsonLayout): JsonLayout {
  const gid = pageGroupId(pageId);
  // 深拷贝后白名单过滤（丢弃面板需 console.error——原 loadLayout 静默删除）
  const layout = deepClone(raw);
  const discarded: string[] = [];
  filterInvalidPanels(layout, (id) => discarded.push(id));
  patchLegacyShape(layout);

  // 收集旧布局全部 leaf（含嵌套 branch——旧实例内多组布局）
  const leaves = collectLeaves(layout.grid?.root as JsonLayout | undefined);
  const panelsIn: Record<string, JsonLayout> =
    (layout.panels as Record<string, JsonLayout> | undefined) ?? {};

  // 按树序遍历视图 id（去重——跨组引用同一面板仅一份）
  const seen = new Set<string>();
  const ordered: string[] = [];
  for (const leaf of leaves) {
    const views = leaf.data?.views;
    if (!Array.isArray(views)) continue;
    for (const v of views) {
      if (typeof v === "string" && !seen.has(v)) { seen.add(v); ordered.push(v); }
    }
  }

  // 重写面板：新 id = {pageId}:{oldId}；params.panelId 同步改写
  const panelsOut: Record<string, JsonLayout> = {};
  const viewsOut: string[] = [];
  for (const oldId of ordered) {
    if (!panelsIn[oldId]) {
      // panels 条目缺失（幽灵视图引用）→ 无法归组，丢弃
      console.error(
        `[layoutSerde] 布局迁移丢弃幽灵面板引用 ${oldId}（无 panels 条目，页面 ${pageId}）`,
      );
      continue;
    }
    const state = deepClone(panelsIn[oldId]);
    const newId = panelIdInPage(pageId, oldId);
    state.id = newId;
    if (state.params && typeof state.params === "object") {
      if (typeof state.params.panelId === "string") state.params.panelId = newId;
    }
    panelsOut[newId] = state;
    viewsOut.push(newId);
  }
  if (discarded.length > 0) {
    console.error(
      `[layoutSerde] 布局迁移丢弃 ${discarded.length} 个失效类型面板（页面 ${pageId}）：${discarded.join(", ")}`,
    );
  }

  return {
    grid: {
      orientation: "HORIZONTAL",
      root: { type: "branch", data: [{ type: "leaf", data: { id: gid, views: viewsOut } }] },
    },
    panels: panelsOut,
    activeGroup: gid,
  };
}

/**
 * 页布局归一化（存取单点）：任意形态（新格式切片/旧多实例格式/空占位）→
 * 规范页切片。返回 null = 无法解析（无 grid 且无 panels → 视为空页切片，
 * 不在此路径返回 null；仅当输入非对象时返回 null）。
 */
export function normalizePageLayout(
  pageId: string,
  saved: unknown,
): Record<string, unknown> | null {
  if (typeof saved !== "object" || saved === null || Array.isArray(saved)) return null;
  const raw = saved as JsonLayout;

  // 空占位（makeEmptyLayout {} / 无内容）→ 空页切片
  const hasGrid = raw.grid && typeof raw.grid === "object";
  const hasPanels = raw.panels && typeof raw.panels === "object"
    && Object.keys(raw.panels).length > 0;
  if (!hasGrid && !hasPanels) return emptyPageLayout(pageId);

  // 新格式（已含 page- 页组）→ 规范化：取本页页组 leaf + 页内 panels
  if (hasPageGroupLeaf(raw)) {
    const layout = deepClone(raw);
    filterInvalidPanels(layout);
    patchLegacyShape(layout);
    const leaves = collectLeaves(layout.grid?.root as JsonLayout | undefined);
    const myLeaf = leaves.find(
      (l) => l.data?.id === pageGroupId(pageId),
    ) ?? { type: "leaf", data: { id: pageGroupId(pageId), views: [] } };
    const panelsAll = (layout.panels as Record<string, JsonLayout> | undefined) ?? {};
    const views = Array.isArray(myLeaf.data?.views) ? myLeaf.data?.views as string[] : [];
    const panelsOut: Record<string, JsonLayout> = {};
    for (const v of views) {
      if (typeof v === "string" && panelsAll[v]) panelsOut[v] = panelsAll[v];
    }
    // 本页组不在树中（损坏）但 panels 存在 → 全部并入（防御）
    const inMyLeaf = new Set(views);
    if (myLeaf.data?.id !== pageGroupId(pageId)) {
      for (const [k, v] of Object.entries(panelsAll)) {
        if (!inMyLeaf.has(k)) panelsOut[k] = v;
      }
    }
    return {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: {
              id: pageGroupId(pageId),
              views: Object.keys(panelsOut),
              activeView: views[0] ?? undefined,
            },
            ...(typeof myLeaf.size === "number" ? { size: myLeaf.size } : {}),
          }],
        },
      },
      panels: panelsOut,
      activeGroup: pageGroupId(pageId),
    };
  }

  // 旧多实例格式 → 迁移
  return migrateLegacyToSlice(pageId, raw);
}

/**
 * 从宿主全量 JSON 提取某页的规范切片（布局变更写回单点——saveLayout 后按页切分）。
 * 宿主中不存在该页页组 → 返回空页切片（页组尚未挂入/刚删除）。
 */
export function slicePageLayout(
  pageId: string,
  fullLayout: unknown,
): Record<string, unknown> {
  if (typeof fullLayout !== "object" || fullLayout === null) {
    return emptyPageLayout(pageId);
  }
  const layout = fullLayout as JsonLayout;
  const gid = pageGroupId(pageId);
  const leaves = collectLeaves(layout.grid?.root as JsonLayout | undefined);
  const myLeaf = leaves.find((l) => l.data?.id === gid);
  const panelsAll = (layout.panels as Record<string, JsonLayout> | undefined) ?? {};
  const views = myLeaf && Array.isArray(myLeaf.data?.views)
    ? (myLeaf.data?.views as string[]).filter((v) => typeof v === "string" && panelsAll[v])
    : [];
  const panelsOut: Record<string, JsonLayout> = {};
  for (const v of views) panelsOut[v] = panelsAll[v];
  return {
    grid: {
      orientation: "HORIZONTAL",
      root: {
        type: "branch",
        data: [{
          type: "leaf",
          data: {
            id: gid,
            views,
            ...(myLeaf?.data?.activeView != null
              ? { activeView: myLeaf.data.activeView }
              : {}),
          },
          ...(myLeaf && typeof myLeaf.size === "number" ? { size: myLeaf.size } : {}),
        }],
      },
    },
    panels: panelsOut,
    activeGroup: gid,
  };
}

/**
 * 宿主全量 JSON 汇编（启动/页增删后重建单点）：全部页切片合并为单宿主
 * Dockview fromJSON 输入——每个操作页面 = 根 branch 下一个顶级 leaf 页组，
 * panels 取并集，activeGroup 指向给定活跃页（无活跃页 → 第一页）。
 */
export function composeHostLayout(
  pageSlices: Array<{ pageId: string; layout: unknown }>,
  activePageId: string | null,
): Record<string, unknown> {
  const rootData: JsonLayout[] = [];
  const panels: Record<string, JsonLayout> = {};
  let firstGid: string | null = null;
  for (const { pageId, layout } of pageSlices) {
    const slice = (normalizePageLayout(pageId, layout)
      ?? emptyPageLayout(pageId)) as JsonLayout;
    const leaves = collectLeaves(slice.grid?.root);
    const leaf = leaves.find((l) => l.data?.id === pageGroupId(pageId))
      ?? { type: "leaf", data: { id: pageGroupId(pageId), views: [] } };
    rootData.push(leaf);
    const slicePanels: Record<string, JsonLayout> = slice.panels ?? {};
    for (const [k, v] of Object.entries(slicePanels)) panels[k] = v;
    firstGid ??= pageGroupId(pageId);
  }
  const activeGid = activePageId ? pageGroupId(activePageId) : null;
  return {
    grid: {
      orientation: "HORIZONTAL",
      root: { type: "branch", data: rootData },
    },
    panels,
    activeGroup: activeGid && rootData.some((l) => l.data?.id === activeGid)
      ? activeGid
      : (firstGid ?? undefined),
  };
}

// ── Dockview 恢复原语 ─────────────────────────────────────

/** 旧格式修补（对旧实例布局的兜底修补——loadLayout 通用路径保留） */
function patchLegacyLayout(layout: JsonLayout): void {
  try {
    patchLegacyShape(layout);
  } catch {
    // 修补失败不阻塞，fromJSON 会自然报错
  }
}

/**
 * 恢复布局到 Dockview（通用整树恢复），返回是否成功。
 * 失败原因：旧格式不兼容、组件白名单过滤后无剩余面板、Dockview 内部异常。
 * 宿主启动路径：composeHostLayout 汇编 → 本函数；失败交由 Watermark 接管。
 */
export function loadLayout(api: DockviewApi, saved: object): boolean {
  try {
    const layout = deepClone(saved) as JsonLayout;

    if (layout.panels && typeof layout.panels === "object") {
      const panels = layout.panels as Record<string, JsonLayout>;
      for (const key of Object.keys(panels)) {
        const panel = panels[key];
        const id = panel.contentComponent ?? panel.component;
        if (id && !isValidPanelType(id)) delete panels[key];
      }
    }

    patchLegacyLayout(layout);

    api.fromJSON(layout as Parameters<DockviewApi["fromJSON"]>[0], { reuseExistingPanels: true });
    return true;
  } catch (err) {
    console.error("布局恢复失败:", err);
    return false;
  }
}

/**
 * 运行期页组挂载（loadPageGroup——页新增/重启补页单点）：页切片并入当前宿主
 * 全量 JSON 后 fromJSON（reuseExistingPanels——既有面板实例跨恢复存活，终端
 * 不二次 open）；返回是否成功。调用方须在成功后自行处理可见性/标题恢复。
 */
export function loadPageGroup(
  api: DockviewApi,
  pageId: string,
  saved: unknown,
): boolean {
  try {
    const rawSlice = normalizePageLayout(pageId, saved);
    if (!rawSlice) return false;
    const slice = rawSlice as JsonLayout;
    const current = deepClone(saveLayout(api)) as JsonLayout;
    const gid = pageGroupId(pageId);
    // 移除同页旧叶（幂等——重复挂载防双叶）
    const root = current.grid?.root;
    if (root?.type === "branch" && Array.isArray(root.data)) {
      root.data = (root.data as JsonLayout[]).filter(
        (child: JsonLayout) => !(child?.type === "leaf" && child.data?.id === gid),
      );
    }
    const leaves = collectLeaves(slice.grid?.root);
    const leaf = leaves.find((l) => l.data?.id === gid)
      ?? { type: "leaf", data: { id: gid, views: [] } };
    if (!root) {
      current.grid = { orientation: "HORIZONTAL", root: { type: "branch", data: [] } };
    }
    current.grid.root.data.push(leaf);
    current.panels = { ...(current.panels ?? {}) };
    const slicePanels = (slice.panels as Record<string, JsonLayout> | undefined) ?? {};
    for (const [k, v] of Object.entries(slicePanels)) {
      (current.panels as Record<string, JsonLayout>)[k] = v;
    }
    current.activeGroup = gid;

    api.fromJSON(current as Parameters<DockviewApi["fromJSON"]>[0], { reuseExistingPanels: true });
    return true;
  } catch (err) {
    console.error(`页组 ${pageId} 挂载失败:`, err);
    return false;
  }
}
