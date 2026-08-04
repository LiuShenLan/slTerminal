// ipc-ping.test.ts — ping() wrapper 健康检查命令测试（IHE-07①）
//
// 改调 src/ipc/index.ts 导出的 ping() wrapper（非裸 invoke）——
// 与架构硬约束 #1（invoke 只出现在 src/ipc/）保持一致。

import { describe, it, expect, afterEach } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { ping } from '../ipc';

afterEach(() => {
  clearMocks();
});

describe('IPC ping', () => {
  it('ping() wrapper 应调用 ping 命令并返回 pong', async () => {
    mockIPC((cmd) => {
      if (cmd === 'ping') return 'pong';
    });

    const result = await ping();
    expect(result).toBe('pong');
  });

  it('ping() wrapper: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'ping') throw new Error('IPC 链路中断');
    });

    await expect(ping()).rejects.toThrow('IPC 链路中断');
  });
});
