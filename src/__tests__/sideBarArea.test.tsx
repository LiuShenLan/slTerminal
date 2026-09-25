// sideBarArea.test.tsx — SideBarArea 组件 L2 测试
//
// 验证：单开/双开 pane visible、视图槽条件渲染（FE-21：隐藏视图卸载不保挂载）、
// 换区后视图移槽重建、props 透传、onChange→setSplitRatio。
//
// Mock allotment 为渲染 children 并记录 Pane props 的 stub
// （参照 workspace-e2e-ready 的 MockAllotment 模式但保留 children 渲染）。

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React, { useEffect } from "react";
import { render, cleanup, act } from "@testing-library/react";

// ─── Hoisted mocks（仅状态——组件创建须推迟到 vi.mock 工厂内，hoisted 期 React 未初始化） ───
const mocks = vi.hoisted(() => ({
  /** Pane props 记录器（按渲染顺序追加） */
  panePropsList: [] as Array<Record<string, unknown>>,
  /** 捕获的 Allotment onChange 回调 */
  onChangeCell: { fn: null as ((sizes: number[]) => void) | null },
  /** ref.resize 调用记录（比例像素纠偏契约断言点，2026-09） */
  resizeCalls: [] as number[][],
  getOnChange(): ((sizes: number[]) => void) | null {
    return this.onChangeCell.fn;
  },
}));

vi.mock("allotment", async () => {
  const React = await import("react");
  const MockAllotment: any = React.forwardRef(
    ({ children, onChange }: any, ref: any) => {
      mocks.onChangeCell.fn = onChange;
      React.useImperativeHandle(ref, () => ({
        resize: (sizes: number[]) => {
          mocks.resizeCalls.push([...sizes]);
        },
        reset: () => {},
      }));
      return React.createElement(
        "div",
        { "data-testid": "allotment-vertical" },
        children,
      );
    },
  );

  MockAllotment.Pane = (props: any) => {
    const { children, ...rest } = props;
    mocks.panePropsList.push(rest);
    return React.createElement(
      "div",
      { "data-testid": `allotment-pane-${mocks.panePropsList.length}` },
      children,
    );
  };

  return { Allotment: MockAllotment };
});

// 阻止 store subscribe 触发真实 saveSettings（loaded=false 已足够，但显式 mock 更安全）
// FE-11/D11：wrapper 返回 { data, corrupted }——无文件 = data:null, corrupted:false
vi.mock("../../ipc/settings", () => ({
  loadSettings: vi.fn(() => Promise.resolve({ data: null, corrupted: false })),
  saveSettings: vi.fn(() => Promise.resolve()),
}));

// 阻断 side-effect 注册：真实 nav/explorer/commit 视图不得混入本文件的
// sideViewRegistry 隔离（_reset 后仅注册 stub；TQ-B-02）
vi.mock("../features/sideViews/sideViewDefs", () => ({}));

// ─── 导入（mock 之后）───
import { SideBarArea } from "../features/sideViews/SideBarArea";
import type { SideBarAreaProps } from "../features/sideViews/SideBarArea";
import { useSideBar } from "../stores/sideBar";
import { sideViewRegistry } from "../features/sideViews/sideViewRegistry";
import type { SideViewComponentProps } from "../features/sideViews/sideViewRegistry";
import {
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  SPLIT_DEFAULT,
} from "../features/sideViews/sideBarState";

// ─── 辅助 ───

/** 制造带 data-testid 的 stub 视图组件 */
function makeStubView(
  label: string,
  onMount?: () => void,
  onUnmount?: () => void,
): React.FC<SideViewComponentProps> {
  const StubView: React.FC<SideViewComponentProps> = () => {
    useEffect(() => {
      onMount?.();
      return () => {
        onUnmount?.();
      };
    }, []);
    return React.createElement(
      "div",
      { "data-testid": `stub-view-${label}` },
      label,
    );
  };
  StubView.displayName = `StubView_${label}`;
  return StubView;
}

/** 重置 store 到默认值 + loaded（阻止 persist 触发） */
function resetStore() {
  useSideBar.setState({
    zones: { top: [...DEFAULT_ZONES.top], bottom: [...DEFAULT_ZONES.bottom] },
    open: { ...DEFAULT_OPEN },
    width: 250,
    splitRatio: SPLIT_DEFAULT,
    loaded: true,
  });
}

/** 清空 pane 记录器 + resize 调用记录 */
function resetPaneProps() {
  mocks.panePropsList.length = 0;
  mocks.resizeCalls.length = 0;
}

/** 模拟容器视口高度（jsdom clientHeight 恒 0——覆盖 HTMLElement.prototype getter）。
 *  返回恢复函数（照 explorer-virtualization 先例）。 */
function mockClientHeight(h: number): () => void {
  const proto = HTMLElement.prototype as unknown as { clientHeight: number };
  const desc = Object.getOwnPropertyDescriptor(proto, "clientHeight");
  Object.defineProperty(proto, "clientHeight", {
    configurable: true,
    get: () => h,
  });
  return () => {
    if (desc) Object.defineProperty(proto, "clientHeight", desc);
    else
      delete (proto as unknown as Record<string, unknown>).clientHeight;
  };
}

/** 获取记录的 pane props（按渲染序） */
function getPaneProps(): Record<string, unknown>[] {
  return [...mocks.panePropsList];
}

/** 冲刷 N 帧 rAF——纠偏 effect 的就绪轮询以 rAF 驱动，断言前须跨帧 */
async function flushRaf(rounds = 3): Promise<void> {
  for (let i = 0; i < rounds; i++) {
    await act(async () => {
      await new Promise((resolve) =>
        requestAnimationFrame(() => resolve(null)),
      );
    });
  }
}

/** 默认 props */
const defaultProps: SideBarAreaProps = {
  switchToPage: vi.fn(),
  onDeletePage: vi.fn(),
};

// ─── Tests ───

describe("SideBarArea", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    sideViewRegistry._reset();
    // 防御断言：_reset 后注册表必须为空（sideViewDefs 被 mock 阻断，TQ-B-02）
    expect(sideViewRegistry.getAll().length).toBe(0);
    resetStore();
    resetPaneProps();

    // 注册两个 stub 视图（icon 为组件形态——IC-06）
    sideViewRegistry.register({
      id: "nav",
      title: "项目列表",
      icon: () => null,
      component: makeStubView("nav"),
    });
    sideViewRegistry.register({
      id: "explorer",
      title: "文件浏览器",
      icon: () => null,
      component: makeStubView("explorer"),
    });
  });

  // ─── SB-20: 单开上区全高 ───
  it("单开上区时仅上 pane visible，下 pane hidden", () => {
    useSideBar.setState({
      open: { top: "nav", bottom: null },
    });

    render(React.createElement(SideBarArea, defaultProps));

    const paneProps = getPaneProps();
    // 两个 pane 均应渲染（Allotment 总是渲染所有 pane，visible 控制 CSS 显隐）
    expect(paneProps).toHaveLength(2);
    expect(paneProps[0].visible).toBe(true);
    expect(paneProps[1].visible).toBe(false);
  });

  // ─── SB-20: 单开下区全高 ───
  it("单开下区时仅下 pane visible，上 pane hidden", () => {
    useSideBar.setState({
      zones: { top: [], bottom: ["nav"] },
      open: { top: null, bottom: "nav" },
    });

    render(React.createElement(SideBarArea, defaultProps));

    const paneProps = getPaneProps();
    expect(paneProps).toHaveLength(2);
    expect(paneProps[0].visible).toBe(false);
    expect(paneProps[1].visible).toBe(true);
  });

  // ─── SB-20: 双开两 pane visible ───
  it("双开时两 pane 均 visible", () => {
    useSideBar.setState({
      zones: { top: ["nav"], bottom: ["explorer"] },
      open: { top: "nav", bottom: "explorer" },
    });

    render(React.createElement(SideBarArea, defaultProps));

    const paneProps = getPaneProps();
    expect(paneProps).toHaveLength(2);
    expect(paneProps[0].visible).toBe(true);
    expect(paneProps[1].visible).toBe(true);
  });

  // ─── SB-20: 比例像素纠偏契约（2026-09 二轮重写——preferredSize 无法承载比例语义；
  //     纠偏必须等「就绪探针」（首个双 pane fire = viewItems 已 populate），否则
  //     mount 期调 resize 读空 viewItems.minimumSize 崩溃（Bug 1）；纠偏窗口期
  //     onChange 写回被屏蔽，比例用进入窗口前的快照） ───
  it("挂载即双开（重启恢复路径）：就绪探针 fire 后按 splitRatio 快照纠偏", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({
        splitRatio: 0.6,
        open: { top: "nav", bottom: "explorer" },
      });
      useSideBar.getState().moveButton("explorer", "bottom", 0);

      render(React.createElement(SideBarArea, defaultProps));

      // 就绪探针未 fire 前不得纠偏（Bug 1 崩溃窗口）
      expect(mocks.resizeCalls).toHaveLength(0);

      // 首个双 pane fire = viewItems 已 populate（mount 链瞬时 fire 形态之一 [H,0]）
      act(() => {
        mocks.getOnChange()!([600, 0]);
      });
      await flushRaf();

      expect(mocks.resizeCalls.length).toBe(1);
      expect(mocks.resizeCalls[0]).toEqual([360, 240]); // 600*0.6 / 600*0.4
    } finally {
      restore();
    }
  });

  it("首开下区（单→双转换）默认比例 0.5：resize 为均分像素（需求 2 契约）", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({ open: { top: "nav", bottom: null } });
      render(React.createElement(SideBarArea, defaultProps));
      // 单开不纠偏
      await flushRaf();
      expect(mocks.resizeCalls).toHaveLength(0);

      // 首开下区（explorer 已在默认 top 区——移到下区后打开）
      act(() => {
        useSideBar.getState().moveButton("explorer", "bottom", 0);
        useSideBar.getState().toggleView("explorer");
      });
      // 双 pane 就绪探针
      act(() => {
        mocks.getOnChange()!([300, 300]);
      });
      await flushRaf();

      expect(mocks.resizeCalls.length).toBe(1);
      expect(mocks.resizeCalls[0]).toEqual([300, 300]);
    } finally {
      restore();
    }
  });

  it("单开挂载不纠偏（比例语义只在双开时存在，与 onChange 写回守卫同边界）", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({ open: { top: "nav", bottom: null } });
      render(React.createElement(SideBarArea, defaultProps));
      await flushRaf();
      expect(mocks.resizeCalls).toHaveLength(0);
    } finally {
      restore();
    }
  });

  // ─── Bug 2 防复发：关闭帧瞬时 fire [H,0] 不得污染 splitRatio（旧闭包
  //     ratio=1.0 → clamp 0.9 棘轮），重开按未污染的原比例纠偏 ───
  it("关闭下区瞬时 fire [H,0] 不污染 splitRatio；重开后按原比例纠偏（Bug 2 防复发）", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({
        splitRatio: 0.6,
        zones: { top: ["nav"], bottom: ["explorer"] },
        open: { top: "nav", bottom: "explorer" },
      });
      render(React.createElement(SideBarArea, defaultProps));

      // 走完首轮纠偏窗口（就绪 → resize → 放行写回）
      act(() => {
        mocks.getOnChange()!([360, 240]);
      });
      await flushRaf();
      expect(mocks.resizeCalls[0]).toEqual([360, 240]);
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.6);

      // 关闭下区——库在 layout 期同步 fire [H,0]（Bug 2 旧闭包语义复现点）
      act(() => {
        useSideBar.getState().toggleView("explorer");
      });
      act(() => {
        mocks.getOnChange()!([600, 0]);
      });
      // 零尺寸闸吞掉——store 不被污染成 0.9
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.6);

      // 重开下区 → 按未污染的 0.6 纠偏（不是 [0.9H, 0.1H]）
      act(() => {
        useSideBar.getState().toggleView("explorer");
      });
      act(() => {
        mocks.getOnChange()!([360, 240]); // cached 恢复的双 pane fire
      });
      await flushRaf();
      const last = mocks.resizeCalls[mocks.resizeCalls.length - 1];
      expect(last).toEqual([360, 240]);
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.6);
    } finally {
      restore();
    }
  });

  // ─── 第三隐患防复发：mount 链瞬时 fire [H-30,30]/[H/2,H/2] 在纠偏窗口期
  //     被屏蔽，持久化比例不被冲掉，纠偏用窗口前快照 ───
  it("挂载即双开：纠偏窗口期瞬时 fire 不写回，快照比例免疫（比例棘轮防复发）", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({
        splitRatio: 0.7,
        zones: { top: ["nav"], bottom: ["explorer"] },
        open: { top: "nav", bottom: "explorer" },
      });
      render(React.createElement(SideBarArea, defaultProps));

      // mount 链瞬时 fire 两种形态（库实证 [H-30,30] 与 [H/2,H/2]）
      act(() => {
        mocks.getOnChange()!([570, 30]);
      });
      act(() => {
        mocks.getOnChange()!([300, 300]);
      });
      // 窗口期屏蔽：store 保持持久化值 0.7（不被冲成 0.9/0.5）
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.7);

      await flushRaf();
      // 纠偏用快照 0.7——不是任一瞬时 fire 换算值
      expect(mocks.resizeCalls.length).toBe(1);
      expect(mocks.resizeCalls[0]).toEqual([420, 180]);
    } finally {
      restore();
    }
  });

  // ─── 零尺寸闸独立契约：纠偏窗口已关闭后，双开态 fire 含零尺寸仍不写回 ───
  it("onChange 双开态 fire 含零尺寸不写回（零尺寸闸独立于纠偏窗口）", async () => {
    const restore = mockClientHeight(600);
    try {
      useSideBar.setState({
        splitRatio: 0.5,
        zones: { top: ["nav"], bottom: ["explorer"] },
        open: { top: "nav", bottom: "explorer" },
      });
      render(React.createElement(SideBarArea, defaultProps));

      // 先走完纠偏窗口（否则 pending 闸先行吞掉，测不到零尺寸闸）
      act(() => {
        mocks.getOnChange()!([300, 300]);
      });
      await flushRaf();

      act(() => {
        mocks.getOnChange()!([600, 0]);
      });
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.5);
    } finally {
      restore();
    }
  });

  // ─── SB-20: 视图槽条件渲染（FE-21）───
  it("打开的视图槽渲染，隐藏的视图槽不渲染（FE-21 条件渲染替代 display:none 保挂载）", () => {
    // 两个视图都在上区，仅 nav 打开
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { container } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // 上区 nav 槽渲染
    const navSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-nav"]',
    ) as HTMLElement | null;
    expect(navSlot).not.toBeNull();
    expect(navSlot!.style.display).toBe("flex");

    // 上区 explorer 槽被卸载（隐藏视图不再留 DOM/订阅）
    const explorerSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-explorer"]',
    ) as HTMLElement | null;
    expect(explorerSlot).toBeNull();
  });

  // ─── SB-20: 切换 open 后槽位卸载/挂载同步更新 ───
  it("切换 open 后旧视图槽卸载、新视图槽挂载（FE-21）", () => {
    // 初始：nav 打开
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { container, rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // 切到 explorer 打开（R1 替换）
    useSideBar.getState().toggleView("explorer");

    rerender(React.createElement(SideBarArea, defaultProps));

    const navSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-nav"]',
    ) as HTMLElement | null;
    const explorerSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-explorer"]',
    ) as HTMLElement | null;

    // 旧视图槽卸载、新视图槽挂载
    expect(navSlot).toBeNull();
    expect(explorerSlot).not.toBeNull();
    expect(explorerSlot!.style.display).toBe("flex");
  });

  // ─── SB-20: 隐藏视图组件卸载（FE-21 条件渲染）───
  it("隐藏的视图组件被卸载（FE-21），仅打开的视图挂载", () => {
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { container } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // 仅打开视图组件存在于 DOM
    const navView = container.querySelector(
      '[data-testid="stub-view-nav"]',
    );
    const explorerView = container.querySelector(
      '[data-testid="stub-view-explorer"]',
    );

    expect(navView).not.toBeNull();
    expect(explorerView).toBeNull();
  });

  // ─── SB-20: 切换时旧视图组件卸载（onUnmount 触发）───
  it("切换视图时旧视图组件卸载（onUnmount 触发）——状态丢失语义 ADR-0001 已接受", () => {
    let navUnmountCount = 0;

    sideViewRegistry._reset();
    sideViewRegistry.register({
      id: "nav",
      title: "项目列表",
      icon: () => null,
      component: makeStubView("nav", undefined, () => {
        navUnmountCount++;
      }),
    });
    sideViewRegistry.register({
      id: "explorer",
      title: "文件浏览器",
      icon: () => null,
      component: makeStubView("explorer"),
    });

    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { container, rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );
    expect(navUnmountCount).toBe(0);

    // 切到 explorer：nav 组件卸载（onUnmount 触发一次）
    useSideBar.getState().toggleView("explorer");
    rerender(React.createElement(SideBarArea, defaultProps));

    expect(navUnmountCount).toBe(1);
    expect(
      container.querySelector('[data-testid="stub-view-explorer"]'),
    ).not.toBeNull();
  });

  // ─── SB-20: 换区后视图移槽——旧组件卸载，新组件挂载 ───
  it("换区后视图移槽——旧区组件卸载，新区组件挂载", () => {
    let projectMountCount = 0;
    let projectUnmountCount = 0;

    // 使用带 mount/unmount 跟踪的 stub
    sideViewRegistry._reset();
    sideViewRegistry.register({
      id: "nav",
      title: "项目列表",
      icon: () => null,
      component: makeStubView(
        "nav",
        () => {
          projectMountCount++;
        },
        () => {
          projectUnmountCount++;
        },
      ),
    });
    sideViewRegistry.register({
      id: "explorer",
      title: "文件浏览器",
      icon: () => null,
      component: makeStubView("explorer"),
    });

    // 初始：projects 在上区打开
    useSideBar.setState({
      zones: { top: ["nav", "explorer"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // projects 已挂载一次
    expect(projectMountCount).toBe(1);
    expect(projectUnmountCount).toBe(0);

    // 移动 projects 到下区（带跟随，因为正打开）
    useSideBar.getState().moveButton("nav", "bottom", 0);

    rerender(React.createElement(SideBarArea, defaultProps));

    // 旧区组件卸载 + 新区组件挂载 = 换区重建
    // mount: 初始 1 + 重建 1 = 2
    // unmount: 旧区卸载 1
    expect(projectMountCount).toBe(2);
    expect(projectUnmountCount).toBe(1);
  });

  // ─── SB-20: props 透传到视图组件 ───
  it("switchToPage 和 onDeletePage 透传到视图组件", () => {
    const capturedRef: { current: SideViewComponentProps | null } = {
      current: null,
    };

    sideViewRegistry._reset();
    sideViewRegistry.register({
      id: "nav",
      title: "项目列表",
      icon: () => null,
      component: (props: SideViewComponentProps) => {
        capturedRef.current = props;
        return React.createElement("div", { "data-testid": "stub-view-projects" });
      },
    });

    const switchToPage = vi.fn();
    const onDeletePage = vi.fn();

    render(
      React.createElement(SideBarArea, {
        switchToPage,
        onDeletePage,
      }),
    );

    expect(capturedRef.current).not.toBeNull();
    expect(capturedRef.current!.switchToPage).toBe(switchToPage);
    expect(capturedRef.current!.onDeletePage).toBe(onDeletePage);
  });

  // ─── SB-20: onChange 双开时 write back splitRatio ───
  it("onChange 双开时换算 ratio 写回 setSplitRatio", async () => {
    const restore = mockClientHeight(600);
    try {
      const setSplitRatioSpy = vi.spyOn(
        useSideBar.getState(),
        "setSplitRatio",
      );

      useSideBar.setState({
        zones: { top: ["nav"], bottom: ["explorer"] },
        open: { top: "nav", bottom: "explorer" },
        splitRatio: 0.5,
      });

      render(React.createElement(SideBarArea, defaultProps));

      // 先走完首轮纠偏窗口（就绪探针 + 跨帧），放行后续写回
      act(() => {
        mocks.getOnChange()!([300, 300]);
      });
      await flushRaf();

      const onChange = mocks.getOnChange();
      expect(onChange).not.toBeNull();

      // 模拟拖拽分隔条：上区 300px，下区 200px
      act(() => {
        onChange!([300, 200]);
      });

      expect(setSplitRatioSpy).toHaveBeenCalledWith(0.6); // 300 / 500
      setSplitRatioSpy.mockRestore();
    } finally {
      restore();
    }
  });

  // ─── SB-20: onChange 单开时除零守卫不写回 ───
  it("onChange 单开时不写回 splitRatio（除零守卫）", () => {
    const setSplitRatioSpy = vi.spyOn(
      useSideBar.getState(),
      "setSplitRatio",
    );

    // 仅上区打开
    useSideBar.setState({
      open: { top: "nav", bottom: null },
    });

    render(React.createElement(SideBarArea, defaultProps));

    const onChange = mocks.getOnChange();
    expect(onChange).not.toBeNull();

    // 模拟仅单 pane visible 时的 sizes（Allotment 单 pane 时 sizes 可能为 [500] 或 [500, 0]）
    onChange!([500, 0]);
    // 除零守卫：0 / 0 = NaN，should not call setSplitRatio
    // 或 sizes.length < 2
    onChange!([500]);

    expect(setSplitRatioSpy).not.toHaveBeenCalled();
  });

  // ─── SVC-07: onChange 双开但 total<=0 除零守卫 ───
  it("onChange 双开但 total=0 时不写回 splitRatio（total 除零守卫）", () => {
    const setSplitRatioSpy = vi.spyOn(
      useSideBar.getState(),
      "setSplitRatio",
    );

    // 先单开渲染（bothOpen 过渡 effect 不触发）
    useSideBar.setState({
      zones: { top: ["nav"], bottom: ["explorer"] },
      open: { top: "nav", bottom: null },
      splitRatio: 0.5,
    });
    const { rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // 切双开 → bothOpen false→true 过渡 effect 写回 0.5 一次 → 清掉后再测 onChange
    useSideBar.setState({ open: { top: "nav", bottom: "explorer" } });
    rerender(React.createElement(SideBarArea, defaultProps));
    setSplitRatioSpy.mockClear();

    const onChange = mocks.getOnChange();
    expect(onChange).not.toBeNull();

    // 双开但两 pane 尺寸均为 0 → total=0 → 除零守卫直接 return，不产生 NaN 写回
    expect(() => onChange!([0, 0])).not.toThrow();
    expect(setSplitRatioSpy).not.toHaveBeenCalled();
  });

  // ─── SB-20: 背景色使用 PANEL_BG token ───
  it("外层容器背景色为 PANEL_BG token（非硬编码色值）", () => {
    const { container } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    const outerDiv = container.firstElementChild as HTMLElement;
    expect(outerDiv).not.toBeNull();
    // 背景色应为 theme token（非硬编码 #xxxxxx）
    // PANEL_BG = "#0a0a0b"
    expect(outerDiv.style.background).toBe("rgb(10, 10, 11)");
  });

  // ─── SB-20: 持久化中未注册的 id 被过滤（防御） ───
  it("持久化中未注册的 id 被过滤不渲染视图槽", () => {
    // zones 中含未注册 id "ghost"
    useSideBar.setState({
      zones: { top: ["nav", "ghost"], bottom: [] },
      open: { top: "nav", bottom: null },
    });

    const { container } = render(
      React.createElement(SideBarArea, defaultProps),
    );

    // ghost 不应产生槽
    const ghostSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-ghost"]',
    );
    expect(ghostSlot).toBeNull();

    // projects 正常渲染
    const projectsSlot = container.querySelector(
      '[data-e2e="sidebar-slot-top-nav"]',
    );
    expect(projectsSlot).not.toBeNull();
  });

  // ─── SB-20.13: 越界 splitRatio 双开过渡时回退默认 0.5 ───
  it("双开过渡时 splitRatio 越界（>SPLIT_MAX）回退默认 0.5", () => {
    const setSplitRatioSpy = vi.spyOn(
      useSideBar.getState(),
      "setSplitRatio",
    );

    // 模拟：上区打开、下区关闭、splitRatio 越界（脏数据/极端值）
    useSideBar.setState({
      splitRatio: 0.95,
      open: { top: "nav", bottom: null },
      zones: { top: ["nav"], bottom: ["explorer"] },
    });

    // 首次渲染——bothOpen=false, 不触发回退
    const { rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );
    expect(setSplitRatioSpy).not.toHaveBeenCalled();

    // 下区被打开 → bothOpen: false→true，值越界 → 回退 0.5
    useSideBar.setState({ open: { top: "nav", bottom: "explorer" } });
    rerender(
      React.createElement(SideBarArea, defaultProps),
    );

    expect(setSplitRatioSpy).toHaveBeenCalledWith(0.5);
    setSplitRatioSpy.mockRestore();
  });

  // ─── FE-19: 双开→拖比例→单开→再双开，比例保留 ───
  it("双开→拖比例→单开→再双开，splitRatio 保留用户调节值", async () => {
    const restore = mockClientHeight(600);
    try {
      const setSplitRatioSpy = vi.spyOn(
        useSideBar.getState(),
        "setSplitRatio",
      );

      // 初始双开
      useSideBar.setState({
        zones: { top: ["nav"], bottom: ["explorer"] },
        open: { top: "nav", bottom: "explorer" },
        splitRatio: 0.5,
      });

      render(React.createElement(SideBarArea, defaultProps));

      // 先走完首轮纠偏窗口，放行写回（否则拖拽 fire 被 pending 闸吞掉）
      act(() => {
        mocks.getOnChange()!([300, 300]);
      });
      await flushRaf();

      // 拖分隔条：上 300 / 下 200 → ratio 0.6 写回 store
      const onChange = mocks.getOnChange();
      expect(onChange).not.toBeNull();
      act(() => {
        onChange!([300, 200]);
      });
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.6);

      // 单开（下区关闭）→ 再双开
      act(() => {
        useSideBar.setState({ open: { top: "nav", bottom: null } });
      });
      act(() => {
        useSideBar.setState({ open: { top: "nav", bottom: "explorer" } });
      });
      await flushRaf();

      // 过渡不重置：比例保持用户调节值，0.5 未再被写入
      expect(useSideBar.getState().splitRatio).toBeCloseTo(0.6);
      expect(
        setSplitRatioSpy.mock.calls.filter((c) => c[0] === 0.5),
      ).toHaveLength(0);
      setSplitRatioSpy.mockRestore();
    } finally {
      restore();
    }
  });
});
