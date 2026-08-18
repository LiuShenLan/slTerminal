// workspace-callback-cache.test.tsx — FE-33 回调缓存测试
//
// pageCallbacksRef 回调按 pageId 惰性创建 + 缓存（getOrCreate 模式）：
// - 同一 pageId 跨渲染 onReady/onLayoutChange 引用不变（缓存生效，F2 稳定引用目标）
// - 不同 pageId 回调互不相同（按 pageId 隔离）
// - 页面重命名/布局变更等（页面 ID 集合不变）不触发回调重建（effect 依赖收窄）
// - 新增/删除页面后既有页面回调引用不变
//
// 策略：mock PageDockviewHost 整模块（Workspace.tsx 尾部 re-export 自该模块，
// mock 必须同步提供 createRightHeader/createGetContextMenu/applyRename），
// 捕获每个 pageId 最近一次收到的 onReady/onLayoutChange 引用做 identity 断言。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import React, { act } from "react";
import { render, waitFor, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  /** pageId → 最近一次接收到的回调 props */
  const callbacksByPage = new Map<
    string,
    { onReady: unknown; onLayoutChange: unknown }
  >();

  /** PageDockview 桩：捕获回调 props，不渲染 */
  const MockPageDockview = (props: {
    pageId: string;
    onReady: unknown;
    onLayoutChange: unknown;
  }) => {
    callbacksByPage.set(props.pageId, {
      onReady: props.onReady,
      onLayoutChange: props.onLayoutChange,
    });
    return null;
  };

  return { callbacksByPage, MockPageDockview };
});

vi.mock("../workspace/PageDockviewHost", () => ({
  default: mocks.MockPageDockview,
  // Workspace.tsx 尾部 `export { ... } from "./PageDockviewHost"`——mock 必须提供
  createRightHeader: vi.fn(),
  createGetContextMenu: vi.fn(),
  applyRename: vi.fn(),
}));

vi.mock("allotment", () => ({
  Allotment: Object.assign(
    ({ children }: { children?: React.ReactNode }) =>
      React.createElement("div", { "data-testid": "allotment" }, children),
    {
      Pane: ({ children }: { children?: React.ReactNode }) =>
        React.createElement("div", null, children),
    },
  ),
}));

// Workspace 的 side-effect import——测试中无需真实注册（ActivityBar 空注册表正常渲染）
vi.mock("../features/sideViews/sideViewDefs", () => ({}));
vi.mock("../features/cliProfiles/profiles", () => ({}));

vi.mock("../ipc/fs", () => ({
  setProjectRoot: vi.fn(() => Promise.resolve()),
}));

// ─── 导入（mock 之后）───
import Workspace from "../workspace/Workspace";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { useSideBar } from "../stores/sideBar";
import { titleManager } from "../workspace/titleManager";
import { getPageApi } from "../workspace/pageApis";
import type { OperationPage } from "../stores/projects";

/** 构造测试用页面（固定 pageId，便于断言） */
function makePage(pageId: string): OperationPage {
  return {
    pageId,
    name: pageId,
    layout: {},
    createdAt: Date.now(),
    lastAccessedAt: Date.now(),
  };
}

/** 种子项目（两页面） */
function seedTwoPages() {
  const projId = "proj-cb";
  const pageA = "page-cb-a";
  const pageB = "page-cb-b";
  useProjects.getState().addProject({
    projectId: projId,
    name: "callback-test",
    rootPath: "/tmp/cb",
    pages: [makePage(pageA), makePage(pageB)],
    activePageId: pageA,
    version: 1,
  });
  return { projId, pageA, pageB };
}

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  useSideBar.setState({
    zones: { top: ["nav"], bottom: [] },
    open: { top: "nav", bottom: null },
    width: 250,
    splitRatio: 0.5,
    loaded: true,
  });
  titleManager.reset();
  mocks.callbacksByPage.clear();
});

afterEach(() => {
  cleanup();
});

describe("FE-33 回调缓存（getOrCreate 模式）", () => {
  it("同一 pageId 跨渲染 onReady/onLayoutChange 引用不变（缓存生效）", async () => {
    const { projId, pageA } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { rerender } = render(React.createElement(Workspace));

    // A 页初始化后捕获回调
    await waitFor(() => expect(mocks.callbacksByPage.get(pageA)).toBeTruthy());
    const first = mocks.callbacksByPage.get(pageA)!;

    // 无关变更：重命名另一页面（allPages 内容变化但页面 ID 集合不变）
    act(() => {
      useProjects.getState().renamePage(projId, "page-cb-b", "Beta-renamed");
    });
    rerender(React.createElement(Workspace));

    const second = mocks.callbacksByPage.get(pageA)!;
    expect(second.onReady).toBe(first.onReady);
    expect(second.onLayoutChange).toBe(first.onLayoutChange);
  });

  it("不同 pageId 的回调引用互不相同（按 pageId 隔离），新增页面不重建既有回调", async () => {
    const { projId, pageA, pageB } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { rerender } = render(React.createElement(Workspace));

    await waitFor(() => expect(mocks.callbacksByPage.get(pageA)).toBeTruthy());
    const aFirst = mocks.callbacksByPage.get(pageA)!;

    // 切换并初始化 B 页
    act(() => {
      useLayout.setState({ activePageId: pageB });
    });
    await waitFor(() => expect(mocks.callbacksByPage.get(pageB)).toBeTruthy());

    // 按 pageId 隔离：A/B 回调互不相同
    const b = mocks.callbacksByPage.get(pageB)!;
    expect(b.onReady).not.toBe(aFirst.onReady);
    expect(b.onLayoutChange).not.toBe(aFirst.onLayoutChange);

    // 新增页面 C（ID 集合变化触发 effect，但 A 仍在集合内不清理不重建）
    act(() => {
      useProjects.getState().addPage(projId, makePage("page-cb-c"));
    });
    rerender(React.createElement(Workspace));

    const aSecond = mocks.callbacksByPage.get(pageA)!;
    expect(aSecond.onReady).toBe(aFirst.onReady);
    expect(aSecond.onLayoutChange).toBe(aFirst.onLayoutChange);
  });

  it("删除页面后剩余页面回调引用不变（清理 effect 不误删存活页面）", async () => {
    const { projId, pageA, pageB } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { rerender } = render(React.createElement(Workspace));

    await waitFor(() => expect(mocks.callbacksByPage.get(pageA)).toBeTruthy());
    const aFirst = mocks.callbacksByPage.get(pageA)!;

    // 删除 B 页（ID 集合变化 → 清理 effect 运行）
    act(() => {
      useProjects.getState().removePage(projId, pageB);
    });
    rerender(React.createElement(Workspace));

    // A 页回调引用保持（未被误删重建）
    const aSecond = mocks.callbacksByPage.get(pageA)!;
    expect(aSecond.onReady).toBe(aFirst.onReady);
    expect(aSecond.onLayoutChange).toBe(aFirst.onLayoutChange);
    // B 页已删除——无 Dockview 实例（回调清理后不可见，页面不可达）
    expect(getPageApi(pageB)).toBeUndefined();
  });
});
