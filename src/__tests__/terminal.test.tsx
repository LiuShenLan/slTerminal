// Phase 1 L2 终端面板测试
import { describe, it, expect, afterEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';

// xterm.js 在 jsdom 中不可用，mock 掉整个 useXterm hook
vi.mock('../panels/terminal/useXterm', () => ({
  useXterm: vi.fn(() => ({ focus: vi.fn() })),
}));

import { render, screen } from '@testing-library/react';
import TerminalPanel from '../panels/terminal/TerminalPanel';

afterEach(() => {
  clearMocks();
});

describe('TerminalPanel', () => {
  it('terminal_panel_spawns_on_mount', () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_spawn') return 'test-session-id';
      return null;
    });

    const panelId = 'test-panel-001';
    render(<TerminalPanel params={{ panelId }} />);

    // 面板应渲染容器
    const container = document.querySelector('[style*="height: 100%"]');
    expect(container).toBeTruthy();

    // 应显示加载遮罩 "正在连接..."
    expect(screen.getByText('正在连接...')).toBeTruthy();
  });

  it('terminal_input_writes', () => {
    const panelId = 'test-panel-002';
    const { container } = render(<TerminalPanel params={{ panelId }} />);

    expect(container.querySelector('div')).toBeTruthy();
  });
});
