// ipc-contract.test.ts — IPC wrapper 合约测试（IHE-06 工厂化）
//
// 经共享工厂 describeIpcContract（helpers/ipc-contract.ts）声明式驱动四维断言：
// 1. 命令名正确（snake_case）
// 2. 参数结构正确（字段名、类型；Channel 绑定 / Uint8Array 转换经 assertArgs）
// 3. 返回类型正确（透传 / void / fallback）
// 4. 异常传播
// onFsEvent 为 listen 事件封装——属"wrapper 行为契约"（IHE-01②），手写模拟驱动，
// 不走 invoke 工厂；window 生命周期用例（getCurrentWindow mock）同样保留手写。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、
// Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证，
// 真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { Channel } from '@tauri-apps/api/core';
import { describeIpcContract } from './helpers/ipc-contract';

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
import * as projects from '../ipc/projects';
import * as notify from '../ipc/notify';
import * as git from '../ipc/git';
import * as windowIpc from '../ipc/window';
// ping 测试用——index.ts 的 ping() wrapper（IHE-07①：非裸 invoke）
import { ping } from '../ipc';

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// PTY IPC
// ═══════════════════════════════════════════════════════════════════

const SPAWN_REQUEST = { panelId: 'p1', cwd: 'C:\\test', cols: 120, rows: 40 };
const HELLO_BYTES = new Uint8Array([72, 101, 108, 108, 111]); // "Hello"

describeIpcContract('pty IPC 合约', [
  {
    name: 'spawn: 应调用 pty_spawn 命令，参数包含 request 和 onOutput Channel',
    cmd: 'pty_spawn',
    call: () => pty.spawn(SPAWN_REQUEST, onOutputStub()),
    respond: 'mock-session-01',
    expectArgs: { request: SPAWN_REQUEST, onOutput: expect.any(Channel) },
    expectResult: 'mock-session-01',
    assertArgs: (args) => {
      // 验证 channel.onmessage 已绑定为 onOutput 回调
      expect((args.onOutput as Channel<unknown>).onmessage).toBe(
        lastOnOutput(),
      );
    },
  },
  {
    name: 'write: 应调用 pty_write 命令，data 从 Uint8Array 转换为 number[]',
    cmd: 'pty_write',
    call: () => pty.write('session-1', 'panel-1', HELLO_BYTES),
    expectArgs: {
      sessionId: 'session-1',
      panelId: 'panel-1',
      data: [72, 101, 108, 108, 111],
    },
  },
  {
    name: 'resize: 应调用 pty_resize 命令，参数包含 sessionId, panelId, cols, rows',
    cmd: 'pty_resize',
    call: () => pty.resize('session-2', 'panel-2', 100, 30),
    expectArgs: {
      sessionId: 'session-2',
      panelId: 'panel-2',
      cols: 100,
      rows: 30,
    },
  },
  {
    name: 'kill: 应调用 pty_kill 命令，参数包含 sessionId 和 panelId',
    cmd: 'pty_kill',
    call: () => pty.kill('session-3', 'panel-3'),
    expectArgs: { sessionId: 'session-3', panelId: 'panel-3' },
  },
  {
    name: 'getWindowsBuildNumber: 成功时返回 invoke 结果',
    cmd: 'get_windows_build_number',
    call: () => pty.getWindowsBuildNumber(),
    respond: 22621,
    expectResult: 22621,
  },
  {
    name: 'getWindowsBuildNumber: invoke 失败时 fallback 返回 21376',
    cmd: 'get_windows_build_number',
    call: () => pty.getWindowsBuildNumber(),
    mockThrow: 'Not on Windows',
    expectResult: 21376,
  },
  // ── 异常路径 ──────────────────────────────────────────────
  {
    name: 'spawn: invoke 失败时异常应传播给调用方',
    cmd: 'pty_spawn',
    call: () => pty.spawn({ panelId: 'p1', cwd: 'C:\\tmp', cols: 80, rows: 24 }, vi.fn()),
    mockThrow: 'conpty init failed',
    expectReject: 'conpty init failed',
  },
  {
    name: 'write: invoke 失败时异常应传播',
    cmd: 'pty_write',
    call: () => pty.write('session-1', 'panel-1', new Uint8Array([65])),
    mockThrow: 'session closed',
    expectReject: 'session closed',
  },
  {
    name: 'resize: invoke 失败时异常应传播',
    cmd: 'pty_resize',
    call: () => pty.resize('bad-session', 'panel-1', 100, 30),
    mockThrow: 'invalid session',
    expectReject: 'invalid session',
  },
  {
    name: 'kill: invoke 失败时异常应传播',
    cmd: 'pty_kill',
    call: () => pty.kill('dead-session', 'panel-1'),
    mockThrow: 'already dead',
    expectReject: 'already dead',
  },
]);

// ── PTY 命令 payload 契约守卫（DBG-4）──────────────────────
// 断言每个命令 invoke payload 的键集合精确匹配，
// 任何未来单边加/减键立即红。

describeIpcContract('PTY 命令 payload 契约守卫', [
  {
    name: 'pty_write: payload 键恰好为 sessionId/panelId/data',
    cmd: 'pty_write',
    call: () => pty.write('s1', 'p1', new Uint8Array([65])),
    expectExactKeys: ['data', 'panelId', 'sessionId'],
  },
  {
    name: 'pty_resize: payload 键恰好为 sessionId/panelId/cols/rows',
    cmd: 'pty_resize',
    call: () => pty.resize('s2', 'p2', 100, 30),
    expectExactKeys: ['cols', 'panelId', 'rows', 'sessionId'],
  },
  {
    name: 'pty_kill: payload 键恰好为 sessionId/panelId',
    cmd: 'pty_kill',
    call: () => pty.kill('s3', 'p3'),
    expectExactKeys: ['panelId', 'sessionId'],
  },
]);

// ═══════════════════════════════════════════════════════════════════
// FS IPC
// ═══════════════════════════════════════════════════════════════════

const READ_DIR_RESULT = [
  { name: 'src', path: 'C:/test/src', isDir: true },
  { name: 'README.md', path: 'C:/test/README.md', isDir: false, size: 1024, modified: 1700000000000 },
];

describeIpcContract('fs IPC 合约', [
  {
    name: 'writeFile: 应调用 fs_write_file 命令，参数包含 path 和 content',
    cmd: 'fs_write_file',
    call: () => fs.writeFile('C:\\output.txt', 'hello world'),
    expectArgs: { path: 'C:\\output.txt', content: 'hello world' },
  },
  // ── 异常路径 ──────────────────────────────────────────────
  {
    name: 'writeFile: invoke 失败时异常应传播',
    cmd: 'fs_write_file',
    call: () => fs.writeFile('C:\\full.txt', 'data'),
    mockThrow: 'disk full',
    expectReject: 'disk full',
  },
  // ── 目录操作 ──────────────────────────────────────────────
  {
    name: 'readDir: 应调用 fs_read_dir 命令，参数包含 path',
    cmd: 'fs_read_dir',
    call: () => fs.readDir('C:\\test'),
    respond: READ_DIR_RESULT,
    expectArgs: { path: 'C:\\test' },
    expectResult: READ_DIR_RESULT,
  },
  {
    name: 'readDir: invoke 失败时异常应传播',
    cmd: 'fs_read_dir',
    call: () => fs.readDir('C:\\nope'),
    mockThrow: 'path not found',
    expectReject: 'path not found',
  },
  {
    name: 'createDir: 应调用 fs_create_dir 命令，参数包含 path',
    cmd: 'fs_create_dir',
    call: () => fs.createDir('C:\\new-folder'),
    expectArgs: { path: 'C:\\new-folder' },
  },
  {
    name: 'createDir: invoke 失败时异常应传播',
    cmd: 'fs_create_dir',
    call: () => fs.createDir('C:\\protected'),
    mockThrow: 'permission denied',
    expectReject: 'permission denied',
  },
  {
    name: 'deleteEntry: 应调用 fs_delete 命令，参数包含 path',
    cmd: 'fs_delete',
    call: () => fs.deleteEntry('C:\\to-delete.txt'),
    expectArgs: { path: 'C:\\to-delete.txt' },
  },
  {
    name: 'deleteEntry: invoke 失败时异常应传播',
    cmd: 'fs_delete',
    call: () => fs.deleteEntry('C:\\locked.txt'),
    mockThrow: 'file locked',
    expectReject: 'file locked',
  },
  {
    name: 'rename: 应调用 fs_rename 命令，参数包含 src 和 dst',
    cmd: 'fs_rename',
    call: () => fs.rename('C:\\old.txt', 'C:\\new.txt'),
    expectArgs: { src: 'C:\\old.txt', dst: 'C:\\new.txt' },
  },
  {
    name: 'rename: invoke 失败时异常应传播',
    cmd: 'fs_rename',
    call: () => fs.rename('C:\\a.txt', 'C:\\b.txt'),
    mockThrow: 'target exists',
    expectReject: 'target exists',
  },
]);

// ── readFile（BE-03 Channel 分块）——wrapper 行为契约，手写驱动 ──
// readFile 的 resolve 值由 onChunk Channel 的 done 序列驱动（非 invoke 返回值），
// 工厂 respond/expectResult 无法表达，照 onFsEvent 先例手写。
// mockIPC 只守 JS 侧形状；Channel 真实序列化由 L4 E2E 守卫（IHE-01）。

describe('fs readFile Channel 合约（BE-03）', () => {
  /** 注册 mockIPC，捕获 readFile 的 invoke 参数并返回 onChunk Channel */
  function captureReadChannel(): {
    promise: Promise<string>;
    channel: Channel<fs.FsReadChunk>;
  } {
    let channel: Channel<fs.FsReadChunk> | null = null;
    mockIPC((_cmd, args) => {
      channel = (args as Record<string, unknown>).onChunk as Channel<fs.FsReadChunk>;
      return undefined;
    });
    const promise = fs.readFile('C:\\test.txt');
    expect(channel).not.toBeNull();
    return { promise, channel: channel! };
  }

  it('readFile: 应调用 fs_read_file 命令，payload 为 path + onChunk Channel，按 done 序列拼接完整字符串', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      return undefined;
    });

    const promise = fs.readFile('C:\\test.txt');
    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd, args] = spy.mock.calls[0] as [
      string,
      Record<string, unknown>,
    ];
    expect(cmd).toBe('fs_read_file');
    expect(args.path).toBe('C:\\test.txt');
    expect(args.onChunk).toBeInstanceOf(Channel);
    // payload 键恰好为 path/onChunk（防单边字段漂移）
    expect(Object.keys(args).sort()).toEqual(['onChunk', 'path']);

    // 驱动分块序列：若干 done:false + 终态 { data:"", done:true }
    const channel = args.onChunk as Channel<fs.FsReadChunk>;
    channel.onmessage({ data: 'Hello ', done: false });
    channel.onmessage({ data: 'World', done: false });
    channel.onmessage({ data: '', done: true });

    await expect(promise).resolves.toBe('Hello World');
  });

  it('readFile: 空文件（无数据块直接终态）返回空串', async () => {
    const { promise, channel } = captureReadChannel();
    channel.onmessage({ data: '', done: true });
    await expect(promise).resolves.toBe('');
  });

  it('readFile: invoke 失败时异常应传播', async () => {
    mockIPC(() => {
      throw new Error('access denied');
    });
    await expect(fs.readFile('C:\\protected.txt')).rejects.toThrow(
      'access denied',
    );
  });
});

// ═══════════════════════════════════════════════════════════════════
// Settings IPC
// ═══════════════════════════════════════════════════════════════════

describeIpcContract('settings IPC 合约', [
  {
    name: 'loadSettings: 应调用 load_settings 命令，无参数',
    cmd: 'load_settings',
    call: () => settings.loadSettings(),
    respond: { data: { theme: 'dark' }, corrupted: false },
    expectArgs: {},
    expectResult: { data: { theme: 'dark' }, corrupted: false },
  },
  {
    name: 'loadSettings: 无文件 → data:null, corrupted:false',
    cmd: 'load_settings',
    call: () => settings.loadSettings(),
    respond: { data: null, corrupted: false },
    expectResult: { data: null, corrupted: false },
  },
  {
    name: 'loadSettings: 文件损坏 → 透传 corrupted:true（FE-11/D11）',
    cmd: 'load_settings',
    call: () => settings.loadSettings(),
    respond: { data: null, corrupted: true },
    expectResult: { data: null, corrupted: true },
  },
  {
    name: 'saveSettings: 应调用 save_settings 命令，参数包含 settings',
    cmd: 'save_settings',
    call: () => settings.saveSettings({ fontSize: 16, theme: 'dark' }),
    expectArgs: { settings: { fontSize: 16, theme: 'dark' } },
  },
  {
    name: 'saveSettings: 空对象也正常调用 invoke',
    cmd: 'save_settings',
    call: () => settings.saveSettings({}),
    expectArgs: { settings: {} },
  },
  // ── 异常路径 ──────────────────────────────────────────────
  {
    name: 'loadSettings: invoke 失败时异常应传播',
    cmd: 'load_settings',
    call: () => settings.loadSettings(),
    mockThrow: 'config file corrupted',
    expectReject: 'config file corrupted',
  },
  {
    name: 'saveSettings: invoke 失败时异常应传播',
    cmd: 'save_settings',
    call: () => settings.saveSettings({ theme: 'dark' }),
    mockThrow: 'permission denied',
    expectReject: 'permission denied',
  },
]);

// ═══════════════════════════════════════════════════════════════════
// Projects IPC
// ═══════════════════════════════════════════════════════════════════

const PROJECTS_JSON = '{"projects":{"p1":{"name":"test"}}}';

describeIpcContract('projects IPC 合约', [
  // ── loadProjects ──────────────────────────────────────────
  {
    name: 'loadProjects: 应调用 load_projects 命令，无参数',
    cmd: 'load_projects',
    call: () => projects.loadProjects(),
    respond: { data: PROJECTS_JSON, corrupted: false },
    expectArgs: {},
    expectResult: { data: PROJECTS_JSON, corrupted: false },
  },
  {
    name: 'loadProjects: 无文件 → data 为空 JSON 对象, corrupted:false',
    cmd: 'load_projects',
    call: () => projects.loadProjects(),
    respond: { data: '{}', corrupted: false },
    expectResult: { data: '{}', corrupted: false },
  },
  {
    name: 'loadProjects: 文件损坏 → 透传 corrupted:true（FE-11/D11）',
    cmd: 'load_projects',
    call: () => projects.loadProjects(),
    respond: { data: '{}', corrupted: true },
    expectResult: { data: '{}', corrupted: true },
  },
  {
    name: 'loadProjects: invoke 失败时异常应传播',
    cmd: 'load_projects',
    call: () => projects.loadProjects(),
    mockThrow: 'disk error',
    expectReject: 'disk error',
  },
  // ── saveProjects ──────────────────────────────────────────
  {
    name: 'saveProjects: 应调用 save_projects 命令，参数包含 data',
    cmd: 'save_projects',
    call: () => projects.saveProjects(PROJECTS_JSON),
    expectArgs: { data: PROJECTS_JSON },
  },
  {
    name: 'saveProjects: invoke 失败时异常应传播',
    cmd: 'save_projects',
    call: () => projects.saveProjects('{}'),
    mockThrow: 'permission denied',
    expectReject: 'permission denied',
  },
]);

// ═══════════════════════════════════════════════════════════════════
// Notify IPC（startWatch 走 invoke 工厂；onFsEvent 为 listen 行为契约）
// ═══════════════════════════════════════════════════════════════════

describeIpcContract('notify IPC 合约', [
  {
    name: 'startWatch: 应调用 notify_watch 命令，参数包含 path',
    cmd: 'notify_watch',
    call: () => notify.startWatch('C:\\test-project'),
    expectArgs: { path: 'C:\\test-project' },
  },
  {
    name: 'startWatch: 路径包含正斜杠时应原样传递',
    cmd: 'notify_watch',
    call: () => notify.startWatch('D:/projects/my-app'),
    expectArgs: { path: 'D:/projects/my-app' },
  },
  // ── 异常路径 ──────────────────────────────────────────────
  {
    name: 'startWatch: invoke 失败时异常应传播',
    cmd: 'notify_watch',
    call: () => notify.startWatch('C:\\nonexistent'),
    mockThrow: '路径不存在',
    expectReject: '路径不存在',
  },
  // ── stopWatch（BE-10：项目移除/切换时停止监听）─────────────
  {
    name: 'stopWatch: 应调用 notify_stop_watch 命令，参数包含 path',
    cmd: 'notify_stop_watch',
    call: () => notify.stopWatch('C:\\test-project'),
    expectArgs: { path: 'C:\\test-project' },
  },
  {
    name: 'stopWatch: 路径包含正斜杠时应原样传递',
    cmd: 'notify_stop_watch',
    call: () => notify.stopWatch('D:/projects/my-app'),
    expectArgs: { path: 'D:/projects/my-app' },
  },
  {
    name: 'stopWatch: invoke 失败时异常应传播',
    cmd: 'notify_stop_watch',
    call: () => notify.stopWatch('C:\\nonexistent'),
    mockThrow: 'watcher 不存在',
    expectReject: 'watcher 不存在',
  },
]);

// ── onFsEvent（事件订阅）——wrapper 行为契约（IHE-01②）────────
// Tauri listen 的运行时解包（event.payload → callback）由 L4 E2E 守卫；
// 此处用模拟驱动断言 wrapper 自身的解包逻辑与清理语义。

describe('onFsEvent 合约（wrapper 行为契约）', () => {
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

const GIT_STATUS_RESULT = [
  { path: 'D:/repo/src/main.tsx', status: 'modified' },
  { path: 'D:/repo/src/lib.rs', status: 'untracked' },
];

const GIT_DIFF_RESULT = [
  { oldStart: 1, oldLines: 2, newStart: 1, newLines: 3 },
  { oldStart: 10, oldLines: 1, newStart: 11, newLines: 0 },
];

describeIpcContract('git IPC 合约', [
  {
    name: 'gitStatus: 应调用 git_status 命令，参数包含 repoPath',
    cmd: 'git_status',
    call: () => git.gitStatus('C:\\test-repo'),
    respond: [],
    expectArgs: { repoPath: 'C:\\test-repo' },
  },
  {
    name: 'gitStatus: 返回 GitStatusEntry[] 数组',
    cmd: 'git_status',
    call: () => git.gitStatus('D:\\repo'),
    respond: GIT_STATUS_RESULT,
    expectResult: GIT_STATUS_RESULT,
  },
  {
    name: 'gitStatus: invoke 失败时异常应传播',
    cmd: 'git_status',
    call: () => git.gitStatus('C:\\not-git'),
    mockThrow: 'not a git repository',
    expectReject: 'not a git repository',
  },
  {
    name: 'gitDiff: 应调用 git_diff 命令，参数包含 repoPath 和 filePath',
    cmd: 'git_diff',
    call: () => git.gitDiff('C:\\repo', 'C:\\repo\\src\\main.rs'),
    respond: [],
    expectArgs: { repoPath: 'C:\\repo', filePath: 'C:\\repo\\src\\main.rs' },
  },
  {
    name: 'gitDiff: 返回 DiffHunk[] 数组',
    cmd: 'git_diff',
    call: () => git.gitDiff('C:\\repo', 'C:\\repo\\src\\main.rs'),
    respond: GIT_DIFF_RESULT,
    expectResult: GIT_DIFF_RESULT,
  },
  {
    name: 'gitDiff: invoke 失败时异常应传播',
    cmd: 'git_diff',
    call: () => git.gitDiff('C:\\empty', 'f.txt'),
    mockThrow: 'unborn branch',
    expectReject: 'unborn branch',
  },
  {
    name: 'gitFileAtHead: 应调用 git_file_at_head 命令，参数包含 repoPath 和 filePath',
    cmd: 'git_file_at_head',
    call: () => git.gitFileAtHead('C:\\repo', 'C:\\repo\\src\\main.rs'),
    respond: 'HEAD 文件内容',
    expectArgs: { repoPath: 'C:\\repo', filePath: 'C:\\repo\\src\\main.rs' },
    expectResult: 'HEAD 文件内容',
  },
  {
    name: 'gitFileAtHead: 返回 HEAD 文件内容字符串',
    cmd: 'git_file_at_head',
    call: () => git.gitFileAtHead('C:\\repo', 'C:\\repo\\f.txt'),
    respond: 'line1\nline2\nline3\n',
    expectResult: 'line1\nline2\nline3\n',
  },
  {
    name: 'gitFileAtHead: invoke 失败时异常应传播',
    cmd: 'git_file_at_head',
    call: () => git.gitFileAtHead('C:\\repo', 'C:\\repo\\ghost.txt'),
    mockThrow: '文件在 HEAD 中不存在',
    expectReject: '文件在 HEAD 中不存在',
  },
  {
    name: 'gitRollback: 应调用 git_rollback 命令，参数包含 repoPath 和 filePath',
    cmd: 'git_rollback',
    call: () => git.gitRollback('C:\\repo', 'C:\\repo\\src\\a.txt'),
    expectArgs: { repoPath: 'C:\\repo', filePath: 'C:\\repo\\src\\a.txt' },
  },
  {
    name: 'gitRollback: 成功时返回 void 不抛异常',
    cmd: 'git_rollback',
    call: () => git.gitRollback('C:\\repo', 'C:\\repo\\a.txt'),
    respond: undefined,
    expectUndefined: true,
  },
  {
    name: 'gitRollback: invoke 失败时异常应传播',
    cmd: 'git_rollback',
    call: () => git.gitRollback('C:\\repo', 'C:\\repo\\ghost.txt'),
    mockThrow: '文件在 HEAD 中不存在',
    expectReject: '文件在 HEAD 中不存在',
  },
  {
    name: 'gitUnstage: 应调用 git_unstage 命令，参数包含 repoPath 和 filePath',
    cmd: 'git_unstage',
    call: () => git.gitUnstage('C:\\repo', 'C:\\repo\\src\\b.txt'),
    expectArgs: { repoPath: 'C:\\repo', filePath: 'C:\\repo\\src\\b.txt' },
  },
  {
    name: 'gitUnstage: 成功时返回 void 不抛异常',
    cmd: 'git_unstage',
    call: () => git.gitUnstage('C:\\repo', 'C:\\repo\\b.txt'),
    respond: undefined,
    expectUndefined: true,
  },
  {
    name: 'gitUnstage: invoke 失败时异常应传播',
    cmd: 'git_unstage',
    call: () => git.gitUnstage('C:\\repo', 'C:\\repo\\b.txt'),
    mockThrow: 'reset 文件失败',
    expectReject: 'reset 文件失败',
  },
  {
    name: 'gitUnstage: 参数 camelCase（repoPath 非 repo_path）',
    cmd: 'git_unstage',
    call: () => git.gitUnstage('C:\\repo', 'C:\\repo\\b.txt'),
    assertArgs: (args) => {
      expect(args).toHaveProperty('repoPath');
      expect(args).toHaveProperty('filePath');
      expect(args).not.toHaveProperty('repo_path');
    },
  },
]);

// ═══════════════════════════════════════════════════════════════════
// Window IPC（getCurrentWindow mock，保留手写）
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
// IPC ping（index.ts 的 ping() wrapper，IHE-07①）
// ═══════════════════════════════════════════════════════════════════

describe('IPC ping', () => {
  it('ping() wrapper 应调用 ping 命令并返回 pong', async () => {
    mockIPC((cmd) => {
      if (cmd === 'ping') return 'pong';
    });

    const result = await ping();
    expect(result).toBe('pong');
  });
});

// ── pty 用例辅助：onOutput stub 追踪 ─────────────────────────
// spawn 用例需要断言 Channel.onmessage 绑定到本次传入的 onOutput，
// 经模块级变量记录最近一次调用传入的回调。

let lastOutputStub: ((event: unknown) => void) | null = null;

function onOutputStub(): (event: unknown) => void {
  lastOutputStub = vi.fn(() => {});
  return lastOutputStub;
}

function lastOnOutput(): (event: unknown) => void {
  return lastOutputStub!;
}
