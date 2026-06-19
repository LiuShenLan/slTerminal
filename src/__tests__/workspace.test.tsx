// Phase 1 L2 Workspace 测试
import { describe, it, expect, afterEach } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';

// jsdom 缺少 ResizeObserver（Dockview 依赖）
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

import { render } from '@testing-library/react';
import Workspace from '../workspace/Workspace';

afterEach(() => {
  clearMocks();
});

describe('Workspace', () => {
  it('onReady 自动创建终端面板', () => {
    mockIPC(() => null);

    const { container } = render(<Workspace />);

    // F1: onReady 自动创建终端 + Watermark 含按钮（终端创建前瞬间可见）
    // 终端面板创建后显示 tab 标签和加载遮罩
    const text = container.textContent ?? '';
    expect(text).toContain('terminal-init'); // 自动创建的终端 tab
    expect(text).toContain('正在连接...'); // 加载遮罩
  });
});
