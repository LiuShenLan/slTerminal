// Phase 1 L2 Workspace 测试
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';

// jsdom 缺少 ResizeObserver（Dockview 依赖）
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

import { render } from '@testing-library/react';
import Workspace from '../workspace/Workspace';
import { useProjects } from '../stores/projects';

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
});

afterEach(() => {
  clearMocks();
});

describe('Workspace', () => {
  it('B1 修复：无项目时不自动创建终端，显示空白 Watermark', () => {
    mockIPC(() => null);

    const { container } = render(<Workspace />);

    // B1: store 为空时不应自动创建终端面板
    // 应显示 Watermark 空态和侧栏空白态
    const text = container.textContent ?? '';
    expect(text).toContain('打开终端或编辑器开始工作'); // Watermark
    expect(text).toContain('添加项目'); // 侧栏工具栏
  });

  it('有项目时 onReady 自动创建终端面板', () => {
    mockIPC(() => null);

    // 预先在 store 中添加一个项目
    const projId = 'test-proj-onready';
    useProjects.getState().addProject({
      projectId: projId,
      name: 'test',
      rootPath: '/tmp/test',
      pages: [],
      activePageId: null,
      version: 1,
    });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? '';
    expect(text).toContain('terminal-init'); // 有项目时自动创建终端

    // 清理
    useProjects.getState().removeProject(projId);
  });
});
