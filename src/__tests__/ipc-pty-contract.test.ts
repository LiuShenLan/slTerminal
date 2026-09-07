// ipc-pty-contract.test.ts — pty IPC 领域契约（CP-010 新增命令）
//
// 既有 pty_spawn/write/resize/kill 用例留在 ipc-contract.test.ts（旧共享文件），
// 本文件只承载 CP-010 新增的 pty_conpty_status 契约（命令名 + 无参 + 返回键集合
// camelCase 精确断言 + 异常传播），命名沿用 ipc-<domain>-contract.test.ts 分域约定。
//
// mockIPC 盲区（IHE-01）：只守 JS 侧形状——camelCase/snake_case 真实字段转换由
// L4 E2E 守卫（src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, afterEach, vi } from 'vitest';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { getConptyStatus } from '../ipc/pty';

afterEach(() => {
  clearMocks();
});

describe('pty IPC 合约: pty_conpty_status（CP-010）', () => {
  it('应调用 pty_conpty_status 命令，无参数', async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      return { attempted: true, bundled: true, fallbackReason: null };
    });

    await getConptyStatus();

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toBe('pty_conpty_status');
    // 无参命令：payload 为空对象
    expect(spy.mock.calls[0][1]).toEqual({});
  });

  it('返回键集合恰为 attempted/bundled/fallbackReason（camelCase 精确断言）', async () => {
    const response = { attempted: true, bundled: false, fallbackReason: 'load failed' };
    mockIPC(() => response);

    const result = await getConptyStatus();

    // 键集合精确断言：任何单边加/减键立即红（Rust serde camelCase 镜像）
    expect(Object.keys(result as object).sort()).toEqual([
      'attempted',
      'bundled',
      'fallbackReason',
    ]);
    // 字段透传 + fallbackReason 为 string（非 null 变体）
    expect(result).toEqual(response);
  });

  it('fallbackReason 为 null 时（无回退）原样透传', async () => {
    const response = { attempted: true, bundled: true, fallbackReason: null };
    mockIPC(() => response);

    await expect(getConptyStatus()).resolves.toEqual(response);
  });

  it('invoke 失败时异常应传播（调用方 catch 降级）', async () => {
    mockIPC(() => {
      throw new Error('command not registered');
    });

    await expect(getConptyStatus()).rejects.toThrow('command not registered');
  });
});
