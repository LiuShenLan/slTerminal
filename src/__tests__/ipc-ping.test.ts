import { describe, it, expect, afterEach } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
// eslint-disable-next-line no-restricted-imports
import { invoke } from '@tauri-apps/api/core';

afterEach(() => {
  clearMocks();
});

describe('IPC ping', () => {
  it('mockIPC 应拦截 ping 命令并返回 pong', async () => {
    mockIPC((cmd) => {
      if (cmd === 'ping') return 'pong';
    });

    const result = await invoke('ping');
    expect(result).toBe('pong');
  });
});
