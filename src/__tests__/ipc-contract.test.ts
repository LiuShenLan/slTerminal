// ipc-contract.test.ts — IPC wrapper 合约测试
//
// 使用 mockIPC 拦截真实的 invoke 调用，验证每个 IPC wrapper：
// 1. 命令名正确
// 2. 参数结构正确（字段名、类型）
// 3. 返回类型正确

import { describe, it, expect, afterEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { Channel } from '@tauri-apps/api/core';
import * as pty from '../ipc/pty';
import * as fs from '../ipc/fs';
import * as settings from '../ipc/settings';
// ping 测试用——index.ts 直接重导出 @tauri-apps/api/core 的 invoke
// eslint-disable-next-line no-restricted-imports
import { invoke } from '@tauri-apps/api/core';

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// PTY IPC
// ═══════════════════════════════════════════════════════════════════

describe('pty IPC 合约', () => {
  it('spawn: 应调用 pty_spawn 命令，参数包含 request 和 onOutput Channel', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'pty_spawn') return 'mock-session-01';
    });

    const request = { panelId: 'p1', cwd: 'C:\\test', cols: 120, rows: 40 };
    const onOutput = vi.fn();
    const sessionId = await pty.spawn(request, onOutput);

    expect(sessionId).toBe('mock-session-01');
    expect(spy).toHaveBeenCalledTimes(1);

    const [cmd, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe('pty_spawn');
    expect(args.request).toEqual(request);
    expect(args.onOutput).toBeInstanceOf(Channel);
    // 验证 channel.onmessage 已绑定为 onOutput 回调
    expect((args.onOutput as Channel<unknown>).onmessage).toBe(onOutput);
  });

  it('write: 应调用 pty_write 命令，data 从 Uint8Array 转换为 number[]', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    const testData = new Uint8Array([72, 101, 108, 108, 111]); // "Hello"
    await pty.write('session-1', testData);

    expect(spy).toHaveBeenCalledWith('pty_write', {
      sessionId: 'session-1',
      data: [72, 101, 108, 108, 111],
    });
  });

  it('resize: 应调用 pty_resize 命令，参数包含 sessionId, cols, rows', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.resize('session-2', 100, 30);

    expect(spy).toHaveBeenCalledWith('pty_resize', {
      sessionId: 'session-2',
      cols: 100,
      rows: 30,
    });
  });

  it('kill: 应调用 pty_kill 命令，参数包含 sessionId', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.kill('session-3');

    expect(spy).toHaveBeenCalledWith('pty_kill', {
      sessionId: 'session-3',
    });
  });

  it('getWindowsBuildNumber: 成功时返回 invoke 结果', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_windows_build_number') return 22621;
    });

    const build = await pty.getWindowsBuildNumber();
    expect(build).toBe(22621);
  });

  it('getWindowsBuildNumber: invoke 失败时 fallback 返回 21376', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_windows_build_number') throw new Error('Not on Windows');
    });

    const build = await pty.getWindowsBuildNumber();
    expect(build).toBe(21376);
  });
});

// ═══════════════════════════════════════════════════════════════════
// FS IPC
// ═══════════════════════════════════════════════════════════════════

describe('fs IPC 合约', () => {
  it('readFile: 应调用 fs_read_file 命令，参数包含 path', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'fs_read_file') return 'file content';
    });

    const content = await fs.readFile('C:\\test.txt');

    expect(content).toBe('file content');
    expect(spy).toHaveBeenCalledWith('fs_read_file', {
      path: 'C:\\test.txt',
    });
  });

  it('writeFile: 应调用 fs_write_file 命令，参数包含 path 和 content', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await fs.writeFile('C:\\output.txt', 'hello world');

    expect(spy).toHaveBeenCalledWith('fs_write_file', {
      path: 'C:\\output.txt',
      content: 'hello world',
    });
  });
});

// ═══════════════════════════════════════════════════════════════════
// Settings IPC
// ═══════════════════════════════════════════════════════════════════

describe('settings IPC 合约', () => {
  it('loadSettings: 应调用 load_settings 命令，无参数', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'load_settings') return { theme: 'dark' };
    });

    const result = await settings.loadSettings();
    expect(result).toEqual({ theme: 'dark' });
    expect(spy).toHaveBeenCalledWith('load_settings', {});
  });

  it('loadSettings: 返回 null 表示首次启动', async () => {
    mockIPC((cmd) => {
      if (cmd === 'load_settings') return null;
    });

    const result = await settings.loadSettings();
    expect(result).toBeNull();
  });

  it('saveSettings: 应调用 save_settings 命令，参数包含 settings', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await settings.saveSettings({ fontSize: 16, theme: 'dark' });
    expect(spy).toHaveBeenCalledWith('save_settings', {
      settings: { fontSize: 16, theme: 'dark' },
    });
  });

  it('saveSettings: 空对象也正常调用 invoke', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await settings.saveSettings({});
    expect(spy).toHaveBeenCalledWith('save_settings', { settings: {} });
  });
});

// ═══════════════════════════════════════════════════════════════════
// IPC ping（index.ts 重导出的 invoke）
// ═══════════════════════════════════════════════════════════════════

describe('IPC ping', () => {
  it('invoke("ping") 应返回 "pong"', async () => {
    mockIPC((cmd) => {
      if (cmd === 'ping') return 'pong';
    });

    const result = await invoke('ping');
    expect(result).toBe('pong');
  });
});
