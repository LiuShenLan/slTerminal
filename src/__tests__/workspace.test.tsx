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
  it('closing_last_panel_shows_empty_state', () => {
    mockIPC(() => null);

    const { container } = render(<Workspace />);

    // 空布局应显示 watermark 文本
    expect(container.textContent).toContain('打开终端或编辑器开始工作');
  });
});
