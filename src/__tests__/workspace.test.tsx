// Phase 1 L2 Workspace 测试 — 多 Dockview 实例架构

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
import { titleManager } from '../workspace/titleManager';

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
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

  it('有活跃页面时：Dockview 初始化 + 自动创建终端面板', () => {
    mockIPC(() => null);

    const pageId = 'page-test-active';
    useProjects.getState().addProject({
      projectId: 'proj-active',
      name: 'active-test',
      rootPath: '/tmp/active',
      pages: [{
        pageId,
        name: 'Test Page',
        layout: {},
        cwd: '/tmp/active',
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
    expect(text).toContain('active-test');
    // 默认终端面板已创建——标题为 "terminal-N"（N 受 StrictMode 双重挂载影响）
    expect(text).toMatch(/terminal-\d/);
    // 终端已挂载（Watermark 被面板替换，不再显示）
    expect(text).not.toContain('打开终端或编辑器开始工作');
  });
});
