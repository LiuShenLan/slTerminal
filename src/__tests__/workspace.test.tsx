// L2 Workspace 测试 — 多 Dockview 实例架构

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';

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

// jsdom 缺少 ResizeObserver（Dockview 依赖）
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

import { render } from '@testing-library/react';
import Workspace from '../workspace/Workspace';
import { useProjects } from '../stores/projects';
import { useLayout } from '../stores/layout';
import { useSideBar } from '../stores/sideBar';
import { titleManager } from '../workspace/titleManager';

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  // 种子侧栏 store 默认值（Workspace 三栏改造后依赖 sideBar 状态）
  useSideBar.setState({
    zones: { top: ["projects", "explorer"], bottom: [] },
    open: { top: "projects", bottom: null },
    width: 250,
    splitRatio: 0.5,
    loaded: true,
  });
  titleManager.reset();
});

afterEach(() => {
  clearMocks();
});

describe('Workspace', () => {
  it('无项目时：只渲染侧栏空白态，不创建 Dockview', () => {
    mockIPC(() => null);

    const { container } = render(<Workspace />);

    const text = container.textContent ?? '';
    // 侧栏渲染
    expect(text).toContain('添加项目');
    // 无 Watermark（无 Dockview 实例因为无 active page）
    expect(text).not.toContain('打开终端或编辑器开始工作');
  });

  it('有项目无页面时：侧栏渲染项目名，但不创建 Dockview', () => {
    mockIPC(() => null);

    useProjects.getState().addProject({
      projectId: 'test-proj',
      name: 'test-project-name',
      rootPath: '/tmp/test',
      pages: [],
      activePageId: null,
      version: 1,
    });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? '';
    expect(text).toContain('test-project-name');
    // activePageId 为 null → 无 Dockview → 无 Watermark
    expect(text).not.toContain('打开终端或编辑器开始工作');
  });

  it('T17: 活跃页面 + layout 为空 → Watermark 显示（不自动创建终端）', () => {
    mockIPC(() => null);

    const pageId = 'page-test-empty';
    useProjects.getState().addProject({
      projectId: 'proj-empty',
      name: 'empty-layout-test',
      rootPath: '/tmp/empty',
      pages: [{
        pageId,
        name: 'Empty Page',
        layout: {},
        cwd: '/tmp/empty',
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      }],
      activePageId: pageId,
      version: 1,
    });
    useLayout.setState({ activePageId: pageId });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? '';
    // 侧栏渲染
    expect(text).toContain('empty-layout-test');
    // Watermark 显示（无默认终端）
    expect(text).toContain('打开终端或编辑器开始工作');
    // 不创建 terminal 面板
    expect(text).not.toMatch(/terminal-/);
  });

  it('T18: 活跃页面 + layout 为空 → 文本不含 terminal-', () => {
    mockIPC(() => null);

    const pageId = 'page-no-term';
    useProjects.getState().addProject({
      projectId: 'proj-no-term',
      name: 'no-terminal',
      rootPath: '/tmp/no-term',
      pages: [{
        pageId,
        name: 'No Terminal',
        layout: {},
        cwd: '/tmp/no-term',
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      }],
      activePageId: pageId,
      version: 1,
    });
    useLayout.setState({ activePageId: pageId });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? '';
    // 确认 terminal 面板标题不在 DOM 中
    expect(text).not.toContain('terminal-');
    // Watermark 仍然可见
    expect(text).toContain('打开终端或编辑器开始工作');
  });
});
