// activityBar.test.tsx —— ActivityBar 组件 L2 测试（40 用例）
//
// 测试模式：真实 sideBar store（beforeEach setState 重置默认）+ sideViewRegistry._reset() 注册 stub
// 拖拽测试用 Object.defineProperty 覆盖 DragEvent.dataTransfer（jsdom 兼容）
// 外层容器统一处理 DnD——zone 边界 = 上区末按钮 bottom 或 container top

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// jsdom 未实现 DragEvent 构造函数——用 MouseEvent 子类 polyfill
if (typeof DragEvent === "undefined") {
  (globalThis as Record<string, unknown>).DragEvent = class extends MouseEvent {
    public dataTransfer: unknown = null;
    constructor(type: string, eventInitDict?: MouseEventInit & { dataTransfer?: unknown }) {
      super(type, eventInitDict);
      this.dataTransfer = eventInitDict?.dataTransfer ?? null;
    }
  };
}

// ─── Hoisted mocks（防 store import 时加载真实 Tauri API） ───
const { mockSaveSettings, mockLoadSettings } = vi.hoisted(() => ({
  mockSaveSettings: vi.fn().mockResolvedValue(undefined),
  // FE-11/D11：wrapper 返回 { data, corrupted }——无文件 = data:null, corrupted:false
  mockLoadSettings: vi.fn().mockResolvedValue({ data: null, corrupted: false }),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

// 配置钮入口 mock（NAV-05：点击 = openHooksConfigFromActivityBar——入口唯一化）
const { mockOpenHooksConfigFromActivityBar } = vi.hoisted(() => ({
  mockOpenHooksConfigFromActivityBar: vi.fn(() => Promise.resolve()),
}));
vi.mock("../features/hooksConfig/openHooksConfig", () => ({
  openHooksConfigFromActivityBar: mockOpenHooksConfigFromActivityBar,
}));

import { render, fireEvent, act } from "@testing-library/react";
import { ActivityBar } from "../features/sideViews/ActivityBar";
import * as dropTargetModule from "../features/sideViews/dropTarget";
import { useSideBar } from "../stores/sideBar";
import { sideViewRegistry } from "../features/sideViews/sideViewRegistry";
import {
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  type Zone,
} from "../features/sideViews/sideBarState";
import {
  SIDEBAR_COLORS,
  SIDEBAR_FG,
  DIM_FG,
  ACCENT_FG,
  FOCUS_BORDER,
  ACTIVE_SELECTION_BG,
} from "../theme/colors";
import { ACTIVITY_BAR_SIZE } from "../features/sideViews/sideBarState";

// ── 测试辅助 ──

/** 测试用 stub 图标组件（IC-06：icon 字段为组件形态） */
function StubIcon(): null {
  return null;
}

function registerTestViews(): void {
  sideViewRegistry.register({ id: "nav", title: "导航树", icon: StubIcon, component: () => null });
  sideViewRegistry.register({ id: "explorer", title: "文件浏览器", icon: StubIcon, component: () => null });
  sideViewRegistry.register({ id: "search", title: "搜索", icon: StubIcon, component: () => null });
}

function createMockDataTransfer(initial: Record<string, string> = {}) {
  const store = new Map<string, string>(Object.entries(initial));
  return {
    store,
    dropEffect: "none" as string,
    effectAllowed: "none" as string,
    setData(format: string, value: string) { store.set(format, value); },
    getData(format: string) { return store.get(format) || ""; },
  };
}

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 输入补空格 → "rgba(r, g, b, a)"） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  // 新方案 selection 类 token 为 rgba 形态，jsdom 输出 "rgba(r, g, b, a)"
  const m = hex.match(/^rgba\((\d+),(\d+),(\d+),([0-9.]+)\)$/);
  if (m) return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${m[4]})`;
  return hex;
}

function dispatchDragEvent(
  element: Element,
  type: string,
  dt: ReturnType<typeof createMockDataTransfer>,
  clientY?: number,
): void {
  act(() => {
    const event = new DragEvent(type, { bubbles: true, cancelable: true });
    Object.defineProperty(event, "dataTransfer", { value: dt, configurable: true });
    if (clientY !== undefined) {
      Object.defineProperty(event, "clientY", { value: clientY, configurable: true });
    }
    element.dispatchEvent(event);
  });
}

function getButton(id: string): Element {
  const el = document.querySelector(`[data-e2e="activity-btn-${id}"]`);
  if (!el) throw new Error(`Button not found: activity-btn-${id}`);
  return el;
}

function getActivityBar(): Element {
  const el = document.querySelector('[data-e2e="activity-bar"]');
  if (!el) throw new Error("ActivityBar root not found");
  return el;
}

function getZoneContainer(zone: Zone): Element {
  const el = document.querySelector(`[data-zone="${zone}"]`);
  if (!el) throw new Error(`Zone container not found: ${zone}`);
  return el;
}

/** 安装 getBoundingClientRect spy——按钮 + bottom zone 统一返回模拟矩形 */
function installRectSpy(
  buttonRects: Record<string, { top: number; height: number }>,
  bottomZoneTop: number = 800,
) {
  return vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function (this: HTMLElement) {
      // 活动栏根容器——高 600 供空上区时中点判定（midpoint=300）
      if (this.getAttribute("data-e2e") === "activity-bar") {
        return { top: 0, bottom: 600, height: 600, left: 0, right: 40, width: 40, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
      }
      if (this.getAttribute("data-zone") === "bottom") {
        return { top: bottomZoneTop, bottom: bottomZoneTop, height: 0, left: 0, right: 40, width: 40, x: 0, y: bottomZoneTop, toJSON: () => ({}) } as DOMRect;
      }
      if (this.getAttribute("data-zone") === "top") {
        return { top: 0, bottom: 0, height: 0, left: 0, right: 40, width: 40, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
      }
      const id = this.getAttribute("data-view-id");
      if (id && buttonRects[id]) {
        const r = buttonRects[id];
        return { top: r.top, bottom: r.top + r.height, height: r.height, left: 0, right: 40, width: 40, x: 0, y: r.top, toJSON: () => ({}) } as DOMRect;
      }
      return { top: 0, bottom: 0, height: 0, left: 0, right: 0, width: 0, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
    });
}

// ── 测试套件 ──

describe("ActivityBar", () => {
  beforeEach(() => {
    sideViewRegistry._reset();
    registerTestViews();
    useSideBar.setState({
      zones: { top: [...DEFAULT_ZONES.top], bottom: [...DEFAULT_ZONES.bottom] },
      open: { ...DEFAULT_OPEN },
      width: 250, splitRatio: 0.5, loaded: false,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ═══ 渲染结构 ═══

  it("SB-19.1 渲染上区/下区两按钮组，按 zones 顺序排列", () => {
    render(<ActivityBar />);
    const topBtns = getZoneContainer("top").querySelectorAll("[data-view-id]");
    expect(topBtns).toHaveLength(2);
    expect(topBtns[0].getAttribute("data-view-id")).toBe("nav");
    expect(topBtns[1].getAttribute("data-view-id")).toBe("explorer");
    const bottomBtns = getZoneContainer("bottom").querySelectorAll("[data-view-id]");
    expect(bottomBtns).toHaveLength(0);
  });

  it("SB-19.1b 外层容器有 data-e2e 属性", () => {
    render(<ActivityBar />);
    expect(getActivityBar().getAttribute("data-e2e")).toBe("activity-bar");
  });

  it("SB-19.2 种子下区按钮后正确渲染", () => {
    useSideBar.setState({ zones: { top: ["nav"], bottom: ["explorer", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    expect(getZoneContainer("top").querySelectorAll("[data-view-id]")).toHaveLength(1);
    expect(getZoneContainer("bottom").querySelectorAll("[data-view-id]")).toHaveLength(2);
  });

  // ═══ active 态 ═══

  it("SB-19.3 active 按钮高亮背景（ACCEPT_SELECTION_BG accent-dim）+ 左侧指示条", () => {
    render(<ActivityBar />);
    const p = getButton("nav") as HTMLElement;
    const e = getButton("explorer") as HTMLElement;
    expect(p.style.backgroundColor).toBe(hexToRgb(ACTIVE_SELECTION_BG));
    expect(p.style.borderLeftColor).toBe(hexToRgb(FOCUS_BORDER));
    expect(e.style.borderLeftColor).toBe("transparent");
  });

  it("SB-19.4 点击后 active 态切换", () => {
    render(<ActivityBar />);
    fireEvent.click(getButton("explorer"));
    expect(useSideBar.getState().open.top).toBe("explorer");
    const p = getButton("nav") as HTMLElement;
    const e = getButton("explorer") as HTMLElement;
    expect(p.style.backgroundColor).not.toBe(hexToRgb(ACTIVE_SELECTION_BG));
    expect(e.style.backgroundColor).toBe(hexToRgb(ACTIVE_SELECTION_BG));
  });

  // ═══ 点击 → toggleView ═══

  it("SB-19.5 点击按钮调用 toggleView", () => {
    render(<ActivityBar />);
    const spy = vi.spyOn(useSideBar.getState(), "toggleView");
    fireEvent.click(getButton("explorer"));
    expect(spy).toHaveBeenCalledWith("explorer");
    spy.mockRestore();
  });

  it("SB-19.6 再次点击同一按钮关闭视图", () => {
    render(<ActivityBar />);
    const spy = vi.spyOn(useSideBar.getState(), "toggleView");
    fireEvent.click(getButton("nav"));
    expect(spy).toHaveBeenCalledWith("nav");
    expect(useSideBar.getState().open.top).toBeNull();
    spy.mockRestore();
  });

  // ═══ title / data-e2e ═══

  it("SB-19.7 按钮有 title 和 data-e2e 属性", () => {
    render(<ActivityBar />);
    expect(getButton("nav").getAttribute("title")).toBe("导航树");
    expect(getButton("explorer").getAttribute("title")).toBe("文件浏览器");
    expect(getButton("nav").getAttribute("data-e2e")).toBe("activity-btn-nav");
  });

  // ═══ 拖拽：dragStart（按钮级，不变） ═══

  it("SB-19.8 dragStart 设置 dataTransfer 内容 + effectAllowed", () => {
    render(<ActivityBar />);
    const dt = createMockDataTransfer();
    dispatchDragEvent(getButton("explorer"), "dragstart", dt);
    expect(dt.store.get("application/x-side-view-id")).toBe("explorer");
    expect(dt.effectAllowed).toBe("move");
  });

  it("SB-19.9 dragStart 后按钮半透明", () => {
    render(<ActivityBar />);
    const dt = createMockDataTransfer();
    const btn = getButton("explorer") as HTMLElement;
    dispatchDragEvent(btn, "dragstart", dt);
    expect(btn.style.opacity).toBe("0.5");
  });

  // ═══ 拖拽：dragOver → 外层容器 ═══

  it("SB-19.10 dragOver 调用 preventDefault 并设置 dropEffect（向外层容器派发）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    dispatchDragEvent(getActivityBar(), "dragover", dt, 25);
    expect(dt.dropEffect).toBe("move");
    spy.mockRestore();
  });

  it("SB-19.11 dragOver 更新 dropIndicator——指示线渲染在落点按钮前方（向外层容器派发）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const bar = getActivityBar();
    const dt = createMockDataTransfer();
    expect(() => dispatchDragEvent(bar, "dragover", dt, 25)).not.toThrow();
    // 落点 25 < nav mid=30 → computeDropTarget index 0 → 指示线渲染于 nav 前方
    // （dropIndicator DOM 实断言：top zone 内仅此一条 height 2px 指示线）
    const topZone = getZoneContainer("top");
    const indicators = Array.from(topZone.querySelectorAll("div")).filter(
      (d) => (d as HTMLElement).style.height === "2px",
    );
    expect(indicators).toHaveLength(1);
    // 指示线位于按钮 wrapper 内 button 之前——后邻元素即 nav 按钮
    expect(
      (indicators[0].nextElementSibling as HTMLElement | null)?.getAttribute("data-view-id"),
    ).toBe("nav");
    spy.mockRestore();
  });

  // ═══ 拖拽：drop → 外层容器 ═══

  it("SB-19.12 drop 调用 moveButton——同 zone（向外层容器派发）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 25);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(0); // 落点 25 < projects mid=30 → 插 projects 前方（index 0）
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 拖拽：dragEnd（按钮级） ═══

  it("SB-19.13 dragEnd 清除拖拽状态（opacity 恢复）", () => {
    render(<ActivityBar />);
    const dt = createMockDataTransfer();
    const btn = getButton("explorer") as HTMLElement;
    dispatchDragEvent(btn, "dragstart", dt);
    expect(btn.style.opacity).toBe("0.5");
    dispatchDragEvent(btn, "dragend", createMockDataTransfer());
    expect(btn.style.opacity).toBe("1");
  });

  // ═══ 未注册 id 防御 ═══

  it("SB-19.14 zones 含未注册 id 时正常渲染不抛异常", () => {
    useSideBar.setState({ zones: { top: ["nav", "unknown-view", "explorer"], bottom: [] } });
    render(<ActivityBar />);
    expect(getZoneContainer("top").querySelectorAll("[data-view-id]")).toHaveLength(2);
  });

  // ═══ hover 反馈 ═══

  it("SB-19.15 hover 态显示 hover 背景色，离开恢复", () => {
    render(<ActivityBar />);
    const btn = getButton("explorer") as HTMLElement;
    expect(btn.style.backgroundColor).toBe("transparent");
    fireEvent.mouseEnter(btn);
    expect(btn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.hover));
    fireEvent.mouseLeave(btn);
    expect(btn.style.backgroundColor).toBe("transparent");
  });

  it("SB-19.16 active 按钮 hover 时不覆盖 active 背景色", () => {
    render(<ActivityBar />);
    const btn = getButton("nav") as HTMLElement;
    expect(btn.style.backgroundColor).toBe(hexToRgb(ACTIVE_SELECTION_BG));
    fireEvent.mouseEnter(btn);
    expect(btn.style.backgroundColor).toBe(hexToRgb(ACTIVE_SELECTION_BG));
  });

  // ═══ 新增：图标色三级（IC-06：默认 fg-3 / hover fg-1 / active accentFg） ═══

  it("SB-19.33 图标色三级：默认 DIM_FG（fg-3）、hover SIDEBAR_FG（fg-1）、active ACCENT_FG", () => {
    render(<ActivityBar />);
    // 默认种子：projects active（DEFAULT_OPEN）、explorer 非 active
    const p = getButton("nav") as HTMLElement;
    const e = getButton("explorer") as HTMLElement;
    expect(p.style.color).toBe(hexToRgb(ACCENT_FG));
    expect(e.style.color).toBe(hexToRgb(DIM_FG));
    // hover 非 active 按钮 → fg-1；离开恢复 fg-3
    fireEvent.mouseEnter(e);
    expect(e.style.color).toBe(hexToRgb(SIDEBAR_FG));
    fireEvent.mouseLeave(e);
    expect(e.style.color).toBe(hexToRgb(DIM_FG));
    // active 按钮 hover 时色不变（仍 accentFg）
    fireEvent.mouseEnter(p);
    expect(p.style.color).toBe(hexToRgb(ACCENT_FG));
  });

  // ═══ 新增：跨区拖拽——状态机行为 ═══

  it("SB-19.17 拖拽 explorer 从 top 到空 bottom → moveButton(targetZone='bottom')", () => {
    render(<ActivityBar />);
    // boundary = max(projects.bottom=50, explorer.bottom=90) = 90
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 100);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=400 > midpoint=300 → zone="bottom"
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(0); // bottom 空 → 末尾 index 0
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.18 拖拽 projects 从 bottom 到空 top → moveButton(targetZone='top')", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["nav", "explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top zone 空 → boundary = container midpoint = 300（空上区回退）；clientY=10 < 300 → "top"
    const spy = installRectSpy({ nav: { top: 90, height: 40 }, explorer: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "nav" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 10);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("nav");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(0); // top 空 → 末尾 index 0
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.19 拖拽 explorer 从 top 到有按钮的 bottom → zone + index 均正确", () => {
    useSideBar.setState({ zones: { top: ["explorer"], bottom: ["nav", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = explorer.bottom = 50; bottomZoneTop=90
    const spy = installRectSpy({ explorer: { top: 10, height: 40 }, nav: { top: 90, height: 40 }, search: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=400 > midpoint=300 → zone="bottom"
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(2); // 落点 400 >= search mid=150 → 末尾 index 2
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.20 同区拖拽排序（top zone）→ zone 不变", () => {
    render(<ActivityBar />);
    // boundary=90, clientY=70 < 90 → zone="top"
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "nav" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 70);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(2); // 70 >= explorer mid=70 → 末尾 index 2
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.21 同区拖拽排序（bottom zone）→ zone='bottom'", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["nav", "explorer", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top 空, midpoint=300; clientY=400 > 300 → zone="bottom"
    const spy = installRectSpy({ nav: { top: 90, height: 40 }, explorer: { top: 130, height: 40 }, search: { top: 170, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "search" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(3); // 400 >= search mid=190 → 末尾 index 3
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 新增：zone 判定边界 ═══

  it("SB-19.22 clientY 在上区按钮区域内 → zone='top'", () => {
    useSideBar.setState({ zones: { top: ["nav"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = projects.bottom = 50
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=30 < boundary=50 → zone="top"
    dispatchDragEvent(bar, "dragover", dt, 30);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(1); // 30 >= projects mid=30 → 插 projects 后（index 1）
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.23 clientY >= boundary → zone='bottom'", () => {
    useSideBar.setState({ zones: { top: ["nav"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = projects.bottom = 50
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "nav" });
    const bar = getActivityBar();
    // clientY=50 >= midpoint? No——350 > 300 → "bottom"
    dispatchDragEvent(bar, "dragover", dt, 350);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(1); // 350 >= explorer mid=110 → 末尾 index 1
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.24 bottom zone 有按钮、拖到两按钮之间 → index 正确", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["nav", "explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top 空 → boundary ~0; clientY=110 > 0 → "bottom"
    // projects mid=110, explorer mid=150; clientY=110 >=110 → index after projects
    const spy = installRectSpy({ nav: { top: 90, height: 40 }, explorer: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "search" });
    const bar = getActivityBar();
    // top 空, midpoint=300; clientY=350 > 300 → "bottom"
    dispatchDragEvent(bar, "dragover", dt, 350);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(2); // 350 >= explorer mid=150 → 末尾 index 2
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 新增：指示线与状态清理 ═══

  it("SB-19.25 dragOver 跨 zone 时 indicator.zone 切换", () => {
    useSideBar.setState({ zones: { top: ["nav"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const dropTargetSpy = vi.spyOn(dropTargetModule, "computeDropTarget");
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    // zone 内指示线计数（height 2px 即 dropIndicator 渲染产物）
    const zoneIndicators = (zone: Zone) =>
      Array.from(getZoneContainer(zone).querySelectorAll("div")).filter(
        (d) => (d as HTMLElement).style.height === "2px",
      ).length;
    dispatchDragEvent(bar, "dragover", dt, 20); // → "top" (< 300)
    expect(zoneIndicators("top")).toBe(1); // nav 前方指示线
    expect(zoneIndicators("bottom")).toBe(0);
    expect(() => dispatchDragEvent(bar, "dragover", dt, 400)).not.toThrow(); // → "bottom" (> 300)
    // 两次 dragover → computeDropTarget 各调用一次，targetZone 参数随落点切换
    expect(dropTargetSpy).toHaveBeenCalledTimes(2);
    expect(dropTargetSpy.mock.calls[0][2]).toBe("top");
    expect(dropTargetSpy.mock.calls[1][2]).toBe("bottom");
    // 指示线整体移入 bottom zone（explorer 末尾，index 1 >= defs.length 1）
    expect(zoneIndicators("top")).toBe(0);
    expect(zoneIndicators("bottom")).toBe(1);
    dropTargetSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.26 onDragLeave 离开活动栏 → dropIndicator 清 null（指示线样式移除）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    // 指示线渲染产物——height 2px 的 div（与 SB-19.39 计数口径一致）
    const indicatorCount = () =>
      Array.from(bar.querySelectorAll("div")).filter(
        (d) => (d as HTMLElement).style.height === "2px",
      ).length;
    dispatchDragEvent(bar, "dragover", dt, 20);
    expect(indicatorCount()).toBe(1); // dragover 后指示线出现（nav 前方）
    act(() => {
      const ev = new DragEvent("dragleave", { bubbles: true, cancelable: true });
      Object.defineProperty(ev, "dataTransfer", { value: dt, configurable: true });
      Object.defineProperty(ev, "clientY", { value: 20, configurable: true });
      bar.dispatchEvent(ev);
    });
    // 清理后样式实断言：relatedTarget=null → setDropIndicator(null) → 指示线 DOM 移除
    expect(indicatorCount()).toBe(0);
    spy.mockRestore();
  });

  it("SB-19.27 drop 后 opacity 恢复 + dragEnd 幂等清理", () => {
    useSideBar.setState({ zones: { top: ["nav"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "nav" });
    const bar = getActivityBar();
    const btn = getButton("nav") as HTMLElement;
    dispatchDragEvent(btn, "dragstart", dt);
    expect(btn.style.opacity).toBe("0.5");
    // clientY=400 > midpoint=300 → "bottom" (cross-zone)
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(1); // 400 >= explorer mid=110 → 末尾 index 1
    // drop 中 clearDragState → React 重渲染 → re-query
    const after = getButton("nav") as HTMLElement;
    expect(after.style.opacity).toBe("1");
    dispatchDragEvent(after, "dragend", createMockDataTransfer());
    expect(after.style.opacity).toBe("1");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.28 onDragLeave 子元素间转移不触发清理", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 20);
    // 从按钮派发 dragLeave → e.target=按钮 ≠ e.currentTarget=bar → 不清理
    act(() => {
      const ev = new DragEvent("dragleave", { bubbles: true, cancelable: true });
      Object.defineProperty(ev, "dataTransfer", { value: dt, configurable: true });
      getButton("explorer").dispatchEvent(ev);
    });
    // 不抛异常
    spy.mockRestore();
  });

  // ═══ 新增：同按钮上/下半区插入位置差异（SVC-01） ═══

  it("SB-19.29 落点在按钮上半 → moveButton index 为该按钮前方（0）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // projects mid=30，clientY=20 < 30 → 插 projects 前方（index 0）
    dispatchDragEvent(bar, "dragover", dt, 20);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(0);
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.30 落点在按钮下半 → moveButton index 为该按钮后方（1）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // projects mid=30，clientY=40 >= 30 → 插 projects 后；explorer mid=70，40 < 70 → index 1
    dispatchDragEvent(bar, "dragover", dt, 40);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(1);
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 新增：resolveTargetZone 中点边界（SVC-05） ═══

  it("SB-19.31 clientY 恰好等于容器中点（300）→ zone='bottom'（>= 边界）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // boundary = rect.top + rect.height/2 = 0 + 300 = 300；300 >= 300 → "bottom"
    dispatchDragEvent(bar, "dragover", dt, 300);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    expect(moveSpy.mock.calls[0][2]).toBe(0); // bottom 空 → index 0
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.32 clientY 为中点 -1（299）→ zone='top'（< 边界）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // 299 < 300 → "top"
    dispatchDragEvent(bar, "dragover", dt, 299);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    expect(moveSpy.mock.calls[0][2]).toBe(2); // 299 >= explorer mid=70 → 末尾 index 2
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ NAV-05：46px 栏宽 + 34×34 圆角 6 按钮 + 底部配置钮 ═══

  it("SB-19.34 活动栏宽 46px（ACTIVITY_BAR_SIZE 常量驱动）", () => {
    render(<ActivityBar />);
    expect(ACTIVITY_BAR_SIZE).toBe(46);
    const bar = getActivityBar() as HTMLElement;
    expect(bar.style.width).toBe("46px");
  });

  it("SB-19.35 视图按钮 34×34 圆角 6px", () => {
    render(<ActivityBar />);
    const btn = getButton("nav") as HTMLElement;
    expect(btn.style.width).toBe("34px");
    expect(btn.style.height).toBe("34px");
    expect(btn.style.borderRadius).toBe("6px");
  });

  it("SB-19.36 底部配置钮：data-e2e=activity-btn-config + title=配置，不入 zones/注册表（不可拖拽）", () => {
    render(<ActivityBar />);
    const config = document.querySelector(
      '[data-e2e="activity-btn-config"]',
    ) as HTMLElement;
    expect(config).toBeTruthy();
    expect(config.getAttribute("title")).toBe("配置");
    // 配置钮不参与拖拽（无 draggable 属性）
    expect(config.hasAttribute("draggable")).toBe(false);
    // 不在 zones 数据中（不参与持久化）——zones 不含 config
    const { zones } = useSideBar.getState();
    expect([...zones.top, ...zones.bottom]).not.toContain("config");
  });

  it("SB-19.37 点击配置钮 → openHooksConfigFromActivityBar（入口唯一化）", () => {
    render(<ActivityBar />);
    const config = document.querySelector(
      '[data-e2e="activity-btn-config"]',
    ) as HTMLElement;
    fireEvent.click(config);
    expect(mockOpenHooksConfigFromActivityBar).toHaveBeenCalledTimes(1);
  });

  // ═══ FE-17/23：UI-110 无动效 + dragleave relatedTarget 判定 ═══

  it("SB-19.38 按钮 style 不含 transition（FE-17——UI-110 硬约束无动效）", () => {
    render(<ActivityBar />);
    const btn = getButton("nav") as HTMLElement;
    expect(btn.style.transition).toBe("");
  });

  it("SB-19.39 容器→子元素转移 dragleave 不清指示线；真正离开容器才清（FE-23）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ nav: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 20); // → dropIndicator { top, 0 }（nav 前指示线）
    const indicatorCount = () =>
      Array.from(bar.querySelectorAll("div")).filter(
        (d) => (d as HTMLElement).style.height === "2px",
      ).length;
    expect(indicatorCount()).toBe(1);
    // 容器 → 子元素转移：relatedTarget 为容器内按钮 → 视为未离开，指示线保留
    act(() => {
      const ev = new DragEvent("dragleave", {
        bubbles: true,
        cancelable: true,
        relatedTarget: getButton("explorer"),
      });
      Object.defineProperty(ev, "dataTransfer", { value: dt, configurable: true });
      bar.dispatchEvent(ev);
    });
    expect(indicatorCount()).toBe(1);
    // 真正离开容器：relatedTarget = null → 清指示线
    act(() => {
      const ev = new DragEvent("dragleave", { bubbles: true, cancelable: true });
      Object.defineProperty(ev, "dataTransfer", { value: dt, configurable: true });
      bar.dispatchEvent(ev);
    });
    expect(indicatorCount()).toBe(0);
    spy.mockRestore();
  });
});
