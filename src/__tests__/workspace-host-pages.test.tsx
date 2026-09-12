// workspace-host-pages.test.tsx — 共享宿主多页面集成测试（CP-004 替代原
// workspace-multi-instance.test.tsx——多 Dockview 实例架构消亡，改页组语义）
//
// 验证多页面共存于单一宿主（每页一个顶级页组）时：各页组各自存活（同宿主
// API 内可查）、页面切换不销毁面板（H6：终端跨页面存活——页组可见性切换，
// 面板不卸载）、空页组由 Watermark 接管、无活跃页时宿主隐藏。
// 注：jsdom 中真实 dockview 可跑（同 workspace-page-dockview 集成验证），
// 测试仅验证架构层面行为（页组/面板存在性 + DOM 结构）。

import { describe, it, expect, beforeEach, afterEach, afterAll, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render, waitFor, act, fireEvent, cleanup } from "@testing-library/react";

// Mock @xterm/xterm — xterm.js 6.1+ 渲染器初始化在 jsdom 中抛异常
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
import { getPageApi, getHostApi } from "../workspace/pageApis";
import { resetTerminalPanelSeq } from "../lib/panelId";

/** 构造带两页面的测试项目（展开态，空布局占位——宿主恢复时归空页组） */
function setupTwoPages() {
  const projId = "proj-multi";
  const pageA = "page-alpha";
  const pageB = "page-beta";

  useProjects.getState().addProject({
    projectId: projId,
    name: "multi-test",
    rootPath: "/tmp/multi",
    pages: [
      { pageId: pageA, name: "Alpha", layout: {},
        createdAt: Date.now(), lastAccessedAt: Date.now() },
      { pageId: pageB, name: "Beta", layout: {},
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
  resetTerminalPanelSeq();
  window.__dockviewApi = undefined;
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("共享宿主——多页组架构验证", () => {
  it("T21: 活跃页面空布局 → Watermark 接管（不自动创建终端）", async () => {
    mockIPC(() => null);

    const { pageA } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { container } = render(<Workspace />);
    // 宿主就绪后：空页组由 Watermark 接管显示（页面切换可见性单点保证）
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const text = container.textContent ?? "";
    expect(text).toContain("打开终端或编辑器开始工作");
    // 不创建 terminal 面板
    expect(text).not.toMatch(/terminal-/);
  });

  it("2. 无活跃页面时宿主隐藏（display:none 空白主区——terminal 不卸载）", async () => {
    mockIPC(() => null);

    setupTwoPages();
    useLayout.setState({ activePageId: null });

    const { container } = render(<Workspace />);
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const text = container.textContent ?? "";
    // 侧栏正常渲染
    expect(text).toContain("multi-test");
    // 无活跃页 → 宿主容器 display:none（无 Dockview 内容可见）
    const host = container.querySelector(".slterm-dock-host") as HTMLElement | null;
    expect(host).toBeTruthy();
    expect(host!.style.display).toBe("none");
  });

  it("3. 项目无页面时宿主隐藏（无页组可显）", async () => {
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
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    const text = container.textContent ?? "";
    expect(text).toContain("empty-project");
    expect(text).not.toContain("terminal-");
    const host = container.querySelector(".slterm-dock-host") as HTMLElement | null;
    expect(host!.style.display).toBe("none");
  });

  it("4. 删除活跃页面后，store 正确更新 activePageId", () => {
    const { projId, pageA, pageB } = setupTwoPages();

    useProjects.getState().switchToPage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageA);

    // 删除活跃页面 → store 自动切换到剩余页面
    useProjects.getState().removePage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageB);
  });

  it("H6 宿主唯一：getPageApi 各页均返回同一宿主 api（页组挂载后）", async () => {
    mockIPC(() => null);

    const { pageA, pageB } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    render(<Workspace />);

    // 两页页组恢复后：getPageApi 均解析到同一宿主 API（多实例 map 消亡）
    await waitFor(() => expect(getPageApi(pageA)).toBeTruthy());
    await waitFor(() => expect(getPageApi(pageB)).toBeTruthy());
    const apiA = getPageApi(pageA)!;
    const apiB = getPageApi(pageB)!;
    expect(apiA).toBe(apiB);
    expect(apiA).toBe(getHostApi());
    // __dockviewApi 恒为宿主（重指不变量收敛——宿主唯一）
    expect(window.__dockviewApi).toBe(apiA);

    // 切页：宿主/API 引用不变（不销毁重建）
    act(() => { useLayout.setState({ activePageId: pageB }); });
    expect(getPageApi(pageA)).toBe(apiA);
    expect(window.__dockviewApi).toBe(apiA);
  });

  it("H6 终端不 dispose：页面切换往返后终端面板仍存活（页组可见性切换不卸载）", async () => {
    mockIPC(() => null);

    const { pageA, pageB } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { container } = render(<Workspace />);
    await waitFor(() => expect(getPageApi(pageA)).toBeTruthy());
    const hostApi = getHostApi()!;

    // 经 Watermark 按钮创建终端面板（真实 addPanel → TerminalPanel 挂载）
    const newTermBtn = Array.from(container.querySelectorAll("button")).find(
      (b) => b.textContent === "新建终端",
    );
    expect(newTermBtn).toBeTruthy();
    act(() => { fireEvent.click(newTermBtn as HTMLButtonElement); });

    // 页前缀协议面板 id（活跃页 page-alpha）
    const panelId = `${pageA}:terminal-0`;
    await waitFor(() => expect(hostApi.getPanel(panelId)).toBeTruthy());

    // 切到 B 再切回 A（页组可见性切换——面板不销毁）
    act(() => { useLayout.setState({ activePageId: pageB }); });
    act(() => { useLayout.setState({ activePageId: pageA }); });

    expect(hostApi.getPanel(panelId)).toBeTruthy();
    // 面板仍在 page-alpha 页组（未跨页移动）
    expect(hostApi.getPanel(panelId)!.group.id).toBe(`page-${pageA}`);
  });

  it("FE-09 宿主卸载消费 disposables：__dockviewApi 置空 + 宿主 API 注销", async () => {
    mockIPC(() => null);

    const { pageA } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { unmount } = render(<Workspace />);
    // 宿主就绪：全局 API 与页组查询均可用
    await waitFor(() => expect(getHostApi()).toBeTruthy());
    expect(window.__dockviewApi).toBeTruthy();
    expect(getPageApi(pageA)).toBeTruthy();

    // 卸载宿主 → 自定义清理生效（apiRef/__dockviewApi/unregisterHostApi 置空）
    unmount();
    expect(window.__dockviewApi).toBeUndefined();
    // getPageApi 未就绪契约 = undefined（宿主注销 + 挂载标记清空）
    expect(getPageApi(pageA)).toBeUndefined();
    expect(getHostApi()).toBeNull();
  });
});
