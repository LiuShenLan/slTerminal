// workspace-multi-instance.test.tsx — 多 Dockview 实例集成测试
//
// 验证多页面切换时 Dockview 实例各自存活、CSS 显隐正确、惰性初始化按需创建。
// H6（终端跨页面存活）核心语义：页面切换通过 CSS display 显隐，实例不销毁重建——
// 断言 getPageApi(pageId) 返回同一 api 对象（identity）+ 终端面板不 dispose（WRK-09）。
// 注：jsdom 中 DockviewReact 的 onReady 会创建终端面板，React StrictMode 双重挂载
// 会导致多个 Terminal 实例，测试仅验证架构层面的行为（panel ID 存在性 + DOM 结构）。

import { describe, it, expect, beforeEach, afterEach, afterAll, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render, waitFor, act, fireEvent, cleanup } from "@testing-library/react";

// Mock @xterm/xterm — xterm.js 6.1+ 渲染器初始化在 jsdom 中抛异常（WidthCache 需要真实 DOM 指标）
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

// 模块级 stub 须 afterAll 恢复——防同 worker 后续文件被污染（TQ-A-02）
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
import { getPageApi } from "../workspace/pageApis";

/** 构造带两页面的测试项目（展开态） */
function setupTwoPages() {
  const projId = "proj-multi";
  const pageA = "page-alpha";
  const pageB = "page-beta";

  useProjects.getState().addProject({
    projectId: projId,
    name: "multi-test",
    rootPath: "/tmp/multi",
    pages: [
      { pageId: pageA, name: "Alpha", layout: {}, cwd: "/tmp/multi",
        createdAt: Date.now(), lastAccessedAt: Date.now() },
      { pageId: pageB, name: "Beta", layout: {}, cwd: "/tmp/multi",
        createdAt: Date.now(), lastAccessedAt: Date.now() },
    ],
    activePageId: pageA,
    version: 1,
  });

  const expanded = useProjects.getState().expandedNodes;
  useProjects.setState({ expandedNodes: { ...expanded, [projId]: true } });

  return { projId, pageA, pageB };
}

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  // 种子侧栏 store 默认值（Workspace 三栏改造后依赖 sideBar 状态）
  useSideBar.setState({
    zones: { top: ["nav", "explorer"], bottom: [] },
    open: { top: "nav", bottom: null },
    width: 250,
    splitRatio: 0.5,
    loaded: true,
  });
  titleManager.reset();
});

afterEach(() => {
  // RTL 无 auto-cleanup——Dockview 实例跨测试残留会污染 DOM 断言（如 /terminal-/），
  // 且新实例 onReady 会覆盖 pageApiMap 注册，必须显式卸载
  cleanup();
  clearMocks();
});

describe("多 Dockview 实例——架构验证", () => {
  it("T21: 活跃页面 layout:{} → Watermark 显示（不自动创建终端）", () => {
    mockIPC(() => null);

    const { pageA } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { container } = render(<Workspace />);
    const text = container.textContent ?? "";
    // Watermark 文本可见（空布局不创建默认终端）
    expect(text).toContain("打开终端或编辑器开始工作");
    // 不创建 terminal 面板
    expect(text).not.toMatch(/terminal-/);
  });

  it("2. 无活跃页面时不应初始化任何 Dockview", () => {
    mockIPC(() => null);

    setupTwoPages();
    useLayout.setState({ activePageId: null });

    const { container } = render(<Workspace />);
    const text = container.textContent ?? "";
    // 侧栏正常渲染
    expect(text).toContain("multi-test");
    // 无 Dockview → 无终端面板（标题格式为 "terminal-N"）
    expect(text).not.toMatch(/terminal-/);
  });

  it("3. 项目无页面时也不应创建 Dockview", () => {
    mockIPC(() => null);

    useProjects.getState().addProject({
      projectId: "empty-proj",
      name: "empty-project",
      rootPath: "/tmp/empty",
      pages: [],
      activePageId: null,
      version: 1,
    });
    useLayout.setState({ activePageId: null });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? "";
    expect(text).toContain("empty-project");
    // 无页面 → 无 Dockview → 无 terminal
    expect(text).not.toContain("terminal-");
  });

  it("4. 删除活跃页面后，store 正确更新 activePageId", () => {
    const { projId, pageA, pageB } = setupTwoPages();

    useProjects.getState().switchToPage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageA);

    // 删除活跃页面 → store 自动切换到剩余页面
    useProjects.getState().removePage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageB);
  });

  it("H6 实例存活：切换页面后同一 pageId 返回同一 api 对象（不销毁重建）", async () => {
    mockIPC(() => null);

    const { pageA, pageB } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    render(<Workspace />);

    // A 页 onReady → registerPageApi + handlePageApiReady 重指 __dockviewApi
    await waitFor(() => expect(getPageApi(pageA)).toBeTruthy());
    const apiA = getPageApi(pageA) as NonNullable<ReturnType<typeof getPageApi>>;
    expect(window.__dockviewApi).toBe(apiA);

    // 切到 B 页（惰性初始化）
    act(() => { useLayout.setState({ activePageId: pageB }); });
    await waitFor(() => expect(getPageApi(pageB)).toBeTruthy());
    const apiB = getPageApi(pageB) as NonNullable<ReturnType<typeof getPageApi>>;

    // H6 核心语义：A 实例未销毁（同一引用），B 是新实例
    expect(getPageApi(pageA)).toBe(apiA);
    expect(apiB).not.toBe(apiA);
    // handlePageApiReady 兜底重指活跃页
    expect(window.__dockviewApi).toBe(apiB);

    // 切回 A：仍是原实例
    act(() => { useLayout.setState({ activePageId: pageA }); });
    expect(getPageApi(pageA)).toBe(apiA);
  });

  it("H6 终端不 dispose：页面切换往返后终端面板仍存活", async () => {
    mockIPC(() => null);

    const { pageA, pageB } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { container } = render(<Workspace />);

    await waitFor(() => expect(getPageApi(pageA)).toBeTruthy());
    const apiA = getPageApi(pageA) as NonNullable<ReturnType<typeof getPageApi>>;

    // 经 Watermark 按钮创建终端面板（真实 addPanel → TerminalPanel 挂载）
    const newTermBtn = Array.from(container.querySelectorAll("button")).find(
      (b) => b.textContent === "新建终端",
    );
    expect(newTermBtn).toBeTruthy();
    act(() => { fireEvent.click(newTermBtn as HTMLButtonElement); });

    const panelId = `terminal-${pageA}-0`;
    expect(apiA.getPanel(panelId)).toBeTruthy();

    // 切到 B 再切回 A
    act(() => { useLayout.setState({ activePageId: pageB }); });
    await waitFor(() => expect(getPageApi(pageB)).toBeTruthy());
    act(() => { useLayout.setState({ activePageId: pageA }); });

    // 面板未销毁（H6：页面切换不杀 PTY/面板），实例未重建
    expect(apiA.getPanel(panelId)).toBeTruthy();
    expect(getPageApi(pageA)).toBe(apiA);
  });
});
