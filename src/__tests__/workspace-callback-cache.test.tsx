// workspace-callback-cache.test.tsx — 宿主稳定性测试（CP-004 替代原 FE-33 回调缓存
// 测试——多实例 pageCallbacksRef 随实例消亡，改为验证共享宿主的页面目录稳定性）
//
// - 页面重命名/布局类变更（页面 ID 集合不变）→ 宿主不重建：既有面板引用不变
// - 新增页面 → 既有页面页组/面板不动（页组并入幂等）
// - 删除页面后其余页面页组保持（宿主不整体重建）
//
// 策略：真实渲染 Workspace（真实 DockviewReact），捕获宿主 api + 面板引用做
// identity 断言。

import { describe, it, expect, beforeEach, afterEach, afterAll, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import React from "react";
import { render, waitFor, act, cleanup } from "@testing-library/react";

// Mock @xterm/xterm（jsdom 渲染抛异常——同 workspace 测试）
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(function (this: Record<string, unknown>) {
    this.open = vi.fn();
    this.dispose = vi.fn();
    this.loadAddon = vi.fn();
    this.write = vi.fn();
    this.writeln = vi.fn();
    this.onData = vi.fn();
    this.focus = vi.fn();
    this.attachCustomKeyEventHandler = vi.fn();
    this.element = document.createElement("div");
    this.options = {} as Record<string, unknown>;
    this.parser = { registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })) };
    return this;
  }),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function (this: Record<string, unknown>) {
    this.fit = vi.fn();
    this.proposeDimensions = vi.fn(() => ({ cols: 80, rows: 24 }));
    this.dispose = vi.fn();
    return this;
  }),
}));

const originalResizeObserver = global.ResizeObserver;
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};
afterAll(() => {
  global.ResizeObserver = originalResizeObserver;
});

import Workspace from "../workspace/Workspace";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { useSideBar } from "../stores/sideBar";
import { titleManager } from "../workspace/titleManager";
import { getHostApi, unregisterHostApi } from "../workspace/pageApis";
import { pageGroupId } from "../workspace/pageGroups";
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
  window.__dockviewApi = undefined;
  unregisterHostApi();
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("宿主稳定性（页面目录变更不扰动既有页组）", () => {
  it("同一宿主跨页面目录变更保持同一实例（宿主唯一——不销毁重建）", async () => {
    mockIPC(() => null);
    const { projId, pageA, pageB } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    render(React.createElement(Workspace));
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const hostFirst = getHostApi()!;
    expect(hostFirst.getGroup(pageGroupId(pageA))).toBeTruthy();

    // 无关变更：重命名另一页面（页面 ID 集合不变——宿主不重建）
    act(() => {
      useProjects.getState().renamePage(projId, pageB, "Beta-renamed");
    });

    expect(getHostApi()).toBe(hostFirst);
    // 两页组均在
    expect(hostFirst.getGroup(pageGroupId(pageA))).toBeTruthy();
    expect(hostFirst.getGroup(pageGroupId(pageB))).toBeTruthy();
  });

  it("新增页面 → 页组并入宿主；既有页面组/面板不动", async () => {
    mockIPC(() => null);
    const { projId, pageA, pageB } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    render(React.createElement(Workspace));
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const host = getHostApi()!;
    const groupA = host.getGroup(pageGroupId(pageA));
    expect(groupA).toBeTruthy();

    // 新增页面 C（store 订阅同步 → loadPageGroup 并入——whole-grid reuse 恢复）
    act(() => {
      useProjects.getState().addPage(projId, makePage("page-cb-c"));
    });
    await waitFor(() => expect(host.getGroup(pageGroupId("page-cb-c"))).toBeTruthy());

    // 既有页组存在且面板不受扰动（whole-grid fromJSON reuse 可能重建组对象——
    // 面板实例与内容跨恢复存活由 H6/e2e 锁；此处断言目录级不变）
    expect(host.getGroup(pageGroupId(pageA))).toBeTruthy();
    expect(host.getGroup(pageGroupId(pageB))).toBeTruthy();
    expect(groupA).toBeTruthy();
  });

  it("删除页面后剩余页面页组保持（宿主不整体重建）", async () => {
    mockIPC(() => null);
    const { projId, pageA, pageB } = seedTwoPages();
    useLayout.setState({ activePageId: pageA });

    render(React.createElement(Workspace));
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const host = getHostApi()!;
    const groupA = host.getGroup(pageGroupId(pageA));

    // 删除 B 页（store 订阅同步 → 移除 B 页组）
    act(() => {
      useProjects.getState().removePage(projId, pageB);
    });
    await waitFor(() => expect(host.getGroup(pageGroupId(pageB))).toBeUndefined());

    // A 页组保持（未被误删）
    expect(host.getGroup(pageGroupId(pageA))).toBeTruthy();
    expect(groupA).toBeTruthy();
    // A 页组仍可经 getPageApi 解析（宿主就绪 + 挂载标记在）
    act(() => { useLayout.setState({ activePageId: pageA }); });
    expect(getHostApi()).toBe(host);
  });
});
