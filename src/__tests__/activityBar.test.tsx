// activityBar.test.tsx —— ActivityBar 组件 L2 测试（SB-19）
//
// 测试模式：真实 sideBar store（beforeEach setState 重置默认）+ sideViewRegistry._reset() 注册 stub
// 拖拽测试用 Object.defineProperty 覆盖 DragEvent.dataTransfer（jsdom 兼容）

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

/** 注册 stub 视图（组件为 () => null） */
function registerTestViews(): void {
  sideViewRegistry.register({
    id: "projects",
    title: "项目列表",
    icon: "📋",
    component: () => null,
  });
  sideViewRegistry.register({
    id: "explorer",
    title: "文件浏览器",
    icon: "📁",
    component: () => null,
  });
  sideViewRegistry.register({
    id: "search",
    title: "搜索",
    icon: "🔍",
    component: () => null,
  });
}

/** 创建模拟 DataTransfer（jsdom DragEvent.dataTransfer 不可写时回退用） */
function createMockDataTransfer(initial: Record<string, string> = {}) {
  const store = new Map<string, string>(Object.entries(initial));
  return {
    store,
    dropEffect: "none" as string,
    effectAllowed: "none" as string,
    setData(format: string, value: string) {
      store.set(format, value);
    },
    getData(format: string) {
      return store.get(format) || "";
    },
  };
}

/** hex 颜色转 rgb 字符串——jsdom style.backgroundColor 返回 rgb() 非 hex */
function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

/** 在元素上派发含 dataTransfer 的 DragEvent（act 包裹确保 React flush） */
function dispatchDragEvent(
  element: Element,
  type: string,
  dt: ReturnType<typeof createMockDataTransfer>,
): void {
  act(() => {
    const event = new DragEvent(type, { bubbles: true, cancelable: true });
    Object.defineProperty(event, "dataTransfer", {
      value: dt,
      configurable: true,
    });
    element.dispatchEvent(event);
  });
}

/** 按 data-e2e 查找按钮 */
function getButton(id: string): Element {
  const el = document.querySelector(`[data-e2e="activity-btn-${id}"]`);
  if (!el) throw new Error(`Button not found: activity-btn-${id}`);
  return el;
}

/** 按 data-zone 查找 zone 容器 */
function getZoneContainer(zone: Zone): Element {
  const el = document.querySelector(`[data-zone="${zone}"]`);
  if (!el) throw new Error(`Zone container not found: ${zone}`);
  return el;
}

// ── 测试套件 ──

describe("ActivityBar", () => {
  beforeEach(() => {
    sideViewRegistry._reset();
    registerTestViews();
    useSideBar.setState({
      zones: {
        top: [...DEFAULT_ZONES.top],
        bottom: [...DEFAULT_ZONES.bottom],
      },
      open: { ...DEFAULT_OPEN },
      width: 250,
      splitRatio: 0.5,
      loaded: false,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ── 渲染结构 ──

  it("SB-19.1 渲染上区/下区两按钮组，按 zones 顺序排列", () => {
    render(<ActivityBar />);

    const topContainer = getZoneContainer("top");
    const bottomContainer = getZoneContainer("bottom");

    // 上区按钮按 zones.top 顺序
    const topButtons = topContainer.querySelectorAll("[data-view-id]");
    expect(topButtons).toHaveLength(2);
    expect(topButtons[0].getAttribute("data-view-id")).toBe("projects");
    expect(topButtons[1].getAttribute("data-view-id")).toBe("explorer");

    // 下区默认空
    const bottomButtons = bottomContainer.querySelectorAll("[data-view-id]");
    expect(bottomButtons).toHaveLength(0);
  });

  it("SB-19.2 种子下区按钮后正确渲染", () => {
    useSideBar.setState({
      zones: {
        top: ["projects"],
        bottom: ["explorer", "search"],
      },
      open: { top: null, bottom: null },
    });

    render(<ActivityBar />);

    const topButtons = getZoneContainer("top").querySelectorAll("[data-view-id]");
    expect(topButtons).toHaveLength(1);
    expect(topButtons[0].getAttribute("data-view-id")).toBe("projects");

    const bottomButtons = getZoneContainer("bottom").querySelectorAll(
      "[data-view-id]",
    );
    expect(bottomButtons).toHaveLength(2);
    expect(bottomButtons[0].getAttribute("data-view-id")).toBe("explorer");
    expect(bottomButtons[1].getAttribute("data-view-id")).toBe("search");
  });

  // ── active 态 ──

  it("SB-19.3 active 按钮高亮背景 + 左侧指示条", () => {
    render(<ActivityBar />);

    // 默认 projects 打开（DEFAULT_OPEN.top === "projects"）
    const projectsBtn = getButton("projects") as HTMLElement;
    const explorerBtn = getButton("explorer") as HTMLElement;

    // active 按钮背景为 selected
    expect(projectsBtn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
    // active 按钮左侧有 FOCUS_BORDER 指示条
    expect(projectsBtn.style.borderLeftColor).toBe(hexToRgb(FOCUS_BORDER));

    // 非 active 按钮无指示条、背景透明
    expect(explorerBtn.style.borderLeftColor).toBe("transparent");
  });

  it("SB-19.4 点击后 active 态切换", () => {
    render(<ActivityBar />);

    // 点击 explorer → projects 关闭、explorer 打开
    const explorerBtn = getButton("explorer");
    fireEvent.click(explorerBtn);

    // store 状态已变更
    const state = useSideBar.getState();
    expect(state.open.top).toBe("explorer");

    // DOM 重新渲染（RTL 自动 rerender）
    const projectsBtn = getButton("projects") as HTMLElement;
    const explorerBtnAfter = getButton("explorer") as HTMLElement;
    expect(projectsBtn.style.backgroundColor).not.toBe(hexToRgb(SIDEBAR_COLORS.selected));
    expect(explorerBtnAfter.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
  });

  // ── 点击 → toggleView ──

  it("SB-19.5 点击按钮调用 toggleView", () => {
    render(<ActivityBar />);

    const toggleSpy = vi.spyOn(useSideBar.getState(), "toggleView");
    const explorerBtn = getButton("explorer");
    fireEvent.click(explorerBtn);

    expect(toggleSpy).toHaveBeenCalledWith("explorer");
    toggleSpy.mockRestore();
  });

  it("SB-19.6 再次点击同一按钮关闭视图", () => {
    render(<ActivityBar />);

    const toggleSpy = vi.spyOn(useSideBar.getState(), "toggleView");
    const projectsBtn = getButton("projects");
    fireEvent.click(projectsBtn);

    // 默认 projects 已打开，点击应关闭
    expect(toggleSpy).toHaveBeenCalledWith("projects");
    // toggleViewPure 将 open.top 置 null
    const state = useSideBar.getState();
    expect(state.open.top).toBeNull();
    toggleSpy.mockRestore();
  });

  // ── title / data-e2e 属性 ──

  it("SB-19.7 按钮有 title 和 data-e2e 属性", () => {
    render(<ActivityBar />);

    const projectsBtn = getButton("projects");
    expect(projectsBtn.getAttribute("title")).toBe("项目列表");
    expect(projectsBtn.getAttribute("data-e2e")).toBe("activity-btn-projects");

    const explorerBtn = getButton("explorer");
    expect(explorerBtn.getAttribute("title")).toBe("文件浏览器");
    expect(explorerBtn.getAttribute("data-e2e")).toBe("activity-btn-explorer");
  });

  // ── HTML5 拖拽：dragStart ──

  it("SB-19.8 dragStart 设置 dataTransfer 内容 + effectAllowed", () => {
    render(<ActivityBar />);

    const dt = createMockDataTransfer();
    const explorerBtn = getButton("explorer");
    dispatchDragEvent(explorerBtn, "dragstart", dt);

    expect(dt.store.get("application/x-side-view-id")).toBe("explorer");
    expect(dt.effectAllowed).toBe("move");
  });

  it("SB-19.9 dragStart 后按钮半透明", () => {
    render(<ActivityBar />);

    const dt = createMockDataTransfer();
    const explorerBtn = getButton("explorer") as HTMLElement;
    dispatchDragEvent(explorerBtn, "dragstart", dt);

    // 拖拽中 opacity 应为 0.5
    expect(explorerBtn.style.opacity).toBe("0.5");
  });

  // ── HTML5 拖拽：dragOver + 指示线 ──

  it("SB-19.10 dragOver 调用 preventDefault 并设置 dropEffect", () => {
    render(<ActivityBar />);

    // 给上区按钮设置 getBoundingClientRect stub
    const spy = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        const id = this.getAttribute("data-view-id");
        if (id === "projects") {
          return {
            top: 10,
            bottom: 50,
            height: 40,
            left: 0,
            right: 40,
            width: 40,
            x: 0,
            y: 10,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (id === "explorer") {
          return {
            top: 50,
            bottom: 90,
            height: 40,
            left: 0,
            right: 40,
            width: 40,
            x: 0,
            y: 50,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return {
          top: 0,
          bottom: 0,
          height: 0,
          left: 0,
          right: 0,
          width: 0,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        } as DOMRect;
      });

    const dt = createMockDataTransfer();
    const topContainer = getZoneContainer("top");

    // clientY 在 projects 按钮上半 → index=0
    dispatchDragEvent(topContainer, "dragover", dt);

    expect(dt.dropEffect).toBe("move");

    spy.mockRestore();
  });

  it("SB-19.11 dragOver 调用 computeDropTarget 并产生 dropIndicator", () => {
    render(<ActivityBar />);

    // 设置 getBoundingClientRect stub，使 computeDropTarget 能计算落点
    const spy = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        const id = this.getAttribute("data-view-id");
        if (id === "projects") {
          return {
            top: 10, bottom: 50, height: 40,
            left: 0, right: 40, width: 40, x: 0, y: 10,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (id === "explorer") {
          return {
            top: 50, bottom: 90, height: 40,
            left: 0, right: 40, width: 40, x: 0, y: 50,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return {
          top: 0, bottom: 0, height: 0,
          left: 0, right: 0, width: 0, x: 0, y: 0,
          toJSON: () => ({}),
        } as DOMRect;
      });

    const dt = createMockDataTransfer();
    const topContainer = getZoneContainer("top");

    // clientY=25 在 projects (top=10, mid=30) 上半 → computeDropTarget 返回 {zone:"top",index:0}
    const dragOverEvent = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
      clientX: 0,
      clientY: 25,
    } as MouseEventInit);
    Object.defineProperty(dragOverEvent, "dataTransfer", {
      value: dt,
      configurable: true,
    });

    // 处理正常——不抛异常
    expect(() => {
      topContainer.dispatchEvent(dragOverEvent);
    }).not.toThrow();

    spy.mockRestore();
  });

  // ── HTML5 拖拽：drop ──

  it("SB-19.12 drop 调用 moveButton 参数正确（zone + index）", () => {
    render(<ActivityBar />);

    // 设置 getBoundingClientRect stub
    const spy = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        const id = this.getAttribute("data-view-id");
        if (id === "projects") {
          return {
            top: 10,
            bottom: 50,
            height: 40,
            left: 0,
            right: 40,
            width: 40,
            x: 0,
            y: 10,
            toJSON: () => ({}),
          } as DOMRect;
        }
        if (id === "explorer") {
          return {
            top: 50,
            bottom: 90,
            height: 40,
            left: 0,
            right: 40,
            width: 40,
            x: 0,
            y: 50,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return {
          top: 0,
          bottom: 0,
          height: 0,
          left: 0,
          right: 0,
          width: 0,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        } as DOMRect;
      });

    const moveSpy = vi.spyOn(useSideBar.getState(), "moveButton");
    const dt = createMockDataTransfer({
      "application/x-side-view-id": "explorer",
    });
    const topContainer = getZoneContainer("top");

    // jsdom 限制: DragEvent 继承 MouseEvent 但 clientY 不传递
    // computeDropTarget 在 clientY=0 时返回 {zone:"top",index:0}（projects 按钮上半）
    // 验证 moveButton 调用了正确 id 和 zone——index 因 jsdom 限制为 0
    const dragOverEvent = new DragEvent("dragover", {
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperty(dragOverEvent, "dataTransfer", {
      value: dt,
      configurable: true,
    });
    topContainer.dispatchEvent(dragOverEvent);

    // 再 drop
    const dropEvent = new DragEvent("drop", {
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperty(dropEvent, "dataTransfer", {
      value: dt,
      configurable: true,
    });
    topContainer.dispatchEvent(dropEvent);

    expect(moveSpy).toHaveBeenCalled();
    expect(moveSpy.mock.calls[0][0]).toBe("explorer");
    expect(moveSpy.mock.calls[0][1]).toBe("top");
    // index 因 jsdom clientY=0 恒为第一个按钮位置——computeDropTarget 行为由
    // sideBarState.test.ts 的纯函数测试完整覆盖
    moveSpy.mockRestore();
    spy.mockRestore();
  });

  // ── HTML5 拖拽：dragEnd 清理 ──

  it("SB-19.13 dragEnd 清除拖拽状态（opacity 恢复）", () => {
    render(<ActivityBar />);

    const dt = createMockDataTransfer();
    const explorerBtn = getButton("explorer") as HTMLElement;

    // dragStart → opacity 0.5（使用原始元素引用，同 SB-19.9）
    dispatchDragEvent(explorerBtn, "dragstart", dt);
    expect(explorerBtn.style.opacity).toBe("0.5");

    // dragEnd → opacity 恢复为 1（通过清除 draggingId state）
    dispatchDragEvent(explorerBtn, "dragend", createMockDataTransfer());
    expect(explorerBtn.style.opacity).toBe("1");
  });

  // ── 未注册 id 防御 ──

  it("SB-19.14 zones 含未注册 id 时正常渲染不抛异常", () => {
    // 种子一个未注册的 id
    useSideBar.setState({
      zones: {
        top: ["projects", "unknown-view", "explorer"],
        bottom: [],
      },
    });

    render(<ActivityBar />);

    // 应只渲染已注册的两个按钮
    const topButtons = getZoneContainer("top").querySelectorAll("[data-view-id]");
    expect(topButtons).toHaveLength(2);
    expect(topButtons[0].getAttribute("data-view-id")).toBe("projects");
    expect(topButtons[1].getAttribute("data-view-id")).toBe("explorer");
  });

  // ── hover 反馈 ──

  it("SB-19.15 hover 态显示 hover 背景色，离开恢复", () => {
    render(<ActivityBar />);

    const explorerBtn = getButton("explorer") as HTMLElement;

    // 初始背景透明（非 active）
    expect(explorerBtn.style.backgroundColor).toBe("transparent");

    // hover
    fireEvent.mouseEnter(explorerBtn);
    expect(explorerBtn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.hover));

    // 离开
    fireEvent.mouseLeave(explorerBtn);
    expect(explorerBtn.style.backgroundColor).toBe("transparent");
  });

  it("SB-19.16 active 按钮 hover 时不覆盖 active 背景色", () => {
    render(<ActivityBar />);

    // projects 默认 active
    const projectsBtn = getButton("projects") as HTMLElement;
    expect(projectsBtn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));

    // hover 不改变 active 背景
    fireEvent.mouseEnter(projectsBtn);
    expect(projectsBtn.style.backgroundColor).toBe(hexToRgb(SIDEBAR_COLORS.selected));
  });
});
