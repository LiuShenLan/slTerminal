// sidebar-area-viewstate.test.tsx — SideBarArea 视图状态槽（CP-016）透传测试
//
// 验证：
// - viewState/onViewStateChange 两 prop 透传到视图组件（受控消费方向 外→内 / 内→外）
// - onViewStateChange 上呼后注册表 getViewState(id) 可读回
// - 换区/槽位切换重建（卸载→重挂载）后，新实例经注册表状态槽回填恢复
//
// Mock allotment 为渲染 children 的 stub（照 sideBarArea.test.tsx 模式）。

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const MockAllotment: any = ({ children }: any) =>
    React.createElement(
      "div",
      { "data-testid": "allotment-vertical" },
      children,
    );

  MockAllotment.Pane = ({ children }: any) =>
    React.createElement(
      "div",
      { "data-testid": "allotment-pane" },
      children,
    );

  return { MockAllotment };
});

vi.mock("allotment", () => ({
  Allotment: mocks.MockAllotment,
}));

// 阻止 store subscribe 触发真实 saveSettings（loaded=false 已足够，但显式 mock 更安全）
vi.mock("../../ipc/settings", () => ({
  loadSettings: vi.fn(() => Promise.resolve({ data: null, corrupted: false })),
  saveSettings: vi.fn(() => Promise.resolve()),
}));

// 阻断 side-effect 注册：本文件自注册 stub 视图（TQ-B-02 隔离）
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

/** 假视图状态（FileTreeViewState 形态，注册表槽位内容为 unknown 透传） */
const STORED_STATE = {
  rootPath: "C:/project",
  expandedPaths: ["C:/project/src"],
};

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

/** 默认 props */
const defaultProps: SideBarAreaProps = {
  switchToPage: vi.fn(),
  onDeletePage: vi.fn(),
};

// ─── Tests ───

describe("SideBarArea — CP-016 视图状态槽透传", () => {
  /** 每次挂载捕获一份视图 props（重建即新增条目） */
  let captured: SideViewComponentProps[] = [];

  /** 注册捕获式 stub 视图：挂载时将 props 记入 captured */
  function registerCaptureView() {
    const CaptureView: React.FC<SideViewComponentProps> = (props) => {
      captured.push(props);
      return React.createElement("div", { "data-testid": "capture-view" });
    };
    CaptureView.displayName = "CaptureView";
    sideViewRegistry.register({
      id: "vs",
      title: "捕获视图",
      icon: () => null,
      component: CaptureView,
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    captured = [];
    sideViewRegistry._reset();
    resetStore();
    // 视图注册在 top 区打开
    useSideBar.setState({
      zones: { top: ["vs"], bottom: [] },
      open: { top: "vs", bottom: null },
    });
  });

  it("viewState/onViewStateChange 透传到视图组件（外→内回填方向）", () => {
    sideViewRegistry.setViewState("vs", STORED_STATE);
    registerCaptureView();

    render(React.createElement(SideBarArea, defaultProps));

    expect(captured).toHaveLength(1);
    // viewState = 注册表状态槽当前值（同引用——槽位回填）
    expect(captured[0].viewState).toBe(STORED_STATE);
    expect(captured[0].onViewStateChange).toBeTypeOf("function");
    // 既有两 prop 照常透传
    expect(captured[0].switchToPage).toBe(defaultProps.switchToPage);
    expect(captured[0].onDeletePage).toBe(defaultProps.onDeletePage);
  });

  it("视图经 onViewStateChange 上呼后注册表 getViewState(id) 可读回（内→外持久化方向）", () => {
    registerCaptureView();

    render(React.createElement(SideBarArea, defaultProps));
    expect(sideViewRegistry.getViewState("vs")).toBeUndefined();

    const payload = { ...STORED_STATE };
    act(() => {
      captured[0].onViewStateChange!(payload);
    });
    expect(sideViewRegistry.getViewState("vs")).toBe(payload);
  });

  it("重建（槽位切换卸载→重挂载）后新实例经状态槽回填 viewState", () => {
    sideViewRegistry.setViewState("vs", STORED_STATE);
    registerCaptureView();

    const { rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );
    expect(captured).toHaveLength(1);

    // 槽位关闭（视图卸载）→ 再打开（视图重建）
    useSideBar.setState({ open: { top: null, bottom: null } });
    rerender(React.createElement(SideBarArea, defaultProps));
    expect(captured).toHaveLength(1); // 卸载无新挂载

    useSideBar.setState({ open: { top: "vs", bottom: null } });
    rerender(React.createElement(SideBarArea, defaultProps));

    // 重建实例的 viewState = 注册表状态槽（换区/切换重建后回填恢复）
    expect(captured).toHaveLength(2);
    expect(captured[1].viewState).toBe(STORED_STATE);
  });

  it("重建后新实例上呼经 onViewStateChange 覆盖同 id 槽位（最新状态优先）", () => {
    registerCaptureView();

    const { rerender } = render(
      React.createElement(SideBarArea, defaultProps),
    );
    expect(captured).toHaveLength(1);

    // 第一次挂载期间上呼一次
    act(() => {
      captured[0].onViewStateChange!(STORED_STATE);
    });

    // 卸载 → 重建
    useSideBar.setState({ open: { top: null, bottom: null } });
    rerender(React.createElement(SideBarArea, defaultProps));
    useSideBar.setState({ open: { top: "vs", bottom: null } });
    rerender(React.createElement(SideBarArea, defaultProps));

    // 重建实例回填第一次挂载上呼的状态；继续上呼新状态覆盖同 id 槽位
    expect(captured[1].viewState).toBe(STORED_STATE);
    const newer = { ...STORED_STATE, expandedPaths: [] };
    act(() => {
      captured[1].onViewStateChange!(newer);
    });
    expect(sideViewRegistry.getViewState("vs")).toBe(newer);
  });
});
