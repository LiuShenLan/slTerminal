// sideBarState 纯函数 + dropTarget 单元测试（~30 用例，无 mock）
import { describe, it, expect } from "vitest";
import {
  toggleViewPure,
  moveButtonPure,
  deriveLayout,
  reconcileZones,
  sanitizeSideBar,
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  WIDTH_DEFAULT,
  WIDTH_MIN,
  WIDTH_MAX,
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
  ACTIVITY_BAR_SIZE,
} from "../features/sideViews/sideBarState";
import type { Zones, OpenState } from "../features/sideViews/sideBarState";
import { computeDropTarget } from "../features/sideViews/dropTarget";
import type { ButtonRect } from "../features/sideViews/dropTarget";

// ── 辅助：构造两个视图均在上区的 zones ──
function zonesBothTop(): Zones {
  return { top: ["projects", "explorer"], bottom: [] };
}

// ── toggleViewPure ──

describe("toggleViewPure", () => {
  it("已关闭的视图点击后打开（R1）", () => {
    const zones = zonesBothTop();
    const open: OpenState = { top: null, bottom: null };
    const result = toggleViewPure(zones, open, "projects");
    expect(result).toEqual({ top: "projects", bottom: null });
  });

  it("已打开的视图点击后关闭（R2）", () => {
    const zones = zonesBothTop();
    const open: OpenState = { top: "projects", bottom: null };
    const result = toggleViewPure(zones, open, "projects");
    expect(result).toEqual({ top: null, bottom: null });
  });

  it("打开新视图时隐式关闭同区旧视图（R1 覆盖）", () => {
    const zones = zonesBothTop();
    const open: OpenState = { top: "projects", bottom: null };
    const result = toggleViewPure(zones, open, "explorer");
    // explorer 代替 projects，projects 不出现在返回中（无历史记忆）
    expect(result).toEqual({ top: "explorer", bottom: null });
  });

  it("id 不在 zones 任一区中则原样返回（防御）", () => {
    const zones = zonesBothTop();
    const open: OpenState = { top: "projects", bottom: null };
    const result = toggleViewPure(zones, open, "nonexistent");
    expect(result).toBe(open);
  });

  it("下区视图的开关独立于上区", () => {
    const zones: Zones = { top: ["projects"], bottom: ["explorer"] };
    const open: OpenState = { top: "projects", bottom: null };
    const result = toggleViewPure(zones, open, "explorer");
    expect(result).toEqual({ top: "projects", bottom: "explorer" });
  });
});

// ── moveButtonPure ──

describe("moveButtonPure", () => {
  it("跨区 + 视图打开中：跟随并替换目标区旧视图（R6）", () => {
    // P 在上区打开，E 在下区打开
    const zones: Zones = { top: ["projects"], bottom: ["explorer"] };
    const open: OpenState = { top: "projects", bottom: "explorer" };
    // 拖拽 projects 到下区 index 0
    const result = moveButtonPure(zones, open, "projects", "bottom", 0);
    expect(result.zones).toEqual({ top: [], bottom: ["projects", "explorer"] });
    // projects 跟随到下区，explorer 被关闭
    expect(result.open).toEqual({ top: null, bottom: "projects" });
  });

  it("跨区 + 视图打开中 + 目标区空：跟随到空区", () => {
    const zones: Zones = { top: ["projects"], bottom: [] };
    const open: OpenState = { top: "projects", bottom: null };
    const result = moveButtonPure(zones, open, "projects", "bottom", 0);
    expect(result.zones).toEqual({ top: [], bottom: ["projects"] });
    expect(result.open).toEqual({ top: null, bottom: "projects" });
  });

  it("跨区 + 视图未打开：仅归属变化，展示不变（R7）", () => {
    const zones: Zones = { top: ["projects", "explorer"], bottom: [] };
    const open: OpenState = { top: null, bottom: null };
    const result = moveButtonPure(zones, open, "explorer", "bottom", 0);
    expect(result.zones).toEqual({ top: ["projects"], bottom: ["explorer"] });
    expect(result.open).toEqual({ top: null, bottom: null });
  });

  it("同区内向前移动（更小 index）：仅调顺序不动 open", () => {
    const zones: Zones = {
      top: ["projects", "explorer", "search"],
      bottom: [],
    };
    const open: OpenState = { top: "explorer", bottom: null };
    // 将 explorer 从 index=1 移到 index=0
    const result = moveButtonPure(zones, open, "explorer", "top", 0);
    expect(result.zones).toEqual({
      top: ["explorer", "projects", "search"],
      bottom: [],
    });
    expect(result.open).toEqual(open); // open 不变
  });

  it("同区内向后移动（更大 index）：仅调顺序不动 open", () => {
    const zones: Zones = {
      top: ["projects", "explorer", "search"],
      bottom: [],
    };
    const open: OpenState = { top: "explorer", bottom: null };
    // 将 explorer 从 index=1 移到 index=2（search 之后）
    const result = moveButtonPure(zones, open, "explorer", "top", 3);
    expect(result.zones).toEqual({
      top: ["projects", "search", "explorer"],
      bottom: [],
    });
    expect(result.open).toEqual(open);
  });

  it("同区移至同位：顺序不变，open 不变", () => {
    const zones: Zones = { top: ["projects", "explorer"], bottom: [] };
    const open: OpenState = { top: "projects", bottom: null };
    // 移到自己所在位置
    const result = moveButtonPure(zones, open, "projects", "top", 0);
    expect(result.zones).toEqual(zones);
    expect(result.open).toEqual(open);
  });

  it("index clamp：负数 clamp 到 0", () => {
    const zones: Zones = { top: ["projects", "explorer"], bottom: [] };
    const open: OpenState = { top: null, bottom: null };
    const result = moveButtonPure(zones, open, "explorer", "top", -5);
    expect(result.zones).toEqual({
      top: ["explorer", "projects"],
      bottom: [],
    });
  });

  it("index clamp：超限 clamp 到数组长度", () => {
    const zones: Zones = { top: ["projects", "explorer"], bottom: [] };
    const open: OpenState = { top: null, bottom: null };
    const result = moveButtonPure(zones, open, "projects", "top", 999);
    expect(result.zones).toEqual({
      top: ["explorer", "projects"],
      bottom: [],
    });
  });

  it("id 不在 zones 中：原样返回", () => {
    const zones = zonesBothTop();
    const open: OpenState = { top: "projects", bottom: null };
    const result = moveButtonPure(zones, open, "nonexistent", "bottom", 0);
    expect(result.zones).toBe(zones);
    expect(result.open).toBe(open);
  });
});

// ── deriveLayout ──

describe("deriveLayout", () => {
  it("双空 → hidden（R3）", () => {
    expect(deriveLayout({ top: null, bottom: null })).toBe("hidden");
  });

  it("仅上区打开 → single-top（R4）", () => {
    expect(deriveLayout({ top: "projects", bottom: null })).toBe("single-top");
  });

  it("仅下区打开 → single-bottom（R4）", () => {
    expect(deriveLayout({ top: null, bottom: "explorer" })).toBe(
      "single-bottom"
    );
  });

  it("双开 → split（R5）", () => {
    expect(deriveLayout({ top: "projects", bottom: "explorer" })).toBe("split");
  });
});

// ── reconcileZones ──

describe("reconcileZones", () => {
  const registeredIds = ["projects", "explorer", "search"];

  it("过滤 saved 中已注销的 id，保留 saved 顺序", () => {
    const saved: Zones = {
      top: ["projects", "obsolete", "explorer"],
      bottom: [],
    };
    const open: OpenState = { top: "projects", bottom: null };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.zones).toEqual({
      top: ["projects", "explorer", "search"],
      bottom: [],
    });
    // open 仍指向 projects（projects 在 reconcile 后 zones 中）
    expect(result.open).toEqual({ top: "projects", bottom: null });
  });

  it("缺失的注册 id 追加上区末尾（R9）", () => {
    const saved: Zones = { top: ["projects"], bottom: [] };
    const open: OpenState = { top: "projects", bottom: null };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.zones).toEqual({
      top: ["projects", "explorer", "search"],
      bottom: [],
    });
  });

  it("已注册 id 全部在 zones 中时不追加", () => {
    const saved: Zones = {
      top: ["projects", "explorer", "search"],
      bottom: [],
    };
    const open: OpenState = { top: "projects", bottom: null };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.zones).toEqual({
      top: ["projects", "explorer", "search"],
      bottom: [],
    });
  });

  it("open.top 指向已过滤 id → 置 null", () => {
    const saved: Zones = { top: ["obsolete"], bottom: [] };
    const open: OpenState = { top: "obsolete", bottom: null };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.open.top).toBeNull();
  });

  it("open.bottom 指向已过滤 id → 置 null", () => {
    const saved: Zones = { top: [], bottom: ["obsolete"] };
    const open: OpenState = { top: null, bottom: "obsolete" };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.open.bottom).toBeNull();
  });

  it("open 为 null 保持不变（不受 reconcile 影响）", () => {
    const saved: Zones = { top: ["projects"], bottom: [] };
    const open: OpenState = { top: null, bottom: null };
    const result = reconcileZones(saved, open, registeredIds);
    expect(result.open).toEqual({ top: null, bottom: null });
  });
});

// ── sanitizeSideBar ──

describe("sanitizeSideBar", () => {
  it("raw 非对象（string）→ 全默认", () => {
    const result = sanitizeSideBar("garbage");
    expect(result.zones).toEqual(DEFAULT_ZONES);
    expect(result.open).toEqual(DEFAULT_OPEN);
    expect(result.width).toBe(WIDTH_DEFAULT);
    expect(result.splitRatio).toBe(SPLIT_DEFAULT);
  });

  it("raw 非对象（null）→ 全默认", () => {
    const result = sanitizeSideBar(null);
    expect(result.zones).toEqual(DEFAULT_ZONES);
    expect(result.open).toEqual(DEFAULT_OPEN);
  });

  it("zones 非法（数组而非对象）→ 回退默认 zones", () => {
    const raw = { zones: [1, 2, 3], open: DEFAULT_OPEN, width: 200, splitRatio: 0.3 };
    const result = sanitizeSideBar(raw);
    expect(result.zones).toEqual(DEFAULT_ZONES);
    // 其他字段保留（clamp）
    expect(result.width).toBe(200);
    expect(result.splitRatio).toBe(0.3);
  });

  it("open.top 非 string|null → 回退默认 open", () => {
    const raw = { zones: DEFAULT_ZONES, open: { top: 42, bottom: null }, width: 200, splitRatio: 0.3 };
    const result = sanitizeSideBar(raw);
    // open 结构非法 → 整体回退默认
    expect(result.open).toEqual(DEFAULT_OPEN);
  });

  it("width 非 number → 回退默认，splitRatio 超限 → clamp", () => {
    const raw = { zones: DEFAULT_ZONES, open: DEFAULT_OPEN, width: "nope", splitRatio: 2.5 };
    const result = sanitizeSideBar(raw);
    expect(result.width).toBe(WIDTH_DEFAULT);
    expect(result.splitRatio).toBe(SPLIT_MAX);
  });

  it("width/splitRatio 在合法范围内原样返回", () => {
    const raw = { zones: DEFAULT_ZONES, open: DEFAULT_OPEN, width: 300, splitRatio: 0.4 };
    const result = sanitizeSideBar(raw);
    expect(result.width).toBe(300);
    expect(result.splitRatio).toBe(0.4);
  });

  it("width 低于下限 → clamp", () => {
    const raw = { zones: DEFAULT_ZONES, open: DEFAULT_OPEN, width: 50, splitRatio: 0.5 };
    const result = sanitizeSideBar(raw);
    expect(result.width).toBe(WIDTH_MIN);
  });

  it("splitRatio 低于下限 → clamp", () => {
    const raw = { zones: DEFAULT_ZONES, open: DEFAULT_OPEN, width: 250, splitRatio: -0.3 };
    const result = sanitizeSideBar(raw);
    expect(result.splitRatio).toBe(SPLIT_MIN);
  });

  it("返回完整 SideBarSlice（含所有字段）", () => {
    const result = sanitizeSideBar({});
    expect(result).toHaveProperty("zones");
    expect(result).toHaveProperty("open");
    expect(result).toHaveProperty("width");
    expect(result).toHaveProperty("splitRatio");
  });
});

// ── computeDropTarget ──

describe("computeDropTarget", () => {
  const buttons: ButtonRect[] = [
    { id: "projects", top: 0, height: 40 },
    { id: "explorer", top: 40, height: 40 },
    { id: "search", top: 80, height: 40 },
  ];

  it("clientY 在第一个按钮上半 → index 0", () => {
    const result = computeDropTarget(10, buttons, "top");
    expect(result).toEqual({ zone: "top", index: 0 });
  });

  it("clientY 在第一个按钮下半 → index 1", () => {
    const result = computeDropTarget(30, buttons, "top");
    expect(result).toEqual({ zone: "top", index: 1 });
  });

  it("clientY 在中间按钮上半 → 该按钮 index", () => {
    const result = computeDropTarget(50, buttons, "top");
    expect(result).toEqual({ zone: "top", index: 1 });
  });

  it("clientY 在所有按钮之下 → 数组末尾", () => {
    const result = computeDropTarget(200, buttons, "bottom");
    expect(result).toEqual({ zone: "bottom", index: 3 });
  });

  it("空白区（空按钮数组）→ index 0", () => {
    const result = computeDropTarget(100, [], "top");
    expect(result).toEqual({ zone: "top", index: 0 });
  });

  it("精确在按钮中间线 → 视为下半（>= mid）", () => {
    const result = computeDropTarget(20, buttons, "top");
    // 第一个按钮 top=0, height=40, midY=20
    // clientY=20 >= 20 → 进入下半 → 看下一个按钮
    // 第二个按钮 top=40, midY=60, clientY=20 < 60 → index=1
    expect(result).toEqual({ zone: "top", index: 1 });
  });
});

// ── 场景序列 S1–S6（直译 docs/sidebar-requirements.md §5）──

describe("场景序列（S1–S6）", () => {
  // 初始：P、E 均在上区，均关闭
  const initialZones: Zones = { top: ["projects", "explorer"], bottom: [] };
  const initialOpen: OpenState = { top: null, bottom: null };

  it("S1 基础开关——P 开→E 替换→E 关→侧栏隐藏", () => {
    let open = initialOpen;

    // 1. 点击 P → 全高展示项目列表
    open = toggleViewPure(initialZones, open, "projects");
    expect(open).toEqual({ top: "projects", bottom: null });
    expect(deriveLayout(open)).toBe("single-top");

    // 2. 点击 E → E 打开，P 被隐式关闭，全高展示文件浏览器（R1）
    open = toggleViewPure(initialZones, open, "explorer");
    expect(open).toEqual({ top: "explorer", bottom: null });
    expect(deriveLayout(open)).toBe("single-top");
    // 验证无历史记忆：projects 不出现在返回中
    expect(open.top).not.toBe("projects");

    // 3. 再次点击 E → E 关闭，两半区均空，侧栏区整体隐藏（R2+R3）
    open = toggleViewPure(initialZones, open, "explorer");
    expect(open).toEqual({ top: null, bottom: null });
    expect(deriveLayout(open)).toBe("hidden");
  });

  it("S2 双视图分区——上 P 下 E：全高→分区→恢复全高", () => {
    const zones: Zones = { top: ["projects"], bottom: ["explorer"] };
    let open: OpenState = { top: null, bottom: null };

    // 1. 点击 P → 全高展示项目列表（R4）
    open = toggleViewPure(zones, open, "projects");
    expect(open).toEqual({ top: "projects", bottom: null });
    expect(deriveLayout(open)).toBe("single-top");

    // 2. 点击 E → 上半 P、下半 E（R5）
    open = toggleViewPure(zones, open, "explorer");
    expect(open).toEqual({ top: "projects", bottom: "explorer" });
    expect(deriveLayout(open)).toBe("split");

    // 3. 再次点击 P → P 关闭，全高展示文件浏览器（R4）
    open = toggleViewPure(zones, open, "projects");
    expect(open).toEqual({ top: null, bottom: "explorer" });
    expect(deriveLayout(open)).toBe("single-bottom");
  });

  it("S3 同区替换——P 打开中点击 E，E 替换 P", () => {
    const zones = zonesBothTop();
    let open: OpenState = { top: "projects", bottom: null };

    // 1. 点击 E → E 打开、P 关闭，全高展示文件浏览器（R1）
    open = toggleViewPure(zones, open, "explorer");
    expect(open).toEqual({ top: "explorer", bottom: null });
    expect(deriveLayout(open)).toBe("single-top");

    // 2. 再点击 E → 侧栏区整体隐藏
    open = toggleViewPure(zones, open, "explorer");
    expect(open).toEqual({ top: null, bottom: null });
    expect(deriveLayout(open)).toBe("hidden");
  });

  it("S4 拖拽换区 + 视图跟随——P 从上到下，E 被关闭", () => {
    let zones: Zones = { top: ["projects"], bottom: ["explorer"] };
    let open: OpenState = { top: "projects", bottom: "explorer" };
    expect(deriveLayout(open)).toBe("split");

    // 1. 将 P 从上区拖到下区 → 项目列表立即移到下区展示
    //    下区原有的 E 被关闭（R6）；仅 P 一个视图打开 → 全高（R4）
    const result = moveButtonPure(zones, open, "projects", "bottom", 0);
    zones = result.zones;
    open = result.open;

    expect(zones).toEqual({ top: [], bottom: ["projects", "explorer"] });
    expect(open).toEqual({ top: null, bottom: "projects" });
    expect(deriveLayout(open)).toBe("single-bottom");
  });

  it("S5 拖拽未打开按钮——E 从上到下仅归属变化", () => {
    let zones: Zones = { top: ["projects", "explorer"], bottom: [] };
    let open: OpenState = { top: "projects", bottom: null };
    expect(deriveLayout(open)).toBe("single-top");

    // 1. 将未打开的 E 拖至上区（已在上区，改为拖至下区测试 R7）
    const result = moveButtonPure(zones, open, "explorer", "bottom", 0);
    zones = result.zones;
    open = result.open;

    // 仅归属变化，展示不变（R7）
    expect(zones).toEqual({ top: ["projects"], bottom: ["explorer"] });
    expect(open).toEqual({ top: "projects", bottom: null });
    expect(deriveLayout(open)).toBe("single-top");

    // 2. 点击 E → E 在上区打开？不——E 现在在下区
    open = toggleViewPure(zones, open, "explorer");
    expect(open).toEqual({ top: "projects", bottom: "explorer" });
    expect(deriveLayout(open)).toBe("split");
  });

  it("S6 半区内排序——拖拽改变顺序，开关状态不受影响", () => {
    const zones: Zones = { top: ["a", "b", "c"], bottom: [] };
    const open: OpenState = { top: "a", bottom: null };

    // 1. 拖拽 c 到 a 前方 → 顺序变为 c, a, b
    const result = moveButtonPure(zones, open, "c", "top", 0);
    expect(result.zones).toEqual({ top: ["c", "a", "b"], bottom: [] });
    // 各视图开关状态不受影响（a 仍打开）
    expect(result.open).toEqual({ top: "a", bottom: null });
  });
});

// ── 常量验证 ──

describe("常量", () => {
  it("DEFAULT_ZONES 上区含 projects 和 explorer", () => {
    expect(DEFAULT_ZONES).toEqual({
      top: ["projects", "explorer"],
      bottom: [],
    });
  });

  it("DEFAULT_OPEN 默认打开 projects", () => {
    expect(DEFAULT_OPEN).toEqual({ top: "projects", bottom: null });
  });

  it("ACTIVITY_BAR_SIZE = 40", () => {
    expect(ACTIVITY_BAR_SIZE).toBe(40);
  });

  it("宽度常量范围合理", () => {
    expect(WIDTH_MIN).toBeLessThanOrEqual(WIDTH_DEFAULT);
    expect(WIDTH_DEFAULT).toBeLessThanOrEqual(WIDTH_MAX);
  });

  it("分割比例常量范围合理", () => {
    expect(SPLIT_MIN).toBeLessThanOrEqual(SPLIT_DEFAULT);
    expect(SPLIT_DEFAULT).toBeLessThanOrEqual(SPLIT_MAX);
  });
});
