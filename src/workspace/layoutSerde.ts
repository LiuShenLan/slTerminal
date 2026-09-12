/* eslint-disable @typescript-eslint/no-explicit-any */
// layoutSerde — 布局序列化/反序列化（硬约束 #7 单点：操作页面布局只经本模块存取）
//
// CP-004/S11 契约 + ADR-0020 页内分屏演进（共享宿主 + 派生归属模型）：
// - saveLayout(api) 不变——单宿主全量 toJSON（宿主 = 全部页面叶组）。
// - 存储形态：projects store 的 OperationPage.layout = 该页「子树切片」——
//   root 恒为 branch 壳，其 data = 本页各组节点（单页单组 = branch[leaf]；
//   页内分屏 = branch[leaf, leaf, ...] 或嵌套 branch——子节点直挂宿主根时
//   深度保持，gridview 层级交替朝向语义不变）；visible 标记剥除（宿主 toJSON
//   隐藏叶带 visible:false，入存储会致恢复恒隐藏）；floatingGroups/popoutGroups
//   段不存（D7 仅网格分屏）。
// - 组页归属派生（pageIdOfSerializedLeaf）：主组 id page- 前缀快车道 ??
//   叶 views 首面板页前缀——分屏自生组（dockview 自增 id）归属可解析。
// - composeHostLayout(页切片集合) → 宿主全量 JSON（fromJSON 前汇编单点，
//   切片根 branch 子节点摊平直挂宿主根）。
// - loadPageGroup(api, pageId, saved)：运行期把一页切片并入现有宿主
//   （先按归属移除该页全部旧叶再推入 + fromJSON reuseExistingPanels——
//   既有面板实例不重建）。
// - 旧多实例格式迁移（patchLegacyLayout 增补）：无可解析归属叶的旧布局 →
//   压平成该页单一主组（多组面板并入同一组，按序遍历）；白名单过滤后
//   无法归组/被剔除的面板丢弃 + console.error（不阻断启动，Watermark 接管空页）。

import type { DockviewApi } from "dockview-react";
import { isValidPanelType } from "../panelRegistry";
import { pageGroupId, pageIdOfGroupId, pageOfPanelId, panelIdInPage } from "./pageGroups";

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

/** 新格式判定：grid 树内存在可解析页归属的叶（id 前缀或 views 前缀——
 * 分屏自生组叶经 views 解析）；全无 → 旧多实例格式走迁移 */
function hasAnyOwnedLeaf(layout: JsonLayout): boolean {
  return collectLeaves(layout.grid?.root as JsonLayout | undefined).some(
    (l) => pageIdOfSerializedLeaf(l) !== null,
  );
}

/**
 * 序列化叶 → 属主 pageId（ADR-0020 派生归属的序列化侧等价）：
 * 1. 快车道：叶 id page- 前缀（主组——空主组归属仍成立）；
 * 2. 慢车道：views 内首个页前缀协议面板的 pageOfPanelId（分屏自生组）；
 * 3. 皆无 → null。
 */
function pageIdOfSerializedLeaf(leaf: JsonLayout): string | null {
  const gid = leaf.data?.id;
  if (typeof gid === "string") {
    const fast = pageIdOfGroupId(gid);
    if (fast !== null) return fast;
  }
  const views = leaf.data?.views;
  if (Array.isArray(views)) {
    for (const v of views) {
      if (typeof v !== "string") continue;
      const pid = pageOfPanelId(v);
      if (pid !== null) return pid;
    }
  }
  return null;
}

/**
 * 树剪枝原语（通用）：递归保留 keepLeaf 命中的叶；branch 子树全空 → 剪掉
 * （返回 null），单子 branch → 展平为子节点（单子排布无朝向语义，展平安全）。
 * 不改输入（branch 浅拷输出；leaf 经 transformLeaf 变换，缺省原样引用）。
 */
function pruneTree(
  node: JsonLayout | undefined,
  keepLeaf: (leaf: JsonLayout) => boolean,
  transformLeaf: (leaf: JsonLayout) => JsonLayout = (l) => l,
): JsonLayout | null {
  if (!node || typeof node !== "object") return null;
  if (node.type === "leaf") return keepLeaf(node) ? transformLeaf(node) : null;
  if (node.type !== "branch" || !Array.isArray(node.data)) return null;
  const kept = (node.data as JsonLayout[])
    .map((child) => pruneTree(child, keepLeaf, transformLeaf))
    .filter((c): c is JsonLayout => c !== null);
  if (kept.length === 0) return null;
  if (kept.length === 1) return kept[0];
  return { ...node, data: kept };
}

/** 叶浅拷规范化（切片持久化用）：剥 visible（宿主 toJSON 隐藏叶带
 * visible:false——入存储会致恢复恒隐藏）；activeView ∉ views 时归位 views[0]
 * （恢复后激活页签确定；views 空删 activeView 键） */
function normalizeLeafCopy(leaf: JsonLayout): JsonLayout {
  const data = { ...(leaf.data as JsonLayout) };
  delete data.visible;
  const views = Array.isArray(data.views) ? (data.views as string[]) : [];
  if (views.length === 0) {
    delete data.activeView;
  } else if (typeof data.activeView !== "string" || !views.includes(data.activeView)) {
    data.activeView = views[0];
  }
  return { ...leaf, data };
}

/** 收集节点子树全部叶 id 与 views（views 按页前缀过滤——混组存储防御） */
function collectLeafMeta(
  node: JsonLayout | undefined,
  pageId: string,
): { leafIds: string[]; views: string[] } {
  const leafIds: string[] = [];
  const views: string[] = [];
  for (const leaf of collectLeaves(node)) {
    if (typeof leaf.data?.id === "string") leafIds.push(leaf.data.id);
    const vs = leaf.data?.views;
    if (!Array.isArray(vs)) continue;
    for (const v of vs) {
      if (typeof v === "string" && pageOfPanelId(v) === pageId) views.push(v);
    }
  }
  return { leafIds, views };
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
 * 页布局归一化（存取单点）：任意形态（页子树切片/宿主全量/旧多实例格式/
 * 空占位）→ 规范页切片。返回 null = 无法解析（仅当输入非对象时）。
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

  // 新格式（存在可解析归属叶）→ 剪枝规范化（与 slicePageLayout 同路径幂等）
  if (hasAnyOwnedLeaf(raw)) {
    const layout = deepClone(raw);
    filterInvalidPanels(layout);
    patchLegacyShape(layout);
    const sliced = slicePageLayout(pageId, layout) as JsonLayout;
    // 损坏防御：本页面板存在但无叶引用（剪枝为空或部分孤儿）→ 并入首叶
    const slicedPanels = (sliced.panels as Record<string, JsonLayout> | undefined) ?? {};
    const orphans = Object.keys(
      (layout.panels as Record<string, JsonLayout> | undefined) ?? {},
    ).filter((k) => pageOfPanelId(k) === pageId && !slicedPanels[k]);
    if (orphans.length > 0) {
      console.error(
        `[layoutSerde] 布局规范化：${orphans.length} 个本页面板无叶引用，并入首叶（页面 ${pageId}）`,
      );
      const root = sliced.grid.root as JsonLayout;
      const firstLeaf = collectLeaves(root)[0];
      const panelsAll = layout.panels as Record<string, JsonLayout>;
      for (const k of orphans) {
        firstLeaf.data.views.push(k);
        slicedPanels[k] = panelsAll[k];
      }
      sliced.panels = slicedPanels;
    }
    return sliced;
  }

  // 旧多实例格式 → 迁移
  return migrateLegacyToSlice(pageId, raw);
}

/**
 * 从宿主全量 JSON 提取某页的规范切片（布局变更写回单点——saveLayout 后按页
 * 切分）：按派生归属剪枝保留本页各组子树（页内分屏多组随切片持久化），
 * 剥 visible 标记，包根 branch 壳（子节点直挂宿主根时深度保持，朝向语义不变）。
 * 宿主中不存在该页任何组 → 返回空页切片（页组尚未挂入/刚删除）。
 */
export function slicePageLayout(
  pageId: string,
  fullLayout: unknown,
): Record<string, unknown> {
  if (typeof fullLayout !== "object" || fullLayout === null) {
    return emptyPageLayout(pageId);
  }
  const layout = fullLayout as JsonLayout;
  const root = layout.grid?.root as JsonLayout | undefined;
  const rootChildren = root?.type === "branch" && Array.isArray(root.data)
    ? (root.data as JsonLayout[])
    : [];
  // 剪枝保留本页叶（normalizeLeafCopy 浅拷规范化——不污染共享输入）
  const kept = rootChildren
    .map((c) => pruneTree(c, (leaf) => pageIdOfSerializedLeaf(leaf) === pageId, normalizeLeafCopy))
    .filter((c): c is JsonLayout => c !== null);
  if (kept.length === 0) return emptyPageLayout(pageId);

  const sliceRoot: JsonLayout = { type: "branch", data: kept };
  const { leafIds, views } = collectLeafMeta(sliceRoot, pageId);
  const panelsAll = (layout.panels as Record<string, JsonLayout> | undefined) ?? {};
  const panelsOut: Record<string, JsonLayout> = {};
  for (const v of views) {
    if (panelsAll[v]) panelsOut[v] = panelsAll[v];
  }
  // activeGroup：宿主 activeGroup 属本页 → 保留；否则本页首叶
  const hostActive = typeof layout.activeGroup === "string" ? layout.activeGroup : null;
  const activeGid = hostActive !== null && leafIds.includes(hostActive)
    ? hostActive
    : leafIds[0];
  return {
    grid: {
      orientation:
        typeof (layout.grid as JsonLayout | undefined)?.orientation === "string"
          ? (layout.grid as JsonLayout).orientation
          : "HORIZONTAL",
      root: sliceRoot,
    },
    panels: panelsOut,
    activeGroup: activeGid,
  };
}

/**
 * 宿主全量 JSON 汇编（启动/页增删后重建单点）：全部页切片合并为单宿主
 * Dockview fromJSON 输入——每页切片根 branch 的子节点摊平直挂宿主根 branch
 * （切片包壳保持的深度关系不变，gridview 层级交替朝向语义保持）；panels 取
 * 并集；activeGroup 指向活跃页首叶（无活跃页 → 第一页首叶）。
 */
export function composeHostLayout(
  pageSlices: Array<{ pageId: string; layout: unknown }>,
  activePageId: string | null,
): Record<string, unknown> {
  const rootData: JsonLayout[] = [];
  const panels: Record<string, JsonLayout> = {};
  /** 每页首叶 id + 切片声明的 activeGroup（活跃组解析用——分屏页声明值属本页
   *  时保留，重启后聚焦回到用户工作组） */
  const firstLeafOfPage = new Map<string, string>();
  const declaredActiveOfPage = new Map<string, string>();
  let firstLeafId: string | null = null;
  for (const { pageId, layout } of pageSlices) {
    const slice = (normalizePageLayout(pageId, layout)
      ?? emptyPageLayout(pageId)) as JsonLayout;
    const sliceRoot = slice.grid?.root as JsonLayout | undefined;
    const children = sliceRoot?.type === "branch" && Array.isArray(sliceRoot.data)
      ? (sliceRoot.data as JsonLayout[])
      : [];
    rootData.push(...children);
    const leaves = collectLeaves(sliceRoot);
    const leafIds = new Set(
      leaves.map((l) => l.data?.id).filter((id): id is string => typeof id === "string"),
    );
    const firstId = leaves[0]?.data?.id;
    if (typeof firstId === "string") {
      firstLeafOfPage.set(pageId, firstId);
      firstLeafId ??= firstId;
    }
    const declared = slice.activeGroup;
    if (typeof declared === "string" && leafIds.has(declared)) {
      declaredActiveOfPage.set(pageId, declared);
    }
    const slicePanels: Record<string, JsonLayout> = slice.panels ?? {};
    for (const [k, v] of Object.entries(slicePanels)) panels[k] = v;
  }
  const activeGid = activePageId
    ? (declaredActiveOfPage.get(activePageId) ?? firstLeafOfPage.get(activePageId))
    : undefined;
  return {
    grid: {
      orientation: "HORIZONTAL",
      root: { type: "branch", data: rootData },
    },
    panels,
    activeGroup: activeGid ?? firstLeafId ?? undefined,
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
 * 运行期页组挂载（loadPageGroup——页新增/重启补页单点）：先按派生归属移除
 * 该页全部旧叶（递归剪枝——分屏多叶/嵌套分支一并清除，幂等防双叶），再把
 * 页切片根 branch 子节点摊平推入宿主根；fromJSON reuseExistingPanels——
 * 既有面板实例跨恢复存活，终端不二次 open；返回是否成功。调用方须在成功后
 * 自行处理可见性/标题恢复。
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
    // D7 防御剔除（仅网格分屏——floating/popout 段不入恢复）
    delete current.floatingGroups;
    delete current.popoutGroups;
    // 移除本页全部旧叶（含嵌套分支内的；空 branch 随剪枝清除）
    let root = current.grid?.root as JsonLayout | undefined;
    if (root?.type === "branch" && Array.isArray(root.data)) {
      root.data = (root.data as JsonLayout[])
        .map((c) => pruneTree(c, (leaf) => pageIdOfSerializedLeaf(leaf) !== pageId))
        .filter((c): c is JsonLayout => c !== null);
    } else if (!root) {
      current.grid = { orientation: "HORIZONTAL", root: { type: "branch", data: [] } };
      root = current.grid.root as JsonLayout;
    }
    // 切片根 branch 子节点摊平推入宿主根（深度保持——朝向语义不变）
    const sliceRoot = slice.grid?.root as JsonLayout | undefined;
    const children = sliceRoot?.type === "branch" && Array.isArray(sliceRoot.data)
      ? (sliceRoot.data as JsonLayout[])
      : [];
    (root as JsonLayout).data.push(...children);
    current.panels = { ...(current.panels ?? {}) };
    const slicePanels = (slice.panels as Record<string, JsonLayout> | undefined) ?? {};
    for (const [k, v] of Object.entries(slicePanels)) {
      (current.panels as Record<string, JsonLayout>)[k] = v;
    }
    // activeGroup = 切片首叶（并入页成为聚焦组——现状语义保持）
    const sliceLeaves = collectLeaves(sliceRoot);
    const firstId = sliceLeaves[0]?.data?.id;
    current.activeGroup = typeof firstId === "string" ? firstId : pageGroupId(pageId);

    api.fromJSON(current as Parameters<DockviewApi["fromJSON"]>[0], { reuseExistingPanels: true });
    return true;
  } catch (err) {
    console.error(`页组 ${pageId} 挂载失败:`, err);
    return false;
  }
}
