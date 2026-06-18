import { describe, it, expect, afterEach } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { invoke } from '../ipc/index';

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
