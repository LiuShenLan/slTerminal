// 侧栏视图状态机纯函数——单槽位模型
// 全部展示由 open 纯推导，无历史记忆，无中间态

/** 侧栏区半区标识 */
export type Zone = "top" | "bottom";

/** 按钮归属：各区 id 列表（含排列顺序） */
export interface Zones {
  top: string[];
  bottom: string[];
}

/** 各半区当前打开的视图 id（null 表示槽位为空） */
export interface OpenState {
  top: string | null;
  bottom: string | null;
}

/** 侧栏区展示形态——由打开状态纯推导 */
export type LayoutKind = "hidden" | "single-top" | "single-bottom" | "split";

/** 持久化 / store 中的完整侧栏切片 */
export interface SideBarSlice {
  zones: Zones;
  open: OpenState;
  width: number;
  splitRatio: number;
}

// ── 默认值常量 ──

/** 默认按钮归属：项目列表、文件浏览器、commit 均在上区 */
export const DEFAULT_ZONES: Zones = {
  top: ["projects", "explorer", "commit"],
  bottom: [],
};

/** 默认打开状态：项目列表打开，其余关闭 */
export const DEFAULT_OPEN: OpenState = {
  top: "projects",
  bottom: null,
};

/** 活动栏固定宽度（px） */
export const ACTIVITY_BAR_SIZE = 40;

/** 侧栏区宽度默认值 / 范围（px） */
export const WIDTH_DEFAULT = 250;
export const WIDTH_MIN = 160;
export const WIDTH_MAX = 500;

/** 上下区分割比例默认值 / 范围 */
export const SPLIT_DEFAULT = 0.5;
export const SPLIT_MIN = 0.1;
export const SPLIT_MAX = 0.9;

// ── 内部辅助 ──

/** 查找 id 所属的 Zone，未找到返回 null */
function findZone(zones: Zones, id: string): Zone | null {
  if (zones.top.includes(id)) return "top";
  if (zones.bottom.includes(id)) return "bottom";
  return null;
}

/** 将值 clamp 到 [min, max] */
function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, value));
}

// ── 纯函数 ──

/**
 * 切换视图开关（R1/R2）
 * - id 所在半区槽位为空 → 打开（R1，覆盖即隐式关闭同区旧视图）
 * - id 所在半区槽位已为此 id → 关闭（R2）
 * - id 不在 zones 任一区 → 原样返回（防御）
 * 无历史记忆——返回值仅由入参推导
 */
export function toggleViewPure(
  zones: Zones,
  open: OpenState,
  id: string
): OpenState {
  const zone = findZone(zones, id);
  if (zone === null) return open;

  if (open[zone] === id) {
    // R2：已打开则关闭
    return { ...open, [zone]: null };
  }
  // R1：未打开则打开（直接覆盖，隐式关闭同区旧视图）
  return { ...open, [zone]: id };
}

/**
 * 移动按钮归属（R6/R7/R8）
 * - 源区移除 id、按 clamp 后 index 插入目标区
 * - open[源区] === id → open[源区] = null 且 open[目标区] = id（R6 跟随+替换）
 * - 未打开 → open 不变（R7）
 * - 同区移动只调顺序不动 open（R8）
 * - 不校验 id 是否已注册（注册表职责分离）
 */
export function moveButtonPure(
  zones: Zones,
  open: OpenState,
  id: string,
  targetZone: Zone,
  index: number
): { zones: Zones; open: OpenState } {
  const sourceZone = findZone(zones, id);
  if (sourceZone === null) return { zones, open };

  // 新 zones：源区移除
  const newZones: Zones = {
    top: [...zones.top],
    bottom: [...zones.bottom],
  };
  const srcIdx = newZones[sourceZone].indexOf(id);
  newZones[sourceZone].splice(srcIdx, 1);

  // 目标区插入（index clamp 到 [0, len]）
  const targetLen = newZones[targetZone].length;
  const clampedIndex = clamp(index, 0, targetLen);
  newZones[targetZone].splice(clampedIndex, 0, id);

  // open 处理
  let newOpen = open;
  const isOpen = open[sourceZone] === id;

  if (sourceZone === targetZone) {
    // R8：同区移动只调顺序，不动 open
    return { zones: newZones, open: newOpen };
  }

  if (isOpen) {
    // R6：跨区 + 视图打开中 → 跟随到目标区，替换目标区旧视图
    newOpen = {
      ...open,
      [sourceZone]: null,
      [targetZone]: id,
    };
  } else {
    // R7：跨区 + 未打开 → 仅归属变化，展示不变
    // newOpen 保持原样（但因跨区后 id 已不在 sourceZone，若 open[sourceZone]===id
    // 不会成立，所以对 R7 而言 open 在结构上不依赖原 zones，无需修改）
  }

  return { zones: newZones, open: newOpen };
}

/**
 * 从打开状态推导侧栏区展示形态（R3/R4/R5）
 * - 双空 → hidden
 * - 仅上 → single-top
 * - 仅下 → single-bottom
 * - 双开 → split
 */
export function deriveLayout(open: OpenState): LayoutKind {
  const topOpen = open.top !== null;
  const bottomOpen = open.bottom !== null;

  if (!topOpen && !bottomOpen) return "hidden";
  if (topOpen && !bottomOpen) return "single-top";
  if (!topOpen && bottomOpen) return "single-bottom";
  return "split";
}

/**
 * 持久化恢复后对齐注册表（R9）
 * - 两数组过滤 registeredIds，保留 saved 顺序
 * - 缺失的注册 id 按 registeredIds 顺序追加上区末尾（R9）
 * - open 逐项校验：值非 null 且不在 reconcile 后 zones 中 → null
 */
export function reconcileZones(
  saved: Zones,
  open: OpenState,
  registeredIds: string[]
): { zones: Zones; open: OpenState } {
  const registeredSet = new Set(registeredIds);

  // 过滤未注册 id，保留 saved 顺序
  const top = saved.top.filter((id) => registeredSet.has(id));
  const bottom = saved.bottom.filter((id) => registeredSet.has(id));

  const existingIds = new Set([...top, ...bottom]);

  // 缺失的注册 id 按 registeredIds 顺序追加上区末尾（R9）
  for (const id of registeredIds) {
    if (!existingIds.has(id)) {
      top.push(id);
    }
  }

  const reconciledZones: Zones = { top, bottom };

  // open 校验：值非 null 但不在 reconcile 后 zones 中 → null
  const reconciledOpen: OpenState = { ...open };
  const allZoneIds = new Set([...top, ...bottom]);

  if (reconciledOpen.top !== null && !allZoneIds.has(reconciledOpen.top)) {
    reconciledOpen.top = null;
  }
  if (
    reconciledOpen.bottom !== null &&
    !allZoneIds.has(reconciledOpen.bottom)
  ) {
    reconciledOpen.bottom = null;
  }

  return { zones: reconciledZones, open: reconciledOpen };
}

/** 原始值是否为非空对象 */
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 原始值是否为 string[]，空数组也通过 */
function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((v) => typeof v === "string");
}

/** 原始值是否为 string 或 null */
function isStringOrNull(value: unknown): value is string | null {
  return typeof value === "string" || value === null;
}

/**
 * 合法化持久化数据——非法输入回退默认值 + clamp
 * 返回完整 SideBarSlice（无 partial）
 */
export function sanitizeSideBar(raw: unknown): SideBarSlice {
  if (!isObject(raw)) {
    return {
      zones: { ...DEFAULT_ZONES },
      open: { ...DEFAULT_OPEN },
      width: WIDTH_DEFAULT,
      splitRatio: SPLIT_DEFAULT,
    };
  }

  // zones
  let zones: Zones;
  const rawZones = raw["zones"];
  if (isObject(rawZones) && isStringArray(rawZones["top"]) && isStringArray(rawZones["bottom"])) {
    zones = {
      top: rawZones["top"] as string[],
      bottom: rawZones["bottom"] as string[],
    };
  } else {
    zones = { ...DEFAULT_ZONES };
  }

  // open
  let open: OpenState;
  const rawOpen = raw["open"];
  if (
    isObject(rawOpen) &&
    isStringOrNull(rawOpen["top"]) &&
    isStringOrNull(rawOpen["bottom"])
  ) {
    open = {
      top: rawOpen["top"] as string | null,
      bottom: rawOpen["bottom"] as string | null,
    };
  } else {
    // open 结构非法时回退默认（不是置 null）
    open = { ...DEFAULT_OPEN };
  }

  // width
  const rawWidth = raw["width"];
  const width =
    typeof rawWidth === "number"
      ? clamp(rawWidth, WIDTH_MIN, WIDTH_MAX)
      : WIDTH_DEFAULT;

  // splitRatio
  const rawSplit = raw["splitRatio"];
  const splitRatio =
    typeof rawSplit === "number"
      ? clamp(rawSplit, SPLIT_MIN, SPLIT_MAX)
      : SPLIT_DEFAULT;

  return { zones, open, width, splitRatio };
}
