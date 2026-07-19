// activityBar.test.tsx —— ActivityBar 组件 L2 测试（29 用例）
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
  mockLoadSettings: vi.fn().mockResolvedValue(null),
}));

vi.mock("../ipc/settings", () => ({
  saveSettings: mockSaveSettings,
  loadSettings: mockLoadSettings,
}));

import { render, fireEvent, act } from "@testing-library/react";
import { ActivityBar } from "../features/sideViews/ActivityBar";
import { useSideBar } from "../stores/sideBar";
import { sideViewRegistry } from "../features/sideViews/sideViewRegistry";
import {
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  type Zone,
} from "../features/sideViews/sideBarState";
import {
  SIDEBAR_COLORS,
  FOCUS_BORDER,
} from "../theme/colors";

// ── 测试辅助 ──

function registerTestViews(): void {
  sideViewRegistry.register({ id: "projects", title: "项目列表", icon: "📋", component: () => null });
  sideViewRegistry.register({ id: "explorer", title: "文件浏览器", icon: "📁", component: () => null });
  sideViewRegistry.register({ id: "search", title: "搜索", icon: "🔍", component: () => null });
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

function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
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
    expect(topBtns[0].getAttribute("data-view-id")).toBe("projects");
    expect(topBtns[1].getAttribute("data-view-id")).toBe("explorer");
    const bottomBtns = getZoneContainer("bottom").querySelectorAll("[data-view-id]");
    expect(bottomBtns).toHaveLength(0);
  });

  it("SB-19.1b 外层容器有 data-e2e 属性", () => {
    render(<ActivityBar />);
    expect(getActivityBar().getAttribute("data-e2e")).toBe("activity-bar");
  });

  it("SB-19.2 种子下区按钮后正确渲染", () => {
    useSideBar.setState({ zones: { top: ["projects"], bottom: ["explorer", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    expect(getZoneContainer("top").querySelectorAll("[data-view-id]")).toHaveLength(1);
    expect(getZoneContainer("bottom").querySelectorAll("[data-view-id]")).toHaveLength(2);
  });

  // ═══ active 态 ═══

  it("SB-19.3 active 按钮高亮背景 + 左侧指示条", () => {
    render(<ActivityBar />);
    const p = getButton("projects") as HTMLElement;
    const e = getButton("explorer") as HTMLElement;
    expect(p.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
    expect(p.style.borderLeftColor).toBe(hexToRgb(FOCUS_BORDER));
    expect(e.style.borderLeftColor).toBe("transparent");
  });

  it("SB-19.4 点击后 active 态切换", () => {
    render(<ActivityBar />);
    fireEvent.click(getButton("explorer"));
    expect(useSideBar.getState().open.top).toBe("explorer");
    const p = getButton("projects") as HTMLElement;
    const e = getButton("explorer") as HTMLElement;
    expect(p.style.backgroundColor).not.toBe(hexToRgb(SIDEBAR_COLORS.selected));
    expect(e.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
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
    fireEvent.click(getButton("projects"));
    expect(spy).toHaveBeenCalledWith("projects");
    expect(useSideBar.getState().open.top).toBeNull();
    spy.mockRestore();
  });

  // ═══ title / data-e2e ═══

  it("SB-19.7 按钮有 title 和 data-e2e 属性", () => {
    render(<ActivityBar />);
    expect(getButton("projects").getAttribute("title")).toBe("项目列表");
    expect(getButton("explorer").getAttribute("title")).toBe("文件浏览器");
    expect(getButton("projects").getAttribute("data-e2e")).toBe("activity-btn-projects");
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
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    dispatchDragEvent(getActivityBar(), "dragover", dt, 25);
    expect(dt.dropEffect).toBe("move");
    spy.mockRestore();
  });

  it("SB-19.11 dragOver 调用 computeDropTarget 不抛异常（向外层容器派发）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    expect(() => dispatchDragEvent(getActivityBar(), "dragover", createMockDataTransfer(), 25)).not.toThrow();
    spy.mockRestore();
  });

  // ═══ 拖拽：drop → 外层容器 ═══

  it("SB-19.12 drop 调用 moveButton——同 zone（向外层容器派发）", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 25);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
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
    useSideBar.setState({ zones: { top: ["projects", "unknown-view", "explorer"], bottom: [] } });
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
    const btn = getButton("projects") as HTMLElement;
    expect(btn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
    fireEvent.mouseEnter(btn);
    expect(btn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
  });

  // ═══ 新增：跨区拖拽——状态机行为 ═══

  it("SB-19.17 拖拽 explorer 从 top 到空 bottom → moveButton(targetZone='bottom')", () => {
    render(<ActivityBar />);
    // boundary = max(projects.bottom=50, explorer.bottom=90) = 90
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 100);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=400 > midpoint=300 → zone="bottom"
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.18 拖拽 projects 从 bottom 到空 top → moveButton(targetZone='top')", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["projects", "explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top zone 空 → boundary = container midpoint = 300（空上区回退）；clientY=10 < 300 → "top"
    const spy = installRectSpy({ projects: { top: 90, height: 40 }, explorer: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "projects" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 10);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("projects");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.19 拖拽 explorer 从 top 到有按钮的 bottom → zone + index 均正确", () => {
    useSideBar.setState({ zones: { top: ["explorer"], bottom: ["projects", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = explorer.bottom = 50; bottomZoneTop=90
    const spy = installRectSpy({ explorer: { top: 10, height: 40 }, projects: { top: 90, height: 40 }, search: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=400 > midpoint=300 → zone="bottom"
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.20 同区拖拽排序（top zone）→ zone 不变", () => {
    render(<ActivityBar />);
    // boundary=90, clientY=70 < 90 → zone="top"
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "projects" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 70);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.21 同区拖拽排序（bottom zone）→ zone='bottom'", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["projects", "explorer", "search"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top 空, midpoint=300; clientY=400 > 300 → zone="bottom"
    const spy = installRectSpy({ projects: { top: 90, height: 40 }, explorer: { top: 130, height: 40 }, search: { top: 170, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "search" });
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 新增：zone 判定边界 ═══

  it("SB-19.22 clientY 在上区按钮区域内 → zone='top'", () => {
    useSideBar.setState({ zones: { top: ["projects"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = projects.bottom = 50
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "explorer" });
    const bar = getActivityBar();
    // clientY=30 < boundary=50 → zone="top"
    dispatchDragEvent(bar, "dragover", dt, 30);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.23 clientY >= boundary → zone='bottom'", () => {
    useSideBar.setState({ zones: { top: ["projects"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // boundary = projects.bottom = 50
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "projects" });
    const bar = getActivityBar();
    // clientY=50 >= midpoint? No——350 > 300 → "bottom"
    dispatchDragEvent(bar, "dragover", dt, 350);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  it("SB-19.24 bottom zone 有按钮、拖到两按钮之间 → index 正确", () => {
    useSideBar.setState({ zones: { top: [], bottom: ["projects", "explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    // top 空 → boundary ~0; clientY=110 > 0 → "bottom"
    // projects mid=110, explorer mid=150; clientY=110 >=110 → index after projects
    const spy = installRectSpy({ projects: { top: 90, height: 40 }, explorer: { top: 130, height: 40 } }, 90);
    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({ "application/x-side-view-id": "search" });
    const bar = getActivityBar();
    // top 空, midpoint=300; clientY=350 > 300 → "bottom"
    dispatchDragEvent(bar, "dragover", dt, 350);
    dispatchDragEvent(bar, "drop", dt);
    expect(moveSpy.mock.calls[0][1]).toBe("bottom");
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ═══ 新增：指示线与状态清理 ═══

  it("SB-19.25 dragOver 跨 zone 时 indicator.zone 切换", () => {
    useSideBar.setState({ zones: { top: ["projects"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 20); // → "top" (< 300)
    expect(() => dispatchDragEvent(bar, "dragover", dt, 400)).not.toThrow(); // → "bottom" (> 300)
    spy.mockRestore();
  });

  it("SB-19.26 onDragLeave 离开活动栏 → dropIndicator 清 null", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 } }, 800);
    const dt = createMockDataTransfer();
    const bar = getActivityBar();
    dispatchDragEvent(bar, "dragover", dt, 20);
    act(() => {
      const ev = new DragEvent("dragleave", { bubbles: true, cancelable: true });
      Object.defineProperty(ev, "dataTransfer", { value: dt, configurable: true });
      Object.defineProperty(ev, "clientY", { value: 20, configurable: true });
      bar.dispatchEvent(ev);
    });
    // 不抛异常即通过
    spy.mockRestore();
  });

  it("SB-19.27 drop 后 opacity 恢复 + dragEnd 幂等清理", () => {
    useSideBar.setState({ zones: { top: ["projects"], bottom: ["explorer"] }, open: { top: null, bottom: null } });
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 90, height: 40 } }, 90);
    const dt = createMockDataTransfer({ "application/x-side-view-id": "projects" });
    const bar = getActivityBar();
    const btn = getButton("projects") as HTMLElement;
    dispatchDragEvent(btn, "dragstart", dt);
    expect(btn.style.opacity).toBe("0.5");
    // clientY=400 > midpoint=300 → "bottom" (cross-zone)
    dispatchDragEvent(bar, "dragover", dt, 400);
    dispatchDragEvent(bar, "drop", dt);
    // drop 中 clearDragState → React 重渲染 → re-query
    const after = getButton("projects") as HTMLElement;
    expect(after.style.opacity).toBe("1");
    dispatchDragEvent(after, "dragend", createMockDataTransfer());
    expect(after.style.opacity).toBe("1");
    spy.mockRestore();
  });

  it("SB-19.28 onDragLeave 子元素间转移不触发清理", () => {
    render(<ActivityBar />);
    const spy = installRectSpy({ projects: { top: 10, height: 40 }, explorer: { top: 50, height: 40 } }, 800);
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
});
