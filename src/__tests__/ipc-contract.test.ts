// ipc-contract.test.ts — IPC wrapper 合约测试
//
// 使用 mockIPC 拦截真实的 invoke 调用，验证每个 IPC wrapper：
// 1. 命令名正确
// 2. 参数结构正确（字段名、类型）
// 3. 返回类型正确

import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { Channel } from '@tauri-apps/api/core';

// setup.ts 全局 mock 了 ../ipc/notify → 本测试需要真实 startWatch 实现
vi.mock("../ipc/notify", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/notify")>();
});

// Mock @tauri-apps/api/event — onFsEvent 依赖 listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// Mock @tauri-apps/api/window — registerCloseHandler 依赖 getCurrentWindow
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
}));

// eslint-disable-next-line no-restricted-imports
import { listen } from "@tauri-apps/api/event";
// eslint-disable-next-line no-restricted-imports
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as pty from '../ipc/pty';
import * as fs from '../ipc/fs';
import * as settings from '../ipc/settings';
import * as notify from '../ipc/notify';
import * as git from '../ipc/git';
import * as windowIpc from '../ipc/window';
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
    await pty.write('session-1', 'panel-1', testData);

    expect(spy).toHaveBeenCalledWith('pty_write', {
      sessionId: 'session-1',
      panelId: 'panel-1',
      data: [72, 101, 108, 108, 111],
    });
  });

  it('resize: 应调用 pty_resize 命令，参数包含 sessionId, panelId, cols, rows', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.resize('session-2', 'panel-2', 100, 30);

    expect(spy).toHaveBeenCalledWith('pty_resize', {
      sessionId: 'session-2',
      panelId: 'panel-2',
      cols: 100,
      rows: 30,
    });
  });

  it('kill: 应调用 pty_kill 命令，参数包含 sessionId 和 panelId', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.kill('session-3', 'panel-3');

    expect(spy).toHaveBeenCalledWith('pty_kill', {
      sessionId: 'session-3',
      panelId: 'panel-3',
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

  // ── 异常路径 ──────────────────────────────────────────────

  it('spawn: invoke 失败时异常应传播给调用方', async () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_spawn') throw new Error('conpty init failed');
    });

    await expect(
      pty.spawn({ panelId: 'p1', cwd: 'C:\\tmp', cols: 80, rows: 24 }, vi.fn()),
    ).rejects.toThrow('conpty init failed');
  });

  it('write: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_write') throw new Error('session closed');
    });

    await expect(
      pty.write('session-1', 'panel-1', new Uint8Array([65])),
    ).rejects.toThrow('session closed');
  });

  it('resize: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_resize') throw new Error('invalid session');
    });

    await expect(pty.resize('bad-session', 'panel-1', 100, 30)).rejects.toThrow('invalid session');
  });

  it('kill: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_kill') throw new Error('already dead');
    });

    await expect(pty.kill('dead-session', 'panel-1')).rejects.toThrow('already dead');
  });

  // ── reattach（Channel 替换 + ring buffer 回放）────────────

  it('reattach: 应调用 pty_reattach 命令，参数包含 sessionId 和 onOutput Channel', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    const onOutput = vi.fn();
    await pty.reattach('session-9', onOutput);

    expect(spy).toHaveBeenCalledTimes(1);

    const [cmd, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe('pty_reattach');
    expect(args.sessionId).toBe('session-9');
    expect(args.onOutput).toBeInstanceOf(Channel);
    // 验证 channel.onmessage 已绑定为 onOutput 回调
    expect((args.onOutput as Channel<unknown>).onmessage).toBe(onOutput);
  });

  it('reattach: invoke 失败时异常应传播给调用方', async () => {
    mockIPC((cmd) => {
      if (cmd === 'pty_reattach') throw new Error('session not found');
    });

    await expect(
      pty.reattach('ghost-session', vi.fn()),
    ).rejects.toThrow('session not found');
  });
});

// ── PTY 命令 payload 契约守卫 ───────────────────────────
// 断言每个命令 invoke payload 的键集合精确匹配，
// 任何未来单边加/减键立即红。

describe('PTY 命令 payload 契约守卫', () => {
  it('pty_write: payload 键恰好为 sessionId/panelId/data', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.write('s1', 'p1', new Uint8Array([65]));

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args).sort()).toEqual(['data', 'panelId', 'sessionId']);
  });

  it('pty_resize: payload 键恰好为 sessionId/panelId/cols/rows', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.resize('s2', 'p2', 100, 30);

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args).sort()).toEqual(['cols', 'panelId', 'rows', 'sessionId']);
  });

  it('pty_kill: payload 键恰好为 sessionId/panelId', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await pty.kill('s3', 'p3');

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args).sort()).toEqual(['panelId', 'sessionId']);
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

  // ── 异常路径 ──────────────────────────────────────────────

  it('readFile: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_read_file') throw new Error('access denied');
    });

    await expect(fs.readFile('C:\\protected.txt')).rejects.toThrow('access denied');
  });

  it('writeFile: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_write_file') throw new Error('disk full');
    });

    await expect(fs.writeFile('C:\\full.txt', 'data')).rejects.toThrow('disk full');
  });

  // ── 目录操作 ──────────────────────────────────────────────

  it('readDir: 应调用 fs_read_dir 命令，参数包含 path', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'fs_read_dir') return [
        { name: 'src', path: 'C:/test/src', isDir: true },
        { name: 'README.md', path: 'C:/test/README.md', isDir: false, size: 1024, modified: 1700000000000 },
      ];
    });

    const entries = await fs.readDir('C:\\test');

    expect(entries).toHaveLength(2);
    expect(entries[0].name).toBe('src');
    expect(entries[0].isDir).toBe(true);
    expect(spy).toHaveBeenCalledWith('fs_read_dir', { path: 'C:\\test' });
  });

  it('readDir: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_read_dir') throw new Error('path not found');
    });

    await expect(fs.readDir('C:\\nope')).rejects.toThrow('path not found');
  });

  it('createDir: 应调用 fs_create_dir 命令，参数包含 path', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await fs.createDir('C:\\new-folder');

    expect(spy).toHaveBeenCalledWith('fs_create_dir', { path: 'C:\\new-folder' });
  });

  it('createDir: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_create_dir') throw new Error('permission denied');
    });

    await expect(fs.createDir('C:\\protected')).rejects.toThrow('permission denied');
  });

  it('deleteEntry: 应调用 fs_delete 命令，参数包含 path', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await fs.deleteEntry('C:\\to-delete.txt');

    expect(spy).toHaveBeenCalledWith('fs_delete', { path: 'C:\\to-delete.txt' });
  });

  it('deleteEntry: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_delete') throw new Error('file locked');
    });

    await expect(fs.deleteEntry('C:\\locked.txt')).rejects.toThrow('file locked');
  });

  it('rename: 应调用 fs_rename 命令，参数包含 src 和 dst', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await fs.rename('C:\\old.txt', 'C:\\new.txt');

    expect(spy).toHaveBeenCalledWith('fs_rename', {
      src: 'C:\\old.txt',
      dst: 'C:\\new.txt',
    });
  });

  it('rename: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'fs_rename') throw new Error('target exists');
    });

    await expect(fs.rename('C:\\a.txt', 'C:\\b.txt')).rejects.toThrow('target exists');
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

  // ── 异常路径 ──────────────────────────────────────────────

  it('loadSettings: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'load_settings') throw new Error('config file corrupted');
    });

    await expect(settings.loadSettings()).rejects.toThrow('config file corrupted');
  });

  it('saveSettings: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'save_settings') throw new Error('permission denied');
    });

    await expect(settings.saveSettings({ theme: 'dark' })).rejects.toThrow(
      'permission denied',
    );
  });
});

// ═══════════════════════════════════════════════════════════════════
// Notify IPC
// ═══════════════════════════════════════════════════════════════════

describe('notify IPC 合约', () => {
  it('startWatch: 应调用 notify_watch 命令，参数包含 path', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await notify.startWatch('C:\\test-project');

    expect(spy).toHaveBeenCalledWith('notify_watch', {
      path: 'C:\\test-project',
    });
  });

  it('startWatch: 路径包含正斜杠时应原样传递', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
    });

    await notify.startWatch('D:/projects/my-app');

    expect(spy).toHaveBeenCalledWith('notify_watch', {
      path: 'D:/projects/my-app',
    });
  });

  // ── 异常路径 ──────────────────────────────────────────────

  it('startWatch: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'notify_watch') throw new Error('路径不存在');
    });

    await expect(notify.startWatch('C:\\nonexistent')).rejects.toThrow('路径不存在');
  });

  // ── onFsEvent（事件订阅）──────────────────────────────────

  it('onFsEvent: 应调用 listen("fs-event", callback)', () => {
    const mockUnlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(mockUnlisten);

    const cb = vi.fn();
    notify.onFsEvent(cb);

    expect(listen).toHaveBeenCalledWith("fs-event", expect.any(Function));
  });

  it('onFsEvent: listen 回调应解包 event.payload 传给 callback', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let capturedHandler: ((event: any) => void) | null = null;
    vi.mocked(listen).mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ((_event: string, handler: (event: any) => void) => {
        capturedHandler = handler;
        return Promise.resolve(vi.fn());
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any,
    );

    const cb = vi.fn();
    notify.onFsEvent(cb);

    const testPayload = { paths: ["C:/a.txt"], kind: "modify" as const };
    capturedHandler!({ payload: testPayload });
    expect(cb).toHaveBeenCalledWith(testPayload);
  });

  it('onFsEvent: 返回的 unsubscribe 调用后应触发 unlisten', async () => {
    const mockUnlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(mockUnlisten);

    const unsub = notify.onFsEvent(vi.fn());
    // unsub() 内部调 unlisten.then(fn => fn())，微任务刷新后 mockUnlisten 应被调用
    unsub();
    await Promise.resolve();

    expect(mockUnlisten).toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════
// Git IPC
// ═══════════════════════════════════════════════════════════════════

describe('git IPC 合约', () => {
  it('gitStatus: 应调用 git_status 命令，参数包含 repoPath', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'git_status') return [];
    });

    await git.gitStatus('C:\\test-repo');

    expect(spy).toHaveBeenCalledWith('git_status', {
      repoPath: 'C:\\test-repo',
    });
  });

  it('gitStatus: 返回 GitStatusEntry[] 数组', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_status') return [
        { path: 'D:/repo/src/main.tsx', status: 'modified' },
        { path: 'D:/repo/src/lib.rs', status: 'untracked' },
      ];
    });

    const entries = await git.gitStatus('D:\\repo');
    expect(entries).toHaveLength(2);
    expect(entries[0].path).toBe('D:/repo/src/main.tsx');
    expect(entries[0].status).toBe('modified');
    expect(entries[1].path).toBe('D:/repo/src/lib.rs');
    expect(entries[1].status).toBe('untracked');
  });

  it('gitStatus: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_status') throw new Error('not a git repository');
    });

    await expect(git.gitStatus('C:\\not-git')).rejects.toThrow('not a git repository');
  });

  it('gitDiff: 应调用 git_diff 命令，参数包含 repoPath 和 filePath', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'git_diff') return [];
    });

    await git.gitDiff('C:\\repo', 'C:\\repo\\src\\main.rs');

    expect(spy).toHaveBeenCalledWith('git_diff', {
      repoPath: 'C:\\repo',
      filePath: 'C:\\repo\\src\\main.rs',
    });
  });

  it('gitDiff: 返回 DiffHunk[] 数组', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_diff') return [
        { oldStart: 1, oldLines: 2, newStart: 1, newLines: 3 },
        { oldStart: 10, oldLines: 1, newStart: 11, newLines: 0 },
      ];
    });

    const hunks = await git.gitDiff('C:\\repo', 'C:\\repo\\src\\main.rs');
    expect(hunks).toHaveLength(2);
    expect(hunks[0].oldStart).toBe(1);
    expect(hunks[0].oldLines).toBe(2);
    expect(hunks[1].newLines).toBe(0);
  });

  it('gitDiff: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_diff') throw new Error('unborn branch');
    });

    await expect(git.gitDiff('C:\\empty', 'f.txt')).rejects.toThrow('unborn branch');
  });

  it('gitFileAtHead: 应调用 git_file_at_head 命令，参数包含 repoPath 和 filePath', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === 'git_file_at_head') return 'HEAD 文件内容';
    });

    const content = await git.gitFileAtHead('C:\\repo', 'C:\\repo\\src\\main.rs');

    expect(content).toBe('HEAD 文件内容');
    expect(spy).toHaveBeenCalledWith('git_file_at_head', {
      repoPath: 'C:\\repo',
      filePath: 'C:\\repo\\src\\main.rs',
    });
  });

  it('gitFileAtHead: 返回 HEAD 文件内容字符串', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_file_at_head') return 'line1\nline2\nline3\n';
    });

    const content = await git.gitFileAtHead('C:\\repo', 'C:\\repo\\f.txt');
    expect(content).toBe('line1\nline2\nline3\n');
  });

  it('gitFileAtHead: invoke 失败时异常应传播', async () => {
    mockIPC((cmd) => {
      if (cmd === 'git_file_at_head') throw new Error('文件在 HEAD 中不存在');
    });

    await expect(git.gitFileAtHead('C:\\repo', 'C:\\repo\\ghost.txt')).rejects.toThrow('文件在 HEAD 中不存在');
  });
});

// ═══════════════════════════════════════════════════════════════════
// Window IPC
// ═══════════════════════════════════════════════════════════════════

describe('window IPC 合约', () => {
  let mockDestroy: ReturnType<typeof vi.fn>;
  let mockOnCloseRequested: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockDestroy = vi.fn().mockResolvedValue(undefined);
    mockOnCloseRequested = vi.fn();
    vi.mocked(getCurrentWindow).mockReturnValue({
      onCloseRequested: mockOnCloseRequested,
      destroy: mockDestroy,
    } as unknown as ReturnType<typeof getCurrentWindow>);
  });

  it('registerCloseHandler: onCloseRequested 回调中调用 preventDefault', () => {
    mockOnCloseRequested.mockReturnValue(Promise.resolve(vi.fn()));
    windowIpc.registerCloseHandler(vi.fn().mockResolvedValue(undefined));

    const handler = mockOnCloseRequested.mock.calls[0][0] as (e: { preventDefault: () => void }) => void;
    const mockEvent = { preventDefault: vi.fn() };

    handler(mockEvent);
    expect(mockEvent.preventDefault).toHaveBeenCalled();
  });

  it('registerCloseHandler: cb 成功后调用 destroy', async () => {
    mockOnCloseRequested.mockReturnValue(Promise.resolve(vi.fn()));
    const cb = vi.fn().mockResolvedValue(undefined);
    windowIpc.registerCloseHandler(cb);

    const handler = mockOnCloseRequested.mock.calls[0][0] as (e: { preventDefault: () => void }) => Promise<void>;
    await handler({ preventDefault: vi.fn() });

    expect(cb).toHaveBeenCalled();
    expect(mockDestroy).toHaveBeenCalled();
  });

  it('registerCloseHandler: cb 抛错后 finally 中仍调 destroy', async () => {
    mockOnCloseRequested.mockReturnValue(Promise.resolve(vi.fn()));
    const cb = vi.fn().mockRejectedValue(new Error('save failed'));
    windowIpc.registerCloseHandler(cb);

    const handler = mockOnCloseRequested.mock.calls[0][0] as (e: { preventDefault: () => void }) => Promise<void>;
    try {
      await handler({ preventDefault: vi.fn() });
    } catch {
      // 预期：handler 因 cb reject 而整体 reject，但 finally 中 destroy 仍被调用
    }

    expect(mockDestroy).toHaveBeenCalled();
  });

  it('registerCloseHandler: 返回的清理函数调用 unlisten', async () => {
    const mockUnlisten = vi.fn();
    mockOnCloseRequested.mockReturnValue(Promise.resolve(mockUnlisten));

    const unsub = windowIpc.registerCloseHandler(vi.fn().mockResolvedValue(undefined));
    unsub();
    await Promise.resolve();

    expect(mockUnlisten).toHaveBeenCalled();
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
