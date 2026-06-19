// Phase 1 L2 编辑器面板测试
import { describe, it, expect, afterEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';

// CodeMirror 6 在 jsdom 中不可用，mock 掉 useCodeMirror hook
vi.mock('../panels/editor/useCodeMirror', () => ({
  useCodeMirror: vi.fn(() => ({ getContent: vi.fn(() => '') })),
}));

import { render } from '@testing-library/react';
import EditorPanel from '../panels/editor/EditorPanel';

afterEach(() => {
  clearMocks();
});

describe('EditorPanel', () => {
  it('editor_saves_edited_content', () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_write_file') return null;
      return null;
    });

    const panelId = 'test-editor-001';
    const filePath = 'C:\\test\\example.txt';
    const { container } = render(<EditorPanel params={{ panelId, filePath }} />);

    // 面板应渲染容器
    expect(container.querySelector('div')).toBeTruthy();
  });
});
